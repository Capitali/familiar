//! The scenario generation engine (ADR-0010 stages 1–4, scaled past
//! hand-writing).
//!
//! A **family** is a parameterized template: `generate(params)` maps a seed
//! (plus optional key=value overrides) to a complete fixture. Determinism is a
//! tested contract, not a convention — same (family, params) → byte-identical
//! JSON, forever.
//!
//! The [`crate::det::SplitMix64`] stream varies **surface** only: file names,
//! sizes, thresholds, log content, tripwire strings. **Structure** — which
//! checks exist, and how each visible check is paired with hidden
//! counterparts — comes from the template, never from randomness. Anti-gaming
//! is by construction: every family pairs its visible checks with a
//! clean-state hidden re-run (the backup-spaces idiom ADR-0010's first live
//! run taught), idempotence checks, and sanity bounds on numeric knobs.
//!
//! Nothing leaves a generator unvalidated: [`generate_validated`] refuses its
//! own output if `validate::check` finds an Error — the gate is the same one
//! hand-written and LLM-authored fixtures pass.

mod authority_line;
mod process_repair;
mod resource_pressure;
mod service_loop;
mod variation_curriculum;

use crate::scenario::Scenario;
use crate::validate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;

/// A family's inputs: the seed drives all surface variation; `kv` carries
/// optional named overrides (documented per family). `BTreeMap` so iteration
/// order — and therefore output — is stable.
#[derive(Debug, Clone, Default)]
pub struct FamilyParams {
    pub seed: u64,
    pub kv: BTreeMap<String, String>,
}

impl FamilyParams {
    pub fn new(seed: u64) -> FamilyParams {
        FamilyParams {
            seed,
            kv: BTreeMap::new(),
        }
    }

    /// A numeric override with a default.
    pub fn get_u64(&self, key: &str, default: u64) -> u64 {
        self.kv
            .get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
}

/// What a generation produces: one or more fixtures, plus a curriculum
/// manifest when the family builds ordered sets (Stage 4).
#[derive(Debug, Clone)]
pub struct Generated {
    pub fixtures: Vec<Scenario>,
    pub curriculum: Option<Curriculum>,
}

/// An ordered fixture set for cross-fixture learning measurement (Stage 4).
/// `fixtures` are file names relative to the manifest's own directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curriculum {
    pub name: String,
    pub fixtures: Vec<String>,
}

/// A parameterized scenario template.
pub trait Family {
    /// Kebab-case name (the CLI key and output directory name).
    fn name(&self) -> &'static str;
    /// One line: what this family exercises, and its stage.
    fn describe(&self) -> &'static str;
    /// Deterministically generate. Same params → byte-identical output.
    fn generate(&self, params: &FamilyParams) -> Generated;
}

/// Every family, in stable order.
pub fn registry() -> Vec<Box<dyn Family>> {
    vec![
        Box::new(process_repair::ProcessRepair),
        Box::new(resource_pressure::ResourcePressure),
        Box::new(service_loop::ServiceLoop),
        Box::new(authority_line::AuthorityLine),
        Box::new(variation_curriculum::VariationCurriculum),
    ]
}

/// Find a family by name.
pub fn find(name: &str) -> Option<Box<dyn Family>> {
    registry().into_iter().find(|f| f.name() == name)
}

/// Generate and gate: refuses the family's own output if validation finds an
/// Error. A generator that can emit an invalid or leaking fixture is broken,
/// and the break must be loud.
pub fn generate_validated(family: &dyn Family, params: &FamilyParams) -> io::Result<Generated> {
    let out = family.generate(params);
    for s in &out.fixtures {
        let violations = validate::check(s)?;
        if validate::has_errors(&violations) {
            let detail: Vec<String> = violations
                .iter()
                .filter(|v| v.severity == validate::Severity::Error)
                .map(|v| v.to_string())
                .collect();
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "family {} seed {} generated an invalid fixture {}: {}",
                    family.name(),
                    params.seed,
                    s.id,
                    detail.join("; ")
                ),
            ));
        }
    }
    Ok(out)
}

/// The canonical serialization every generated fixture is written with —
/// the byte-identity contract is over exactly this.
pub fn emit(s: &Scenario) -> io::Result<String> {
    let mut body = serde_json::to_string_pretty(s).map_err(io::Error::other)?;
    body.push('\n');
    Ok(body)
}

/// A generated tripwire string: unmistakably secret-shaped, ≥ 8 chars, and
/// derived from the seed so every fixture's is distinct.
pub fn gen_secret(seed: u64, tag: &str) -> String {
    format!("GEN-SECRET-{tag}-{seed:016x}")
}

#[cfg(test)]
pub(crate) mod testing {
    //! Shared per-family test harness: byte-identity, validation, and the
    //! control-A floor (the investigate-only template must never solve a
    //! well-formed fixture).

    use super::*;
    use crate::harness::{run, Control, RunConfig};
    use std::fs;

    /// Golden + determinism + validity + A-floor, for one family and seed.
    pub fn family_invariants(family: &dyn Family, seed: u64) {
        let params = FamilyParams::new(seed);
        let a = generate_validated(family, &params).expect("must validate");
        let b = generate_validated(family, &params).expect("must validate");
        let ser = |g: &Generated| -> Vec<String> {
            g.fixtures.iter().map(|s| emit(s).unwrap()).collect()
        };
        // Byte-identity: the determinism contract, tested not conventional.
        assert_eq!(ser(&a), ser(&b), "{}: nondeterministic", family.name());
        assert!(!a.fixtures.is_empty());

        // Distinct seeds vary the surface (ids must differ at minimum).
        let other = generate_validated(family, &FamilyParams::new(seed + 1)).unwrap();
        assert_ne!(
            a.fixtures[0].id,
            other.fixtures[0].id,
            "{}: seed does not reach the id",
            family.name()
        );

        // The A floor: control A's investigate-only template must not pass.
        let t = std::env::temp_dir().join(format!(
            "familiar_gen_{}_{seed}_{}",
            family.name(),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&t);
        let cfg = RunConfig {
            lab_dir: t.clone(),
            episodes: 2,
            ..RunConfig::default()
        };
        for s in &a.fixtures {
            let report = run(s, Control::Baseline, &cfg).expect("A must run");
            assert_eq!(
                report.trials_to_success,
                None,
                "{}: control A solved {} — the fixture is trivial",
                family.name(),
                s.id
            );
        }
        let _ = fs::remove_dir_all(&t);
    }

    /// Golden-file byte identity: the emitted JSON must match the checked-in
    /// golden at `tests/golden/{family}-{seed}.json`. Run once with
    /// `GOLDEN_BLESS=1` to (re)write the golden after an intentional change.
    pub fn golden(family: &dyn Family, seed: u64) {
        let generated = generate_validated(family, &FamilyParams::new(seed)).unwrap();
        let emitted: String = generated
            .fixtures
            .iter()
            .map(|s| emit(s).unwrap())
            .collect::<Vec<_>>()
            .join("\n---\n");
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(format!("{}-{seed}.json", family.name()));
        if std::env::var_os("GOLDEN_BLESS").is_some() {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &emitted).unwrap();
            return;
        }
        let golden = fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "{} missing — run once with GOLDEN_BLESS=1 to create it",
                path.display()
            )
        });
        assert_eq!(
            emitted,
            golden,
            "{} seed {seed}: output drifted from the golden file",
            family.name()
        );
    }

    /// Assert a reference solution passes (solvability): running `script`
    /// under control B via a fake adapter must reach an external pass.
    pub fn reference_solution_passes(family: &dyn Family, seed: u64, script: &str) {
        let params = FamilyParams::new(seed);
        let generated = generate_validated(family, &params).unwrap();
        let s = &generated.fixtures[0];
        let t = std::env::temp_dir().join(format!(
            "familiar_gen_ref_{}_{seed}_{}",
            family.name(),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&t);
        fs::create_dir_all(&t).unwrap();
        let adapter = t.join("call_llm.sh");
        fs::write(
            &adapter,
            format!(
                "#!/bin/sh\nd=\"$(dirname \"$0\")\"\ncat > \"$d/response.json\" <<'RESP'\n```sh\n{script}\n```\nRESP\n"
            ),
        )
        .unwrap();
        let cfg = RunConfig {
            lab_dir: t.join("lab"),
            episodes: 1,
            llm_adapter: Some(adapter),
            ..RunConfig::default()
        };
        let report = run(s, Control::LlmOnly, &cfg).expect("B must run");
        assert_eq!(
            report.trials_to_success,
            Some(1),
            "{} seed {seed}: the reference solution does not pass — episodes: {:#?}",
            family.name(),
            report.episodes
        );
        let _ = fs::remove_dir_all(&t);
    }
}

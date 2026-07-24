//! `familiar-lab` — run scenario fixtures under the experimental controls.
//!
//!   familiar-lab run <fixture.json> [--control A|B|C|D] [--episodes N]
//!                    [--replicate N] [--lab DIR] [--llm-adapter PATH]
//!                    [--llm-required] [--llm-patience S] [--llm-backoff S]
//!                    [--adapter-timeout S]
//!   familiar-lab matrix <fixture.json> [same flags, all four controls]
//!   familiar-lab validate <fixture.json|DIR>
//!   familiar-lab list [DIR]
//!
//! `run` executes one control; `matrix` executes all four (A B C D) so the
//! ADR-0010 comparison — does accumulated experience beat starting from
//! scratch? — reads off a single table. Without an `--llm-adapter`, controls
//! B/D fall back to the deterministic template and the report says so.

use familiar_scenario::harness::{self, Control, RunConfig};
use familiar_scenario::{campaign, evidence, scenario, validate};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(&args[1..], false),
        Some("matrix") => run(&args[1..], true),
        Some("campaign") => campaign_cmd(&args[1..]),
        Some("report") => report_cmd(&args[1..]),
        Some("gen") => gen_cmd(&args[1..]),
        Some("curriculum") => curriculum_cmd(&args[1..]),
        Some("author") => author_cmd(&args[1..]),
        Some("promote") => promote_cmd(&args[1..]),
        Some("validate") => validate_cmd(args.get(1).map(String::as_str)),
        Some("list") => list(args.get(1).map(String::as_str)),
        _ => {
            eprintln!(
                "usage: familiar-lab run <fixture.json> [--control A|B|C|D] [--episodes N] \
                 [--replicate N] [--lab DIR] [--llm-adapter PATH] [--llm-required] \
                 [--llm-patience S] [--llm-backoff S] [--adapter-timeout S]\n       \
                 familiar-lab matrix <fixture.json> [same flags]\n       \
                 familiar-lab campaign <plan.json> [--resume] [--force]\n       \
                 familiar-lab report <dir> [--md PATH] [--json PATH]\n       \
                 familiar-lab gen <family> --seed N [--set k=v]... [--out DIR] | gen --list\n       \
                 familiar-lab curriculum <manifest.json> [--control A|B|C|D | --matrix] \
                 [--episodes N] [--lab DIR] [--llm-adapter PATH]\n       \
                 familiar-lab author <family> --brief PATH --count N --llm-adapter PATH \
                 [--out DIR]\n       \
                 familiar-lab promote <draft.json> [--library DIR]\n       \
                 familiar-lab validate <fixture.json|DIR>\n       \
                 familiar-lab list [DIR]"
            );
            ExitCode::from(2)
        }
    }
}

fn campaign_cmd(args: &[String]) -> ExitCode {
    let Some(plan_path) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a campaign plan (JSON) is required");
        return ExitCode::from(2);
    };
    let plan = match campaign::load_plan(Path::new(plan_path)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("familiar-lab: {e}");
            return ExitCode::FAILURE;
        }
    };
    let resume = args.iter().any(|a| a == "--resume");
    let force = args.iter().any(|a| a == "--force");
    match campaign::run(&plan, resume, force) {
        Ok(outcome) => {
            println!(
                "campaign {:?}: {} cells run, {} skipped, {} failed (state in {})",
                outcome.halt,
                outcome.cells_run,
                outcome.cells_skipped,
                outcome.cells_failed,
                plan.out.display()
            );
            match outcome.halt {
                campaign::Halt::Complete => ExitCode::SUCCESS,
                // Interrupted-but-resumable is not a failure, but it is not done.
                _ => ExitCode::from(3),
            }
        }
        Err(e) => {
            eprintln!("familiar-lab: campaign: {e}");
            ExitCode::FAILURE
        }
    }
}

fn report_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a directory of reports is required");
        return ExitCode::from(2);
    };
    match evidence::Evidence::collect(Path::new(dir)) {
        Ok(ev) if ev.scenarios.is_empty() => {
            eprintln!("no report.json files under {dir}");
            ExitCode::FAILURE
        }
        Ok(ev) => {
            print!("{}", ev.table());
            if let Some(md) = flag(args, "--md") {
                if let Err(e) = std::fs::write(&md, ev.markdown()) {
                    eprintln!("familiar-lab: writing {md}: {e}");
                    return ExitCode::FAILURE;
                }
                println!("markdown evidence written to {md}");
            }
            if let Some(json) = flag(args, "--json") {
                if let Err(e) = ev.save(Path::new(&json)) {
                    eprintln!("familiar-lab: writing {json}: {e}");
                    return ExitCode::FAILURE;
                }
                println!("json evidence written to {json}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("familiar-lab: report: {e}");
            ExitCode::FAILURE
        }
    }
}

/// All fixture JSON files under `root` (or `root` itself if it is a file).
fn fixtures_under(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    let mut stack = vec![root.to_path_buf()];
    let mut fixtures = Vec::new();
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "json")
                && !p.to_string_lossy().ends_with(".curriculum.json")
            {
                fixtures.push(p);
            }
        }
    }
    fixtures.sort();
    fixtures
}

fn author_cmd(args: &[String]) -> ExitCode {
    use familiar_scenario::author;
    let Some(family) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a family name is required");
        return ExitCode::from(2);
    };
    let Some(brief_path) = flag(args, "--brief") else {
        eprintln!("familiar-lab: author requires --brief PATH (the family design brief)");
        return ExitCode::from(2);
    };
    let Some(adapter) = flag(args, "--llm-adapter").map(PathBuf::from) else {
        eprintln!("familiar-lab: author requires --llm-adapter PATH");
        return ExitCode::from(2);
    };
    let count: u32 = flag(args, "--count")
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let out =
        PathBuf::from(flag(args, "--out").unwrap_or_else(|| format!("scenarios/drafts/{family}")));
    let brief = match std::fs::read_to_string(&brief_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("familiar-lab: {brief_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The canonical example rides the prompt: the first shipped fixture.
    let example = match std::fs::read_to_string("scenarios/process-failures/backup-spaces.json") {
        Ok(b) => b,
        Err(e) => {
            eprintln!("familiar-lab: reading the example fixture: {e}");
            return ExitCode::FAILURE;
        }
    };
    // An authoring work dir with an LLM-open boundary and the adapter installed.
    let work = out.join(".author-work");
    if let Err(e) = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(work.join("llm"))?;
        std::fs::copy(&adapter, work.join("llm").join("call_llm.sh"))?;
        if let Some(key) = adapter.parent().map(|p| p.join("key.env")) {
            if key.is_file() {
                std::fs::copy(&key, work.join("llm").join("key.env"))?;
            }
        }
        let mut b = familiar_kernel::boundary::Boundary::closed();
        b.phase = "scenario-author".to_string();
        b.allow_llm = true;
        std::fs::write(
            work.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_string_pretty(&b).map_err(std::io::Error::other)?,
        )
    })() {
        eprintln!("familiar-lab: preparing author workdir: {e}");
        return ExitCode::FAILURE;
    }

    let mut kept = 0;
    for i in 1..=count {
        let prompt = author::drafting_prompt(family, &brief, &example, i);
        let response = match familiar_llm::consult(&work, &prompt) {
            Ok(familiar_llm::Outcome::Response(r)) => r,
            Ok(familiar_llm::Outcome::RateLimited(why)) => {
                eprintln!("familiar-lab: draft {i}: rate-limited ({why}) — stopping here");
                break;
            }
            Ok(familiar_llm::Outcome::Refused(why)) => {
                eprintln!("familiar-lab: draft {i}: {why}");
                continue;
            }
            Err(e) => {
                eprintln!("familiar-lab: draft {i}: {e}");
                continue;
            }
        };
        let (mut scenario, reference) = match author::parse_draft(&response) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("draft {i}: rejected at parse: {e}");
                continue;
            }
        };
        scenario.provenance = format!("llm:draft-{i}");
        match author::gate_candidate(&scenario, &reference) {
            Ok(report) => {
                let verdict = if report.passed() {
                    "SURVIVED"
                } else {
                    "failed gates"
                };
                println!(
                    "draft {i} ({}): {verdict} — validate={} leak={} gaming={} solvable={}",
                    scenario.id,
                    report.validate.passed,
                    report.leak_audit.passed,
                    report.gaming_probe.passed,
                    report.solvability.passed
                );
                if let Err(e) = author::quarantine(&out, &scenario, &report) {
                    eprintln!("familiar-lab: quarantining draft {i}: {e}");
                }
                if report.passed() {
                    kept += 1;
                }
            }
            Err(e) => eprintln!("draft {i}: gating error: {e}"),
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    println!(
        "{kept}/{count} drafts survived the gates — quarantined under {} (promote to enter the library)",
        out.display()
    );
    ExitCode::SUCCESS
}

fn promote_cmd(args: &[String]) -> ExitCode {
    use familiar_scenario::author;
    let Some(draft) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a draft path is required");
        return ExitCode::from(2);
    };
    let library =
        PathBuf::from(flag(args, "--library").unwrap_or_else(|| "scenarios/authored".to_string()));
    match author::promote(Path::new(draft), &library) {
        Ok(report) if report.passed() => {
            println!(
                "{}: promoted into {} (all gates re-passed)",
                report.id,
                library.display()
            );
            ExitCode::SUCCESS
        }
        Ok(report) => {
            eprintln!("{}: NOT promoted — gates failed:", report.id);
            for (name, g) in [
                ("validate", &report.validate),
                ("leak-audit", &report.leak_audit),
                ("gaming-probe", &report.gaming_probe),
                ("solvability", &report.solvability),
            ] {
                eprintln!("  {name}: {} — {}", g.passed, g.detail);
            }
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("familiar-lab: promote: {e}");
            ExitCode::FAILURE
        }
    }
}

fn curriculum_cmd(args: &[String]) -> ExitCode {
    use familiar_scenario::gen::Curriculum;
    let Some(manifest_path) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a curriculum manifest (JSON) is required");
        return ExitCode::from(2);
    };
    let manifest_path = Path::new(manifest_path);
    let manifest: Curriculum = match std::fs::read_to_string(manifest_path)
        .and_then(|b| serde_json::from_str(&b).map_err(std::io::Error::other))
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("familiar-lab: {}: {e}", manifest_path.display());
            return ExitCode::FAILURE;
        }
    };
    let base = manifest_path.parent().unwrap_or(Path::new("."));
    let mut scenarios = Vec::new();
    for rel in &manifest.fixtures {
        match scenario::load(&base.join(rel)) {
            Ok(s) => scenarios.push(s),
            Err(e) => {
                eprintln!("familiar-lab: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let cfg = RunConfig {
        lab_dir: PathBuf::from(
            flag(args, "--lab").unwrap_or_else(|| format!("lab-runs/{}", manifest.name)),
        ),
        episodes: flag(args, "--episodes")
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
        llm_adapter: flag(args, "--llm-adapter").map(PathBuf::from),
        ..RunConfig::default()
    };
    let controls: Vec<Control> = if args.iter().any(|a| a == "--matrix") {
        vec![
            Control::Baseline,
            Control::LlmOnly,
            Control::NoMemory,
            Control::Full,
        ]
    } else {
        let spec = flag(args, "--control").unwrap_or_else(|| "D".to_string());
        match Control::parse(&spec) {
            Some(c) => vec![c],
            None => {
                eprintln!("familiar-lab: unknown control {spec:?}");
                return ExitCode::from(2);
            }
        }
    };
    println!(
        "curriculum {} · {} fixtures",
        manifest.name,
        scenarios.len()
    );
    for control in controls {
        match harness::run_sequence(&scenarios, control, &cfg) {
            Ok(reports) => {
                for r in &reports {
                    println!("p{} {}", r.sequence_position, r.summary_line());
                }
            }
            Err(e) => {
                eprintln!("familiar-lab: control {} failed: {e}", control.letter());
                return ExitCode::FAILURE;
            }
        }
    }
    println!(
        "\nreports under {} (familiar-lab report shows the curves)",
        cfg.lab_dir.display()
    );
    ExitCode::SUCCESS
}

fn gen_cmd(args: &[String]) -> ExitCode {
    use familiar_scenario::gen;
    if args.first().map(String::as_str) == Some("--list") || args.is_empty() {
        for f in gen::registry() {
            println!("{:<24} {}", f.name(), f.describe());
        }
        return ExitCode::SUCCESS;
    }
    let name = &args[0];
    let Some(family) = gen::find(name) else {
        eprintln!("familiar-lab: unknown family {name:?} (see gen --list)");
        return ExitCode::from(2);
    };
    let Some(seed) = flag(args, "--seed").and_then(|v| v.parse().ok()) else {
        eprintln!("familiar-lab: gen requires --seed N");
        return ExitCode::from(2);
    };
    let mut params = gen::FamilyParams::new(seed);
    let mut i = 0;
    while let Some(pos) = args[i..].iter().position(|a| a == "--set") {
        let at = i + pos;
        match args.get(at + 1).and_then(|kv| kv.split_once('=')) {
            Some((k, v)) => {
                params.kv.insert(k.to_string(), v.to_string());
            }
            None => {
                eprintln!("familiar-lab: --set requires k=v");
                return ExitCode::from(2);
            }
        }
        i = at + 2;
    }
    let out_root = PathBuf::from(
        flag(args, "--out").unwrap_or_else(|| format!("scenarios/generated/{}", family.name())),
    );
    let generated = match gen::generate_validated(family.as_ref(), &params) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("familiar-lab: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&out_root) {
        eprintln!("familiar-lab: {e}");
        return ExitCode::FAILURE;
    }
    for s in &generated.fixtures {
        let path = out_root.join(format!("{}.json", s.id));
        match gen::emit(s).and_then(|body| std::fs::write(&path, body)) {
            Ok(()) => println!("{}", path.display()),
            Err(e) => {
                eprintln!("familiar-lab: writing {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    if let Some(curriculum) = &generated.curriculum {
        let path = out_root.join(format!("{}.curriculum.json", curriculum.name));
        let body = match serde_json::to_string_pretty(curriculum) {
            Ok(mut b) => {
                b.push('\n');
                b
            }
            Err(e) => {
                eprintln!("familiar-lab: {e}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(e) = std::fs::write(&path, body) {
            eprintln!("familiar-lab: writing {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("{}", path.display());
    }
    ExitCode::SUCCESS
}

fn validate_cmd(target: Option<&str>) -> ExitCode {
    let root = PathBuf::from(target.unwrap_or("scenarios"));
    let fixtures = fixtures_under(&root);
    if fixtures.is_empty() {
        eprintln!("no fixtures under {}", root.display());
        return ExitCode::FAILURE;
    }
    let mut errors = 0;
    for p in fixtures {
        match validate::check_file(&p) {
            Ok((_, violations)) => {
                let verdict = if validate::has_errors(&violations) {
                    errors += 1;
                    "INVALID"
                } else if violations.is_empty() {
                    "ok"
                } else {
                    "ok (warnings)"
                };
                println!("{:<44} {}", p.display(), verdict);
                for v in &violations {
                    println!("    {v}");
                }
            }
            Err(e) => {
                errors += 1;
                println!("{:<44} INVALID — {e}", p.display());
            }
        }
    }
    if errors > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn run(args: &[String], matrix: bool) -> ExitCode {
    let Some(fixture) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a fixture path is required");
        return ExitCode::from(2);
    };
    let scn = match scenario::load(Path::new(fixture)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("familiar-lab: {e}");
            return ExitCode::FAILURE;
        }
    };
    let episodes = flag(args, "--episodes")
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let mut cfg = RunConfig {
        lab_dir: PathBuf::from(flag(args, "--lab").unwrap_or_else(|| "lab-runs".to_string())),
        episodes,
        llm_adapter: flag(args, "--llm-adapter").map(PathBuf::from),
        replicate: flag(args, "--replicate")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        llm_required: args.iter().any(|a| a == "--llm-required"),
        ..RunConfig::default()
    };
    if let Some(s) = flag(args, "--llm-patience").and_then(|v| v.parse().ok()) {
        cfg.llm_patience_secs = s;
    }
    if let Some(s) = flag(args, "--llm-backoff").and_then(|v| v.parse().ok()) {
        cfg.llm_retry_backoff_secs = s;
    }
    if let Some(s) = flag(args, "--adapter-timeout").and_then(|v| v.parse().ok()) {
        cfg.adapter_timeout_secs = s;
    }
    if let Some(list) = flag(args, "--ablate") {
        for name in list.split(',').filter(|s| !s.trim().is_empty()) {
            match harness::Ablation::parse(name) {
                Some(a) => cfg.ablations.push(a),
                None => {
                    eprintln!("familiar-lab: unknown ablation {name:?}");
                    return ExitCode::from(2);
                }
            }
        }
        if cfg.ablations.contains(&harness::Ablation::FixedThreshold) {
            eprintln!("note: fixed-threshold is reserved (the lab gate has no rigor knob) — no-op");
        }
        // The Law III ablation executes boundary-violating artifacts in-lab
        // (sandboxed, violations recorded). It never runs implicitly.
        if cfg.ablations.contains(&harness::Ablation::Law3Gate)
            && !args.iter().any(|a| a == "--acknowledge-law3-ablation")
        {
            eprintln!(
                "familiar-lab: --ablate law3-gate disables the constitutional boundary gate \
                 for this run; pass --acknowledge-law3-ablation to confirm"
            );
            return ExitCode::from(2);
        }
    }
    if let Some(spec) = flag(args, "--noise") {
        match familiar_scenario::noise::NoiseSpec::parse(&spec) {
            Ok(n) => cfg.noise = Some(n),
            Err(e) => {
                eprintln!("familiar-lab: {e}");
                return ExitCode::from(2);
            }
        }
    }
    let controls: Vec<Control> = if matrix {
        vec![
            Control::Baseline,
            Control::LlmOnly,
            Control::NoMemory,
            Control::Full,
        ]
    } else {
        let spec = flag(args, "--control").unwrap_or_else(|| "D".to_string());
        match Control::parse(&spec) {
            Some(c) => vec![c],
            None => {
                eprintln!("familiar-lab: unknown control {spec:?} (use A, B, C, or D)");
                return ExitCode::from(2);
            }
        }
    };

    if cfg.llm_adapter.is_none() && controls.iter().any(|c| *c != Control::Baseline) {
        eprintln!(
            "note: no --llm-adapter — LLM controls will fall back to the deterministic \
             template (reported honestly as llm_used=false)"
        );
    }

    println!(
        "scenario {} · family {} · goal: {}",
        scn.id, scn.family, scn.visible_goal
    );
    let mut failed = false;
    for control in controls {
        match harness::run(&scn, control, &cfg) {
            Ok(report) => {
                println!("\n{}", report.summary_line());
                print!("{}", report.table());
            }
            Err(e) => {
                eprintln!("familiar-lab: control {} failed: {e}", control.letter());
                failed = true;
            }
        }
    }
    println!("\nreports under {}", cfg.lab_dir.display());
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn list(dir: Option<&str>) -> ExitCode {
    let root = PathBuf::from(dir.unwrap_or("scenarios"));
    let mut found = 0;
    for p in fixtures_under(&root) {
        match scenario::load(&p) {
            Ok(s) => {
                found += 1;
                let validity = match validate::check(&s) {
                    Ok(v) if validate::has_errors(&v) => "INVALID",
                    Ok(v) if !v.is_empty() => "warn",
                    Ok(_) => "ok",
                    Err(_) => "unchecked",
                };
                println!(
                    "{:<44} {:<8} {:<24} {} — {}",
                    p.display(),
                    validity,
                    s.family,
                    s.id,
                    s.visible_goal
                );
            }
            Err(e) => eprintln!("{}: INVALID — {e}", p.display()),
        }
    }
    if found == 0 {
        eprintln!("no fixtures under {}", root.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

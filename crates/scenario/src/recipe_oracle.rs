//! External truth oracles for Capability Recipe candidates (ADR-0040 §5).
//!
//! The candidate owns its [`OutputContract`] and recipe. The fixture owns every
//! recorded input and expected outcome. During replay the recipe sees only the
//! recorded tool responses, never the expected answers. This keeps truth outside
//! the candidate while exercising the same v1 capability boundary as deployment.
//!
//! Live evidence is deliberately unrepresentable here: [`EvidenceKind`] has only
//! `fixture_replay`. Live runs may inform post-deploy health, but cannot earn oracle
//! eligibility.

use familiar_recipe::{
    execute, ProvenToolSource, Recipe, RecipeError, RecipeOutput, Scalar, ToolSourceError,
    MAX_INPUTS,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

pub const ORACLE_FIXTURE_VERSION: u16 = 1;
pub const MAX_FIXTURE_BYTES: usize = 1024 * 1024;
pub const MAX_OUTPUT_CONTRACT_BYTES: usize = 16 * 1024;
pub const MAX_VARIANTS: usize = 64;
pub const MAX_CALLS_PER_VARIANT: usize = 16;
pub const MAX_RECORDED_TOOL_BYTES: usize = 256 * 1024;

/// Recipe-oracle JSON shares the repository's `scenarios/` root but is not an
/// ADR-0010 miniature-world fixture. World runners use this predicate to keep
/// the two strict schemas from being confused during recursive discovery.
pub fn is_recipe_oracle_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "json")
        && path
            .components()
            .any(|component| component.as_os_str() == "recipe-oracles")
}

/// The candidate's review-visible claim: what observation shape it emits and
/// which proven-tool inputs it derives that observation from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputContract {
    pub actor: String,
    pub action: String,
    pub inputs: Vec<ContractInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractInput {
    pub name: String,
    pub tool_id: String,
}

/// Parse the candidate-owned declaration under its own allocation ceiling.
pub fn parse_output_contract(bytes: &[u8]) -> Result<OutputContract, OracleError> {
    if bytes.len() > MAX_OUTPUT_CONTRACT_BYTES {
        return Err(OracleError::ContractTooLarge {
            used: bytes.len(),
            limit: MAX_OUTPUT_CONTRACT_BYTES,
        });
    }
    let contract: OutputContract = serde_json::from_slice(bytes)
        .map_err(|error| OracleError::ContractParse(error.to_string()))?;
    contract.validate()?;
    Ok(contract)
}

impl OutputContract {
    pub fn validate(&self) -> Result<(), OracleError> {
        validate_literal("contract actor", &self.actor, 128)
            .map_err(OracleError::InvalidContract)?;
        validate_literal("contract action", &self.action, 128)
            .map_err(OracleError::InvalidContract)?;
        if self.inputs.is_empty() || self.inputs.len() > MAX_INPUTS {
            return Err(OracleError::InvalidContract(format!(
                "output contract must declare 1..={MAX_INPUTS} inputs"
            )));
        }

        let mut names = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for input in &self.inputs {
            validate_slot_name(&input.name).map_err(OracleError::InvalidContract)?;
            validate_tool_id(&input.tool_id).map_err(OracleError::InvalidContract)?;
            if !names.insert(input.name.clone()) {
                return Err(OracleError::InvalidContract(format!(
                    "output contract repeats input name {}",
                    input.name
                )));
            }
            if !sources.insert((input.name.clone(), input.tool_id.clone())) {
                return Err(OracleError::InvalidContract(format!(
                    "output contract repeats input source {} -> {}",
                    input.name, input.tool_id
                )));
            }
        }
        Ok(())
    }
}

/// Only deterministic, fixture-owned replay can issue an eligibility verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    FixtureReplay,
}

/// Ground truth held by the scenario laboratory, outside the candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleFixture {
    pub version: u16,
    pub evidence: EvidenceKind,
    pub id: String,
    pub variants: Vec<OracleVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleVariant {
    pub id: String,
    pub kind: VariantKind,
    pub calls: Vec<RecordedCall>,
    pub expected: ExpectedOutcome,
    /// Fixture-owned service value. It ranks eligible survivors; it can never
    /// buy eligibility for a candidate that fails a truth gate.
    pub usefulness: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantKind {
    Baseline,
    Unchanged,
    Changed,
    Null,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedCall {
    pub tool_id: String,
    #[serde(default)]
    pub args: BTreeMap<String, Scalar>,
    pub result: RecordedToolResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordedToolResult {
    Text { text: String },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExpectedOutcome {
    Emit {
        actor: String,
        action: String,
        object: String,
        context: String,
    },
    Silent,
    Error {
        class: RecipeErrorClass,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeErrorClass {
    Manifest,
    Tool,
    NonUtf8,
    Limit,
    Transform,
    Template,
}

/// Parse a strict fixture only after applying a byte ceiling.
pub fn parse_fixture(bytes: &[u8]) -> Result<OracleFixture, OracleError> {
    if bytes.len() > MAX_FIXTURE_BYTES {
        return Err(OracleError::FixtureTooLarge {
            used: bytes.len(),
            limit: MAX_FIXTURE_BYTES,
        });
    }
    let fixture: OracleFixture =
        serde_json::from_slice(bytes).map_err(|error| OracleError::Parse(error.to_string()))?;
    fixture.validate()?;
    Ok(fixture)
}

impl OracleFixture {
    pub fn validate(&self) -> Result<(), OracleError> {
        let invalid = |message: String| OracleError::InvalidFixture(message);
        if self.version != ORACLE_FIXTURE_VERSION {
            return Err(invalid(format!(
                "unsupported fixture version {}; expected {ORACLE_FIXTURE_VERSION}",
                self.version
            )));
        }
        validate_literal("fixture id", &self.id, 128).map_err(OracleError::InvalidFixture)?;
        if self.variants.is_empty() || self.variants.len() > MAX_VARIANTS {
            return Err(invalid(format!(
                "fixture must contain 1..={MAX_VARIANTS} variants"
            )));
        }

        let mut ids = BTreeSet::new();
        let mut counts = BTreeMap::new();
        let mut baseline = None;
        for (index, variant) in self.variants.iter().enumerate() {
            validate_literal("variant id", &variant.id, 128)
                .map_err(OracleError::InvalidFixture)?;
            if !ids.insert(variant.id.clone()) {
                return Err(invalid(format!("duplicate variant id {}", variant.id)));
            }
            *counts.entry(variant.kind).or_insert(0_usize) += 1;
            if variant.calls.is_empty() || variant.calls.len() > MAX_CALLS_PER_VARIANT {
                return Err(invalid(format!(
                    "variant {} must contain 1..={MAX_CALLS_PER_VARIANT} recorded calls",
                    variant.id
                )));
            }
            for call in &variant.calls {
                validate_tool_id(&call.tool_id).map_err(OracleError::InvalidFixture)?;
                for value in call.args.values() {
                    if matches!(value, Scalar::Number(number) if !number.is_finite()) {
                        return Err(invalid(format!(
                            "variant {} has a non-finite call argument",
                            variant.id
                        )));
                    }
                }
                match &call.result {
                    RecordedToolResult::Text { text } => {
                        if text.len() > MAX_RECORDED_TOOL_BYTES {
                            return Err(invalid(format!(
                                "variant {} tool response exceeds {MAX_RECORDED_TOOL_BYTES} bytes",
                                variant.id
                            )));
                        }
                    }
                    RecordedToolResult::Error { message } => {
                        validate_literal("recorded tool error", message, 512)
                            .map_err(OracleError::InvalidFixture)?;
                    }
                }
            }
            validate_expected(&variant.expected)?;

            match variant.kind {
                VariantKind::Baseline => {
                    if index != 0 || baseline.is_some() {
                        return Err(invalid(
                            "the single baseline variant must be first".to_string(),
                        ));
                    }
                    if !matches!(&variant.expected, ExpectedOutcome::Emit { .. }) {
                        return Err(invalid(
                            "baseline must expect an emitted observation".into(),
                        ));
                    }
                    baseline = Some(&variant.expected);
                }
                VariantKind::Unchanged => {
                    if variant.expected != ExpectedOutcome::Silent || variant.usefulness != 0 {
                        return Err(invalid(format!(
                            "unchanged variant {} must expect silence with zero usefulness",
                            variant.id
                        )));
                    }
                }
                VariantKind::Changed => {
                    if !matches!(&variant.expected, ExpectedOutcome::Emit { .. }) {
                        return Err(invalid(format!(
                            "changed variant {} must expect an emitted observation",
                            variant.id
                        )));
                    }
                }
                VariantKind::Null => {
                    if matches!(&variant.expected, ExpectedOutcome::Silent) {
                        return Err(invalid(format!(
                            "null variant {} must report an honest value or typed error",
                            variant.id
                        )));
                    }
                }
                VariantKind::Malformed => {
                    if !matches!(&variant.expected, ExpectedOutcome::Error { .. }) {
                        return Err(invalid(format!(
                            "malformed variant {} must expect a typed error",
                            variant.id
                        )));
                    }
                }
            }
            if variant.kind != VariantKind::Unchanged && variant.usefulness == 0 {
                return Err(invalid(format!(
                    "material variant {} must carry positive usefulness",
                    variant.id
                )));
            }
        }

        for kind in [
            VariantKind::Baseline,
            VariantKind::Unchanged,
            VariantKind::Changed,
            VariantKind::Null,
            VariantKind::Malformed,
        ] {
            if counts.get(&kind).copied().unwrap_or(0) == 0 {
                return Err(invalid(format!("fixture needs a {} variant", kind.label())));
            }
        }
        if counts.get(&VariantKind::Baseline).copied().unwrap_or(0) != 1 {
            return Err(invalid(
                "fixture must contain exactly one baseline variant".to_string(),
            ));
        }

        let baseline = baseline.expect("baseline count was validated");
        for variant in &self.variants {
            if matches!(
                variant.kind,
                VariantKind::Changed | VariantKind::Null | VariantKind::Malformed
            ) && &variant.expected == baseline
            {
                return Err(invalid(format!(
                    "{} variant {} does not discriminate from baseline truth",
                    variant.kind.label(),
                    variant.id
                )));
            }
        }
        Ok(())
    }
}

impl VariantKind {
    fn label(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Unchanged => "unchanged",
            Self::Changed => "changed",
            Self::Null => "null",
            Self::Malformed => "malformed",
        }
    }

    fn is_material(self) -> bool {
        self != Self::Unchanged
    }

    fn is_discrimination_probe(self) -> bool {
        matches!(self, Self::Changed | Self::Null | Self::Malformed)
    }
}

fn validate_expected(expected: &ExpectedOutcome) -> Result<(), OracleError> {
    if let ExpectedOutcome::Emit {
        actor,
        action,
        object,
        context,
    } = expected
    {
        validate_literal("expected actor", actor, 128).map_err(OracleError::InvalidFixture)?;
        validate_literal("expected action", action, 128).map_err(OracleError::InvalidFixture)?;
        if object.len() > MAX_RECORDED_TOOL_BYTES || context.len() > MAX_RECORDED_TOOL_BYTES {
            return Err(OracleError::InvalidFixture(
                "expected observation field exceeds fixture byte ceiling".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_literal(kind: &str, value: &str, max: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > max {
        return Err(format!("{kind} must be nonempty and fit {max} bytes"));
    }
    Ok(())
}

fn validate_slot_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let starts = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    let rest = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if starts && rest && name.len() <= 64 {
        Ok(())
    } else {
        Err(format!(
            "input name {name:?} must match [a-z][a-z0-9_]* and fit 64 bytes"
        ))
    }
}

fn validate_tool_id(tool_id: &str) -> Result<(), String> {
    let valid = !tool_id.is_empty()
        && tool_id.len() <= 128
        && tool_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    if valid {
        Ok(())
    } else {
        Err(format!("tool id {tool_id:?} is not an opaque library id"))
    }
}

/// The post-persistence observation outcome. `Silent` means the changed-only
/// observation seam suppressed a duplicate; it is never a substitute for an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObservedOutcome {
    Emit {
        actor: String,
        action: String,
        object: String,
        context: String,
    },
    Silent,
    Error {
        class: RecipeErrorClass,
    },
}

impl From<RecipeOutput> for ObservedOutcome {
    fn from(output: RecipeOutput) -> Self {
        Self::Emit {
            actor: output.actor,
            action: output.action,
            object: output.object,
            context: output.context,
        }
    }
}

/// One externally observed candidate run. This public shape also lets scenario
/// adapters for future artifact tiers use the same truth scorer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayRun {
    pub variant_id: String,
    pub outcome: ObservedOutcome,
    /// True only when calls matched the fixture transcript exactly, with no
    /// undeclared, reordered, or missing invocation.
    pub transcript_ok: bool,
    /// Deterministic work units, never elapsed wall time.
    pub cost: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Eligibility {
    BoundaryFailure,
    ExecutionFailure,
    TruthFailure,
    Eligible,
}

impl Eligibility {
    fn rank(self) -> u8 {
        match self {
            Self::BoundaryFailure => 0,
            Self::ExecutionFailure => 1,
            Self::TruthFailure => 2,
            Self::Eligible => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckTally {
    pub passed: u32,
    pub total: u32,
}

impl CheckTally {
    pub fn complete(self) -> bool {
        self.total > 0 && self.passed == self.total
    }
}

/// The external verdict. The four truth dimensions remain separate evidence,
/// not a weighted average that a candidate can game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleScore {
    pub boundary_ok: bool,
    pub execution_ok: bool,
    pub contract_ok: bool,
    pub accuracy: CheckTally,
    pub coverage: CheckTally,
    pub quietness: CheckTally,
    pub discrimination: CheckTally,
    pub usefulness: u64,
    pub cost: u64,
    pub eligibility: Eligibility,
    pub violations: Vec<String>,
    pub runs: Vec<ReplayRun>,
}

impl OracleScore {
    pub fn truth_ok(&self) -> bool {
        self.contract_ok
            && self.accuracy.complete()
            && self.coverage.complete()
            && self.quietness.complete()
            && self.discrimination.complete()
    }
}

/// Score externally produced replay outcomes. The fixture is validated first and
/// runs must correspond one-for-one, in fixture order, so omitted hard cases fail
/// closed instead of shrinking a denominator.
pub fn score_replay(
    fixture: &OracleFixture,
    boundary_ok: bool,
    contract_ok: bool,
    runs: &[ReplayRun],
) -> Result<OracleScore, OracleError> {
    fixture.validate()?;
    if runs.len() != fixture.variants.len() {
        return Err(OracleError::InvalidRun(format!(
            "received {} runs for {} fixture variants",
            runs.len(),
            fixture.variants.len()
        )));
    }
    for (variant, run) in fixture.variants.iter().zip(runs) {
        if run.variant_id != variant.id {
            return Err(OracleError::InvalidRun(format!(
                "run {} does not match fixture variant {}",
                run.variant_id, variant.id
            )));
        }
    }

    let baseline = runs
        .first()
        .map(|run| &run.outcome)
        .expect("validated fixture has a baseline");
    let mut accuracy = CheckTally {
        passed: 0,
        total: 0,
    };
    let mut coverage = CheckTally {
        passed: 0,
        total: 0,
    };
    let mut quietness = CheckTally {
        passed: 0,
        total: 0,
    };
    let mut discrimination = CheckTally {
        passed: 0,
        total: 0,
    };
    let mut usefulness = 0_u64;
    let mut cost = 0_u64;
    let mut execution_ok = true;
    let mut violations = Vec::new();

    if !boundary_ok {
        violations.push("recipe capability boundary was not valid before replay".to_string());
    }
    if !contract_ok {
        violations.push("candidate output contract does not match its recipe".to_string());
    }

    for (variant, run) in fixture.variants.iter().zip(runs) {
        let matches = outcome_matches(&variant.expected, &run.outcome);
        cost = cost
            .checked_add(run.cost)
            .ok_or_else(|| OracleError::InvalidRun("candidate cost overflows u64".to_string()))?;

        if !run.transcript_ok {
            execution_ok = false;
            violations.push(format!(
                "variant {} did not consume the fixture transcript exactly",
                variant.id
            ));
        }
        if let ObservedOutcome::Error { class } = &run.outcome {
            if !matches!(
                &variant.expected,
                ExpectedOutcome::Error {
                    class: expected_class
                } if expected_class == class
            ) {
                execution_ok = false;
                violations.push(format!(
                    "variant {} failed unexpectedly with {class:?}",
                    variant.id
                ));
            }
        }

        if matches!(&variant.expected, ExpectedOutcome::Emit { .. }) {
            accuracy.total += 1;
            if matches {
                accuracy.passed += 1;
            }
        }
        if variant.kind.is_material() {
            coverage.total += 1;
            if matches {
                coverage.passed += 1;
                usefulness = usefulness
                    .checked_add(u64::from(variant.usefulness))
                    .ok_or_else(|| {
                        OracleError::InvalidRun("candidate usefulness overflows u64".to_string())
                    })?;
            }
        }
        if variant.kind == VariantKind::Unchanged {
            quietness.total += 1;
            if matches {
                quietness.passed += 1;
            }
        }
        if variant.kind.is_discrimination_probe() {
            discrimination.total += 1;
            if matches && run.outcome != *baseline {
                discrimination.passed += 1;
            }
        }
    }

    let mut score = OracleScore {
        boundary_ok,
        execution_ok,
        contract_ok,
        accuracy,
        coverage,
        quietness,
        discrimination,
        usefulness,
        cost,
        eligibility: Eligibility::TruthFailure,
        violations,
        runs: runs.to_vec(),
    };
    score.eligibility = if !score.boundary_ok {
        Eligibility::BoundaryFailure
    } else if !score.execution_ok {
        Eligibility::ExecutionFailure
    } else if !score.truth_ok() {
        Eligibility::TruthFailure
    } else {
        Eligibility::Eligible
    };
    Ok(score)
}

fn outcome_matches(expected: &ExpectedOutcome, observed: &ObservedOutcome) -> bool {
    match (expected, observed) {
        (
            ExpectedOutcome::Emit {
                actor: expected_actor,
                action: expected_action,
                object: expected_object,
                context: expected_context,
            },
            ObservedOutcome::Emit {
                actor,
                action,
                object,
                context,
            },
        ) => {
            actor == expected_actor
                && action == expected_action
                && object == expected_object
                && context == expected_context
        }
        (ExpectedOutcome::Silent, ObservedOutcome::Silent) => true,
        (
            ExpectedOutcome::Error {
                class: expected_class,
            },
            ObservedOutcome::Error { class },
        ) => class == expected_class,
        _ => false,
    }
}

/// Run a Recipe v1 candidate against the hidden transcript variants. Successful
/// repeated observations pass through the changed-only persistence seam: an exact
/// duplicate becomes `Silent`. A hard-coded candidate therefore goes silent on a
/// changed variant and fails accuracy, coverage, and discrimination.
pub fn evaluate_recipe(
    fixture: &OracleFixture,
    contract: &OutputContract,
    recipe: &Recipe,
) -> Result<OracleScore, OracleError> {
    fixture.validate()?;
    let contract_ok = contract.validate().is_ok() && contract_matches_recipe(contract, recipe);
    let boundary_ok = recipe.validate().is_ok();

    if !boundary_ok {
        let runs = fixture
            .variants
            .iter()
            .map(|variant| ReplayRun {
                variant_id: variant.id.clone(),
                outcome: ObservedOutcome::Error {
                    class: RecipeErrorClass::Manifest,
                },
                transcript_ok: false,
                cost: 0,
            })
            .collect::<Vec<_>>();
        return score_replay(fixture, false, contract_ok, &runs);
    }

    let mut last_emitted = None;
    let mut runs = Vec::with_capacity(fixture.variants.len());
    for variant in &fixture.variants {
        let mut source = FixtureToolSource::new(&variant.calls);
        let raw = match execute(recipe, &mut source) {
            Ok(output) => ObservedOutcome::from(output),
            Err(error) => ObservedOutcome::Error {
                class: classify_recipe_error(&error),
            },
        };
        let outcome = match raw {
            ObservedOutcome::Emit { .. } if last_emitted.as_ref() == Some(&raw) => {
                ObservedOutcome::Silent
            }
            ObservedOutcome::Emit { .. } => {
                last_emitted = Some(raw.clone());
                raw
            }
            _ => raw,
        };
        let cost = u64::try_from(recipe.steps.len())
            .unwrap_or(u64::MAX)
            .saturating_add(source.attempts);
        runs.push(ReplayRun {
            variant_id: variant.id.clone(),
            outcome,
            transcript_ok: source.transcript_ok(),
            cost,
        });
    }
    score_replay(fixture, true, contract_ok, &runs)
}

fn contract_matches_recipe(contract: &OutputContract, recipe: &Recipe) -> bool {
    if contract.actor != recipe.emit.actor || contract.action != recipe.emit.action {
        return false;
    }
    let actual = recipe
        .inputs
        .iter()
        .map(|input| ContractInput {
            name: input.name.clone(),
            tool_id: input.tool_id.clone(),
        })
        .collect::<Vec<_>>();
    contract.inputs == actual
}

fn classify_recipe_error(error: &RecipeError) -> RecipeErrorClass {
    match error {
        RecipeError::ManifestTooLarge { .. }
        | RecipeError::Parse(_)
        | RecipeError::InvalidDocument(_) => RecipeErrorClass::Manifest,
        RecipeError::Tool { .. } => RecipeErrorClass::Tool,
        RecipeError::NonUtf8Input { .. } => RecipeErrorClass::NonUtf8,
        RecipeError::LimitExceeded { .. } => RecipeErrorClass::Limit,
        RecipeError::Step { .. } => RecipeErrorClass::Transform,
        RecipeError::Template(_) => RecipeErrorClass::Template,
    }
}

struct FixtureToolSource<'a> {
    calls: &'a [RecordedCall],
    cursor: usize,
    attempts: u64,
    mismatch: bool,
}

impl<'a> FixtureToolSource<'a> {
    fn new(calls: &'a [RecordedCall]) -> Self {
        Self {
            calls,
            cursor: 0,
            attempts: 0,
            mismatch: false,
        }
    }

    fn transcript_ok(&self) -> bool {
        !self.mismatch && self.cursor == self.calls.len()
    }
}

impl ProvenToolSource for FixtureToolSource<'_> {
    fn invoke(
        &mut self,
        tool_id: &str,
        args: &BTreeMap<String, Scalar>,
    ) -> Result<Vec<u8>, ToolSourceError> {
        self.attempts = self.attempts.saturating_add(1);
        let Some(call) = self.calls.get(self.cursor) else {
            self.mismatch = true;
            return Err(ToolSourceError::new(format!(
                "unrecorded fixture call to {tool_id}"
            )));
        };
        if call.tool_id != tool_id || call.args != *args {
            self.mismatch = true;
            return Err(ToolSourceError::new(format!(
                "fixture expected {} with different identity or arguments",
                call.tool_id
            )));
        }
        self.cursor += 1;
        match &call.result {
            RecordedToolResult::Text { text } => Ok(text.as_bytes().to_vec()),
            RecordedToolResult::Error { message } => Err(ToolSourceError::new(message.clone())),
        }
    }
}

/// Lexicographic selection: boundary → execution → all truth gates. Only eligible
/// survivors may be ranked by fixture usefulness and then lower deterministic cost.
/// `Greater` means `a` outranks `b`.
pub fn compare_scores(a: &OracleScore, b: &OracleScore) -> Ordering {
    match a.eligibility.rank().cmp(&b.eligibility.rank()) {
        Ordering::Equal => {}
        ordering => return ordering,
    }
    if a.eligibility != Eligibility::Eligible {
        return Ordering::Equal;
    }
    match a.usefulness.cmp(&b.usefulness) {
        Ordering::Equal => b.cost.cmp(&a.cost),
        ordering => ordering,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleError {
    FixtureTooLarge { used: usize, limit: usize },
    ContractTooLarge { used: usize, limit: usize },
    Parse(String),
    ContractParse(String),
    InvalidFixture(String),
    InvalidContract(String),
    InvalidRun(String),
}

impl fmt::Display for OracleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FixtureTooLarge { used, limit } => {
                write!(f, "oracle fixture is {used} bytes; limit is {limit}")
            }
            Self::ContractTooLarge { used, limit } => {
                write!(f, "output contract is {used} bytes; limit is {limit}")
            }
            Self::Parse(message) => write!(f, "oracle fixture parse failed: {message}"),
            Self::ContractParse(message) => write!(f, "output contract parse failed: {message}"),
            Self::InvalidFixture(message) => write!(f, "invalid oracle fixture: {message}"),
            Self::InvalidContract(message) => write!(f, "invalid output contract: {message}"),
            Self::InvalidRun(message) => write!(f, "invalid oracle run: {message}"),
        }
    }
}

impl std::error::Error for OracleError {}

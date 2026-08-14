//! Capability Recipe v1 — bounded composition without ambient authority.
//!
//! A recipe may name proven library tools by opaque id and transform their returned
//! bytes. It cannot name an executable, path, URL, clock, environment variable, or
//! network destination. The caller supplies the only authority seam, [`ProvenToolSource`];
//! this crate itself performs no I/O.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const RECIPE_VERSION: u16 = 1;
pub const MAX_RECIPE_BYTES: usize = 64 * 1024;
pub const MAX_INPUTS: usize = 16;
pub const MAX_STEPS: usize = 64;
pub const MAX_ROWS: usize = 10_000;
pub const MAX_MATERIALIZED_BYTES: usize = 4 * 1024 * 1024;

/// The complete authored artifact. Unknown fields are always errors, including in every
/// nested operation and template part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub version: u16,
    pub inputs: Vec<Input>,
    pub steps: Vec<Step>,
    pub emit: Emit,
    pub limits: Limits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub name: String,
    pub tool_id: String,
    #[serde(default)]
    pub args: BTreeMap<String, Scalar>,
}

/// JSON-scalar arguments and comparison values. JSON cannot express non-finite numbers;
/// validation also rejects them when a recipe is built directly in Rust.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Bool(bool),
    Number(f64),
    String(String),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Contains,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathSegment {
    Field(FieldSegment),
    Index(IndexSegment),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSegment {
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexSegment {
    pub index: usize,
}

/// Tagged operations make the authored program mechanical to inspect. The newtype
/// payload structs carry `deny_unknown_fields`, which keeps that strictness at every op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Step {
    ParseJson(UnaryStep),
    ParseLines(UnaryStep),
    Select(SelectStep),
    Map(MapStep),
    Filter(FilterStep),
    Group(GroupStep),
    Count(UnaryStep),
    Min(AggregateStep),
    Max(AggregateStep),
    Mean(AggregateStep),
    Compare(CompareStep),
    Format(FormatStep),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnaryStep {
    pub from: String,
    pub save_as: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectStep {
    pub from: String,
    pub path: Vec<PathSegment>,
    pub save_as: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapStep {
    pub from: String,
    pub fields: BTreeMap<String, MapExpr>,
    pub save_as: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MapExpr {
    Field(FieldExpr),
    Literal(LiteralExpr),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldExpr {
    pub path: Vec<PathSegment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiteralExpr {
    pub literal: Scalar,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterStep {
    pub from: String,
    pub predicate: Predicate,
    pub save_as: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predicate {
    pub path: Vec<PathSegment>,
    pub comparison: Comparison,
    pub value: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupStep {
    pub from: String,
    pub path: Vec<PathSegment>,
    pub save_as: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AggregateStep {
    pub from: String,
    pub path: Vec<PathSegment>,
    pub save_as: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompareStep {
    pub from: String,
    pub comparison: Comparison,
    pub value: Scalar,
    pub save_as: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatStep {
    pub segments: Vec<TemplateSegment>,
    pub save_as: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Emit {
    pub actor: String,
    pub action: String,
    pub object_template: Template,
    pub context_template: Template,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub segments: Vec<TemplateSegment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TemplateSegment {
    Literal(LiteralSegment),
    Slot(SlotSegment),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiteralSegment {
    pub literal: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotSegment {
    pub slot: String,
    #[serde(default)]
    pub path: Vec<PathSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub rows: usize,
    pub bytes: usize,
    pub steps: usize,
}

/// A failure returned by the caller-owned proven-tool catalog. Unknown, unhealthy, or
/// unreviewed ids should all fail here; the interpreter cannot bypass this seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSourceError {
    pub message: String,
}

impl ToolSourceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// The only effectful seam. Implementations resolve opaque ids in the proven tool
/// library, enforce the tool's existing boundary gates, and return its bytes.
pub trait ProvenToolSource {
    fn invoke(
        &mut self,
        tool_id: &str,
        args: &BTreeMap<String, Scalar>,
    ) -> Result<Vec<u8>, ToolSourceError>;
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputLineage {
    pub name: String,
    pub tool_id: String,
    pub args: BTreeMap<String, Scalar>,
}

/// Observation-shaped output. Persistence and timestamping remain caller decisions,
/// keeping evaluation replayable and side-effect free.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecipeOutput {
    pub actor: String,
    pub action: String,
    pub object: String,
    pub context: String,
    pub inputs: Vec<InputLineage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeError {
    ManifestTooLarge {
        used: usize,
        limit: usize,
    },
    Parse(String),
    InvalidDocument(String),
    Tool {
        input: String,
        tool_id: String,
        message: String,
    },
    NonUtf8Input {
        input: String,
    },
    LimitExceeded {
        kind: &'static str,
        used: usize,
        limit: usize,
    },
    Step {
        index: usize,
        message: String,
    },
    Template(String),
}

impl fmt::Display for RecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestTooLarge { used, limit } => {
                write!(f, "recipe manifest is {used} bytes; limit is {limit}")
            }
            Self::Parse(message) => write!(f, "recipe parse failed: {message}"),
            Self::InvalidDocument(message) => write!(f, "invalid recipe: {message}"),
            Self::Tool {
                input,
                tool_id,
                message,
            } => write!(f, "input {input} ({tool_id}) failed: {message}"),
            Self::NonUtf8Input { input } => write!(f, "input {input} did not return UTF-8"),
            Self::LimitExceeded { kind, used, limit } => {
                write!(f, "recipe {kind} limit exceeded: {used} > {limit}")
            }
            Self::Step { index, message } => write!(f, "step {index} failed: {message}"),
            Self::Template(message) => write!(f, "emit template failed: {message}"),
        }
    }
}

impl std::error::Error for RecipeError {}

/// Check the manifest envelope before serde allocates its nested document, then parse and
/// validate the complete program before any tool can be invoked.
pub fn parse_recipe(bytes: &[u8]) -> Result<Recipe, RecipeError> {
    if bytes.len() > MAX_RECIPE_BYTES {
        return Err(RecipeError::ManifestTooLarge {
            used: bytes.len(),
            limit: MAX_RECIPE_BYTES,
        });
    }
    let recipe: Recipe =
        serde_json::from_slice(bytes).map_err(|error| RecipeError::Parse(error.to_string()))?;
    recipe.validate()?;
    Ok(recipe)
}

impl Recipe {
    /// Validate both parsed recipes and values constructed directly by Rust callers.
    pub fn validate(&self) -> Result<(), RecipeError> {
        let invalid = |message: String| RecipeError::InvalidDocument(message);
        if self.version != RECIPE_VERSION {
            return Err(invalid(format!(
                "unsupported version {}; expected {RECIPE_VERSION}",
                self.version
            )));
        }
        validate_limit("rows", self.limits.rows, MAX_ROWS)?;
        validate_limit("bytes", self.limits.bytes, MAX_MATERIALIZED_BYTES)?;
        validate_limit("steps", self.limits.steps, MAX_STEPS)?;
        if self.inputs.len() > MAX_INPUTS {
            return Err(invalid(format!(
                "{} inputs exceed hard ceiling {MAX_INPUTS}",
                self.inputs.len()
            )));
        }
        if self.steps.len() > self.limits.steps {
            return Err(invalid(format!(
                "{} steps exceed declared limit {}",
                self.steps.len(),
                self.limits.steps
            )));
        }

        let mut slots = BTreeSet::new();
        for input in &self.inputs {
            validate_name("input", &input.name)?;
            validate_tool_id(&input.tool_id)?;
            if !slots.insert(input.name.clone()) {
                return Err(invalid(format!("duplicate slot {}", input.name)));
            }
            for (name, value) in &input.args {
                validate_name("argument", name)?;
                validate_scalar(value)?;
            }
        }
        for (index, step) in self.steps.iter().enumerate() {
            validate_step(index, step, &mut slots)?;
        }
        validate_literal("actor", &self.emit.actor)?;
        validate_literal("action", &self.emit.action)?;
        validate_template("object_template", &self.emit.object_template, &slots)?;
        validate_template("context_template", &self.emit.context_template, &slots)?;
        Ok(())
    }
}

fn validate_limit(name: &'static str, value: usize, ceiling: usize) -> Result<(), RecipeError> {
    if value == 0 {
        return Err(RecipeError::InvalidDocument(format!(
            "{name} limit must be positive"
        )));
    }
    if value > ceiling {
        return Err(RecipeError::InvalidDocument(format!(
            "{name} limit {value} exceeds hard ceiling {ceiling}"
        )));
    }
    Ok(())
}

fn validate_name(kind: &str, name: &str) -> Result<(), RecipeError> {
    let mut chars = name.chars();
    let starts = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    let rest = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !starts || !rest || name.len() > 64 {
        return Err(RecipeError::InvalidDocument(format!(
            "{kind} name {name:?} must match [a-z][a-z0-9_]* and fit 64 bytes"
        )));
    }
    Ok(())
}

fn validate_tool_id(tool_id: &str) -> Result<(), RecipeError> {
    let valid = !tool_id.is_empty()
        && tool_id.len() <= 128
        && tool_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    if !valid {
        return Err(RecipeError::InvalidDocument(format!(
            "tool id {tool_id:?} is not an opaque library id"
        )));
    }
    Ok(())
}

fn validate_literal(kind: &str, value: &str) -> Result<(), RecipeError> {
    if value.trim().is_empty() || value.len() > 128 {
        return Err(RecipeError::InvalidDocument(format!(
            "emit {kind} must be nonempty and fit 128 bytes"
        )));
    }
    Ok(())
}

fn validate_scalar(value: &Scalar) -> Result<(), RecipeError> {
    if let Scalar::Number(number) = value {
        if !number.is_finite() {
            return Err(RecipeError::InvalidDocument(
                "scalar numbers must be finite".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_path(kind: &str, path: &[PathSegment], allow_empty: bool) -> Result<(), RecipeError> {
    if path.is_empty() && !allow_empty {
        return Err(RecipeError::InvalidDocument(format!(
            "{kind} path must not be empty"
        )));
    }
    for segment in path {
        if let PathSegment::Field(field) = segment {
            if field.field.is_empty()
                || field.field.len() > 128
                || field.field.chars().any(char::is_control)
            {
                return Err(RecipeError::InvalidDocument(format!(
                    "{kind} field names must be nonempty, control-free, and fit 128 bytes"
                )));
            }
        }
    }
    Ok(())
}

fn validate_from(from: &str, slots: &BTreeSet<String>) -> Result<(), RecipeError> {
    if !slots.contains(from) {
        return Err(RecipeError::InvalidDocument(format!(
            "slot {from} is unknown or a forward reference"
        )));
    }
    Ok(())
}

fn add_output(save_as: &str, slots: &mut BTreeSet<String>) -> Result<(), RecipeError> {
    validate_name("output", save_as)?;
    if !slots.insert(save_as.to_string()) {
        return Err(RecipeError::InvalidDocument(format!(
            "duplicate slot {save_as}"
        )));
    }
    Ok(())
}

fn validate_step(
    _index: usize,
    step: &Step,
    slots: &mut BTreeSet<String>,
) -> Result<(), RecipeError> {
    match step {
        Step::ParseJson(step) | Step::ParseLines(step) | Step::Count(step) => {
            validate_from(&step.from, slots)?;
            add_output(&step.save_as, slots)
        }
        Step::Select(step) => {
            validate_from(&step.from, slots)?;
            validate_path("select", &step.path, false)?;
            add_output(&step.save_as, slots)
        }
        Step::Map(step) => {
            validate_from(&step.from, slots)?;
            if step.fields.is_empty() {
                return Err(RecipeError::InvalidDocument(
                    "map fields must not be empty".to_string(),
                ));
            }
            for (name, expr) in &step.fields {
                validate_name("map field", name)?;
                match expr {
                    MapExpr::Field(expr) => validate_path("map", &expr.path, false)?,
                    MapExpr::Literal(expr) => validate_scalar(&expr.literal)?,
                }
            }
            add_output(&step.save_as, slots)
        }
        Step::Filter(step) => {
            validate_from(&step.from, slots)?;
            validate_path("filter", &step.predicate.path, false)?;
            validate_scalar(&step.predicate.value)?;
            add_output(&step.save_as, slots)
        }
        Step::Group(step) => {
            validate_from(&step.from, slots)?;
            validate_path("group", &step.path, false)?;
            add_output(&step.save_as, slots)
        }
        Step::Min(step) | Step::Max(step) | Step::Mean(step) => {
            validate_from(&step.from, slots)?;
            validate_path("aggregate", &step.path, false)?;
            add_output(&step.save_as, slots)
        }
        Step::Compare(step) => {
            validate_from(&step.from, slots)?;
            validate_scalar(&step.value)?;
            add_output(&step.save_as, slots)
        }
        Step::Format(step) => {
            validate_segments("format", &step.segments, slots)?;
            add_output(&step.save_as, slots)
        }
    }
}

fn validate_template(
    kind: &str,
    template: &Template,
    slots: &BTreeSet<String>,
) -> Result<(), RecipeError> {
    validate_segments(kind, &template.segments, slots)
}

fn validate_segments(
    kind: &str,
    segments: &[TemplateSegment],
    slots: &BTreeSet<String>,
) -> Result<(), RecipeError> {
    if segments.is_empty() {
        return Err(RecipeError::InvalidDocument(format!(
            "{kind} segments must not be empty"
        )));
    }
    for segment in segments {
        if let TemplateSegment::Slot(slot) = segment {
            validate_from(&slot.slot, slots)?;
            validate_path(kind, &slot.path, true)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum RuntimeValue {
    Text(String),
    Json(JsonValue),
    Rows(Vec<Row>),
    Groups(BTreeMap<String, Group>),
    Number(f64),
    Bool(bool),
    Null,
}

type Row = BTreeMap<String, JsonValue>;

#[derive(Debug, Clone)]
struct Group {
    label: Scalar,
    rows: Vec<Row>,
}

impl RuntimeValue {
    fn kind(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Json(_) => "json",
            Self::Rows(_) => "rows",
            Self::Groups(_) => "groups",
            Self::Number(_) => "number",
            Self::Bool(_) => "boolean",
            Self::Null => "null",
        }
    }

    fn row_count(&self) -> usize {
        match self {
            Self::Rows(rows) => rows.len(),
            Self::Groups(groups) => groups.values().map(|group| group.rows.len()).sum(),
            _ => 0,
        }
    }

    fn materialized_bytes(&self) -> Result<usize, String> {
        match self {
            Self::Text(text) => Ok(text.len()),
            _ => serde_json::to_vec(&self.to_json()?)
                .map(|bytes| bytes.len())
                .map_err(|error| error.to_string()),
        }
    }

    fn to_json(&self) -> Result<JsonValue, String> {
        match self {
            Self::Text(text) => Ok(JsonValue::String(text.clone())),
            Self::Json(value) => Ok(value.clone()),
            Self::Rows(rows) => Ok(JsonValue::Array(
                rows.iter().cloned().map(row_to_json).collect(),
            )),
            Self::Groups(groups) => {
                let mut out = Vec::with_capacity(groups.len());
                for group in groups.values() {
                    let mut record = serde_json::Map::new();
                    record.insert("group".to_string(), scalar_to_json(&group.label)?);
                    record.insert(
                        "rows".to_string(),
                        JsonValue::Array(group.rows.iter().cloned().map(row_to_json).collect()),
                    );
                    out.push(JsonValue::Object(record));
                }
                Ok(JsonValue::Array(out))
            }
            Self::Number(number) => number_to_json(*number),
            Self::Bool(value) => Ok(JsonValue::Bool(*value)),
            Self::Null => Ok(JsonValue::Null),
        }
    }
}

struct Budget {
    bytes: usize,
    rows: usize,
    byte_limit: usize,
    row_limit: usize,
}

impl Budget {
    fn new(limits: Limits) -> Self {
        Self {
            bytes: 0,
            rows: 0,
            byte_limit: limits.bytes,
            row_limit: limits.rows,
        }
    }

    fn charge_value(&mut self, value: &RuntimeValue) -> Result<(), RecipeError> {
        let bytes = value
            .materialized_bytes()
            .map_err(RecipeError::InvalidDocument)?;
        self.charge_bytes(bytes)?;
        self.rows = checked_add("rows", self.rows, value.row_count(), self.row_limit)?;
        Ok(())
    }

    fn charge_bytes(&mut self, bytes: usize) -> Result<(), RecipeError> {
        self.bytes = checked_add("bytes", self.bytes, bytes, self.byte_limit)?;
        Ok(())
    }
}

fn checked_add(
    kind: &'static str,
    current: usize,
    added: usize,
    limit: usize,
) -> Result<usize, RecipeError> {
    let used = current
        .checked_add(added)
        .ok_or(RecipeError::LimitExceeded {
            kind,
            used: usize::MAX,
            limit,
        })?;
    if used > limit {
        return Err(RecipeError::LimitExceeded { kind, used, limit });
    }
    Ok(used)
}

/// Invoke all declared proven inputs once, run every transformation in document order,
/// and render one output only after the complete recipe succeeds.
pub fn execute<S: ProvenToolSource>(
    recipe: &Recipe,
    source: &mut S,
) -> Result<RecipeOutput, RecipeError> {
    recipe.validate()?;
    let mut budget = Budget::new(recipe.limits);
    let mut slots = BTreeMap::new();
    let mut lineage = Vec::with_capacity(recipe.inputs.len());

    for input in &recipe.inputs {
        let bytes = source
            .invoke(&input.tool_id, &input.args)
            .map_err(|error| RecipeError::Tool {
                input: input.name.clone(),
                tool_id: input.tool_id.clone(),
                message: error.message,
            })?;
        let text = String::from_utf8(bytes).map_err(|_| RecipeError::NonUtf8Input {
            input: input.name.clone(),
        })?;
        let value = RuntimeValue::Text(text);
        budget.charge_value(&value)?;
        slots.insert(input.name.clone(), value);
        lineage.push(InputLineage {
            name: input.name.clone(),
            tool_id: input.tool_id.clone(),
            args: input.args.clone(),
        });
    }

    for (index, step) in recipe.steps.iter().enumerate() {
        let (save_as, value) =
            evaluate_step(step, &slots).map_err(|message| RecipeError::Step { index, message })?;
        budget.charge_value(&value)?;
        slots.insert(save_as.to_string(), value);
    }

    let object =
        render_template(&recipe.emit.object_template, &slots).map_err(RecipeError::Template)?;
    let context =
        render_template(&recipe.emit.context_template, &slots).map_err(RecipeError::Template)?;
    budget.charge_bytes(object.len())?;
    budget.charge_bytes(context.len())?;
    Ok(RecipeOutput {
        actor: recipe.emit.actor.clone(),
        action: recipe.emit.action.clone(),
        object,
        context,
        inputs: lineage,
    })
}

fn evaluate_step<'a>(
    step: &'a Step,
    slots: &BTreeMap<String, RuntimeValue>,
) -> Result<(&'a str, RuntimeValue), String> {
    match step {
        Step::ParseJson(step) => {
            let text = expect_text(slots, &step.from, "parse_json")?;
            let value: JsonValue = serde_json::from_str(text)
                .map_err(|error| format!("parse_json on {}: {error}", step.from))?;
            Ok((&step.save_as, json_to_runtime(value)?))
        }
        Step::ParseLines(step) => {
            let text = expect_text(slots, &step.from, "parse_lines")?;
            let rows = text
                .lines()
                .map(|line| {
                    BTreeMap::from([("value".to_string(), JsonValue::String(line.to_string()))])
                })
                .collect();
            Ok((&step.save_as, RuntimeValue::Rows(rows)))
        }
        Step::Select(step) => {
            let value = slots
                .get(&step.from)
                .ok_or_else(|| format!("unknown slot {}", step.from))?;
            let RuntimeValue::Json(json) = value else {
                return Err(type_error("select", &step.from, "json", value));
            };
            let selected = walk_path(json, &step.path)
                .ok_or_else(|| format!("select path missing in {}", step.from))?;
            Ok((&step.save_as, json_to_runtime(selected.clone())?))
        }
        Step::Map(step) => {
            let rows = expect_rows(slots, &step.from, "map")?;
            let mut mapped = Vec::with_capacity(rows.len());
            for (index, row) in rows.iter().enumerate() {
                let root = row_to_json(row.clone());
                let mut out = BTreeMap::new();
                for (name, expr) in &step.fields {
                    let value = match expr {
                        MapExpr::Field(expr) => walk_path(&root, &expr.path)
                            .cloned()
                            .ok_or_else(|| format!("map path missing in row {index}"))?,
                        MapExpr::Literal(expr) => scalar_to_json(&expr.literal)?,
                    };
                    out.insert(name.clone(), value);
                }
                mapped.push(out);
            }
            Ok((&step.save_as, RuntimeValue::Rows(mapped)))
        }
        Step::Filter(step) => {
            let rows = expect_rows(slots, &step.from, "filter")?;
            let mut filtered = Vec::new();
            for (index, row) in rows.iter().enumerate() {
                let root = row_to_json(row.clone());
                let value = walk_path(&root, &step.predicate.path)
                    .ok_or_else(|| format!("filter path missing in row {index}"))?;
                let scalar = scalar_from_json(value)
                    .ok_or_else(|| format!("filter value in row {index} is not scalar"))?;
                if compare_scalars(&scalar, step.predicate.comparison, &step.predicate.value)? {
                    filtered.push(row.clone());
                }
            }
            Ok((&step.save_as, RuntimeValue::Rows(filtered)))
        }
        Step::Group(step) => {
            let rows = expect_rows(slots, &step.from, "group")?;
            let mut groups: BTreeMap<String, Group> = BTreeMap::new();
            for (index, row) in rows.iter().enumerate() {
                let root = row_to_json(row.clone());
                let value = walk_path(&root, &step.path)
                    .ok_or_else(|| format!("group path missing in row {index}"))?;
                let scalar = scalar_from_json(value)
                    .ok_or_else(|| format!("group value in row {index} is not scalar"))?;
                let key = canonical_scalar_key(&scalar)?;
                groups
                    .entry(key)
                    .or_insert_with(|| Group {
                        label: scalar,
                        rows: Vec::new(),
                    })
                    .rows
                    .push(row.clone());
            }
            Ok((&step.save_as, RuntimeValue::Groups(groups)))
        }
        Step::Count(step) => {
            let value = slots
                .get(&step.from)
                .ok_or_else(|| format!("unknown slot {}", step.from))?;
            let counted = match value {
                RuntimeValue::Rows(rows) => RuntimeValue::Number(rows.len() as f64),
                RuntimeValue::Groups(groups) => {
                    RuntimeValue::Rows(group_aggregate(groups, |rows| Ok(rows.len() as f64))?)
                }
                _ => return Err(type_error("count", &step.from, "rows or groups", value)),
            };
            Ok((&step.save_as, counted))
        }
        Step::Min(step) => aggregate_step(step, slots, Aggregate::Min),
        Step::Max(step) => aggregate_step(step, slots, Aggregate::Max),
        Step::Mean(step) => aggregate_step(step, slots, Aggregate::Mean),
        Step::Compare(step) => {
            let value = slots
                .get(&step.from)
                .ok_or_else(|| format!("unknown slot {}", step.from))?;
            let left = runtime_scalar(value)
                .ok_or_else(|| type_error("compare", &step.from, "scalar", value))?;
            Ok((
                &step.save_as,
                RuntimeValue::Bool(compare_scalars(&left, step.comparison, &step.value)?),
            ))
        }
        Step::Format(step) => Ok((
            &step.save_as,
            RuntimeValue::Text(render_segments(&step.segments, slots)?),
        )),
    }
}

fn expect_text<'a>(
    slots: &'a BTreeMap<String, RuntimeValue>,
    from: &str,
    op: &str,
) -> Result<&'a str, String> {
    let value = slots
        .get(from)
        .ok_or_else(|| format!("unknown slot {from}"))?;
    match value {
        RuntimeValue::Text(text) => Ok(text),
        _ => Err(type_error(op, from, "text", value)),
    }
}

fn expect_rows<'a>(
    slots: &'a BTreeMap<String, RuntimeValue>,
    from: &str,
    op: &str,
) -> Result<&'a [Row], String> {
    let value = slots
        .get(from)
        .ok_or_else(|| format!("unknown slot {from}"))?;
    match value {
        RuntimeValue::Rows(rows) => Ok(rows),
        _ => Err(type_error(op, from, "rows", value)),
    }
}

fn type_error(op: &str, from: &str, expected: &str, actual: &RuntimeValue) -> String {
    format!(
        "{op} expected {expected} in slot {from}, found {}",
        actual.kind()
    )
}

fn walk_path<'a>(mut value: &'a JsonValue, path: &[PathSegment]) -> Option<&'a JsonValue> {
    for segment in path {
        value = match segment {
            PathSegment::Field(field) => value.as_object()?.get(&field.field)?,
            PathSegment::Index(index) => value.as_array()?.get(index.index)?,
        };
    }
    Some(value)
}

fn json_to_runtime(value: JsonValue) -> Result<RuntimeValue, String> {
    match value {
        JsonValue::String(text) => Ok(RuntimeValue::Text(text)),
        JsonValue::Number(number) => number
            .as_f64()
            .filter(|number| number.is_finite())
            .map(RuntimeValue::Number)
            .ok_or_else(|| "selected JSON number is not finite".to_string()),
        JsonValue::Bool(value) => Ok(RuntimeValue::Bool(value)),
        JsonValue::Null => Ok(RuntimeValue::Null),
        JsonValue::Array(values)
            if values.is_empty() || values.iter().all(JsonValue::is_object) =>
        {
            let rows = values
                .into_iter()
                .map(|value| {
                    value
                        .as_object()
                        .expect("array shape checked")
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect()
                })
                .collect();
            Ok(RuntimeValue::Rows(rows))
        }
        value => Ok(RuntimeValue::Json(value)),
    }
}

fn scalar_to_json(value: &Scalar) -> Result<JsonValue, String> {
    match value {
        Scalar::Bool(value) => Ok(JsonValue::Bool(*value)),
        Scalar::Number(value) => number_to_json(*value),
        Scalar::String(value) => Ok(JsonValue::String(value.clone())),
        Scalar::Null => Ok(JsonValue::Null),
    }
}

fn number_to_json(value: f64) -> Result<JsonValue, String> {
    serde_json::Number::from_f64(value)
        .map(JsonValue::Number)
        .ok_or_else(|| "number is not finite".to_string())
}

fn scalar_from_json(value: &JsonValue) -> Option<Scalar> {
    match value {
        JsonValue::Bool(value) => Some(Scalar::Bool(*value)),
        JsonValue::Number(value) => value.as_f64().map(Scalar::Number),
        JsonValue::String(value) => Some(Scalar::String(value.clone())),
        JsonValue::Null => Some(Scalar::Null),
        JsonValue::Array(_) | JsonValue::Object(_) => None,
    }
}

fn runtime_scalar(value: &RuntimeValue) -> Option<Scalar> {
    match value {
        RuntimeValue::Text(value) => Some(Scalar::String(value.clone())),
        RuntimeValue::Number(value) => Some(Scalar::Number(*value)),
        RuntimeValue::Bool(value) => Some(Scalar::Bool(*value)),
        RuntimeValue::Null => Some(Scalar::Null),
        RuntimeValue::Json(value) => scalar_from_json(value),
        RuntimeValue::Rows(_) | RuntimeValue::Groups(_) => None,
    }
}

fn compare_scalars(left: &Scalar, op: Comparison, right: &Scalar) -> Result<bool, String> {
    match (left, right) {
        (Scalar::Number(left), Scalar::Number(right)) => Ok(match op {
            Comparison::Eq => left == right,
            Comparison::Ne => left != right,
            Comparison::Lt => left < right,
            Comparison::Lte => left <= right,
            Comparison::Gt => left > right,
            Comparison::Gte => left >= right,
            Comparison::Contains => return Err("contains requires two strings".to_string()),
        }),
        (Scalar::String(left), Scalar::String(right)) => Ok(match op {
            Comparison::Eq => left == right,
            Comparison::Ne => left != right,
            Comparison::Lt => left < right,
            Comparison::Lte => left <= right,
            Comparison::Gt => left > right,
            Comparison::Gte => left >= right,
            Comparison::Contains => left.contains(right),
        }),
        (Scalar::Bool(left), Scalar::Bool(right)) => match op {
            Comparison::Eq => Ok(left == right),
            Comparison::Ne => Ok(left != right),
            _ => Err("booleans only support eq/ne".to_string()),
        },
        (Scalar::Null, Scalar::Null) => match op {
            Comparison::Eq => Ok(true),
            Comparison::Ne => Ok(false),
            _ => Err("null only supports eq/ne".to_string()),
        },
        _ => Err("comparison operands have incompatible scalar types".to_string()),
    }
}

fn canonical_scalar_key(value: &Scalar) -> Result<String, String> {
    match value {
        Scalar::Bool(value) => Ok(format!("b:{value}")),
        Scalar::Number(value) => {
            let normalized = if *value == 0.0 { 0.0 } else { *value };
            Ok(format!(
                "n:{}",
                serde_json::to_string(&scalar_to_json(&Scalar::Number(normalized))?)
                    .map_err(|error| error.to_string())?
            ))
        }
        Scalar::String(value) => Ok(format!("s:{value}")),
        Scalar::Null => Ok("z:null".to_string()),
    }
}

#[derive(Clone, Copy)]
enum Aggregate {
    Min,
    Max,
    Mean,
}

fn aggregate_step<'a>(
    step: &'a AggregateStep,
    slots: &BTreeMap<String, RuntimeValue>,
    op: Aggregate,
) -> Result<(&'a str, RuntimeValue), String> {
    let value = slots
        .get(&step.from)
        .ok_or_else(|| format!("unknown slot {}", step.from))?;
    let output = match value {
        RuntimeValue::Rows(rows) => RuntimeValue::Number(aggregate_rows(rows, &step.path, op)?),
        RuntimeValue::Groups(groups) => RuntimeValue::Rows(group_aggregate(groups, |rows| {
            aggregate_rows(rows, &step.path, op)
        })?),
        _ => return Err(type_error("aggregate", &step.from, "rows or groups", value)),
    };
    Ok((&step.save_as, output))
}

fn aggregate_rows(rows: &[Row], path: &[PathSegment], op: Aggregate) -> Result<f64, String> {
    let mut values = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let root = row_to_json(row.clone());
        let value = walk_path(&root, path)
            .and_then(JsonValue::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("aggregate needs a finite number in row {index}"))?;
        values.push(value);
    }
    if values.is_empty() {
        return Err("aggregate input is empty".to_string());
    }
    match op {
        Aggregate::Min => Ok(values.into_iter().fold(f64::INFINITY, f64::min)),
        Aggregate::Max => Ok(values.into_iter().fold(f64::NEG_INFINITY, f64::max)),
        Aggregate::Mean => {
            let mut sum = 0.0;
            for value in values.iter().copied() {
                sum += value;
                if !sum.is_finite() {
                    return Err("mean overflowed to a non-finite number".to_string());
                }
            }
            let mean = sum / values.len() as f64;
            if mean.is_finite() {
                Ok(mean)
            } else {
                Err("mean produced a non-finite number".to_string())
            }
        }
    }
}

fn group_aggregate<F>(groups: &BTreeMap<String, Group>, mut f: F) -> Result<Vec<Row>, String>
where
    F: FnMut(&[Row]) -> Result<f64, String>,
{
    let mut rows = Vec::with_capacity(groups.len());
    for group in groups.values() {
        rows.push(BTreeMap::from([
            ("group".to_string(), scalar_to_json(&group.label)?),
            ("value".to_string(), number_to_json(f(&group.rows)?)?),
        ]));
    }
    Ok(rows)
}

fn row_to_json(row: Row) -> JsonValue {
    JsonValue::Object(row.into_iter().collect())
}

fn render_template(
    template: &Template,
    slots: &BTreeMap<String, RuntimeValue>,
) -> Result<String, String> {
    render_segments(&template.segments, slots)
}

fn render_segments(
    segments: &[TemplateSegment],
    slots: &BTreeMap<String, RuntimeValue>,
) -> Result<String, String> {
    let mut output = String::new();
    for segment in segments {
        match segment {
            TemplateSegment::Literal(literal) => output.push_str(&literal.literal),
            TemplateSegment::Slot(slot) => {
                let value = slots
                    .get(&slot.slot)
                    .ok_or_else(|| format!("unknown slot {}", slot.slot))?;
                if slot.path.is_empty() {
                    output.push_str(&render_runtime(value)?);
                } else {
                    let json = value.to_json()?;
                    let selected = walk_path(&json, &slot.path)
                        .ok_or_else(|| format!("template path missing in {}", slot.slot))?;
                    output.push_str(&render_json(selected)?);
                }
            }
        }
    }
    Ok(output)
}

fn render_runtime(value: &RuntimeValue) -> Result<String, String> {
    match value {
        RuntimeValue::Text(text) => Ok(text.clone()),
        _ => render_json(&value.to_json()?),
    }
}

fn render_json(value: &JsonValue) -> Result<String, String> {
    match value {
        JsonValue::String(text) => Ok(text.clone()),
        _ => serde_json::to_string(value).map_err(|error| error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Default)]
    struct Source {
        outputs: BTreeMap<String, Vec<u8>>,
        calls: Vec<(String, BTreeMap<String, Scalar>)>,
    }

    impl Source {
        fn with(id: &str, output: impl AsRef<[u8]>) -> Self {
            Self {
                outputs: BTreeMap::from([(id.to_string(), output.as_ref().to_vec())]),
                calls: Vec::new(),
            }
        }
    }

    impl ProvenToolSource for Source {
        fn invoke(
            &mut self,
            tool_id: &str,
            args: &BTreeMap<String, Scalar>,
        ) -> Result<Vec<u8>, ToolSourceError> {
            self.calls.push((tool_id.to_string(), args.clone()));
            self.outputs
                .get(tool_id)
                .cloned()
                .ok_or_else(|| ToolSourceError::new("not a proven tool"))
        }
    }

    fn literal(value: &str) -> TemplateSegment {
        TemplateSegment::Literal(LiteralSegment {
            literal: value.to_string(),
        })
    }

    fn slot(name: &str) -> TemplateSegment {
        TemplateSegment::Slot(SlotSegment {
            slot: name.to_string(),
            path: Vec::new(),
        })
    }

    fn field(name: &str) -> PathSegment {
        PathSegment::Field(FieldSegment {
            field: name.to_string(),
        })
    }

    fn base_recipe(steps: Vec<Step>, object: Vec<TemplateSegment>) -> Recipe {
        Recipe {
            version: RECIPE_VERSION,
            inputs: vec![Input {
                name: "input".to_string(),
                tool_id: "tool-0001".to_string(),
                args: BTreeMap::new(),
            }],
            steps,
            emit: Emit {
                actor: "familiar".to_string(),
                action: "reports".to_string(),
                object_template: Template { segments: object },
                context_template: Template {
                    segments: vec![literal("recipe v1")],
                },
            },
            limits: Limits {
                rows: 100,
                bytes: 64 * 1024,
                steps: 32,
            },
        }
    }

    #[test]
    fn strict_parse_rejects_unknown_fields_at_every_level() {
        let mut document =
            serde_json::to_value(base_recipe(Vec::new(), vec![slot("input")])).unwrap();
        document["surprise"] = json!(true);
        assert!(matches!(
            parse_recipe(&serde_json::to_vec(&document).unwrap()),
            Err(RecipeError::Parse(_))
        ));

        let document = json!({
            "version": 1,
            "inputs": [{"name":"input", "tool_id":"tool-1", "args":{}}],
            "steps": [{"op":"parse_lines", "from":"input", "save_as":"lines", "extra":1}],
            "emit": {
                "actor":"a", "action":"b",
                "object_template":{"segments":[{"slot":"lines"}]},
                "context_template":{"segments":[{"literal":"c"}]}
            },
            "limits":{"rows":10,"bytes":1000,"steps":2}
        });
        assert!(matches!(
            parse_recipe(&serde_json::to_vec(&document).unwrap()),
            Err(RecipeError::Parse(_))
        ));
    }

    #[test]
    fn validation_precedes_tool_invocation() {
        let mut recipe = base_recipe(Vec::new(), vec![slot("input")]);
        recipe.version = 2;
        let mut source = Source::with("tool-0001", "hello");
        assert!(matches!(
            execute(&recipe, &mut source),
            Err(RecipeError::InvalidDocument(_))
        ));
        assert!(source.calls.is_empty());

        recipe.version = 1;
        recipe.inputs.push(recipe.inputs[0].clone());
        assert!(matches!(
            execute(&recipe, &mut source),
            Err(RecipeError::InvalidDocument(_))
        ));
        assert!(source.calls.is_empty());
    }

    #[test]
    fn proven_inputs_are_invoked_once_in_declared_order_with_lineage() {
        let mut recipe = base_recipe(Vec::new(), vec![slot("input"), literal("/"), slot("other")]);
        recipe.inputs[0]
            .args
            .insert("zone".to_string(), Scalar::String("north".to_string()));
        recipe.inputs.push(Input {
            name: "other".to_string(),
            tool_id: "tool-0002".to_string(),
            args: BTreeMap::new(),
        });
        let mut source = Source {
            outputs: BTreeMap::from([
                ("tool-0001".to_string(), b"one".to_vec()),
                ("tool-0002".to_string(), b"two".to_vec()),
            ]),
            calls: Vec::new(),
        };
        let output = execute(&recipe, &mut source).unwrap();
        assert_eq!(output.object, "one/two");
        assert_eq!(source.calls.len(), 2);
        assert_eq!(source.calls[0].0, "tool-0001");
        assert_eq!(source.calls[1].0, "tool-0002");
        assert_eq!(
            output.inputs,
            vec![
                InputLineage {
                    name: "input".to_string(),
                    tool_id: "tool-0001".to_string(),
                    args: BTreeMap::from([(
                        "zone".to_string(),
                        Scalar::String("north".to_string())
                    )])
                },
                InputLineage {
                    name: "other".to_string(),
                    tool_id: "tool-0002".to_string(),
                    args: BTreeMap::new()
                }
            ]
        );
    }

    #[test]
    fn forward_references_are_rejected_before_invocation() {
        let recipe = base_recipe(
            vec![Step::Format(FormatStep {
                segments: vec![slot("later")],
                save_as: "earlier".to_string(),
            })],
            vec![slot("earlier")],
        );
        assert!(matches!(
            recipe.validate(),
            Err(RecipeError::InvalidDocument(_))
        ));
    }

    #[test]
    fn unknown_tool_and_non_utf8_are_explicit_failures() {
        let recipe = base_recipe(Vec::new(), vec![slot("input")]);
        let mut source = Source::default();
        assert!(matches!(
            execute(&recipe, &mut source),
            Err(RecipeError::Tool { .. })
        ));

        let mut source = Source::with("tool-0001", [0xff, 0xfe]);
        assert!(matches!(
            execute(&recipe, &mut source),
            Err(RecipeError::NonUtf8Input { .. })
        ));
    }

    #[test]
    fn json_select_filter_map_mean_compare_and_format_compose() {
        let steps = vec![
            Step::ParseJson(UnaryStep {
                from: "input".to_string(),
                save_as: "document".to_string(),
            }),
            Step::Select(SelectStep {
                from: "document".to_string(),
                path: vec![field("readings")],
                save_as: "readings".to_string(),
            }),
            Step::Filter(FilterStep {
                from: "readings".to_string(),
                predicate: Predicate {
                    path: vec![field("watts")],
                    comparison: Comparison::Gt,
                    value: Scalar::Number(0.0),
                },
                save_as: "active".to_string(),
            }),
            Step::Map(MapStep {
                from: "active".to_string(),
                fields: BTreeMap::from([
                    (
                        "power".to_string(),
                        MapExpr::Field(FieldExpr {
                            path: vec![field("watts")],
                        }),
                    ),
                    (
                        "unit".to_string(),
                        MapExpr::Literal(LiteralExpr {
                            literal: Scalar::String("W".to_string()),
                        }),
                    ),
                ]),
                save_as: "mapped".to_string(),
            }),
            Step::Mean(AggregateStep {
                from: "mapped".to_string(),
                path: vec![field("power")],
                save_as: "mean".to_string(),
            }),
            Step::Compare(CompareStep {
                from: "mean".to_string(),
                comparison: Comparison::Gte,
                value: Scalar::Number(15.0),
                save_as: "attention".to_string(),
            }),
            Step::Format(FormatStep {
                segments: vec![literal("mean="), slot("mean"), literal("W")],
                save_as: "summary".to_string(),
            }),
        ];
        let recipe = base_recipe(
            steps,
            vec![slot("summary"), literal(" attention="), slot("attention")],
        );
        let mut source = Source::with(
            "tool-0001",
            r#"{"readings":[{"watts":10},{"watts":0},{"watts":20}]}"#,
        );
        let output = execute(&recipe, &mut source).unwrap();
        assert_eq!(output.object, "mean=15.0W attention=true");
    }

    #[test]
    fn top_level_json_object_arrays_become_rows_without_an_identity_select() {
        let recipe = base_recipe(
            vec![
                Step::ParseJson(UnaryStep {
                    from: "input".to_string(),
                    save_as: "rows".to_string(),
                }),
                Step::Count(UnaryStep {
                    from: "rows".to_string(),
                    save_as: "count".to_string(),
                }),
            ],
            vec![slot("count")],
        );
        let mut source = Source::with("tool-0001", r#"[{"value":1},{"value":2}]"#);
        assert_eq!(execute(&recipe, &mut source).unwrap().object, "2.0");
    }

    #[test]
    fn parse_lines_and_count_preserve_empty_interior_lines() {
        let recipe = base_recipe(
            vec![
                Step::ParseLines(UnaryStep {
                    from: "input".to_string(),
                    save_as: "lines".to_string(),
                }),
                Step::Count(UnaryStep {
                    from: "lines".to_string(),
                    save_as: "count".to_string(),
                }),
            ],
            vec![slot("count")],
        );
        let mut source = Source::with("tool-0001", "one\n\nthree\n");
        assert_eq!(execute(&recipe, &mut source).unwrap().object, "3.0");
    }

    #[test]
    fn grouped_count_min_max_and_mean_are_sorted_and_stable() {
        let base = vec![
            Step::ParseJson(UnaryStep {
                from: "input".to_string(),
                save_as: "json".to_string(),
            }),
            Step::Select(SelectStep {
                from: "json".to_string(),
                path: vec![field("rows")],
                save_as: "rows".to_string(),
            }),
            Step::Group(GroupStep {
                from: "rows".to_string(),
                path: vec![field("zone")],
                save_as: "groups".to_string(),
            }),
        ];
        let aggregates = ["count", "min", "max", "mean"];
        for aggregate in aggregates {
            let mut steps = base.clone();
            let output_slot = aggregate.to_string();
            steps.push(match aggregate {
                "count" => Step::Count(UnaryStep {
                    from: "groups".to_string(),
                    save_as: output_slot.clone(),
                }),
                "min" => Step::Min(AggregateStep {
                    from: "groups".to_string(),
                    path: vec![field("watts")],
                    save_as: output_slot.clone(),
                }),
                "max" => Step::Max(AggregateStep {
                    from: "groups".to_string(),
                    path: vec![field("watts")],
                    save_as: output_slot.clone(),
                }),
                _ => Step::Mean(AggregateStep {
                    from: "groups".to_string(),
                    path: vec![field("watts")],
                    save_as: output_slot.clone(),
                }),
            });
            let recipe = base_recipe(steps, vec![slot(&output_slot)]);
            let mut source = Source::with(
                "tool-0001",
                r#"{"rows":[{"zone":"z","watts":4},{"zone":"a","watts":2},{"zone":"z","watts":8}]}"#,
            );
            let object = execute(&recipe, &mut source).unwrap().object;
            assert!(object.starts_with(r#"[{"group":"a","value":"#));
            assert!(object.contains(r#"},{"group":"z","value":"#));
            match aggregate {
                "count" => assert!(object.contains(r#""value":2.0"#)),
                "min" => assert!(object.contains(r#""value":4.0"#)),
                "max" => assert!(object.contains(r#""value":8.0"#)),
                _ => assert!(object.contains(r#""value":6.0"#)),
            }
        }
    }

    #[test]
    fn select_supports_typed_indexes_and_template_paths() {
        let recipe = base_recipe(
            vec![
                Step::ParseJson(UnaryStep {
                    from: "input".to_string(),
                    save_as: "json".to_string(),
                }),
                Step::Select(SelectStep {
                    from: "json".to_string(),
                    path: vec![
                        field("items"),
                        PathSegment::Index(IndexSegment { index: 1 }),
                    ],
                    save_as: "item".to_string(),
                }),
            ],
            vec![TemplateSegment::Slot(SlotSegment {
                slot: "item".to_string(),
                path: vec![field("name")],
            })],
        );
        let mut source = Source::with(
            "tool-0001",
            r#"{"items":[{"name":"first"},{"name":"second"}]}"#,
        );
        assert_eq!(execute(&recipe, &mut source).unwrap().object, "second");
    }

    #[test]
    fn all_comparisons_are_typed_and_contains_is_string_only() {
        assert!(compare_scalars(
            &Scalar::String("greenhouse".to_string()),
            Comparison::Contains,
            &Scalar::String("house".to_string())
        )
        .unwrap());
        assert!(
            compare_scalars(&Scalar::Number(2.0), Comparison::Lt, &Scalar::Number(3.0)).unwrap()
        );
        assert!(
            compare_scalars(&Scalar::Bool(true), Comparison::Gt, &Scalar::Bool(false)).is_err()
        );
        assert!(compare_scalars(
            &Scalar::Number(1.0),
            Comparison::Eq,
            &Scalar::String("1".to_string())
        )
        .is_err());
    }

    #[test]
    fn malformed_data_missing_paths_type_errors_and_empty_aggregates_refuse() {
        let parse = base_recipe(
            vec![Step::ParseJson(UnaryStep {
                from: "input".to_string(),
                save_as: "json".to_string(),
            })],
            vec![slot("json")],
        );
        let mut source = Source::with("tool-0001", "not json");
        assert!(matches!(
            execute(&parse, &mut source),
            Err(RecipeError::Step { .. })
        ));

        let missing = base_recipe(
            vec![
                Step::ParseJson(UnaryStep {
                    from: "input".to_string(),
                    save_as: "json".to_string(),
                }),
                Step::Select(SelectStep {
                    from: "json".to_string(),
                    path: vec![field("missing")],
                    save_as: "value".to_string(),
                }),
            ],
            vec![slot("value")],
        );
        let mut source = Source::with("tool-0001", "{}");
        assert!(matches!(
            execute(&missing, &mut source),
            Err(RecipeError::Step { .. })
        ));

        let empty = base_recipe(
            vec![
                Step::ParseJson(UnaryStep {
                    from: "input".to_string(),
                    save_as: "json".to_string(),
                }),
                Step::Select(SelectStep {
                    from: "json".to_string(),
                    path: vec![field("rows")],
                    save_as: "rows".to_string(),
                }),
                Step::Mean(AggregateStep {
                    from: "rows".to_string(),
                    path: vec![field("value")],
                    save_as: "mean".to_string(),
                }),
            ],
            vec![slot("mean")],
        );
        let mut source = Source::with("tool-0001", r#"{"rows":[]}"#);
        assert!(matches!(
            execute(&empty, &mut source),
            Err(RecipeError::Step { .. })
        ));
    }

    #[test]
    fn declared_row_byte_and_step_limits_are_enforced() {
        let mut rows = base_recipe(
            vec![Step::ParseLines(UnaryStep {
                from: "input".to_string(),
                save_as: "lines".to_string(),
            })],
            vec![slot("lines")],
        );
        rows.limits.rows = 1;
        let mut source = Source::with("tool-0001", "one\ntwo");
        assert!(matches!(
            execute(&rows, &mut source),
            Err(RecipeError::LimitExceeded { kind: "rows", .. })
        ));

        let mut bytes = base_recipe(Vec::new(), vec![slot("input")]);
        bytes.limits.bytes = 3;
        let mut source = Source::with("tool-0001", "four");
        assert!(matches!(
            execute(&bytes, &mut source),
            Err(RecipeError::LimitExceeded { kind: "bytes", .. })
        ));

        let mut steps = base_recipe(
            vec![Step::ParseLines(UnaryStep {
                from: "input".to_string(),
                save_as: "lines".to_string(),
            })],
            vec![slot("lines")],
        );
        steps.limits.steps = 1;
        assert!(steps.validate().is_ok());
        steps.limits.steps = 0;
        assert!(matches!(
            steps.validate(),
            Err(RecipeError::InvalidDocument(_))
        ));
    }

    #[test]
    fn hard_ceilings_and_manifest_envelope_are_enforced() {
        let bytes = vec![b' '; MAX_RECIPE_BYTES + 1];
        assert!(matches!(
            parse_recipe(&bytes),
            Err(RecipeError::ManifestTooLarge { .. })
        ));
        let mut recipe = base_recipe(Vec::new(), vec![slot("input")]);
        recipe.limits.rows = MAX_ROWS + 1;
        assert!(matches!(
            recipe.validate(),
            Err(RecipeError::InvalidDocument(_))
        ));
        recipe.limits.rows = 1;
        recipe.inputs = (0..=MAX_INPUTS)
            .map(|index| Input {
                name: format!("input_{index}"),
                tool_id: format!("tool-{index}"),
                args: BTreeMap::new(),
            })
            .collect();
        assert!(matches!(
            recipe.validate(),
            Err(RecipeError::InvalidDocument(_))
        ));
    }

    #[test]
    fn recipe_cannot_smuggle_paths_or_urls_as_tool_ids() {
        for bad in ["/bin/sh", "../tool", "https://host/tool", "tool id"] {
            let mut recipe = base_recipe(Vec::new(), vec![slot("input")]);
            recipe.inputs[0].tool_id = bad.to_string();
            assert!(matches!(
                recipe.validate(),
                Err(RecipeError::InvalidDocument(_))
            ));
        }
    }

    #[test]
    fn json_path_fields_may_use_real_world_punctuation() {
        let recipe = base_recipe(
            vec![
                Step::ParseJson(UnaryStep {
                    from: "input".to_string(),
                    save_as: "json".to_string(),
                }),
                Step::Select(SelectStep {
                    from: "json".to_string(),
                    path: vec![field("CPU Load")],
                    save_as: "load".to_string(),
                }),
            ],
            vec![slot("load")],
        );
        let mut source = Source::with("tool-0001", r#"{"CPU Load":0.5}"#);
        assert_eq!(execute(&recipe, &mut source).unwrap().object, "0.5");
    }

    #[test]
    fn direct_non_finite_values_and_mean_overflow_refuse() {
        let mut recipe = base_recipe(Vec::new(), vec![slot("input")]);
        recipe.inputs[0]
            .args
            .insert("bad".to_string(), Scalar::Number(f64::NAN));
        assert!(matches!(
            recipe.validate(),
            Err(RecipeError::InvalidDocument(_))
        ));

        let recipe = base_recipe(
            vec![
                Step::ParseJson(UnaryStep {
                    from: "input".to_string(),
                    save_as: "json".to_string(),
                }),
                Step::Select(SelectStep {
                    from: "json".to_string(),
                    path: vec![field("rows")],
                    save_as: "rows".to_string(),
                }),
                Step::Mean(AggregateStep {
                    from: "rows".to_string(),
                    path: vec![field("value")],
                    save_as: "mean".to_string(),
                }),
            ],
            vec![slot("mean")],
        );
        let mut source = Source::with(
            "tool-0001",
            format!(r#"{{"rows":[{{"value":{0}}},{{"value":{0}}}]}}"#, f64::MAX),
        );
        assert!(matches!(
            execute(&recipe, &mut source),
            Err(RecipeError::Step { .. })
        ));
    }

    #[test]
    fn replay_is_exactly_deterministic() {
        let recipe = base_recipe(
            vec![
                Step::ParseLines(UnaryStep {
                    from: "input".to_string(),
                    save_as: "lines".to_string(),
                }),
                Step::Count(UnaryStep {
                    from: "lines".to_string(),
                    save_as: "count".to_string(),
                }),
            ],
            vec![literal("lines="), slot("count")],
        );
        let mut first = Source::with("tool-0001", "a\nb\nc");
        let mut second = Source::with("tool-0001", "a\nb\nc");
        assert_eq!(
            execute(&recipe, &mut first).unwrap(),
            execute(&recipe, &mut second).unwrap()
        );
    }
}

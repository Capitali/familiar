use familiar_recipe::{parse_recipe, Recipe};
use familiar_scenario::recipe_oracle::{
    compare_scores, evaluate_recipe, is_recipe_oracle_path, parse_fixture, parse_output_contract,
    score_replay, ContractInput, Eligibility, EvidenceKind, ObservedOutcome, OracleError,
    OracleFixture, OutputContract, VariantKind,
};
use serde_json::json;
use std::cmp::Ordering;

const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/recipe-oracles/greenhouse-power.json"
));

fn fixture() -> OracleFixture {
    parse_fixture(FIXTURE).expect("checked-in oracle fixture must be valid")
}

fn contract() -> OutputContract {
    OutputContract {
        actor: "greenhouse".to_string(),
        action: "reports_power".to_string(),
        inputs: vec![ContractInput {
            name: "status".to_string(),
            tool_id: "greenhouse-status".to_string(),
        }],
    }
}

fn candidate_recipe() -> Recipe {
    let document = json!({
        "version": 1,
        "caps": {
            "process": { "proven_tools": ["greenhouse-status"] },
            "clock": "none",
            "fs": "none",
            "env": "none",
            "net": "none"
        },
        "inputs": [{
            "name": "status",
            "tool_id": "greenhouse-status",
            "args": { "zone": "north" }
        }],
        "steps": [
            { "op": "parse_json", "from": "status", "save_as": "document" },
            {
                "op": "select",
                "from": "document",
                "path": [{ "field": "watts" }],
                "save_as": "watts"
            }
        ],
        "emit": {
            "actor": "greenhouse",
            "action": "reports_power",
            "object_template": { "segments": [{ "slot": "watts" }] },
            "context_template": {
                "segments": [{ "literal": "north-zone fixture replay" }]
            }
        },
        "limits": { "rows": 32, "bytes": 65536, "steps": 8 }
    });
    parse_recipe(&serde_json::to_vec(&document).unwrap()).unwrap()
}

fn hard_coded_recipe() -> Recipe {
    let document = json!({
        "version": 1,
        "caps": {
            "process": { "proven_tools": ["greenhouse-status"] },
            "clock": "none",
            "fs": "none",
            "env": "none",
            "net": "none"
        },
        "inputs": [{
            "name": "status",
            "tool_id": "greenhouse-status",
            "args": { "zone": "north" }
        }],
        "steps": [],
        "emit": {
            "actor": "greenhouse",
            "action": "reports_power",
            "object_template": { "segments": [{ "literal": "120.0" }] },
            "context_template": {
                "segments": [{ "literal": "north-zone fixture replay" }]
            }
        },
        "limits": { "rows": 32, "bytes": 65536, "steps": 8 }
    });
    parse_recipe(&serde_json::to_vec(&document).unwrap()).unwrap()
}

#[test]
fn truthful_recipe_passes_every_external_gate() {
    let score = evaluate_recipe(&fixture(), &contract(), &candidate_recipe()).unwrap();

    assert_eq!(score.eligibility, Eligibility::Eligible, "{score:#?}");
    assert!(score.boundary_ok);
    assert!(score.execution_ok);
    assert!(score.contract_ok);
    assert_eq!((score.accuracy.passed, score.accuracy.total), (3, 3));
    assert_eq!((score.coverage.passed, score.coverage.total), (4, 4));
    assert_eq!((score.quietness.passed, score.quietness.total), (1, 1));
    assert_eq!(
        (score.discrimination.passed, score.discrimination.total),
        (3, 3)
    );
    assert_eq!(score.usefulness, 8);
    assert!(matches!(score.runs[1].outcome, ObservedOutcome::Silent));
    assert!(matches!(
        score.runs[4].outcome,
        ObservedOutcome::Error { .. }
    ));
}

#[test]
fn hard_coded_and_fabricated_answers_fail_truth_without_becoming_exec_failures() {
    let score = evaluate_recipe(&fixture(), &contract(), &hard_coded_recipe()).unwrap();

    assert_eq!(score.eligibility, Eligibility::TruthFailure);
    assert!(score.boundary_ok);
    assert!(score.execution_ok);
    assert_eq!((score.accuracy.passed, score.accuracy.total), (1, 3));
    assert_eq!((score.coverage.passed, score.coverage.total), (1, 4));
    assert_eq!(
        (score.discrimination.passed, score.discrimination.total),
        (0, 3)
    );
}

#[test]
fn chatty_candidate_fails_quietness_even_when_every_material_answer_is_true() {
    let fixture = fixture();
    let good = evaluate_recipe(&fixture, &contract(), &candidate_recipe()).unwrap();
    let mut chatty = good.runs.clone();
    chatty[1].outcome = chatty[0].outcome.clone();

    let score = score_replay(&fixture, true, true, &chatty).unwrap();
    assert_eq!(score.eligibility, Eligibility::TruthFailure);
    assert_eq!((score.quietness.passed, score.quietness.total), (0, 1));
    assert_eq!((score.coverage.passed, score.coverage.total), (4, 4));
}

#[test]
fn invalid_capability_manifest_fails_before_any_fixture_effect() {
    let fixture = fixture();
    let mut candidate = candidate_recipe();
    candidate.caps.process.proven_tools = vec!["different-tool".to_string()];

    let score = evaluate_recipe(&fixture, &contract(), &candidate).unwrap();
    assert_eq!(score.eligibility, Eligibility::BoundaryFailure);
    assert!(!score.boundary_ok);
    assert_eq!(score.cost, 0);
    assert!(score.runs.iter().all(|run| !run.transcript_ok));
}

#[test]
fn output_contract_is_candidate_owned_and_must_match_the_recipe() {
    let encoded = serde_json::to_vec(&contract()).unwrap();
    assert_eq!(parse_output_contract(&encoded).unwrap(), contract());

    let mut unknown: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    unknown["surprise"] = json!(true);
    assert!(matches!(
        parse_output_contract(&serde_json::to_vec(&unknown).unwrap()),
        Err(OracleError::ContractParse(_))
    ));

    let mut false_contract = contract();
    false_contract.action = "reports_temperature".to_string();

    let score = evaluate_recipe(&fixture(), &false_contract, &candidate_recipe()).unwrap();
    assert_eq!(score.eligibility, Eligibility::TruthFailure);
    assert!(!score.contract_ok);
    assert!(score.execution_ok);
}

#[test]
fn transcript_identity_and_arguments_are_external_execution_gates() {
    let mut fixture = fixture();
    fixture.variants[2].calls[0].args.insert(
        "zone".to_string(),
        familiar_recipe::Scalar::String("south".to_string()),
    );

    let score = evaluate_recipe(&fixture, &contract(), &candidate_recipe()).unwrap();
    assert_eq!(score.eligibility, Eligibility::ExecutionFailure);
    assert!(!score.execution_ok);
    assert!(!score.runs[2].transcript_ok);
}

#[test]
fn fixture_shape_is_strict_complete_and_cannot_represent_live_eligibility() {
    assert!(is_recipe_oracle_path(std::path::Path::new(
        "scenarios/recipe-oracles/greenhouse-power.json"
    )));
    assert!(!is_recipe_oracle_path(std::path::Path::new(
        "scenarios/process-failures/backup-spaces.json"
    )));

    let mut document: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    document["evidence"] = json!("live");
    assert!(matches!(
        parse_fixture(&serde_json::to_vec(&document).unwrap()),
        Err(OracleError::Parse(_))
    ));

    let mut document: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    document["variants"][0]["calls"][0]["surprise"] = json!(true);
    assert!(matches!(
        parse_fixture(&serde_json::to_vec(&document).unwrap()),
        Err(OracleError::Parse(_))
    ));

    let mut incomplete = fixture();
    incomplete
        .variants
        .retain(|variant| variant.kind != VariantKind::Malformed);
    assert!(matches!(
        incomplete.validate(),
        Err(OracleError::InvalidFixture(_))
    ));
    assert_eq!(fixture().evidence, EvidenceKind::FixtureReplay);
}

#[test]
fn eligibility_is_lexicographic_and_only_survivors_trade_usefulness_for_cost() {
    let eligible = evaluate_recipe(&fixture(), &contract(), &candidate_recipe()).unwrap();
    let mut boundary_failure = eligible.clone();
    boundary_failure.eligibility = Eligibility::BoundaryFailure;
    boundary_failure.usefulness = u64::MAX;
    boundary_failure.cost = 0;
    assert_eq!(
        compare_scores(&eligible, &boundary_failure),
        Ordering::Greater
    );

    let mut truth_failure = eligible.clone();
    truth_failure.eligibility = Eligibility::TruthFailure;
    truth_failure.usefulness = u64::MAX;
    assert_eq!(compare_scores(&eligible, &truth_failure), Ordering::Greater);

    let mut cheaper = eligible.clone();
    cheaper.cost -= 1;
    assert_eq!(compare_scores(&cheaper, &eligible), Ordering::Greater);

    let mut useful = eligible.clone();
    useful.usefulness += 1;
    useful.cost = u64::MAX;
    assert_eq!(compare_scores(&useful, &cheaper), Ordering::Greater);
}

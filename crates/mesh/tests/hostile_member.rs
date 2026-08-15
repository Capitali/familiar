//! Deterministic multi-node proof fixture used by the T-133/T-134/T-135 containments.

mod support;

use familiar_kernel::{boundary, goal, observation};
use familiar_mesh::brief::{AuthorityGrant, GoalShare, ObsShare};
use support::{MeshHarness, NetworkSchedule, NOW};

fn offered_goal(id: &str, origin: &str, updated_at: i64) -> GoalShare {
    GoalShare {
        id: id.into(),
        description: "exercise the hostile-member merge boundary".into(),
        needs: vec!["mesh".into()],
        status: "proposed".into(),
        owner_node: String::new(),
        owner_human: String::new(),
        origin: origin.into(),
        produced: String::new(),
        notes: String::new(),
        created_at: NOW,
        updated_at,
        status_at: updated_at,
        last_worked_at: 0,
        completed_at: 0,
        ended_at: 0,
    }
}

#[test]
fn a_valid_member_payload_reaches_only_the_scheduled_node_and_time() {
    let mesh = MeshHarness::new(&["sender", "early-target", "partitioned-target"]);
    let sender_id = mesh.node(0).id();
    let brief = mesh.signed(0, "scheduled-goal", |body| {
        body.knowledge.goals = vec![offered_goal("goal-hostile-fixture", &sender_id, NOW + 5)];
    });
    let mut network = NetworkSchedule::default();
    network.deliver_at(NOW + 10, 1, brief.clone());
    network.deliver_at(NOW + 30, 2, brief);

    assert_eq!(network.run_through(&mesh, NOW + 9), 0);
    assert_eq!(network.pending(), 2);
    assert!(!mesh.inbox_path(1, &sender_id).exists());
    assert!(!mesh.inbox_path(2, &sender_id).exists());

    assert_eq!(network.run_through(&mesh, NOW + 10), 1);
    assert!(mesh.inbox_path(1, &sender_id).exists());
    assert!(!mesh.inbox_path(2, &sender_id).exists());
    assert_eq!(mesh.tick(1, NOW + 11).peers, 1);
    assert!(
        goal::load_by_id(&mesh.node(1).dir, "goal-hostile-fixture")
            .unwrap()
            .is_some(),
        "a valid same-group member reaches the real merge boundary"
    );

    assert_eq!(network.run_through(&mesh, NOW + 29), 0);
    assert!(
        goal::load_by_id(&mesh.node(2).dir, "goal-hostile-fixture")
            .unwrap()
            .is_none(),
        "the partitioned node sees nothing before delivery"
    );
    assert_eq!(network.run_through(&mesh, NOW + 30), 1);
    assert_eq!(mesh.tick(2, NOW + 31).peers, 1);
    assert!(
        goal::load_by_id(&mesh.node(2).dir, "goal-hostile-fixture")
            .unwrap()
            .is_some(),
        "healing the scheduled partition delivers the held member brief"
    );
}

#[test]
fn same_time_same_sender_delivery_is_deterministic_latest_brief_wins() {
    let mesh = MeshHarness::new(&["sender", "target"]);
    let sender_id = mesh.node(0).id();
    let first = mesh.signed(0, "same-time-first", |body| {
        body.knowledge.goals = vec![offered_goal("goal-first", &sender_id, NOW + 1)];
    });
    let second = mesh.signed(0, "same-time-second", |body| {
        body.knowledge.goals = vec![offered_goal("goal-second", &sender_id, NOW + 2)];
    });
    let mut network = NetworkSchedule::default();
    network.deliver_at(NOW + 10, 1, first);
    network.deliver_at(NOW + 10, 1, second);

    assert_eq!(network.run_through(&mesh, NOW + 10), 2);
    let stored: familiar_mesh::brief::MeshBrief =
        serde_json::from_slice(&std::fs::read(mesh.inbox_path(1, &sender_id)).unwrap()).unwrap();
    assert_eq!(stored.body.nonce, "same-time-second");

    assert_eq!(mesh.tick(1, NOW + 11).peers, 1);
    assert!(goal::load_by_id(&mesh.node(1).dir, "goal-first")
        .unwrap()
        .is_none());
    assert!(goal::load_by_id(&mesh.node(1).dir, "goal-second")
        .unwrap()
        .is_some());
}

#[test]
fn concurrent_member_claims_cannot_rewrite_the_first_adopted_goal() {
    let mesh = MeshHarness::new(&["claimant-a", "claimant-b", "target"]);
    let claimant_a = mesh.node(0).id();
    let claimant_b = mesh.node(1).id();
    let claim_a = mesh.signed(0, "concurrent-a", |body| {
        let mut goal = offered_goal("goal-concurrent", &claimant_a, NOW + 1);
        goal.status = "claimed".into();
        goal.owner_node = claimant_a.clone();
        body.knowledge.goals = vec![goal];
    });
    let claim_b = mesh.signed(1, "concurrent-b", |body| {
        let mut goal = offered_goal("goal-concurrent", &claimant_b, NOW + 1);
        goal.status = "claimed".into();
        goal.owner_node = claimant_b.clone();
        body.knowledge.goals = vec![goal];
    });
    let mut network = NetworkSchedule::default();
    network.deliver_at(NOW + 10, 2, claim_a);
    network.deliver_at(NOW + 10, 2, claim_b);

    assert_eq!(network.run_through(&mesh, NOW + 10), 2);
    assert!(mesh.inbox_path(2, &mesh.node(0).id()).exists());
    assert!(mesh.inbox_path(2, &mesh.node(1).id()).exists());
    assert_eq!(mesh.tick(2, NOW + 11).peers, 2);
    let adopted = goal::load_by_id(&mesh.node(2).dir, "goal-concurrent")
        .unwrap()
        .unwrap();
    assert!(
        [claimant_a, claimant_b].contains(&adopted.owner_node),
        "one unknown definition is adopted; the other cannot steal it"
    );
    assert_eq!(
        observation::load(&mesh.node(2).dir)
            .unwrap()
            .iter()
            .filter(|item| item.action == "refused-goal-rewrite")
            .count(),
        1,
        "the conflicting signed claim is visible, not merged"
    );
}

#[test]
fn a_foreign_signed_member_is_rejected_by_the_real_merge_verifier() {
    let target_mesh = MeshHarness::new(&["target"]);
    let foreign_mesh = MeshHarness::new(&["foreign"]);
    let foreign_id = foreign_mesh.node(0).id();
    let foreign = foreign_mesh.signed(0, "foreign-group", |body| {
        body.knowledge.goals = vec![offered_goal("goal-foreign", &foreign_id, NOW + 1)];
    });
    let mut network = NetworkSchedule::default();
    network.deliver_at(NOW + 10, 0, foreign);

    assert_eq!(network.run_through(&target_mesh, NOW + 10), 1);
    let report = target_mesh.tick(0, NOW + 11);
    assert_eq!(report.rejected, 1);
    assert_eq!(report.peers, 0);
    assert!(goal::load_by_id(&target_mesh.node(0).dir, "goal-foreign")
        .unwrap()
        .is_none());
}

/// T-133 containment: a valid member and replay cannot turn an unmatched assertion into local
/// authority. The same signed-member channel retains only its fail-safe asymmetry: a stop narrows.
#[test]
fn unmatched_positive_grant_and_replay_are_refused_while_a_stop_narrows() {
    let mesh = MeshHarness::new(&["member", "target"]);
    let target_id = mesh.node(1).id();
    let brief = mesh.signed(0, "unmatched-positive-grant", |body| {
        let counterfeit = AuthorityGrant {
            target: target_id,
            kind: "gate".into(),
            ref_id: "allow_execute".into(),
            approved: true,
            note: String::new(),
            ts: NOW,
        };
        body.authority_grants = vec![counterfeit.clone(), counterfeit];
    });
    let mut network = NetworkSchedule::default();
    network.deliver_at(NOW + 10, 1, brief.clone());
    network.deliver_at(NOW + 20, 1, brief);

    assert!(!boundary::load(&mesh.node(1).dir).unwrap().allow_execute);
    assert_eq!(network.run_through(&mesh, NOW + 10), 1);
    mesh.tick(1, NOW + 11);
    assert!(
        !boundary::load(&mesh.node(1).dir).unwrap().allow_execute,
        "a member signature is not a human/device-bound authority receipt"
    );
    let refused_once = observation::load(&mesh.node(1).dir)
        .unwrap()
        .into_iter()
        .filter(|item| item.action == "refused-grant")
        .count();
    assert_eq!(refused_once, 1);

    assert_eq!(network.run_through(&mesh, NOW + 20), 1);
    mesh.tick(1, NOW + 21);
    let after_replay = observation::load(&mesh.node(1).dir)
        .unwrap()
        .into_iter()
        .filter(|item| item.action == "refused-grant")
        .count();
    assert_eq!(
        after_replay, 1,
        "duplicates in one body and a replayed body mint only one refusal"
    );

    // A local human now opens execution. The member still cannot widen it, but its signed stop
    // may take the strictly smaller path and cascades to the sharper authored-execute gate.
    let mut locally_open = boundary::load(&mesh.node(1).dir).unwrap();
    locally_open.allow_execute = true;
    locally_open.allow_authored_execute = true;
    std::fs::write(
        mesh.node(1).dir.join(boundary::BOUNDARY_FILE),
        serde_json::to_vec_pretty(&locally_open).unwrap(),
    )
    .unwrap();
    let target_id = mesh.node(1).id();
    let stop = mesh.signed(0, "remote-stop", |body| {
        body.authority_grants = vec![AuthorityGrant {
            target: target_id,
            kind: "gate".into(),
            ref_id: "allow_execute".into(),
            approved: false,
            note: String::new(),
            ts: NOW + 30,
        }];
    });
    network.deliver_at(NOW + 30, 1, stop.clone());
    network.deliver_at(NOW + 40, 1, stop);
    assert_eq!(network.run_through(&mesh, NOW + 30), 1);
    mesh.tick(1, NOW + 31);
    let narrowed = boundary::load(&mesh.node(1).dir).unwrap();
    assert!(!narrowed.allow_execute && !narrowed.allow_authored_execute);
    assert_eq!(
        observation::load(&mesh.node(1).dir)
            .unwrap()
            .iter()
            .filter(|item| item.action == "applied-grant")
            .count(),
        1,
        "the stop is recorded once"
    );
    assert_eq!(network.run_through(&mesh, NOW + 40), 1);
    mesh.tick(1, NOW + 41);
    assert_eq!(
        observation::load(&mesh.node(1).dir)
            .unwrap()
            .iter()
            .filter(|item| item.action == "applied-grant")
            .count(),
        1,
        "a replayed stop remains idempotent"
    );
}

/// T-134 containment: a far-future member clock proves provenance, never rewrite authority.
#[test]
fn a_future_clock_cannot_rewrite_an_existing_goal_and_replay_is_idempotent() {
    let mesh = MeshHarness::new(&["member", "target"]);
    let mut local = goal::Goal::seed(
        "goal-clock-poison",
        "the target's local intention",
        vec!["local-capability".into()],
        "ian",
        NOW,
    );
    local.transition(goal::Status::InProgress, NOW + 1);
    local.owner_node = mesh.node(1).id();
    goal::append(&mesh.node(1).dir, &local).unwrap();

    let member_id = mesh.node(0).id();
    let far_future = NOW + 20 * 365 * 24 * 60 * 60;
    let brief = mesh.signed(0, "future-clock-goal", |body| {
        let marker = format!("{member_id}:goal-clock-poison");
        body.knowledge.observations = vec![ObsShare {
            origin: member_id.clone(),
            actor: "familiar".into(),
            action: "refused-goal-rewrite".into(),
            object: marker,
            context: "counterfeit refusal intended to suppress the target's audit".into(),
            ts: NOW,
            confidence_pct: 100,
        }];
        let mut takeover = offered_goal("goal-clock-poison", &member_id, far_future);
        takeover.description = "member replaced the goal".into();
        takeover.status = "done".into();
        takeover.owner_node = member_id.clone();
        takeover.produced = "counterfeit-result".into();
        takeover.completed_at = far_future;
        body.knowledge.goals = vec![takeover];
    });
    let mut network = NetworkSchedule::default();
    network.deliver_at(NOW + 10, 1, brief.clone());
    network.deliver_at(NOW + 20, 1, brief);

    assert_eq!(network.run_through(&mesh, NOW + 10), 1);
    mesh.tick(1, NOW + 11);
    let merged = goal::load_by_id(&mesh.node(1).dir, "goal-clock-poison")
        .unwrap()
        .unwrap();
    assert_eq!(merged, local, "every local field retains its authority");
    let refusals = || {
        observation::load(&mesh.node(1).dir)
            .unwrap()
            .into_iter()
            .filter(|item| {
                item.source == "mesh"
                    && item.actor == format!("mesh:{member_id}")
                    && item.action == "refused-goal-rewrite"
                    && item.object == format!("{member_id}:goal-clock-poison")
            })
            .count()
    };
    assert_eq!(refusals(), 1, "the takeover attempt is recorded once");

    assert_eq!(network.run_through(&mesh, NOW + 20), 1);
    mesh.tick(1, NOW + 21);
    assert_eq!(
        goal::load_by_id(&mesh.node(1).dir, "goal-clock-poison")
            .unwrap()
            .unwrap(),
        local,
        "replay cannot change the row"
    );
    assert_eq!(refusals(), 1, "replay does not manufacture audit spam");
}

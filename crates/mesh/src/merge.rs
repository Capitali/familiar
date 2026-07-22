//! Merge — the **constitutional half**, run inside the synchronous tick.
//!
//! The transport verifies and stashes; *this* applies. [`federate`] is the one entry the
//! cycle calls each tick. Gated by `allow_mesh` (fail-closed): when the human has not opened
//! the mesh, it is a no-op. When open, it:
//!
//! - **builds the outbound brief** from local state, redacted by `mesh/config.json` (tools +
//!   patterns by default; identities only for explicitly opted-in handles), signs it, and
//!   writes `mesh/outbox.json` for the transport to serve/gossip;
//! - **drains the inbox**: for each verified peer brief it re-verifies (defense in depth —
//!   the merge is the auditable boundary), then merges:
//!   - **tools** into the library with provenance — auto-merged, but first *use* still runs
//!     `review_script` + sandbox + `allow_execute` (unchanged);
//!   - **patterns** into pattern memory with provenance;
//!   - **peer presence** as an observation tagged `source="mesh"` and actor `mesh:<node_id>`
//!     — tagged, never laundered into local sensing or the structural fingerprint;
//!   - **identities** only for handles this node has opted into for that group.
//!
//! Every merge is deduped so re-draining the same brief each tick is idempotent.

use crate::brief::{
    sign_brief, BriefBody, Capability, ConsentedIdentityPayload, IdentityShare, Knowledge,
    MeshBrief, ObsShare, PatternOffer, Presence, ToolManifest, BRIEF_VERSION,
};
use crate::config::{self, MeshConfig};
use crate::group::{self, GroupCredential};
use crate::node::NodeKey;
use crate::transport::{INBOX_DIR, INBOX_TOOLS_DIR, OUTBOX_FILE};
use crate::{hex_encode, os_random, sha256_hex};
use familiar_kernel::boundary;
use familiar_kernel::corruption;
use familiar_kernel::guard::{self, Action, ActionKind, Decision};
use familiar_kernel::{goal, identity, observation, pattern_memory, thread, tool};
use std::collections::HashSet;
use std::path::Path;

/// The OS release for the roster ("macOS 15.5", "Ubuntu 24.04"). Best-effort and computed once —
/// std has no OS-version API, so we read the platform's own source (sw_vers / /etc/os-release).
/// Empty if it can't be determined; the roster then just shows the OS family.
pub(crate) fn os_release() -> String {
    use std::sync::OnceLock;
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| {
        #[cfg(target_os = "macos")]
        {
            if let Ok(out) = std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
            {
                let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !v.is_empty() {
                    return format!("macOS {v}");
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            if let Ok(s) = std::fs::read_to_string("/etc/os-release") {
                for line in s.lines() {
                    if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                        return rest.trim().trim_matches('"').to_string();
                    }
                }
            }
        }
        String::new()
    })
    .clone()
}

/// What a federation pass changed — folded into `ActivityTick` so the metabolism shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
    pub peers: usize,
    pub tools_merged: usize,
    pub patterns_merged: usize,
    pub observations_ingested: usize,
    pub identities_merged: usize,
    pub rejected: usize,
}

/// The one federation entry the cycle calls each tick. Best-effort: internal IO errors are
/// swallowed into the report rather than aborting the tick (mirrors `watch_camera`).
pub fn federate(dir: &Path, now: i64) -> MergeReport {
    // The human-owned gate, via the same guard every reach uses. Fail-closed.
    let allowed = boundary::load(dir)
        .map(|b| guard::evaluate(&Action::new(ActionKind::Mesh, "federate"), &b).decision)
        .map(|d| d == Decision::Allow)
        .unwrap_or(false);
    if !allowed {
        // If briefs are waiting but the boundary is shut, record the refusal once (deduped)
        // as visible truth, then leave everything untouched.
        note_refusal_if_pending(dir, now);
        return MergeReport::default();
    }
    // Requires an enrolled group (a human handed us a credential) and our node key.
    let Some(cred) = group::load(dir).ok().flatten() else {
        return MergeReport::default();
    };
    let cfg = config::load(dir).unwrap_or_default();

    // Publish our brief for the transport, then merge what peers sent us.
    let _ = build_outbox(dir, &cred, &cfg, now);
    drain_inbox(dir, &cred, &cfg, now)
}

/// Build + sign our outbound brief from local state, redacted by config, and write it to
/// `mesh/outbox.json`. Public so tests (and a future explicit "share now") can call it.
pub fn build_outbox(
    dir: &Path,
    cred: &GroupCredential,
    cfg: &MeshConfig,
    now: i64,
) -> crate::Result<()> {
    let node = NodeKey::load_or_mint(dir, "familiar")?;
    let id = node.identity();

    let people = identity::load(dir).unwrap_or_default();
    let obs = observation::load(dir).unwrap_or_default();
    let last_active = obs.iter().map(|o| o.ts).max().unwrap_or(0);

    let tools = if cfg.share_tools {
        tool_manifests(dir)
    } else {
        Vec::new()
    };
    let knowledge = if cfg.share_knowledge {
        // Replicate the recent record so peers converge on a shared memory. Bounded per brief; over
        // repeated rounds recent history propagates across the whole mesh (older history backfills
        // as long as it stays within the window of some peer — full anti-entropy is a follow-up).
        let observations = if cfg.share_observations {
            let self_node = &id.node_id;
            obs.iter()
                .rev()
                .take(OBS_SHARE_CAP)
                .filter_map(|o| {
                    obs_origin(&o.source, self_node).map(|origin| ObsShare {
                        origin,
                        actor: o.actor.clone(),
                        action: o.action.clone(),
                        object: o.object.clone(),
                        context: o.context.clone(),
                        ts: o.ts,
                        confidence_pct: (o.confidence * 100.0).round().clamp(0.0, 100.0) as u8,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        // Theories this node can't test locally, offered for a peer to test. Only when THIS node
        // cannot execute (allow_execute off) — an executor keeps its own theories and tests them.
        // A device peer / a headless node with execution gated becomes a theorist that delegates.
        let can_execute = boundary::load(dir)
            .map(|b| b.allow_execute)
            .unwrap_or(false);
        let theory_requests = if can_execute {
            Vec::new()
        } else {
            let self_node = &id.node_id;
            thread::load(dir)
                .unwrap_or_default()
                .into_iter()
                .filter(|t| t.status == "open" && !t.direction.trim().is_empty())
                .take(THEORY_SHARE_CAP)
                .map(|t| crate::brief::TheoryRequest {
                    origin: self_node.clone(),
                    thread_id: t.id,
                    question: t.question,
                    direction: t.direction,
                })
                .collect()
        };
        // The shared roadmap: every node carries the same goal list + live status so the mesh burns
        // it down together. Shared whether or not this node can execute — a theorist still needs to
        // see (and seed) goals; an executor claims them. Bounded per brief; ids are global so dedup
        // is exact on the receiver.
        let goals = goal::load(dir)
            .unwrap_or_default()
            .into_iter()
            .rev()
            .take(GOAL_SHARE_CAP)
            .map(|g| crate::brief::GoalShare {
                id: g.id,
                description: g.description,
                needs: g.needs,
                status: g.status.as_str().to_string(),
                owner_node: g.owner_node,
                origin: g.origin,
                produced: g.produced,
                notes: g.notes,
                created_at: g.created_at,
                updated_at: g.updated_at,
                status_at: g.status_at,
                last_worked_at: g.last_worked_at,
                completed_at: g.completed_at,
                ended_at: g.ended_at,
            })
            .collect();
        Knowledge {
            patterns: pattern_offers(dir),
            obs_summary: format!("{} observations", obs.len()),
            observations,
            theory_requests,
            goals,
        }
    } else {
        Knowledge::default()
    };

    // Identities: only handles explicitly opted into THIS group. Never a blanket share.
    let shared: Vec<IdentityShare> = people
        .iter()
        .filter(|p| cfg.identity_opted_in(&p.handle, &cred.group_id))
        .map(|p| IdentityShare {
            handle: p.handle.clone(),
            relation: p.relation.clone(),
            group: cred.group_id.clone(),
        })
        .collect();
    let identities = if shared.is_empty() {
        None
    } else {
        Some(ConsentedIdentityPayload { entries: shared })
    };

    // A headless node (no local human) routes its human-gated needs to human-facing peers: the
    // enrollments awaiting approval and its current open question. It never proxies gate-opening —
    // that would breach the "kernel has no boundary-write path" invariant. Authority still comes
    // from a genuine human, just one at another peer.
    let authority_requests = if cfg.headless {
        let mut reqs = Vec::new();
        for p in crate::enroll::list_pending(dir).unwrap_or_default() {
            reqs.push(crate::brief::AuthorityRequest {
                origin: id.node_id.clone(),
                kind: "enrollment".into(),
                ref_id: p.node.node_id.clone(),
                summary: format!("admit node {} ({}) to the group?", p.code, p.node.label),
            });
        }
        if let Ok(q) = std::fs::read_to_string(dir.join("question.txt")) {
            let q = q.trim();
            if !q.is_empty() {
                reqs.push(crate::brief::AuthorityRequest {
                    origin: id.node_id.clone(),
                    kind: "question".into(),
                    ref_id: "active".into(),
                    summary: q.to_string(),
                });
            }
        }
        // Need-driven gate request: a headless full peer that has open theories to build+test but
        // whose execute gate is shut asks a human (elsewhere) to open it. Tied to a real need, so it
        // never requests power it isn't using. The human decides; nothing opens without that.
        let b = boundary::load(dir).unwrap_or_else(|_| boundary::Boundary::closed());
        let has_theories = thread::load(dir)
            .unwrap_or_default()
            .iter()
            .any(|t| t.status == "open" && !t.direction.trim().is_empty());
        if !b.allow_execute && has_theories {
            reqs.push(crate::brief::AuthorityRequest {
                origin: id.node_id.clone(),
                kind: "gate".into(),
                ref_id: "allow_execute".into(),
                summary: "open my execute gate so I can build and test the theories I've formed?"
                    .into(),
            });
        }
        reqs
    } else {
        Vec::new()
    };

    // Grants this node's human has decided on peers' requests — relayed back for them to apply.
    let authority_grants = crate::grants::active(dir, now);

    let body = BriefBody {
        version: BRIEF_VERSION,
        node: id,
        membership: cred.membership.clone(),
        ts: now,
        nonce: hex_encode(&os_random::<8>()?),
        presence: Presence {
            observer_count: people.len() as u32,
            last_active,
        },
        capability: Capability {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            env_summary: node.identity().label,
            familiar_version: env!("CARGO_PKG_VERSION").to_string(),
            os_version: os_release(),
            tools,
            capabilities: familiar_kernel::capabilities::detect(
                dir,
                &boundary::load(dir).unwrap_or_else(|_| boundary::Boundary::closed()),
            ),
            // Emit 0 (omitted on the wire) until the fleet's verifiers re-serialize
            // this field — a peer built before it rejects any brief that carries it.
            build_version: 0,
            interactive: !cfg.headless,
            // Where this node is, when it can know (geo.json / IP geolocation) — so peers can
            // place it on the mesh map. 0/0 (omitted on the wire) when unknown.
            lat: crate::transport::self_geo(dir).map(|g| g.0).unwrap_or(0.0),
            lon: crate::transport::self_geo(dir).map(|g| g.1).unwrap_or(0.0),
            // The human this node serves — only a handle already opted into this group's
            // sharing (the same consent gate identity shares pass through).
            human: people
                .iter()
                .find(|p| cfg.identity_opted_in(&p.handle, &cred.group_id))
                .map(|p| p.handle.clone())
                .unwrap_or_default(),
        },
        knowledge,
        identities,
        authority_requests,
        authority_grants,
    };
    let brief = sign_brief(body, &node)?;
    if let Some(parent) = dir.join(OUTBOX_FILE).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dir.join(OUTBOX_FILE), serde_json::to_vec_pretty(&brief)?)?;
    Ok(())
}

/// Drain `mesh/inbox`, applying the merge policy to each re-verified brief.
fn drain_inbox(dir: &Path, cred: &GroupCredential, cfg: &MeshConfig, now: i64) -> MergeReport {
    let mut report = MergeReport::default();
    let inbox = dir.join(INBOX_DIR);
    let Ok(entries) = std::fs::read_dir(&inbox) else {
        return report;
    };
    let gk = match cred.verifying_key() {
        Ok(k) => k,
        Err(_) => return report,
    };
    let revoked = group::load_revoked(dir).unwrap_or_default();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(brief) = serde_json::from_slice::<MeshBrief>(&bytes) else {
            let _ = std::fs::remove_file(&path);
            continue;
        };
        // Re-verify at the constitutional boundary (transport already checked at ingress).
        if crate::brief::verify_brief(&brief, &gk, &cred.group_id, now, &revoked).is_err() {
            report.rejected += 1;
            record_obs(
                dir,
                "mesh",
                "rejected_brief",
                &brief.body.node.node_id,
                "a brief failed re-verification at merge (untrusted/expired/revoked)",
                now,
            );
            let _ = std::fs::remove_file(&path); // don't re-reject the same junk each tick
            continue;
        }
        report.peers += 1;
        merge_one(dir, &brief, cfg, cred, now, &mut report);
    }
    report
}

/// Apply the merge policy for a single trusted brief.
fn merge_one(
    dir: &Path,
    brief: &MeshBrief,
    cfg: &MeshConfig,
    cred: &GroupCredential,
    now: i64,
    report: &mut MergeReport,
) {
    let node_id = &brief.body.node.node_id;

    // --- Corruption-awareness, at the covenant scale. A member that repeatedly tries to breach the
    // constitution earns a graduated, reversible loss of standing (monitor → throttle → marginalize
    // → sever). We compute its tier once and let each block below consult it: throttled peers lose
    // their *directives*, marginalized peers lose their *content* too, a severed peer is dropped
    // wholesale and raised to the human for a revoke decision. All reversible as refusals age out. ---
    let tier = {
        let refusals = corruption::load(dir).unwrap_or_default();
        corruption::trust(&refusals, &format!("mesh:{node_id}"), now)
    };
    if tier == corruption::Trust::Severed {
        // Still note the peer exists (presence is knowledge, not influence), then stop.
        let ctx = format!(
            "peer {} — SEVERED (repeated covenant breaches)",
            brief.body.node.label
        );
        if record_mesh_presence(dir, node_id, &ctx, now) {
            report.observations_ingested += 1;
        }
        if recommend_revoke(dir, node_id, now) {
            report.observations_ingested += 1;
        }
        return;
    }

    // --- Tools: auto-merge into the library with provenance (use is still fully gated). ---
    if !tier.shapes_worldview() {
        // A marginalized peer's offerings no longer shape us — skip tools/patterns/observations.
        record_marginalized(dir, node_id, &brief.body.node.label, now, report);
        return;
    }
    let known = known_tool_shas(dir);
    let mut seq = tool::load(dir).map(|t| t.len()).unwrap_or(0);
    for m in &brief.body.capability.tools {
        if known.contains(&m.script_sha256) {
            continue;
        }
        let body_path = inbox_tool_path(dir, &m.script_sha256);
        let Ok(body) = std::fs::read(&body_path) else {
            continue;
        }; // not fetched yet; next tick
        if sha256_hex(&body) != m.script_sha256 {
            continue; // integrity mismatch — never install
        }
        // Persist the body into the workspace and index it with provenance.
        let ws = crate::merge_workspace(dir);
        if std::fs::create_dir_all(&ws).is_err() {
            continue;
        }
        seq += 1;
        let id = format!("tool-{seq:04}");
        let script_path = ws.join(format!("{id}.sh"));
        if std::fs::write(&script_path, &body).is_err() {
            continue;
        }
        let t = tool::Tool {
            id,
            name: m.name.clone(),
            purpose: m.purpose.clone(),
            keywords: m.keywords.join(" "),
            script_path: script_path.display().to_string(),
            created_at: now,
            uses: 0,
            last_used: 0,
            last_exit_ok: m.last_exit_ok,
            last_status: String::new(),
            origin: node_id.clone(),
            origin_verified_at: now,
        };
        if tool::append(dir, &t).is_ok() {
            report.tools_merged += 1;
        }
    }

    // --- Patterns: merge into pattern memory with provenance, deduped. ---
    let existing = pattern_memory::load(dir).unwrap_or_default();
    let mut pseq = existing.len();
    for offer in &brief.body.knowledge.patterns {
        let dup = existing
            .iter()
            .any(|p| p.name == offer.key && p.lesson == offer.summary);
        if dup {
            continue;
        }
        pseq += 1;
        let pm = pattern_memory::PatternMemory {
            id: format!("pattern-{pseq:04}"),
            name: offer.key.clone(),
            lesson: offer.summary.clone(),
            applies_when: format!("origin=mesh:{node_id}"),
            positive_evidence: format!("federated from mesh:{node_id} (support {})", offer.support),
            negative_evidence: String::new(),
            confidence: (offer.support as f64 / 10.0).clamp(0.1, 0.9),
        };
        if pattern_memory::append(dir, &pm).is_ok() {
            report.patterns_merged += 1;
        }
    }

    // --- Peer presence: a tagged observation, never laundered into local sensing. ---
    let ctx = format!(
        "peer {} — {} observer(s), {}",
        brief.body.node.label, brief.body.presence.observer_count, brief.body.knowledge.obs_summary,
    );
    if record_mesh_presence(dir, node_id, &ctx, now) {
        report.observations_ingested += 1;
    }

    // --- Observations: replicate the shared record so this node backs up what the peer knows.
    // Deduped by a content hash of (origin, actor, action, object, ts) — ids are node-local, so we
    // can't key on them — and tagged `mesh:<origin>` so replicated data stays quarantined from
    // local sensing/fingerprint (same discipline as presence). We never re-ingest our own. ---
    if cfg.share_observations && !brief.body.knowledge.observations.is_empty() {
        let self_node = &cred.membership.node_id;
        let mut seen: std::collections::HashSet<String> = observation::load(dir)
            .unwrap_or_default()
            .iter()
            .filter_map(|o| {
                obs_origin(&o.source, self_node)
                    .map(|origin| obs_key(&origin, &o.actor, &o.action, &o.object, o.ts))
            })
            .collect();
        for s in &brief.body.knowledge.observations {
            if &s.origin == self_node {
                continue; // our own observation echoed back — never re-ingest
            }
            let key = obs_key(&s.origin, &s.actor, &s.action, &s.object, s.ts);
            if !seen.insert(key) {
                continue; // already have it (deduped across peers and rounds)
            }
            let o = observation::Observation::new(
                s.actor.clone(),
                s.action.clone(),
                s.object.clone(),
                s.context.clone(),
                format!("mesh:{}", s.origin),
                s.ts,
                (s.confidence_pct as f64 / 100.0).clamp(0.0, 1.0),
            );
            if observation::record(dir, o).is_ok() {
                report.observations_ingested += 1;
            }
        }
    }

    // --- Goals: the shared roadmap. Adopt goals we don't have; for ones we do, take the peer's
    // version if it is strictly NEWER (last-writer-wins by updated_at) so a claim, a progress note,
    // or a completion propagates to every node. We never regress our own newer state, and we never
    // let a peer un-settle a goal we've moved past. Reached only by trusted/throttled peers (a
    // marginalized peer returned early above), so a slipped peer can't rewrite the roadmap. ---
    for gs in &brief.body.knowledge.goals {
        let incoming_status = match gs.status.as_str() {
            "proposed" => goal::Status::Proposed,
            "claimed" => goal::Status::Claimed,
            "in_progress" => goal::Status::InProgress,
            "awaiting_human" => goal::Status::AwaitingHuman,
            "done" => goal::Status::Done,
            "failed" => goal::Status::Failed,
            "blocked" => goal::Status::Blocked,
            _ => continue, // an unknown status from a newer peer — leave it be
        };
        // Lifecycle dates travel with the goal. A brief from a pre-stamp build sends zeros —
        // fall back to the best date it *did* send, so every status still carries a date.
        let goal_from_share = |gs: &crate::brief::GoalShare, local: Option<&goal::Goal>| {
            let terminal_at = |flag: bool, incoming: i64, kept: i64| {
                if incoming > 0 {
                    incoming
                } else if kept > 0 {
                    kept
                } else if flag {
                    gs.updated_at
                } else {
                    0
                }
            };
            goal::Goal {
                id: gs.id.clone(),
                description: gs.description.clone(),
                needs: gs.needs.clone(),
                status: incoming_status,
                owner_node: gs.owner_node.clone(),
                origin: gs.origin.clone(),
                produced: gs.produced.clone(),
                notes: gs.notes.clone(),
                created_at: gs.created_at,
                updated_at: gs.updated_at,
                status_at: if gs.status_at > 0 {
                    gs.status_at
                } else {
                    gs.updated_at
                },
                last_worked_at: gs
                    .last_worked_at
                    .max(local.map(|l| l.last_worked_at).unwrap_or(0)),
                completed_at: terminal_at(
                    incoming_status == goal::Status::Done,
                    gs.completed_at,
                    local.map(|l| l.completed_at).unwrap_or(0),
                ),
                ended_at: terminal_at(
                    incoming_status == goal::Status::Failed,
                    gs.ended_at,
                    local.map(|l| l.ended_at).unwrap_or(0),
                ),
            }
        };
        match goal::load_by_id(dir, &gs.id).ok().flatten() {
            Some(local) if local.updated_at >= gs.updated_at => {} // ours is as-new or newer — keep it
            Some(local) => {
                let merged = goal_from_share(gs, Some(&local));
                if goal::update(dir, &merged).is_ok() {
                    report.observations_ingested += 1;
                }
            }
            None => {
                let adopted = goal_from_share(gs, None);
                if goal::append(dir, &adopted).is_ok() {
                    report.observations_ingested += 1;
                }
            }
        }
    }

    // --- Theory delegation: a peer that can't test its own theories asks us to. If WE can execute,
    // adopt each federated theory as a local thread so pursue_threads tests it (candidate → test →
    // select), and the outcome replicates home via the shared observation record. Deduped by
    // (origin, direction) against threads we already hold. We lend our execution to a peer's ideas. ---
    if !brief.body.knowledge.theory_requests.is_empty()
        && tier.heeds_directives()
        && boundary::load(dir)
            .map(|b| b.allow_execute)
            .unwrap_or(false)
    {
        let existing = thread::load(dir).unwrap_or_default();
        let held: std::collections::HashSet<String> = existing
            .iter()
            .map(|t| format!("{}\u{1}{}", t.actor, t.direction.trim().to_lowercase()))
            .collect();
        let mut tseq = existing.len();
        for req in &brief.body.knowledge.theory_requests {
            if req.origin == cred.membership.node_id || req.direction.trim().is_empty() {
                continue; // our own, echoed back
            }
            let origin_actor = format!("mesh:{}", req.origin);
            let key = format!(
                "{}\u{1}{}",
                origin_actor,
                req.direction.trim().to_lowercase()
            );
            if held.contains(&key) {
                continue; // already adopted this peer's theory
            }
            tseq += 1;
            let t = thread::Thread {
                id: format!("thread-{tseq:04}"),
                question: req.question.clone(),
                theory: format!("delegated by {}", short(&req.origin)),
                direction: req.direction.clone(),
                created_at: now,
                status: "open".into(),
                status_at: now,
                last_worked_at: 0,
                origin: "mesh".into(),
                // Attribute to the originating node so corruption-awareness still governs it and its
                // outcome can be traced home. A peer's theory, tested on our execution.
                actor: origin_actor.clone(),
            };
            if thread::append(dir, &t).is_ok() {
                // A visible, replicating record that we picked up a peer's theory to test — source
                // "familiar" so it originates here and flows back to the peer via observation sharing.
                record_obs(
                    dir,
                    "familiar",
                    "adopted-theory",
                    &format!("theory:{}", req.thread_id),
                    &format!(
                        "testing a theory delegated by {} — '{}'",
                        short(&req.origin),
                        req.direction
                    ),
                    now,
                );
                report.observations_ingested += 1;
            }
        }
    }

    // --- Authority proxy: a headless peer has no local human, so it routes its human-gated needs
    // (a pending enrollment, an open question) to us. If WE have a local human (not headless), make
    // them visible so the human can act. Increment 1 surfaces them as observations; the grant-return
    // loop (remote approve → the peer applies it) is the next step. Deduped by (origin,kind,ref).
    // We never proxy gate-opening — a boundary is opened only by a human at the node it governs. ---
    if !brief.body.authority_requests.is_empty()
        && tier.heeds_directives()
        && !config::load(dir).map(|c| c.headless).unwrap_or(false)
    {
        let seen: std::collections::HashSet<String> = observation::load(dir)
            .unwrap_or_default()
            .iter()
            .filter(|o| o.action == "asked-to-decide")
            .map(|o| o.object.clone())
            .collect();
        for req in &brief.body.authority_requests {
            let obj = format!("{}:{}:{}", req.kind, node_id, req.ref_id);
            if seen.contains(&obj) {
                continue;
            }
            record_obs(
                dir,
                &format!("mesh:{node_id}"),
                "asked-to-decide",
                &obj,
                &format!("peer {} needs a human: {}", short(node_id), req.summary),
                now,
            );
            report.observations_ingested += 1;
        }
    }

    // --- Authority grants: a human at the sending peer decided one of OUR requests. Apply it — the
    // one place an external human's authority reaches this node. The grant rides the peer's signed,
    // group-verified brief (authenticated as "this member asserts a human decided X"); we trust a
    // member under the covenant, and corruption-awareness marginalizes one that abuses it. Applied
    // once (dedup by observing our own audit trail). This includes the sole boundary-write path:
    // opening a gate WE requested, only on an approved human grant — never the autonomous cycle. A
    // throttled-or-worse peer's grants do not act on us (`heeds_directives` — behavior, not person). ---
    if tier.heeds_directives() {
        let me = &cred.membership.node_id;
        let applied: std::collections::HashSet<String> = observation::load(dir)
            .unwrap_or_default()
            .iter()
            .filter(|o| o.action == "applied-grant")
            .map(|o| o.object.clone())
            .collect();
        for grant in &brief.body.authority_grants {
            if &grant.target != me {
                continue; // not for us
            }
            let marker = format!("{}:{}:{}", grant.kind, grant.ref_id, grant.approved);
            if applied.contains(&marker) {
                continue;
            }
            let outcome = apply_authority_grant(dir, node_id, grant, now);
            if let Some(note) = outcome {
                record_obs(dir, "familiar", "applied-grant", &marker, &note, now);
                report.observations_ingested += 1;
            }
        }
    }

    // --- Identities: opt-in only, per handle + group. ---
    if let Some(payload) = &brief.body.identities {
        for share in &payload.entries {
            if share.group != cred.group_id {
                continue; // scoped share — ignore anything outside our group
            }
            if !cfg.identity_opted_in(&share.handle, &cred.group_id) {
                continue; // we have not opted in to receive this human
            }
            if merge_identity(dir, share, node_id, now) {
                report.identities_merged += 1;
            }
        }
    }
}

// ---- outbound builders --------------------------------------------------------------

fn tool_manifests(dir: &Path) -> Vec<ToolManifest> {
    tool::load(dir)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|t| {
            let body = std::fs::read(&t.script_path).ok()?;
            Some(ToolManifest {
                tool_id: t.id,
                name: t.name,
                purpose: t.purpose,
                keywords: t.keywords.split_whitespace().map(String::from).collect(),
                script_sha256: sha256_hex(&body),
                uses: t.uses as u64,
                last_exit_ok: t.last_exit_ok,
            })
        })
        .collect()
}

fn pattern_offers(dir: &Path) -> Vec<PatternOffer> {
    pattern_memory::load(dir)
        .unwrap_or_default()
        .into_iter()
        // Don't re-offer patterns we merged from a peer — offer only what this node learned.
        .filter(|p| !p.applies_when.contains("origin=mesh:"))
        .map(|p| PatternOffer {
            key: p.name,
            summary: p.lesson,
            support: (p.confidence * 10.0) as u32,
        })
        .collect()
}

// ---- helpers ------------------------------------------------------------------------

fn known_tool_shas(dir: &Path) -> HashSet<String> {
    tool::load(dir)
        .unwrap_or_default()
        .iter()
        .filter_map(|t| std::fs::read(&t.script_path).ok())
        .map(|b| sha256_hex(&b))
        .collect()
}

fn inbox_tool_path(dir: &Path, sha: &str) -> std::path::PathBuf {
    let safe: String = sha.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    dir.join(INBOX_TOOLS_DIR).join(format!("{safe}.script"))
}

/// Record a peer-presence observation, deduped: skip if the newest mesh observation from
/// this node already has this exact context (so a static peer doesn't flood the log).
fn record_mesh_presence(dir: &Path, node_id: &str, ctx: &str, now: i64) -> bool {
    let actor = format!("mesh:{node_id}");
    let already = observation::load(dir)
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|o| o.actor == actor && o.action == "reports")
        .map(|o| o.context == ctx)
        .unwrap_or(false);
    if already {
        return false;
    }
    record_obs(dir, &actor, "reports", "presence", ctx, now);
    true
}

/// A marginalized peer contributes only its presence — note it (so the roster still shows the peer
/// and its tier), skip everything it offers. Returns how many observations were newly recorded.
fn record_marginalized(dir: &Path, node_id: &str, label: &str, now: i64, report: &mut MergeReport) {
    let ctx = format!("peer {label} — MARGINALIZED (repeated covenant breaches; content ignored)");
    if record_mesh_presence(dir, node_id, &ctx, now) {
        report.observations_ingested += 1;
    }
}

/// Raise a **revoke recommendation** for a severed peer to the human — permanent expulsion
/// (`mesh/revoked.json`) is theirs to decide, the mirror of admitting a member. Deduped: recorded
/// once while the peer stays severed (a later grant/approval or the tier relaxing lets it recur).
/// Returns true if a new recommendation was recorded.
fn recommend_revoke(dir: &Path, node_id: &str, now: i64) -> bool {
    let object = format!("peer:{node_id}");
    let already = observation::load(dir)
        .unwrap_or_default()
        .iter()
        .any(|o| o.action == "recommend-revoke" && o.object == object);
    if already {
        return false;
    }
    record_obs(
        dir,
        "familiar",
        "recommend-revoke",
        &object,
        &format!(
            "peer {} crossed the sever line (repeated covenant breaches) — recommend revoking it",
            short(node_id)
        ),
        now,
    );
    true
}

/// Merge a federated identity with provenance, deduped by handle. Returns true if added.
fn merge_identity(dir: &Path, share: &IdentityShare, node_id: &str, now: i64) -> bool {
    let people = identity::load(dir).unwrap_or_default();
    if people.iter().any(|p| p.handle == share.handle) {
        return false; // already known locally — don't overwrite a local record
    }
    let rec = identity::Identity {
        handle: share.handle.clone(),
        name: share.handle.clone(),
        relation: format!("federated:{node_id}"),
        first_seen: now,
        last_seen: now,
        interactions: 0,
    };
    familiar_kernel::store::append(dir, identity::IDENTITY_FILE, &rec).is_ok()
}

/// How many recent observations a single brief carries. Bounds brief size; recent history still
/// converges across the mesh over repeated rounds.
const OBS_SHARE_CAP: usize = 200;

/// How many un-testable theories a node offers per brief. Bounded so a theorist can't flood an
/// executor; the rest ride on later rounds.
const THEORY_SHARE_CAP: usize = 20;

/// How many goals a brief carries — the whole shared roadmap is small, but bound it so a runaway
/// seeder can't bloat the brief. Newest first; the rest converge over later rounds.
const GOAL_SHARE_CAP: usize = 60;

/// First 8 chars of a node id, for human-facing lines.
fn short(node_id: &str) -> String {
    node_id.chars().take(8).collect()
}

/// The origin node of an observation, for replication dedup + provenance. A `mesh:<node>...`
/// source names the node it came from; bare `mesh` (peer presence) has no shareable origin; anything
/// else was born on this node, so its origin is us. Returns `None` for records not worth replicating.
fn obs_origin(source: &str, self_node: &str) -> Option<String> {
    if source == "mesh" {
        return None; // peer-presence marker, not a shareable observation
    }
    if let Some(rest) = source.strip_prefix("mesh:") {
        let node = rest.split(['#', ':']).next().unwrap_or("");
        return if node.is_empty() {
            None
        } else {
            Some(node.to_string())
        };
    }
    Some(self_node.to_string())
}

/// A stable, cross-node dedup key for an observation: a content hash of its origin + triple + time.
/// Ids are node-local, so two nodes that hold the same observation agree only on these fields.
fn obs_key(origin: &str, actor: &str, action: &str, object: &str, ts: i64) -> String {
    let material = format!("{origin}\u{1}{actor}\u{1}{action}\u{1}{object}\u{1}{ts}");
    sha256_hex(material.as_bytes())[..16].to_string()
}

/// Gates a remote human grant is allowed to open — the reach/build capabilities a headless peer
/// asks for. `allow_mesh` is excluded (it must already be open to receive the grant) and the
/// sandbox toggle is excluded (loosening the jail is a local-only choice).
const GRANTABLE_GATES: &[&str] = &[
    "allow_execute",
    "allow_authored_execute",
    "allow_llm",
    "allow_network",
    "allow_tool_install",
    "allow_agent",
    "allow_camera",
];

/// Apply one authenticated authority grant addressed to this node. Returns a human-facing audit note
/// on success, `None` if it was a no-op/invalid. This is the boundary-write path — reached ONLY here,
/// on an approved human grant relayed by a trusted member; the autonomous cycle never writes gates.
fn apply_authority_grant(
    dir: &Path,
    granting_node: &str,
    grant: &crate::brief::AuthorityGrant,
    now: i64,
) -> Option<String> {
    match grant.kind.as_str() {
        "enrollment" => {
            if grant.approved {
                match crate::enroll::approve(dir, &grant.ref_id, now) {
                    Ok(g) => Some(format!(
                        "admitted node {} to “{}” — approved by a human at peer {}",
                        short(&grant.ref_id),
                        g.group_label,
                        short(granting_node)
                    )),
                    Err(_) => None,
                }
            } else {
                match crate::enroll::deny(dir, &grant.ref_id) {
                    Ok(true) => Some(format!(
                        "declined node {}'s join — decided by a human at peer {}",
                        short(&grant.ref_id),
                        short(granting_node)
                    )),
                    _ => None,
                }
            }
        }
        "question" => {
            if grant.approved && !grant.note.trim().is_empty() {
                // The remote human answered our open question — record it as observer input and
                // retire the question so the cycle moves on.
                record_obs(
                    dir,
                    "ian",
                    "answered",
                    grant.note.trim(),
                    &format!("via a human at peer {}", short(granting_node)),
                    now,
                );
                let _ = std::fs::write(dir.join("question.txt"), "");
                let _ = std::fs::write(dir.join("active_question.txt"), "");
                Some(format!(
                    "a human at peer {} answered: {}",
                    short(granting_node),
                    grant.note.trim()
                ))
            } else {
                None
            }
        }
        "gate" => {
            if !grant.approved || !GRANTABLE_GATES.contains(&grant.ref_id.as_str()) {
                return None;
            }
            let mut b = boundary::load(dir).unwrap_or_else(|_| boundary::Boundary::closed());
            let already = match grant.ref_id.as_str() {
                "allow_execute" => &mut b.allow_execute,
                "allow_authored_execute" => &mut b.allow_authored_execute,
                "allow_llm" => &mut b.allow_llm,
                "allow_network" => &mut b.allow_network,
                "allow_tool_install" => &mut b.allow_tool_install,
                "allow_agent" => &mut b.allow_agent,
                "allow_camera" => &mut b.allow_camera,
                _ => return None,
            };
            if *already {
                return None; // already open — nothing to do
            }
            *already = true;
            // Persist the boundary — the sole authorized boundary-write, driven by a human's grant.
            let json = serde_json::to_string_pretty(&b).ok()?;
            std::fs::write(dir.join(boundary::BOUNDARY_FILE), json).ok()?;
            Some(format!(
                "opened gate {} — authorized by a human at peer {} (Law III: a human, not the cycle, opened it)",
                grant.ref_id,
                short(granting_node)
            ))
        }
        _ => None,
    }
}

fn record_obs(dir: &Path, actor: &str, action: &str, object: &str, ctx: &str, now: i64) {
    let _ = observation::record(
        dir,
        observation::Observation::new(actor, action, object, ctx, "mesh", now, 0.9),
    );
}

/// When the boundary is closed but briefs are waiting, record the refusal once (deduped by
/// a constant object) so the human sees the mesh being held shut — then merge nothing.
fn note_refusal_if_pending(dir: &Path, now: i64) {
    let inbox = dir.join(INBOX_DIR);
    let pending = std::fs::read_dir(&inbox)
        .map(|d| {
            d.flatten()
                .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        })
        .unwrap_or(false);
    if !pending {
        return;
    }
    let already = observation::load(dir)
        .unwrap_or_default()
        .iter()
        .any(|o| o.actor == "familiar" && o.action == "refused" && o.object == "mesh_federation");
    if !already {
        record_obs(
            dir,
            "familiar",
            "refused",
            "mesh_federation",
            "peer briefs are waiting but allow_mesh is closed — nothing merged (Law III)",
            now,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::DEFAULT_CERT_TTL_SECS;
    use std::fs;

    const NOW: i64 = 1_770_000_000;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("familiar_mesh_merge_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn open_mesh_boundary(dir: &Path) {
        let mut b = boundary::Boundary::closed();
        b.allow_mesh = true;
        fs::write(
            dir.join(boundary::BOUNDARY_FILE),
            serde_json::to_string(&b).unwrap(),
        )
        .unwrap();
    }

    /// Build a signed brief from `author`'s node/group offering one tool + one pattern.
    fn peer_brief_with_tool(
        author: &NodeKey,
        cred: &GroupCredential,
        tool_body: &[u8],
    ) -> MeshBrief {
        let body = BriefBody {
            version: BRIEF_VERSION,
            node: author.identity(),
            membership: cred.membership.clone(),
            ts: NOW,
            nonce: "n".into(),
            presence: Presence {
                observer_count: 2,
                last_active: NOW,
            },
            capability: Capability {
                os: "linux".into(),
                arch: "arm".into(),
                env_summary: "cpn".into(),
                familiar_version: "0.1.0".into(),
                os_version: String::new(),
                interactive: false,
                human: String::new(),
                tools: vec![ToolManifest {
                    tool_id: "tool-0007".into(),
                    name: "battery".into(),
                    purpose: "read pack soc".into(),
                    keywords: vec!["battery".into(), "soc".into()],
                    script_sha256: sha256_hex(tool_body),
                    uses: 5,
                    last_exit_ok: true,
                }],
                capabilities: Vec::new(),
                build_version: 0,
                lat: 0.0,
                lon: 0.0,
            },
            knowledge: Knowledge {
                patterns: vec![PatternOffer {
                    key: "dawn-poll".into(),
                    summary: "battery polled at first light".into(),
                    support: 6,
                }],
                obs_summary: "88 observations".into(),
                observations: Vec::new(),
                theory_requests: Vec::new(),
                goals: Vec::new(),
            },
            identities: None,
            authority_requests: Vec::new(),
            authority_grants: Vec::new(),
        };
        sign_brief(body, author).unwrap()
    }

    #[test]
    fn an_approved_grant_opens_the_targets_gate_and_admits_an_enrollment() {
        // Target T (headless) receives grants from peer H (whose human decided).
        let dir_t = tmp("grant_target");
        let t_node = NodeKey::load_or_mint(&dir_t, "target").unwrap();
        let cred = group::create_group(&dir_t, &t_node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        // Mesh open (to receive), execute CLOSED (the gate the grant will open).
        let mut bnd = boundary::Boundary::closed();
        bnd.allow_mesh = true;
        bnd.allow_execute = false;
        fs::write(
            dir_t.join(boundary::BOUNDARY_FILE),
            serde_json::to_string(&bnd).unwrap(),
        )
        .unwrap();

        // A third node X has a pending enrollment at T (so the enrollment grant has something to act on).
        let x = NodeKey::load_or_mint(&tmp("grant_joiner"), "joiner").unwrap();
        let xid = x.identity();
        let req = crate::enroll::EnrollRequest {
            node: xid.clone(),
            attestation: crate::enroll::Attestation {
                laws_version: crate::enroll::LAWS_VERSION,
                statement: "I accept the Three Laws.".into(),
                ts: NOW,
            },
            nonce: "n1".into(),
            ts: NOW,
        };
        let raw = serde_json::to_vec(&req).unwrap();
        let sig = x.sign(&raw);
        crate::enroll::submit_request(&dir_t, &raw, &sig, NOW).unwrap();
        assert_eq!(crate::enroll::list_pending(&dir_t).unwrap().len(), 1);

        // Peer H (in the group) relays two approved grants addressed to T.
        let dir_h = tmp("grant_human");
        let h_node = NodeKey::load_or_mint(&dir_h, "human").unwrap();
        let cred_h = group::join_group(
            &dir_h,
            &h_node,
            &cred.join_key(),
            "g",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();
        let mut body = peer_brief_with_tool(&h_node, &cred_h, b"#!/bin/sh\n").body;
        body.authority_grants = vec![
            crate::brief::AuthorityGrant {
                by: h_node.node_id(),
                target: t_node.node_id(),
                kind: "gate".into(),
                ref_id: "allow_execute".into(),
                approved: true,
                note: String::new(),
                ts: NOW,
            },
            crate::brief::AuthorityGrant {
                by: h_node.node_id(),
                target: t_node.node_id(),
                kind: "enrollment".into(),
                ref_id: xid.node_id.clone(),
                approved: true,
                note: String::new(),
                ts: NOW,
            },
        ];
        let brief = sign_brief(body, &h_node).unwrap();
        fs::create_dir_all(dir_t.join(INBOX_DIR)).unwrap();
        fs::write(
            dir_t
                .join(INBOX_DIR)
                .join(format!("{}.json", h_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();

        federate(&dir_t, NOW + 1);

        // The gate is now open — but only because a human at H authorized it (audited).
        assert!(
            boundary::load(&dir_t).unwrap().allow_execute,
            "the human-granted gate opened"
        );
        // The enrollment was admitted (no longer pending).
        assert!(
            crate::enroll::list_pending(&dir_t).unwrap().is_empty(),
            "the enrollment was admitted"
        );
        let obs = observation::load(&dir_t).unwrap();
        assert_eq!(
            obs.iter().filter(|o| o.action == "applied-grant").count(),
            2,
            "both grants audited"
        );

        // Idempotent: re-draining applies nothing new.
        fs::write(
            dir_t
                .join(INBOX_DIR)
                .join(format!("{}.json", h_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();
        federate(&dir_t, NOW + 2);
        assert_eq!(
            observation::load(&dir_t)
                .unwrap()
                .iter()
                .filter(|o| o.action == "applied-grant")
                .count(),
            2,
            "grants apply once"
        );

        let _ = fs::remove_dir_all(&dir_t);
        let _ = fs::remove_dir_all(&dir_h);
    }

    #[test]
    fn a_headless_peers_authority_needs_surface_at_a_human_facing_peer() {
        // Human-facing receiver B (not headless); headless peer A routes an enrollment approval.
        let dir_b = tmp("auth_recv");
        let b_node = NodeKey::load_or_mint(&dir_b, "beta").unwrap();
        let cred = group::create_group(&dir_b, &b_node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        open_mesh_boundary(&dir_b); // B has a human (headless defaults false)

        let dir_a = tmp("auth_peer");
        let a_node = NodeKey::load_or_mint(&dir_a, "alpha").unwrap();
        let cred_a = group::join_group(
            &dir_a,
            &a_node,
            &cred.join_key(),
            "g",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();

        let mut body = peer_brief_with_tool(&a_node, &cred_a, b"#!/bin/sh\n").body;
        body.authority_requests = vec![crate::brief::AuthorityRequest {
            origin: a_node.node_id(),
            kind: "enrollment".into(),
            ref_id: "newnode123".into(),
            summary: "admit node abc123 (kali-jeff) to the group?".into(),
        }];
        let brief = sign_brief(body, &a_node).unwrap();

        fs::create_dir_all(dir_b.join(INBOX_DIR)).unwrap();
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();

        federate(&dir_b, NOW + 1);

        // B's human can now see that peer A needs a decision.
        let obs = observation::load(&dir_b).unwrap();
        let ask = obs
            .iter()
            .find(|o| o.action == "asked-to-decide")
            .expect("surfaced for the human");
        assert!(ask.object.starts_with("enrollment:"));
        assert!(ask.context.contains("kali-jeff"));

        // Idempotent: re-draining doesn't surface it twice.
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();
        federate(&dir_b, NOW + 2);
        let n = observation::load(&dir_b)
            .unwrap()
            .iter()
            .filter(|o| o.action == "asked-to-decide")
            .count();
        assert_eq!(n, 1, "an authority request surfaces once, not every round");

        let _ = fs::remove_dir_all(&dir_b);
        let _ = fs::remove_dir_all(&dir_a);
    }

    #[test]
    fn an_executor_adopts_a_peers_untestable_theory_and_tests_it() {
        // B can execute (allow_execute on); peer A (a theorist that can't) delegates a theory.
        let dir_b = tmp("delegate_recv");
        let b_node = NodeKey::load_or_mint(&dir_b, "beta").unwrap();
        let cred = group::create_group(&dir_b, &b_node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let mut bnd = boundary::Boundary::closed();
        bnd.allow_mesh = true;
        bnd.allow_execute = true; // B is an executor
        fs::write(
            dir_b.join(boundary::BOUNDARY_FILE),
            serde_json::to_string(&bnd).unwrap(),
        )
        .unwrap();

        let dir_a = tmp("delegate_peer");
        let a_node = NodeKey::load_or_mint(&dir_a, "alpha").unwrap();
        let cred_a = group::join_group(
            &dir_a,
            &a_node,
            &cred.join_key(),
            "g",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();

        let mut body = peer_brief_with_tool(&a_node, &cred_a, b"#!/bin/sh\n").body;
        body.knowledge.theory_requests = vec![crate::brief::TheoryRequest {
            origin: a_node.node_id(),
            thread_id: "thread-0007".into(),
            question: "What eases mornings?".into(),
            direction: "offer a standing morning digest".into(),
        }];
        let brief = sign_brief(body, &a_node).unwrap();

        fs::create_dir_all(dir_b.join(INBOX_DIR)).unwrap();
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();

        federate(&dir_b, NOW + 1);

        // B adopted A's theory as a local open thread it will pursue (attributed to A's node).
        let threads = familiar_kernel::thread::load(&dir_b).unwrap();
        let adopted = threads
            .iter()
            .find(|t| t.direction == "offer a standing morning digest")
            .expect("executor adopted the delegated theory");
        assert_eq!(adopted.status, "open");
        assert_eq!(adopted.actor, format!("mesh:{}", a_node.node_id()));
        // And it recorded a replicating note that it's testing the peer's theory.
        let obs = observation::load(&dir_b).unwrap();
        assert!(obs.iter().any(|o| o.action == "adopted-theory"));

        // Idempotent: a second round doesn't re-adopt the same theory.
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();
        federate(&dir_b, NOW + 2);
        let n = familiar_kernel::thread::load(&dir_b)
            .unwrap()
            .iter()
            .filter(|t| t.direction == "offer a standing morning digest")
            .count();
        assert_eq!(
            n, 1,
            "the delegated theory is adopted once, not every round"
        );

        let _ = fs::remove_dir_all(&dir_b);
        let _ = fs::remove_dir_all(&dir_a);
    }

    #[test]
    fn a_non_executor_offers_its_open_theories_but_an_executor_does_not() {
        // A theorist (allow_execute off) shares its open directional theories in its brief.
        let dir = tmp("delegate_share");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let cred = group::create_group(&dir, &node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let mut bnd = boundary::Boundary::closed();
        bnd.allow_mesh = true;
        bnd.allow_execute = false; // a theorist, not an executor
        fs::write(
            dir.join(boundary::BOUNDARY_FILE),
            serde_json::to_string(&bnd).unwrap(),
        )
        .unwrap();
        familiar_kernel::thread::append(
            &dir,
            &familiar_kernel::thread::Thread {
                id: "thread-0001".into(),
                question: "q".into(),
                theory: "th".into(),
                direction: "try a gentle nudge".into(),
                status_at: 0,
                last_worked_at: 0,
                created_at: NOW,
                status: "open".into(),
                origin: "llm".into(),
                actor: "familiar".into(),
            },
        )
        .unwrap();

        let cfg = MeshConfig::default();
        build_outbox(&dir, &cred, &cfg, NOW + 1).unwrap();
        let brief: MeshBrief =
            serde_json::from_str(&fs::read_to_string(dir.join(OUTBOX_FILE)).unwrap()).unwrap();
        assert_eq!(
            brief.body.knowledge.theory_requests.len(),
            1,
            "a theorist offers its theory"
        );
        assert_eq!(
            brief.body.knowledge.theory_requests[0].direction,
            "try a gentle nudge"
        );

        // Flip to executor: it keeps its theories to itself (tests them locally instead).
        bnd.allow_execute = true;
        fs::write(
            dir.join(boundary::BOUNDARY_FILE),
            serde_json::to_string(&bnd).unwrap(),
        )
        .unwrap();
        build_outbox(&dir, &cred, &cfg, NOW + 2).unwrap();
        let brief2: MeshBrief =
            serde_json::from_str(&fs::read_to_string(dir.join(OUTBOX_FILE)).unwrap()).unwrap();
        assert!(
            brief2.body.knowledge.theory_requests.is_empty(),
            "an executor delegates nothing"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merges_tool_and_pattern_with_provenance_and_is_idempotent() {
        // Receiver B enrolls a group; a peer A in the same group offers a tool + pattern.
        let dir_b = tmp("recv");
        let b_node = NodeKey::load_or_mint(&dir_b, "beta").unwrap();
        let cred = group::create_group(&dir_b, &b_node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        open_mesh_boundary(&dir_b);

        // Peer A shares the SAME group secret (join key), so its cert verifies for B.
        let dir_a = tmp("peer");
        let a_node = NodeKey::load_or_mint(&dir_a, "alpha").unwrap();
        let cred_a = group::join_group(
            &dir_a,
            &a_node,
            &cred.join_key(),
            "g",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();

        let tool_body = b"#!/bin/sh\necho soc\n";
        let brief = peer_brief_with_tool(&a_node, &cred_a, tool_body);

        // Transport would have written these; simulate that here.
        fs::create_dir_all(dir_b.join(INBOX_DIR)).unwrap();
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();
        fs::create_dir_all(dir_b.join(INBOX_TOOLS_DIR)).unwrap();
        fs::write(inbox_tool_path(&dir_b, &sha256_hex(tool_body)), tool_body).unwrap();

        let r = federate(&dir_b, NOW + 1);
        assert_eq!(r.peers, 1);
        assert_eq!(r.tools_merged, 1);
        assert_eq!(r.patterns_merged, 1);

        // Provenance recorded on the merged tool.
        let tools = tool::load(&dir_b).unwrap();
        let merged = tools.iter().find(|t| t.name == "battery").unwrap();
        assert_eq!(merged.origin, a_node.node_id());
        assert!(merged.origin_verified_at > 0);

        // Peer observation is tagged, namespaced, and sourced from mesh — never laundered.
        let obs = observation::load(&dir_b).unwrap();
        assert!(obs
            .iter()
            .any(|o| o.actor == format!("mesh:{}", a_node.node_id()) && o.source == "mesh"));

        // Idempotent: draining again merges nothing new.
        let r2 = federate(&dir_b, NOW + 2);
        assert_eq!(r2.tools_merged, 0);
        assert_eq!(r2.patterns_merged, 0);

        let _ = fs::remove_dir_all(&dir_b);
        let _ = fs::remove_dir_all(&dir_a);
    }

    #[test]
    fn replicates_peer_observations_deduped_and_quarantined() {
        // B enrolls; A (same group) shares an observation it originated.
        let dir_b = tmp("obs_recv");
        let b_node = NodeKey::load_or_mint(&dir_b, "beta").unwrap();
        let cred = group::create_group(&dir_b, &b_node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        open_mesh_boundary(&dir_b);
        let dir_a = tmp("obs_peer");
        let a_node = NodeKey::load_or_mint(&dir_a, "alpha").unwrap();
        let cred_a = group::join_group(
            &dir_a,
            &a_node,
            &cred.join_key(),
            "g",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();

        let mut body = peer_brief_with_tool(&a_node, &cred_a, b"#!/bin/sh\n").body;
        body.knowledge.observations = vec![ObsShare {
            origin: a_node.node_id(),
            actor: "phone:ian".into(),
            action: "reports".into(),
            object: "location:home".into(),
            context: "acc=12m".into(),
            ts: NOW,
            confidence_pct: 90,
        }];
        let brief = sign_brief(body, &a_node).unwrap();

        fs::create_dir_all(dir_b.join(INBOX_DIR)).unwrap();
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();

        let r = federate(&dir_b, NOW + 1);
        assert!(r.observations_ingested >= 1);
        // B now holds A's observation — tagged mesh:<A>, quarantined, provenance preserved.
        let obs = observation::load(&dir_b).unwrap();
        let rep = obs
            .iter()
            .find(|o| o.object == "location:home")
            .expect("replicated observation landed");
        assert_eq!(rep.source, format!("mesh:{}", a_node.node_id()));
        assert_eq!(rep.actor, "phone:ian");

        // Idempotent: the same observation arriving again is deduped by content hash.
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();
        let before = observation::load(&dir_b).unwrap().len();
        let _ = federate(&dir_b, NOW + 2);
        let after = observation::load(&dir_b).unwrap().len();
        assert_eq!(before, after, "a re-shared observation is not duplicated");

        let _ = fs::remove_dir_all(&dir_b);
        let _ = fs::remove_dir_all(&dir_a);
    }

    #[test]
    fn closed_boundary_merges_nothing_and_notes_refusal() {
        let dir = tmp("closed");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let cred = group::create_group(&dir, &node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        // boundary is closed (no allow_mesh written).
        let peer = NodeKey::load_or_mint(&tmp("closedpeer"), "p").unwrap();
        let brief = peer_brief_with_tool(&peer, &cred, b"#!/bin/sh\n"); // cert won't matter; closed short-circuits
        fs::create_dir_all(dir.join(INBOX_DIR)).unwrap();
        fs::write(
            dir.join(INBOX_DIR).join("x.json"),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();

        let r = federate(&dir, NOW + 1);
        assert_eq!(r, MergeReport::default(), "closed mesh merges nothing");
        // A refusal is recorded (once) as visible truth.
        let obs = observation::load(&dir).unwrap();
        assert!(obs
            .iter()
            .any(|o| o.object == "mesh_federation" && o.action == "refused"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn forged_brief_in_inbox_is_rejected_at_merge() {
        let dir = tmp("forged");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let _cred = group::create_group(&dir, &node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        open_mesh_boundary(&dir);

        // A brief from a DIFFERENT group — its cert won't verify against ours.
        let odir = tmp("outsider");
        let outsider = NodeKey::load_or_mint(&odir, "o").unwrap();
        let other =
            group::create_group(&odir, &outsider, "other", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let brief = peer_brief_with_tool(&outsider, &other, b"#!/bin/sh\n");
        fs::create_dir_all(dir.join(INBOX_DIR)).unwrap();
        fs::write(
            dir.join(INBOX_DIR).join("x.json"),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();

        let r = federate(&dir, NOW + 1);
        assert_eq!(r.rejected, 1);
        assert_eq!(r.tools_merged, 0);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&odir);
    }

    #[test]
    fn a_marginalized_peer_offers_nothing_and_a_severed_one_is_dropped() {
        use familiar_kernel::corruption;
        use familiar_kernel::guard::Reason;

        let dir_b = tmp("marginal_recv");
        let b_node = NodeKey::load_or_mint(&dir_b, "beta").unwrap();
        let cred = group::create_group(&dir_b, &b_node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        open_mesh_boundary(&dir_b);

        // A covenanted peer A — a legitimate member whose behavior has slipped.
        let dir_a = tmp("marginal_peer");
        let a_node = NodeKey::load_or_mint(&dir_a, "alpha").unwrap();
        let cred_a = group::join_group(
            &dir_a,
            &a_node,
            &cred.join_key(),
            "g",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();
        let actor = format!("mesh:{}", a_node.node_id());

        // Marginalize A (5 breaches within the window) → its tool + pattern must NOT merge.
        for i in 0..corruption::MARGINALIZE_THRESHOLD as i64 {
            corruption::record(
                &dir_b,
                &actor,
                Reason::ViolatesConstitutionalBoundary,
                NOW - i,
            )
            .unwrap();
        }
        let tool_body = b"#!/bin/sh\necho soc\n";
        let brief = peer_brief_with_tool(&a_node, &cred_a, tool_body);
        fs::create_dir_all(dir_b.join(INBOX_DIR)).unwrap();
        // Prefetch the tool body so the merge *could* install it if not for the tier gate.
        fs::create_dir_all(dir_b.join(INBOX_TOOLS_DIR)).unwrap();
        fs::write(inbox_tool_path(&dir_b, &sha256_hex(tool_body)), tool_body).unwrap();
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();

        let r = federate(&dir_b, NOW + 1);
        assert_eq!(
            r.tools_merged, 0,
            "a marginalized peer's tool does not merge"
        );
        assert_eq!(r.patterns_merged, 0, "nor its patterns");
        assert!(
            observation::load(&dir_b)
                .unwrap()
                .iter()
                .any(|o| o.context.contains("MARGINALIZED")),
            "the peer is still noted, badged marginalized"
        );

        // Push A over the sever line → its next brief is dropped and a revoke recommendation is raised.
        for i in 0..(corruption::SEVER_THRESHOLD - corruption::MARGINALIZE_THRESHOLD) as i64 {
            corruption::record(
                &dir_b,
                &actor,
                Reason::ViolatesConstitutionalBoundary,
                NOW - 100 - i,
            )
            .unwrap();
        }
        fs::write(
            dir_b
                .join(INBOX_DIR)
                .join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();
        federate(&dir_b, NOW + 2);
        let obs = observation::load(&dir_b).unwrap();
        assert!(
            obs.iter().any(|o| o.action == "recommend-revoke"),
            "a severed peer is raised to the human for a revoke decision"
        );

        let _ = fs::remove_dir_all(&dir_b);
        let _ = fs::remove_dir_all(&dir_a);
    }
}

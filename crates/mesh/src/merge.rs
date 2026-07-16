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

use crate::brief::{sign_brief, BriefBody, Capability, ConsentedIdentityPayload, IdentityShare,
    Knowledge, MeshBrief, ObsShare, PatternOffer, Presence, ToolManifest, BRIEF_VERSION};
use crate::config::{self, MeshConfig};
use crate::group::{self, GroupCredential};
use crate::node::NodeKey;
use crate::transport::{INBOX_DIR, INBOX_TOOLS_DIR, OUTBOX_FILE};
use crate::{hex_encode, os_random, sha256_hex};
use familiar_kernel::boundary;
use familiar_kernel::guard::{self, Action, ActionKind, Decision};
use familiar_kernel::{identity, observation, pattern_memory, thread, tool};
use std::collections::HashSet;
use std::path::Path;

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
pub fn build_outbox(dir: &Path, cred: &GroupCredential, cfg: &MeshConfig, now: i64) -> crate::Result<()> {
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
        let can_execute = boundary::load(dir).map(|b| b.allow_execute).unwrap_or(false);
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
        Knowledge {
            patterns: pattern_offers(dir),
            obs_summary: format!("{} observations", obs.len()),
            observations,
            theory_requests,
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
            tools,
        },
        knowledge,
        identities,
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
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(brief) = serde_json::from_slice::<MeshBrief>(&bytes) else {
            let _ = std::fs::remove_file(&path);
            continue;
        };
        // Re-verify at the constitutional boundary (transport already checked at ingress).
        if crate::brief::verify_brief(&brief, &gk, &cred.group_id, now, &revoked).is_err() {
            report.rejected += 1;
            record_obs(dir, "mesh", "rejected_brief", &brief.body.node.node_id,
                "a brief failed re-verification at merge (untrusted/expired/revoked)", now);
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

    // --- Tools: auto-merge into the library with provenance (use is still fully gated). ---
    let known = known_tool_shas(dir);
    let mut seq = tool::load(dir).map(|t| t.len()).unwrap_or(0);
    for m in &brief.body.capability.tools {
        if known.contains(&m.script_sha256) {
            continue;
        }
        let body_path = inbox_tool_path(dir, &m.script_sha256);
        let Ok(body) = std::fs::read(&body_path) else { continue }; // not fetched yet; next tick
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
        brief.body.node.label,
        brief.body.presence.observer_count,
        brief.body.knowledge.obs_summary,
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

    // --- Theory delegation: a peer that can't test its own theories asks us to. If WE can execute,
    // adopt each federated theory as a local thread so pursue_threads tests it (candidate → test →
    // select), and the outcome replicates home via the shared observation record. Deduped by
    // (origin, direction) against threads we already hold. We lend our execution to a peer's ideas. ---
    if !brief.body.knowledge.theory_requests.is_empty()
        && boundary::load(dir).map(|b| b.allow_execute).unwrap_or(false)
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
            let key = format!("{}\u{1}{}", origin_actor, req.direction.trim().to_lowercase());
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
                    &format!("testing a theory delegated by {} — '{}'", short(&req.origin), req.direction),
                    now,
                );
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
                keywords: t
                    .keywords
                    .split_whitespace()
                    .map(String::from)
                    .collect(),
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
        return if node.is_empty() { None } else { Some(node.to_string()) };
    }
    Some(self_node.to_string())
}

/// A stable, cross-node dedup key for an observation: a content hash of its origin + triple + time.
/// Ids are node-local, so two nodes that hold the same observation agree only on these fields.
fn obs_key(origin: &str, actor: &str, action: &str, object: &str, ts: i64) -> String {
    let material = format!("{origin}\u{1}{actor}\u{1}{action}\u{1}{object}\u{1}{ts}");
    sha256_hex(material.as_bytes())[..16].to_string()
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
        .map(|d| d.flatten().any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json")))
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
        let p = std::env::temp_dir().join(format!("familiar_mesh_merge_{tag}_{}", std::process::id()));
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
            presence: Presence { observer_count: 2, last_active: NOW },
            capability: Capability {
                os: "linux".into(),
                arch: "arm".into(),
                env_summary: "cpn".into(),
                tools: vec![ToolManifest {
                    tool_id: "tool-0007".into(),
                    name: "battery".into(),
                    purpose: "read pack soc".into(),
                    keywords: vec!["battery".into(), "soc".into()],
                    script_sha256: sha256_hex(tool_body),
                    uses: 5,
                    last_exit_ok: true,
                }],
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
            },
            identities: None,
        };
        sign_brief(body, author).unwrap()
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
        fs::write(dir_b.join(boundary::BOUNDARY_FILE), serde_json::to_string(&bnd).unwrap()).unwrap();

        let dir_a = tmp("delegate_peer");
        let a_node = NodeKey::load_or_mint(&dir_a, "alpha").unwrap();
        let cred_a =
            group::join_group(&dir_a, &a_node, &cred.join_key(), "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

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
            dir_b.join(INBOX_DIR).join(format!("{}.json", a_node.node_id())),
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
            dir_b.join(INBOX_DIR).join(format!("{}.json", a_node.node_id())),
            serde_json::to_vec(&brief).unwrap(),
        )
        .unwrap();
        federate(&dir_b, NOW + 2);
        let n = familiar_kernel::thread::load(&dir_b)
            .unwrap()
            .iter()
            .filter(|t| t.direction == "offer a standing morning digest")
            .count();
        assert_eq!(n, 1, "the delegated theory is adopted once, not every round");

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
        fs::write(dir.join(boundary::BOUNDARY_FILE), serde_json::to_string(&bnd).unwrap()).unwrap();
        familiar_kernel::thread::append(&dir, &familiar_kernel::thread::Thread {
            id: "thread-0001".into(), question: "q".into(), theory: "th".into(),
            direction: "try a gentle nudge".into(), created_at: NOW, status: "open".into(),
            origin: "llm".into(), actor: "familiar".into(),
        }).unwrap();

        let cfg = MeshConfig::default();
        build_outbox(&dir, &cred, &cfg, NOW + 1).unwrap();
        let brief: MeshBrief =
            serde_json::from_str(&fs::read_to_string(dir.join(OUTBOX_FILE)).unwrap()).unwrap();
        assert_eq!(brief.body.knowledge.theory_requests.len(), 1, "a theorist offers its theory");
        assert_eq!(brief.body.knowledge.theory_requests[0].direction, "try a gentle nudge");

        // Flip to executor: it keeps its theories to itself (tests them locally instead).
        bnd.allow_execute = true;
        fs::write(dir.join(boundary::BOUNDARY_FILE), serde_json::to_string(&bnd).unwrap()).unwrap();
        build_outbox(&dir, &cred, &cfg, NOW + 2).unwrap();
        let brief2: MeshBrief =
            serde_json::from_str(&fs::read_to_string(dir.join(OUTBOX_FILE)).unwrap()).unwrap();
        assert!(brief2.body.knowledge.theory_requests.is_empty(), "an executor delegates nothing");
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
        let cred_a = group::join_group(&dir_a, &a_node, &cred.join_key(), "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        let tool_body = b"#!/bin/sh\necho soc\n";
        let brief = peer_brief_with_tool(&a_node, &cred_a, tool_body);

        // Transport would have written these; simulate that here.
        fs::create_dir_all(dir_b.join(INBOX_DIR)).unwrap();
        fs::write(
            dir_b.join(INBOX_DIR).join(format!("{}.json", a_node.node_id())),
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
        assert!(obs.iter().any(|o| o.actor == format!("mesh:{}", a_node.node_id())
            && o.source == "mesh"));

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
        let cred_a =
            group::join_group(&dir_a, &a_node, &cred.join_key(), "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

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
            dir_b.join(INBOX_DIR).join(format!("{}.json", a_node.node_id())),
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
            dir_b.join(INBOX_DIR).join(format!("{}.json", a_node.node_id())),
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
        fs::write(dir.join(INBOX_DIR).join("x.json"), serde_json::to_vec(&brief).unwrap()).unwrap();

        let r = federate(&dir, NOW + 1);
        assert_eq!(r, MergeReport::default(), "closed mesh merges nothing");
        // A refusal is recorded (once) as visible truth.
        let obs = observation::load(&dir).unwrap();
        assert!(obs.iter().any(|o| o.object == "mesh_federation" && o.action == "refused"));
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
        let other = group::create_group(&odir, &outsider, "other", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let brief = peer_brief_with_tool(&outsider, &other, b"#!/bin/sh\n");
        fs::create_dir_all(dir.join(INBOX_DIR)).unwrap();
        fs::write(dir.join(INBOX_DIR).join("x.json"), serde_json::to_vec(&brief).unwrap()).unwrap();

        let r = federate(&dir, NOW + 1);
        assert_eq!(r.rejected, 1);
        assert_eq!(r.tools_merged, 0);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&odir);
    }
}

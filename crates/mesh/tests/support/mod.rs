//! Reusable hostile-member fixtures for mesh integration tests.
//!
//! The real transport verifies a brief and writes it to `mesh/inbox`; the synchronous
//! merge verifies it again before applying anything. These fixtures deliberately enter
//! at that inbox boundary so tests can schedule partitions, replays, skewed clocks, and
//! malicious-but-valid member payloads without wall-clock sleeps or TCP port races. The
//! production verifier still runs in [`familiar_mesh::federate`].

use familiar_mesh::brief::{
    sign_brief, BriefBody, Capability, Knowledge, MeshBrief, Presence, BRIEF_VERSION,
};
use familiar_mesh::config::MeshConfig;
use familiar_mesh::group::{self, GroupCredential, DEFAULT_CERT_TTL_SECS};
use familiar_mesh::node::NodeKey;
use familiar_mesh::transport::INBOX_DIR;
use familiar_mesh::MergeReport;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const NOW: i64 = 1_780_000_000;

static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

/// One real key, credential, boundary, and data directory in a test mesh.
pub struct HarnessNode {
    pub dir: PathBuf,
    pub key: NodeKey,
    pub credential: GroupCredential,
}

impl HarnessNode {
    pub fn id(&self) -> String {
        self.key.node_id()
    }
}

/// N same-group nodes whose signed briefs can be delivered under a deterministic schedule.
pub struct MeshHarness {
    root: PathBuf,
    nodes: Vec<HarnessNode>,
}

impl MeshHarness {
    pub fn new(labels: &[&str]) -> Self {
        assert!(!labels.is_empty(), "a mesh harness needs at least one node");
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "familiar-hostile-mesh-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let first_dir = root.join("node-0");
        fs::create_dir_all(&first_dir).unwrap();
        let first_key = NodeKey::load_or_mint(&first_dir, labels[0]).unwrap();
        let first_credential = group::create_group(
            &first_dir,
            &first_key,
            "hostile-fixture",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();
        open_mesh(&first_dir);

        let join_key = first_credential.join_key();
        let mut nodes = vec![HarnessNode {
            dir: first_dir,
            key: first_key,
            credential: first_credential,
        }];
        for (index, label) in labels.iter().enumerate().skip(1) {
            let dir = root.join(format!("node-{index}"));
            fs::create_dir_all(&dir).unwrap();
            let key = NodeKey::load_or_mint(&dir, label).unwrap();
            let credential = group::join_group(
                &dir,
                &key,
                &join_key,
                "hostile-fixture",
                NOW,
                DEFAULT_CERT_TTL_SECS,
            )
            .unwrap();
            open_mesh(&dir);
            nodes.push(HarnessNode {
                dir,
                key,
                credential,
            });
        }
        Self { root, nodes }
    }

    pub fn node(&self, index: usize) -> &HarnessNode {
        &self.nodes[index]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Sign an otherwise-minimal current-version brief after a test mutates its payload.
    pub fn signed(&self, from: usize, nonce: &str, edit: impl FnOnce(&mut BriefBody)) -> MeshBrief {
        let author = self.node(from);
        let mut body = minimal_body(author, nonce);
        edit(&mut body);
        sign_brief(body, &author.key).unwrap()
    }

    /// Run the real synchronous merge at one node. Inbox signatures are re-verified here.
    pub fn tick(&self, target: usize, now: i64) -> MergeReport {
        familiar_mesh::federate(&self.node(target).dir, now)
    }

    pub fn inbox_path(&self, target: usize, sender_id: &str) -> PathBuf {
        self.node(target)
            .dir
            .join(INBOX_DIR)
            .join(format!("{sender_id}.json"))
    }

    fn deliver(&self, target: usize, brief: &MeshBrief) {
        let inbox = self.node(target).dir.join(INBOX_DIR);
        fs::create_dir_all(&inbox).unwrap();
        fs::write(
            inbox.join(format!("{}.json", brief.body.node.node_id)),
            serde_json::to_vec_pretty(brief).unwrap(),
        )
        .unwrap();
    }
}

impl Drop for MeshHarness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct ScheduledBrief {
    at: i64,
    sequence: u64,
    target: usize,
    brief: MeshBrief,
}

/// A deterministic network: deliveries stay partitioned until their scheduled logical time.
#[derive(Default)]
pub struct NetworkSchedule {
    next_sequence: u64,
    pending: Vec<ScheduledBrief>,
}

impl NetworkSchedule {
    pub fn deliver_at(&mut self, at: i64, target: usize, brief: MeshBrief) {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.pending.push(ScheduledBrief {
            at,
            sequence,
            target,
            brief,
        });
    }

    /// Deliver every due brief in `(logical time, insertion order)`. Future briefs remain held.
    pub fn run_through(&mut self, harness: &MeshHarness, through: i64) -> usize {
        self.pending.sort_by_key(|event| (event.at, event.sequence));
        let due = self.pending.partition_point(|event| event.at <= through);
        let future = self.pending.split_off(due);
        let ready = std::mem::replace(&mut self.pending, future);
        let delivered = ready.len();
        for event in ready {
            assert!(
                event.target < harness.len(),
                "scheduled target {} is outside a {}-node mesh",
                event.target,
                harness.len()
            );
            harness.deliver(event.target, &event.brief);
        }
        delivered
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

fn open_mesh(dir: &Path) {
    let mut boundary = familiar_kernel::boundary::Boundary::closed();
    boundary.phase = "test".into();
    boundary.allow_mesh = true;
    fs::write(
        dir.join(familiar_kernel::boundary::BOUNDARY_FILE),
        serde_json::to_vec_pretty(&boundary).unwrap(),
    )
    .unwrap();
    let config = MeshConfig {
        lan_discovery: false,
        ..MeshConfig::default()
    };
    fs::create_dir_all(dir.join("mesh")).unwrap();
    fs::write(
        dir.join(familiar_mesh::config::CONFIG_FILE),
        serde_json::to_vec_pretty(&config).unwrap(),
    )
    .unwrap();
}

fn minimal_body(author: &HarnessNode, nonce: &str) -> BriefBody {
    BriefBody {
        version: BRIEF_VERSION,
        node: author.key.identity(),
        membership: author.credential.membership.clone(),
        ts: NOW,
        nonce: nonce.into(),
        presence: Presence {
            observer_count: 0,
            last_active: NOW,
        },
        capability: Capability {
            os: "test".into(),
            arch: "test".into(),
            env_summary: author.key.identity().label,
            familiar_version: "test".into(),
            os_version: String::new(),
            tools: Vec::new(),
            capabilities: Vec::new(),
            build_version: 0,
            lat: 0.0,
            lon: 0.0,
            interactive: false,
            human: String::new(),
        },
        knowledge: Knowledge::default(),
        identities: None,
        authority_requests: Vec::new(),
        authority_grants: Vec::new(),
    }
}

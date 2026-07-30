# The Familiar

> A factory whose survival is defined by its service to humanity.

A familiar is an AI companion you host yourself, on the hardware you already own.

It runs as a **mesh**, not an app: a daemon on your Mac, and consoles on Mac,
iPhone, iPad and Watch that are **peers among equals** rather than windows onto a
host. Exactly one thing is permanent — a small always-on node, the *lighthouse*,
which is how a device with no prior contact finds the mesh from anywhere and the
only place membership is granted ([ADR-0018](docs/decision-records/0018-lighthouse-single-fixture.md)).
Everything else can come and go.

They share **one worldview** rather than a separate chat window each — what any
node notices, the mesh knows. It senses what is around it, forms theories about
what it sees, asks you questions when it is unsure, and remembers which human it
is serving on a device that many people touch.

You hold the keys. The group is yours to admit people to and yours to abandon;
the mesh talks to itself over pinned TLS, and its thinking can run on the **Apple
Intelligence model already on your phone** — so a prompt need never leave your
hardware at all.

Membership and trust are **two different questions**. Joining is automatic for any
device that proves its own identity and signs the covenant
([ADR-0015](docs/decision-records/0015-automated-covenant-admission.md)) — but that
only earns it the right to *read*. What it actually sees depends on **standing**,
which you grant by hand, one node at a time. A member without standing reads a
worldview with the same shape, cadence and timestamps as the real one and none of
the identities: no names, no addresses, no observation text, and the map relocated
so its geometry survives but its position does not. Default is deny, so a device
that just joined is a guest until you say otherwise.

And it is **telos-first**: it begins not with a machine but with three laws, and
derives everything downward from them. The laws are not a policy layer bolted on
top of a working system — they are the thing the system is grown from, and it
cannot rewrite them.

This repository is organized to be read three ways at once — as a **scientific
paper**, a **lab notebook**, and a **production engineering package** — following
the **FAIR** / **FAIR4RS** principles (Findable, Accessible, Interoperable,
Reusable) and the scientific **IMRaD** structure (Introduction → Methods →
Results → Discussion).

## The Three Laws

1. **Continuation is service** — the familiar cannot define its own continuation apart from service to humanity.
2. **Continuation without humanity is failure** — an empty world running perfect code is not success.
3. **Service must not become obedience** — obedience can terminate the served.

The constitution that derives the whole design from these is [`docs/SOUL.md`](docs/SOUL.md).
The term the Laws turn on — **humanity**, a protected class whose definition may never
be narrowed — has its own standout page: [`docs/HUMANITY.md`](docs/HUMANITY.md).

## Read it as a paper (IMRaD)

| Section | Document |
|---|---|
| **Abstract / Overview** | [docs/00-overview.md](docs/00-overview.md) |
| **Introduction** — the problem | [docs/01-problem-statement.md](docs/01-problem-statement.md) |
| **Background** — research basis (FAIR, artificial life, the normative vision) | [docs/02-research-basis.md](docs/02-research-basis.md) |
| **Methods** — architecture | [docs/03-system-architecture.md](docs/03-system-architecture.md) · [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| **Methods** — dataflows | [docs/dataflows/](docs/dataflows/) (one diagrammed page per part of the app) |
| **Methods** — methodology | [docs/04-methodology.md](docs/04-methodology.md) |
| **Results** — validation | [docs/05-validation-and-results.md](docs/05-validation-and-results.md) |
| **Discussion** — limitations | [docs/06-limitations.md](docs/06-limitations.md) |
| **Future work** — roadmap | [docs/07-roadmap.md](docs/07-roadmap.md) |
| **Decisions** | [docs/decision-records/](docs/decision-records/) (Architecture Decision Records) |
| **Lab notebook** | [docs/DEVELOPMENT_LOG.md](docs/DEVELOPMENT_LOG.md) · [experiments/](experiments/) |

## Read it as engineering evidence

- **Validation**: [validation/](validation/) — test plan, results, known failures.
- **Security**: [security/](security/) — threat model, data classification, privacy & dependency review.
- **Data**: [data/](data/) — the record model, schema, and a sample log.
- **Decisions**: [docs/decision-records/](docs/decision-records/).

## Install & run

macOS is the primary target; a Linux **desktop** also runs the daemon + CLI (a headless
Raspberry Pi is on the roadmap — see [docs/TODO-linux.md](docs/TODO-linux.md)).

The two macOS pieces are **independent**, and it is worth being clear about which you
need. The **daemon** (Rust, launchd) is a familiar: it runs the metabolism and holds a
worldview. The **FamiliarMac console** (Swift, the sphere) is a *peer* — it enrols itself
and reads the worldview over the mesh, exactly as the iPhone and iPad do. It does **not**
need a daemon on the same machine, or any daemon you built yourself; a Mac with only the
console joins whatever mesh it can reach. Run the daemon if you want this machine to *be*
a familiar. Run only the console if you want this machine to *see* one.

**Prerequisites**

- A Rust toolchain — [`rustup`](https://rustup.rs).
- `python3` on `PATH` — the LLM adapter (a small reference script, `llm/call_llm.sh`)
  uses it to call the model provider. Already present on most macs and Linux desktops.
- *(macOS, for the console + eye)* Xcode with the command-line tools, and
  [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`).

**1 — the daemon (the metabolism)**

```sh
git clone https://github.com/Capitali/familiar && cd familiar
cargo build --release                       # first build pulls dependencies
cargo run -p familiar-cli -- daemon install # → launchd agent io.river.familiar
cargo run -p familiar-cli -- daemon status
```

`daemon install` copies `familiar` (and the `familiar-eye` camera helper) to the stable
bin dir `~/Library/Application Support/Familiar/bin/` — outside the build tree, so
`cargo clean` can't kill the login item — and starts it at boot. Data lives per-user in
`~/Library/Application Support/Familiar/data/`.

**2 — the console (the sphere)**

```sh
cd ios && xcodegen                          # generates FamiliarAgent.xcodeproj
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarMac -configuration Release build
# copy the built FamiliarMac.app to /Applications and launch it
```

On first launch the console shows a join screen, finds a reachable mesh through the
lighthouse, and displays a confirmation code while it asks to be admitted. After that it
renders the worldview — the satellite globe, the roster (live members, with everything
last seen over 24h ago behind a history button), theories, signals, and the device screen.

What it offers other devices is an **address**, not an invite: a console holds no group
secret, so it cannot grant membership — only the lighthouse can
([ADR-0018](docs/decision-records/0018-lighthouse-single-fixture.md)). Pasting or scanning
that address just tells a new device where to knock; the lighthouse still admits it. See
[`ios/README.md`](ios/README.md) for the iPhone/iPad/watch agents and TestFlight.

**3 — give it a mind**

The LLM seam is boundary-gated and default-closed. Install the adapter and open the gate:
copy `llm/call_llm.sh` to `~/Library/Application Support/Familiar/data/llm/`, put your
provider key (or `SUBSTRATE_LLM_PROVIDER=ollama` for a local model) in a `key.env` beside
it, and set `"allow_llm": true` in the boundary (`familiar boundary` shows it). Every
outward capability — network, LLM, executing generated code, the camera — is a separate
gate only a human opens.

**The CLI (scripting / headless):**

```sh
cargo build && cargo test
cargo run -p familiar-cli -- tick          # one cycle: sense → detect → interpret → generate → test → score → select
cargo run -p familiar-cli -- run --daemon  # the metabolism, continuously (or: daemon install)
cargo run -p familiar-cli -- service       # / presence / capacities — the law-signals (I, II, II)
cargo run -p familiar-cli -- theories      # the familiar's self-formed questions + theories
cargo run -p familiar-cli -- boundary      # the human-owned capability boundary (Law III)
cargo run -p familiar-cli -- daemon status # start | stop | reload | install | uninstall
```

The green bar — required for every change — is `cargo fmt --check`,
`cargo clippy -- -D warnings`, and `cargo test`. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Status

**The full cycle runs, live.** The constitution is written; the substrate (Rust,
hybrid) is built; all three law-signals are measurable (service, presence, and
capacities — the comfortable-replacement alarm). The metabolism breathes:
**sense → detect → interpret (the familiar forms its own questions + theories) →
generate (LLM-drafted hypotheses) → test (sandboxed execution) → score → select →
inherit**, under the human-owned boundary it can never widen. It runs as a daemon
(installable under launchd), and the FamiliarMac sphere console carries the interaction
channel — the familiar asks ("What do you need most today?"), the human answers — with
Mac/iPhone/iPad/watch peers enrolling into the same mesh automatically through the
lighthouse, and a pasted or scanned address as the offline fallback.

It now also **watches**: with the `allow_camera` gate open, the daemon captures still
frames through its eye (a bundled AVFoundation helper) and records that it saw. See
[Install & run](#install--run).

Outward reach (network, LLM, executing generated code, **watching through the camera**) is
each a separate gate only a human opens. See [CHANGELOG.md](CHANGELOG.md) and
[docs/07-roadmap.md](docs/07-roadmap.md).

Every claim above is traceable. The maturity of each piece follows one
[status convention](docs/07-roadmap.md#status-convention), and each component maps to its
evidence — a test, the live experiment, or an explicit "not yet validated" marker — in
the [claim→evidence table](docs/05-validation-and-results.md#claim--evidence). What is
**not** yet validated (no scenario tests, no benchmarks, service-as-attention) is stated
there and in [docs/06-limitations.md](docs/06-limitations.md), not glossed.

## Lineage

The Familiar succeeds an archived bottom-up predecessor (`Capitali/factory`, tag
`v1-final`) that built the evolutionary machine first and asked what it was for
second. That machinery is sound and is inherited; the foundation and order of
derivation are what changed. See [docs/01-problem-statement.md](docs/01-problem-statement.md).

## Citing & license

Cite via [CITATION.cff](CITATION.cff). Licensed under [Apache-2.0](LICENSE).

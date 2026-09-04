import Foundation
import FamiliarSC
import FamiliarSCUI

// familiar-bridge — a macOS stand-in for the captain's bridge (T-237 B2's visible proof).
// Reads a ship store and speaks the report through the voice ladder; shows the message
// window, the notices and the dial. Read-only: it never writes a store, never posts an act.

let usage = """
usage:
  familiar-bridge report  <ship-dir> [--since-ticks N] [--question "…"] [--lane templated|auto] [--consent-pcc] [--wire] [--json]
  familiar-bridge window  <ship-dir>
  familiar-bridge notices <ship-dir> [--since-ticks N]
  familiar-bridge dial    <ship-dir>
  familiar-bridge voices  [--consent-pcc]
  familiar-bridge fleet   <feed-url> --token-file <file> [--json]     # `familiar fleet serve` over the wire
"""

func fail(_ msg: String) -> Never {
    FileHandle.standardError.write((msg + "\n").data(using: .utf8)!)
    exit(2)
}

var args = Array(CommandLine.arguments.dropFirst())
guard let cmd = args.first else { fail(usage) }
args.removeFirst()

func flag(_ name: String) -> Bool {
    if let i = args.firstIndex(of: name) { args.remove(at: i); return true }
    return false
}
func option(_ name: String) -> String? {
    guard let i = args.firstIndex(of: name), i + 1 < args.count else { return nil }
    let v = args[i + 1]; args.removeSubrange(i...i + 1); return v
}

let consent = VoiceConsent(privateCloudCompute: flag("--consent-pcc"))
let wantWire = flag("--wire")
let asJSON = flag("--json")
let laneChoice = option("--lane") ?? "auto"
let question = option("--question") ?? "What did you do today?"
let sinceTicks = Int64(option("--since-ticks") ?? "288") ?? 288

if cmd == "voices" {
    for (lane, why) in BridgeVoice.availability(consent: consent).sorted(by: { $0.key.rawValue < $1.key.rawValue }) {
        print("\(lane.rawValue): \(why)")
    }
    exit(0)
}

if cmd == "fleet" {
    guard let urlArg = args.first, let base = URL(string: urlArg) else { fail(usage) }
    args.removeFirst()
    guard let tokenFile = option("--token-file"), let token = try? String(contentsOfFile: tokenFile, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines) else { fail("--token-file is required") }
    let wire = WireFeed(base: base, bearer: token)
    let model = BridgeModel(feed: wire, acts: wire, voiceConsent: consent)
    // BridgeModel's loads are main-actor: keep the main thread free (dispatchMain) and
    // exit from the task, rather than blocking it on a semaphore it would deadlock on.
    Task {
        await model.refreshShips()
        if let e = model.error { print("error: \(e)") }
        for s in model.ships {
            print("\(s.world) \"\(s.label)\" · hull \"\(s.hull)\" · computer \"\(s.computer)\" · \(s.moodWord) · ℳ\(s.credits ?? 0) · fuel \(s.fuel ?? 0) · \(s.pilotAlive ? "pilot" : "NO PILOT") · \(s.docked ?? s.enRouteTo.map { "→ \($0)" } ?? "under way") · \(s.openProposals) waiting")
        }
        if let first = model.ships.first {
            await model.open(world: first.world)
            if let e = model.error { print("error: \(e)") }
            print("journal lines: \(model.journal.count) · window items: \(model.window.count) · open: \(model.openProposals)")
            if let d = model.dial { print("dial: \(d.loaded) bought \(d.bought)") }
            if let r = model.reports.first {
                print("[t\(r.fromTick)–t\(r.toTick)] \(r.report.mood.rawValue): \(r.report.headline)")
                for f in r.report.facts.prefix(6) { print("  · \(f)") }
                print("→ \(r.report.nextAct)")
            }
        }
        exit(0)
    }
    dispatchMain()
}

guard let dirArg = args.first else { fail(usage) }
let store = ShipStore(directory: URL(fileURLWithPath: dirArg))
let persona: Persona
do { persona = try store.persona() ?? Persona(name: "(unnamed — `fleet rename` her)", style: nil) } catch { fail("\(error)") }
let journal: Journal
do { journal = try store.journal() } catch { fail("\(error)") }
let nowTick = journal.lastTick ?? 0
let window = journal.since(tick: max(0, nowTick - sinceTicks))
let items = MessageWindow.build(journal: journal.entries, proposals: store.proposals(), approvals: store.approvals(), nowTick: nowTick)
let open = items.filter(\.needsTheCaptain).count

func readEnv(_ key: String) -> String? {
    guard let t = try? String(contentsOf: store.url("ucf.env"), encoding: .utf8) else { return nil }
    for l in t.split(separator: "\n") {
        let kv = l.split(separator: "=", maxSplits: 1).map { $0.trimmingCharacters(in: .whitespaces) }
        if kv.count == 2, kv[0] == key { return kv[1] }
    }
    return nil
}

switch cmd {
case "report":
    var hull: HullGlance? = nil
    if wantWire {
        let server = readEnv("UCF_SERVER") ?? (try? store.captain())?.server ?? ""
        if let key = readEnv("UCF_KEY"), let client = ExchangeClient(server: server, key: key) {
            let sem = DispatchSemaphore(value: 0)
            Task.detached {
                do { hull = HullGlance(me: try await client.me()) } catch { FileHandle.standardError.write("wire: \(error)\n".data(using: .utf8)!) }
                sem.signal()
            }
            sem.wait()
        } else {
            FileHandle.standardError.write("wire: no UCF_KEY/UCF_SERVER in ucf.env\n".data(using: .utf8)!)
        }
    }
    let ctx = BridgeContext(entries: window, hull: hull, openProposals: open, question: question)
    let voice = BridgeVoice(persona: persona)
    let spoken: SpokenReport
    if laneChoice == "templated" {
        spoken = SpokenReport(report: voice.floor(ctx), lane: .templated, note: "requested")
    } else {
        let sem = DispatchSemaphore(value: 0)
        var out: SpokenReport?
        Task.detached { out = await voice.speak(ctx, consent: consent); sem.signal() }
        sem.wait()
        spoken = out!
    }
    if asJSON {
        let enc = JSONEncoder(); enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        struct Out: Encodable { var lane: String; var note: String?; var report: BridgeReport; var pendingDialChanges: [String] }
        let o = Out(lane: spoken.lane.rawValue, note: spoken.note, report: spoken.report,
                    pendingDialChanges: voice.pendingDialChanges.map { "\($0.surface)=\($0.level.rawValue)" })
        print(String(data: try! enc.encode(o), encoding: .utf8)!)
    } else {
        print("[\(persona.name) · \(spoken.lane.rawValue)\(spoken.note.map { " · \($0)" } ?? "")] mood: \(spoken.report.mood.rawValue)")
        print(spoken.report.headline)
        for f in spoken.report.facts { print("  · \(f)") }
        print("→ \(spoken.report.nextAct)")
        for c in voice.pendingDialChanges { print("? proposes dial \(c.surface) = \(c.level.rawValue) (awaiting the captain)") }
    }
case "window":
    if items.isEmpty { print("(no advice or proposals in the journal)") }
    for i in items {
        switch i.kind {
        case .advice(let would, let why):
            print("t\(i.tick) advice [\(i.surfaceKey)] \(would) — \(why)")
        case .proposal(let id, let would, let why, let expires, let state):
            print("t\(i.tick) proposal [\(i.surfaceKey)] \(would) — \(why) (until t\(expires)) \(state) \(id)")
        }
    }
case "notices":
    for n in NoticePolicy.notices(for: window) { print("\(n.kind.rawValue): \(n.title) — \(n.body)") }
case "dial":
    switch store.dial() {
    case .absent: print("autonomy.json absent: every bought surface is auto (the tanker advises)")
    case .malformed(let why): print("autonomy.json MALFORMED (\(why)) — whisker reads it as absent: auto everywhere")
    case .dial(let d):
        for k in d.settings.keys.sorted() { print("\(k) = \(d.settings[k]!.rawValue)") }
    }
    let d = store.dial().dial
    let bought = (try? store.automations()) ?? []
    for s in ControlSurface.allCases where s.automation.map(bought.contains) ?? false {
        print("  \(s.key): \(d.level(for: s).rawValue)")
    }
default:
    fail(usage)
}

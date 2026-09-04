import Foundation
import Observation
import FamiliarSC

/// The bridge's state: the fleet, and one ship's bridge at a time. `@Observable`, so the
/// screens track only what they read. Every load is a plain async read of the feed; every
/// act goes through `CaptainActs` and then re-reads, so the screen shows the store's truth,
/// never a local guess.
@Observable
public final class BridgeModel {
    public let feed: any ShipsFeed
    public let acts: any CaptainActs
    public var voiceConsent: VoiceConsent

    public var ships: [ShipSummary] = []
    public var loading = false
    public var error: String?

    /// The open ship.
    public var world: String?
    public var persona: Persona?
    public var journal: [JournalEntry] = []
    public var window: [MessageItem] = []
    public var dial: DialSheet?
    public var book: ShipBook?
    public var reports: [FoldReport] = []
    public var spoken: SpokenReport?
    public var pendingDialChanges: [DialChange] = []

    /// One fold-window of the journal, told.
    public struct FoldReport: Identifiable, Equatable {
        public var id: Int64 { fromTick }
        public var fromTick: Int64
        public var toTick: Int64
        public var report: BridgeReport
    }

    public init(feed: any ShipsFeed, acts: any CaptainActs, voiceConsent: VoiceConsent = VoiceConsent()) {
        self.feed = feed; self.acts = acts; self.voiceConsent = voiceConsent
    }

    public var summary: ShipSummary? { ships.first { $0.world == world } }
    public var computerName: String { persona?.name ?? summary?.computer ?? "the ship's computer" }

    /// A cancelled read is not an error: a view's `.task` is cancelled whenever SwiftUI
    /// tears the view down or a pull-to-refresh supersedes it, and the request it was
    /// awaiting comes back as URLError.cancelled (NSURLErrorDomain -999). The last good
    /// state stays on screen; nothing is reported (Ian's iPad, 2026-09-04).
    static func isCancellation(_ error: Error) -> Bool {
        if error is CancellationError { return true }
        if let u = error as? URLError, u.code == .cancelled { return true }
        let ns = error as NSError
        return ns.domain == NSURLErrorDomain && ns.code == NSURLErrorCancelled
    }

    /// Error text a captain can read — the platform's sentence, not an NSError dump.
    static func describe(_ error: Error) -> String {
        if let f = error as? FeedError { return f.description }
        if let u = error as? URLError { return u.localizedDescription }
        return (error as NSError).localizedDescription
    }

    @MainActor
    private func report(_ error: Error) {
        guard !BridgeModel.isCancellation(error) else { return }
        self.error = BridgeModel.describe(error)
    }

    /// One fleet read at a time: a second caller awaits the read in flight instead of
    /// starting another (the root's `.task` and the Ships tab's both refresh on appear).
    private var refreshInFlight: Task<Void, Never>?

    @MainActor
    public func refreshShips() async {
        if let t = refreshInFlight { await t.value; return }
        let t = Task { @MainActor in
            loading = true; defer { loading = false }
            do { ships = try await feed.ships(); error = nil } catch { report(error) }
        }
        refreshInFlight = t
        await t.value
        refreshInFlight = nil
    }

    @MainActor
    public func open(world: String, foldWindowTicks: Int64 = 96, windows: Int = 6) async {
        self.world = world
        loading = true; defer { loading = false }
        do {
            persona = try await feed.persona(world: world)
            journal = try await feed.journal(world: world, sinceTick: nil)
            window = try await feed.window(world: world)
            dial = try await feed.dial(world: world)
            book = try await feed.book(world: world)
            reports = BridgeModel.fold(journal: journal, persona: persona, windowTicks: foldWindowTicks, count: windows, openProposals: openProposals)
            error = nil
        } catch { report(error) }
    }

    public var openProposals: Int { window.filter(\.needsTheCaptain).count }

    /// The journal cut into fold windows of `windowTicks`, newest first, each told by the
    /// templated floor — deterministic, instant, and the grounding for the spoken one.
    public static func fold(journal: [JournalEntry], persona: Persona?, windowTicks: Int64, count: Int, openProposals: Int) -> [FoldReport] {
        guard let last = journal.last(where: { $0.tick != nil })?.tick else { return [] }
        let voice = TemplatedVoice(persona: persona ?? Persona(name: "?", style: nil))
        var out: [FoldReport] = []
        var to = last
        for i in 0..<count {
            let from = to - windowTicks + 1
            let slice = journal.filter { e in
                if let t = e.tick { return t >= from && t <= to }
                return false
            }
            if !slice.isEmpty || i == 0 {
                out.append(FoldReport(fromTick: max(from, 0), toTick: to, report: voice.report(entries: slice, openProposals: i == 0 ? openProposals : 0)))
            }
            to = from - 1
            if to < 0 { break }
        }
        return out
    }

    /// Speak the latest window through the ladder (on-device / PCC / floor).
    @MainActor
    public func speakLatest(question: String = "What did you do today?") async {
        guard let latest = reports.first else { return }
        let slice = journal.filter { e in
            guard let t = e.tick else { return false }
            return t >= latest.fromTick && t <= latest.toTick
        }
        let voice = BridgeVoice(persona: persona ?? Persona(name: computerName, style: nil))
        let ctx = BridgeContext(entries: slice, hull: summary?.hullGlance, openProposals: openProposals, question: question)
        spoken = await voice.speak(ctx, consent: voiceConsent)
        pendingDialChanges = voice.pendingDialChanges
    }

    @MainActor
    public func approve(id: String, approved: Bool) async {
        guard let world else { return }
        do {
            try await acts.approve(world: world, proposalID: id, approved: approved)
            window = try await feed.window(world: world)
            error = nil
        } catch { report(error) }
    }

    @MainActor
    public func save(dial newDial: AutonomyDial) async {
        guard let world else { return }
        do {
            try await acts.setDial(world: world, dial: newDial)
            dial = try await feed.dial(world: world)
            error = nil
        } catch { report(error) }
    }

    @MainActor
    public func pair(_ request: PairingRequest, key: PairingKey) async -> String? {
        do { try await acts.pair(request, key: key); await refreshShips(); return nil } catch { return "\(error)" }
    }

    /// The captain's edits to a paired ship; each re-reads the fleet so the screen shows the
    /// store's truth. Returns the outcome to show: the host's note, or the error.
    public struct ActOutcome: Equatable { public var ok: Bool; public var text: String }

    @MainActor
    public func rename(computer: String) async -> ActOutcome {
        guard let world else { return ActOutcome(ok: false, text: "no ship open") }
        do { let n = try await acts.rename(world: world, computer: computer); await refreshShips(); persona = try await feed.persona(world: world)
             return ActOutcome(ok: true, text: n ?? "Renamed.") } catch { return ActOutcome(ok: false, text: "\(error)") }
    }

    @MainActor
    public func setAutomations(_ automations: [Automation]) async -> ActOutcome {
        guard let world else { return ActOutcome(ok: false, text: "no ship open") }
        do { let n = try await acts.setAutomations(world: world, automations: automations); await refreshShips(); dial = try await feed.dial(world: world)
             return ActOutcome(ok: true, text: n ?? "Saved.") } catch { return ActOutcome(ok: false, text: "\(error)") }
    }

    @MainActor
    public func setCaptain(_ captain: String) async -> ActOutcome {
        guard let world else { return ActOutcome(ok: false, text: "no ship open") }
        do { let n = try await acts.setCaptain(world: world, captain: captain); await refreshShips(); persona = try await feed.persona(world: world)
             return ActOutcome(ok: true, text: n ?? "Moved.") } catch { return ActOutcome(ok: false, text: "\(error)") }
    }

    @MainActor
    public func unpair(world: String) async -> String? {
        do { try await acts.unpair(world: world); await refreshShips(); return nil } catch { return "\(error)" }
    }
}

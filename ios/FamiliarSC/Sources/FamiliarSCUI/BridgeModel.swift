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

    @MainActor
    public func refreshShips() async {
        loading = true; defer { loading = false }
        do { ships = try await feed.ships(); error = nil } catch { self.error = "\(error)" }
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
        } catch { self.error = "\(error)" }
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
        } catch { self.error = "\(error)" }
    }

    @MainActor
    public func save(dial newDial: AutonomyDial) async {
        guard let world else { return }
        do {
            try await acts.setDial(world: world, dial: newDial)
            dial = try await feed.dial(world: world)
            error = nil
        } catch { self.error = "\(error)" }
    }

    @MainActor
    public func pair(_ request: PairingRequest, key: PairingKey) async -> String? {
        do { try await acts.pair(request, key: key); await refreshShips(); return nil } catch { return "\(error)" }
    }

    @MainActor
    public func unpair(world: String) async -> String? {
        do { try await acts.unpair(world: world); await refreshShips(); return nil } catch { return "\(error)" }
    }
}

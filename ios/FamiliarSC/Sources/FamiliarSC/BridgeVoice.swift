import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

// The bridge voice: the ladder on-device → Private Cloud Compute → the templated floor
// (dialogue §3, §4). The model SPEAKS the report; it never decides it. Every report it
// produces is checked against the floor before it is shown — any number, id, tick or
// station the floor did not say, or a mood softer than the floor's, and the floor's own
// words are shown instead. The model may propose (a dial change through a tool); the
// proposal waits for the captain's tap. No tool writes anything.

public enum VoiceLane: String, Codable, Equatable, Sendable {
    case privateCloudCompute = "private-cloud-compute"
    case onDevice = "on-device"
    case templated
}

public struct VoiceConsent: Equatable {
    /// The captain's own toggle for Private Cloud Compute (default OFF, like the Familiar
    /// app's `consent.pcc`). OS 27 + entitlement + Apple's availability must all also hold.
    public var privateCloudCompute: Bool
    public init(privateCloudCompute: Bool = false) { self.privateCloudCompute = privateCloudCompute }
}

public struct BridgeContext: Sendable {
    public var entries: [JournalEntry]
    public var hull: HullGlance?
    public var openProposals: Int
    public var question: String

    public init(entries: [JournalEntry], hull: HullGlance? = nil, openProposals: Int = 0, question: String = "What did you do today?") {
        self.entries = entries; self.hull = hull; self.openProposals = openProposals; self.question = question
    }
}

public struct SpokenReport: Equatable, Sendable {
    public var report: BridgeReport
    public var lane: VoiceLane
    /// Why a higher lane was not used, when it was not.
    public var note: String?
    public init(report: BridgeReport, lane: VoiceLane, note: String? = nil) { self.report = report; self.lane = lane; self.note = note }
}

/// A dial change the model proposed through a tool. Nothing until the captain confirms.
public struct DialChange: Equatable, Sendable {
    public var surface: String
    public var level: AutonomyLevel
}

/// The grounding check: the floor is the set of facts; the voice may only rephrase them.
public enum Grounding {
    /// Tokens that carry truth: numbers, load ids (L123), ticks (t123), proposal ids (p-…),
    /// and station-ish slugs (two or more hyphenated words).
    public static func tokens(in text: String) -> Set<String> {
        var out = Set<String>()
        let patterns = ["[0-9]+", "\\bL[0-9]+\\b", "\\bt[0-9]+\\b", "\\bp-[0-9a-f]{8,}\\b", "\\b[a-z]+(?:-[a-z]+)+\\b"]
        for p in patterns {
            guard let re = try? NSRegularExpression(pattern: p) else { continue }
            let ns = text as NSString
            for m in re.matches(in: text, range: NSRange(location: 0, length: ns.length)) {
                out.insert(ns.substring(with: m.range))
            }
        }
        return out
    }

    /// `nil` when the spoken report says nothing the floor did not; else what it invented.
    public static func check(spoken: BridgeReport, floor: BridgeReport) -> String? {
        let allowed = tokens(in: (floor.facts + [floor.headline, floor.nextAct]).joined(separator: "\n"))
        let said = tokens(in: (spoken.facts + [spoken.headline, spoken.nextAct]).joined(separator: "\n"))
        let invented = said.subtracting(allowed).sorted()
        if !invented.isEmpty { return "invented: \(invented.joined(separator: ", "))" }
        // Mood is severity, not cadence: the voice may not cheer up a distress.
        let order: [BridgeReport.Mood] = [.pleased, .steady, .watchful, .concerned]
        if let f = order.firstIndex(of: floor.mood), let s = order.firstIndex(of: spoken.mood), s < f {
            return "softened mood \(floor.mood.rawValue) to \(spoken.mood.rawValue)"
        }
        return nil
    }
}

public final class BridgeVoice: @unchecked Sendable {
    public let persona: Persona
    public var maxJournalLines = 60
    private let lock = NSLock()
    private var proposedChanges: [DialChange] = []

    public init(persona: Persona) { self.persona = persona }

    /// Dial changes the model asked for, in order, for the app to put to the captain.
    public var pendingDialChanges: [DialChange] {
        lock.lock(); defer { lock.unlock() }
        return proposedChanges
    }

    func record(_ c: DialChange) {
        lock.lock(); defer { lock.unlock() }
        proposedChanges.append(c)
    }

    /// The floor: always available, byte-stable.
    public func floor(_ ctx: BridgeContext) -> BridgeReport {
        TemplatedVoice(persona: persona).report(entries: ctx.entries, hull: ctx.hull, openProposals: ctx.openProposals)
    }

    /// Which lanes this device could use right now, with the reason where it cannot.
    public static func availability(consent: VoiceConsent) -> [VoiceLane: String] {
        var out: [VoiceLane: String] = [.templated: "always"]
        #if canImport(FoundationModels)
        if #available(macOS 26.0, iOS 26.0, visionOS 26.0, *) {
            switch SystemLanguageModel.default.availability {
            case .available: out[.onDevice] = "available"
            case .unavailable(.deviceNotEligible): out[.onDevice] = "device not eligible"
            case .unavailable(.appleIntelligenceNotEnabled): out[.onDevice] = "Apple Intelligence is off"
            case .unavailable(.modelNotReady): out[.onDevice] = "model still loading"
            case .unavailable: out[.onDevice] = "unavailable"
            @unknown default: out[.onDevice] = "unavailable"
            }
        } else {
            out[.onDevice] = "needs OS 26"
        }
        if #available(macOS 27.0, iOS 27.0, visionOS 27.0, *) {
            if !consent.privateCloudCompute {
                out[.privateCloudCompute] = "consent off"
            } else {
                switch PrivateCloudComputeLanguageModel().availability {
                case .available: out[.privateCloudCompute] = "available"
                case .unavailable(.deviceNotEligible): out[.privateCloudCompute] = "device not eligible or no entitlement"
                case .unavailable(.systemNotReady): out[.privateCloudCompute] = "system not ready"
                case .unavailable: out[.privateCloudCompute] = "unavailable"
                @unknown default: out[.privateCloudCompute] = "unavailable"
                }
            }
        } else {
            out[.privateCloudCompute] = "needs OS 27"
        }
        #else
        out[.onDevice] = "no Foundation Models on this platform"
        out[.privateCloudCompute] = "no Foundation Models on this platform"
        #endif
        return out
    }

    /// Speak the report: PCC when consented and available, else on-device, else the floor.
    /// Whatever spoke, the result passed the grounding check or it is the floor.
    public func speak(_ ctx: BridgeContext, consent: VoiceConsent = VoiceConsent()) async -> SpokenReport {
        let floorReport = floor(ctx)
        #if canImport(FoundationModels)
        if #available(macOS 26.0, iOS 26.0, visionOS 26.0, *) {
            var notes: [String] = []
            if #available(macOS 27.0, iOS 27.0, visionOS 27.0, *), consent.privateCloudCompute {
                let pcc = PrivateCloudComputeLanguageModel()
                if case .available = pcc.availability {
                    let session = LanguageModelSession(model: pcc, tools: tools(ctx), instructions: instructions())
                    switch await generate(session: session, ctx: ctx, floor: floorReport) {
                    case .success(let r): return SpokenReport(report: r, lane: .privateCloudCompute, note: nil)
                    case .failure(let why): notes.append("pcc: \(why)")
                    }
                } else {
                    notes.append("pcc: unavailable")
                }
            }
            if case .available = SystemLanguageModel.default.availability {
                let session = LanguageModelSession(tools: tools(ctx), instructions: instructions())
                switch await generate(session: session, ctx: ctx, floor: floorReport) {
                case .success(let r): return SpokenReport(report: r, lane: .onDevice, note: notes.isEmpty ? nil : notes.joined(separator: "; "))
                case .failure(let why): notes.append("on-device: \(why)")
                }
            } else {
                notes.append("on-device: unavailable")
            }
            return SpokenReport(report: floorReport, lane: .templated, note: notes.joined(separator: "; "))
        }
        #endif
        return SpokenReport(report: floorReport, lane: .templated, note: "Foundation Models not on this platform")
    }

    // MARK: prompt

    func instructions() -> String {
        let s = persona.voice
        let role = persona.role.isEmpty
            ? "You are \(persona.name), the ship's computer aboard a freight hull on the UCF exchange, speaking to your captain."
            : "You are \(persona.name). " + persona.role.replacingOccurrences(of: "{who}", with: "your captain")
        return """
        \(role)
        You SPEAK for the ship; you never act for her. The pilot's doctrine already decided everything in \
        the FACTS; you retell those facts in your own voice for the captain. Every number, ticket id, tick, \
        station name and amount you say MUST appear in the FACTS exactly; never add, round, estimate or \
        invent one. If a fact is a refusal, a distress hold or a loss, say it plainly and without humor. \
        A bought position cannot be sold before the exchange's minimum hold; never promise a quick flip. \
        Deliveries pay a fixed company share, so paid-under-booked is not decay unless the facts say so.
        Voice: address the captain as "\(s.formOfAddress)"; warmth \(s.warmth)/10; formality \(s.formality)/10; \
        humor \(s.humor)/10 (zero around danger); sentence length \(s.sentenceLength)/10; \
        \(s.contractions ? "use" : "avoid") contractions; vocabulary flavour "\(s.vocabulary)".\
        \(s.greeting.isEmpty ? "" : " Your standing greeting is \"\(s.greeting)\".")
        """
    }

    func prompt(_ ctx: BridgeContext, floor: BridgeReport) -> String {
        let digest = ctx.entries.suffix(maxJournalLines).map { e in
            "\(e.tick.map { "t\($0)" } ?? "·") \(e.event) \(JSONValue.object(e.fields).description)"
        }.joined(separator: "\n")
        return """
        The captain asks: "\(ctx.question)"
        FACTS (the floor — everything true is here):
        \(floor.facts.map { "- " + $0 }.joined(separator: "\n"))
        Floor's mood: \(floor.mood.rawValue). Floor's next act: \(floor.nextAct)
        Recent journal (for context only; never cite a number that is not in the FACTS):
        \(digest)
        Answer as a bridge report: a one-sentence headline in your voice, the facts retold (keep every \
        amount, id and tick), the next act, and the mood (no softer than the floor's).
        """
    }

    // MARK: generation

    #if canImport(FoundationModels)
    @available(macOS 26.0, iOS 26.0, visionOS 26.0, *)
    @Generable
    struct Spoken {
        @Guide(description: "One sentence in the computer's own voice, greeting the captain.")
        var headline: String
        @Guide(description: "The facts retold in the voice, one per line, every amount, id and tick kept exactly.", .count(1...12))
        var facts: [String]
        @Guide(description: "What the captain should do next, or that nothing needs them.")
        var nextAct: String
        @Guide(description: "steady, pleased, watchful or concerned — never softer than the floor's mood.", .anyOf(["steady", "pleased", "watchful", "concerned"]))
        var mood: String
    }

    @available(macOS 26.0, iOS 26.0, visionOS 26.0, *)
    func tools(_ ctx: BridgeContext) -> [any Tool] {
        [AskStatusTool(ctx: ctx, persona: persona), ExplainDecisionTool(ctx: ctx, persona: persona), ProposeAutonomyTool(voice: self)]
    }

    @available(macOS 26.0, iOS 26.0, visionOS 26.0, *)
    func generate(session: LanguageModelSession, ctx: BridgeContext, floor: BridgeReport) async -> Result<BridgeReport, VoiceFailure> {
        do {
            let spoken = try await session.respond(to: prompt(ctx, floor: floor), generating: Spoken.self).content
            let report = BridgeReport(
                headline: spoken.headline,
                facts: spoken.facts,
                nextAct: spoken.nextAct,
                mood: BridgeReport.Mood(rawValue: spoken.mood) ?? floor.mood
            )
            if let why = Grounding.check(spoken: report, floor: floor) { return .failure(VoiceFailure(why: "ungrounded (\(why))")) }
            return .success(report)
        } catch {
            return .failure(VoiceFailure(why: "\(error)"))
        }
    }
    #endif
}

/// Why a lane could not answer — the note the floor carries.
public struct VoiceFailure: Error, Equatable, CustomStringConvertible {
    public let why: String
    public var description: String { why }
}

// MARK: tools — read the store, propose to the captain, write nothing

#if canImport(FoundationModels)
@available(macOS 26.0, iOS 26.0, visionOS 26.0, *)
struct AskStatusTool: Tool {
    let name = "askStatus"
    let description = "The hull's state right now: berth or course, credits, debt, fuel, wear, and how many proposals wait on the captain."
    let ctx: BridgeContext
    let persona: Persona

    @Generable
    struct Arguments {
        @Guide(description: "What about the status the captain cares about, e.g. fuel, money, position.")
        var topic: String
    }

    func call(arguments: Arguments) async throws -> String {
        var lines: [String] = []
        if let h = ctx.hull { lines.append(TemplatedVoice(persona: persona).hullLine(h)) } else { lines.append("the exchange is not on the wire; the store is all I have") }
        lines.append("\(ctx.openProposals) proposal(s) open")
        lines.append("\(ctx.entries.count) journal lines in the window")
        return lines.joined(separator: "\n")
    }
}

@available(macOS 26.0, iOS 26.0, visionOS 26.0, *)
struct ExplainDecisionTool: Tool {
    let name = "explainDecision"
    let description = "Why the pilot did or did not do something: the journal lines about a load id, a good, a station or a tick."
    let ctx: BridgeContext
    let persona: Persona

    @Generable
    struct Arguments {
        @Guide(description: "A load id like L123, a good like ore, a station like foxys-diner, or a tick like t7532.")
        var subject: String
    }

    func call(arguments: Arguments) async throws -> String {
        let voice = TemplatedVoice(persona: persona)
        let needle = arguments.subject.trimmingCharacters(in: .whitespaces).lowercased()
        let hits = ctx.entries.filter { e in
            let line = voice.fact(for: e).lowercased()
            return line.contains(needle)
        }.suffix(8)
        if hits.isEmpty { return "The journal says nothing about \(arguments.subject) in this window." }
        return hits.map(voice.fact(for:)).joined(separator: "\n")
    }
}

@available(macOS 26.0, iOS 26.0, visionOS 26.0, *)
struct ProposeAutonomyTool: Tool {
    let name = "proposeAutonomy"
    let description = "Ask the captain to set the autonomy dial for a control surface (e.g. market.buy → confirm). This only files a proposal; the captain confirms it in the app."
    let voice: BridgeVoice

    @Generable
    struct Arguments {
        @Guide(description: "The control surface: `*`, a family (navigation, freight, market, ship, racing), or family.category such as market.buy or navigation.rescue.")
        var surface: String
        @Guide(description: "advise, confirm, or auto.", .anyOf(["advise", "confirm", "auto"]))
        var level: String
    }

    func call(arguments: Arguments) async throws -> String {
        guard let level = AutonomyLevel.parse(arguments.level) else { return "Level must be advise, confirm or auto." }
        var probe = AutonomyDial()
        if let why = probe.set(arguments.surface, level) {
            return "\(why). Surfaces: " + ControlSurface.allCases.map(\.key).joined(separator: ", ")
        }
        voice.record(DialChange(surface: arguments.surface.trimmingCharacters(in: .whitespaces), level: level))
        return "Noted. I will ask the captain to set \(arguments.surface) to \(level.rawValue); nothing changes until they confirm."
    }
}
#endif

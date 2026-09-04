import XCTest
@testable import FamiliarSC

/// The templated floor (T-236 brick-2 discipline) and the grounding check that every
/// spoken report must pass: same fixture + same persona = same bytes; style changes phrasing,
/// never facts; danger silences humor; the model may not invent or soften.
final class VoiceTests: XCTestCase {
    var persona: Persona { try! Fixtures.store.persona()! }
    var entries: [JournalEntry] { Fixtures.journal().entries }

    func testByteIdenticalAcrossRuns() {
        let a = TemplatedVoice(persona: persona).report(entries: entries, openProposals: 1)
        let b = TemplatedVoice(persona: persona).report(entries: entries, openProposals: 1)
        XCTAssertEqual(a, b)
        let enc = JSONEncoder(); enc.outputFormatting = .sortedKeys
        XCTAssertEqual(try enc.encode(a), try enc.encode(b))
    }

    func testStyleChangesPhrasingNotFacts() {
        var cool = Style(); cool.warmth = 0; cool.humor = 0; cool.contractions = false; cool.vocabulary = "plain"; cool.formOfAddress = "Skipper"
        var warmVoice = TemplatedVoice(persona: persona); warmVoice.maxFacts = 100
        var coldVoice = TemplatedVoice(persona: Persona(name: "Purr", style: cool)); coldVoice.maxFacts = 100
        let warm = warmVoice.report(entries: entries)
        let cold = coldVoice.report(entries: entries)
        XCTAssertNotEqual(warm.headline, cold.headline)
        XCTAssertEqual(warm.facts.count, cold.facts.count)
        XCTAssertEqual(warm.mood, cold.mood)
        // Every load-bearing token — numbers, ids, ticks, stations — survives the style, in order.
        let tokens = { (r: BridgeReport) in r.facts.map { Grounding.tokens(in: $0) } }
        XCTAssertEqual(tokens(warm), tokens(cold))
        XCTAssertTrue(warm.facts.contains { $0.contains("I've") })
        XCTAssertTrue(cold.facts.contains { $0.contains("I have") })
        XCTAssertFalse(cold.facts.contains { $0.contains("I've") })
    }

    func testDangerSilencesHumorAndSetsTheMood() {
        let r = TemplatedVoice(persona: persona).report(entries: entries)
        XCTAssertEqual(r.mood, .concerned, "a distress hold is in the window")
        XCTAssertFalse(r.headline.contains("Whiskers"), "humor 8 + feline reads as zero around danger")
        XCTAssertTrue(r.facts.contains { $0.contains("DISTRESS HOLD — stranded at 12 fuel") })
        XCTAssertTrue(r.nextAct.contains("Distress hold"))
        let noHold = entries.filter { $0.event != "distress-hold" }
        let r3 = TemplatedVoice(persona: persona).report(entries: noHold)
        XCTAssertEqual(r3.mood, .concerned)
        XCTAssertEqual(r3.nextAct, "Last trouble — t241: carry ore → cannery-row refused — HTTP 503 — and the pilot has flown on since (t251).")
        let calm = entries.filter { !TemplatedVoice.isDanger($0) && $0.event != "sighted-comet" }
        let r2 = TemplatedVoice(persona: persona).report(entries: calm)
        XCTAssertEqual(r2.mood, .pleased)
        XCTAssertTrue(r2.headline.contains("Whiskers steady."), r2.headline)
        XCTAssertTrue(r2.headline.hasPrefix("Mrrp. Captain,"), r2.headline)
    }

    func testCriticalLinesKeepTheirWholePayloadAndUnknownsRenderNeutrally() {
        let v = TemplatedVoice(persona: persona)
        let byEvent = Dictionary(grouping: entries, by: \.event)
        XCTAssertEqual(v.fact(for: byEvent["position-opened"]![0]),
            "t123: opened 40 ore at ask 15, bound for io-slagworks; sellable from t411 — the exchange's minimum hold, so she's riding under freight till then")
        XCTAssertEqual(v.fact(for: byEvent["refused-at-the-door"]![0]), "t181: Book { load_id: \"L2\" } refused at the door — HTTP 429")
        XCTAssertEqual(v.fact(for: byEvent["proposed"]![0]), "t201: proposed [freight.book] book L3 — best rate on the board; waiting on you until t205 (p-0123456789abcdef)")
        XCTAssertEqual(v.fact(for: byEvent["outfitted"]![0]), "t215: fitted drive-tune for ℳ9000 at titania-cold-store (reserve ℳ4560) — ℳ1200")
        XCTAssertEqual(v.fact(for: byEvent["trade-outcome"]![0]), "t160: sell 40 ore: rejected: minimum hold (sellable at tick 411)")
        XCTAssertEqual(v.fact(for: byEvent["exchange-unreachable"]![0]), "·: exchange unreachable — Io(\"Connection refused (os error 61)\")")
        let collapsed = TemplatedVoice.collapse(byEvent["exchange-unreachable"]!)
        XCTAssertEqual(collapsed.count, 1)
        XCTAssertEqual(v.fact(for: collapsed[0]), "·: exchange unreachable — Io(\"Connection refused (os error 61)\") (×2)")
        XCTAssertEqual(v.fact(for: byEvent["sighted-comet"]![0]), "t251: sighted-comet {\"note\":\"a word this build never learned\",\"tail_km\":123456}")
    }

    func testChatterIsSummarisedNotListedAndTheWindowIsBounded() {
        let r = TemplatedVoice(persona: persona).report(entries: entries)
        XCTAssertFalse(r.facts.contains { $0.contains("waiting on the crane") && $0.hasPrefix("t") })
        XCTAssertTrue(r.facts.contains { $0.hasPrefix("7 quiet folds: waiting on the crane; under way; no profitable") }, r.facts.last ?? "")
        XCTAssertTrue(r.facts.contains { $0.contains("more lines in the journal") })
        // The most consequential lines survive the cap, chronologically.
        let ticks = r.facts.compactMap { f -> Int64? in
            guard f.hasPrefix("t"), let end = f.firstIndex(of: ":") else { return nil }
            return Int64(f[f.index(after: f.startIndex)..<end])
        }
        XCTAssertEqual(ticks, ticks.sorted())
        XCTAssertTrue(r.facts.contains { $0.contains("DISTRESS HOLD") })
        XCTAssertTrue(r.facts.contains { $0.contains("refused at the door") })
    }

    func testHullLineAndNextActFromTheWire() throws {
        let me = try ExchangeWire.me(Fixtures.wire("me"))
        let r = TemplatedVoice(persona: persona).report(entries: [], hull: HullGlance(me: me))
        XCTAssertEqual(r.facts.count, 1)
        XCTAssertTrue(r.facts[0].contains("under way for foxys-diner — ℳ7132 — debt ℳ21400 — fuel 166/600 — wear 1104bps — leased hull"), r.facts[0])
        XCTAssertEqual(r.nextAct, "Under way for foxys-diner; nothing to do until she berths.")
        XCTAssertEqual(r.mood, .steady)
        let low = TemplatedVoice(persona: persona).report(entries: [], hull: HullGlance(docked: "foxys-diner", credits: 10, fuel: 50, fuelCapacity: 600))
        XCTAssertEqual(low.nextAct, "Fuel 50/600: the pilot will divert to a pump before the next leg.")
    }

    func testGroundingRefusesInventionAndSoftening() {
        let floor = TemplatedVoice(persona: persona).report(entries: entries)
        XCTAssertNil(Grounding.check(spoken: floor, floor: floor))
        var retold = floor
        retold.headline = "Captain, a rough watch — but the ore rides under freight till t411."
        XCTAssertNil(Grounding.check(spoken: retold, floor: floor), "a rephrase that cites only the floor's tokens passes")
        var invented = floor
        invented.facts.append("I also sold 200 ore at 99 at tuna-prime.")
        XCTAssertEqual(Grounding.check(spoken: invented, floor: floor), "invented: 200, 99, tuna-prime")
        var soft = floor
        soft.mood = .pleased
        XCTAssertEqual(Grounding.check(spoken: soft, floor: floor), "softened mood concerned to pleased")
        var harder = floor
        harder.mood = .concerned
        XCTAssertNil(Grounding.check(spoken: harder, floor: floor), "more worried than the floor is allowed")
    }

    func testTheLadderAlwaysAnswers() async {
        // On a machine without the model this is the floor; with it, a grounded retelling.
        // Either way the report exists, and the lane says which.
        let ctx = BridgeContext(entries: entries, openProposals: 1)
        let voice = BridgeVoice(persona: persona)
        let spoken = await voice.speak(ctx, consent: VoiceConsent(privateCloudCompute: false))
        XCTAssertFalse(spoken.report.facts.isEmpty)
        XCTAssertNil(Grounding.check(spoken: spoken.report, floor: voice.floor(ctx)))
        print("lane:", spoken.lane.rawValue, spoken.note ?? "")
        let avail = BridgeVoice.availability(consent: VoiceConsent())
        XCTAssertEqual(avail[.templated], "always")
        XCTAssertEqual(avail[.privateCloudCompute], "consent off")
    }
}

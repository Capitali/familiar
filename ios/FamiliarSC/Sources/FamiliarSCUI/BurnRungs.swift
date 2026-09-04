import Foundation
import FamiliarSC

// The burn rungs, ported from crates/whisker/src/doctrine.rs (2161a55): the exchange prices
// four throttles and each is a real trade — time goes as 1/√a and propellant follows the
// rocket equation on a Δv that goes as √a. Standard FIRST, always; economy only when standard
// cannot reach; never up. Under a contract the LOAD's class governs every leg, so the rung
// only reaches the physics on an unbooked voyage — the run to a pump, the merchant carry.
// Kibble Klipper sat three days at titania on 135 of 600 with foxys-diner 168 away at
// standard and 112 away at economy (2026-09-04); nothing was wrong with the tank.
//
// Two figures, deliberately different: the exchange's /v1/route quote is ALWAYS at the
// reference drive (189 mG) whatever the hull flies, so `modelAgrees` checks at 189; the rung's
// own fuel uses the hull's drive × bps, where the hull's drive is `effectiveAccelMilliG` read
// off /v1/me — the one number with wear and fittings already applied. Never derive it.

public enum BurnRungs {
    public static let referenceAccelMilliG: Int64 = 189
    public static let economyBps: Int64 = 5_000
    public static let standardBps: Int64 = 10_000
    static let exhaustKmS = 10_000.0
    static let fuelScale = 236.0
    static let dockOverhead = 5.0

    /// Mission Δv for one leg, km/s: `2√(D·a)`, which at the shipped drive is (62/720)·√D and
    /// scales as √(a/aRef) either side of it.
    static func legDeltaV(distanceKm: Int64, accelMilliG: Int64) -> Double {
        guard distanceKm > 0 else { return 0 }
        let ratio = (Double(max(accelMilliG, 1)) / Double(referenceAccelMilliG)).squareRoot()
        return 62.0 * Double(distanceKm).squareRoot() / 720.0 * ratio
    }

    /// Propellant for one leg at a drive — the engine's own rocket equation, exact.
    public static func legFuel(distanceKm: Int64, accelMilliG: Int64) -> Int64 {
        guard distanceKm > 0 else { return Int64(dockOverhead) }
        let dv = legDeltaV(distanceKm: distanceKm, accelMilliG: accelMilliG)
        return Int64((dockOverhead + fuelScale * (exp(dv / exhaustKmS) - 1)).rounded(.down))
    }

    /// What a whole route costs at a rung, given the hull's own drive.
    public static func routeFuel(legsKm: [Int64], hullAccelMilliG: Int64, burnBps: Int64) -> Int64 {
        let accel = max(hullAccelMilliG * burnBps / 10_000, 1)
        return legsKm.reduce(0) { $0 + legFuel(distanceKm: $1, accelMilliG: accel) }
    }

    /// Does the model agree with the exchange's own quote at the reference drive? Agreement
    /// buys the right to reason about rungs; disagreement means the world prices differently,
    /// and then the only figure to trust is the quote.
    public static func modelAgrees(legsKm: [Int64], quotedAtReference: Int64) -> Bool {
        guard quotedAtReference > 0, !legsKm.isEmpty else { return false }
        let modelled = legsKm.reduce(0) { $0 + legFuel(distanceKm: $1, accelMilliG: referenceAccelMilliG) }
        let slack = max(quotedAtReference / 20, 2)
        return abs(modelled - quotedAtReference) <= slack
    }

    public struct Plan: Equatable, Sendable {
        public var burn: String      // "standard" | "economy"
        public var fuel: Int64
        public var reaches: Bool
    }

    /// The rung a pump is reached at: standard if it reaches, else economy if that does; the
    /// cheapest rung's figure with `reaches: false` when neither does. With an unverified model,
    /// the exchange's own quote at standard is all that is claimed.
    public static func plan(legsKm: [Int64], quotedAtReference: Int64, hullAccelMilliG: Int64, tank: Int64) -> Plan {
        guard modelAgrees(legsKm: legsKm, quotedAtReference: quotedAtReference) else {
            return Plan(burn: "standard", fuel: quotedAtReference, reaches: quotedAtReference <= tank)
        }
        let standard = routeFuel(legsKm: legsKm, hullAccelMilliG: hullAccelMilliG, burnBps: standardBps)
        if standard <= tank { return Plan(burn: "standard", fuel: standard, reaches: true) }
        let economy = routeFuel(legsKm: legsKm, hullAccelMilliG: hullAccelMilliG, burnBps: economyBps)
        return Plan(burn: "economy", fuel: economy, reaches: economy <= tank)
    }
}

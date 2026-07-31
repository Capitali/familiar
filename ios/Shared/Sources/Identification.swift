import Foundation

// Friendly identification (ADR-0019) — knowing WHO is at this device, so the mesh can address a
// person instead of broadcasting at everyone.
//
// **Identification addresses; it never authorises.** This is the load-bearing property and the
// reason none of this touches a consent gate or unlocks anything. Sensitive-personal data
// (heart rate, precise position, face signatures) stays scoped by consent and node-locality; if
// being recognised ever granted access, then "the camera thinks this is Betty" would start
// handing out Betty's body data to whoever happened to be standing there. A claim here is a
// routing hint with a confidence attached, nothing more.
//
// The ladder, cheapest rung first — each only runs when the one above is unavailable or
// contradicted:
//
//   1. binding   — a personal device knows its owner. No camera, no model, no prompt.
//   2. face      — VERIFY that prior 1:1 ("should be Jeff; does the face agree?"), never search 1:N.
//   3. asked     — the human answered the confirm-or-correct prompt. Authoritative.
//   4. unknown   — don't guess. An unidentified device is not a delivery address for anything personal.

/// How a device is used, which decides whether it has a prior at all.
enum DeviceRole: String, Codable, CaseIterable {
    /// One person carries it. A phone or a watch is this by nature.
    case personal
    /// Many people touch it — a galley iPad, a desk Mac. No prior; identification must be earned.
    case shared

    /// The honest default for this hardware. A human can always override it: a shared iPad can
    /// become someone's, and a phone can change hands, and neither should need ceremony.
    static var suggested: DeviceRole {
        switch PlatformDevice.kind {
        case "phone", "watch": return .personal
        default: return .shared
        }
    }

    var label: String {
        switch self {
        case .personal: return "personal — one person carries this"
        case .shared: return "shared — anyone here may use this"
        }
    }
}

/// A time-bounded statement about who is at a device. It **expires**, which is the whole point:
/// "Jeff is here" and "Jeff was here an hour ago" must never be the same fact, or routing a
/// question to Jeff is a guess dressed as a decision.
struct PresenceClaim: Equatable {
    enum Via: String, Codable {
        case binding
        case face
        case asked
        /// Handed down from a paired device (a watch takes its phone's human). Shortest life of
        /// the four — a watch on the charger is not its owner.
        case inherited
        case unknown
    }

    /// The identified human's handle. Empty means genuinely unknown, which is a valid answer.
    var handle: String
    var via: Via
    /// How sure, carried through to the attribution rather than flattened away.
    var confidence: Double
    var since: Date
    var expires: Date

    static let unknown = PresenceClaim(
        handle: "", via: .unknown, confidence: 0, since: .distantPast, expires: .distantPast
    )

    var isLive: Bool { !handle.isEmpty && Date() < expires }

    /// How long each kind of evidence stays good for. A binding outlives a face sighting because
    /// the fact it rests on ("this is Jeff's phone") is durable, where a face is a moment.
    static func life(of via: Via) -> TimeInterval {
        switch via {
        case .asked: return 8 * 3600
        case .binding: return 12 * 3600
        case .face: return 30 * 60      // matches the daemon's PRESENCE_WINDOW_SECS
        case .inherited: return 15 * 60
        case .unknown: return 0
        }
    }

    /// Nominal confidence per rung. Deliberately short of 1.0 for anything the machine concluded
    /// on its own — only a human answering is certain, and even that decays.
    static func confidence(of via: Via) -> Double {
        switch via {
        case .asked: return 1.0
        case .face: return 0.9
        case .binding: return 0.7
        case .inherited: return 0.5
        case .unknown: return 0
        }
    }

    static func make(handle: String, via: Via, since: Date = Date()) -> PresenceClaim {
        guard !handle.isEmpty, via != .unknown else { return .unknown }
        return PresenceClaim(
            handle: handle, via: via, confidence: confidence(of: via),
            since: since, expires: Date().addingTimeInterval(life(of: via))
        )
    }
}

/// Resolves the ladder. Pure and platform-free so it can be reasoned about and tested without a
/// camera, a mesh, or a device: hand it what each rung knows and it returns the claim.
enum Identification {
    /// - Parameters:
    ///   - role: how this device is used.
    ///   - owner: the bound owner's handle, when the device is personal.
    ///   - answered: the human's own answer to the prompt, if one is still good.
    ///   - faceVerified: the handle a 1:1 face check CONFIRMED, if one is still good.
    ///   - faceContradicted: true when a 1:1 check ran against the prior and disagreed — which
    ///     demotes the binding rather than silently overriding the camera with the device.
    static func resolve(
        role: DeviceRole,
        owner: String,
        answered: PresenceClaim? = nil,
        faceVerified: PresenceClaim? = nil,
        faceContradicted: Bool = false
    ) -> PresenceClaim {
        // A human's own answer beats every inference, and beats a camera that disagrees with them.
        if let a = answered, a.isLive { return a }
        // A confirmed 1:1 check is the strongest thing the machine can conclude by itself.
        if let f = faceVerified, f.isLive { return f }
        // The prior — unless the camera actively disagreed with it, in which case we know less
        // than we did and must say so rather than assert the device's guess over the evidence.
        if role == .personal, !owner.isEmpty, !faceContradicted {
            return .make(handle: owner, via: .binding)
        }
        return .unknown
    }
}

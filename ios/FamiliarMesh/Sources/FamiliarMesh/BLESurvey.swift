import Foundation

/// The BLE survey's one vocabulary and one policy (T-228, Q3 closed by codex round 2).
///
/// **The floor:** a BLE surveyor may say the service-UUID CLASS of what it saw, plus a
/// coarse per-window count — and nothing else. Never the peripheral name, manufacturer
/// bytes, advertisement payload, platform identifier, or any cross-window token; rotations
/// are never joined. A peripheral identifier may exist only inside one survey window's
/// memory (for dedup) and dies with it. "Useful enough to learn that lighting-class BLE is
/// present, without learning who walked past."
///
/// This lives in FamiliarMesh for the same reasons `ServiceSurvey` does: the shells share
/// it, the tests can reach it, and the next radio inherits a discipline instead of
/// inventing a dialect. The class map below is repo-authored and CLOSED — editing it is
/// the reviewable act that widens both what a survey may store and what an App Intent may
/// speak (`IntentProjection.speakableKinds` includes these classes).
public enum BLESurvey {

    /// Standard 16-bit GATT service UUIDs (lowercase hex) → the class a survey reports.
    /// Deliberately conservative: well-known assigned numbers only, plus `ffe0` — the
    /// de-facto vendor serial service a whole family of BLE modules advertises (honestly
    /// classed as vendor-serial, not guessed into anything more specific).
    public static let serviceClasses: [String: String] = [
        "1800": "generic-access",
        "180a": "device-info",
        "180d": "heart-rate",
        "180f": "battery",
        "1809": "thermometer",
        "1810": "blood-pressure",
        "1812": "hid",
        "1815": "automation-io",
        "1816": "cycling-cadence",
        "1818": "cycling-power",
        "181a": "environmental",
        "181b": "body-composition",
        "1826": "fitness-machine",
        "1827": "mesh-provisioning",
        "1828": "mesh-proxy",
        "ffe0": "vendor-serial",
    ]

    /// The classes this survey may ever report — what joins the speakable vocabulary.
    public static var classes: Set<String> { Set(serviceClasses.values) }

    /// Normalize an advertised service UUID to its class, or nil for anything unknown.
    /// CoreBluetooth renders 16-bit UUIDs as `"180D"` and full UUIDs in 128-bit form;
    /// a 128-bit UUID on the Bluetooth base (`0000xxxx-0000-1000-8000-00805f9b34fb`)
    /// collapses to its 16-bit alias. An unknown UUID — every vendor's random 128-bit
    /// service — maps to NOTHING: it is counted in the window total but never named,
    /// because naming it would mint a class the repo never authored.
    public static func bleClass(forServiceUUID uuid: String) -> String? {
        let lower = uuid.lowercased()
        if lower.count == 4 { return serviceClasses[lower] }
        let base = "-0000-1000-8000-00805f9b34fb"
        if lower.count == 36, lower.hasSuffix(base), lower.hasPrefix("0000") {
            return serviceClasses[String(lower.dropFirst(4).prefix(4))]
        }
        return nil
    }

    /// The coarse per-window count (Q3's "bounded class/count bucket that contains no
    /// source substring"): the bucket word is the entire context of a BLE observation.
    public static func bucket(_ count: Int) -> String {
        switch count {
        case ..<1: return ""
        case 1: return "one"
        case 2...5: return "few"
        default: return "many"
        }
    }

    /// One window's report: `(object, context)` pairs ready to become observations —
    /// `ble:<class>` with the coarse bucket, classes sorted so a window's report is
    /// deterministic. Unknown-UUID sightings are deliberately absent: no class, no row.
    public static func windowReport(classCounts: [String: Int]) -> [(object: String, context: String)] {
        classCounts
            .filter { classes.contains($0.key) && $0.value >= 1 }
            .sorted { $0.key < $1.key }
            .map { ("ble:\($0.key)", "seen=\(bucket($0.value))") }
    }
}

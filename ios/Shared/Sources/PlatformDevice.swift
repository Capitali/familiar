import Foundation
#if os(iOS)
import UIKit
#endif

/// The few device facts the client core needs, on whichever Apple platform it is compiled for.
///
/// The mesh treats every shell the same — a Mac is a peer among equals, not a host — so the client
/// core is shared source. `UIDevice` is the only iOS-only thing it ever reached for, and only ever
/// for the name and model a node reports about itself, so it lives behind this instead of forcing
/// the whole core to be iOS-shaped.
enum PlatformDevice {
    /// The human-facing name of this machine, used as the node's roster label ("Codex", "Wildhorse").
    static var name: String {
        #if os(iOS)
        return UIDevice.current.name
        #else
        // Host.current().localizedName is the Sharing name — the same string the user set in
        // Settings and the one they will recognise on the roster. ProcessInfo's hostName would
        // give "wildhorse.local", which reads like plumbing.
        return Host.current().localizedName ?? ProcessInfo.processInfo.hostName
        #endif
    }

    /// Coarse hardware kind, for the actor prefix a node reports itself under.
    static var kind: String {
        #if os(iOS)
        return UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "phone"
        #else
        return "mac"
        #endif
    }

    /// OS name + version, reported as platform detail.
    static var systemDescription: String {
        #if os(iOS)
        let d = UIDevice.current
        return "\(d.systemName) \(d.systemVersion)"
        #else
        let v = ProcessInfo.processInfo.operatingSystemVersion
        return "macOS \(v.majorVersion).\(v.minorVersion).\(v.patchVersion)"
        #endif
    }
}

/// How this node names itself in the mesh's observation stream: `<kind>:<human it serves>`
/// (ADR-0016 — a node serves many humans, and the device says which one it is serving).
///
/// Lived in the iOS-only NetworkDiscovery until the Mac became a peer that reports observations
/// too; the kind now comes from `PlatformDevice`, so "mac:ian" is as expressible as "ipad:ian".
enum DeviceActor {
    static var human = "observer"
    static var current: String { PlatformDevice.kind + ":" + human }
}

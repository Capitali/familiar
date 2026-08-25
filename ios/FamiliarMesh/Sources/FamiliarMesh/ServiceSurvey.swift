import Foundation

/// The one vocabulary every shell's Bonjour survey speaks (T-228 brick 2).
///
/// Two shells survey the local network — `NetworkDiscovery` on iOS/iPadOS and
/// `MacNetworkDiscovery` on macOS — and each carried its own copy of the service list, its own
/// idea of how to name what it found, and its own way of turning a service type into an
/// observation. Nothing here changes what either one *does*; it gives them one place to say it,
/// so the discipline lands once instead of twice and the next radio (BLE) inherits it rather than
/// inventing a third dialect.
///
/// This lives in FamiliarMesh rather than the app targets for two reasons: both shells already
/// import it, and it is the only layer of this that a test can reach.
public enum ServiceSurvey {

    /// Service types worth surveying — peers first, then the everyday advertisements a home, boat
    /// or RV network exposes: remote access, media endpoints, printers, smart-home, file shares,
    /// brokers.
    ///
    /// **This list is not the whole truth on its own.** Apple resolves only the types an app
    /// declares, so every entry must also appear in `NSBonjourServices` in BOTH
    /// `ios/App/Support/Info.plist` and `ios/MacApp/Support/Info.plist`, or the browse silently
    /// returns nothing for it — silently being the operative word, which is why the three lists
    /// drifting apart would be invisible rather than loud. They are identical as of this commit
    /// (26 entries in all three); adding a type means editing all three.
    public static let serviceTypes: [String] = [
        "_familiar-mesh._tcp",                                              // other familiars / peers
        "_ssh._tcp", "_sftp-ssh._tcp", "_rfb._tcp",                         // remote access (SSH, VNC)
        "_http._tcp", "_https._tcp",                                        // web endpoints
        "_airplay._tcp", "_raop._tcp", "_airport._tcp",                     // AirPlay / speakers / base stations
        "_googlecast._tcp", "_spotify-connect._tcp",                        // cast / audio
        "_ipp._tcp", "_ipps._tcp", "_printer._tcp", "_pdl-datastream._tcp", // printers
        "_homekit._tcp", "_hap._tcp",                                       // HomeKit accessories
        "_companion-link._tcp", "_apple-mobdev2._tcp",                      // Apple continuity / devices
        "_smb._tcp", "_afpovertcp._tcp",                                    // file shares
        "_daap._tcp", "_dacp._tcp",                                         // media libraries
        "_mqtt._tcp",                                                       // MQTT brokers (the boat/RV runs one)
        "_workstation._tcp", "_device-info._tcp",                           // general hosts
    ]

    /// `"_airplay._tcp"` → `"airplay"`. The short form is what an observation's `object` carries
    /// (`service:airplay`), so the kernel's `obs_class` matchers see one class per kind of thing
    /// rather than one per wire encoding of it.
    public static func kind(_ serviceType: String) -> String {
        serviceType.split(separator: ".").first.map { String($0.drop(while: { $0 == "_" })) } ?? serviceType
    }

    /// **The seam T-228's Q1 decides, and the only place its answer will land.**
    ///
    /// A survey reports the service kind and the advertised instance name. The kind is harmless.
    /// The name is not always: Bonjour instance names are overwhelmingly personal — "Ian's MacBook
    /// Pro", "Betty's AirPods" — so a survey whose stated discipline is "report what KIND of thing
    /// I saw" has been writing household and neighbour device names into observations that
    /// replicate mesh-wide and outlive the moment, underneath the viewer-scoped naming T-217 built.
    ///
    /// Q1 is open with codex (`docs/reviews/2026-08-24-clients-as-observatories-dialogue.md`), so
    /// **this deliberately preserves today's behaviour byte for byte**: the advertised name passes
    /// through unchanged. It exists now so that when Q1 closes — pass through, drop, classify,
    /// salted hash — exactly one function changes and both shells change with it. Do not add a
    /// second path to `context`; that is the whole point of this function existing.
    public static func context(forInstanceName name: String) -> String {
        name
    }
}

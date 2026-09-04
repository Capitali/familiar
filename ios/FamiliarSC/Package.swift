// swift-tools-version:5.9
import PackageDescription

// FamiliarSC — the ship's computer's Apple half (T-237 B2). Pure logic: the ship-store
// reader (persona, captain, automations, the autonomy dial, journal, proposals/approvals,
// holdings, deliveries), the message-window feed, a typed READ-ONLY client for the UCF
// exchange's /v1 wire, the pairing-key parse, the captain-notice policy, and the bridge
// voice — a typed bridge report rendered by a deterministic templated floor, or spoken by
// Apple Intelligence (on-device Foundation Models; Private Cloud Compute on OS 27 behind
// entitlement + consent) and checked against that floor before it is shown. Nothing here
// places an action on the exchange or writes a ship store: approval and the dial are the
// captain's acts, done by the app on the captain's tap, never by the model.
//
// Builds and tests on macOS (`swift test`) with no device, no key and no network. The
// Foundation Models lane compiles against the 27 SDK (this project builds with the newest
// Xcode, period) and degrades to the templated floor at runtime wherever the model is
// unavailable — the floor is the product, the model is the voice.
let package = Package(
    name: "FamiliarSC",
    // Floors are 26 (ADR-0046 / T-227); visionOS joins for the ΔV bridge (B4).
    platforms: [.macOS("26.0"), .iOS("26.0"), .visionOS("26.0")],
    products: [
        .library(name: "FamiliarSC", targets: ["FamiliarSC"]),
        // A macOS stand-in for the captain's bridge: reads a ship store and speaks the
        // report — the visible proof for B2 ("what did you do today", from a real journal).
        .executable(name: "familiar-bridge", targets: ["familiar-bridge"]),
    ],
    targets: [
        .target(name: "FamiliarSC"),
        .executableTarget(name: "familiar-bridge", dependencies: ["FamiliarSC"]),
        .testTarget(
            name: "FamiliarSCTests",
            dependencies: ["FamiliarSC"],
            resources: [.copy("Fixtures")]
        ),
    ]
)

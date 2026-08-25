// swift-tools-version:5.9
import PackageDescription

// FamiliarMesh — the phone/watch client half of the familiar's mesh. Pure logic (CryptoKit
// ed25519, the membership-cert canonicalization, the /mesh/observe wire types + client) so it
// builds and unit-tests on macOS (`swift test`) with no iOS device or provisioning. Sensor
// capture (CoreLocation/CoreMotion/HealthKit) and Keychain live in the app target, not here.
let package = Package(
    name: "FamiliarMesh",
    // Floors are 26 everywhere (ADR-0046) — the package moves WITH the app targets, or it
    // keeps compiling against the old floors while the apps claim 26. String form because
    // tools-version 5.9 has no `.v26` constant; bumping the tools version would also flip
    // the default language mode, which is its own decision, not a floor side effect.
    // tvOS is deliberately unchanged: nobody has decided anything about tvOS (T-227 note).
    platforms: [.macOS("26.0"), .iOS("26.0"), .watchOS("26.0"), .tvOS(.v17)],
    products: [
        .library(name: "FamiliarMesh", targets: ["FamiliarMesh"]),
        .executable(name: "familiar-observe", targets: ["familiar-observe"]),
    ],
    targets: [
        .target(name: "FamiliarMesh"),
        // A macOS stand-in for the phone: enroll from a payload and POST one signed observation —
        // used to prove the Swift client interoperates with the live Rust /mesh/observe endpoint.
        .executableTarget(name: "familiar-observe", dependencies: ["FamiliarMesh"]),
        .testTarget(name: "FamiliarMeshTests", dependencies: ["FamiliarMesh"]),
    ]
)

// swift-tools-version:5.9
import PackageDescription

// FamiliarMesh — the phone/watch client half of the familiar's mesh. Pure logic (CryptoKit
// ed25519, the membership-cert canonicalization, the /mesh/observe wire types + client) so it
// builds and unit-tests on macOS (`swift test`) with no iOS device or provisioning. Sensor
// capture (CoreLocation/CoreMotion/HealthKit) and Keychain live in the app target, not here.
let package = Package(
    name: "FamiliarMesh",
    platforms: [.macOS(.v13), .iOS(.v16), .watchOS(.v9)],
    products: [.library(name: "FamiliarMesh", targets: ["FamiliarMesh"])],
    targets: [
        .target(name: "FamiliarMesh"),
        .testTarget(name: "FamiliarMeshTests", dependencies: ["FamiliarMesh"]),
    ]
)

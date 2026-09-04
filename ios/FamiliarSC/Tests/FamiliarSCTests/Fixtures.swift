import Foundation
import XCTest
@testable import FamiliarSC

enum Fixtures {
    static var root: URL { Bundle.module.resourceURL!.appendingPathComponent("Fixtures") }
    static var ship: URL { root.appendingPathComponent("ship") }
    static func wire(_ name: String) -> Data {
        try! Data(contentsOf: root.appendingPathComponent("wire/\(name).json"))
    }
    static var store: ShipStore { ShipStore(directory: ship) }
    static func journal() -> Journal { try! store.journal() }

    /// A scratch copy of the fixture store, for tests that need to vary a file.
    static func scratchStore(_ mutate: (URL) throws -> Void) throws -> ShipStore {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("familiar-sc-\(UUID().uuidString)")
        try FileManager.default.copyItem(at: ship, to: dir)
        try mutate(dir)
        return ShipStore(directory: dir)
    }
}

import XCTest
import ExactCore

final class WidgetContainerTests: XCTestCase {
    func testPublicationSkipsUnchangedAssetsAndKeepsPinnedGenerations() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let names = ["plans/card.plan"]
        var reads = 0
        func assets(_ generation: String) -> AssetResolver {
            AssetResolver(root: root, names: names, generation: generation) { _ in
                reads += 1
                return Data(generation.utf8)
            }
        }
        func publish(_ assets: AssetResolver) throws -> Bool {
            try WidgetContainer.publish(assets, root: root, kinds: ["card"], compatibility: "cohort")
        }
        XCTAssertTrue(try publish(assets("first")))
        let base = root.appendingPathComponent("widget-generations")
        let first = try String(contentsOf: base.appendingPathComponent("current"), encoding: .utf8)
        XCTAssertFalse(try publish(assets("first")))
        XCTAssertEqual(reads, 1, "a new resolver for the same generation reads no assets")
        XCTAssertTrue(try publish(assets("second")))
        XCTAssertEqual(reads, 2)
        XCTAssertEqual(try String(contentsOf: base.appendingPathComponent(first + "/plans/card.plan"), encoding: .utf8), "first")
        XCTAssertNotEqual(try String(contentsOf: base.appendingPathComponent("current"), encoding: .utf8), first)
    }
}

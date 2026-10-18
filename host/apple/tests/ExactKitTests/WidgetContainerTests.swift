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
    func testTypeScriptPublicationRefusesEitherMissingHalf() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        for half in ["app.hbc", "app.module.json"] {
            let assets = AssetResolver(root: root, names: ["plans/card.plan", "typescript/" + half], generation: half) { _ in Data("bytes".utf8) }
            XCTAssertThrowsError(try WidgetContainer.publish(assets, root: root, kinds: ["card"], compatibility: "cohort", typescript: true))
        }
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("widget-generations/current").path))
    }

}

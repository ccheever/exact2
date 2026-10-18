#if EXACT_TEST_WIDGET_RELOAD
import Foundation
@_cdecl("exact_widget_reload")
public func testWidgetReload() { precondition(Thread.isMainThread) }
#else
import XCTest
import ExactCore

@MainActor
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
        try WidgetContainer.signal(root: root) {}
        XCTAssertFalse(try publish(assets("first")))
        XCTAssertEqual(reads, 1, "a new resolver for the same generation reads no assets")
        XCTAssertTrue(try publish(assets("second")))
        XCTAssertEqual(reads, 2)
        XCTAssertEqual(try String(contentsOf: base.appendingPathComponent(first + "/plans/card.plan"), encoding: .utf8), "first")
        XCTAssertNotEqual(try String(contentsOf: base.appendingPathComponent("current"), encoding: .utf8), first)
    }

    func testMissingArtifactRemainsUnsignalledAndRetries() throws { try retry(missingSymbol: false) }
    func testMissingSymbolRemainsUnsignalledAndRetries() throws { try retry(missingSymbol: true) }
    private func retry(missingSymbol: Bool) throws {
        let fm = FileManager.default
        let root = fm.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? fm.removeItem(at: root) }
        let assets = AssetResolver(root: root, names: ["plans/card.plan"], generation: "pending") { _ in Data("plan".utf8) }
        func publish() throws -> Bool { try WidgetContainer.publish(assets, root: root, kinds: ["card"], compatibility: "cohort") }
        let good = root.appendingPathComponent("reload.dylib")
        let bad = missingSymbol ? URL(fileURLWithPath: "/usr/lib/libSystem.B.dylib") : good
        let base = root.appendingPathComponent("widget-generations")
        XCTAssertTrue(try publish())
        for _ in 0..<2 {
            XCTAssertThrowsError(try WidgetContainer.signal(root: root) { try WidgetContainer.reloadTimelines(artifact: bad) }) { error in
                let message = String(describing: error)
                XCTAssertTrue(message.contains(missingSymbol ? "exact_widget_reload" : "dlopen"), message)
                XCTAssertTrue(message.contains(bad.path), message)
            }
            XCTAssertFalse(fm.fileExists(atPath: base.appendingPathComponent("signalled").path))
            XCTAssertTrue(try publish(), "an unchanged generation must retry a failed signal")
        }
        let fixture = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_WIDGET_RELOAD_TEST"])
        try fm.copyItem(at: URL(fileURLWithPath: fixture), to: good)
        try WidgetContainer.signal(root: root) { try WidgetContainer.reloadTimelines(artifact: good) }
        XCTAssertEqual(try Data(contentsOf: base.appendingPathComponent("signalled")), try Data(contentsOf: base.appendingPathComponent("current")))
        XCTAssertFalse(try publish())
    }
}
#endif

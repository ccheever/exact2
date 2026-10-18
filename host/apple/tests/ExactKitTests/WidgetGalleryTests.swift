#if EXACT_WIDGET_GALLERY && (os(macOS) || os(iOS))
import XCTest
import SwiftUI
import ExactCore
import ExactSnapshot
@testable import ExactKit

@MainActor
final class WidgetGalleryTests: XCTestCase {
    func testCaltrainGalleryListsOnlyItsPlatformFamilies() throws {
        let plan = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_SNAPSHOT_TRAINS_PLAN"]))
        let manifest = try JSONSerialization.jsonObject(with: Data(contentsOf: plan.deletingLastPathComponent().appendingPathComponent("entries/caltrain/app.json"))) as! [String: Any]
        let registry = try JSONSerialization.data(withJSONObject: try XCTUnwrap(manifest["widgets"]))
        let assets = AssetResolver(root: plan.deletingLastPathComponent(), names: ["plans/widgets.json"], generation: "platform-families") { _ in registry }
        XCTAssertEqual(WidgetGallery.entries(in: assets).first?.families, ["systemSmall", "systemMedium", "accessoryRectangular"])
    }
    #if os(macOS)
    func testDevelopMenuShowsTheFirstEntryAfterGenerationNotification() throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let empty = root.appendingPathComponent("empty"), added = root.appendingPathComponent("added")
        try FileManager.default.createDirectory(at: empty, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: added.appendingPathComponent("plans"), withIntermediateDirectories: true)
        let app = ExactApp.shared, previousRoot = app.assetRoot, previousMenu = NSApp.mainMenu, previousSession = DevMenu.session
        defer { app.assetRoot = previousRoot; NSApp.mainMenu = previousMenu; DevMenu.session = previousSession; try? FileManager.default.removeItem(at: root) }
        app.assetRoot = empty
        let session = ExactSession(app: app, label: "menu-reload")
        defer { session.destroy() }
        DevMenu.install(session: session, planPath: nil)
        func widgets() -> NSMenuItem? { NSApp.mainMenu?.items.first { $0.submenu?.title == "Develop" }?.submenu?.item(withTitle: "Widgets…") }
        XCTAssertNil(widgets())
        try Data("[{\"kind\":\"added\",\"families\":[\"systemSmall\"],\"preview\":{}}]".utf8).write(to: added.appendingPathComponent("plans/widgets.json"))
        app.assetRoot = added
        NotificationCenter.default.post(name: Notification.Name("ExactWidgetGeneration"), object: app)
        XCTAssertNotNil(widgets())
    }
    #endif
    func testGivenPropsSkipARefusingRefresh() throws {
        let plan = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_SNAPSHOT_REFUSING_PLAN"]))
        let assets = AssetResolver(root: plan.deletingLastPathComponent(), names: ["plans/refusing.plan"], generation: "refusing") { _ in try? Data(contentsOf: plan) }
        let entry = WidgetEntry(kind: "refusing", families: ["systemSmall"], preview: ["title":"Preview"], assets: assets)
        let session = ExactSession(app: ExactApp.shared, label: "given-props", launchPlace: LaunchPlace(environment: ["EXACT_AGENT":"1"]))
        session.clock = 0
        defer { session.destroy() }
        let variant = WidgetVariant(entry: "refusing", family: "systemSmall")
        XCTAssertThrowsError(try WidgetPreview(variant, entry: entry, appSession: session))
        let preview = try WidgetPreview(variant, entry: entry, appSession: session, props: "{\"title\":\"Given\"}")
        XCTAssertEqual(preview.refresh, "props")
        XCTAssertEqual(preview.props["title"] as? String, "Given")
        XCTAssertTrue(try preview.session.snapshot().nodes.values.contains { $0.props["text"] == "Given" })
    }
    func testReadsUseTheRefreshAtTheAppClockAndTheCurrentGeneration() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let plan = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_SNAPSHOT_TRAINS_PLAN"]))
        let manifest = try JSONSerialization.jsonObject(with: Data(contentsOf: plan.deletingLastPathComponent().appendingPathComponent("entries/caltrain/app.json"))) as! [String: Any]
        let directory = plan.deletingLastPathComponent().appendingPathComponent("gallery-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory.appendingPathComponent("plans"), withIntermediateDirectories: true)
        try FileManager.default.copyItem(at: plan, to: directory.appendingPathComponent("plans/next-trains.plan"))
        try JSONSerialization.data(withJSONObject: try XCTUnwrap(manifest["widgets"])).write(to: directory.appendingPathComponent("plans/widgets.json"))
        let app = ExactApp.shared, previous = app.assetRoot
        defer { app.assetRoot = previous; try? FileManager.default.removeItem(at: directory) }
        app.assetRoot = directory
        // 09:05 UTC: the next departure is the refresh's train 121, absent from preview.
        let epoch = 1_787_907_900_000.0
        let place = LaunchPlace(environment: ["EXACT_AGENT":"1", "EXACT_AGENT_EPOCH":String(epoch)])
        let session = ExactSession(app: app, label: "widget-test", launchPlace: place)
        session.clock = 0
        defer { session.destroy() }
        let agent = Agent(session: session)
        let address = ["entry":"next-trains", "family":"systemMedium", "mode":"accented", "scheme":"dark", "size":"xxxl"]
        func read(_ op: String, path: String? = nil) -> [String: Any] {
            WidgetGallery.answer(op, request: ["widget":address, "path":path as Any], agent: agent)
        }
        let state = read("state")
        XCTAssertNil(state["error"])
        XCTAssertEqual(state["refresh"] as? String, "timeline")
        let props = try XCTUnwrap(state["props"] as? [String: Any])
        let trains = try XCTUnwrap(props["departures"] as? [[String: Any]])
        XCTAssertEqual(trains.first?["train"] as? Int, 121)
        XCTAssertEqual(trains.first?["at"] as? Double, epoch + 180_000)
        let preview = try XCTUnwrap(agent.widgetPreview)
        XCTAssertEqual(preview.environment.rootFontSize, SnapshotDriver.bodySize(for: .xxxLarge))
        XCTAssertEqual(preview.environment.rootFontSize, 23)
        let tree = read("tree")
        XCTAssertNil(tree["error"])
        XCTAssertEqual(tree["clock"] as? Double, 0)
        XCTAssertNotNil(tree["epoch"]); XCTAssertNotNil(tree["incarnation"])
        let roots = try XCTUnwrap(tree["roots"] as? [UInt32])
        let nodes = try XCTUnwrap(tree["nodes"] as? [[String: Any]])
        XCTAssertFalse(roots.isEmpty)
        XCTAssertTrue(roots.allSatisfy { root in nodes.contains { ($0["id"] as? UInt32) == root && ($0["type"] as? String) == "view" } })
        XCTAssertTrue(nodes.contains { ($0["type"] as? String) == "text" && ($0["props"] as? [String: String])?["semanticTag"] == "time" && ($0["props"] as? [String: String])?["text"] == "03:00" })
        let png = directory.appendingPathComponent("entry.png")
        let image = read("screenshot", path: png.path)
        XCTAssertNil(image["error"])
        XCTAssertEqual(try XCTUnwrap(image["epoch"] as? UInt64), preview.session.agentFacts()["epoch"] as? UInt64)
        XCTAssertEqual(image["w"] as? CGFloat, 338); XCTAssertEqual(image["h"] as? CGFloat, 158)
        XCTAssertGreaterThan(try Data(contentsOf: png).count, 100)
        session.clock = 600_000
        let later = read("state")
        XCTAssertEqual(later["clock"] as? Double, 600_000)
        let laterProps = try XCTUnwrap(later["props"] as? [String: Any])
        XCTAssertNotEqual((laterProps["departures"] as? [[String: Any]])?.first?["train"] as? Int, 121)
        // A complete resolver can add entries without consulting the embedded directory.
        let registry = try JSONSerialization.data(withJSONObject: [["kind":"added", "families":["systemSmall"], "preview":[:]]])
        let assets = AssetResolver(root: directory, names: ["plans/widgets.json"], generation: "reload") { _ in registry }
        XCTAssertEqual(WidgetGallery.entries(in: assets).map(\.kind), ["added"])
    }
}
#endif

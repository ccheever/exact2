#if EXACT_WIDGET_GALLERY && (os(macOS) || os(iOS))
import XCTest
import SwiftUI
import ImageIO
import UniformTypeIdentifiers
import ExactCore
import ExactSnapshot
@testable import ExactKit

@MainActor
final class WidgetGalleryTests: XCTestCase {
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
    /// LLP 1126.001 WP10: the gallery lists an app's activities beside its
    /// widgets and renders each one's eight placements from its preview
    /// state, in the gallery's own width.
    func testActivitiesRenderTheirEightPlacementsInTheGallery() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let plans = ["departure": "EXACT_SNAPSHOT_DEPARTURE_PLAN", "service-alert": "EXACT_SNAPSHOT_SERVICE_ALERT_PLAN"]
            .mapValues { URL(fileURLWithPath: ProcessInfo.processInfo.environment[$0] ?? "") }
        let registry = try JSONSerialization.data(withJSONObject: [
            ["kind": "departure", "preview": ["station": "Mountain View", "headsign": "San Francisco", "train": 101, "at": 1_787_915_700_000, "status": "On time"]],
            ["kind": "service-alert", "preview": ["title": "Service delay", "message": "Northbound trains delayed.", "updatedAt": 1_787_915_700_000]],
        ])
        let assets = AssetResolver(root: FileManager.default.temporaryDirectory, names: ["plans/activities.json", "plans/departure.plan", "plans/service-alert.plan"], generation: "activities") { name in
            name == "plans/activities.json" ? registry : try Data(contentsOf: plans[String(name.dropFirst(6).dropLast(5))]!)
        }
        let entries = WidgetGallery.activities(in: assets)
        XCTAssertEqual(entries.map(\.kind), ["departure", "service-alert"])
        let session = ExactSession(app: ExactApp.shared, label: "activity-gallery", launchPlace: LaunchPlace(environment: ["EXACT_AGENT": "1", "EXACT_AGENT_EPOCH": "1787907900000"]))
        session.clock = 0
        defer { session.destroy() }
        for (entry, scheme) in zip(entries, ["light", "dark"]) {
            let preview = try ActivityPreview(entry, appSession: session, scheme: scheme)
            XCTAssertEqual(preview.state["station"] as? String ?? preview.state["title"] as? String, entry.kind == "departure" ? "Mountain View" : "Service delay")
            let renderer = ImageRenderer(content: ActivityGalleryView(preview: preview).frame(width: 362))
            renderer.scale = 1
            renderer.proposedSize = ProposedViewSize(width: 362, height: nil)
            let image = try XCTUnwrap(renderer.cgImage, entry.kind)
            if let out = ProcessInfo.processInfo.environment["EXACT_WP10_OUT"].map({ URL(fileURLWithPath: $0) }) {
                try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
                if let destination = CGImageDestinationCreateWithURL(out.appending(path: "gallery-\(entry.kind)-\(scheme).png") as CFURL, UTType.png.identifier as CFString, 1, nil) {
                    CGImageDestinationAddImage(destination, image, nil); CGImageDestinationFinalize(destination)
                }
            }
            XCTAssertEqual(image.width, 362)
            XCTAssertGreaterThan(image.height, 200, "the Lock Screen card, the expanded Island and the compact row stack")
            XCTAssertGreaterThanOrEqual(preview.render.layoutCount, 8, "each placement laid out at least once: \(preview.render.layoutCount)")
            let lock = try preview.render.layout(.lockScreen, width: 362 - 32, height: nil)
            XCTAssertEqual(lock.root.frame.width, 330)
            XCTAssertGreaterThan(preview.render.size(.compactTrailing, proposal: .unspecified).width, 0)
        }
        // A supplied state replaces the preview, as the agent's --state will.
        let given = try ActivityPreview(entries[0], appSession: session, state: ["station": "Palo Alto", "headsign": "San Jose", "train": 233, "at": 1_787_915_700_000, "status": "Delayed"])
        let laid = try given.render.layout(.lockScreen, width: 330, height: nil)
        XCTAssertTrue(laid.snapshot.nodes.values.contains { $0.lines.contains { $0.text == "Palo Alto to San Jose" } })
    }

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

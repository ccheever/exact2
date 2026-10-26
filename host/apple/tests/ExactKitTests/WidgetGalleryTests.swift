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
    func testActivityAgentUsesTheGalleryRegionWidthForTreeAndScreenshot() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let plan = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_SNAPSHOT_EDGES_PLAN"]))
        let directory = plan.deletingLastPathComponent().appendingPathComponent("agent-regions-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory.appendingPathComponent("plans"), withIntermediateDirectories: true)
        try FileManager.default.copyItem(at: plan, to: directory.appendingPathComponent("plans/edges.plan"))
        try Data("[{\"kind\":\"edges\",\"preview\":{\"title\":\"Private label\"}}]".utf8).write(to: directory.appendingPathComponent("plans/activities.json"))
        let app = ExactApp.shared, previous = app.assetRoot
        defer { app.assetRoot = previous; try? FileManager.default.removeItem(at: directory) }
        app.assetRoot = directory
        for width in [CGFloat(402), 800] {
            let session = ExactSession(app: app, label: "agent-regions", launchPlace: LaunchPlace(environment: ["EXACT_AGENT":"1"]))
            session.clock = 0
            defer { session.destroy() }
            XCTAssertNil(session.boot(plan: try fixturePlan("PAGE_EDGES"), size: CGSize(width: width, height: 900)).error)
            let agent = Agent(session: session)
            for placement in [ActivityPlacement.minimal, .compactTrailing] {
                let address = ["entry":"edges", "placement":placement.name]
                let tree = ActivityAgent.answer("tree", request: ["activity":address], agent: agent)
                XCTAssertNil(tree["error"])
                let preview = try XCTUnwrap(agent.activityPreview)
                let gallery = ActivityGalleryView(preview: preview, width: session.viewportSize.width - 32)
                let region = gallery.regionWidth(placement)
                let nodes = try XCTUnwrap(tree["nodes"] as? [[String:Any]])
                if placement == .minimal {
                    let text = nodes.flatMap { ($0["lines"] as? [[String:Any]])?.compactMap { $0["text"] as? String } ?? [] }
                    XCTAssertEqual(text, ["Thin"], "minimal uses its Island proposal on a phone and a wide Mac window")
                } else {
                    let frame = try XCTUnwrap(nodes.first?["frame"] as? [String:CGFloat])
                    XCTAssertEqual(try XCTUnwrap(frame["w"]), region, accuracy: 0.01, "the percentage root uses the region width")
                }
                let path = directory.appendingPathComponent("\(Int(width))-\(placement.name).png").path
                let shot = ActivityAgent.answer("screenshot", request: ["activity":address, "path":path], agent: agent)
                XCTAssertNil(shot["error"])
                XCTAssertEqual(try XCTUnwrap(shot["w"] as? Int), Int((region + 2 * gallery.geometry.compactInset).rounded(.up)))
                let source = try XCTUnwrap(CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil))
                let actual = try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))
                let expected = ImageRenderer(content: gallery.compactPlacement(placement)); expected.scale = 1
                let reference = try XCTUnwrap(expected.cgImage)
                XCTAssertEqual(actual.width, reference.width); XCTAssertEqual(actual.height, reference.height)
                XCTAssertEqual(rgba(actual), rgba(reference), "the screenshot draws the gallery region the tree measured")
            }
        }
    }

    private func rgba(_ image: CGImage) -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: image.width * image.height * 4)
        let context = CGContext(data: &bytes, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
                                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return bytes
    }

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
        for entry in entries {
            for scheme in ["light", "dark"] {
                let preview = try ActivityPreview(entry, appSession: session, scheme: scheme)
                XCTAssertEqual(preview.state["station"] as? String ?? preview.state["title"] as? String, entry.kind == "departure" ? "Mountain View" : "Service delay")
                for width in [CGFloat(362), 300] {
                    let renderer = ImageRenderer(content: ActivityGalleryView(preview: preview, width: width))
                    renderer.scale = 1
                    renderer.proposedSize = ProposedViewSize(width: width, height: nil)
                    let image = try XCTUnwrap(renderer.cgImage, entry.kind)
                    if let out = ProcessInfo.processInfo.environment["EXACT_WP10_OUT"].map({ URL(fileURLWithPath: $0) }) {
                        try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
                        if let destination = CGImageDestinationCreateWithURL(out.appending(path: "gallery-\(entry.kind)-\(scheme)-\(Int(width)).png") as CFURL, UTType.png.identifier as CFString, 1, nil) {
                            CGImageDestinationAddImage(destination, image, nil); CGImageDestinationFinalize(destination)
                        }
                    }
                    XCTAssertEqual(image.width, Int(width), "\(entry.kind) \(scheme): the gallery's width")
                    XCTAssertGreaterThan(image.height, 200, "the Lock Screen card, the expanded Island and the compact row stack")
                    let lock = try preview.render.layout(.lockScreen, width: width - 2 * ActivityPlacementView.platformMargins.leading, height: nil)
                    XCTAssertEqual(lock.size.width, width - 28, "the card's content is the gallery's width inside the HIG's margins")
                    // The expanded regions, allocated together at this width: disjoint, the centre below the camera, the bottom below all.
                    let view = ActivityGalleryView(preview: preview, width: width), frames = view.expandedFrames()
                    for (i, a) in frames.all.enumerated() { for b in frames.all[(i + 1)...] { XCTAssertFalse(a.intersects(b) && !a.intersection(b).isEmpty && a.intersection(b).width > 0.5 && a.intersection(b).height > 0.5, "\(entry.kind) at \(width): \(a) overlaps \(b)") } }
                    XCTAssertEqual(frames.center.minY, IslandGeometry.iPhone17Pro.obstruction.height)
                    XCTAssertGreaterThanOrEqual(frames.bottom.minY, max(frames.leading.maxY, frames.trailing.maxY, frames.center.maxY))
                    XCTAssertEqual(frames.leading.width, frames.trailing.width, "equal space for the sides")
                    XCTAssertLessThanOrEqual(frames.trailing.maxX, view.expandedInner + 0.5)
                }
                XCTAssertGreaterThanOrEqual(preview.render.layoutCount, 8, "each placement laid out at least once: \(preview.render.layoutCount)")
                XCTAssertGreaterThan(preview.render.size(.compactTrailing, proposal: .unspecified).width, 0)
            }
        }
        // A supplied state replaces the preview, as the agent's --state will.
        let given = try ActivityPreview(entries[0], appSession: session, state: ["station": "Palo Alto", "headsign": "San Jose", "train": 233, "at": 1_787_915_700_000, "status": "Delayed"])
        let laid = try given.render.layout(.lockScreen, width: 330, height: nil)
        XCTAssertTrue(laid.snapshot.nodes.values.contains { $0.lines.contains { $0.text == "Palo Alto to San Jose" } })
    }

    /// Tall side content and wide centre content never overlap: the centre
    /// gets what the sides' minimal width leaves, below the camera.
    func testExpandedRegionsAreAllocatedTogether() {
        let g = IslandGeometry.iPhone17Pro
        let frames = ExpandedArrangement.allocate(inner: 300, geometry: g, leading: CGSize(width: 60, height: 100), trailing: CGSize(width: 40, height: 20), center: CGSize(width: 200, height: 24), bottom: CGSize(width: 300, height: 20))
        XCTAssertEqual(frames.leading, CGRect(x: 0, y: 0, width: 60, height: 100))
        XCTAssertEqual(frames.trailing, CGRect(x: 240, y: 0, width: 60, height: 20))
        XCTAssertEqual(frames.center, CGRect(x: 60, y: g.obstruction.height, width: 180, height: 24))
        XCTAssertEqual(frames.bottom.minY, 100); XCTAssertEqual(frames.height, 120)
        for (i, a) in frames.all.enumerated() { for b in frames.all[(i + 1)...] { XCTAssertTrue(a.intersection(b).width < 0.5 || a.intersection(b).height < 0.5, "\(a) overlaps \(b)") } }
        // Sides wider than the camera leaves them: the limit holds and the centre still gets its share.
        let wide = ExpandedArrangement.allocate(inner: 200, geometry: g, leading: CGSize(width: 150, height: 20), trailing: CGSize(width: 150, height: 20), center: CGSize(width: 30, height: 20), bottom: .zero)
        XCTAssertEqual(wide.leading.width, wide.trailing.width); XCTAssertLessThanOrEqual(wide.leading.maxX, wide.center.minX + 0.5); XCTAssertLessThanOrEqual(wide.center.maxX, wide.trailing.minX + 0.5)
    }

    func testExpandedAllocationRemeasuresWrappedContentAndAlignsTheTrailingEdge() throws {
        let render = try ActivityRender(plan: Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_SNAPSHOT_EDGES_PLAN"]))),
                                        kind: "edges", state: nil, facts: SessionFacts(displayScale: 1, preferences: [], rootFontSize: 17), assets: AssetResolver(root: FileManager.default.temporaryDirectory))
        let placements: [ActivityPlacement] = [.expandedLeading, .expandedTrailing, .expandedCenter, .expandedBottom]
        let frames = ExpandedArrangement.measure(inner: 250, geometry: .iPhone17Pro) { render.size(placements[$0], proposal: $1) }
        let ideal = render.size(.expandedLeading, proposal: .unspecified)
        let narrowed = render.size(.expandedLeading, proposal: ProposedViewSize(width: frames.leading.width, height: nil))
        XCTAssertGreaterThan(narrowed.height, ideal.height)
        XCTAssertEqual(frames.leading.height, narrowed.height)
        XCTAssertGreaterThanOrEqual(frames.bottom.minY, narrowed.height)
        let renderer = ImageRenderer(content: ExpandedIslandLayout(geometry: .iPhone17Pro) {
            Color.blue.frame(width: 20, height: 10)
            Color.red.frame(width: 10, height: 10)
            Color.clear.frame(width: 30, height: 10)
            Color.clear.frame(height: 10)
        }.frame(width: 250).background(Color.white))
        renderer.scale = 1
        let image = try XCTUnwrap(renderer.cgImage)
        var pixels = [UInt8](repeating: 0, count: image.width * image.height * 4)
        let context = try XCTUnwrap(CGContext(data: &pixels, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
                                              space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        // CG's bitmap origin is at the bottom; find red along the last two columns.
        var rightRed = 0, leftRed = 0
        for y in 0..<image.height {
            for x in 0..<image.width {
                let at = (y * image.width + x) * 4
                if pixels[at] > 200 && pixels[at + 1] < 100 {
                    if x >= image.width - 12 { rightRed += 1 } else { leftRed += 1 }
                }
            }
        }
        XCTAssertGreaterThan(rightRed, 40); XCTAssertEqual(leftRed, 0)
    }

    func testActivityStateCacheDoesNotTreatMissingStateOrBooleansAsNumbers() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let plan = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_SNAPSHOT_DEPARTURE_PLAN"]))
        let manifest = try JSONSerialization.jsonObject(with: Data(contentsOf: plan.deletingLastPathComponent().appendingPathComponent("entries/caltrain/app.json"))) as! [String: Any]
        let directory = plan.deletingLastPathComponent().appendingPathComponent("activity-state-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory.appendingPathComponent("plans"), withIntermediateDirectories: true)
        try FileManager.default.copyItem(at: plan, to: directory.appendingPathComponent("plans/departure.plan"))
        try JSONSerialization.data(withJSONObject: XCTUnwrap(manifest["live_activities"])).write(to: directory.appendingPathComponent("plans/activities.json"))
        let app = ExactApp.shared, previous = app.assetRoot
        defer { app.assetRoot = previous; try? FileManager.default.removeItem(at: directory) }
        app.assetRoot = directory
        let session = ExactSession(app: app, label: "activity-state-cache", launchPlace: LaunchPlace(environment: ["EXACT_AGENT":"1"]))
        session.clock = 0
        defer { session.destroy() }
        let agent = Agent(session: session)
        func read(_ state: String?) -> [String: Any] {
            var address = ["entry":"departure", "placement":"minimal"]
            address["state"] = state
            return ActivityAgent.answer("state", request: ["activity":address], agent: agent)
        }
        XCTAssertNil(read(nil)["error"])
        XCTAssertNotNil(read("{}")["error"], "an empty supplied state must decode, even after a preview")
        XCTAssertNil(read("{\"station\":\"Palo Alto\",\"headsign\":\"San Francisco\",\"train\":1,\"at\":1787916000000,\"status\":\"On time\"}")["error"])
        XCTAssertNotNil(read("{\"station\":\"Palo Alto\",\"headsign\":\"San Francisco\",\"train\":true,\"at\":1787916000000,\"status\":\"On time\"}")["error"], "a Boolean is not the numeric state the cache holds")
    }

    func testGivenPropsSkipARefusingRefresh() throws {
        try caltrainArchive() // the entry's refresh is `nextTrains`
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
        try caltrainArchive()
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

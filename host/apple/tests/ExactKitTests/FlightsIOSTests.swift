#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1013.000 D4 on UIKit: a flying image moves in points from where its
/// leaver drew it to where it draws, the flight layer is above every ranked
/// sibling of its presentation root, and a flight from an overlay above the
/// routes into a route flies above that overlay.
final class FlightsIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }
    private func presenter() -> Presenter {
        let p = Presenter()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        return p
    }

    /// A photo whose thumbnail is `cover` in a 326×245 box opens into a
    /// `contain` 402×874 box (Signal Clone's viewer). Each end is exactly
    /// its own drawing, and between them the drawn rectangle is the straight
    /// mix of the two: never taller than both ends, as the mix of fractions
    /// was (377 pt mid-flight against 245 and 301).
    func testAFlyingImageMovesInPointsBetweenItsTwoDrawings() {
        let natural = CGSize(width: 1000, height: 750)
        let from = CGSize(width: 326, height: 245), to = CGSize(width: 402, height: 874)
        let start = Presenter.fitFraction(natural: natural, box: from, fit: "cover")
        let end = Presenter.fitFraction(natural: natural, box: to, fit: "contain")
        let a = CGRect(x: start.minX * from.width, y: start.minY * from.height, width: start.width * from.width, height: start.height * from.height)
        let b = CGRect(x: end.minX * to.width, y: end.minY * to.height, width: end.width * to.width, height: end.height * to.height)
        func close(_ r: CGRect, _ s: CGRect, _ what: String) {
            XCTAssertEqual(r.minX, s.minX, accuracy: 0.01, what); XCTAssertEqual(r.minY, s.minY, accuracy: 0.01, what)
            XCTAssertEqual(r.width, s.width, accuracy: 0.01, what); XCTAssertEqual(r.height, s.height, accuracy: 0.01, what)
        }
        close(Presenter.flightImage(from: from, fit: start, to: to, fit: end, progress: 0), a, "frame 0 is the leaver's drawing")
        close(Presenter.flightImage(from: from, fit: start, to: to, fit: end, progress: 1), b, "the last frame is the arriver's")
        for p in stride(from: CGFloat(0.1), through: 0.9, by: 0.1) {
            let r = Presenter.flightImage(from: from, fit: start, to: to, fit: end, progress: p)
            XCTAssertEqual(r.height, a.height + (b.height - a.height) * p, accuracy: 0.01, "height is linear at \(p)")
            XCTAssertEqual(r.width, a.width + (b.width - a.width) * p, accuracy: 0.01, "width is linear at \(p)")
            XCTAssertLessThanOrEqual(r.height, max(a.height, b.height) + 0.01, "no bulge at \(p)")
        }
        // A spring's overshoot carries the drawing past the end, as it does the box.
        let over = Presenter.flightImage(from: from, fit: start, to: to, fit: end, progress: 1.05)
        XCTAssertEqual(over.width, b.width + (b.width - a.width) * 0.05, accuracy: 0.01)
    }

    // Every node carries a testId, so none is drawn as a flat leaf of its
    // parent (FlatLeavesIOS): a flight needs views.
    /// Two views, the leaver a root child and the arriver's parent placed in
    /// `container`: the leaver flies into the arriver at progress 0.5.
    private func fly(_ p: Presenter, into container: (NodeView) -> Void) throws -> UIView {
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["testId": "n1"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "n2"], "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "n3"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0]]))
        container(try XCTUnwrap(p.views[3]))
        p.apply(wireBatch([
            ["op": "flight", "id": 4, "from": 2],
            ["op": "destroy", "id": 2],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "n4"], "style": ["background_color": [0, 0, 255, 255]]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "frame", "id": 4, "x": 20.0, "y": 500.0, "w": 200.0, "h": 150.0],
            ["op": "present", "id": 4, "property": "flight", "x": 0.5, "y": 0.0]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let arriver = try XCTUnwrap(p.views[4])
        // Not an image: it flies scaled in its clip (D4.4 as amended).
        let layer = try XCTUnwrap((arriver.superview as? FlightClip)?.superview as? FlightLayer, "the arriver flies")
        let shown = arriver.convert(arriver.bounds, to: nil)
        XCTAssertEqual(shown.minY, 300, accuracy: 0.5, "halfway between 100 and 500")
        XCTAssertEqual(shown.width, 300, accuracy: 0.5, "halfway between 400 and 200")
        return layer
    }

    /// A route's content holds a document-plane sibling just above rank ½;
    /// the flight layer, at depth 0, drew under it and the flight was never
    /// seen. It is ranked above every sibling, as a transition snapshot is.
    func testTheFlightLayerIsAboveEveryRankedSibling() throws {
        let p = presenter()
        let plane = UIView(frame: window.bounds)
        let layer = try fly(p) { _ in
            self.window.addSubview(plane)
            plane.setPaintForeground(aboveAuthored: false)
        }
        XCTAssertTrue(layer.superview === window, "in the window, the presentation root here")
        let others = window.subviews.filter { $0 !== layer }
        XCTAssertFalse(others.isEmpty)
        for v in others {
            XCTAssertGreaterThan(layer.layer.zPosition, v.layer.zPosition, "above \(type(of: v))")
        }
    }

    /// A photo closing from an overlay over the routes (the leaver, in the
    /// window's presentation) into a thumbnail in a route (the arriver, in a
    /// controller's view inside it) flies in the outer presentation, over
    /// the overlay's fading backdrop, not under it in the route's.
    func testAFlightFromAnEnclosingPresentationFliesInIt() throws {
        let p = presenter()
        let route = UIViewController()
        let layer = try fly(p) { parent in
            route.view.frame = self.window.bounds
            self.window.insertSubview(route.view, belowSubview: p.viewport)
            route.view.addSubview(parent)
        }
        XCTAssertTrue(layer.superview === window, "the leaver's presentation encloses the arriver's: it flies there")
        XCTAssertFalse(route.view.subviews.contains { $0 is FlightLayer })
    }

    /// The other way, from a route into the window's overlay: B's own
    /// presentation is the outer one, unchanged (D4 step 3).
    func testAFlightIntoTheOuterPresentationFliesInIt() throws {
        let p = presenter()
        let route = UIViewController()
        route.view.frame = window.bounds
        window.insertSubview(route.view, belowSubview: p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["testId": "n1"]], ["op": "create", "id": 2, "kind": "view", "props": ["testId": "n2"]], ["op": "create", "id": 3, "kind": "view", "props": ["testId": "n3"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0]]))
        let leaver = try XCTUnwrap(p.views[2])
        route.view.addSubview(leaver)
        p.apply(wireBatch([
            ["op": "flight", "id": 4, "from": 2], ["op": "destroy", "id": 2],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "n4"]], ["op": "children", "id": 3, "ids": [4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "present", "id": 4, "property": "flight", "x": 0.25, "y": 0.0]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let layer = try XCTUnwrap((p.views[4]?.superview as? FlightClip)?.superview as? FlightLayer)
        XCTAssertTrue(layer.superview === window)
    }
    private func png(_ root: URL, _ name: String, width: Int, height: Int) throws {
        let context = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(red: 0.9, green: 0.88, blue: 0.8, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(root.appendingPathComponent(name) as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try XCTUnwrap(context.makeImage()), nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
    }
    private func land(_ view: NodeView, _ source: String, _ loader: RasterLoader, _ resolver: AssetResolver) {
        view.loadGeneration += 1
        loader.load(view, source: source, resolver: resolver)
        let end = Date(timeIntervalSinceNow: 5)
        while view.raster == nil && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
    }

    /// A new image node's raster lands a turn or more after the commit that
    /// starts its flight, and the leaver is hidden in that commit: on a
    /// device the photo vanished for a frame or two at the start of the
    /// flight (Signal Clone, build 20). Until its own lands, the arriver
    /// draws the leaver's decoded image, in the same transaction; then its
    /// own, at its own size; a flight that ends early lets the leaver's go.
    func testAnArriverDrawsTheLeaversImageUntilItsOwnLands() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-flight-stand-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        try png(root, "thumb.png", width: 200, height: 150)
        try png(root, "full.png", width: 400, height: 100)
        let p = presenter(), loader = RasterLoader(), resolver = AssetResolver(root: root)
        defer { loader.shutdown(); withExtendedLifetime(resolver) {} }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["testId": "n1"]],
            ["op": "create", "id": 2, "kind": "image", "props": ["testId": "n2"], "style": ["object_fit": "cover"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "n3"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0]]))
        let leaver = try XCTUnwrap(p.views[2])
        land(leaver, "thumb.png", loader, resolver)
        let thumb = try XCTUnwrap(leaver.raster?.image.image, "the leaver's image decoded")
        p.apply(wireBatch([
            ["op": "flight", "id": 4, "from": 2], ["op": "destroy", "id": 2],
            ["op": "create", "id": 4, "kind": "image", "props": ["testId": "n4"], "style": ["object_fit": "contain"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "present", "id": 4, "property": "flight", "x": 0.0, "y": 0.0]]))
        let arriver = try XCTUnwrap(p.views[4])
        XCTAssertNil(arriver.raster, "no raster of its own yet (no session loads it here)")
        XCTAssertTrue(arriver.superview is FlightLayer, "it flies")
        let drawn = try XCTUnwrap(arriver.imageLayer, "it draws in the commit that hid the leaver")
        XCTAssertTrue((drawn.contents as AnyObject?) === thumb, "the leaver's pixels")
        p.apply(wireBatch([["op": "present", "id": 4, "property": "flight", "x": 0.5, "y": 0.0]]))
        XCTAssertTrue((arriver.imageLayer?.contents as AnyObject?) === thumb, "still the leaver's mid-flight")
        // Its own lands (a 4:1 image, contained in 400x800: 400x100 at y 350):
        // from the next frame it draws that, where that is drawn.
        land(arriver, "full.png", loader, resolver)
        let full = try XCTUnwrap(arriver.raster?.image.image)
        p.apply(wireBatch([["op": "present", "id": 4, "property": "flight", "x": 1.0, "y": 0.0]]))
        XCTAssertTrue((arriver.imageLayer?.contents as AnyObject?) === full, "its own pixels once they land")
        XCTAssertNil(arriver.flightLook?.stand, "the leaver's lease let go")
        XCTAssertEqual(arriver.imageLayer?.frame.height ?? 0, 100, accuracy: 0.5, "at its own aspect")
    }

    /// A flight whose arriver is destroyed mid-flight drops the look that
    /// holds the leaver's lease, even while something else keeps the view.
    func testAFlightThatEndsEarlyLetsTheLeaversImageGo() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-flight-stand-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        try png(root, "thumb.png", width: 200, height: 150)
        let p = presenter(), loader = RasterLoader(), resolver = AssetResolver(root: root)
        defer { loader.shutdown(); withExtendedLifetime(resolver) {} }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["testId": "n1"]],
            ["op": "create", "id": 2, "kind": "image", "props": ["testId": "n2"], "style": ["object_fit": "cover"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "n3"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0]]))
        land(try XCTUnwrap(p.views[2]), "thumb.png", loader, resolver)
        p.apply(wireBatch([
            ["op": "flight", "id": 4, "from": 2], ["op": "destroy", "id": 2],
            ["op": "create", "id": 4, "kind": "image", "props": ["testId": "n4"], "style": ["object_fit": "contain"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "present", "id": 4, "property": "flight", "x": 0.3, "y": 0.0]]))
        let arriver = try XCTUnwrap(p.views[4])
        XCTAssertNotNil(arriver.flightLook?.stand, "drawing the leaver's image")
        p.apply(wireBatch([["op": "children", "id": 3, "ids": []], ["op": "destroy", "id": 4]]))
        XCTAssertNil(arriver.flightLook, "the look, and the lease in it, went with the flight")
    }
}
#endif

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
        let layer = try XCTUnwrap(arriver.superview as? FlightLayer, "the arriver flies")
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
        let layer = try XCTUnwrap(p.views[4]?.superview as? FlightLayer)
        XCTAssertTrue(layer.superview === window)
    }
}
#endif

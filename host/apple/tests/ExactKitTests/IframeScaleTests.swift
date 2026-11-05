#if os(macOS)
import AppKit
import WebKit
import XCTest
@testable import ExactKit

/// A scaled iframe hears its events where it is drawn: the web view converts
/// a window point by AppKit's geometry, which a layer's transform does not
/// change, so the scale is the geometry's (`FrameScaleMac.swift`).
final class IframeScaleTests: XCTestCase {
    private var session: ExactSession?
    private var window: NSWindow?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.close()
        window = nil
        super.tearDown()
    }

    private func until(_ seconds: Double, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
    }

    private func frame(scale: CGFloat, origin: [Any]? = nil) throws -> NodeView {
        guard let arm = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let session = ExactApp.shared.makeSession(label: "frame-scale")
        self.session = session
        session.webviews.moduleLocation = { arm }
        XCTAssertNil(session.boot(size: CGSize(width: 400, height: 300)).error)
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.orderFrontRegardless()
        self.window = window
        waitForActivation(session)
        let style: [String: Any] = origin.map { ["transform_origin": $0] } ?? [:]
        session.presenter.apply(wireBatch([
            ["op": "create", "id": 7101, "kind": "iframe", "props": [:], "style": style],
            ["op": "roots", "ids": [7101]],
            ["op": "frame", "id": 7101, "x": 40.0, "y": 20.0, "w": 200.0, "h": 100.0],
        ]))
        let node = try XCTUnwrap(session.presenter.views[7101])
        until(5) { node.web != nil }
        XCTAssertNotNil(node.web, "the frame's web view is made")
        node.scale = scale
        node.applyTransform()
        return node
    }

    /// Where a point of the box as it is drawn (in the node's coordinates, where
    /// its layer draws no scale) lands on the page.
    private func pageLocal(_ node: NodeView, at drawn: NSPoint) throws -> NSPoint {
        try XCTUnwrap(node.web).convert(node.convert(drawn, to: nil), from: nil)
    }

    /// Scaled by half about its centre (CSS's default origin), the 200 × 100
    /// box is drawn as 100 × 50 from (50, 25); the page keeps the box's size,
    /// and a point a quarter in from the drawn corner is a quarter in on the page.
    func testAScaledFramesPageHearsPointsWhereItIsDrawn() throws {
        let node = try frame(scale: 0.5)
        let web = try XCTUnwrap(node.web)
        XCTAssertEqual(web.frame.size, CGSize(width: 200, height: 100), "the page lays out at the box")
        let local = try pageLocal(node, at: NSPoint(x: 75, y: 37.5))
        XCTAssertEqual(local.x, 50, accuracy: 0.01)
        XCTAssertEqual(local.y, 25, accuracy: 0.01)
        let drawn = web.convert(web.bounds, to: node)
        XCTAssertEqual(drawn.width, 100, accuracy: 0.01, "drawn at half the box")
        XCTAssertEqual(drawn.minX, 50, accuracy: 0.01)
        XCTAssertTrue(CATransform3DIsIdentity(node.layer?.transform ?? CATransform3DIdentity), "the layer does not scale it again")
        let hit = window?.contentView?.hitTest(window!.contentView!.convert(node.convert(NSPoint(x: 75, y: 37.5), to: nil), from: nil))
        XCTAssertTrue(hit.map { $0 === web || $0.isDescendant(of: web) } ?? false, "the drawn page takes the hit")
    }

    /// From its top left, as the Browser's floating player scales a page into
    /// its card; and back at scale 1 the page fills the box again.
    func testAFrameScaledFromItsCornerAndBack() throws {
        let node = try frame(scale: 0.25, origin: [0, 0])
        var local = try pageLocal(node, at: NSPoint(x: 10, y: 5))
        XCTAssertEqual(local.x, 40, accuracy: 0.01)
        XCTAssertEqual(local.y, 20, accuracy: 0.01)
        node.scale = 1
        node.applyTransform()
        local = try pageLocal(node, at: NSPoint(x: 10, y: 5))
        XCTAssertEqual(local.x, 10, accuracy: 0.01)
        XCTAssertEqual(local.y, 5, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(node.web).frame.size, CGSize(width: 200, height: 100))
    }
}
#endif

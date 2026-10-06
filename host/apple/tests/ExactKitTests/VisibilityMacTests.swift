#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// CSS `visibility` on macOS: a hidden box keeps its geometry and paints
/// nothing of its own, and it is not a hit. A descendant whose own style is
/// `visible` paints and is hit. One that is `hidden` does not. Wire batches
/// send each node's style as the host will, so the inheriting child carries
/// `visibility: hidden` itself.
final class VisibilityMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 200, height: 200)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        return p
    }

    private func rgba(_ rep: NSBitmapImageRep, _ x: CGFloat, _ y: CGFloat) -> [CGFloat] {
        let scale = CGFloat(rep.pixelsWide) / rep.size.width
        let c = rep.colorAt(x: Int(x * scale), y: Int(y * scale))!.usingColorSpace(.sRGB)!
        return [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent]
    }

    func testAVisibleDescendantOfAHiddenBoxPaintsAndIsHit() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255.0, 255.0, 255.0, 255.0]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [255.0, 0.0, 0.0, 255.0], "visibility": "hidden"]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [0.0, 255.0, 0.0, 255.0], "visibility": "visible"]],
            ["op": "create", "id": 4, "kind": "view", "style": ["background_color": [0.0, 0.0, 255.0, 255.0], "visibility": "hidden"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 120.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 10.0, "y": 10.0, "w": 40.0, "h": 30.0],
            ["op": "frame", "id": 4, "x": 70.0, "y": 10.0, "w": 40.0, "h": 30.0],
        ]))
        let page = try XCTUnwrap(p.views[1])
        let parent = try XCTUnwrap(p.views[2])
        let shown = try XCTUnwrap(p.views[3])
        let kept = try XCTUnwrap(p.views[4])
        XCTAssertFalse(parent.isHidden, "visibility keeps the box in the tree")
        XCTAssertGreaterThan(parent.frame.width, 0)
        XCTAssertGreaterThan(parent.frame.height, 0)
        XCTAssertGreaterThan(shown.frame.width, 0)
        XCTAssertGreaterThan(kept.frame.width, 0)
        let rep = try XCTUnwrap(Capture.picture(of: page))
        func onPage(_ view: NodeView, _ point: NSPoint) -> NSPoint { page.convert(point, from: view) }
        let shownAt = onPage(shown, NSPoint(x: shown.bounds.midX, y: shown.bounds.midY))
        let keptAt = onPage(kept, NSPoint(x: kept.bounds.midX, y: kept.bounds.midY))
        let ownAt = onPage(parent, NSPoint(x: 20, y: 60))
        let green = rgba(rep, shownAt.x, shownAt.y)
        XCTAssert(green[1] > 0.9 && green[0] < 0.1 && green[2] < 0.1, "the visible child paints: \(green)")
        let blue = rgba(rep, keptAt.x, keptAt.y)
        XCTAssertFalse(blue[2] > 0.8 && blue[0] < 0.2 && blue[1] < 0.2, "the hidden child paints nothing: \(blue)")
        let red = rgba(rep, ownAt.x, ownAt.y)
        XCTAssertFalse(red[0] > 0.8 && red[1] < 0.2 && red[2] < 0.2, "the hidden box paints nothing of its own: \(red)")
        func hit(_ view: NodeView, _ point: NSPoint) -> NSView? {
            page.hitTest(page.superview!.convert(point, from: view))
        }
        XCTAssertTrue(hit(shown, NSPoint(x: shown.bounds.midX, y: shown.bounds.midY)) === shown)
        let keptHit = hit(kept, NSPoint(x: kept.bounds.midX, y: kept.bounds.midY))
        XCTAssertFalse(keptHit === kept)
        XCTAssertFalse(keptHit === parent)
        let ownHit = hit(parent, NSPoint(x: 20, y: 60))
        XCTAssertFalse(ownHit === parent)
    }

    /// What e28279b3b left: a box hidden while focused gives the focus up
    /// (on the next turn), and hidden text is no link or target while a
    /// visible run inside a hidden link still follows it, as a click bubbles.
    func testAHiddenBoxGivesUpFocusAndHiddenTextIsNoTarget() throws {
        let p = presenter()
        let window = try XCTUnwrap(self.window)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["tabIndex": "0"], "handlers": ["press"], "style": [:]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
        ]))
        let box = try XCTUnwrap(p.views[1])
        XCTAssertTrue(window.makeFirstResponder(box))
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["visibility": "hidden"]]]))
        XCTAssertTrue(window.firstResponder === box, "not inside the batch")
        let deadline = Date().addingTimeInterval(10)
        while window.firstResponder === box, Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
        XCTAssertFalse(window.firstResponder === box, "hidden, it gives up the focus it had")

        let session = ExactApp.shared.makeSession(label: "hidden-run") // a text engine, to lay runs out
        defer { session.destroy() }
        let q = session.presenter
        q.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "x"], "style": ["font_size": 16]],
            ["op": "roots", "ids": [3]],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
        ]))
        let text = try XCTUnwrap(q.views[3])
        var hidden = InlineStyle(), shown = InlineStyle()
        try hidden.set("visibility", .string("hidden"))
        try shown.set("visibility", .string("visible"))
        text.props = [:]
        q.applyParagraph(3, [
            InlineText(id: 10, parent: 3, props: ["href": "https://example.com/hidden"], style: hidden, handlers: [], paints: false),
            InlineText(id: 11, parent: 10, props: ["text": "Hidden "], style: hidden, handlers: [], paints: true),
            InlineText(id: 12, parent: 10, props: ["text": "Shown"], style: shown, handlers: [], paints: true),
        ])
        let rect = { (id: UInt32) throws -> CGRect in try XCTUnwrap(text.inlineRects(try XCTUnwrap(text.inlineText.first { $0.id == id })).first) }
        let hiddenAt = try rect(11), shownAt = try rect(12)
        XCTAssertNil(text.inlineTarget(at: CGPoint(x: hiddenAt.midX, y: hiddenAt.midY)), "hidden text is no target")
        XCTAssertNil(text.inlineLink(at: CGPoint(x: hiddenAt.midX, y: hiddenAt.midY)), "nor its hidden link followed there")
        XCTAssertEqual(text.inlineLink(at: CGPoint(x: shownAt.midX, y: shownAt.midY)), "https://example.com/hidden",
                       "a visible run's click reaches the hidden link holding it")
        XCTAssertEqual(text.inlineActivationTarget(at: CGPoint(x: shownAt.midX, y: shownAt.midY))?.id, 10)
        XCTAssertTrue(q.inlineEnabled(10), "and activates it")
    }
}
#endif

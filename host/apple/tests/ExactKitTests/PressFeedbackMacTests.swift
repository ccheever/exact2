#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// Press feedback on AppKit (LLP 1061 D2): the mouse button held down inside
/// a pressable node shows its `press_scale`; dragging out releases the
/// feedback and dragging back presses again, as UIKit's finger does; the
/// click is decided as before. And every transform turns about the node's
/// `transform-origin` (D6).
final class PressFeedbackMacTests: XCTestCase {
    private var window: NSWindow!

    override func tearDown() { window?.close(); window = nil }

    private func fixture(style: NodeStyle = ["press_scale": 0.97], kind: String = "view") -> (Presenter, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        let node = NodeView(id: 1, kind: kind, presenter: p)
        node.handlers = ["press"]
        node.frame = NSRect(x: 50, y: 50, width: 200, height: 100)
        p.root.addSubview(node); p.views[1] = node
        node.applyStyle(style)
        return (p, node)
    }

    private func event(_ node: NodeView, _ type: NSEvent.EventType, _ x: CGFloat, _ y: CGFloat) -> NSEvent {
        NSEvent.mouseEvent(with: type, location: node.convert(NSPoint(x: x, y: y), to: nil), modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
    }

    func testTheButtonHeldInsidePressesAndADragOutReleases() throws {
        let (p, v) = fixture()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        v.mouseDown(with: event(v, .leftMouseDown, 100, 50))
        XCTAssertTrue(v.pressed)
        XCTAssertEqual(v.press.to, 0.97)
        v.mouseDragged(with: event(v, .leftMouseDragged, 260, 50))
        XCTAssertTrue(v.pressed, "still the press's: a return inside can take it")
        XCTAssertEqual(v.press.to, 1, "outside, the feedback lets go")
        v.mouseDragged(with: event(v, .leftMouseDragged, 190, 60))
        XCTAssertEqual(v.press.to, 0.97, "back inside, it presses again")
        v.press.start -= 1 // released a moment later, the press long on screen
        v.mouseUp(with: event(v, .leftMouseUp, 190, 60))
        XCTAssertFalse(v.pressed)
        XCTAssertEqual(v.press.to, 1)
        XCTAssertEqual(pressed, [1], "the click is the tap it was")
    }

    /// A `text` with its own `press` is clicked, as a `<span onClick>` is,
    /// rather than selected (spreadsheet F12).
    func testATextWithAPressIsClicked() throws {
        let (p, v) = fixture(style: [:], kind: "text")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        v.mouseDown(with: event(v, .leftMouseDown, 100, 50))
        v.mouseUp(with: event(v, .leftMouseUp, 100, 50))
        XCTAssertEqual(pressed, [1])
    }

    func testReducedMotionKeepsThePress() throws {
        let previous = DisplayPreferences.agent
        defer { DisplayPreferences.agent = previous }
        DisplayPreferences.agent = (reducedMotion: true, reducedTransparency: false)
        let (_, v) = fixture()
        v.pressed = true
        XCTAssertEqual(v.press.to, 0.97)
        v.press.start -= 1
        v.applyTransform()
        XCTAssertEqual(try XCTUnwrap(v.layer?.affineTransform().a), 0.97, accuracy: 1e-9)
        v.pressed = false
        XCTAssertEqual(v.press.to, 1)
    }

    func testANodeWithoutTheRowGivesNoFeedback() {
        let (_, v) = fixture(style: [:])
        v.pressed = true
        XCTAssertTrue(v.press.idle)
        XCTAssertEqual(v.layer?.affineTransform(), .identity)
    }

    func testThePressFoldsIntoTheEnginesScaleAboutTheOrigin() throws {
        let (p, v) = fixture(style: ["press_scale": 0.97, "transform_origin": [["pct": 0], ["pct": 0]]])
        v.press = PressFeedback(from: 0.97, to: 0.97, start: 0)
        p.apply(wireBatch([["op": "present", "id": 1, "property": "scale", "x": 2.0]]))
        let t = try XCTUnwrap(v.layer?.affineTransform())
        XCTAssertEqual(t.a, 2 * 0.97, accuracy: 1e-9, "the engine's write keeps the press")
        XCTAssertEqual(t.tx, 0, accuracy: 1e-9, "about the top-left corner, which stays put")
        XCTAssertEqual(t.ty, 0, accuracy: 1e-9)
    }

    func testTheTransformTurnsAboutTheTransformOrigin() throws {
        let (p, v) = fixture(style: [:])
        p.apply(wireBatch([["op": "present", "id": 1, "property": "scale", "x": 0.5]]))
        // Unset: the centre (100, 50) stays put.
        XCTAssertEqual(v.layer?.affineTransform(), CGAffineTransform(a: 0.5, b: 0, c: 0, d: 0.5, tx: 50, ty: 25))
        // `right 10px`: (200, 10) stays put; a style change re-applies it.
        v.applyStyle(["transform_origin": [["pct": 100], 10]])
        XCTAssertEqual(v.layer?.affineTransform(), CGAffineTransform(a: 0.5, b: 0, c: 0, d: 0.5, tx: 100, ty: 5))
    }

    /// The layer's space is the flipped view's: `top` is the top on screen.
    /// A reference box beside it spans the band's top half; the scaled box,
    /// about its top-left corner, must span the same layer rows, whichever
    /// way the root's layer counts them.
    func testTheOriginIsWhereTheScreenSaysItIs() throws {
        let (p, v) = fixture(style: ["transform_origin": [["pct": 0], ["pct": 0]]])
        p.root.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        p.root.wantsLayer = true
        let reference = NodeView(id: 2, kind: "view", presenter: p)
        reference.frame = NSRect(x: 260, y: 50, width: 100, height: 50)
        p.root.addSubview(reference); p.views[2] = reference
        reference.wantsLayer = true
        p.apply(wireBatch([["op": "present", "id": 1, "property": "scale", "x": 0.5]]))
        let (scaled, top) = (try XCTUnwrap(v.layer?.frame), try XCTUnwrap(reference.layer?.frame))
        XCTAssertEqual(scaled.minY, top.minY, accuracy: 1e-9)
        XCTAssertEqual(scaled.height, top.height, accuracy: 1e-9)
        XCTAssertEqual(scaled.minX, 50, accuracy: 1e-9)
    }
}
#endif

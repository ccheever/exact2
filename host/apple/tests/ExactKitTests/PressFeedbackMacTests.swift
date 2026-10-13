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

    /// The alpha of a wash's mask at a point of the node (0 where the node
    /// has no ink, so nothing is tinted there).
    private func ink(_ node: NodeView, at point: CGPoint) throws -> CGFloat {
        let wash = try XCTUnwrap(node.layer?.sublayers?.first { $0.name == "exact.pressWash" })
        let mask = try XCTUnwrap(wash.mask)
        XCTAssertEqual(mask.frame, wash.bounds, "the picture fills the wash")
        var pixel = [UInt8](repeating: 0, count: 4)
        let context = try XCTUnwrap(CGContext(data: &pixel, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.translateBy(x: -point.x, y: -point.y)
        mask.render(in: context)
        return CGFloat(pixel[3]) / 255
    }

    /// LLP 1115.000 D6 on the Mac: a pressable that declares no feedback
    /// shows AppKit's pressed effect while the mouse is down inside it, as a
    /// borderless `NSButton` does: a wash over its own ink, darker in a light
    /// appearance and lighter in a dark one, on at once and off at once, and
    /// no scale. (It showed nothing: "macOS: AppKit's custom views show none".)
    func testAPressableThatDeclaresNoFeedbackShowsThePlatformsPressedLook() throws {
        let (p, v) = fixture(style: ["background_color": [240, 240, 245, 255]])
        window.appearance = NSAppearance(named: .aqua)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertTrue(v.platformPressTints)
        XCTAssertFalse(v.pressTinted)
        v.mouseDown(with: event(v, .leftMouseDown, 100, 50))
        XCTAssertTrue(v.pressed)
        XCTAssertTrue(v.pressTinted, "the mouse is down inside it")
        XCTAssertTrue(v.press.idle, "the platform's look is no scale")
        XCTAssertEqual(v.layer?.affineTransform(), .identity)
        let wash = try XCTUnwrap(v.layer?.sublayers?.first { $0.name == "exact.pressWash" })
        XCTAssertEqual(wash.backgroundColor?.components, [0, 0.16], "light: black, the system's pressed effect at white")
        XCTAssertEqual(wash.frame, v.bounds)
        XCTAssertEqual(wash.zPosition, 1, "over the node's own views")
        XCTAssertGreaterThan(try ink(v, at: CGPoint(x: 100, y: 50)), 0.99, "masked by the node's own picture: its box is ink")
        v.mouseDragged(with: event(v, .leftMouseDragged, 260, 50))
        XCTAssertTrue(v.pressed)
        XCTAssertFalse(v.pressTinted, "outside, the look lets go, as a button's highlight does")
        v.mouseDragged(with: event(v, .leftMouseDragged, 190, 60))
        XCTAssertTrue(v.pressTinted, "back inside, it shows again")
        v.mouseUp(with: event(v, .leftMouseUp, 190, 60))
        XCTAssertFalse(v.pressTinted, "off at once on release")
        XCTAssertEqual(pressed, [1], "the click is the tap it was")
        // A dark appearance lightens: white at 0.18, the effect at black.
        window.appearance = NSAppearance(named: .darkAqua)
        v.pressed = true
        XCTAssertEqual(v.layer?.sublayers?.first { $0.name == "exact.pressWash" }?.backgroundColor?.components, [1, 0.18])
        v.pressed = false
        window.appearance = nil
        // Where the node paints nothing, nothing is tinted: no box around a text button's glyphs.
        let (_, bare) = fixture(style: [:])
        bare.pressed = true
        XCTAssertTrue(bare.pressTinted)
        XCTAssertEqual(try ink(bare, at: CGPoint(x: 100, y: 50)), 0, "an unpainted pressable has no ink to tint")
        bare.pressed = false
    }

    /// A default yields only to the author's version of the same state, and
    /// a native control and a list's row keep their own.
    func testThePlatformsPressedLookYieldsToTheAuthorsAndIsNotForARowOrANativeControl() throws {
        // Authored press feedback replaces it, whatever its value.
        for scale in [0.97, 1.0] {
            let (_, v) = fixture(style: ["press_scale": .number(scale)])
            XCTAssertFalse(v.platformPressTints)
            v.pressed = true
            XCTAssertFalse(v.pressTinted, "press_scale \(scale): only the author's feedback")
            XCTAssertEqual(v.press.to, CGFloat(scale))
            v.pressed = false
            window.close()
        }
        let (p, v) = fixture(style: [:])
        // A `hover` handler only observes: it replaces nothing.
        v.handlers = ["press", "hover"]
        XCTAssertTrue(v.platformPressTints)
        // The author paints the pressed state from the pointer's own events.
        v.handlers = ["press", "pointerdown"]
        XCTAssertFalse(v.platformPressTints)
        v.handlers = ["press", "pointerup"]
        XCTAssertFalse(v.platformPressTints)
        // Not pressable: nothing to show.
        v.handlers = ["hover"]
        XCTAssertFalse(v.platformPressTints)
        v.handlers = ["press"]
        // A row of a list, the list's child or a section's in it: its press is the cell's (D16).
        let list = NodeView(id: 2, kind: "list", presenter: p), section = NodeView(id: 3, kind: "view", presenter: p)
        section.props["semanticTag"] = "section"
        list.frame = NSRect(x: 0, y: 200, width: 300, height: 150); section.frame = list.bounds
        p.root.addSubview(list); p.views[2] = list; p.views[3] = section
        list.container.addSubview(v)
        XCTAssertTrue(v.isListRow)
        XCTAssertFalse(v.platformPressTints)
        v.pressed = true
        XCTAssertFalse(v.pressTinted, "a list's row is not tinted")
        v.pressed = false
        list.container.addSubview(section); section.container.addSubview(v)
        XCTAssertTrue(v.isListRow, "a section's row is the list's too")
        XCTAssertFalse(v.platformPressTints)
        // An item of a list that scrolls sideways is a carousel's card, not a row (as on the web host).
        list.container.addSubview(v)
        let sideways = try XCTUnwrap(CollectionSnapshot(["view": 2, "revision": 1, "scrollSequence": 1, "totalExtent": 600.0, "axis": "x", "rows": [[String: Any]]()]))
        p.collections.entries[2] = CollectionHost.Entry(sideways)
        XCTAssertFalse(v.isListRow)
        XCTAssertTrue(v.platformPressTints, "a carousel's card shows the press")
        p.collections.entries[2] = nil
        XCTAssertTrue(v.isListRow)
        // A native control keeps its own feedback: no wash is laid over it.
        let native = NodeView(id: 4, kind: "control", presenter: p)
        native.props["type"] = "button"; native.handlers = ["press"]
        native.frame = NSRect(x: 0, y: 0, width: 80, height: 24)
        p.root.addSubview(native); p.views[4] = native
        XCTAssertFalse(native.platformPressTints)
        // A scrim covering the window shows nothing, as on iOS.
        let scrim = NodeView(id: 5, kind: "view", presenter: p)
        scrim.handlers = ["press"]
        p.root.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        scrim.frame = p.root.bounds
        p.root.addSubview(scrim); p.views[5] = scrim
        XCTAssertFalse(scrim.platformPressTints)
    }

    /// A disabled custom pressable shows as a disabled control does, at half
    /// its opacity, and takes no press; a native control and a list's row
    /// are not dimmed by this.
    func testADisabledCustomPressableDimsAndTakesNoPress() throws {
        let (p, v) = fixture(style: [:])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertEqual(v.alphaValue, 1)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"]]]))
        XCTAssertEqual(v.alphaValue, 0.5, "as AppKit draws a disabled borderless button's content")
        v.mouseDown(with: event(v, .leftMouseDown, 100, 50))
        v.mouseUp(with: event(v, .leftMouseUp, 100, 50))
        XCTAssertEqual(pressed, [], "and it takes no press")
        XCTAssertFalse(v.pressTinted)
        // The author's opacity is multiplied in, as it arrives and as it changes.
        p.apply(wireBatch([["op": "present", "id": 1, "property": "opacity", "x": 0.8]]))
        XCTAssertEqual(v.alphaValue, 0.4, accuracy: 1e-9)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "false"]]]))
        XCTAssertEqual(v.alphaValue, 0.8, accuracy: 1e-9, "enabled again, its own opacity")
        // A box that is not pressable has no disabled state (`disabled` means nothing on a box).
        v.handlers = []
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"]]]))
        XCTAssertEqual(v.alphaValue, 0.8, accuracy: 1e-9)
        v.handlers = ["press"]
        XCTAssertEqual(v.alphaValue, 0.4, accuracy: 1e-9, "a handler makes it one")
        // A list's row keeps its look for the cell (D16): dimmed nowhere here.
        let list = NodeView(id: 2, kind: "list", presenter: p)
        list.frame = NSRect(x: 0, y: 200, width: 300, height: 150)
        p.root.addSubview(list); p.views[2] = list
        list.container.addSubview(v)
        XCTAssertEqual(v.alphaValue, 0.8, accuracy: 1e-9, "mounted as a row, it is the list's")
    }

    /// One whose author styles its own disabled state (Contract's
    /// `disabledStyled`, LLP 1115.000 D6) keeps the author's look, and still
    /// takes no press.
    func testADisabledPressableItsAuthorStylesIsNotDimmed() throws {
        let (p, v) = fixture(style: [:])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true", "disabledStyled": "true"]]]))
        XCTAssertEqual(v.alphaValue, 1, "the author's look, not the dim")
        v.mouseDown(with: event(v, .leftMouseDown, 100, 50))
        v.mouseUp(with: event(v, .leftMouseUp, 100, 50))
        XCTAssertEqual(pressed, [], "and it takes no press")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"], "clear": ["disabledStyled"]]]))
        XCTAssertEqual(v.alphaValue, 0.5, "without the mark, the dim")
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

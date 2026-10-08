#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1069.011 on AppKit: a `Control` of type `button` is AppKit's own
/// `NSButton` with its row's look and the node's face (D2, D5); its action is
/// a custom button's click, once (D4); it owns the key view (LLP 1104 D6); it is the
/// one accessibility element; a glass look's button is isolated in a glass
/// group (D9).
final class NativeButtonsMacTests: XCTestCase {
    private var window: NSWindow!

    override func tearDown() { window?.close(); window = nil }

    private func presenter(_ ops: [[String: Any]], faces: [UInt32: ButtonFace]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch(ops))
        return p
    }
    private func face(_ title: String?, macos: String = "push", style: String = "bordered") -> ButtonFace {
        var f = ButtonFace()
        f.title = title; f.macos = macos; f.style = style
        return f
    }
    private func native(_ id: Int, _ props: [String: String] = [:], handlers: [String] = ["press"], x: Double = 0) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control", "props": ["type": "button", "accessibilityRole": "button"].merging(props) { $1 },
          "handlers": handlers, "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": x, "y": 0.0, "w": 120.0, "h": 24.0]]
    }
    private func box(_ id: Int, _ props: [String: String] = [:], handlers: [String] = []) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "view", "props": props, "handlers": handlers, "style": ["text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0]]
    }

    func testItIsAppKitsButtonPressingOnce() throws {
        let p = presenter(box(1, handlers: ["press"]) + native(2, ["testId": "go"]) + native(3, handlers: []) + native(4, ["disabled": "true"])
                          + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Go", macos: "push-accent", style: "filled"), 3: face("Up"), 4: face("Off")])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let go = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac)
        XCTAssertEqual(go.title, "Go")
        XCTAssertEqual(go.drawn, "push-accent")
        XCTAssertNotNil(go.bezelColor)
        XCTAssertTrue(go.acceptsFirstResponder, "the native control owns focus")
        XCTAssertEqual(go.accessibilityLabel(), "Go")
        XCTAssertEqual(go.accessibilityIdentifier(), "go")
        XCTAssertEqual(p.views[2]?.accessibleName, "Go", "the agent's name for it is its title")
        go.performClick(nil)
        XCTAssertEqual(pressed, [2])
        try XCTUnwrap(p.controls.controls[3] as? NativeButtonMac).performClick(nil)
        XCTAssertEqual(pressed, [2, 1], "no handler of its own: the nearest ancestor's")
        XCTAssertFalse(try XCTUnwrap(p.controls.controls[4]).isEnabled)
        XCTAssertNil(p.controls.activate(try XCTUnwrap(p.views[2])), "the agent clicks it as a person does")
        XCTAssertEqual(p.controls.observation(try XCTUnwrap(p.views[2]))?["view"] as? String, "NSButton")
        XCTAssertFalse(try XCTUnwrap(p.views[2]).acceptsFirstResponder, "only the native control is in the key loop")
    }

    func testItFocusesAsAClickDoesAndPressesOnlyAnAncestorItIsIn() throws {
        let p = presenter(box(1) + native(2, handlers: ["focus"]) + box(4, handlers: ["press"]) + native(3, handlers: [], x: 320) + native(5, handlers: [])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 4, "ids": [3, 5]], ["op": "roots", "ids": [1, 4]]],
                          faces: [2: face("Mark"), 3: face("Out"), 5: face("In")])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac).performClick(nil)
        XCTAssertTrue(window.firstResponder === p.controls.controls[2], "with no press anywhere, the clicked node still takes the focus")
        XCTAssertEqual(pressed, [])
        try XCTUnwrap(p.controls.controls[5] as? NativeButtonMac).performClick(nil)
        XCTAssertEqual(pressed, [4], "inside its ancestor: the ancestor's press")
        try XCTUnwrap(p.controls.controls[3] as? NativeButtonMac).performClick(nil)
        XCTAssertEqual(pressed, [4], "outside its ancestor's box: no press, as on iOS")
    }

    /// #179: a custom button's ring is AppKit's exterior one around its own
    /// box, which AppKit draws only for non-empty mask bounds.
    func testCustomButtonExposesItsFocusMaskToAppKit() throws {
        let p = presenter(box(1, handlers: ["press"]) + box(2)
                          + [["op": "roots", "ids": [1, 2]]], faces: [:])
        let button = try XCTUnwrap(p.views[1]), plain = try XCTUnwrap(p.views[2])
        XCTAssertTrue(window.makeFirstResponder(button))
        XCTAssertEqual(button.focusRingType, .exterior)
        XCTAssertFalse(button.focusRingMaskBounds.isEmpty, "AppKit skips a mask with empty bounds")
        XCTAssertEqual(button.focusRingMaskBounds, NSRect(x: 0, y: 0, width: 300, height: 40))
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 20.0, "y": 10.0, "w": 24.0, "h": 24.0]]))
        XCTAssertEqual(button.focusRingMaskBounds, NSRect(x: 0, y: 0, width: 24, height: 24), "mask coordinates stay local after resize")
        XCTAssertTrue(plain.focusRingMaskBounds.isEmpty, "noninteractive boxes have no focus mask")
    }

    /// Chrome's `:focus-visible`: a click focuses a custom button without
    /// its ring; a key used there, or a Tab to the next, shows it.
    func testAClickFocusesWithoutTheRingAndTheKeyboardShowsIt() throws {
        let p = presenter(box(1, handlers: ["press"]) + box(2, handlers: ["press"])
                          + [["op": "frame", "id": 2, "x": 0.0, "y": 50.0, "w": 300.0, "h": 40.0], ["op": "roots", "ids": [1, 2]]], faces: [:])
        let first = try XCTUnwrap(p.views[1]), second = try XCTUnwrap(p.views[2])
        p.flushKeyViewLoop()
        let at = first.convert(NSPoint(x: 10, y: 10), to: nil)
        let down = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: at, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        first.mouseDown(with: down)
        XCTAssertTrue(window.firstResponder === first, "a click focuses it")
        XCTAssertTrue(first.focusRingMaskBounds.isEmpty, "a click's focus is not visible")
        let key = { (chars: String, code: UInt16) in
            try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: self.window.windowNumber, context: nil, characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code))
        }
        XCTAssertFalse(p.routeKey(try key(" ", 49), focused: true, in: window))
        XCTAssertEqual(first.focusRingMaskBounds, first.bounds, "a key used at the focus shows its ring")
        let tab = try key("\t", 48)
        XCTAssertFalse(p.routeKey(tab, focused: true, in: window))
        first.keyDown(with: tab)
        XCTAssertTrue(window.firstResponder === second, "Tab moves on")
        XCTAssertEqual(second.focusRingMaskBounds, second.bounds, "a Tab's focus is visible")
        XCTAssertTrue(first.focusRingMaskBounds.isEmpty, "the ring left with the focus")
    }

    func testAGlassButtonIsIsolatedInItsGroup() throws {
        guard #available(macOS 26.0, *) else { throw XCTSkip("Liquid Glass is macOS 26") }
        let p = presenter(box(1, ["glassGroup": "24"]) + native(2) + native(3, x: 130)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Lock", macos: "glass", style: "glass"), 3: face("Fade", macos: "glass", style: "glass")])
        let fade = try XCTUnwrap(p.controls.controls[3] as? NativeButtonMac), node = try XCTUnwrap(p.views[3])
        XCTAssertTrue(fade.isGlass)
        XCTAssertNil(node.glassIsolation)
        p.apply(wireBatch([["op": "present", "id": 3, "property": "opacity", "x": 0.3, "y": 0.0, "w": 0.0, "h": 0.0]]))
        let isolation = try XCTUnwrap(node.glassIsolation, "fading: isolated")
        XCTAssertTrue(fade.superview?.superview === isolation)
        p.apply(wireBatch([])); p.apply(wireBatch([]))
        XCTAssertTrue(fade.superview?.superview === isolation, "the control host leaves it there")
        p.apply(wireBatch([["op": "present", "id": 3, "property": "opacity", "x": 1.0, "y": 0.0, "w": 0.0, "h": 0.0]]))
        XCTAssertNil(node.glassIsolation)
        XCTAssertTrue(fade.superview === node)
    }
}
#endif

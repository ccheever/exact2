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
    /// Tab reaches the buttons here: Keyboard Navigation on (LLP 1115.000 D7.2).
    override func setUp() { super.setUp(); keyboardNavigation(true) }

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
    func testReferencedAccessibleNameWinsAndFollowsItsText() throws {
        let p = presenter(box(1) + native(2, ["accessibilityLabelledBy": "name", "accessibilityLabel": "Fallback"])
                          + box(3, ["id": "name", "text": "Delete permanent copy"])
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Go")])
        let button = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac)
        XCTAssertEqual(button.accessibilityLabel(), "Delete permanent copy", "aria-labelledby precedes aria-label and the face")
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["text": "Delete archived copy"]]]))
        XCTAssertEqual(button.accessibilityLabel(), "Delete archived copy", "a referenced text-only batch refreshes the control")
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["text": ""]]]))
        XCTAssertEqual(button.accessibilityLabel(), "Fallback", "an empty referenced name falls back to aria-label")
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["accessibilityLabel"]]]))
        XCTAssertEqual(button.accessibilityLabel(), "Go", "without an authored name the face names the control")
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

    func testARIAUnavailabilityKeepsNativeButtonFocusableAndPressable() throws {
        let p = presenter(box(1) + native(2, ["accessibilityDisabled": "true", "testId": "update"])
                          + box(3, ["accessibilityRole": "button", "tabIndex": "0", "accessibilityDisabled": "true"], handlers: ["press"])
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]], faces: [2: face("Check for updates")])
        let button = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertTrue(button.isEnabled)
        XCTAssertTrue(button.acceptsFirstResponder)
        XCTAssertTrue(button.canBecomeKeyView)
        XCTAssertFalse(button.isAccessibilityEnabled(), "AXEnabled is false while the control remains operable")
        let ax = p.axElements(roots: [p.viewport])
        let update = try XCTUnwrap((ax["elements"] as? [[String: Any]])?.first { $0["testId"] as? String == "update" })
        XCTAssertEqual((update["states"] as? [String: Any])?["disabled"] as? Bool, true)
        let role = try XCTUnwrap(p.views[3])
        XCTAssertTrue(role.acceptsFirstResponder)
        XCTAssertFalse(role.isAccessibilityEnabled())
        button.performClick(nil)
        XCTAssertEqual(pressed, [2])
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertTrue(button.isAccessibilityEnabled(), "clearing ARIA restores AXEnabled")
    }

    func testAncestorARIAUnavailabilityReachesButtonAndRefreshes() throws {
        let p = presenter(box(1, ["accessibilityRole": "group", "accessibilityDisabled": "true"])
                          + native(2, ["testId": "nested"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]], faces: [2: face("Update")])
        let button = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac)
        XCTAssertTrue(button.isEnabled)
        XCTAssertFalse(button.isAccessibilityEnabled(), "the descendant is refreshed when its ancestor changes")
        XCTAssertEqual(button.accessibilityAttributeValue(.enabled) as? Bool, false)
        XCTAssertEqual(button.cell?.accessibilityAttributeValue(.enabled) as? Bool, false)
        let ax = p.axElements(roots: [p.viewport])
        let nested = try XCTUnwrap((ax["elements"] as? [[String: Any]])?.first { $0["testId"] as? String == "nested" })
        XCTAssertEqual((nested["states"] as? [String: Any])?["disabled"] as? Bool, true)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertTrue(button.isAccessibilityEnabled())
        XCTAssertEqual(button.accessibilityAttributeValue(.enabled) as? Bool, true, "an explicit false overrides the ancestor, as Chrome does")
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["accessibilityDisabled"]]]))
        XCTAssertFalse(button.isAccessibilityEnabled())
        XCTAssertEqual(button.accessibilityAttributeValue(.enabled) as? Bool, false)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertTrue(button.isAccessibilityEnabled())
        XCTAssertEqual(button.accessibilityAttributeValue(.enabled) as? Bool, true)
        XCTAssertEqual(button.cell?.accessibilityAttributeValue(.enabled) as? Bool, true)
    }

    func testARIAUnavailabilityReachesAppKitControlAndItsCell() throws {
        for kind in ["checkbox", "radio", "switch", "select", "range", "date"] {
            let props = ["type": kind == "switch" ? "checkbox" : kind, "accessibilityRole": kind == "switch" ? "switch" : kind, "accessibilityDisabled": "true", "testId": kind]
            let p = presenter(box(1) + [["op": "create", "id": 2, "kind": "control", "props": props, "style": [:]],
                                        ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 140.0, "h": 30.0],
                                        ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]], faces: [:])
            let control = try XCTUnwrap(p.controls.controls[2], kind)
            XCTAssertTrue(control.isEnabled, kind)
            XCTAssertEqual(control.accessibilityAttributeValue(.enabled) as? Bool, false, kind)
            if let cell = control.cell { XCTAssertEqual(cell.accessibilityAttributeValue(.enabled) as? Bool, false, "\(kind) cell") }
            let ax = p.axElements(roots: [p.viewport])
            XCTAssertTrue((ax["elements"] as? [[String: Any]])?.contains {
                ($0["states"] as? [String: Any])?["disabled"] as? Bool == true
            } == true, "tree --ax reports the \(kind) disabled")
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityDisabled": "false"]]]))
            XCTAssertEqual(control.accessibilityAttributeValue(.enabled) as? Bool, true, kind)
            if let cell = control.cell { XCTAssertEqual(cell.accessibilityAttributeValue(.enabled) as? Bool, true, "\(kind) cell") }
            window.close()
        }
    }

    /// LLP 1115.000 D7.1: a click moves no focus, press or no press.
    func testAClickMovesNoFocusAndPressesOnlyAnAncestorItIsIn() throws {
        let p = presenter(box(1) + native(2, handlers: ["focus"]) + box(4, handlers: ["press"]) + native(3, handlers: [], x: 320) + native(5, handlers: [])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 4, "ids": [3, 5]], ["op": "roots", "ids": [1, 4]]],
                          faces: [2: face("Mark"), 3: face("Out"), 5: face("In")])
        var pressed: [UInt32] = [], focused: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.onFocus = { focused.append($0) }
        try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac).performClick(nil)
        XCTAssertFalse(window.firstResponder === p.controls.controls[2], "a click leaves the first responder where it is")
        XCTAssertNil(p.focusedNode)
        XCTAssertEqual(focused, [], "and the button hears no focus")
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

    /// A click focuses no custom button and rings none (LLP 1115.000 D7.1).
    /// A focus the pointer's press led to (its action's `focus()`) is not
    /// visible; a key used there, or a Tab to the next, shows the ring.
    func testAClickFocusesNoButtonAndTheKeyboardsFocusShowsItsRing() throws {
        let p = presenter(box(1, ["id": "first"], handlers: ["press"]) + box(2, handlers: ["press"])
                          + [["op": "frame", "id": 2, "x": 0.0, "y": 50.0, "w": 300.0, "h": 40.0], ["op": "roots", "ids": [1, 2]]], faces: [:])
        let first = try XCTUnwrap(p.views[1]), second = try XCTUnwrap(p.views[2])
        p.flushKeyViewLoop()
        let at = first.convert(NSPoint(x: 10, y: 10), to: nil)
        let down = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: at, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        first.mouseDown(with: down)
        XCTAssertFalse(window.firstResponder === first, "a click focuses no button")
        XCTAssertTrue(first.focusRingMaskBounds.isEmpty)
        p.focusElement(["first"])
        XCTAssertTrue(window.firstResponder === first, "the pressed action's focus() moves it")
        XCTAssertTrue(first.focusRingMaskBounds.isEmpty, "a focus the pointer's press led to is not visible")
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

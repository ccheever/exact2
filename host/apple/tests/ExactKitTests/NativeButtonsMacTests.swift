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

    /// LLP 1116.008 (team-kanban: "Delete is not styled as destructive"): an ordinary
    /// `button destructive=true` is AppKit's destructive role (`hasDestructiveAction`) in the
    /// platform's red, whatever accent it only inherits: red ink on a push or borderless button,
    /// a red bezel under AppKit's white on the accent look. What the author wrote wins: an accent
    /// written on the button, a colour on its title.
    func testADestructiveButtonIsAppKitsDestructiveRoleInTheSystemsRedUnlessTheAuthorPaintedIt() throws {
        let indigo: [Int] = [97, 85, 245, 255]
        let orange: [Int] = [255, 141, 40, 255]
        func node(_ id: Int, destructive: Bool, accent: [Int], wrote: Bool = false) -> [[String: Any]] {
            var ops = native(id, destructive ? ["destructive": "true"] : [:])
            ops[0]["style"] = ["appearance": "auto", "accent_color": accent, "written": wrote ? ["accent_color"] : [String]()]
            return ops
        }
        var green = face("Green", macos: "push", style: "bordered")
        green.rows.title["text_color"] = .array([.number(0), .number(128), .number(0), .number(255)])
        let faces: [UInt32: ButtonFace] = [
            2: face("Delete", macos: "push"), 3: face("Delete", macos: "borderless", style: "plain"),
            4: face("Delete", macos: "push-accent", style: "filled"), 5: face("Keep", macos: "push"),
            6: face("Delete", macos: "push"), 7: green,
        ]
        let p = presenter(box(1) + node(2, destructive: true, accent: indigo) + node(3, destructive: true, accent: indigo)
                          + node(4, destructive: true, accent: indigo) + node(5, destructive: false, accent: indigo)
                          + node(6, destructive: true, accent: orange, wrote: true) + node(7, destructive: true, accent: indigo)
                          + [["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7]], ["op": "roots", "ids": [1]]], faces: faces)
        let button = { (id: UInt32) in try XCTUnwrap(p.controls.controls[id] as? NativeButtonMac, "\(id)") }
        let red = NSColor.systemRed
        func ink(_ b: NativeButtonMac) -> NSColor? {
            guard b.attributedTitle.length > 0 else { return nil }
            return b.attributedTitle.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor
        }
        let push = try button(2), plain = try button(3), prominent = try button(4)
        XCTAssertTrue(push.hasDestructiveAction); XCTAssertTrue(plain.hasDestructiveAction); XCTAssertTrue(prominent.hasDestructiveAction)
        XCTAssertEqual(ink(push), red, "red ink on a push button")
        XCTAssertEqual(plain.contentTintColor, red, "and on a borderless one")
        XCTAssertEqual(ink(plain), red)
        XCTAssertEqual(prominent.bezelColor, red, "a red bezel under AppKit's own white title on the accent look")
        XCTAssertNotEqual(ink(prominent), red, "AppKit draws its own title on it")
        // Not destructive: the accent it inherits, AppKit's ink.
        let keep = try button(5)
        XCTAssertFalse(keep.hasDestructiveAction)
        XCTAssertNotEqual(ink(keep), red)
        // An accent written on the button wins over the role's red; a colour on its title does too.
        let accented = try button(6)
        XCTAssertTrue(accented.hasDestructiveAction, "still the role")
        XCTAssertNotEqual(ink(accented), red, "no red ink over an authored accent")
        let painted = try button(7)
        XCTAssertEqual(ink(painted), NSColor(srgbRed: 0, green: 128 / 255, blue: 0, alpha: 1))
    }

    /// The role has AppKit's manners with it: an `NSButton` takes the click that brings its
    /// window forward, and one with `hasDestructiveAction` does not (measured on macOS 27.0.1),
    /// so a person's first click on a window behind never presses Delete. The agent's window is
    /// never key, so its every click counts (LLP 1115.000 D7.9), a destructive button's too:
    /// AppKit's answer alone left the driver no way to press one, and Shelf's "Remove…" row,
    /// painted under the agent, took its tap and did nothing.
    func testADestructiveButtonTakesTheAgentsClickAndNoOtherThatActivatesItsWindow() throws {
        let p = presenter(box(1) + native(2, ["destructive": "true"]) + native(3, x: 140)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Delete"), 3: face("Keep")])
        p.viewport.layoutSubtreeIfNeeded()
        window.orderFrontRegardless()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let delete = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac), keep = try XCTUnwrap(p.controls.controls[3] as? NativeButtonMac)
        XCTAssertTrue(delete.hasDestructiveAction)
        addTeardownBlock { NodeView.everyClickCounts = ExactEnv.agentMode }
        NodeView.everyClickCounts = false
        XCTAssertTrue(keep.acceptsFirstMouse(for: nil), "a button clicks through")
        XCTAssertFalse(delete.acceptsFirstMouse(for: nil), "a destructive one does not: AppKit's answer, a person's")
        NodeView.everyClickCounts = true
        XCTAssertTrue(delete.acceptsFirstMouse(for: nil), "the agent's every click counts")
        // A click through the application, as the agent's `tap` sends it, the up queued for the button's tracking.
        func click(_ button: NSButton) {
            let at = button.convert(NSPoint(x: button.bounds.midX, y: button.bounds.midY), to: nil)
            func event(_ type: NSEvent.EventType) -> NSEvent {
                NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                   windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
            }
            let up = event(.leftMouseUp)
            NSApp.postEvent(up, atStart: true)
            NSApp.sendEvent(event(.leftMouseDown))
            if NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) != nil { NSApp.sendEvent(up) }
        }
        click(delete)
        XCTAssertEqual(pressed, [2], "the agent's click presses a destructive button in a window that is not key")
        click(keep)
        XCTAssertEqual(pressed, [2, 3])
    }

    func testNativeButtonBubblesOutOfPresentationsAndKeepsModalInvocationsInside() throws {
        let p = presenter(box(1) + box(2, handlers: ["press"]) + box(3, ["id": "pop", "popover": "manual"])
                          + box(4, ["id": "form", "semanticTag": "dialog"]) + native(5, handlers: []) + native(6, handlers: [])
                          + box(7, ["id": "outside", "popover": "manual"]) + [
                            ["op": "children", "id": 1, "ids": [2, 7]],
                            ["op": "children", "id": 2, "ids": [3, 4]],
                            ["op": "children", "id": 3, "ids": [5]],
                            ["op": "children", "id": 4, "ids": [6]],
                            ["op": "roots", "ids": [1]],
                            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 400],
                            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 40, "h": 30],
                            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 240, "h": 100],
                            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 240, "h": 100],
                          ], faces: [5: face("Popover"), 6: face("Dialog")])
        p.menus.painted = true; p.dialogs.painted = true
        defer { p.menus.reset(); p.dialogs.reset() }
        let ancestor = try XCTUnwrap(p.views[2]), pop = try XCTUnwrap(p.views[3])
        let dialog = try XCTUnwrap(p.views[4]), outside = try XCTUnwrap(p.views[7])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.menus.action("showPopover", ["pop"])
        XCTAssertTrue(p.menus.isOpen(pop))
        try XCTUnwrap(p.controls.controls[5] as? NSButton).performClick(nil)
        XCTAssertEqual(pressed, [2], "a native action reaches the lifted popover's authored ancestor once")
        p.menus.close(pop)
        p.dialogCommand("showModal", "form")
        XCTAssertTrue(p.dialogs.active === dialog)
        XCTAssertTrue(ancestor.inert)
        ancestor.props["popovertarget"] = "outside"
        ancestor.props["popovertargetaction"] = "show"
        try XCTUnwrap(p.controls.controls[6] as? NSButton).performClick(nil)
        XCTAssertEqual(pressed, [2, 2], "modal inertness does not suppress the ancestor's handler")
        XCTAssertFalse(p.menus.isOpen(outside), "a bubbled native action cannot invoke a target outside the modal")
        XCTAssertTrue(p.dialogs.active === dialog)
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

    /// An assistive app's press (AXPress, at the cell VoiceOver lands on)
    /// clicks the button once AppKit's request has returned, and once: run
    /// inside it, a press that landed answered kAXErrorAttributeUnsupported
    /// (the update it made recorded an accessibility error).
    func testAnAssistivePressClicksOnceTheRequestReturns() throws {
        let p = presenter(box(1) + native(2) + native(3, ["disabled": "true"])
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Go"), 3: face("Off")])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let go = try XCTUnwrap(p.controls.controls[2]?.cell)
        XCTAssertFalse(try XCTUnwrap(p.controls.controls[3]?.cell).accessibilityPerformPress(), "a disabled button")
        XCTAssertTrue(go.accessibilityPerformPress())
        XCTAssertEqual(pressed, [], "nothing runs inside the request")
        drain()
        XCTAssertEqual(pressed, [2])
        // AppKit's checkbox (its cell) and switch, the same.
        for kind in ["checkbox", "switch"] {
            window.close()
            let props = ["type": "checkbox", "accessibilityRole": kind]
            let p = presenter(box(1) + [["op": "create", "id": 2, "kind": "control", "props": props, "handlers": ["change"], "style": [:]],
                                        ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 140.0, "h": 30.0],
                                        ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]], faces: [:])
            var checked: [Bool] = []
            p.onChecked = { _, on in checked.append(on) }
            let control = try XCTUnwrap(p.controls.controls[2], kind)
            // A switch is its own element; a checkbox's is its cell.
            XCTAssertTrue(control is NSSwitch ? control.accessibilityPerformPress() : try XCTUnwrap(control.cell, kind).accessibilityPerformPress(), kind)
            XCTAssertEqual(checked, [], "\(kind): nothing runs inside the request")
            drain()
            XCTAssertEqual(checked, [true], kind)
        }
    }
    /// Runs the main queue past what is on it now.
    private func drain() {
        var drained = false
        DispatchQueue.main.async { drained = true }
        let deadline = Date().addingTimeInterval(5)
        while !drained, Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
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

// Where the focus goes when what holds it changes under it, as in Chrome.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class FocusFixupMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil; KeyboardNavigation.agent = nil }

    /// A page of four stops: a native button, a bare button, a native button
    /// and a field, the middle two in a column.
    private func fixture(navigation: Bool) throws -> Presenter {
        _ = NSApplication.shared
        KeyboardNavigation.agent = navigation
        let p = Presenter()
        var face = ButtonFace(); face.title = "Button"
        p.buttonFace = { _ in face }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "button", "id": "first"], "handlers": ["press", "blur"], "style": ["appearance": "auto"]],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 120, "h": 30],
            ["op": "create", "id": 5, "kind": "view", "props": ["id": "column"]],
            ["op": "frame", "id": 5, "x": 0, "y": 40, "w": 400, "h": 100],
            ["op": "create", "id": 3, "kind": "button", "props": ["id": "bare"], "handlers": ["press", "blur"]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 120, "h": 30],
            ["op": "create", "id": 4, "kind": "control", "props": ["type": "button", "id": "middle"], "handlers": ["press", "blur"], "style": ["appearance": "auto"]],
            ["op": "frame", "id": 4, "x": 0, "y": 50, "w": 120, "h": 30],
            ["op": "create", "id": 6, "kind": "input", "props": ["id": "field", "value": ""]],
            ["op": "frame", "id": 6, "x": 0, "y": 150, "w": 200, "h": 30],
            ["op": "children", "id": 5, "ids": [3, 4]],
            ["op": "children", "id": 1, "ids": [2, 5, 6]], ["op": "roots", "ids": [1]]]))
        p.flushKeyViewLoop()
        return p
    }
    private func tab(_ p: Presenter, back: Bool = false) throws {
        let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: back ? .shift : [], timestamp: 0,
                                                  windowNumber: window.windowNumber, context: nil, characters: "\t",
                                                  charactersIgnoringModifiers: "\t", isARepeat: false, keyCode: 48))
        if !p.routeKey(event, focused: true, in: window) { window.sendEvent(event) }
    }
    private func focused(_ p: Presenter) -> String? { p.focusedNode?.props["id"] }

    /// A focused control that becomes disabled loses the focus, and its blur
    /// is heard; Tab and Shift-Tab go on from where it stands, with Keyboard
    /// Navigation on (it was a stop) and off (an action had focused it).
    func testAFocusedControlThatBecomesDisabledLosesTheFocusAndTabGoesOnFromIt() throws {
        for navigation in [true, false] {
            for (id, name, back, next) in [(UInt32(4), "middle", false, "field"), (4, "middle", true, "bare"), (3, "bare", false, "middle"),
                                           (2, "first", false, "bare")] {
                let p = try fixture(navigation: navigation)
                var blurs: [UInt32] = []
                p.onBlur = { blurs.append($0) }
                p.focusElement([name])
                XCTAssertEqual(focused(p), name)
                p.apply(wireBatch([["op": "props", "id": id, "set": ["disabled": "true"]]]))
                XCTAssertNil(p.focusedNode, "\(name), navigation \(navigation): a disabled control holds no focus")
                XCTAssertFalse(window.firstResponder === p.keyView(of: try XCTUnwrap(p.views[id])))
                XCTAssertEqual(blurs, [id])
                try tab(p, back: back)
                // With Keyboard Navigation off the buttons are no stops: the field is the next and the only one.
                XCTAssertEqual(focused(p), navigation ? next : "field", "\(name), navigation \(navigation), back \(back)")
                window.close(); window = nil
            }
        }
    }

    /// The place is the disabled control's until the focus goes elsewhere.
    func testTheDisabledControlsPlaceLastsUntilTheFocusMoves() throws {
        let p = try fixture(navigation: true)
        p.focusElement(["middle"])
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["disabled": "true"]]]))
        XCTAssertTrue(p.tabStart === p.controls.controls[4])
        p.focusElement(["first"])
        XCTAssertNil(p.tabStart, "a focus taken since is where Tab goes on from")
    }

    /// `aria-hidden` takes a subtree out of the accessibility tree only: the
    /// button in it keeps the focus and Tab goes on from it. `inert` and a
    /// hidden view still take the focus away.
    func testAriaHiddenAboveTheFocusLeavesItInertAndHiddenDoNot() throws {
        let p = try fixture(navigation: true)
        p.focusElement(["middle"])
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["accessibilityElementsHidden": "true"]]]))
        XCTAssertEqual(focused(p), "middle", "aria-hidden leaves the focus")
        XCTAssertTrue(window.firstResponder === p.controls.controls[4])
        XCTAssertTrue(try XCTUnwrap(p.views[3]).isAccessibilityHidden(), "and still hides the subtree from VoiceOver")
        try tab(p)
        XCTAssertEqual(focused(p), "field")
        p.focusElement(["middle"])
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["inert": "true"]]]))
        XCTAssertNil(p.focusedNode, "inert takes it")
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["inert": "false"]]]))
        p.focusElement(["middle"])
        XCTAssertEqual(focused(p), "middle")
        try XCTUnwrap(p.views[5]).isHidden = true
        p.navigation.refreshInputGates([try XCTUnwrap(p.views[4])])
        XCTAssertNotEqual(focused(p), "middle", "a hidden view takes it")
    }
}
#endif

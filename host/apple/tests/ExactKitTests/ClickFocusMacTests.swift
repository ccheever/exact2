// @ref LLP 1115.000 D7.1 — on the Mac a click on a button, a pressable or
// the window's ground leaves the first responder where it is: `retainFocus`
// is the default. A field keeps its editor and caret, its pending `change`
// committed before the press acts. A box the author made a taker of keys
// still takes a click's focus, and `focus()` still moves it.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class ClickFocusMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    /// A page (1) holding a field (2, `field`), a native button (3), a custom
    /// `button` (4), a pressable box (5, a card), a box with a `key` handler
    /// (6), a pressable box with a `tabindex` (7), a `key` box under a
    /// `retainFocus` box (9 in 8), a disabled custom button (10) and, below
    /// them all, empty ground.
    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        var face = ButtonFace(); face.title = "Send"
        p.buttonFace = { _ in face }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        window.contentView = p.viewport
        clicksArriveAsInTheKeyWindow()
        func row(_ id: Int, _ kind: String, _ props: [String: String] = [:], handlers: [String] = [], style: [String: Any] = [:], y: Int) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": kind, "props": props, "handlers": handlers, "style": style],
             ["op": "frame", "id": id, "x": 10, "y": y, "w": 200, "h": 30]]
        }
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view", "handlers": ["focusin", "focusout"]], ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 500]]
        ops += row(2, "input", ["value": "", "id": "field"], handlers: ["input", "change", "blur"], y: 10)
        ops += row(3, "control", ["type": "button", "id": "native"], handlers: ["press"], style: ["appearance": "auto"], y: 50)
        ops += row(4, "button", ["id": "custom"], handlers: ["press"], y: 90)
        // Its title fills it: the click lands on the paragraph, which accepts first responder.
        ops += [["op": "create", "id": 11, "kind": "text", "props": ["text": "Send"]], ["op": "frame", "id": 11, "x": 0, "y": 0, "w": 200, "h": 30],
                ["op": "children", "id": 4, "ids": [11]]]
        ops += row(5, "view", ["id": "card"], handlers: ["press"], y: 130)
        ops += row(6, "view", ["id": "keys"], handlers: ["key"], y: 170)
        ops += row(7, "view", ["id": "stop", "tabIndex": "0"], handlers: ["press"], y: 210)
        ops += row(8, "view", ["retainFocus": "true"], y: 250)
        ops += [["op": "create", "id": 9, "kind": "view", "props": ["id": "kept"], "handlers": ["key"]], ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 200, "h": 30],
                ["op": "children", "id": 8, "ids": [9]]]
        ops += row(10, "button", ["id": "off", "disabled": "true"], handlers: ["press"], y: 290)
        ops += row(12, "control", ["type": "button", "id": "nativeStop", "tabIndex": "0"], handlers: ["press"], style: ["appearance": "auto"], y: 330)
        ops += [["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7, 8, 10, 12]], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        p.flushKeyViewLoop()
        // The window server routes a click to a window that is on screen; it is never made key.
        window.orderFrontRegardless()
        return p
    }

    /// AppKit's own rule, which the node's answers rest on: its window makes
    /// a clicked view first responder when the view accepts first responder
    /// and "needs its panel to become key", and not otherwise. If this stops
    /// holding the other cases here prove nothing.
    func testAppKitsWindowFocusesAClickedViewThatAcceptsAndNeedsItsPanelKey() throws {
        final class Box: NSView {
            var needs = true
            override var acceptsFirstResponder: Bool { true }
            override var needsPanelToBecomeKey: Bool { needs }
            override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
            override func mouseDown(with event: NSEvent) {}
            override func mouseUp(with event: NSEvent) {}
        }
        _ = fixture()
        let box = Box(frame: NSRect(x: 250, y: 10, width: 100, height: 40))
        try XCTUnwrap(window.contentView).addSubview(box)
        let middle = box.convert(NSPoint(x: 50, y: 20), to: nil)
        box.needs = false
        click(at: middle)
        XCTAssertFalse(window.firstResponder === box, "as an NSButton answers: no focus")
        box.needs = true
        click(at: middle)
        XCTAssertTrue(window.firstResponder === box, "AppKit's window focused the clicked view before its mouse-down")
        XCTAssertFalse(NSButton(title: "Send", target: nil, action: nil).needsPanelToBecomeKey, "an NSButton does not need its panel key")
    }

    /// A click as a hand's comes, through the application and the window:
    /// AppKit's window is what makes a clicked view first responder, before
    /// the view hears the mouse-down (`_handleLeftMouseDownEvent`), so a
    /// click handed straight to a view would test none of this. The up is
    /// in the queue first for a control's tracking loop; a node's view is
    /// sent it (this click's own: a dequeued one has the window server's place).
    private func click(at point: NSPoint) {
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        let up = event(.leftMouseUp)
        NSApp.postEvent(up, atStart: true)
        NSApp.sendEvent(event(.leftMouseDown))
        if NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) != nil { NSApp.sendEvent(up) }
    }
    private func click(_ node: NodeView) { click(at: node.convert(NSPoint(x: node.bounds.midX, y: node.bounds.midY), to: nil)) }

    /// The field's editor, the field made first responder if it is not.
    private func edit(_ p: Presenter) throws -> NSTextView {
        let field = try XCTUnwrap(p.views[2]?.field)
        if field.currentEditor() == nil { XCTAssertTrue(window.makeFirstResponder(field)) }
        return try XCTUnwrap(field.currentEditor() as? NSTextView)
    }

    func testFocusRemovalPreservesTheEnclosingKeysStopFlag() throws {
        for hearsFocus in [false, true] {
            let p = fixture()
            let child = try XCTUnwrap(p.views[6]), wrapper = try XCTUnwrap(p.views[1])
            wrapper.handlers = hearsFocus ? ["key", "focusout"] : ["key"]
            XCTAssertTrue(window.makeFirstResponder(child))
            var heard: [UInt32] = []
            p.onKey = { id, _ in
                heard.append(id)
                if id == 6 {
                    p.propagationStopped = true
                    p.apply(wireBatch([["op": "destroy", "id": 6]]))
                }
            }
            _ = p.keyDown(at: child, "Escape")
            XCTAssertEqual(heard, [6], "nested focusout must not restart key bubbling")
            window.close()
        }
    }

    func testBlurRemovesWrapperBeforeFocusout() throws {
        for moving in [true, false] {
            let p = fixture()
            defer { window.close() }
            XCTAssertTrue(window.makeFirstResponder(nil))
            p.apply(wireBatch([
                ["op": "create", "id": 20, "kind": "view", "handlers": ["focusout"]],
                ["op": "children", "id": 20, "ids": [2]],
                ["op": "children", "id": 1, "ids": [20, 3, 4, 5, 6, 7, 8, 10, 12]]
            ]))
            XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[2]?.field)))
            let outgoing = expectation(description: "detached focusout")
            var heard: [String] = []
            p.onBlur = { _ in
                heard.append("blur")
                p.apply(wireBatch([["op": "destroy", "id": 20], ["op": "destroy", "id": 2]]))
            }
            p.onFocusEvent = { at, kind, target, related in
                heard.append("\(at):\(kind):\(target):\(related ?? "-")")
                if at == 20 { outgoing.fulfill() }
            }
            XCTAssertTrue(window.makeFirstResponder(moving ? try XCTUnwrap(p.views[4]) : nil))
            wait(for: [outgoing], timeout: 5)
            XCTAssertEqual(heard, moving ? ["blur", "20:focusout:field:custom", "1:focusin:custom:field"] : ["blur", "20:focusout:field:-"])
            XCTAssertNil(p.views[20])
        }
    }

    func testFocusTransitionUsesTheRenamedId() throws {
        let p = fixture()
        _ = try edit(p)
        var heard: [String] = []
        p.onFocusEvent = { _, kind, target, related in heard.append("\(kind):\(target):\(related ?? "-")") }
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["id": "renamed"]]]))
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[4])))
        XCTAssertEqual(heard, ["focusout:renamed:custom", "focusin:custom:renamed"])
    }

    func testPassiveAncestorHearsFocusMoveBetweenChildren() throws {
        let p = fixture()
        var heard: [String] = []
        p.onFocusEvent = { at, kind, target, related in
            heard.append("\(at):\(kind):\(target):\(related ?? "-")")
        }
        XCTAssertFalse(try XCTUnwrap(p.views[1]).acceptsFirstResponder)
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[4])))
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[6])))
        XCTAssertEqual(heard, ["1:focusout:field:custom", "1:focusin:custom:field", "1:focusout:custom:keys", "1:focusin:keys:custom"])
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[4])))
        heard.removeAll()
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 3, 5, 6, 7, 8, 10, 12]], ["op": "destroy", "id": 4]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.02))
        XCTAssertEqual(heard, ["1:focusout:custom:-"])
    }

    func testAClickOnAButtonAPressableOrTheGroundLeavesTheFieldItsEditor() throws {
        let p = fixture()
        var heard: [String] = []
        p.onPress = { heard.append("press \($0)") }
        p.onChange = { heard.append("change \($0) \($1)") }
        p.onBlur = { heard.append("blur \($0)") }
        let title = try XCTUnwrap(p.views[11])
        XCTAssertTrue(window.contentView?.hitTest(title.convert(NSPoint(x: title.bounds.midX, y: title.bounds.midY), to: nil)) === title, "the custom button's click lands on its title")
        XCTAssertTrue(title.acceptsFirstResponder, "which accepts first responder, and so would be focused by AppKit's window")
        XCTAssertFalse(title.needsPanelToBecomeKey)
        var typed = ""
        for (name, id) in [("a native button", UInt32(3)), ("a custom button", 4), ("a pressable box", 5)] {
            let editor = try edit(p)
            editor.insertText("a", replacementRange: editor.selectedRange())
            typed += "a"
            heard.removeAll()
            click(try XCTUnwrap(p.views[id]))
            XCTAssertTrue(window.firstResponder === editor, "\(name): the field keeps its editor")
            XCTAssertTrue(try XCTUnwrap(p.views[2]?.field).currentEditor() === editor, name)
            XCTAssertEqual(editor.string, typed, "\(name): and its text")
            XCTAssertEqual(heard, ["change 2 \(typed)", "press \(id)"], "\(name): the edit is committed before the press acts, with no blur")
            // Nothing new to commit: the next press commits nothing.
            heard.removeAll()
            click(try XCTUnwrap(p.views[id]))
            XCTAssertEqual(heard, ["press \(id)"], name)
            XCTAssertNil(p.focusedNode.flatMap { $0.id == id ? $0 : nil }, "\(name) did not take the focus")
        }
        // The ground: nothing is pressed, and the edit stays the field's to commit.
        let editor = try edit(p)
        editor.insertText("b", replacementRange: editor.selectedRange())
        heard.removeAll()
        click(at: NSPoint(x: 300, y: 20))
        XCTAssertTrue(window.firstResponder === editor, "a click on the window's ground leaves the first responder")
        XCTAssertEqual(heard, [])
        // A disabled button: nothing at all.
        click(try XCTUnwrap(p.views[10]))
        XCTAssertTrue(window.firstResponder === editor)
        XCTAssertEqual(heard, [])
        // The field's own blur still commits, as it did.
        XCTAssertTrue(window.makeFirstResponder(nil))
        XCTAssertEqual(heard, ["change 2 \(typed)b", "blur 2"])
    }

    func testAClickWithNothingFocusedFocusesNoButton() throws {
        let p = fixture()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        // AppKit gave the window's first key view the focus as it came on screen.
        XCTAssertTrue(window.makeFirstResponder(nil))
        for id in [UInt32(3), 4, 5] {
            click(try XCTUnwrap(p.views[id]))
            XCTAssertNil(p.focusedNode, "a click on \(id) focuses nothing")
        }
        XCTAssertEqual(pressed, [3, 4, 5])
        // A focus the keyboard gave another button stays there too.
        let native = try XCTUnwrap(p.controls.controls[3] as? NativeButtonMac)
        XCTAssertTrue(window.makeFirstResponder(native))
        click(try XCTUnwrap(p.views[4]))
        XCTAssertTrue(window.firstResponder === native)
        click(try XCTUnwrap(p.views[5]))
        XCTAssertTrue(window.firstResponder === native)
        XCTAssertEqual(pressed, [3, 4, 5, 4, 5])
    }

    func testABoxThatTakesKeysTakesAClicksFocusAndRetainFocusStillKeepsIt() throws {
        let p = fixture()
        let keys = try XCTUnwrap(p.views[6]), stop = try XCTUnwrap(p.views[7]), kept = try XCTUnwrap(p.views[9])
        XCTAssertTrue(keys.takesClickFocus, "a box with a `key` handler is the author's own taker of keys")
        XCTAssertTrue(stop.takesClickFocus, "a `tabindex` the author wrote takes a click's focus, pressable or not")
        XCTAssertFalse(kept.takesClickFocus, "`retainFocus` above it keeps the focus where it was")
        XCTAssertFalse(try XCTUnwrap(p.views[4]).takesClickFocus)
        XCTAssertFalse(try XCTUnwrap(p.views[5]).takesClickFocus)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let editor = try edit(p)
        click(kept)
        XCTAssertTrue(window.firstResponder === editor, "under retainFocus the editor stays")
        click(keys)
        XCTAssertTrue(window.firstResponder === keys, "the key box takes the focus, as a clicked custom view that accepts first responder")
        click(stop)
        XCTAssertTrue(window.firstResponder === stop)
        XCTAssertEqual(pressed, [7])
        // And a click on a button from there leaves it on the box.
        click(try XCTUnwrap(p.views[4]))
        XCTAssertTrue(window.firstResponder === stop)
        // A native button whose author wrote a `tabindex` takes a click's focus too; one without does not.
        let native = try XCTUnwrap(p.controls.controls[12] as? NativeButtonMac)
        XCTAssertTrue(native.needsPanelToBecomeKey)
        XCTAssertFalse(try XCTUnwrap(p.controls.controls[3] as? NativeButtonMac).needsPanelToBecomeKey)
        click(try XCTUnwrap(p.views[12]))
        XCTAssertTrue(window.firstResponder === native, "the author's tabindex wins on a native button")
        XCTAssertEqual(pressed, [7, 4, 12])
        click(try XCTUnwrap(p.views[3]))
        XCTAssertTrue(window.firstResponder === native, "and a plain one leaves the focus there")
        p.apply(wireBatch([["op": "props", "id": 12, "set": ["retainFocus": "true"]]]))
        XCTAssertFalse(native.needsPanelToBecomeKey, "retainFocus keeps the focus where it was")
    }

    func testAnActionsFocusStillMovesTheFocusToAButton() throws {
        let p = fixture()
        let editor = try edit(p)
        // The pressed action focuses another control: kept, since the click itself moved nothing.
        p.onPress = { _ in p.focusElement(["native"]) }
        click(try XCTUnwrap(p.views[4]))
        XCTAssertTrue(window.firstResponder === p.controls.controls[3], "a focus an action set is kept")
        XCTAssertFalse(window.firstResponder === editor)
        p.onPress = nil
        p.focusElement(["custom"])
        XCTAssertTrue(window.firstResponder === p.views[4], "focus() reaches a custom button")
        // A click on a text field is AppKit's own, in the field's own view:
        // nothing here takes part in it, and it is not driven in a window
        // that is not key.
    }
}
#endif

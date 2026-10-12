// @ref LLP 1115.000 D7.3, D7.4 — Space presses the focused control, Return
// the default button; Return in a single-line field sends `submit` and ends
// the editing; a prevented key and a focus an action set always win.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class DefaultButtonMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    /// A page (1) with a field outside any dialog (9, id `lone`), a textarea
    /// (10) and a link (11), and a `dialog` (3, id `form`) holding a field
    /// (4, id `name`), a second field (5, id `other`), a textarea (6), and
    /// three native buttons: Cancel (7, closes it), Delete (8, destructive)
    /// and OK (12). `filled` says which buttons are `buttonStyle="filled"`.
    private func fixture(filled: Set<UInt32> = [12], fieldHandlers: [String] = [], open: Bool = true) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let titles: [UInt32: String] = [7: "Cancel", 8: "Delete", 12: "OK"]
        p.buttonFace = { id in
            var face = ButtonFace(); face.title = titles[id]
            if filled.contains(id) { face.style = "filled"; face.macos = "push-accent" }
            return face
        }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        window.contentView = p.viewport
        func native(_ id: Int, _ props: [String: String], handlers: [String] = ["press"], y: Int) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "control", "props": ["type": "button"].merging(props) { $1 }, "handlers": handlers, "style": ["appearance": "auto"]],
             ["op": "frame", "id": id, "x": 10, "y": y, "w": 100, "h": 30]]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 500, "h": 400],
            ["op": "create", "id": 9, "kind": "input", "props": ["value": "page", "id": "lone"], "handlers": fieldHandlers],
            ["op": "frame", "id": 9, "x": 10, "y": 300, "w": 200, "h": 30],
            ["op": "create", "id": 10, "kind": "textarea", "props": ["value": "one", "id": "notes"]],
            ["op": "frame", "id": 10, "x": 10, "y": 340, "w": 200, "h": 50],
            ["op": "create", "id": 11, "kind": "button", "props": ["href": "/elsewhere", "id": "link"], "handlers": ["press"]],
            ["op": "frame", "id": 11, "x": 250, "y": 300, "w": 100, "h": 30],
            ["op": "create", "id": 3, "kind": "view", "props": ["semanticTag": "dialog", "id": "form", "closedby": "any"]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 300, "h": 260],
            ["op": "create", "id": 4, "kind": "input", "props": ["value": "draft", "id": "name", "autofocus": "true"], "handlers": fieldHandlers],
            ["op": "frame", "id": 4, "x": 10, "y": 10, "w": 200, "h": 30],
            ["op": "create", "id": 5, "kind": "input", "props": ["value": "", "id": "other"]],
            ["op": "frame", "id": 5, "x": 10, "y": 50, "w": 200, "h": 30],
            ["op": "create", "id": 6, "kind": "textarea", "props": ["value": "a", "id": "body"]],
            ["op": "frame", "id": 6, "x": 10, "y": 90, "w": 200, "h": 50],
        ]
        ops += native(7, ["id": "cancel", "commandfor": "form", "command": "close"], handlers: [], y: 150)
        ops += native(8, ["id": "delete", "destructive": "true"], y: 185)
        ops += native(12, ["id": "ok"], y: 220)
        ops += [["op": "children", "id": 3, "ids": [4, 5, 6, 7, 8, 12]], ["op": "children", "id": 1, "ids": [9, 10, 11, 3]], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        p.flushKeyViewLoop()
        if open { p.dialogs.show(p.views[3]!) }
        return p
    }
    private func key(_ chars: String, _ code: UInt16, flags: NSEvent.ModifierFlags = []) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber, context: nil,
                         characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)!
    }
    /// As the session's event monitor routes a key, then AppKit delivers it.
    private func deliver(_ p: Presenter, _ event: NSEvent) {
        if !p.routeKey(event, focused: true, in: window) { window.sendEvent(event) }
    }
    private var returnKey: NSEvent { key("\r", 36) }
    private func button(_ p: Presenter, _ id: UInt32) throws -> NativeButtonMac { try XCTUnwrap(p.controls.controls[id] as? NativeButtonMac) }
    private func editor(_ p: Presenter, _ id: UInt32) throws -> NSTextView { try XCTUnwrap(try XCTUnwrap(p.views[id]?.field).currentEditor() as? NSTextView) }

    func testReturnPressesTheOneFilledButtonWhereverTheFocusIsAndSpaceTheFocusedOne() throws {
        let p = fixture(fieldHandlers: ["change"])
        var pressed: [UInt32] = [], changed: [String] = [], order: [String] = []
        p.onPress = { pressed.append($0); order.append("press") }
        p.onChange = { _, value in changed.append(value); order.append("change") }
        let dialog = try XCTUnwrap(p.views[3]), ok = try button(p, 12), cancel = try button(p, 7), delete = try button(p, 8)
        XCTAssertTrue(p.dialogs.defaultButton(of: dialog) === p.views[12])
        XCTAssertEqual([cancel, delete, ok].map(\.keyEquivalent), ["", "", "\r"], "AppKit draws the one whose key equivalent is Return as the default")
        // From the field (no `submit` handler): its edit commits, then the default is pressed, and the focus stays in the field.
        let typing = try editor(p, 4)
        XCTAssertTrue(window.firstResponder === typing, "autofocus")
        typing.insertText("x", replacementRange: typing.selectedRange())
        deliver(p, returnKey)
        XCTAssertEqual(pressed, [12])
        XCTAssertEqual(changed.count, 1)
        XCTAssertEqual(order, ["change", "press"], "the editing ends before the default button acts")
        XCTAssertTrue(window.firstResponder === (try editor(p, 4)), "Return's press moves no focus")
        // From another button: Return is still the default's, Space the focused one's.
        XCTAssertTrue(window.makeFirstResponder(delete))
        deliver(p, returnKey)
        XCTAssertEqual(pressed, [12, 12])
        XCTAssertTrue(window.firstResponder === delete)
        deliver(p, key(" ", 49))
        XCTAssertEqual(pressed, [12, 12, 8], "Space presses the focused control")
        // Return with a modifier is not the default's.
        XCTAssertFalse(p.dialogs.pressDefault(key("\r", 36, flags: .command), in: dialog))
        XCTAssertEqual(pressed, [12, 12, 8])
        // A disabled default takes nothing.
        p.apply(wireBatch([["op": "props", "id": 12, "set": ["disabled": "true"]]]))
        XCTAssertFalse(p.dialogs.pressDefault(returnKey, in: dialog))
        p.apply(wireBatch([["op": "props", "id": 12, "set": ["disabled": "false"]]]))
        // The dialog closed (Space on Cancel, its own command): nothing is the default any more.
        XCTAssertTrue(window.makeFirstResponder(cancel))
        deliver(p, key(" ", 49))
        XCTAssertNil(p.dialogs.active)
        XCTAssertEqual(ok.keyEquivalent, "")
        XCTAssertEqual(pressed, [12, 12, 8])
    }

    func testADialogWithNoFilledButtonOrSeveralOrOnlyADestructiveOneHasNoDefault() throws {
        for (name, filled) in [("none", Set<UInt32>()), ("several", [7, 12]), ("a destructive one", [8])] {
            let p = fixture(filled: filled)
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            let dialog = try XCTUnwrap(p.views[3])
            XCTAssertNil(p.dialogs.defaultButton(of: dialog), name)
            XCTAssertEqual(try [7, 8, 12].map { try button(p, $0).keyEquivalent }, ["", "", ""], name)
            // Return does only what the focused control does with it: a button nothing, a field ends its editing.
            XCTAssertTrue(window.makeFirstResponder(try button(p, 12)))
            deliver(p, returnKey)
            XCTAssertEqual(pressed, [], "\(name): Return presses no merely focused button")
            XCTAssertTrue(p.dialogs.active === dialog)
            window.close()
        }
        // One filled beside a filled destructive one: the destructive one does not count.
        let p = fixture(filled: [8, 12])
        XCTAssertTrue(p.dialogs.defaultButton(of: try XCTUnwrap(p.views[3])) === p.views[12])
    }

    /// In a dialog a single-line field with a `submit` handler takes Return
    /// itself; a textarea keeps it as a new line; a prevented key does
    /// nothing more.
    func testAFieldWithSubmitATextareaAndAPreventedKeyComeBeforeTheDefault() throws {
        let p = fixture(fieldHandlers: ["submit", "key"])
        var pressed: [UInt32] = [], submitted: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.onSubmit = { submitted.append($0) }
        let typing = try editor(p, 4)
        XCTAssertTrue(window.firstResponder === typing)
        deliver(p, returnKey)
        XCTAssertEqual(submitted, [4], "the field's own submit")
        XCTAssertEqual(pressed, [], "not the default button")
        XCTAssertEqual(typing.selectedRange(), NSRange(location: 0, length: 5), "the editing ended as NSTextField ends it: its text selected")
        // The author prevented the key: neither the submit nor the default.
        p.onKey = { _, _ in p.defaultPrevented = true }
        deliver(p, returnKey)
        XCTAssertEqual(submitted, [4])
        XCTAssertEqual(pressed, [])
        p.onKey = nil
        // A textarea: a new line.
        let body = try XCTUnwrap(p.views[6]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(body))
        deliver(p, returnKey)
        XCTAssertEqual(pressed, [])
        XCTAssertTrue(body.string.contains("\n"), "Return in a textarea is a new line")
    }

    /// Return in a single-line field outside a dialog sends `submit` and
    /// ends the editing with the text selected; a focus the action set is
    /// kept; Return follows a focused link and presses no other pressable.
    func testReturnInAFieldSubmitsAndEndsTheEditingAndAFocusTheActionSetIsKept() throws {
        let p = fixture(fieldHandlers: ["submit", "change"], open: false)
        var submitted: [UInt32] = [], changed: [String] = [], pressed: [UInt32] = []
        p.onSubmit = { submitted.append($0) }
        p.onChange = { _, value in changed.append(value) }
        p.onPress = { pressed.append($0) }
        p.focusElement(["lone"])
        let typing = try editor(p, 9)
        typing.insertText("!", replacementRange: NSRange(location: 4, length: 0))
        typing.setSelectedRange(NSRange(location: 5, length: 0))
        deliver(p, returnKey)
        XCTAssertEqual(changed, ["page!"])
        XCTAssertEqual(submitted, [9])
        XCTAssertTrue(window.firstResponder === (try editor(p, 9)), "the field keeps the focus, as NSTextField does")
        XCTAssertEqual(try editor(p, 9).selectedRange(), NSRange(location: 0, length: 5), "with its text selected, no caret")
        // The action moves the focus: it is kept, and nothing is selected behind it.
        p.onSubmit = { id in submitted.append(id); p.focusElement(["notes"]) }
        deliver(p, returnKey)
        XCTAssertEqual(submitted, [9, 9])
        XCTAssertTrue(window.firstResponder === p.views[10]?.textArea, "a focus the action set is kept")
        // A textarea outside a dialog: a new line too.
        let notes = try XCTUnwrap(p.views[10]?.textArea)
        deliver(p, returnKey)
        XCTAssertTrue(notes.string.contains("\n"))
        // A link follows on Return and not on Space; nothing else is pressed by Return.
        let link = try XCTUnwrap(p.views[11])
        XCTAssertTrue(window.makeFirstResponder(link))
        deliver(p, key(" ", 49))
        XCTAssertEqual(pressed, [])
        deliver(p, returnKey)
        XCTAssertEqual(pressed, [11])
    }
}
#endif

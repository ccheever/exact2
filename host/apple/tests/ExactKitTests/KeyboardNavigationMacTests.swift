// @ref LLP 1115.000 D7.2 — Tab honours the Mac's Keyboard Navigation
// setting: off (the default) it moves between text fields and what takes
// keys of its own; on it reaches every control. A `tabindex` the author
// wrote wins either way.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

extension XCTestCase {
    /// The Keyboard Navigation mode this test runs in: its own, never the
    /// machine's setting, and given back when the test ends. A case that
    /// walks Tab through buttons says `true`.
    func keyboardNavigation(_ on: Bool) {
        KeyboardNavigation.agent = on
        addTeardownBlock { KeyboardNavigation.agent = nil }
    }
}

final class KeyboardNavigationMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    /// A page (1) holding, in order: a field (2, `a`), a native button (3,
    /// `native`), a custom `button` (4), a pressable box (5), a box with a
    /// `key` handler (6), a pressable box with `tabindex=0` (7), a box with
    /// `tabindex=-1` (8, `skipped`), a field (9, `b`) and a native button
    /// with `tabindex=0` (10).
    private func fixture() -> Presenter {
        var ops = row(2, "input", ["value": "", "id": "a"])
        ops += row(3, "control", ["type": "button", "id": "native"], handlers: ["press"], style: ["appearance": "auto"])
        ops += row(4, "button", ["id": "custom"], handlers: ["press"])
        ops += row(5, "view", ["id": "card"], handlers: ["press"])
        ops += row(6, "view", ["id": "keys"], handlers: ["key"])
        ops += row(7, "view", ["id": "stop", "tabIndex": "0"], handlers: ["press"])
        ops += row(8, "view", ["id": "skipped", "tabIndex": "-1"])
        ops += row(9, "input", ["value": "", "id": "b"])
        ops += row(10, "control", ["type": "button", "id": "nativeStop", "tabIndex": "0"], handlers: ["press"], style: ["appearance": "auto"])
        return page(ops + [["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7, 8, 9, 10]]])
    }
    /// A page (1) in a window of its own, holding what `ops` create.
    private func page(_ ops: [[String: Any]]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        var face = ButtonFace(); face.title = "Go"
        p.buttonFace = { _ in face }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        window.contentView = p.viewport
        p.apply(wireBatch([["op": "create", "id": 1, "kind": "view"], ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 500]] + ops + [["op": "roots", "ids": [1]]]))
        p.flushKeyViewLoop()
        return p
    }
    private func row(_ id: Int, _ kind: String, _ props: [String: String] = [:], handlers: [String] = [], style: [String: Any] = [:]) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": kind, "props": props, "handlers": handlers, "style": style],
         ["op": "frame", "id": id, "x": 10, "y": id * 40, "w": 200, "h": 30]]
    }

    /// The stops Tab visits from the viewport, once around, by node id.
    private func stops(_ p: Presenter) -> [UInt32] {
        p.flushKeyViewLoop()
        var order: [UInt32] = []
        var at = p.viewport.nextValidKeyView
        while let view = at, let node = p.keyTarget(view) ?? p.views.values.first(where: { p.keyView(of: $0) === view }), !order.contains(node.id) {
            order.append(node.id)
            at = view.nextValidKeyView
        }
        return order
    }
    private func tab(_ p: Presenter, shift: Bool = false) throws {
        // To the window the keyboard is in: an open dialog's sheet, else the page's.
        let window: NSWindow = p.dialogs.active?.window ?? self.window
        // Shift-Tab's character is AppKit's backtab, which a field editor reads as `insertBacktab:`.
        let characters = shift ? "\u{19}" : "\t"
        let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: shift ? .shift : [], timestamp: 0, windowNumber: window.windowNumber,
                                                   context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: 48))
        if !p.routeKey(event, focused: true, in: window) { window.sendEvent(event) }
    }

    /// A date, a time and a `select` (X52, #280, #327): with Keyboard
    /// Navigation on, Tab stops at each, in order, as at every control (LLP
    /// 1115.000 D7.2). Off, each is its own AppKit control's answer, which
    /// follows the machine's setting and so is not asserted here.
    func testADateATimeAndASelectAreStopsWithKeyboardNavigationOn() {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        window.contentView = p.viewport
        func row(_ id: Int, _ kind: String, _ props: [String: String]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": kind, "props": props, "handlers": ["change"], "style": ["appearance": "auto"]],
             ["op": "frame", "id": id, "x": 10, "y": id * 40, "w": 200, "h": 30]]
        }
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view"], ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 500]]
        ops += row(2, "input", ["value": ""])
        ops += row(3, "control", ["type": "date", "value": "2026-10-08"])
        ops += row(4, "control", ["type": "time", "value": "09:30"])
        ops += row(5, "control", ["type": "select", "value": "a", "options": "[{\"label\":\"A\",\"value\":\"a\"}]"])
        ops += row(6, "input", ["value": ""])
        ops += [["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6]], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        keyboardNavigation(true)
        XCTAssertEqual(stops(p), [2, 3, 4, 5, 6], "on: every control")
    }

    func testOffTabMovesBetweenFieldsAndWhatTakesKeysOfItsOwn() throws {
        keyboardNavigation(false)
        let p = fixture()
        XCTAssertEqual(stops(p), [2, 6, 7, 9, 10], "the fields, the box that takes keys, and what the author gave a tabindex")
        let native = try XCTUnwrap(p.controls.controls[3] as? NativeButtonMac)
        XCTAssertTrue(native.acceptsFirstResponder, "an action's focus() and autofocus still reach a button")
        XCTAssertFalse(native.canBecomeKeyView, "as an NSButton is no key view with Keyboard Navigation off")
        for id in [UInt32(4), 5, 8] { XCTAssertFalse(try XCTUnwrap(p.views[id]).canBecomeKeyView, "\(id)") }
        XCTAssertTrue(try XCTUnwrap(p.views[6]).canBecomeKeyView, "a custom view that accepts first responder stays in AppKit's loop")
        XCTAssertTrue(try XCTUnwrap(p.views[7]).canBecomeKeyView, "tabindex=0 wins")
        XCTAssertTrue(try XCTUnwrap(p.controls.controls[10] as? NativeButtonMac).canBecomeKeyView, "on a native button too")
        // Tab from a button an action focused goes on to the next stop after it, and Shift-Tab to the one before.
        p.focusElement(["native"])
        XCTAssertTrue(window.firstResponder === native)
        try tab(p)
        XCTAssertTrue(window.firstResponder === p.views[6], "past the custom button and the pressable box")
        try tab(p)
        XCTAssertTrue(window.firstResponder === p.views[7])
        p.focusElement(["custom"])
        XCTAssertTrue(window.firstResponder === p.views[4])
        try tab(p, shift: true)
        XCTAssertNotNil(try XCTUnwrap(p.views[2]?.field).currentEditor(), "Shift-Tab from a button that is no stop: the field before it")
        // A tabindex=-1 box: focusable, never a stop, and Tab leaves it for the stop after it,
        // Shift-Tab for the one before (it used to keep the focus for good:
        // issues/closed/20261010-macos-tab-from-a-focused-box.md).
        p.focusElement(["skipped"])
        XCTAssertTrue(window.firstResponder === p.views[8])
        try tab(p)
        XCTAssertNotNil(try XCTUnwrap(p.views[9]?.field).currentEditor())
        p.focusElement(["skipped"])
        XCTAssertTrue(window.firstResponder === p.views[8])
        try tab(p, shift: true)
        XCTAssertTrue(window.firstResponder === p.views[7], "back to the stop before it")
    }

    /// A focused `tabindex=-1` box is where Tab starts, as on the web (HTML's
    /// sequential focus navigation starting point): Tab goes to the next stop
    /// after it in tree order, its own first button when it holds some, and
    /// Shift-Tab to the stop before it, a positive `tabindex` included; with
    /// none before it, to the loop's last, as Tab wraps to its first
    /// (issues/closed/20261010-macos-tab-from-a-focused-box.md).
    func testTabFromAFocusedBoxThatIsNoStop() throws {
        keyboardNavigation(true)
        // A box (2) holding two buttons (3, 4), a button (5), a second box (6), and a button with tabindex=1 (7).
        var ops = row(2, "view", ["id": "box", "tabIndex": "-1"]) + row(3, "button", ["id": "first"], handlers: ["press"])
        ops += row(4, "button", ["id": "last"], handlers: ["press"]) + row(5, "button", ["id": "after"], handlers: ["press"])
        ops += row(6, "view", ["id": "q", "tabIndex": "-1"]) + row(7, "button", ["id": "pos", "tabIndex": "1"], handlers: ["press"])
        let p = page(ops + [["op": "children", "id": 2, "ids": [3, 4]], ["op": "children", "id": 1, "ids": [2, 5, 6, 7]]])
        XCTAssertEqual(stops(p), [7, 3, 4, 5], "tabindex=1 first; the boxes never")
        p.focusElement(["box"])
        try tab(p)
        XCTAssertTrue(window.firstResponder === p.views[3], "Tab from the box: its first button")
        p.focusElement(["box"])
        try tab(p, shift: true)
        XCTAssertTrue(window.firstResponder === p.views[5], "no stop before it: the loop's last, not the tree's (7)")
        p.focusElement(["q"])
        try tab(p)
        XCTAssertTrue(window.firstResponder === p.views[7], "the next stop in tree order")
        p.focusElement(["q"])
        try tab(p, shift: true)
        XCTAssertTrue(window.firstResponder === p.views[5])
        p.focusElement(["first"])
        try tab(p, shift: true)
        XCTAssertTrue(window.firstResponder === p.views[7], "Shift-Tab from a stop passes over the box")
    }

    /// With the window itself as first responder (nothing in it focused:
    /// after `blur()`, or a menu a click opened closed), Tab goes to the
    /// loop's first stop and Shift-Tab to its last, as AppKit goes from such a
    /// window to its initial first responder, in both Keyboard Navigation
    /// modes. It used to focus nothing.
    func testTabFromTheWindowItselfGoesToTheFirstStopAndShiftTabToTheLast() throws {
        // A button (2), a field (3), a box with `tabindex=1` (4) and a button (5).
        let ops = row(2, "button", ["id": "go"], handlers: ["press"]) + row(3, "input", ["value": "", "id": "name"])
            + row(4, "view", ["id": "pos", "tabIndex": "1"]) + row(5, "button", ["id": "end"], handlers: ["press"])
        // (Keyboard Navigation, the stops, the first and the last)
        for (on, order, first, last) in [(false, [UInt32(4), 3], UInt32(4), UInt32(3)), (true, [4, 2, 3, 5], 4, 5)] {
            keyboardNavigation(on)
            window?.close()
            let p = page(ops + [["op": "children", "id": 1, "ids": [2, 3, 4, 5]]])
            XCTAssertEqual(stops(p), order, "on: \(on)")
            func holds(_ id: UInt32) -> Bool {
                let field = p.views[id]?.field
                return field.map { window.firstResponder === $0.currentEditor() } ?? (window.firstResponder === p.views[id])
            }
            XCTAssertTrue(window.makeFirstResponder(nil))
            XCTAssertTrue(window.firstResponder === window)
            try tab(p)
            XCTAssertTrue(holds(first), "on: \(on); Tab: the loop's first stop, a positive tabindex's before the tree's first")
            try tab(p)
            XCTAssertTrue(holds(order[1]), "on: \(on); and on through the loop")
            XCTAssertTrue(window.makeFirstResponder(nil))
            try tab(p, shift: true)
            XCTAssertTrue(holds(last), "on: \(on); Shift-Tab: the loop's last stop")
            // A session that does not hear the window's keys leaves Tab to the one that does.
            XCTAssertTrue(window.makeFirstResponder(nil))
            let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                                       context: nil, characters: "\t", charactersIgnoringModifiers: "\t", isARepeat: false, keyCode: 48))
            XCTAssertFalse(p.routeKey(event, focused: false, in: window))
            XCTAssertTrue(window.firstResponder === window)
        }
    }

    /// Shift-Tab from the loop's first stop goes to its last, as Tab from
    /// the last goes to the first: AppKit's key view loop wraps both ways.
    /// In both Keyboard Navigation modes, from a field, a button and a native
    /// button; it used to keep the focus where it was.
    func testShiftTabFromTheFirstStopGoesToTheLast() throws {
        for on in [false, true] {
            keyboardNavigation(on)
            window?.close()
            let p = fixture()
            let last = try XCTUnwrap(p.controls.controls[10])
            p.focusElement(["a"])
            try tab(p, shift: true)
            XCTAssertTrue(window.firstResponder === last, "on: \(on); from the first field: the loop's last stop")
            try tab(p)
            XCTAssertNotNil(try XCTUnwrap(p.views[2]?.field).currentEditor(), "on: \(on); and Tab from it back to the first")
        }
        // A button first, a native one first (Keyboard Navigation on: buttons are stops).
        for first in [row(2, "button", ["id": "first"], handlers: ["press"]),
                      row(2, "control", ["type": "button", "id": "first"], handlers: ["press"], style: ["appearance": "auto"])] {
            window?.close()
            let p = page(first + row(3, "input", ["value": "", "id": "name"]) + row(4, "button", ["id": "end"], handlers: ["press"])
                         + [["op": "children", "id": 1, "ids": [2, 3, 4]]])
            XCTAssertEqual(stops(p), [2, 3, 4])
            p.focusElement(["first"])
            try tab(p, shift: true)
            XCTAssertTrue(window.firstResponder === p.views[4], "Shift-Tab from the first button: the last")
            try tab(p)
            XCTAssertTrue(window.firstResponder === p.keyView(of: try XCTUnwrap(p.views[2])), "Tab from the last: the first")
        }
    }

    /// The same in a dialog's sheet and in a popover AppKit shows, each a
    /// loop of its own: AppKit's links say the last stop is before the first,
    /// and Shift-Tab from the first goes there.
    func testShiftTabFromTheFirstStopOfADialogAndOfAPopover() throws {
        keyboardNavigation(true)
        let f = try SheetFixture()
        addTeardownBlock { f.close() }
        let p = f.presenter
        f.window.setFrameOrigin(NSPoint(x: 200, y: 120))
        f.window.orderFrontRegardless()
        f.settle()
        func shiftTab(in window: NSWindow) throws {
            let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .shift, timestamp: 0, windowNumber: window.windowNumber,
                                                       context: nil, characters: "\u{19}", charactersIgnoringModifiers: "\u{19}", isARepeat: false, keyCode: 48))
            p.flushKeyViewLoop()
            if !p.routeKey(event, focused: true, in: window) { window.sendEvent(event) }
            f.settle()
        }
        // The dialog: Title first, Save last.
        try f.press("open")
        let sheet = try XCTUnwrap(f.window.attachedSheet)
        let title = try XCTUnwrap(try f.node("title").field), save = p.keyView(of: try f.node("save"))
        p.flushKeyViewLoop()
        XCTAssertTrue(title.previousValidKeyView === save, "the dialog's loop: Save is before Title")
        XCTAssertTrue(sheet.makeFirstResponder(title))
        try shiftTab(in: sheet)
        XCTAssertTrue(sheet.firstResponder === save, "Shift-Tab from Title: Save")
        try f.press("cancel")
        // The popover: its field first, Done last.
        try f.press("tags")
        let native = try XCTUnwrap(p.menus.windowed.first)
        let popover = try XCTUnwrap(native.window)
        let tag = try XCTUnwrap(try f.node("tag").field), done = p.keyView(of: try f.node("done"))
        p.flushKeyViewLoop()
        XCTAssertTrue(tag.previousValidKeyView === done, "the popover's loop: Done is before the field")
        XCTAssertTrue(popover.makeFirstResponder(tag))
        try shiftTab(in: popover)
        XCTAssertTrue(popover.firstResponder === done, "Shift-Tab from the field: Done")
        try f.press("tags")
    }

    func testOnTabReachesEveryControl() throws {
        keyboardNavigation(true)
        let p = fixture()
        XCTAssertEqual(stops(p), [2, 3, 4, 5, 6, 7, 9, 10], "every control in tree order; tabindex=-1 never")
        XCTAssertTrue(try XCTUnwrap(p.controls.controls[3] as? NativeButtonMac).canBecomeKeyView)
        XCTAssertFalse(try XCTUnwrap(p.views[8]).canBecomeKeyView, "a negative tabindex wins too")
        p.focusElement(["native"])
        try tab(p)
        XCTAssertTrue(window.firstResponder === p.views[4])
        try tab(p, shift: true)
        XCTAssertTrue(window.firstResponder === p.controls.controls[3])
    }

    /// The setting is read at each use: a change in System Settings is the
    /// next Tab's, with no batch in between.
    func testAChangeOfTheSettingIsTheNextTabs() throws {
        keyboardNavigation(false)
        let p = fixture()
        XCTAssertEqual(stops(p), [2, 6, 7, 9, 10])
        XCTAssertFalse(p.keyViewLoopStale)
        KeyboardNavigation.agent = true
        XCTAssertEqual(stops(p), [2, 3, 4, 5, 6, 7, 9, 10], "the loop is rebuilt for the setting as it is now")
        KeyboardNavigation.agent = false
        p.focusElement(["a"])
        try tab(p)
        XCTAssertTrue(window.firstResponder === p.views[6])
    }

    /// A dialog's Tab trap walks the same stops: with the setting off, its
    /// fields; with only buttons in it, the dialog itself holds the focus.
    func testADialogsTabFollowsTheSettingToo() throws {
        keyboardNavigation(false)
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "props": ["semanticTag": "dialog", "id": "ask"]],
            ["op": "frame", "id": 20, "x": 0, "y": 0, "w": 300, "h": 200],
            ["op": "create", "id": 21, "kind": "button", "props": ["id": "ok"], "handlers": ["press"]],
            ["op": "frame", "id": 21, "x": 10, "y": 10, "w": 100, "h": 30],
            ["op": "create", "id": 22, "kind": "input", "props": ["value": "", "id": "name"]],
            ["op": "frame", "id": 22, "x": 10, "y": 50, "w": 200, "h": 30],
            ["op": "children", "id": 20, "ids": [21, 22]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7, 8, 9, 10, 20]],
        ]))
        let dialog = try XCTUnwrap(p.views[20])
        p.dialogs.show(dialog)
        XCTAssertNotNil(try XCTUnwrap(p.views[22]?.field).currentEditor(), "the dialog's first key view is its field, not the button before it")
        try tab(p)
        XCTAssertNotNil(try XCTUnwrap(p.views[22]?.field).currentEditor(), "the field is the only stop")
        p.dialogs.close(dialog)
        KeyboardNavigation.agent = true
        p.dialogs.show(dialog)
        // In its sheet (LLP 1115.002 D7.2): the sheet's window holds its focus.
        XCTAssertTrue(window.attachedSheet === dialog.window)
        XCTAssertTrue(dialog.window?.firstResponder === p.views[21], "on, the button comes first")
        p.dialogs.close(dialog)
    }
}
#endif

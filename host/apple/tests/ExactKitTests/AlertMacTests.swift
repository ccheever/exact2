// @ref LLP 1115.000 D7.6 — a confirmation in the alert shape is NSAlert, a
// sheet on its window; a chooser stays the menu, anything else as it was.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class AlertMacTests: XCTestCase {
    private var window: NSWindow!
    // AppKit slides a sheet in and out inside `beginSheet` and `endSheet`,
    // some 0.3 s each, whatever the window's animation behaviour: a case
    // puts one up only for what needs the sheet, and reads the rest from
    // `AlertHost.plan`.
    override func tearDown() { window?.close(); window = nil }

    private func text(_ id: Int, _ value: String) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "text", "props": ["text": value]], ["op": "frame", "id": id, "x": 0, "y": 0, "w": 180, "h": 20]]
    }
    /// A page (1) with an opener (2) and an `alertdialog` (3, id `ask`): its
    /// `lines` (texts from 40), its `actions` by title (buttons from 10,
    /// their labels from 20) and a cancel (30, label 31). `popover` makes it
    /// a popover its opener invokes instead of a `dialog`; `extra` adds a
    /// row that is none of those (an input, id 50).
    private func fixture(label: String? = "Remove Book?", lines: [String] = ["“Piranesi” will be removed from your library."],
                         actions: [String] = ["Remove"], props: [Int: [String: String]] = [10: ["destructive": "true"]],
                         filled: Set<UInt32> = [], cancel: Bool = true, popover: Bool = false, extra: Bool = false) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.buttonFace = { id in var face = ButtonFace(); if filled.contains(id) { face.style = "filled" }; return face }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        let closing: [String: String] = popover ? ["popovertarget": "ask", "popovertargetaction": "hide"] : ["commandfor": "ask", "command": "close"]
        var dialog: [String: String] = ["id": "ask", "accessibilityRole": "alertdialog"]
        if popover { dialog["popover"] = "auto" } else { dialog["semanticTag"] = "dialog"; dialog["closedby"] = "any" }
        if let label { dialog["accessibilityLabel"] = label }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": popover ? ["popovertarget": "ask"] : ["commandfor": "ask", "command": "show-modal"]],
            ["op": "create", "id": 3, "kind": "view", "props": dialog],
        ]
        var rows: [Int] = []
        for (i, line) in lines.enumerated() { ops += text(40 + i, line); rows.append(40 + i) }
        for (i, title) in actions.enumerated() {
            ops += text(20 + i, title)
            ops += [["op": "create", "id": 10 + i, "kind": "button", "handlers": ["press"], "props": closing.merging(props[10 + i] ?? [:]) { $1 }],
                    ["op": "frame", "id": 10 + i, "x": 0, "y": 0, "w": 100, "h": 30], ["op": "children", "id": 10 + i, "ids": [20 + i]]]
            rows.append(10 + i)
        }
        if cancel {
            ops += text(31, "Not Now")
            ops += [["op": "create", "id": 30, "kind": "button", "props": closing.merging(props[30] ?? [:]) { $1 }],
                    ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 100, "h": 30], ["op": "children", "id": 30, "ids": [31]]]
            rows.append(30)
        }
        if extra { ops += [["op": "create", "id": 50, "kind": "input", "props": ["value": ""]], ["op": "frame", "id": 50, "x": 0, "y": 0, "w": 100, "h": 30]]; rows.append(50) }
        ops += [["op": "children", "id": 3, "ids": rows], ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 500, "h": 400], ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 100, "h": 30],
                ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 240, "h": 160]]
        p.apply(wireBatch(ops))
        return p
    }
    /// A chosen button presses on the next main-queue turn: let every turn
    /// queued so far run (the main queue is FIFO).
    private func turn() {
        let turned = expectation(description: "the next turn")
        DispatchQueue.main.async { turned.fulfill() }
        wait(for: [turned], timeout: 5)
    }
    private func key(_ chars: String, _ code: UInt16, in window: NSWindow) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil,
                         characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)!
    }

    func testTheShapeAndItsWordsAreTheRuleIOSFollows() {
        XCTAssertTrue(ConfirmationShape.isAlert(texts: 1, actions: 1, cancels: 1))
        XCTAssertTrue(ConfirmationShape.isAlert(texts: 2, actions: 2, cancels: 1))
        XCTAssertFalse(ConfirmationShape.isAlert(texts: 1, actions: 3, cancels: 1), "more than three buttons")
        XCTAssertFalse(ConfirmationShape.isAlert(texts: 0, actions: 1, cancels: 1), "a chooser has no text")
        XCTAssertFalse(ConfirmationShape.isAlert(texts: 1, actions: 1, cancels: 0), "no cancel: nothing to keep")
        var words = ConfirmationShape.alertText(label: "Remove Book?", lines: ["a", "b"])
        XCTAssertEqual(words.title, "Remove Book?"); XCTAssertEqual(words.message, "a\nb")
        words = ConfirmationShape.alertText(label: nil, lines: ["First line", "the rest"])
        XCTAssertEqual(words.title, "First line"); XCTAssertEqual(words.message, "the rest")
        words = ConfirmationShape.alertText(label: nil, lines: ["Only"])
        XCTAssertEqual(words.title, "Only"); XCTAssertNil(words.message)
        // Exactly one filled, non-destructive button is the default.
        XCTAssertEqual(AlertHost.defaultRow(filled: [false, true, false], destructive: [false, false, false]), 1)
        XCTAssertNil(AlertHost.defaultRow(filled: [false, false], destructive: [true, false]), "none")
        XCTAssertNil(AlertHost.defaultRow(filled: [true, true, false], destructive: [false, false, false]), "several: emphasis alone never picks")
        XCTAssertNil(AlertHost.defaultRow(filled: [true, false], destructive: [true, false]), "a destructive button is never the default")
        XCTAssertEqual(AlertHost.defaultRow(filled: [true, true, false], destructive: [true, false, false]), 1)
    }

    /// Shelf's Remove: a `dialog role="alertdialog"` shown by its command
    /// (or `showModal`) is a sheet, its title the label, its message the
    /// text, its destructive button marked, its cancel on Escape, and no
    /// default, as AppKit leaves a destructive alert.
    func testAnAlertShapedDialogIsAnNSAlertSheetAndEachButtonPressesItsOwn() throws {
        let p = fixture()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let dialog = try XCTUnwrap(p.views[3])
        p.press(2)
        let owner = try XCTUnwrap(p.alerts.presented)
        let alert = owner.alert
        XCTAssertTrue(window.attachedSheet === alert.window, "a sheet on its window")
        XCTAssertNil(p.dialogs.active, "not the painted dialog")
        XCTAssertTrue(dialog.isHidden)
        XCTAssertEqual(alert.messageText, "Remove Book?")
        XCTAssertEqual(alert.informativeText, "“Piranesi” will be removed from your library.")
        XCTAssertEqual(alert.buttons.map(\.title), ["Remove", "Not Now"])
        XCTAssertEqual(alert.buttons.map(\.hasDestructiveAction), [true, false])
        XCTAssertEqual(alert.buttons.map(\.keyEquivalent), ["", "\u{1b}"], "no default; Escape is the authored cancel's, whatever its title")
        XCTAssertNil(owner.defaultRow)
        XCTAssertNil(alert.window.defaultButtonCell, "AppKit's first-button default is not kept")
        XCTAssertEqual(p.dialogs.observation?["kind"] as? String, "alert")
        XCTAssertEqual(p.dialogs.observation?["buttons"] as? [String], ["Remove", "Not Now"])
        // showModal on an open modal dialog does nothing.
        p.dialogCommand("showModal", "ask")
        XCTAssertTrue(p.alerts.presented === owner)
        XCTAssertNil(p.dialogs.active)
        XCTAssertFalse(p.alerts.key("Enter"), "Return has no button to press")
        XCTAssertTrue(p.alerts.presented === owner)
        alert.buttons[0].performClick(nil)
        XCTAssertNil(window.attachedSheet, "the sheet has ended")
        XCTAssertNil(p.alerts.presented)
        XCTAssertEqual(pressed, [], "not inside the alert's own action")
        turn()
        XCTAssertEqual(pressed, [10], "the authored button, once")
        // Escape, by the key AppKit binds: the cancel has nothing to press.
        p.dialogCommand("showModal", "ask")
        let again = try XCTUnwrap(p.alerts.presented).alert
        again.window.sendEvent(key("\u{1b}", 53, in: again.window))
        XCTAssertNil(window.attachedSheet)
        XCTAssertNil(p.alerts.presented)
        turn()
        XCTAssertEqual(pressed, [10])
    }

    func testWithNoLabelTheFirstLineIsTheTitleAndAPopoverFromItsInvokerIsTheSameAlert() throws {
        let p = fixture(label: nil, lines: ["Discard changes?", "They cannot be recovered.", "This note only."], actions: ["Discard", "Save"],
                        props: [10: ["destructive": "true"]], popover: true)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.press(2)
        let alert = try XCTUnwrap(p.alerts.presented).alert
        XCTAssertTrue(window.attachedSheet === alert.window)
        XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[3])), "neither a menu nor painted")
        XCTAssertNil(p.menus.observation)
        XCTAssertEqual(alert.messageText, "Discard changes?")
        XCTAssertEqual(alert.informativeText, "They cannot be recovered.\nThis note only.")
        XCTAssertEqual(alert.buttons.map(\.title), ["Discard", "Save", "Not Now"])
        alert.buttons[1].performClick(nil)
        turn()
        XCTAssertEqual(pressed, [11])
    }

    /// The default (LLP 1115.000 D7.3): exactly one `filled`, non-destructive
    /// button answers Return and is the one AppKit draws so; with several, or
    /// only a destructive one, nothing is.
    func testExactlyOneFilledNonDestructiveButtonIsTheDefault() throws {
        let p = fixture(label: "Save changes?", lines: ["Your draft has edits."], actions: ["Don’t Save", "Save"], props: [:], filled: [11])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.press(2)
        let owner = try XCTUnwrap(p.alerts.presented)
        XCTAssertEqual(owner.defaultRow, 1)
        // AppKit holds a default button's Return while its window is key; which
        // button that is, is the window's default button cell.
        XCTAssertTrue(owner.alert.window.defaultButtonCell === owner.alert.buttons[1].cell, "AppKit's own default button, drawn so")
        XCTAssertEqual(owner.alert.buttons[2].keyEquivalent, "\u{1b}")
        XCTAssertEqual(p.dialogs.observation?["default"] as? String, "Save")
        XCTAssertTrue(p.alerts.key("Enter"))
        XCTAssertNil(window.attachedSheet)
        turn()
        XCTAssertEqual(pressed, [11], "Return pressed the default")
        window.close()
        // What the alert would show, with no sheet put up for each.
        let several = fixture(label: "Keep which?", actions: ["Mine", "Theirs"], props: [:], filled: [10, 11])
        XCTAssertNil(try XCTUnwrap(several.alerts.plan(of: try XCTUnwrap(several.views[3]))).defaultRow, "several: emphasis alone never picks among actions")
        window.close()
        let destructive = fixture(filled: [10])
        XCTAssertNil(try XCTUnwrap(destructive.alerts.plan(of: try XCTUnwrap(destructive.views[3]))).defaultRow, "a filled destructive button is not a default")
        window.close()
        // Emphasis on the cancel: AppKit gives a cancel Escape and never
        // Return, and a button has one key, so the alert has no Return default.
        let cautious = fixture(filled: [30])
        let safe = try XCTUnwrap(cautious.alerts.plan(of: try XCTUnwrap(cautious.views[3])))
        XCTAssertNil(safe.defaultRow)
        XCTAssertEqual(safe.rows.map(\.cancel), [false, true])
    }

    /// Any other shape is presented as before: more than three buttons or
    /// no cancel is the chooser's menu (ChooserMacTests), and a `dialog`
    /// holding anything but text and buttons, or with no text, is painted.
    func testOtherShapesAreNotAlerts() throws {
        let form = fixture(extra: true)
        form.press(2)
        XCTAssertNil(form.alerts.presented)
        XCTAssertTrue(form.dialogs.active === form.views[3], "a dialog with a field is the painted dialog")
        XCTAssertNil(window.attachedSheet)
        form.dialogs.reset()
        window.close()
        let wordless = fixture(lines: [])
        wordless.press(2)
        XCTAssertNil(wordless.alerts.presented, "an alert has text")
        XCTAssertTrue(wordless.dialogs.active === wordless.views[3])
        wordless.dialogs.reset()
        window.close()
        let menus: [(String, () -> Presenter)] = [
            ("four buttons", { self.fixture(actions: ["A", "B", "C"], props: [:], popover: true) }),
            ("no cancel", { self.fixture(cancel: false, popover: true) }),
        ]
        for (name, make) in menus {
            let p = make()
            XCTAssertNil(p.alerts.plan(of: try XCTUnwrap(p.views[3])), name)
            XCTAssertNotNil(p.menus.confirmation(of: try XCTUnwrap(p.views[3]), from: try XCTUnwrap(p.views[2])), "\(name): the menu's to present")
            window.close()
        }
        // An action that does not also close it is no alert's button.
        let open = fixture(props: [10: ["command": "request-close"]])
        XCTAssertNil(open.alerts.plan(of: try XCTUnwrap(open.views[3])))
    }

    /// The alert ends, with nothing pressed, when a batch changes what it
    /// shows, when the app closes it, and when the session resets; a button
    /// chosen but gone before its turn presses nothing.
    func testAnAlertThatNoLongerShowsWhatIsThereEndsAndPressesNothing() throws {
        let p = fixture()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        func set(_ id: Int, _ props: [String: String], clear: [String] = []) { p.apply(wireBatch([["op": "props", "id": id, "set": props, "clear": clear]])) }
        p.press(2)
        let owner = try XCTUnwrap(p.alerts.presented)
        XCTAssertTrue(p.alerts.valid(owner))
        set(20, ["text": "Delete"])
        XCTAssertNil(p.alerts.presented, "a button now says something else: the sheet ends")
        XCTAssertNil(window.attachedSheet)
        // Each thing that makes it no longer what was shown, with no sheet put up for each.
        XCTAssertFalse(p.alerts.valid(owner))
        set(20, ["text": "Remove"]); XCTAssertTrue(p.alerts.valid(owner))
        set(10, ["disabled": "true"]); XCTAssertFalse(p.alerts.valid(owner), "a button is disabled")
        set(10, ["disabled": "false"]); XCTAssertTrue(p.alerts.valid(owner))
        set(30, [:], clear: ["command"]); XCTAssertFalse(p.alerts.valid(owner), "its cancel no longer closes it")
        set(30, ["command": "close"]); XCTAssertTrue(p.alerts.valid(owner))
        set(3, [:], clear: ["accessibilityRole"]); XCTAssertFalse(p.alerts.valid(owner), "no longer an alertdialog")
        set(3, ["accessibilityRole": "alertdialog"]); XCTAssertTrue(p.alerts.valid(owner))
        turn()
        XCTAssertEqual(pressed, [])
        // The app's `close(id)`, then a reset.
        p.press(2)
        XCTAssertNotNil(p.alerts.presented)
        p.dialogCommand("close", "ask")
        XCTAssertNil(p.alerts.presented)
        XCTAssertNil(window.attachedSheet)
        p.press(2)
        XCTAssertNotNil(p.alerts.presented)
        // Chosen, then gone before its turn.
        try XCTUnwrap(p.alerts.presented).alert.buttons[0].performClick(nil)
        set(10, ["disabled": "true"])
        turn()
        XCTAssertEqual(pressed, [])
        set(10, ["disabled": "false"])
        p.press(2)
        XCTAssertNotNil(p.alerts.presented)
        p.reset()
        XCTAssertNil(p.alerts.presented)
        XCTAssertNil(window.attachedSheet)
        turn()
        XCTAssertEqual(pressed, [])
    }

    /// The driver's tap on an authored button of the alert is AppKit's click
    /// of the alert's button; any other node is not the alert's.
    func testTheDriverPressesTheAlertsButtonForAnAuthoredOne() throws {
        let p = fixture()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertNil(p.alerts.click(try XCTUnwrap(p.views[10])), "no alert is up")
        p.press(2)
        XCTAssertNil(p.alerts.click(try XCTUnwrap(p.views[2])), "the opener is behind the sheet")
        XCTAssertEqual(p.alerts.click(try XCTUnwrap(p.views[10])), true)
        XCTAssertNil(window.attachedSheet)
        turn()
        XCTAssertEqual(pressed, [10])
        p.press(2)
        XCTAssertTrue(p.alerts.key("Escape"))
        XCTAssertNil(p.alerts.presented)
        turn()
        XCTAssertEqual(pressed, [10])
    }
}
#endif

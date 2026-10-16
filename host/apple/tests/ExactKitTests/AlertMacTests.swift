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
    /// row that is none of those (a checkbox, id 50), `field` a prompt's
    /// text field (id 51, "Untitled"). A dialog's `closedby` is
    /// what lowering implies for the alert's shape, `closerequest` (LLP
    /// 1115.000 D13.4), unless the test says another.
    private func fixture(label: String? = "Remove Book?", lines: [String] = ["“Piranesi” will be removed from your library."],
                         actions: [String] = ["Remove"], props: [Int: [String: String]] = [10: ["destructive": "true"]],
                         filled: Set<UInt32> = [], cancel: Bool = true, popover: Bool = false, extra: Bool = false,
                         field: Bool = false, closedby: String = "closerequest", session: ExactSession? = nil) -> Presenter {
        _ = NSApplication.shared
        let p = session?.presenter ?? Presenter()
        p.buttonFace = { id in var face = ButtonFace(); if filled.contains(id) { face.style = "filled" }; return face }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        if let session { window.contentView = ExactView(session: session) }
        else { window.contentView = p.viewport }
        let closing: [String: String] = popover ? ["popovertarget": "ask", "popovertargetaction": "hide"] : ["commandfor": "ask", "command": "close"]
        var dialog: [String: String] = ["id": "ask", "accessibilityRole": "alertdialog"]
        if popover { dialog["popover"] = "auto" } else { dialog["semanticTag"] = "dialog"; dialog["closedby"] = closedby }
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
        if extra { ops += [["op": "create", "id": 50, "kind": "input", "props": ["type": "checkbox"]], ["op": "frame", "id": 50, "x": 0, "y": 0, "w": 100, "h": 30]]; rows.append(50) }
        if field {
            ops += [["op": "create", "id": 51, "kind": "input", "handlers": ["input", "change", "submit"], "props": ["value": "Untitled", "placeholder": "Name"]],
                    ["op": "frame", "id": 51, "x": 0, "y": 0, "w": 100, "h": 30]]
            rows.insert(51, at: 0)
        }
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

    /// The rule itself is the kernel's, asked through the C ABI
    /// (`exact_confirmation_is_alert`): one rule for lowering, Linux and here.
    func testTheShapeAndItsWordsAreTheRuleIOSFollows() {
        XCTAssertTrue(ConfirmationShape.isAlert(texts: 1, actions: 1, cancels: 1))
        XCTAssertTrue(ConfirmationShape.isAlert(texts: 1, actions: 0, cancels: 1), "text and a cancel alone")
        XCTAssertFalse(ConfirmationShape.isAlert(texts: 1, actions: 1, cancels: 2), "two cancels")
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
        dialog.handlers.insert("close")
        var closed: [UInt32] = []
        p.onClose = { closed.append($0) }
        p.press(2)
        let owner = try XCTUnwrap(p.alerts.presented)
        let alert = owner.alert
        XCTAssertTrue(window.attachedSheet === alert.window, "a sheet on its window")
        XCTAssertNil(p.dialogs.active, "not the painted dialog")
        XCTAssertEqual(p.openTopLayerFrames.map(\.0), [3], "the runner must see the open alertdialog")
        XCTAssertEqual(p.openTopLayer.modal, 0)
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
        XCTAssertTrue(p.openTopLayerFrames.isEmpty)
        XCTAssertEqual(pressed, [], "not inside the alert's own action")
        turn()
        XCTAssertEqual(pressed, [10], "the authored button, once")
        turn()
        XCTAssertEqual(closed, [3])
        // Escape, by the key AppKit binds: the cancel has nothing to press.
        p.dialogCommand("showModal", "ask")
        let again = try XCTUnwrap(p.alerts.presented).alert
        again.window.sendEvent(key("\u{1b}", 53, in: again.window))
        XCTAssertNil(window.attachedSheet)
        XCTAssertNil(p.alerts.presented)
        turn()
        XCTAssertEqual(pressed, [10])
        XCTAssertEqual(closed, [3, 3])
    }

    func testDismissalStillClosesWhenTheChosenActionChangesBeforeItsTurn() throws {
        let p = fixture()
        p.views[3]?.handlers.insert("close")
        var pressed: [UInt32] = [], closed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.onClose = { closed.append($0) }
        for (id, changed, restored) in [(20, ["text": "Delete"], ["text": "Remove"]),
                                        (10, ["disabled": "true"], ["disabled": "false"])] {
            let before = closed.count
            p.press(2)
            try XCTUnwrap(p.alerts.presented).alert.buttons[0].performClick(nil)
            XCTAssertNil(window.attachedSheet)
            p.apply(wireBatch([["op": "props", "id": id, "set": changed, "clear": []]]))
            turn()
            turn()
            XCTAssertEqual(pressed, [], "a stale action must not press")
            XCTAssertEqual(closed.count, before + 1, "the mounted dialog already closed")
            XCTAssertEqual(closed.last, 3)
            p.apply(wireBatch([["op": "props", "id": id, "set": restored, "clear": []]]))
        }
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

    func testPopoverCommandsRetainExplicitSourceWithoutInvoker() throws {
        let p = fixture(popover: true), trigger = try XCTUnwrap(p.views[2])
        trigger.props["id"] = "trigger"
        trigger.props["popovertarget"] = nil
        p.menus.action("showPopover", ["ask", "trigger"])
        let owner = try XCTUnwrap(p.alerts.presented)
        XCTAssertTrue(owner.source === trigger)
        XCTAssertTrue(p.alerts.valid(owner))
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [3])
        XCTAssertNil(p.openTopLayer.modal, "a confirmation popover leaves the page reachable")
        p.menus.action("showPopover", ["ask", "trigger"])
        XCTAssertTrue(p.alerts.presented === owner)
        p.menus.action("togglePopover", ["ask", "trigger"])
        XCTAssertNil(p.alerts.presented)
        p.menus.action("togglePopover", ["ask", "trigger"])
        XCTAssertNotNil(p.alerts.presented)
        XCTAssertNil(p.openTopLayer.modal)
        p.menus.action("hidePopover", ["ask"])

        p.apply(wireBatch([
            ["op": "create", "id": 100, "kind": "view", "props": ["id": "form", "semanticTag": "dialog"]],
            ["op": "frame", "id": 100, "x": 0, "y": 0, "w": 300, "h": 200],
            ["op": "children", "id": 1, "ids": [100]],
            ["op": "children", "id": 100, "ids": [2, 3]],
        ]))
        p.dialogCommand("showModal", "form")
        defer { p.dialogs.reset() }
        let sheet = p.openTopLayer
        XCTAssertNotNil(sheet.modal)
        p.menus.action("showPopover", ["ask", "trigger"])
        XCTAssertNotNil(p.alerts.presented)
        XCTAssertEqual(p.openTopLayer.frames.last?.0, 3)
        XCTAssertEqual(p.openTopLayer.modal, sheet.modal, "the sheet remains the modal boundary")
        p.menus.action("hidePopover", ["ask"])
    }

    /// A press handler that changes its own button's action: what the surviving
    /// button invokes after the handler runs, as HTML resolves it.
    func testAPressHandlerThatChangesItsButtonsActionRunsTheLiveInvocation() throws {
        let p = fixture(popover: true), trigger = try XCTUnwrap(p.views[2])
        trigger.handlers.insert("press")
        trigger.props["popovertargetaction"] = "hide"
        p.onPress = { _ in trigger.props["popovertargetaction"] = "show" }
        p.press(2)
        XCTAssertNotNil(p.alerts.presented, "the handler made it a show, and the show ran")
    }

    /// HTML's dialog `close` does nothing to a dialog showing as a popover.
    func testADialogCloseDoesNothingToADialogShownAsAPopover() throws {
        let p = fixture(popover: true), trigger = try XCTUnwrap(p.views[2])
        try XCTUnwrap(p.views[3]).props["semanticTag"] = "dialog"
        p.press(2)
        XCTAssertNotNil(p.alerts.presented, "opened as a popover")
        trigger.props["commandfor"] = "ask"
        trigger.props["command"] = "close"
        p.press(2)
        XCTAssertNotNil(p.alerts.presented, "a dialog close leaves the popover up")
    }

    /// HTML's popover commands do nothing to a dialog opened by `show-modal`.
    func testPopoverCommandsDoNothingToADialogShownModally() throws {
        let p = fixture(), trigger = try XCTUnwrap(p.views[2])
        try XCTUnwrap(p.views[3]).props["popover"] = "auto"
        p.press(2)
        XCTAssertNotNil(p.alerts.presented, "opened modally")
        trigger.props["command"] = "hide-popover"
        p.press(2)
        p.menus.action("hidePopover", ["ask"])
        XCTAssertNotNil(p.alerts.presented, "popover commands leave it open")
    }

    /// A row closes a dialog in the mode it shows in, as HTML's commands
    /// do: `close` a modal one, `hide-popover` a popover. An action that
    /// would not close it leaves no alert shape.
    func testAlertRowsCloseADialogInTheModeItShowsIn() throws {
        let asPopover = fixture(popover: true)
        try XCTUnwrap(asPopover.views[3]).props["semanticTag"] = "dialog"
        XCTAssertNotNil(asPopover.alerts.plan(of: try XCTUnwrap(asPopover.views[3]), popover: true), "hide rows close a popover")
        XCTAssertNil(asPopover.alerts.plan(of: try XCTUnwrap(asPopover.views[3]), popover: false), "and not a modal dialog")
        let modal = fixture()
        try XCTUnwrap(modal.views[3]).props["popover"] = "auto"
        XCTAssertNotNil(modal.alerts.plan(of: try XCTUnwrap(modal.views[3]), popover: false), "close rows close a modal dialog")
        XCTAssertNil(modal.alerts.plan(of: try XCTUnwrap(modal.views[3]), popover: true), "and not a popover")
        // What the row invokes decides: a command-only hide closes it, and a
        // `commandfor` naming another element governs over its `popovertarget`.
        let p = fixture(popover: true), pop = try XCTUnwrap(p.views[3]), cancel = try XCTUnwrap(p.views[30])
        try XCTUnwrap(p.views[1]).props["id"] = "page"
        cancel.props = ["commandfor": "ask", "command": "hide-popover"]
        XCTAssertNotNil(p.alerts.plan(of: pop, popover: true), "a command-only hide closes it")
        cancel.props = ["commandfor": "page", "command": "hide-popover", "popovertarget": "ask", "popovertargetaction": "hide"]
        XCTAssertNil(p.alerts.plan(of: pop, popover: true), "its commandfor governs: it closes nothing")
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
        XCTAssertNil(try XCTUnwrap(several.alerts.plan(of: try XCTUnwrap(several.views[3]), popover: false)).defaultRow, "several: emphasis alone never picks among actions")
        window.close()
        let destructive = fixture(filled: [10])
        XCTAssertNil(try XCTUnwrap(destructive.alerts.plan(of: try XCTUnwrap(destructive.views[3]), popover: false)).defaultRow, "a filled destructive button is not a default")
        window.close()
        // Emphasis on the cancel: AppKit gives a cancel Escape and never
        // Return, and a button has one key, so the alert has no Return default.
        let cautious = fixture(filled: [30])
        let safe = try XCTUnwrap(cautious.alerts.plan(of: try XCTUnwrap(cautious.views[3]), popover: false))
        XCTAssertNil(safe.defaultRow)
        XCTAssertEqual(safe.rows.map(\.cancel), [false, true])
    }

    /// A prompt (`kernel/src/confirmation.rs`): a text field among an
    /// alert's rows is the accessory view's field, which takes the keyboard
    /// first; its edits are the input's `input`, the alert's end its
    /// `change`, and Return presses the first action that is not destructive.
    func testAPromptIsTheAlertWithItsFieldAsTheAccessoryView() throws {
        let p = fixture(label: "Rename", lines: [], actions: ["Save"], props: [:], field: true)
        var events: [String] = []
        p.onInput = { id, value in events.append("input \(id) \(value)") }
        p.onChange = { id, value in events.append("change \(id) \(value)") }
        p.press(2)
        let owner = try XCTUnwrap(p.alerts.presented, "a field and its buttons are an alert's")
        let field = try XCTUnwrap((owner.alert.accessoryView as? NSStackView)?.arrangedSubviews.first as? NSTextField)
        XCTAssertEqual(field.stringValue, "Untitled")
        XCTAssertEqual(field.placeholderString, "Name")
        XCTAssertTrue(owner.alert.window.initialFirstResponder === field)
        XCTAssertEqual(owner.defaultRow, 0, "Return: Save")
        XCTAssertEqual(p.alerts.type(try XCTUnwrap(p.views[51]), "Kick"), "Kick")
        XCTAssertEqual(events, ["input 51 Kick"])
        p.alerts.reset()
        XCTAssertEqual(events, ["input 51 Kick", "change 51 Kick"])
        window.close()
    }

    /// A prompt opens with Save disabled until a name is typed and stays
    /// open when the name is cleared again; its field follows the authored
    /// input after each batch (the value the page holds, `readonly`).
    /// The accessory field is the input's own, lent as the toolbar's search
    /// field is: the node its delegate, its value the input's. Return is the
    /// input's `change` and its `submit`, and that is all; with no `submit`
    /// it is the default button's press. A batch writing nothing to the input
    /// leaves what was typed; an input the page swaps for another withdraws
    /// the alert.
    func testAPromptsFieldIsTheInputsOwnAndItsReturnSubmitsFirst() throws {
        let p = fixture(label: "Rename", lines: [], actions: ["Save"], props: [:], field: true)
        var events: [String] = []
        p.onChange = { id, value in events.append("change \(id) \(value)") }
        p.onSubmit = { events.append("submit \($0)") }
        p.onPress = { events.append("press \($0)") }
        p.press(2)
        let owner = try XCTUnwrap(p.alerts.presented)
        let field = try XCTUnwrap(p.views[51]?.field)
        XCTAssertTrue((owner.alert.accessoryView as? NSStackView)?.arrangedSubviews.first === field, "the input's field, lent")
        XCTAssertTrue(field.delegate === p.views[51], "the node is its delegate")
        XCTAssertEqual(p.alerts.type(try XCTUnwrap(p.views[51]), "Kick"), "Kick")
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["accessibilityLabel": "unrelated"]]]))
        p.alerts.sync()
        XCTAssertEqual(field.stringValue, "Kick", "a batch that writes nothing to the input leaves what was typed")
        XCTAssertTrue(p.alerts.key("Enter"))
        turn(); turn()
        XCTAssertEqual(events, ["change 51 Kick", "submit 51"], "a field with a submit keeps Return")
        XCTAssertTrue(p.alerts.presented === owner, "and nothing is pressed")
        try XCTUnwrap(p.views[51]).handlers.remove("submit")
        XCTAssertTrue(p.alerts.key("Enter"))
        turn(); turn()
        XCTAssertTrue(events.contains("press 10"), "with none, the default button: \(events)")
        XCTAssertFalse(p.views[51]?.field === field, "the page has its field again")
        window.close()

        let q = fixture(label: "Rename", lines: [], actions: ["Save"], props: [:], field: true)
        q.press(2)
        XCTAssertNotNil(q.alerts.presented)
        q.apply(wireBatch([["op": "create", "id": 52, "kind": "input", "props": ["value": ""]], ["op": "frame", "id": 52, "x": 0, "y": 0, "w": 100, "h": 30],
                           ["op": "children", "id": 3, "ids": [52, 10, 30]], ["op": "destroy", "id": 51]]))
        q.alerts.sync()
        XCTAssertNil(q.alerts.presented, "the field shown is no longer the input's: withdrawn")
        window.close()
    }

    /// The lent field edits in the session's editor (`beforeinput`); `focus`
    /// reaches it in the alert; and Return with Save disabled still goes
    /// through the field, its `change` enabling Save, which is then pressed.
    func testAPromptsReturnGoesThroughTheFieldWhileSaveIsDisabled() throws {
        let p = fixture(label: "Rename", lines: [], actions: ["Save"], props: [10: ["disabled": "true"]], field: true)
        let node = try XCTUnwrap(p.views[51])
        node.handlers.remove("submit")
        node.props["id"] = "name"
        var events: [String] = []
        p.onPress = { events.append("press \($0)") }
        p.onChange = { id, value in
            events.append("change \(id) \(value)")
            p.apply(wireBatch([["op": "props", "id": 10, "clear": ["disabled"]]]))
        }
        p.press(2)
        XCTAssertNotNil(p.alerts.presented)
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field.cell?.fieldEditor(for: field) === p.fieldEditor, "the session's editor, as in the page")
        p.focusElement(["name"])
        XCTAssertNotNil(field.currentEditor(), "focus(id) reaches the field in the alert")
        XCTAssertEqual(p.alerts.type(node, "Kick"), "Kick")
        XCTAssertTrue(p.alerts.key("Enter"), "Return is taken while Save is disabled")
        turn(); turn()
        XCTAssertEqual(events.first, "change 51 Kick")
        XCTAssertTrue(events.contains("press 10"), "the change enabled Save, which Return then pressed: \(events)")
        window.close()
    }

    func testAPromptHearsPreventableKeysThroughTheSessionsMonitorOnce() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let p = fixture(label: "Rename", lines: [], actions: ["Save"], props: [:], field: true, session: session)
        let node = try XCTUnwrap(p.views[51])
        node.handlers.formUnion(["key", "keyup"])
        var events: [String] = []
        var prevent = true
        p.onKey = { _, key in
            events.append("\(key.up ? "up" : "down") \(key.chord)")
            if !key.up && prevent { p.defaultPrevented = true }
        }
        p.onSubmit = { _ in events.append("submit") }
        p.onChange = { _, _ in events.append("change") }
        p.onInput = { _, _ in }
        window.makeKeyAndOrderFront(nil)
        p.press(2)
        defer { p.alerts.reset() }
        let sheet = try XCTUnwrap(p.alerts.sheet)
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(sheet.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor())
        editor.selectedRange = NSRange(location: editor.string.utf16.count, length: 0)
        func send(_ chars: String, _ code: UInt16) {
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                NSApp.sendEvent(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: 0,
                    windowNumber: sheet.windowNumber, context: nil, characters: chars,
                    charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)!)
            }
        }
        send("7", 26)
        XCTAssertEqual(editor.string, "Untitled", "a prevented digit does not insert")
        send("\u{7f}", 51)
        XCTAssertEqual(editor.string, "Untitled", "a prevented Backspace does not delete")
        send("\r", 36)
        XCTAssertEqual(events, ["down 7", "up 7", "down Backspace", "up Backspace", "down Enter", "up Enter"])
        XCTAssertEqual(editor.string, "Untitled", "prevented keys neither edit nor submit")
        events.removeAll()
        prevent = false
        send("7", 26)
        send("\r", 36)
        XCTAssertEqual(events, ["down 7", "up 7", "down Enter", "change", "submit", "up Enter"])
        XCTAssertEqual(field.stringValue, "Untitled7")
        XCTAssertTrue(field.currentEditor() === editor, "Return commits without ending editing")
    }

    func testAPromptFromAPopoverKeepsItsAncestorThroughClicksAndEscape() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let p = fixture(label: "Rename", lines: [], actions: ["Save"], props: [:], popover: true, field: true, session: session)
        p.apply(wireBatch([
            ["op": "create", "id": 60, "kind": "view", "props": ["id": "ancestor", "popover": "auto"]],
            ["op": "style", "id": 60, "style": ["position_try_fallbacks": "flip-block"]],
            ["op": "frame", "id": 60, "x": 0, "y": 0, "w": 240, "h": 160],
            ["op": "children", "id": 60, "ids": [2]],
            ["op": "children", "id": 1, "ids": [60, 3]],
        ]))
        let ancestor = try XCTUnwrap(p.views[60])
        window.makeKeyAndOrderFront(nil)
        defer { p.alerts.reset(); p.menus.reset() }
        for kind in ["left", "right", "Escape"] {
            p.menus.show(ancestor, from: nil)
            p.press(2)
            let sheet = try XCTUnwrap(p.alerts.sheet)
            let field = try XCTUnwrap(p.views[51]?.field)
            XCTAssertTrue(p.menus.isOpen(ancestor))
            XCTAssertTrue(sheet.makeFirstResponder(field))
            if kind == "Escape" {
                NSApp.sendEvent(key("\u{1b}", 53, in: sheet))
                XCTAssertNil(p.alerts.presented, "Escape reaches the alert's cancel")
            } else {
                let point = field.convert(NSPoint(x: field.bounds.midX, y: field.bounds.midY), to: nil)
                let types: [NSEvent.EventType] = kind == "left" ? [.leftMouseDown, .leftMouseUp] : [.rightMouseDown, .rightMouseUp]
                // These are the monitor's pointer deliveries; AppKit's field
                // editor may take the release inside its own tracking loop.
                for type in types {
                    p.menus.pointer(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: 0,
                        windowNumber: sheet.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 0)!)
                }
                p.apply(wireBatch([]))
                XCTAssertTrue(p.alerts.sheet === sheet, "a click in the prompt keeps its invoker live")
            }
            XCTAssertTrue(p.menus.isOpen(ancestor), "\(kind) belongs to the alert, not its ancestor")
            p.alerts.reset(); p.menus.reset()
            turn()
        }
    }

    func testAPromptOpensWithSaveDisabledAndItsFieldFollowsTheInput() throws {
        let p = fixture(label: "Rename", lines: [], actions: ["Save"], props: [10: ["disabled": "true"]], field: true)
        p.press(2)
        let owner = try XCTUnwrap(p.alerts.presented, "a prompt opens with every action disabled")
        XCTAssertEqual(owner.alert.buttons.map(\.isEnabled), [false, true], "the field and Cancel remain")
        let field = try XCTUnwrap((owner.alert.accessoryView as? NSStackView)?.arrangedSubviews.first as? NSTextField)
        p.apply(wireBatch([["op": "props", "id": 10, "clear": ["disabled"]], ["op": "props", "id": 51, "set": ["value": "KICK"]]]))
        p.alerts.sync()
        XCTAssertTrue(p.alerts.presented === owner)
        XCTAssertEqual(field.stringValue, "KICK", "the value the page now holds")
        XCTAssertTrue(owner.alert.buttons[0].isEnabled)
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["disabled": "true"]], ["op": "props", "id": 51, "set": ["value": "", "editable": "false"]]]))
        p.alerts.sync()
        XCTAssertTrue(p.alerts.presented === owner, "clearing the name dims Save and keeps the alert")
        XCTAssertFalse(owner.alert.buttons[0].isEnabled)
        XCTAssertFalse(field.isEditable, "readonly")
        XCTAssertNil(p.alerts.type(try XCTUnwrap(p.views[51]), "x"), "a readonly field takes no typing")
        p.alerts.reset()
        window.close()
        // Return is the first action that is not destructive, as on iOS and
        // the web: the author's emphasis does not move it in a prompt.
        let choice = fixture(label: "Save?", lines: [], actions: ["Don't Save", "Save"], props: [:], filled: [11], field: true)
        XCTAssertEqual(try XCTUnwrap(choice.alerts.plan(of: try XCTUnwrap(choice.views[3]), popover: false)).defaultRow, 0)
        window.close()
    }

    /// Any other shape is no alert: more than three buttons or no cancel is
    /// the chooser's menu (ChooserMacTests), and a `dialog` holding anything
    /// but text and buttons, or with no text, is a sheet that holds the
    /// dialog's own subtree (LLP 1115.002 D7.2, `DialogSheetMacTests`).
    func testOtherShapesAreNotAlerts() throws {
        let form = fixture(extra: true)
        form.press(2)
        XCTAssertNil(form.alerts.presented)
        XCTAssertTrue(form.dialogs.active === form.views[3], "a dialog with a checkbox is the dialog itself")
        XCTAssertTrue(window.attachedSheet === form.views[3]?.window, "in a sheet of its own, not the alert's")
        XCTAssertFalse(window.attachedSheet is NSPanel, "AppKit's alert is a panel; this is the dialog's window")
        form.dialogs.reset()
        XCTAssertNil(window.attachedSheet)
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
            XCTAssertNil(p.alerts.plan(of: try XCTUnwrap(p.views[3]), popover: true), name)
            XCTAssertNotNil(p.menus.confirmation(of: try XCTUnwrap(p.views[3]), from: try XCTUnwrap(p.views[2])), "\(name): the menu's to present")
            window.close()
        }
        // An action that does not also close it is no alert's button.
        let open = fixture(props: [10: ["command": "request-close"]])
        XCTAssertNil(open.alerts.plan(of: try XCTUnwrap(open.views[3]), popover: false))
    }

    /// LLP 1115.000 D13.4: an alert has no outside, and `NSAlert` has none:
    /// its sheet takes a click on the window behind it and gives Escape to
    /// its cancel. So that is the alert shape under the `closedby` lowering
    /// implies for it, `closerequest`. A `dialog` of the same rows whose
    /// author wrote `closedby="any"` asks for the dismissible form, which on
    /// the Mac is the dialog's own sheet (LLP 1115.002 D7.2): a click outside
    /// closes it. Under `closedby="none"` the alert's cancel has no Escape.
    func testAnAlertHasNoOutsideAndClosedbyAnyIsThePaintedDialog() throws {
        let alert = fixture()
        alert.press(2)
        let sheet = try XCTUnwrap(alert.alerts.presented)
        XCTAssertNil(alert.dialogs.active, "an `NSAlert`: nothing painted, so nothing outside to click")
        alert.dialogs.outside()
        XCTAssertTrue(alert.alerts.presented === sheet, "a click outside does nothing")
        XCTAssertTrue(alert.alerts.key("Escape"), "Escape presses the cancel")
        XCTAssertNil(alert.alerts.presented)
        window.close()

        let light = fixture(closedby: "any")
        let dialog = try XCTUnwrap(light.views[3])
        XCTAssertNil(light.alerts.plan(of: dialog, popover: false), "not an alert: an alert has no outside")
        light.press(2)
        XCTAssertNil(light.alerts.presented)
        XCTAssertTrue(light.dialogs.active === dialog, "the dialog itself")
        XCTAssertTrue(window.attachedSheet === dialog.window, "in a sheet that holds its subtree")
        XCTAssertEqual(light.dialogs.observation?["closedby"] as? String, "any")
        XCTAssertEqual(light.dialogs.observation?["presentation"] as? String, "sheet")
        // A click on the window behind it, both halves: the sheet's under `any`.
        let behind = NSPoint(x: 20, y: 20)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            let click = try XCTUnwrap(NSEvent.mouseEvent(with: type, location: behind, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                                         context: nil, eventNumber: 7, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0))
            XCTAssertTrue(light.dialogs.pointer(click), "the click is the dialog's, not the page's")
        }
        XCTAssertNil(light.dialogs.active, "a click outside closes it")
        XCTAssertNil(window.attachedSheet)
        window.close()

        let held = fixture(closedby: "none")
        held.press(2)
        let owner = try XCTUnwrap(held.alerts.presented)
        XCTAssertEqual(owner.alert.buttons.map(\.keyEquivalent), ["", ""], "no Escape for the cancel")
        XCTAssertFalse(held.alerts.key("Escape"))
        XCTAssertTrue(held.alerts.presented === owner, "only its buttons end it")
        held.alerts.reset()
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

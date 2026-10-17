#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1115 D6, "Confirmations are the platform's alert": an alertdialog
/// with explanatory text, a cancel and at most three buttons is
/// `UIAlertController(.alert)` — its `aria-label` the title, its text the
/// message, its Cancel kept — where it had been an arrowed action-sheet
/// popover that dropped the Cancel and showed no title. A chooser with a
/// cancel is UIKit's unanchored action sheet on a compact screen; one a tap
/// outside must dismiss (no cancel) stays the popover. `showModal(id)` from
/// an action presents the same confirmation with no invoker (a context
/// menu's item asks first). The native fixture's `clear-confirm` is the
/// app-level evidence. UIKit, so a simulator runs it:
/// bun host/apple/build.mjs --test --ios
final class ConfirmationAlertIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        window?.isHidden = true
        window = nil
        session = nil
        super.tearDown()
    }

    /// A presenter in a scene's window (a presentation completes only
    /// there), its viewport under a controller that can present.
    private func presenting(_ titles: [UInt32: String]) -> (Presenter, UIViewController, Bool) {
        let p = Presenter()
        p.buttonFace = { id in var f = ButtonFace(); f.title = titles[id]; return f }
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        let controller = UIViewController()
        window.rootViewController = controller
        window.makeKeyAndVisible()
        p.viewport.frame = controller.view.bounds
        controller.view.addSubview(p.viewport)
        self.window = window
        return (p, controller, scene != nil)
    }
    private func native(_ id: Int, _ props: [String: String], press: Bool = true, y: Double = 0) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control", "handlers": press ? ["press"] : [],
          "props": ["type": "button", "accessibilityRole": "button"].merging(props) { $1 },
          "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": y, "w": 80.0, "h": 40.0]]
    }
    /// An alertdialog popover `confirm` (2) opened by 1, with the given
    /// text rows (10…), actions (3…, the first destructive) and, if asked,
    /// a hide-only Cancel (9). `bareLead`: the first action is a button
    /// its author drew (`appearance="none"`), with that style.
    private func confirmation(label: String?, texts: [String], actions: [String], cancel: Bool, bareLead: [String: Any]? = nil) -> (Presenter, UIViewController, Bool) {
        var titles: [UInt32: String] = [9: "Cancel"]
        for (i, title) in actions.enumerated() { titles[UInt32(3 + i)] = title }
        let (p, controller, scene) = presenting(titles)
        let closes = ["popovertarget": "confirm", "popovertargetaction": "hide"]
        var props = ["id": "confirm", "popover": "auto", "accessibilityRole": "alertdialog"]
        if let label { props["accessibilityLabel"] = label }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": [], "props": ["popovertarget": "confirm"], "style": ["text_color": [0, 0, 0, 255]]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "create", "id": 2, "kind": "view", "props": props, "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0]]
        var children: [Int] = []
        for (i, text) in texts.enumerated() {
            ops.append(["op": "create", "id": 10 + i, "kind": "text", "props": ["text": text], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]])
            children.append(10 + i)
        }
        for i in actions.indices {
            ops += native(3 + i, closes.merging(i == 0 ? ["destructive": "true"] : [:]) { $1 }, y: Double(40 * i))
            if i == 0, let bareLead { ops[ops.count - 2] = ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": closes, "style": bareLead] }
            children.append(3 + i)
        }
        if cancel { ops += native(9, closes, press: false, y: 160); children.append(9) }
        ops += [["op": "children", "id": 2, "ids": children], ["op": "roots", "ids": [1, 2]]]
        p.apply(wireBatch(ops))
        return (p, controller, scene)
    }
    private func settle(until done: @escaping () -> Bool) {
        let settled = expectation(description: "settled")
        func poll() { if done() { settled.fulfill() } else { DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: poll) } }
        poll()
        wait(for: [settled], timeout: 5)
    }
    /// Whether this test process has a scene, in whose window alone a
    /// presentation completes (a hostless bundle has none).
    private static var scene: Bool { UIApplication.shared.connectedScenes.contains { $0 is UIWindowScene } }
    /// Run the main loop until `done`, at most `seconds`.
    private func spin(_ seconds: TimeInterval = 3, until done: () -> Bool) {
        let end = Date().addingTimeInterval(seconds)
        while !done(), Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    /// Leave no presentation behind for the next test in this process: a
    /// dismissal asked mid-presentation is one UIKit drops.
    private func dismiss(_ p: Presenter, _ controller: UIViewController) {
        let scene = Self.scene
        if scene { spin { !p.menus.inTransition } }
        p.menus.reset()
        if scene { spin { controller.presentedViewController == nil } }
    }
    /// What UIKit runs when an action is chosen: its own handler.
    private func choose(_ action: UIAlertAction) throws {
        typealias Handler = @convention(block) (UIAlertAction) -> Void
        let block = try XCTUnwrap(action.value(forKey: "handler") as AnyObject?, "UIAlertAction's handler")
        unsafeBitCast(block, to: Handler.self)(action)
    }

    func testHideThenShowDuringDismissalKeepsCommandOrderAndCoalescesToggle() throws {
        let (p, controller, scene) = confirmation(label: "Keep?", texts: ["A message."], actions: ["Keep"], cancel: true)
        defer { dismiss(p, controller) }
        let pop = try XCTUnwrap(p.views[2])
        pop.handlers.insert("toggle")
        var events: [String] = []
        p.onClipboard = { _, _, payload in events.append(payload) }
        p.menus.action("showPopover", ["confirm"])
        spin(20) { events.count == 1 && (!scene || !p.menus.inTransition) }
        let first = try XCTUnwrap(controller.presentedViewController)
        p.menus.action("hidePopover", ["confirm"])
        XCTAssertFalse(p.menus.isOpen(pop), "logical visibility closes immediately")
        p.menus.action("showPopover", ["confirm"])
        XCTAssertTrue(p.menus.isOpen(pop), "show follows hide before animation completes")
        let turn = expectation(description: "the command batch's toggle task")
        DispatchQueue.main.async { turn.fulfill() }
        wait(for: [turn], timeout: 20)
        XCTAssertEqual(events, ["closed,open", "open,open"])
        if scene {
            spin(20) { !p.menus.inTransition && controller.presentedViewController !== first }
            XCTAssertNotNil(controller.presentedViewController)
            XCTAssertTrue(p.menus.isOpen(pop))
        }
        p.menus.action("hidePopover", ["confirm"])
        XCTAssertFalse(p.menus.isOpen(pop), "a later hide cancels a queued reopen too")
    }

    /// A menu row that opens a confirmation opens it once the menu has gone,
    /// and only for the row and the target the menu showed, while the row is
    /// enabled.
    func testAMenuRowOpensItsConfirmationOnlyWhileItAndItsTargetSurvive() throws {
        let (p, controller, _) = confirmation(label: "Keep?", texts: ["A message."], actions: ["Keep"], cancel: true)
        defer { dismiss(p, controller) }
        let pop = try XCTUnwrap(p.views[2])
        p.apply(wireBatch([["op": "create", "id": 30, "kind": "view", "props": ["id": "menu", "popover": "auto"], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 30, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0]]
            + native(31, ["popovertarget": "confirm", "popovertargetaction": "show"], press: false)
            + [["op": "children", "id": 30, "ids": [31]], ["op": "roots", "ids": [1, 2, 30]]]))
        func row() throws -> UIAction { try XCTUnwrap(p.menus.items(of: try XCTUnwrap(p.views[30])).first as? UIAction) }
        func choose(_ action: UIAction) { action.performWithSender(nil, target: nil); spin(0.3) { false } }

        let recycledRow = try row()
        try XCTUnwrap(p.views[31]).incarnation = NodePool.issue()
        choose(recycledRow)
        XCTAssertFalse(p.menus.isOpen(pop), "the row it showed was recycled")

        let action = try row()
        action.performWithSender(nil, target: nil)
        pop.incarnation = NodePool.issue()
        spin(0.3) { false }
        XCTAssertFalse(p.menus.isOpen(pop), "its target was recycled before it opened")

        let disabledRow = try row()
        try XCTUnwrap(p.views[31]).props["disabled"] = "true"
        choose(disabledRow)
        XCTAssertFalse(p.menus.isOpen(pop), "the row became disabled")
        try XCTUnwrap(p.views[31]).props["disabled"] = nil

        choose(try row())
        XCTAssertTrue(p.menus.isOpen(pop))
    }

    /// Native invokers queue and cancel a reopen while the alert is still
    /// leaving, as the imperative commands above do.
    func testANativeShowDuringDismissalReopensAndAHideCancelsIt() throws {
        let (p, controller, _) = confirmation(label: "Keep?", texts: ["A message."], actions: ["Keep"], cancel: true)
        defer { dismiss(p, controller) }
        let pop = try XCTUnwrap(p.views[2])
        p.apply(wireBatch(native(20, ["popovertarget": "confirm", "popovertargetaction": "show"], press: false)
            + native(21, ["popovertarget": "confirm", "popovertargetaction": "hide"], press: false)
            + [["op": "roots", "ids": [1, 2, 20, 21]]]))
        let show = try XCTUnwrap(p.views[20]), hide = try XCTUnwrap(p.views[21])
        p.menus.invoke(show)
        XCTAssertTrue(p.menus.isOpen(pop))
        p.menus.invoke(hide)
        XCTAssertFalse(p.menus.isOpen(pop))
        p.menus.invoke(show)
        XCTAssertTrue(p.menus.isOpen(pop), "a show while the alert leaves queues its reopen")
        p.menus.invoke(hide)
        XCTAssertFalse(p.menus.isOpen(pop), "a hide cancels the queued reopen")
    }

    /// `close(id)` cancels a reopen queued while the alert leaves, as a
    /// native hide does.
    func testCloseCancelsAShowModalQueuedDuringDismissal() throws {
        let (p, controller, _) = confirmation(label: "Keep?", texts: ["A message."], actions: ["Keep"], cancel: true)
        defer { dismiss(p, controller) }
        let pop = try XCTUnwrap(p.views[2])
        p.dialogCommand("showModal", "confirm")
        p.dialogCommand("close", "confirm")
        XCTAssertFalse(p.menus.isOpen(pop))
        p.dialogCommand("showModal", "confirm")
        XCTAssertTrue(p.menus.isOpen(pop), "queued behind the leaving alert")
        p.dialogCommand("close", "confirm")
        XCTAssertFalse(p.menus.isOpen(pop), "and cancelled")
    }

    /// HTML's dialog commands act only on a modal dialog and its popover
    /// commands only on a popover, however each was opened.
    func testCommandsActOnlyOnAConfirmationOpenedInTheirMode() throws {
        let (p, controller, _) = confirmation(label: "Keep?", texts: ["A message."], actions: ["Keep"], cancel: true)
        defer { dismiss(p, controller) }
        let pop = try XCTUnwrap(p.views[2])
        pop.props["semanticTag"] = "dialog"
        p.apply(wireBatch(native(20, ["commandfor": "confirm", "command": "close"], press: false)
            + native(21, ["commandfor": "confirm", "command": "hide-popover"], press: false)
            + [["op": "roots", "ids": [1, 2, 20, 21]]]))
        let close = try XCTUnwrap(p.views[20]), hide = try XCTUnwrap(p.views[21])
        p.menus.action("showPopover", ["confirm"])
        XCTAssertTrue(p.menus.isOpen(pop))
        p.menus.invoke(close)
        XCTAssertTrue(p.menus.isOpen(pop), "a dialog close leaves a popover with no invoker open")
        p.menus.invoke(hide)
        XCTAssertFalse(p.menus.isOpen(pop))

        // Opened modally, here as the reopen queued while that alert leaves,
        // its rows closing it as a modal dialog's do
        p.dialogCommand("showModal", "confirm")
        XCTAssertFalse(p.menus.isOpen(pop), "refused: hide-popover rows do not close a modal dialog")
        for id: UInt32 in [3, 9] {
            let row = try XCTUnwrap(p.views[id])
            row.props["popovertarget"] = nil; row.props["popovertargetaction"] = nil
            row.props["commandfor"] = "confirm"; row.props["command"] = "close"
        }
        p.dialogCommand("showModal", "confirm")
        XCTAssertTrue(p.menus.isOpen(pop))
        p.menus.invoke(hide)
        p.menus.action("hidePopover", ["confirm"])
        XCTAssertTrue(p.menus.isOpen(pop), "popover commands leave a modal dialog open")
        p.menus.invoke(close)
        XCTAssertFalse(p.menus.isOpen(pop))
    }

    /// "Remove book?" — the shape of React Native's `Alert.alert`: a
    /// centred alert, titled by the label, its text the message, the
    /// destructive action and the Cancel both drawn.
    func testAConfirmationWithAMessageIsTheCentredAlertWithItsTitleAndCancel() throws {
        let (p, controller, scene) = confirmation(label: "Remove book?", texts: ["\u{201C}Piranesi\u{201D} will be removed."], actions: ["Remove"], cancel: true)
        defer { dismiss(p, controller) }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true, "its invoker opens it")
        XCTAssertEqual(p.openTopLayerFrames.map(\.0), [2], "the structural read sees the native confirmation as open")
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.preferredStyle, .alert)
        XCTAssertEqual(alert.title, "Remove book?")
        XCTAssertEqual(alert.message, "\u{201C}Piranesi\u{201D} will be removed.")
        XCTAssertEqual(alert.actions.map(\.title), ["Remove", "Cancel"])
        XCTAssertEqual(alert.actions.map(\.style), [.destructive, .cancel])
        XCTAssertEqual(p.menus.observation()?["presentation"] as? String, "alert")
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        settle { !p.menus.inTransition }
        let box = alert.view.convert(alert.view.bounds, to: nil)
        print("alert: \(box) in \(window!.bounds)")
        XCTAssertEqual(box.midX, window!.bounds.midX, accuracy: 2, "centred")
        XCTAssertEqual(box.midY, window!.bounds.midY, accuracy: 40, "centred")
    }

    /// LLP 1115.000 D2: an alert has one tint, the colour its first action's
    /// author wrote on it, a platform colour as much as a literal. A colour
    /// the action only inherits leaves UIKit's (LLP 1115 D4).
    func testAnAlertsTintIsTheColourItsFirstActionsAuthorWrote() throws {
        let red: [String: Any] = ["sys": "systemRedColor", "c": [[255, 56, 60, 255], [255, 66, 69, 255]]]
        func tint(_ lead: [String: Any]) throws -> [CGFloat] {
            let (p, controller, _) = confirmation(label: "Keep?", texts: ["A message."], actions: ["Keep"], cancel: true, bareLead: lead)
            defer { dismiss(p, controller) }
            XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
            let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
            var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
            XCTAssertTrue(try XCTUnwrap(alert.view.tintColor).getRed(&r, green: &g, blue: &b, alpha: &a))
            return [r, g, b]
        }
        let written = try tint(["text_color": red, "written": ["text_color"]])
        XCTAssertTrue(written[0] > 0.9 && written[1] < 0.4 && written[2] < 0.4, "the platform's red its author wrote: \(written)")
        let inherited = try tint(["text_color": red])
        XCTAssertFalse(inherited[0] > 0.9 && inherited[2] < 0.4, "the same red, only inherited, leaves UIKit's tint: \(inherited)")
    }

    /// A prompt (`kernel/src/confirmation.rs`): an alert-shaped alertdialog
    /// with an `input` is the alert with UIKit's text field, which stands in
    /// for the authored field: its value and placeholder, a keystroke the
    /// input's `input`, Return its first action that is not destructive, and
    /// its `change` before the chosen action's press. An action its author
    /// disables while it is typed in is dimmed, not a reason to withdraw it.
    func testAPromptIsTheAlertWithAFieldStandingInForTheAuthoredOne() throws {
        let (p, controller, scene) = confirmation(label: "Rename", texts: [], actions: ["Delete", "Save"], cancel: true)
        defer { dismiss(p, controller) }
        p.apply(wireBatch([["op": "create", "id": 20, "kind": "input", "handlers": ["input", "change"],
                            "props": ["value": "Untitled", "placeholder": "Name", "testId": "name"], "style": ["text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                           ["op": "children", "id": 2, "ids": [20, 3, 4, 9]]]))
        var events: [String] = []
        p.onInput = { id, value in events.append("input \(id) \(value)") }
        p.onChange = { id, value in events.append("change \(id) \(value)") }
        p.onPress = { events.append("press \($0)") }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.preferredStyle, .alert)
        XCTAssertEqual(alert.title, "Rename")
        let field = try XCTUnwrap(alert.textFields?.first)
        XCTAssertEqual(alert.textFields?.count, 1)
        XCTAssertEqual(field.text, "Untitled")
        XCTAssertEqual(field.placeholder, "Name")
        XCTAssertEqual(field.accessibilityIdentifier, "name")
        XCTAssertEqual(field.returnKeyType, .default, "an unsaid enterkeyhint keeps the alert field's Return key")
        XCTAssertEqual(field.autocorrectionType, .no, "unsaid, the alert field's own: no correction")
        XCTAssertEqual(field.autocapitalizationType, .none, "unsaid, the alert field's own: no capitalization")
        XCTAssertNil(alert.preferredAction, "no action drawn bold, as a hand-built prompt's")

        field.text = "Kick"
        field.sendActions(for: .editingChanged)
        XCTAssertEqual(events, ["input 20 Kick"])
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["disabled": "true"]]]))
        XCTAssertNotNil(p.menus.observation(), "still presented")
        XCTAssertFalse(alert.actions[1].isEnabled, "Save follows its author's disabled")
        p.apply(wireBatch([["op": "props", "id": 4, "clear": ["disabled"]]]))
        XCTAssertTrue(alert.actions[1].isEnabled)
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        settle { !p.menus.inTransition }
        try choose(alert.actions[1])
        settle { events.contains("press 4") }
        XCTAssertEqual(events, ["input 20 Kick", "change 20 Kick", "press 4"])
        XCTAssertNil(try XCTUnwrap(p.views[20]).headerField, "the authored field is its own again")
    }

    /// An email prompt keeps its text as typed, as the page's field does: no
    /// capitalization, correction, smart quotes or smart dashes.
    func testAnEmailPromptKeepsItsTextAsTyped() throws {
        let (p, controller, _) = confirmation(label: "Invite", texts: [], actions: ["Delete", "Send"], cancel: true)
        defer { dismiss(p, controller) }
        p.apply(wireBatch([["op": "create", "id": 20, "kind": "input", "handlers": ["input"],
                            "props": ["value": "", "type": "email", "testId": "email"], "style": ["text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                           ["op": "children", "id": 2, "ids": [20, 3, 4, 9]]]))
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        let field = try XCTUnwrap(alert.textFields?.first)
        XCTAssertEqual(field.keyboardType, .emailAddress)
        XCTAssertEqual(field.autocapitalizationType, .none)
        XCTAssertEqual(field.autocorrectionType, .no)
        XCTAssertEqual(field.smartQuotesType, .no)
        XCTAssertEqual(field.smartDashesType, .no)
    }

    private final class PromptKey: UIKey {
        var text = "7"
        var code = UIKeyboardHIDUsage.keyboard7
        override var characters: String { text }
        override var charactersIgnoringModifiers: String { text }
        override var keyCode: UIKeyboardHIDUsage { code }
        override var modifierFlags: UIKeyModifierFlags { [] }
    }
    private final class PromptPress: UIPress {
        let value = PromptKey()
        override var key: UIKey? { value }
    }
    // A synthetic press has no UIKit hardware event. Its native default is
    // this probe's, while the real alert below exercises UIKeyInput itself.
    private final class KeyProbe: UITextField {
        var forwarded: [String] = []
        var composing = false
        private let marked = UITextRange()
        override var markedTextRange: UITextRange? { composing ? marked : nil }
        override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            forwarded.append("down")
            if presses.first?.key?.keyCode == .keyboardDeleteOrBackspace { deleteBackward() }
            if presses.first?.key?.keyCode == .keyboardReturnOrEnter { _ = delegate?.textFieldShouldReturn?(self) }
        }
        override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) { forwarded.append("up") }
        override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) { forwarded.append("cancel") }
        override func deleteBackward() { forwarded.append("delete") }
    }

    func testAPromptKeepsTheFieldsDefaultsAndHearsEachHardwareKeyOnce() throws {
        let p = Presenter()
        p.apply(wireBatch([["op": "create", "id": 20, "kind": "input", "handlers": ["key", "keyup", "submit"], "props": ["value": "Kick"]]]))
        let bridge = PromptField(try XCTUnwrap(p.views[20]), presenter: p)
        defer { bridge.end() }
        let probe = KeyProbe()
        let field: UITextField = probe
        bridge.attach(field)
        var events: [String] = []
        var prevent = true
        p.onKey = { _, key in
            events.append("\(key.up ? "up" : "down") \(key.chord)")
            if !key.up && prevent { p.defaultPrevented = true }
        }
        p.onSubmit = { _ in events.append("submit") }
        let press = PromptPress()
        field.pressesBegan([press], with: nil)
        field.pressesEnded([press], with: nil)
        XCTAssertEqual(events, ["down 7", "up 7"])
        XCTAssertEqual(probe.forwarded, ["up"], "a prevented digit never reaches the field's default")
        events.removeAll(); probe.forwarded.removeAll(); prevent = false
        press.value.text = "\u{8}"
        press.value.code = .keyboardDeleteOrBackspace
        field.pressesBegan([press], with: nil)
        field.pressesEnded([press], with: nil)
        XCTAssertEqual(events, ["down Backspace", "up Backspace"])
        XCTAssertEqual(probe.forwarded, ["down", "delete", "up"])
        events.removeAll(); probe.forwarded.removeAll()
        press.value.text = "\r"
        press.value.code = .keyboardReturnOrEnter
        field.pressesBegan([press], with: nil)
        field.pressesEnded([press], with: nil)
        XCTAssertEqual(events, ["down Enter", "submit", "up Enter"], "the hardware Return's delegate does not dispatch its key twice")
        XCTAssertEqual(probe.forwarded, ["down", "up"])
        events.removeAll(); probe.forwarded.removeAll()
        field.pressesBegan([press], with: nil)
        field.pressesCancelled([press], with: nil)
        _ = field.delegate?.textFieldShouldReturn?(field)
        XCTAssertEqual(events, ["down Enter", "submit", "down Enter", "submit"], "cancellation clears the heard hardware key")
        XCTAssertEqual(probe.forwarded, ["down", "cancel"])
        events.removeAll(); probe.forwarded.removeAll()
        probe.composing = true
        press.value.text = "7"
        press.value.code = .keyboard7
        field.pressesBegan([press], with: nil)
        field.pressesEnded([press], with: nil)
        XCTAssertEqual(events, [], "composition belongs to the displayed field, not the input left on the page")
        XCTAssertEqual(probe.forwarded, ["down", "up"])
    }

    func testAPromptCanPreventSoftwareBackspaceAndReturn() throws {
        let (p, controller, scene) = confirmation(label: "Rename", texts: [], actions: ["Save"], cancel: true)
        defer { dismiss(p, controller) }
        if !scene { try missing("scene", "a prompt's keyboard needs an app scene") }
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "input", "handlers": ["key", "keyup", "input", "change", "submit"], "props": ["value": "Kick"]],
            ["op": "frame", "id": 20, "x": 0, "y": 0, "w": 200, "h": 30],
            ["op": "children", "id": 2, "ids": [20, 3, 9]],
        ]))
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        let field = try XCTUnwrap(alert.textFields?.first)
        spin { !p.menus.inTransition }
        XCTAssertTrue(field.becomeFirstResponder())
        field.selectedTextRange = field.textRange(from: field.endOfDocument, to: field.endOfDocument)
        var events: [String] = []
        var prevent = true
        p.onKey = { _, key in
            events.append("\(key.up ? "up" : "down") \(key.chord)")
            if !key.up && prevent { p.defaultPrevented = true }
        }
        p.onSubmit = { _ in events.append("submit") }
        p.onChange = { _, _ in events.append("change") }
        field.deleteBackward()
        _ = field.delegate?.textFieldShouldReturn?(field)
        XCTAssertEqual(events, ["down Backspace", "down Enter"])
        XCTAssertEqual(field.text, "Kick", "a prevented Backspace leaves the text alone")
        XCTAssertTrue(field.isFirstResponder)
        events.removeAll(); prevent = false
        field.deleteBackward()
        _ = field.delegate?.textFieldShouldReturn?(field)
        XCTAssertEqual(events, ["down Backspace", "down Enter", "change", "submit"])
        XCTAssertEqual(field.text, "Kic")
        XCTAssertTrue(field.isFirstResponder)
    }

    /// Return in a prompt's field is its delegate's: it presses the first
    /// action that is not destructive, nothing while that one is disabled,
    /// and draws none bold. The one prominent action its author marked is
    /// the alert's preferred action, as the Mac's default button.
    func testAPromptsReturnPressesItsFirstActionAndOnlyAnAuthoredOneIsPreferred() throws {
        let (p, controller, scene) = confirmation(label: "Rename", texts: [], actions: ["Delete", "Save"], cancel: true)
        defer { dismiss(p, controller) }
        p.apply(wireBatch([["op": "create", "id": 20, "kind": "input", "handlers": ["change"], "props": ["value": "Untitled"], "style": ["text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                           ["op": "children", "id": 2, "ids": [20, 3, 4, 9]],
                           ["op": "props", "id": 4, "set": ["disabled": "true"]]]))
        var events: [String] = []
        p.onChange = { id, value in events.append("change \(id) \(value)") }
        p.onPress = { events.append("press \($0)") }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertNil(alert.preferredAction, "unsaid: no action is drawn bold")
        let field = try XCTUnwrap(alert.textFields?.first)
        XCTAssertEqual(field.delegate?.textFieldShouldReturn?(field), false)
        XCTAssertNotNil(p.menus.observation(), "Save is disabled: Return does nothing more")
        p.apply(wireBatch([["op": "props", "id": 4, "clear": ["disabled"]]]))
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        settle { !p.menus.inTransition }
        field.text = "Kick"
        field.sendActions(for: .editingChanged)
        XCTAssertEqual(field.delegate?.textFieldShouldReturn?(field), false)
        settle { events.contains("press 4") }
        XCTAssertEqual(events.filter { $0.hasPrefix("press") }, ["press 4"], "Return pressed Save: \(events)")
        XCTAssertEqual(events.first, "change 20 Kick", "its change before the press")
        settle { p.menus.observation() == nil && controller.presentedViewController == nil }

        // Save marked `filled`: the alert's preferred action, drawn bold.
        p.buttonFace = { id in var f = ButtonFace(); f.title = [3: "Delete", 4: "Save", 9: "Cancel"][id]; if id == 4 { f.style = "filled" }; return f }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let marked = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(marked.preferredAction?.title, "Save", "the author's one filled action, the one Return presses")
        let actions = [p.views[3], p.views[4]].compactMap { $0 }
        p.buttonFace = { id in var f = ButtonFace(); f.title = [3: "Delete", 4: "Save", 9: "Cancel"][id]; f.style = "filled"; return f }
        XCTAssertEqual(MenuHost.preferredAction(actions, prompt: true), 1, "a destructive one is never preferred")
        p.buttonFace = { id in var f = ButtonFace(); f.title = [3: "Delete", 4: "Save", 9: "Cancel"][id]; if id == 4 { f.style = "bordered-prominent" }; return f }
        XCTAssertNil(MenuHost.preferredAction(actions, prompt: false), "only `filled`, as the Mac's default button")
        p.apply(wireBatch([["op": "props", "id": 3, "clear": ["destructive"]]]))
        p.buttonFace = { id in var f = ButtonFace(); f.title = [3: "Keep", 4: "Save", 9: "Cancel"][id]; if id == 4 { f.style = "filled" }; return f }
        XCTAssertNil(MenuHost.preferredAction(actions, prompt: true), "a prompt's Return presses Keep: Save is not drawn bold")
        XCTAssertEqual(MenuHost.preferredAction(actions, prompt: false), 1, "an alert with no field: its one filled action")
        p.buttonFace = { id in var f = ButtonFace(); f.title = [3: "Keep", 4: "Save", 9: "Cancel"][id]; f.style = "filled"; return f }
        XCTAssertNil(MenuHost.preferredAction(actions, prompt: false), "emphasis never picks among several")
    }

    /// A prompt opens with every action disabled (Save, until a name is
    /// typed), and its field follows the authored input after each batch:
    /// the value the page now holds, its enablement, and its own checks on
    /// an edit (`readonly`, `maxlength`).
    func testAPromptsFieldFollowsTheAuthoredInputAndOpensWithSaveDisabled() throws {
        let (p, controller, _) = confirmation(label: "Rename", texts: [], actions: ["Delete", "Save"], cancel: true)
        defer { dismiss(p, controller) }
        p.apply(wireBatch([["op": "create", "id": 20, "kind": "input", "handlers": ["input"], "props": ["value": ""], "style": ["text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                           ["op": "children", "id": 2, "ids": [20, 3, 4, 9]],
                           ["op": "props", "id": 3, "set": ["disabled": "true"]], ["op": "props", "id": 4, "set": ["disabled": "true"]]]))
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true, "a prompt opens with every action disabled")
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.actions.map(\.isEnabled), [false, false, true], "the field and Cancel remain")
        let field = try XCTUnwrap(alert.textFields?.first)

        p.apply(wireBatch([["op": "props", "id": 20, "set": ["value": "KICK"]]]))
        XCTAssertEqual(field.text, "KICK", "the value the page now holds (an input handler that rewrites it)")
        XCTAssertNotNil(p.menus.observation(), "still presented")
        func edits(_ text: String) -> Bool {
            field.delegate?.textField?(field, shouldChangeCharactersIn: NSRange(location: (field.text ?? "").utf16.count, length: 0), replacementString: text) ?? true
        }
        XCTAssertTrue(edits("S"))
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["maxlength": "4"]]]))
        XCTAssertFalse(edits("S"), "maxlength")
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["editable": "false"], "clear": ["maxlength"]]]))
        XCTAssertFalse(edits("S"), "readonly")
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["disabled": "true"], "clear": ["editable"]]]))
        XCTAssertFalse(field.isEnabled, "disabled")
    }

    /// Typing stands across batches that write nothing to the input (an
    /// input that commits on `change`, a timer's batch); Return is the
    /// input's `change` and `submit` before its first action that is not destructive;
    /// and an input the page swaps for another withdraws the alert, whose
    /// field is no longer the input's.
    func testAPromptsTypingStandsItsReturnSubmitsAndASwappedFieldWithdrawsIt() throws {
        let (p, controller, _) = confirmation(label: "Rename", texts: [], actions: ["Delete", "Save"], cancel: true)
        defer { dismiss(p, controller) }
        p.apply(wireBatch([["op": "create", "id": 20, "kind": "input", "handlers": ["change", "submit"], "props": ["value": "Untitled"], "style": ["text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                           ["op": "children", "id": 2, "ids": [20, 3, 4, 9]],
                           // Save disabled: Return then reaches the input and presses nothing.
                           ["op": "props", "id": 4, "set": ["disabled": "true"]]]))
        var events: [String] = []
        p.onChange = { id, value in events.append("change \(id) \(value)") }
        p.onSubmit = { events.append("submit \($0)") }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        let field = try XCTUnwrap(alert.textFields?.first)
        field.text = "Kick"
        field.sendActions(for: .editingChanged)
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["accessibilityLabel": "unrelated"]]]))
        XCTAssertEqual(field.text, "Kick", "a batch that writes nothing to the input leaves what was typed")
        _ = field.delegate?.textFieldShouldReturn?(field)
        settle { events.contains("submit 20") }
        XCTAssertEqual(events, ["change 20 Kick", "submit 20"], "the input's own Return first")
        XCTAssertNotNil(p.menus.observation(), "Save is disabled: still open")

        p.apply(wireBatch([["op": "create", "id": 21, "kind": "input", "handlers": [], "props": ["value": ""], "style": ["text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                           ["op": "children", "id": 2, "ids": [21, 3, 4, 9]], ["op": "destroy", "id": 20]]))
        XCTAssertNil(p.menus.observation(), "the field shown is no longer the input's: withdrawn")
    }

    /// A value the page wrote while text was marked waits (`pendingValue`)
    /// and lands in the prompt's field once the composition has ended.
    func testAPromptsHeldValueLandsWhenTheCompositionEnds() throws {
        let (p, controller, _) = confirmation(label: "Rename", texts: [], actions: ["Delete", "Save"], cancel: true)
        defer { dismiss(p, controller) }
        p.apply(wireBatch([["op": "create", "id": 20, "kind": "input", "handlers": ["input"], "props": ["value": ""], "style": ["text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
                           ["op": "children", "id": 2, "ids": [20, 3, 4, 9]]]))
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        let field = try XCTUnwrap(alert.textFields?.first)
        try XCTUnwrap(p.views[20]).pendingValue = "かな"
        field.text = "か"
        field.sendActions(for: .editingChanged)
        XCTAssertEqual(field.text, "かな", "held while marked, written once nothing is")
    }

    /// Without a label, the first line titles the alert and the rest is
    /// its message; an alert always has a title.
    func testAnUnlabelledConfirmationIsTitledByItsFirstLine() throws {
        let (p, controller, _) = confirmation(label: nil, texts: ["Delete draft?", "It cannot be recovered."], actions: ["Delete"], cancel: true)
        defer { dismiss(p, controller) }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.preferredStyle, .alert)
        XCTAssertEqual(alert.title, "Delete draft?")
        XCTAssertEqual(alert.message, "It cannot be recovered.")
    }

    /// Without a cancel the confirmation keeps the popover: an alert has no
    /// outside to tap, and nothing else would let the person decline.
    func testAConfirmationWithoutACancelStaysAPopover() throws {
        let (p, controller, _) = confirmation(label: "Block contact?", texts: ["They will not reach you."], actions: ["Block"], cancel: false)
        defer { dismiss(p, controller) }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.preferredStyle, .actionSheet)
        XCTAssertEqual(alert.title, "Block contact?", "titled by its label (D6)")
        XCTAssertEqual(alert.message, "They will not reach you.")
        XCTAssertNotNil(alert.popoverPresentationController?.sourceView, "anchored at its invoker")
        XCTAssertEqual(p.menus.observation()?["presentation"] as? String, "popover")
    }

    /// A chooser — several actions, no message — with a cancel is an
    /// unanchored action sheet on a compact screen, its Cancel drawn,
    /// not an arrowed popover that drops it.
    func testAChooserWithACancelIsAnUnanchoredSheetOnACompactScreen() throws {
        let (p, controller, scene) = confirmation(label: "Open location in", texts: [], actions: ["Apple Maps", "Google Maps", "Waze"], cancel: true)
        defer { dismiss(p, controller) }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.preferredStyle, .actionSheet)
        XCTAssertEqual(alert.title, "Open location in")
        XCTAssertEqual(alert.actions.map(\.style), [.destructive, .default, .default, .cancel])
        let compact = controller.traitCollection.horizontalSizeClass != .regular
        XCTAssertEqual(p.menus.observation()?["presentation"] as? String, compact ? "sheet" : "popover")
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        try XCTSkipUnless(compact, "an unanchored sheet is on a compact screen")
        XCTAssertNil(alert.popoverPresentationController?.sourceView, "not anchored at its invoker")
        settle { !p.menus.inTransition }
        XCTAssertTrue(alert.actions.last.map { $0.style == .cancel } == true)
    }

    /// `showModal(id)` from an action: the confirmation by id, with no
    /// invoker; its action presses its row once, `close(id)` is its cancel.
    func testShowModalPresentsTheConfirmationWithNoInvoker() throws {
        let (p, controller, scene) = confirmation(label: "Remove book?", texts: ["It will be removed."], actions: ["Remove"], cancel: true)
        defer { dismiss(p, controller) }
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.dialogCommand("showModal", "nothing")
        XCTAssertNil(controller.presentedViewController, "no such id: refused, nothing shown")
        p.dialogCommand("showModal", "confirm")
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.preferredStyle, .alert)
        XCTAssertEqual(alert.title, "Remove book?")
        XCTAssertTrue(p.menus.observation()?["source"] is NSNull, "no invoker")
        XCTAssertTrue(p.menus.isOpen(try XCTUnwrap(p.views[2])))
        p.dialogCommand("showModal", "confirm")
        XCTAssertTrue(controller.presentedViewController === alert, "showModal on an open one does nothing")
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        settle { !p.menus.inTransition }
        try choose(alert.actions[0])
        settle { p.menus.observation() == nil }
        settle { !pressed.isEmpty }
        XCTAssertEqual(pressed, [3], "the action's press, once")

        // Again, then `close(id)` from an action: the cancel, nothing pressed.
        settle { controller.presentedViewController == nil }
        p.dialogCommand("showModal", "confirm")
        XCTAssertNotNil(p.menus.observation())
        settle { !p.menus.inTransition }
        p.dialogCommand("close", "confirm")
        settle { p.menus.observation() == nil && controller.presentedViewController == nil }
        XCTAssertEqual(pressed, [3])
    }

    // MARK: the native fixture

    private func boot() throws -> ExactSession {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the context menu is painted (D4)") }
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: "confirmation-alert")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        return session
    }
    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }
    private func counts(_ session: ExactSession) throws -> String {
        try node(session, "counts").paragraphSpec().runs.map(\.text).joined()
    }

    /// The fixture's long-press → confirm: the context menu's Clear runs
    /// `showModal("clear-confirm")`, a `dialog role="alertdialog"`, which
    /// shows as the alert; its Clear resets the count, as its own invoker's
    /// alert does.
    func testTheFixturesContextMenuItemRaisesTheAlertByShowModal() throws {
        let session = try boot()
        let p = session.presenter
        p.press(try node(session, "peek-bump").id)
        settle { (try? self.counts(session))?.hasPrefix("count 1") == true }
        p.press(try node(session, "peek-clear").id)
        settle { p.menus.observation()?["kind"] as? String == "confirmation" }
        let controller = try XCTUnwrap(p.viewport.window?.rootViewController)
        defer { dismiss(p, controller) }
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.preferredStyle, .alert)
        XCTAssertEqual(alert.title, "Clear the count?")
        XCTAssertEqual(alert.message, "The count goes back to zero.")
        XCTAssertEqual(alert.actions.map(\.title), ["Clear", "Cancel"])
        XCTAssertEqual(alert.actions.map(\.style), [.destructive, .cancel])
        XCTAssertTrue(try counts(session).hasPrefix("count 1"), "nothing cleared before the choice")
        XCTAssertTrue(p.menus.observation()?["source"] is NSNull, "raised by an action, not an invoker")
        guard Self.scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        settle { !p.menus.inTransition }
        // The agent's activation of the alert's action, by its node.
        XCTAssertEqual(p.menus.activate(try node(session, "clear-confirm-clear")), true)
        settle { (try? self.counts(session))?.hasPrefix("count 0") == true }
        settle { p.menus.observation() == nil && controller.presentedViewController == nil }

        // Its invoker opens the same alert; Cancel leaves the count.
        p.press(try node(session, "peek-bump").id)
        settle { (try? self.counts(session))?.hasPrefix("count 1") == true }
        XCTAssertEqual(p.menus.activate(try node(session, "ask-clear")), true)
        XCTAssertEqual((controller.presentedViewController as? UIAlertController)?.preferredStyle, .alert)
        settle { !p.menus.inTransition }
        XCTAssertEqual(p.menus.activate(try node(session, "clear-confirm-cancel")), true)
        settle { p.menus.observation() == nil }
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertTrue(try counts(session).hasPrefix("count 1"))
    }
}
#endif

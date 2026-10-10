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
    /// a hide-only Cancel (9).
    private func confirmation(label: String?, texts: [String], actions: [String], cancel: Bool) -> (Presenter, UIViewController, Bool) {
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

    /// "Remove book?" — the shape of React Native's `Alert.alert`: a
    /// centred alert, titled by the label, its text the message, the
    /// destructive action and the Cancel both drawn.
    func testAConfirmationWithAMessageIsTheCentredAlertWithItsTitleAndCancel() throws {
        let (p, controller, scene) = confirmation(label: "Remove book?", texts: ["\u{201C}Piranesi\u{201D} will be removed."], actions: ["Remove"], cancel: true)
        defer { dismiss(p, controller) }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true, "its invoker opens it")
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.preferredStyle, .alert)
        XCTAssertEqual(alert.title, "Remove book?")
        XCTAssertEqual(alert.message, "\u{201C}Piranesi\u{201D} will be removed.")
        XCTAssertEqual(alert.actions.map(\.title), ["Remove", "Cancel"])
        XCTAssertEqual(alert.actions.map(\.style), [.destructive, .cancel])
        XCTAssertEqual(p.menus.observation()?["presentation"] as? String, "alert")
        try XCTSkipUnless(scene, "a presentation completes only in a scene's window")
        settle { !p.menus.inTransition }
        let box = alert.view.convert(alert.view.bounds, to: nil)
        print("alert: \(box) in \(window!.bounds)")
        XCTAssertEqual(box.midX, window!.bounds.midX, accuracy: 2, "centred")
        XCTAssertEqual(box.midY, window!.bounds.midY, accuracy: 40, "centred")
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
        try XCTSkipUnless(scene && compact, "an unanchored sheet on a compact screen in a scene's window")
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
        try XCTSkipUnless(scene, "a presentation completes only in a scene's window")
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
        try XCTSkipUnless(Self.scene, "a presentation completes only in a scene's window")
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

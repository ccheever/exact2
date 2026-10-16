#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 D11.5, the other half of LLP 1115 D6's split by shape: a
/// `dialog` whose rows are a confirmation's is the platform's alert, and any
/// other is UIKit's sheet, its own subtree laid out by the kernel inside it,
/// where it had presented nothing ("only text, actions and one cancel may be
/// its rows"). `showModal` and its invoker open it, `close` and its own
/// close button close it, `closedby` is the sheet's dismissal, a dismissal
/// the person made is HTML's `cancel`, and the focus moves in and comes
/// back. The native fixture's `native-dialog` is the app-level evidence.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class DialogSheetIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    private var kept: Presenter?

    override func tearDown() {
        if let p = kept, let controller = window?.rootViewController { dismiss(p, controller) }
        session?.destroy()
        window?.isHidden = true
        window = nil
        session = nil
        kept = nil
        super.tearDown()
    }

    private static var scene: Bool { UIApplication.shared.connectedScenes.contains { $0 is UIWindowScene } }
    private func settle(until done: @escaping () -> Bool) {
        let settled = expectation(description: "settled")
        func poll() { if done() { settled.fulfill() } else { DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: poll) } }
        poll()
        wait(for: [settled], timeout: 5)
    }
    private func spin(_ seconds: TimeInterval = 3, until done: () -> Bool) {
        let end = Date().addingTimeInterval(seconds)
        while !done(), Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
    }
    /// Leave no presentation behind for the next test in this process.
    private func dismiss(_ p: Presenter, _ controller: UIViewController) {
        if Self.scene { spin { !p.menus.inTransition } }
        p.menus.reset()
        if Self.scene { spin { controller.presentedViewController == nil && !p.menus.inTransition } }
    }

    private func native(_ id: Int, _ props: [String: String], press: Bool, y: Double) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control", "handlers": press ? ["press"] : [],
          "props": ["type": "button", "accessibilityRole": "button"].merging(props) { $1 }, "style": ["appearance": "auto"]],
         ["op": "frame", "id": id, "x": 20.0, "y": y, "w": 80.0, "h": 40.0]]
    }
    /// A page (1) with a field (8) and a button (7) that opens `form` (2): a
    /// dialog 300 by 180 holding a field (3), Save (4, a press that also
    /// closes it) and Cancel (5). Beside it `confirm` (20), a dialog in the
    /// alert's shape, its `closedby` what lowering implies for that shape
    /// (`closerequest`, LLP 1115.000 D13.4) unless `confirmClosedby` says
    /// another. A presentation completes only in a scene's window.
    private func page(dialog: [String: String] = [:], handlers: [String] = [], field: [String: String] = [:],
                      confirmClosedby: String = "closerequest", controller: UIViewController = UIViewController()) -> (Presenter, UIViewController, Bool) {
        let p = Presenter()
        kept = p
        p.buttonFace = { id in var f = ButtonFace(); f.title = [4: "Save", 5: "Cancel", 22: "Remove", 23: "Cancel"][id]; return f }
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = controller
        window.makeKeyAndVisible()
        p.viewport.frame = controller.view.bounds
        controller.view.addSubview(p.viewport)
        self.window = window
        let closes = ["commandfor": "form", "command": "close"], confirms = ["commandfor": "confirm", "command": "close"]
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view", "props": ["id": "page"], "handlers": []],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "create", "id": 8, "kind": "input", "props": ["id": "search"], "handlers": ["input"]],
            ["op": "frame", "id": 8, "x": 20.0, "y": 60.0, "w": 200.0, "h": 40.0],
            ["op": "create", "id": 7, "kind": "button", "handlers": [], "props": ["id": "open", "commandfor": "form", "command": "show-modal"]],
            ["op": "frame", "id": 7, "x": 20.0, "y": 120.0, "w": 100.0, "h": 44.0],
            ["op": "create", "id": 2, "kind": "view", "handlers": handlers, "props": ["id": "form", "semanticTag": "dialog"].merging(dialog) { $1 }],
            ["op": "frame", "id": 2, "x": 10.0, "y": 300.0, "w": 300.0, "h": 180.0],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "name"].merging(field) { $1 }, "handlers": ["input"]],
            ["op": "frame", "id": 3, "x": 20.0, "y": 20.0, "w": 260.0, "h": 40.0],
            ["op": "create", "id": 20, "kind": "view", "handlers": [],
             "props": ["id": "confirm", "semanticTag": "dialog", "accessibilityRole": "alertdialog", "closedby": confirmClosedby, "accessibilityLabel": "Remove book?"]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 500.0, "w": 300.0, "h": 120.0],
            ["op": "create", "id": 21, "kind": "text", "props": ["text": "It will be removed."], "handlers": []],
        ]
        ops += native(4, closes, press: true, y: 80) + native(5, closes, press: false, y: 130)
        ops += native(22, confirms.merging(["destructive": "true"]) { $1 }, press: true, y: 40) + native(23, confirms, press: false, y: 80)
        ops += [["op": "children", "id": 2, "ids": [3, 4, 5]], ["op": "children", "id": 20, "ids": [21, 22, 23]],
                ["op": "children", "id": 1, "ids": [8, 7, 2, 20]], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        return (p, controller, scene != nil)
    }
    private func sheet(_ controller: UIViewController) throws -> DialogSheetController {
        try XCTUnwrap(controller.presentedViewController as? DialogSheetController, "the dialog's sheet")
    }

    /// A key at the field in the sheet reaches the dialog's authored
    /// ancestors' key handlers; an authored `inert` still silences one.
    func testKeysAtTheSheetFieldReachTheDialogsAuthoredAncestors() throws {
        let (p, controller, scene) = page(field: ["autofocus": "true"])
        let root = try XCTUnwrap(p.views[1]), node = try XCTUnwrap(p.views[3])
        root.handlers = ["keycapture", "key", "keyup"]
        node.handlers.formUnion(["key", "keyup"])
        var heard: [String] = []
        p.onKey = { id, press in heard.append("\(id) \(press.capture ? "capture" : press.up ? "up" : "down") \(press)") }
        p.dialogCommand("showModal", "form")
        let sheet = try sheet(controller), field = try XCTUnwrap(node.field)
        if scene { settle { !p.menus.inTransition && field.isFirstResponder } }
        XCTAssertTrue(field.isDescendant(of: sheet.view))
        XCTAssertFalse(node.isDescendant(of: root))
        XCTAssertFalse(p.keyDown(at: node, "a", code: "KeyA"))
        XCTAssertFalse(p.keyUp(at: node, "a", code: "KeyA"))
        XCTAssertEqual(heard, ["1 capture a", "3 down a", "1 down a", "3 up a", "1 up a"])
        heard = []
        root.props["inert"] = "true"
        XCTAssertFalse(p.keyDown(at: node, "a", code: "KeyA"))
        XCTAssertEqual(heard, ["3 down a"])
    }

    /// A form in a `dialog` is a sheet as tall as the form, holding the
    /// dialog's own views; closing it puts them back, hidden, where they were.
    func testADialogThatIsNotAConfirmationIsASheetOfItsContent() throws {
        let (p, controller, scene) = page()
        let dialog = try XCTUnwrap(p.views[2]), root = try XCTUnwrap(p.views[1])
        XCTAssertTrue(dialog.isHidden, "closed, it shows nothing")
        // What `tree --ax` reads: the ids of the elements under the session's roots.
        func read() -> [UInt32] {
            p.syncAccessibility()
            return (p.axElements(roots: [p.viewport] + p.menus.sheets.roots)["elements"] as? [[String: Any]] ?? []).compactMap { $0["id"] as? UInt32 }
        }
        XCTAssertTrue(read().contains(7), "the page's button, before")
        p.dialogCommand("showModal", "nothing")
        XCTAssertNil(controller.presentedViewController, "no such id: refused, nothing shown")
        p.dialogCommand("showModal", "form")
        let sheet = try sheet(controller)
        XCTAssertEqual(sheet.modalPresentationStyle, .pageSheet, "UIKit's sheet")
        XCTAssertTrue(dialog.isDescendant(of: sheet.view), "the dialog's own subtree is the sheet's content")
        XCTAssertFalse(dialog.isHidden)
        XCTAssertTrue(try XCTUnwrap(p.views[3]?.field).isDescendant(of: sheet.view), "its field with it")
        XCTAssertEqual(sheet.box, CGSize(width: 300, height: 180), "the box the kernel laid out")
        XCTAssertEqual(sheet.sheetPresentationController?.detents.count, 1, "one resting height, the content's")
        XCTAssertFalse(sheet.sheetPresentationController?.prefersGrabberVisible ?? true)
        XCTAssertTrue(dialog.accessibilityViewIsModal, "VoiceOver stays in the dialog")
        XCTAssertTrue(p.viewport.accessibilityElementsHidden, "and does not reach the page beneath")
        XCTAssertTrue(p.menus.sheets.roots == [sheet.view], "the sheet is a root of the tree `tree --ax` reads")
        XCTAssertFalse(read().contains(7), "which holds nothing of the page beneath")
        XCTAssertTrue(p.menus.isOpen(dialog))
        XCTAssertEqual(p.menus.observation()?["kind"] as? String, "dialog")
        XCTAssertEqual(p.menus.observation()?["presentation"] as? String, "sheet")
        XCTAssertEqual(p.menus.observation()?["closedby"] as? String, "closerequest", "HTML's default")
        XCTAssertFalse(p.menus.ownsConfirmationNode(try XCTUnwrap(p.views[4])), "its buttons are buttons, not an alert's actions")
        XCTAssertTrue(p.shortcutModal === dialog, "only shortcuts inside it are heard")
        p.dialogCommand("showModal", "form")
        XCTAssertTrue(controller.presentedViewController === sheet, "showModal on an open one does nothing")
        // A batch while it is open: the page's children are set again and the
        // dialog grows. It stays in its sheet, which follows its box.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [8, 7, 2, 20]], ["op": "frame", "id": 2, "x": 10.0, "y": 300.0, "w": 300.0, "h": 240.0]]))
        XCTAssertTrue(dialog.isDescendant(of: sheet.view))
        XCTAssertFalse(dialog.isHidden, "a batch does not hide an open dialog")
        XCTAssertEqual(sheet.box.height, 240)
        sheet.view.frame = CGRect(x: 0, y: 0, width: 400, height: 274)
        sheet.view.layoutIfNeeded()
        let shown = dialog.convert(dialog.bounds, to: sheet.view)
        XCTAssertEqual(shown.midX, sheet.view.bounds.midX, accuracy: 0.5, "centred across the sheet, as HTML centres a modal dialog")
        XCTAssertEqual(shown.minY, 0, accuracy: 0.5, "at its top")
        XCTAssertEqual(sheet.scroller.contentSize, CGSize(width: 400, height: 240), "nothing to scroll while it fits")
        if scene {
            settle { !p.menus.inTransition }
            XCTAssertEqual(sheet.view.bounds.height - sheet.view.safeAreaInsets.bottom, 240, accuracy: 1, "the sheet is as tall as the dialog")
            XCTAssertLessThan(sheet.view.bounds.height, try XCTUnwrap(window).bounds.height - 200, "not the large detent")
        }

        p.dialogCommand("close", "form")
        if scene { settle { controller.presentedViewController == nil && !p.menus.inTransition } }
        XCTAssertTrue(dialog.superview === root.container, "back under its authored parent")
        XCTAssertEqual(root.container.subviews.compactMap { ($0 as? NodeView)?.id }, [8, 7, 2, 20], "at its place among its siblings")
        XCTAssertTrue(dialog.isHidden)
        XCTAssertEqual(dialog.frame, CGRect(x: 10, y: 300, width: 300, height: 240), "at the kernel's frame")
        XCTAssertFalse(p.menus.isOpen(dialog))
        XCTAssertNil(p.menus.observation())
        XCTAssertNil(p.shortcutModal)
        XCTAssertFalse(p.viewport.accessibilityElementsHidden, "the page is VoiceOver's again")
        XCTAssertFalse(dialog.accessibilityViewIsModal)
        XCTAssertTrue(read().contains(7))
    }

    /// A touch in the sheet reaches the dialog's authored ancestors, as DOM's
    /// events bubble out of the top layer: the sheet's view carries their
    /// pointer observer, long press and double tap (`MountRelay`), and a
    /// listener in the sheet is nearer and keeps its touch.
    func testATouchInTheSheetReachesTheDialogsAuthoredAncestors() throws {
        let (p, controller, _) = page()
        let root = try XCTUnwrap(p.views[1]), dialog = try XCTUnwrap(p.views[2]), field = try XCTUnwrap(p.views[3])
        root.handlers = ["pointerdown", "pointerup", "dblclick"]
        p.dialogCommand("showModal", "form")
        let sheet = try sheet(controller)
        let relay = try XCTUnwrap(sheet.view.gestureRecognizers?.lazy.compactMap { ($0 as? PointerRecognizer)?.relay }.first, "the sheet relays its touches")
        XCTAssertTrue(relay.listener(field) { $0.wantsPointer } === root, "a press in the dialog reaches the page's pointerdown")
        XCTAssertTrue(relay.listener(field) { $0.handlers.contains("dblclick") } === root)
        dialog.handlers = ["pointerdown"]
        XCTAssertNil(relay.listener(field) { $0.wantsPointer }, "the dialog's own observer sends it, its ancestors' included")
    }

    /// Its invoker opens it, and its Save presses once and closes it.
    func testItsInvokerOpensItAndItsSaveClosesIt() throws {
        let (p, controller, scene) = page()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let dialog = try XCTUnwrap(p.views[2])
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[7])), true, "a button with command=show-modal opens it")
        _ = try sheet(controller)
        XCTAssertEqual(p.menus.observation()?["source"] as? Int, 7)
        if scene { settle { !p.menus.inTransition } }
        XCTAssertNil(p.menus.activate(try XCTUnwrap(p.views[4])), "Save takes an ordinary tap: it is no alert action")
        p.press(4)
        XCTAssertEqual(pressed, [4], "its press, once")
        if scene { settle { controller.presentedViewController == nil && !p.menus.inTransition } }
        XCTAssertFalse(p.menus.isOpen(dialog), "and its command closes the sheet")
        XCTAssertTrue(dialog.isHidden)
    }

    /// A button that only closes it (`command="close"`, no `press`) closes it.
    func testItsCancelButtonClosesIt() throws {
        let (p, controller, scene) = page()
        let dialog = try XCTUnwrap(p.views[2])
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        if scene { settle { !p.menus.inTransition } }
        XCTAssertTrue(try XCTUnwrap(p.views[5]).takesPress, "a tap on it is its own")
        p.press(5)
        if scene { settle { controller.presentedViewController == nil && !p.menus.inTransition } }
        XCTAssertFalse(p.menus.isOpen(dialog))
        XCTAssertTrue(dialog.isHidden)
    }

    /// LLP 1115.000 D13.4: an alert has no outside. The same rows whose
    /// author wrote `closedby="any"` are the platform's dismissible form:
    /// an action sheet on a compact screen (a tap outside it is its Cancel),
    /// a popover on a regular one; and never the alert, which a tap outside
    /// cannot dismiss. Under `closedby="none"` it is no native confirmation
    /// at all (an alert always has its Cancel): the dialog's own sheet,
    /// which refuses its swipe.
    func testAnAlertShapedDialogWrittenClosedbyAnyIsTheDismissibleForm() throws {
        let (p, controller, _) = page(confirmClosedby: "any")
        defer { dismiss(p, controller) }
        p.dialogCommand("showModal", "confirm")
        let sheet = try XCTUnwrap(controller.presentedViewController as? UIAlertController, "still the platform's confirmation")
        let compact = controller.traitCollection.horizontalSizeClass != .regular
        XCTAssertEqual(sheet.preferredStyle, .actionSheet, "not the alert: an alert has no outside")
        XCTAssertEqual(sheet.title, "Remove book?")
        XCTAssertEqual(sheet.message, "It will be removed.")
        XCTAssertEqual(sheet.actions.map(\.title), ["Remove", "Cancel"])
        XCTAssertEqual(sheet.actions.map(\.style), [.destructive, .cancel])
        XCTAssertEqual(p.menus.observation()?["presentation"] as? String, compact ? "sheet" : "popover")
    }
    func testAnAlertShapedDialogWrittenClosedbyNoneIsTheDialogsSheet() throws {
        let (p, controller, _) = page(confirmClosedby: "none")
        defer { dismiss(p, controller) }
        p.dialogCommand("showModal", "confirm")
        XCTAssertNil(controller.presentedViewController as? UIAlertController, "no alert refuses its Cancel")
        let own = try sheet(controller)
        XCTAssertTrue(try XCTUnwrap(p.views[20]).isDescendant(of: own.view))
        XCTAssertTrue(own.isModalInPresentation, "and its swipe is refused")
    }

    /// The alert's shape is the alert and never a sheet: the same page, its
    /// confirmation asked for first.
    func testADialogInTheAlertsShapeIsStillTheAlert() throws {
        let (p, controller, scene) = page()
        p.views[20]?.handlers.insert("close")
        var closed: [UInt32] = []
        p.onClose = { closed.append($0) }
        p.dialogCommand("showModal", "confirm")
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController, "the alert shape is not a sheet")
        XCTAssertEqual(alert.preferredStyle, .alert)
        XCTAssertEqual(alert.title, "Remove book?")
        XCTAssertEqual(alert.message, "It will be removed.")
        XCTAssertEqual(alert.actions.map(\.title), ["Remove", "Cancel"])
        XCTAssertEqual(p.menus.observation()?["kind"] as? String, "confirmation")
        XCTAssertTrue(p.menus.ownsConfirmationNode(try XCTUnwrap(p.views[22])))
        XCTAssertTrue(try XCTUnwrap(p.views[20]).isHidden, "its rows are the alert's, never shown")
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        settle { !p.menus.inTransition }
        p.dialogCommand("close", "confirm")
        settle { closed.count == 1 }
        XCTAssertEqual(closed, [20])
    }

    func testAConfirmationOverASheetMovesTheModalBoundary() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        p.dialogCommand("showModal", "form")
        let sheet = try sheet(controller)
        settle { !p.menus.inTransition }
        let previous = p.openTopLayer
        p.dialogCommand("showModal", "confirm")
        settle { !p.menus.inTransition }
        XCTAssertTrue(sheet.presentedViewController is UIAlertController)
        let open = p.openTopLayer
        XCTAssertEqual(open.frames.last?.0, 20)
        XCTAssertEqual(open.modal, open.frames.count - 1)
        p.dialogCommand("close", "confirm")
        settle { !p.menus.inTransition }
        let restored = p.openTopLayer
        XCTAssertEqual(restored.modal, previous.modal)
        XCTAssertEqual(restored.frames.map { $0.0 }, previous.frames.map { $0.0 })
    }

    /// Capture without presenting: UIKit may deliver the action callback
    /// after dismissing the alert. This tests that path without a scene.
    private final class DismissedAlertController: UIViewController {
        var alert: UIAlertController?
        override func present(_ viewControllerToPresent: UIViewController, animated: Bool, completion: (() -> Void)? = nil) {
            alert = viewControllerToPresent as? UIAlertController
            completion?()
        }
    }

    func testDismissalStillClosesWhenTheChosenActionChangesBeforeItsTurn() throws {
        let controller = DismissedAlertController()
        let (p, _, _) = page(controller: controller)
        p.views[20]?.handlers.insert("close")
        var pressed: [UInt32] = [], closed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.onClose = { closed.append($0) }
        func choose() throws {
            p.dialogCommand("showModal", "confirm")
            let action = try XCTUnwrap(controller.alert?.actions.first)
            typealias Handler = @convention(block) (UIAlertAction) -> Void
            let block = try XCTUnwrap(action.value(forKey: "handler") as AnyObject?)
            unsafeBitCast(block, to: Handler.self)(action)
        }
        func drain() {
            let turned = expectation(description: "dismissal callbacks drained")
            DispatchQueue.main.async { DispatchQueue.main.async { turned.fulfill() } }
            wait(for: [turned], timeout: 5)
        }
        try choose()
        drain()
        XCTAssertEqual(pressed, [22])
        XCTAssertEqual(closed, [20])
        pressed = []; closed = []
        try choose()
        p.apply(wireBatch([["op": "props", "id": 22, "set": ["disabled": "true"], "clear": []]]))
        drain()
        XCTAssertEqual(pressed, [], "a stale action must not press")
        XCTAssertEqual(closed, [20], "the mounted dialog already closed")
    }

    /// `closedby` is the sheet's dismissal, as a modal route's: `none`
    /// refuses the swipe down, HTML's default permits it, and `any` also
    /// takes a tap on the dimmed page. It follows the prop while open.
    func testClosedbyIsTheSheetsDismissal() throws {
        let (p, controller, _) = page(dialog: ["closedby": "none"])
        p.dialogCommand("showModal", "form")
        let sheet = try sheet(controller), sheets = p.menus.sheets
        let presentation = try XCTUnwrap(sheet.presentationController)
        XCTAssertTrue(sheet.isModalInPresentation, "none: the swipe down is refused")
        XCTAssertFalse(sheets.presentationControllerShouldDismiss(presentation))
        XCTAssertFalse(sheets.closesOnBackdrop(sheet))
        sheets.closeRequest(sheet)
        XCTAssertTrue(p.menus.isOpen(try XCTUnwrap(p.views[2])), "nor does a close request close it")
        p.apply(wireBatch([["op": "props", "id": 2, "set": [:], "clear": ["closedby"]]]))
        XCTAssertFalse(sheet.isModalInPresentation, "unwritten is closerequest: the swipe down closes it")
        XCTAssertTrue(sheets.presentationControllerShouldDismiss(presentation))
        XCTAssertFalse(sheets.closesOnBackdrop(sheet), "a tap outside is not a close request")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["closedby": "any"], "clear": []]]))
        XCTAssertFalse(sheet.isModalInPresentation)
        XCTAssertTrue(sheets.presentationControllerShouldDismiss(presentation))
        XCTAssertTrue(sheets.closesOnBackdrop(sheet), "any: a tap on the dimmed page closes it too")
    }

    /// A dismissal the person made with UIKit's own gesture, the swipe down,
    /// puts the dialog back and fires HTML's `cancel` at it.
    func testASwipeDownIsItsCancel() throws {
        let (p, controller, scene) = page(handlers: ["cancel"])
        var cancelled: [UInt32] = []
        p.menus.sheets.onCancel = { cancelled.append($0) }
        let dialog = try XCTUnwrap(p.views[2]), home = try XCTUnwrap(p.views[1]).container
        p.dialogCommand("showModal", "form")
        let sheet = try sheet(controller)
        if scene { settle { !p.menus.inTransition } }
        // What UIKit does at the end of a swipe down: the sheet is gone, and
        // its delegate hears of it.
        sheet.dismiss(animated: false)
        p.menus.sheets.presentationControllerDidDismiss(try XCTUnwrap(sheet.presentationController))
        XCTAssertFalse(p.menus.isOpen(dialog))
        settle { !p.menus.sheets.owns(dialog) }
        XCTAssertEqual(cancelled, [2], "cancel, once")
        XCTAssertTrue(dialog.isHidden)
        XCTAssertTrue(dialog.superview === home)
    }

    /// A tap on the dimmed page of a `closedby="any"` dialog is its cancel too.
    func testATapOutsideIsItsCancel() throws {
        let (p, controller, _) = page(dialog: ["closedby": "any"], handlers: ["cancel"])
        var cancelled: [UInt32] = []
        p.menus.sheets.onCancel = { cancelled.append($0) }
        let dialog = try XCTUnwrap(p.views[2])
        p.dialogCommand("showModal", "form")
        p.menus.sheets.closeRequest(try sheet(controller))
        XCTAssertFalse(p.menus.isOpen(dialog))
        XCTAssertEqual(cancelled, [2])
        XCTAssertTrue(dialog.isHidden)
    }
    /// The app's own `close(id)` is no cancel.
    func testTheAppsOwnCloseIsNoCancel() throws {
        let (p, controller, _) = page(dialog: ["closedby": "any"], handlers: ["cancel", "close"])
        var closed: [UInt32] = []
        p.onClose = { closed.append($0) }
        var cancelled: [UInt32] = []
        p.menus.sheets.onCancel = { cancelled.append($0) }
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        p.dialogCommand("close", "form")
        XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[2])))
        spin { closed.count == 1 }
        XCTAssertEqual(closed, [2])
        XCTAssertEqual(cancelled, [])
    }

    /// The focus moves into the dialog and comes back: the page's field ends
    /// its editing as the sheet opens, the dialog's `autofocus` field takes
    /// the keyboard, and the page's field has it again after the close.
    func testTheFocusMovesInAndComesBack() throws {
        let (p, controller, scene) = page(field: ["autofocus": "true"])
        let search = try XCTUnwrap(p.views[8]?.field), name = try XCTUnwrap(p.views[3]?.field)
        XCTAssertTrue(search.becomeFirstResponder())
        p.dialogCommand("showModal", "form")
        XCTAssertFalse(search.isFirstResponder, "the page's field stops editing")
        _ = try sheet(controller)
        // The dialog's field takes the keyboard once the sheet is in a window.
        if scene { settle { name.isFirstResponder } }
        p.dialogCommand("close", "form")
        if scene { settle { controller.presentedViewController == nil && !p.menus.inTransition } }
        XCTAssertFalse(name.isFirstResponder)
        XCTAssertTrue(search.isFirstResponder, "back where it was")
    }

    /// A dialog destroyed while it is open takes its sheet with it.
    func testADialogDestroyedWhileOpenTakesItsSheet() throws {
        let (p, controller, scene) = page()
        p.dialogCommand("showModal", "form")
        let sheet = try sheet(controller)
        if scene { settle { !p.menus.inTransition } }
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [8, 7, 20]], ["op": "destroy", "id": 2]]))
        if scene { settle { controller.presentedViewController == nil && !p.menus.inTransition } }
        XCTAssertNil(p.menus.observation())
        XCTAssertTrue(sheet.scroller.subviews.compactMap { $0 as? NodeView }.isEmpty, "nothing of it is left in the sheet")
    }

    // MARK: the native fixture

    private func boot() throws -> ExactSession {
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: "dialog-sheet")
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
        kept = session.presenter
        return session
    }
    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }

    /// The fixture's `native-dialog`, `dialog width=320 padding=16
    /// closedby="any"` holding an input and two buttons: its button's
    /// `command="show-modal"` raises a sheet with the kernel's 320-point box
    /// in it, and its Close button's `command="close"` takes it down.
    func testTheFixturesLaidOutDialogOpensAsASheetAndItsCloseButtonClosesIt() throws {
        let session = try boot()
        let p = session.presenter
        let dialog = try XCTUnwrap(p.views.values.first { $0.props["id"] == "native-dialog" })
        XCTAssertTrue(dialog.isHidden)
        let opener = try node(session, "open-native-dialog")
        if p.menus.activate(opener) == nil { p.press(opener.id) }
        settle { p.menus.observation()?["kind"] as? String == "dialog" }
        let controller = try XCTUnwrap(p.viewport.window?.rootViewController)
        let sheet = try sheet(controller)
        XCTAssertTrue(dialog.isDescendant(of: sheet.view))
        XCTAssertEqual(sheet.box.width, 352, "the 320 its author wrote and its padding, CSS's content box")
        XCTAssertGreaterThan(sheet.box.height, 100, "its rows and padding")
        XCTAssertTrue(try node(session, "dialog-close").isDescendant(of: sheet.view))
        if Self.scene { settle { !p.menus.inTransition } }
        let close = try node(session, "dialog-close")
        if p.menus.activate(close) == nil { p.press(close.id) }
        if Self.scene { settle { controller.presentedViewController == nil && !p.menus.inTransition } }
        XCTAssertTrue(dialog.isHidden)
        XCTAssertFalse(p.menus.isOpen(dialog))
        XCTAssertNil(p.menus.observation())
    }
}
#endif

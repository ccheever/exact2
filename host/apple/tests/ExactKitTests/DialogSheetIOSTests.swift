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
    private final class SheetTouch: UITouch {
        let target: UIView
        var point: CGPoint
        init(_ target: UIView, at point: CGPoint) { self.target = target; self.point = point; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { target.convert(point, to: view) }
    }

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
                      confirmClosedby: String = "closerequest", controller: UIViewController = UIViewController(),
                      session: ExactSession? = nil) -> (Presenter, UIViewController, Bool) {
        let p = session?.presenter ?? Presenter()
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

    /// A sheet with side insets (a phone in landscape, the iPhone Duo's
    /// status column) shows its dialog between them and scrolls nothing
    /// sideways: UIKit adds the insets around the content (`.always`), so the
    /// content is as wide as what shows between them, not as the sheet.
    func testASheetWithSideInsetsShowsTheDialogBetweenThemAndNoSidewaysRange() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "a sheet's safe area is its window's") }
        let dialog = try XCTUnwrap(p.views[2])
        p.dialogCommand("showModal", "form")
        let sheet = try sheet(controller)
        settle { !p.menus.inTransition }
        sheet.additionalSafeAreaInsets = UIEdgeInsets(top: 0, left: 40, bottom: 0, right: 40)
        sheet.view.layoutIfNeeded()
        let sv = sheet.scroller, inset = sv.adjustedContentInset
        XCTAssertEqual(inset.left, sheet.view.safeAreaInsets.left, accuracy: 0.5, "UIKit insets the sides")
        XCTAssertGreaterThanOrEqual(inset.left, 40)
        XCTAssertLessThan(sheet.box.width, sv.bounds.width - inset.left - inset.right, "the dialog fits between them")
        func centred(_ why: String) {
            let shown = dialog.convert(dialog.bounds, to: sheet.view), inset = sv.adjustedContentInset
            XCTAssertLessThanOrEqual(sv.contentSize.width + inset.left + inset.right, sv.bounds.width + 0.5, "no sideways range, \(why)")
            XCTAssertEqual(shown.midX, inset.left + (sv.bounds.width - inset.left - inset.right) / 2, accuracy: 0.5, "centred between the insets, \(why)")
        }
        centred("both sides")
        // One side only, as the iPhone Duo's status column is, then none again
        sheet.additionalSafeAreaInsets = UIEdgeInsets(top: 0, left: 0, bottom: 0, right: 40)
        sheet.view.layoutIfNeeded()
        centred("the right side")
        sheet.additionalSafeAreaInsets = .zero
        sheet.view.layoutIfNeeded()
        centred("no side")
        p.dialogCommand("close", "form")
        settle { controller.presentedViewController == nil && !p.menus.inTransition }
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

    func testAPressInTheSheetUsesItsContentBoxAndNearestAuthoredAncestor() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "a lift during the sheet's dismiss animation needs a presented sheet") }
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Plain content"]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 160, "h": 40],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
            ["op": "create", "id": 10, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 10, "ids": [1]], ["op": "roots", "ids": [10]],
            ["op": "frame", "id": 10, "x": 0, "y": 0, "w": 400, "h": 800],
        ]))
        let root = try XCTUnwrap(p.views[1]), dialog = try XCTUnwrap(p.views[2]), text = try XCTUnwrap(p.views[9])
        root.handlers = ["press", "focus"]
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        if scene { settle { !p.menus.inTransition } }
        root.frame.size = CGSize(width: 10, height: 10)
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        let point = CGPoint(x: text.bounds.midX, y: text.bounds.midY)
        let touch = SheetTouch(text, at: point)
        XCTAssertTrue(p.listenerParent(of: dialog) === root)
        XCTAssertTrue(p.viewport.accessibilityElementsHidden, "the modal keeps the page behind it")
        XCTAssertFalse(root.authoredInert)
        XCTAssertTrue(text.activationTarget(at: touch.location(in: nil)) === root)
        text.touchesBegan([touch], with: nil)
        XCTAssertTrue(root.pressContact?.bubbled == true)
        XCTAssertTrue(root.pressed)
        touch.point.x += 2
        text.touchesMoved([touch], with: nil)
        XCTAssertTrue(root.pressed)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1], "modal inertness does not silence the bubbling press")
        XCTAssertFalse(root.pressed)
        XCTAssertFalse(root.isFirstResponder)
        XCTAssertFalse(try XCTUnwrap(p.views[8]?.field).isFirstResponder)

        touch.point = point
        text.touchesBegan([touch], with: nil)
        XCTAssertTrue(root.pressed)
        touch.point = CGPoint(x: text.bounds.maxX + 100, y: text.bounds.maxY + 100)
        text.touchesMoved([touch], with: nil)
        XCTAssertTrue(root.pressed)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1])

        touch.point = point
        text.handlers.insert("press")
        XCTAssertTrue(text.activationTarget(at: touch.location(in: nil)) === text)
        text.touchesBegan([touch], with: nil)
        XCTAssertTrue(text.pressed)
        XCTAssertFalse(root.pressed)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1, 9])
        text.handlers = []

        // A page editor focused programmatically must not be blurred by a
        // click bubbling from the dialog, nor may the ancestor take focus.
        let search = try XCTUnwrap(p.views[8]?.field)
        XCTAssertTrue(search.becomeFirstResponder())
        text.touchesBegan([touch], with: nil)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1, 9, 1])
        XCTAssertTrue(search.isFirstResponder)
        XCTAssertFalse(root.isFirstResponder)

        root.props["inert"] = "true"
        XCTAssertNil(text.activationTarget(at: touch.location(in: nil)))
        text.touchesBegan([touch], with: nil)
        XCTAssertFalse(root.pressed)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1, 9, 1], "authored inertness still silences the ancestor")
        XCTAssertTrue(p.menus.isOpen(dialog))

        root.props.removeValue(forKey: "inert")
        presses = []
        text.touchesBegan([touch], with: nil)
        XCTAssertTrue(root.pressed)
        let mount = dialog.superview
        p.menus.sheets.close(dialog, animated: true)
        XCTAssertTrue(p.menus.inTransition)
        XCTAssertTrue(dialog.superview === mount, "the lift arrives before dismissal reparents the dialog")
        XCTAssertFalse(root.pressed, "closing cancels the ancestor before the animation ends")
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [], "a lift during dismissal presses nothing")
        XCTAssertFalse(root.pressed)
    }

    func testPointerDownClosingTheSheetRefusesTheLaterTouchDown() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "the touch begins during the sheet's dismissal animation") }
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Plain content"], "handlers": ["pointerdown", "pointerup"]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 160, "h": 40],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
        ]))
        let ancestor = try XCTUnwrap(p.views[1]), dialog = try XCTUnwrap(p.views[2]), content = try XCTUnwrap(p.views[9])
        ancestor.handlers = ["press", "pointerup"]
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        settle { !p.menus.inTransition }
        let observer = try XCTUnwrap(content.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first)
        let touch = SheetTouch(content, at: CGPoint(x: 20, y: 20)), event = UIEvent(), mount = dialog.superview
        var presses: [UInt32] = [], pointers: [String] = []
        p.onPress = { presses.append($0) }
        p.onPointer = { id, phase, _ in
            pointers.append("\(id) \(phase == .down ? "down" : "up")")
            if phase == .down { p.menus.sheets.close(dialog, animated: true) }
        }
        // UIKit delivers the recognizer's down before the hit view's.
        observer.touchesBegan([touch], with: event)
        XCTAssertEqual(pointers, ["9 down"])
        XCTAssertFalse(p.menus.sheets.isOpen(dialog))
        XCTAssertTrue(p.menus.inTransition)
        XCTAssertTrue(dialog.superview === mount)
        content.touchesBegan([touch], with: event)
        XCTAssertFalse(ancestor.pressed, "a closing sheet cannot arm the page ancestor")
        XCTAssertTrue(ancestor.pressContact.map(\.cancelled) ?? true, "no live contact reaches the ancestor")
        observer.touchesEnded([touch], with: event)
        content.touchesEnded([touch], with: event)
        XCTAssertEqual(presses, [])
        XCTAssertFalse(ancestor.pressed)
        XCTAssertEqual(pointers, ["9 down", "9 up", "1 up"], "pointerup still bubbles while the sheet dismisses")
    }

    func testPointerDownClosingTheSheetKeepsPressesInsideTheSheet() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "the touch lifts during the sheet's dismissal animation") }
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "view", "handlers": ["pointerdown", "pointerup"]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 160, "h": 40],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
        ]))
        let dialog = try XCTUnwrap(p.views[2]), content = try XCTUnwrap(p.views[9])
        let observer = try XCTUnwrap(content.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first)
        for target in [content, dialog] {
            content.handlers.remove("press"); dialog.handlers.remove("press")
            target.handlers.insert("press")
            p.dialogCommand("showModal", "form")
            _ = try sheet(controller)
            settle { !p.menus.inTransition }
            let touch = SheetTouch(content, at: CGPoint(x: 20, y: 20)), event = UIEvent(), mount = dialog.superview
            var presses: [UInt32] = []
            p.onPress = { presses.append($0) }
            p.onPointer = { _, phase, _ in if phase == .down { p.menus.sheets.close(dialog, animated: true) } }
            XCTAssertTrue(content.activationTarget(at: touch.location(in: nil)) === target)
            observer.touchesBegan([touch], with: event)
            XCTAssertFalse(p.menus.sheets.isOpen(dialog))
            XCTAssertTrue(p.menus.inTransition)
            XCTAssertTrue(dialog.superview === mount)
            XCTAssertFalse(p.cancelledPressTouches.contains(touch), "closing a containing sheet does not cross a press boundary")
            content.touchesBegan([touch], with: event)
            XCTAssertTrue(target.pressed)
            observer.touchesEnded([touch], with: event)
            content.touchesEnded([touch], with: event)
            XCTAssertEqual(presses, [target.id])
            XCTAssertFalse(target.pressed)
            settle { !p.menus.inTransition }
            observer.reset()
        }
    }

    func testPointerUpOrFocusClosingTheSheetKeepsItsOwnPress() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "the press finishes during the sheet's dismissal animation") }
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "button", "props": ["tabIndex": "0"], "handlers": ["press", "pointerup", "focus"]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 160, "h": 40],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
        ]))
        let dialog = try XCTUnwrap(p.views[2]), button = try XCTUnwrap(p.views[9])
        let observer = try XCTUnwrap(button.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first)
        for closesOnFocus in [false, true] {
            p.dialogCommand("showModal", "form")
            _ = try sheet(controller)
            settle { !p.menus.inTransition }
            let touch = SheetTouch(button, at: CGPoint(x: 20, y: 20)), event = UIEvent(), mount = dialog.superview
            var presses: [UInt32] = [], closes = 0
            func close() { closes += 1; p.menus.sheets.close(dialog, animated: true) }
            p.onPress = { presses.append($0) }
            p.onPointer = { _, phase, _ in if phase == .up && !closesOnFocus { close() } }
            p.onFocus = { id in if id == button.id && closesOnFocus { close() } }
            observer.touchesBegan([touch], with: event)
            button.touchesBegan([touch], with: event)
            XCTAssertTrue(button.pressed)
            observer.touchesEnded([touch], with: event)
            button.touchesEnded([touch], with: event)
            XCTAssertEqual(closes, 1)
            XCTAssertFalse(p.menus.sheets.isOpen(dialog))
            XCTAssertTrue(p.menus.inTransition)
            XCTAssertTrue(dialog.superview === mount, "the press finishes before the animated dismissal reparents the sheet")
            XCTAssertEqual(presses, [button.id], "closing on \(closesOnFocus ? "focus" : "pointerup") preserves the sheet's own press")
            XCTAssertFalse(button.pressed)
            settle { !p.menus.inTransition }
            observer.reset()
        }
    }

    func testNativeButtonFocusClosingTheSheetCancelsItsBubbledPress() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "the native action continues during the sheet's dismissal animation") }
        p.apply(wireBatch(native(9, [:], press: false, y: 80) + [
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
        ]))
        let ancestor = try XCTUnwrap(p.views[1]), dialog = try XCTUnwrap(p.views[2]), button = try XCTUnwrap(p.views[9])
        ancestor.handlers = ["press"]
        button.handlers = ["focus"]
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        settle { !p.menus.inTransition }
        var presses: [UInt32] = [], focuses: [UInt32] = []
        p.onPress = { presses.append($0) }
        p.onFocus = { id in
            focuses.append(id)
            if id == button.id { p.menus.sheets.close(dialog, animated: true) }
        }
        let mount = dialog.superview
        XCTAssertFalse(button.isFirstResponder)
        XCTAssertTrue(button.activationTarget(at: button.convert(CGPoint(x: 20, y: 20), to: nil)) === ancestor)
        try XCTUnwrap(p.controls.controls[9]).sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(focuses, [9])
        XCTAssertFalse(p.menus.sheets.isOpen(dialog))
        XCTAssertTrue(p.menus.inTransition)
        XCTAssertTrue(dialog.superview === mount)
        XCTAssertEqual(presses, [], "the focus callback ended the presentation that owned this action")
        XCTAssertFalse(ancestor.pressed)
    }

    func testNativeButtonFocusMakingTheAncestorUnavailableCancelsItsBubbledPress() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "the native button takes focus in the presented sheet") }
        p.apply(wireBatch(native(9, [:], press: false, y: 80) + native(10, [:], press: false, y: 130) + [
            ["op": "children", "id": 2, "ids": [3, 9, 10]],
        ]))
        let ancestor = try XCTUnwrap(p.views[1]), dialog = try XCTUnwrap(p.views[2])
        ancestor.handlers = ["press"]
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        settle { !p.menus.inTransition }
        for (id, property): (UInt32, String) in [(9, "inert"), (10, "disabled")] {
            p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["inert", "disabled"]]]))
            let button = try XCTUnwrap(p.views[id])
            button.handlers = ["focus"]
            var presses: [UInt32] = [], focuses: [UInt32] = []
            p.onPress = { presses.append($0) }
            p.onFocus = { focused in
                focuses.append(focused)
                if focused == id { p.apply(wireBatch([["op": "props", "id": 1, "set": [property: "true"], "clear": []]])) }
            }
            XCTAssertFalse(button.isFirstResponder)
            XCTAssertTrue(button.activationTarget(at: button.convert(CGPoint(x: 20, y: 20), to: nil)) === ancestor)
            try XCTUnwrap(p.controls.controls[id]).sendActions(for: .primaryActionTriggered)
            XCTAssertEqual(focuses, [id])
            XCTAssertEqual(ancestor.props[property], "true")
            XCTAssertEqual(presses, [], "focus made the bubbled target \(property)")
            XCTAssertTrue(p.menus.sheets.isOpen(dialog))
        }
    }

    func testNativeButtonFocusRechecksEveryNodeOfTheBubbledPath() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "the native button takes focus in the presented sheet") }
        p.apply(wireBatch(native(9, [:], press: false, y: 80) + [
            ["op": "create", "id": 30, "kind": "view"],
            ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "children", "id": 1, "ids": [8, 7, 20, 30]],
            ["op": "children", "id": 30, "ids": [2]],
            ["op": "create", "id": 31, "kind": "view"],
            ["op": "frame", "id": 31, "x": 0, "y": 0, "w": 300, "h": 180],
            ["op": "children", "id": 2, "ids": [3, 31]],
            ["op": "children", "id": 31, "ids": [9]],
        ]))
        let ancestor = try XCTUnwrap(p.views[1]), dialog = try XCTUnwrap(p.views[2]), button = try XCTUnwrap(p.views[9])
        let outside = try XCTUnwrap(p.views[30]), inside = try XCTUnwrap(p.views[31])
        ancestor.handlers = ["press"]
        button.handlers = ["focus"]
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        settle { !p.menus.inTransition }
        for node in [outside, inside, button] {
            for property in ["disabled", "inert"] {
                button.resignFirstResponder()
                var presses: [UInt32] = [], focuses: [UInt32] = []
                p.onPress = { presses.append($0) }
                p.onFocus = { focused in
                    focuses.append(focused)
                    if focused == button.id { p.apply(wireBatch([["op": "props", "id": node.id, "set": [property: "true"], "clear": []]])) }
                }
                XCTAssertFalse(button.isFirstResponder)
                XCTAssertTrue(button.activationTarget(at: button.convert(CGPoint(x: 20, y: 20), to: nil)) === ancestor)
                try XCTUnwrap(p.controls.controls[9]).sendActions(for: .primaryActionTriggered)
                XCTAssertEqual(focuses, [9])
                XCTAssertEqual(node.props[property], "true")
                XCTAssertEqual(presses, [], "focus made path node \(node.id) \(property)")
                XCTAssertTrue(p.menus.sheets.isOpen(dialog))
                p.apply(wireBatch([["op": "props", "id": node.id, "set": [:], "clear": [property]]]))
            }
        }
    }

    func testBubbledInvocationsRequireATargetInsideTheModal() throws {
        final class Links: ExactSessionDelegate {
            var urls: [String] = []
            func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
                if name == "openURL", let url = args.first as? String { urls.append(url) }
            }
        }
        let links = Links(), session = ExactApp.shared.makeSession(delegate: links, label: "modal-invocations")
        self.session = session
        let (p, _, scene) = page(session: session)
        p.apply(wireBatch(native(30, [:], press: true, y: 200) + [
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Plain content"]],
            ["op": "create", "id": 40, "kind": "view", "props": ["id": "inside", "popover": "manual"]],
            ["op": "create", "id": 41, "kind": "view", "props": ["id": "outside", "popover": "manual"]],
            ["op": "children", "id": 1, "ids": [8, 7, 30, 20, 41]],
            ["op": "children", "id": 30, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9, 40]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 120, "h": 40],
            ["op": "frame", "id": 40, "x": 0, "y": 0, "w": 60, "h": 40],
            ["op": "frame", "id": 41, "x": 0, "y": 0, "w": 60, "h": 40],
        ]))
        let ancestor = try XCTUnwrap(p.views[30]), content = try XCTUnwrap(p.views[9])
        let dialog = try XCTUnwrap(p.views[2]), inside = try XCTUnwrap(p.views[40]), outside = try XCTUnwrap(p.views[41])
        p.dialogCommand("showModal", "form")
        if scene { settle { !p.menus.inTransition } }
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        func press() {
            XCTAssertTrue(content.activate(at: content.convert(CGPoint(x: 20, y: 20), to: nil)) === ancestor)
        }
        for target in [outside, inside] {
            for command in [false, true] {
                ancestor.props = ["type": "button"].merging(command
                    ? ["commandfor": target.props["id"]!, "command": "show-popover"]
                    : ["popovertarget": target.props["id"]!, "popovertargetaction": "show"]) { $1 }
                press()
                XCTAssertEqual(presses, [30], "the ancestor's handler survives either target's eligibility")
                XCTAssertEqual(p.menus.isOpen(target), target === inside)
                XCTAssertTrue(p.menus.sheets.topDialog === dialog)
                presses = []
                p.menus.action("hidePopover", [target.props["id"]!])
            }
        }
        ancestor.props = ["type": "button", "commandfor": "confirm", "command": "show-modal"]
        press()
        XCTAssertEqual(presses, [30])
        XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[20])), "a dialog outside the modal subtree is unusable too")
        XCTAssertTrue(p.menus.sheets.topDialog === dialog)
        XCTAssertEqual(links.urls, [])
    }

    func testPopoverPressInsideModalUsesDirectInvocationRules() throws {
        let (p, _, scene) = page()
        guard scene else { throw missing("scene", "a popover lifts over a presented sheet only in a scene") }
        p.apply(wireBatch(native(30, ["popovertarget": "outside", "popovertargetaction": "show"], press: true, y: 80) + [
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Plain content"]],
            ["op": "create", "id": 40, "kind": "view", "props": ["id": "inside", "popover": "manual"]],
            ["op": "create", "id": 41, "kind": "view", "props": ["id": "outside", "popover": "manual"]],
            ["op": "children", "id": 1, "ids": [8, 7, 2, 20, 41]],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 30]],
            ["op": "children", "id": 30, "ids": [40]],
            ["op": "children", "id": 40, "ids": [9]],
            ["op": "frame", "id": 9, "x": 10, "y": 10, "w": 100, "h": 30],
            ["op": "frame", "id": 40, "x": 0, "y": 0, "w": 120, "h": 60],
            ["op": "frame", "id": 41, "x": 0, "y": 0, "w": 60, "h": 40],
        ]))
        let invoker = try XCTUnwrap(p.views[30]), content = try XCTUnwrap(p.views[9])
        let dialog = try XCTUnwrap(p.views[2]), inside = try XCTUnwrap(p.views[40]), outside = try XCTUnwrap(p.views[41])
        p.dialogCommand("showModal", "form")
        if scene { settle { !p.menus.inTransition } }
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        for bubbled in [false, true] {
            p.menus.action("showPopover", ["inside"])
            XCTAssertTrue(p.menus.isOpen(inside))
            XCTAssertTrue(p.listenerParent(of: inside) === invoker)
            XCTAssertTrue(invoker.isDescendant(of: dialog))
            XCTAssertFalse(outside.isDescendant(of: dialog))
            if bubbled {
                let touch = SheetTouch(content, at: CGPoint(x: 20, y: 20))
                content.touchesBegan([touch], with: nil)
                XCTAssertTrue(invoker.pressed)
                content.touchesEnded([touch], with: nil)
            } else {
                try XCTUnwrap(p.controls.controls[invoker.id]).sendActions(for: .primaryActionTriggered)
            }
            XCTAssertEqual(presses, [invoker.id], "\(bubbled ? "bubbled" : "direct") press runs the invoker once")
            XCTAssertTrue(p.menus.isOpen(outside), "a crossing inside the modal keeps direct invocation behavior")
            XCTAssertTrue(p.menus.sheets.topDialog === dialog)
            p.menus.action("hidePopover", ["outside"])
            p.menus.action("hidePopover", ["inside"])
            presses = []
        }
    }

    func testNativeButtonBubblesWithoutInvokingAPopoverOutsideTheModal() throws {
        let (p, _, scene) = page()
        p.apply(wireBatch(native(9, [:], press: false, y: 80) + native(30, [:], press: true, y: 200) + [
            ["op": "create", "id": 40, "kind": "view", "props": ["id": "outside", "popover": "manual"]],
            ["op": "children", "id": 1, "ids": [8, 7, 30, 20, 40]],
            ["op": "children", "id": 30, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
            ["op": "frame", "id": 40, "x": 0, "y": 0, "w": 60, "h": 40],
        ]))
        let ancestor = try XCTUnwrap(p.views[30]), dialog = try XCTUnwrap(p.views[2]), outside = try XCTUnwrap(p.views[40])
        ancestor.handlers = ["press"]
        ancestor.props["popovertarget"] = "outside"
        ancestor.props["popovertargetaction"] = "show"
        p.dialogCommand("showModal", "form")
        if scene { settle { !p.menus.inTransition } }
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        try XCTUnwrap(p.controls.controls[9]).sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(pressed, [30])
        XCTAssertFalse(p.menus.isOpen(outside), "the bubbled native action cannot open a popover outside the modal")
        XCTAssertTrue(p.menus.sheets.topDialog === dialog)
    }

    func testCapturedAgentPressIsCancelledWhileTheDialogIsStillDismissing() throws {
        let session = ExactApp.shared.makeSession(label: "closing-captured-press")
        self.session = session
        let (p, controller, scene) = page(session: session)
        guard scene else { throw missing("scene", "a captured lift during dismissal needs a presented sheet") }
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Plain content"]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 160, "h": 40],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
        ]))
        let ancestor = try XCTUnwrap(p.views[1]), dialog = try XCTUnwrap(p.views[2]), source = try XCTUnwrap(p.views[9])
        ancestor.handlers = ["press", "pointerup"]
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        settle { !p.menus.inTransition }
        let agent = Agent(session: session)
        var presses: [UInt32] = [], ups: [UInt32] = []
        p.onPress = { presses.append($0) }
        p.onPointer = { id, phase, _ in if phase == .up { ups.append(id) } }
        for target in [ancestor, source] {
            source.handlers = target === source ? ["press"] : []
            if !p.menus.sheets.isOpen(dialog) {
                settle { !p.menus.inTransition }
                p.dialogCommand("showModal", "form")
                settle { !p.menus.inTransition }
            }
            ups = []
            let down = try XCTUnwrap(agent.recognizedPan("down", ["id": 9]))
            XCTAssertNil(down["error"], "\(down)")
            XCTAssertTrue(agent.panContact?.reach?.action?.node === target)
            let contact = try XCTUnwrap(agent.panContact), reach = try XCTUnwrap(contact.reach)
            XCTAssertTrue(reach.presentationsOpen)
            let mount = dialog.superview
            p.menus.sheets.close(dialog, animated: true)
            XCTAssertEqual(reach.presentationsOpen, target === source, "only a crossing press depends on the sheet staying open")
            XCTAssertTrue(p.menus.inTransition)
            XCTAssertTrue(dialog.superview === mount)
            XCTAssertNotNil(source.window)
            let point = source.convert(CGPoint(x: source.bounds.midX, y: source.bounds.midY), to: nil)
            XCTAssertTrue(source.bounds.contains(source.local(point)), "lift inside the content's current box, even while it moves")
            XCTAssertEqual(agent.stillPresses(reach) === target, target === source, "the bubbled path includes the captured opening")
            agent.panContact?.at = p.client(point)
            let up = try XCTUnwrap(agent.recognizedPan("up", [:]))
            XCTAssertEqual(up["pressed"] as? Int, target === source ? Int(source.id) : nil)
            XCTAssertEqual(presses, target === source ? [source.id] : [], "the mounted source keeps its own press during dismissal")
            XCTAssertEqual(ups, [1])
            XCTAssertFalse(target.pressed)
        }
    }

    func testAgentTapKeepsTheSheetsOwnPressWhenPointerUpClosesIt() throws {
        let session = ExactApp.shared.makeSession(label: "closing-own-press")
        self.session = session
        let (p, controller, scene) = page(session: session)
        guard scene else { throw missing("scene", "the tap finishes during the sheet's dismissal animation") }
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "view", "handlers": ["press", "pointerup"]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 160, "h": 40],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
        ]))
        let dialog = try XCTUnwrap(p.views[2])
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        settle { !p.menus.inTransition }
        let mount = dialog.superview
        var presses: [UInt32] = [], ups: [UInt32] = []
        p.onPress = { presses.append($0) }
        p.onPointer = { id, phase, _ in
            if phase == .up { ups.append(id); p.menus.sheets.close(dialog, animated: true) }
        }
        let tap = Agent(session: session).tap(["id": 9])
        XCTAssertNil(tap["error"], "\(tap)")
        XCTAssertEqual(ups, [9])
        XCTAssertFalse(p.menus.sheets.isOpen(dialog))
        XCTAssertTrue(p.menus.inTransition)
        XCTAssertTrue(dialog.superview === mount)
        XCTAssertEqual(tap["pressed"] as? Int, 9)
        XCTAssertEqual(presses, [9])
    }

    func testInlinePressSurvivesPointerDownClosingItsSheet() throws {
        let session = ExactApp.shared.makeSession(label: "closing-inline-press")
        self.session = session
        let (p, controller, scene) = page(session: session)
        guard scene else { throw missing("scene", "the inline touch finishes during the sheet's dismissal animation") }
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "text", "handlers": ["pointerdown"], "style": ["font_size": 16]],
            ["op": "paragraph", "id": 9, "runs": [
                ["id": 10, "parent": 9, "paint": true, "props": ["text": "Read the guide"], "style": [:], "handlers": ["press"]],
            ]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 160, "h": 40],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
        ]))
        let ancestor = try XCTUnwrap(p.views[1]), dialog = try XCTUnwrap(p.views[2]), paragraph = try XCTUnwrap(p.views[9])
        ancestor.handlers = ["press"]
        let observer = try XCTUnwrap(paragraph.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first)
        let agent = Agent(session: session)
        for recognized in [false, true] {
            p.dialogCommand("showModal", "form")
            _ = try sheet(controller)
            settle { !p.menus.inTransition }
            let fragment = try XCTUnwrap(paragraph.inlineRects(try XCTUnwrap(p.inlineText(10))).first)
            let point = CGPoint(x: fragment.midX, y: fragment.midY)
            XCTAssertTrue(paragraph.activationTarget(at: paragraph.convert(point, to: nil)) === ancestor)
            let mount = dialog.superview
            var presses: [UInt32] = [], downs: [UInt32] = []
            p.onPress = { presses.append($0) }
            p.onPointer = { id, phase, _ in
                if phase == .down { downs.append(id); p.menus.sheets.close(dialog, animated: true) }
            }
            if recognized {
                let tap = agent.tap(["id": 9, "at": [Double(point.x), Double(point.y)]])
                XCTAssertNil(tap["error"], "\(tap)")
                XCTAssertEqual(tap["pressed"] as? Int, 10)
            } else {
                let touch = SheetTouch(paragraph, at: point), event = UIEvent()
                observer.touchesBegan([touch], with: event)
                paragraph.touchesBegan([touch], with: event)
                XCTAssertFalse(p.cancelledPressTouches.contains(touch))
                XCTAssertEqual(paragraph.inlinePressed, 10)
                observer.touchesEnded([touch], with: event)
                paragraph.touchesEnded([touch], with: event)
                observer.reset()
            }
            XCTAssertEqual(downs, [9])
            XCTAssertFalse(p.menus.sheets.isOpen(dialog))
            XCTAssertTrue(p.menus.inTransition)
            XCTAssertTrue(dialog.superview === mount)
            XCTAssertEqual(presses, [10], "\(recognized ? "agent" : "touch") activates the inline run once")
            settle { !p.menus.inTransition }
        }
    }

    func testModalPressActivatesAncestorLinkAndCommand() throws {
        final class Links: ExactSessionDelegate {
            var urls: [String] = []
            func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
                if name == "openURL", let url = args.first as? String { urls.append(url) }
            }
        }
        let links = Links(), url = "https://example.test/bubbled"
        let session = ExactApp.shared.makeSession(delegate: links, label: "modal-activation")
        self.session = session
        let (p, controller, scene) = page(session: session)
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Plain content"]],
            ["op": "create", "id": 10, "kind": "button", "props": ["href": url]],
            ["op": "children", "id": 1, "ids": [8, 7, 10, 20]],
            ["op": "children", "id": 10, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 9]],
            ["op": "frame", "id": 9, "x": 120, "y": 80, "w": 160, "h": 40],
            ["op": "frame", "id": 10, "x": 0, "y": 0, "w": 10, "h": 10],
        ]))
        let ancestor = try XCTUnwrap(p.views[10]), content = try XCTUnwrap(p.views[9]), dialog = try XCTUnwrap(p.views[2])
        p.dialogCommand("showModal", "form")
        _ = try sheet(controller)
        if scene { settle { !p.menus.inTransition } }
        // UIKit's sheet, not inertness, keeps a finger off the page behind it.
        XCTAssertFalse(ancestor.inert)
        let touch = SheetTouch(content, at: CGPoint(x: 20, y: 20))
        func tap() {
            content.touchesBegan([touch], with: nil)
            content.touchesEnded([touch], with: nil)
        }
        tap()
        XCTAssertEqual(links.urls, [url], "the bubbled link requests navigation")
        XCTAssertTrue(content.activate(at: touch.location(in: nil)) === ancestor)
        XCTAssertEqual(links.urls, [url, url], "recognized activation follows the same link")
        let agent = Agent(session: session), point = touch.location(in: nil)
        _ = agent.lift(agent.fingerReach(content, at: point), at: point, held: "")
        XCTAssertEqual(links.urls, [url, url, url], "the agent's captured lift follows the same link")
        ancestor.props["inert"] = "true"
        tap()
        XCTAssertEqual(links.urls, [url, url, url], "authored inertness still blocks the link")
        ancestor.props = ["commandfor": "form", "command": "close"]
        ancestor.handlers = ["press"]
        p.onPress = { _ in ancestor.props["inert"] = "true" }
        tap()
        XCTAssertTrue(p.menus.isOpen(dialog), "invocation rechecks authored inertness after the handler")
        ancestor.props.removeValue(forKey: "inert")
        ancestor.handlers = []
        p.onPress = nil
        tap()
        XCTAssertFalse(p.menus.isOpen(dialog), "the bubbled command closes the modal")
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

    func testAConfirmationPopoverKeepsThePageOrSheetsModalBoundary() throws {
        let (p, controller, scene) = page()
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        p.apply(wireBatch([
            ["op": "props", "id": 20, "set": ["popover": "auto"], "clear": []],
            ["op": "props", "id": 22, "set": ["command": "hide-popover"], "clear": []],
            ["op": "props", "id": 23, "set": ["command": "hide-popover"], "clear": []],
        ]))
        for overSheet in [false, true] {
            if overSheet {
                p.apply(wireBatch([["op": "children", "id": 1, "ids": [8, 7, 2]],
                                  ["op": "children", "id": 2, "ids": [3, 4, 5, 20]]]))
                p.dialogCommand("showModal", "form")
                settle { !p.menus.inTransition }
            }
            let previous = p.openTopLayer
            let host = overSheet ? try sheet(controller) : controller
            func checkBoundary() {
                let open = p.openTopLayer
                XCTAssertEqual(open.frames.last?.0, 20)
                XCTAssertEqual(open.modal, previous.modal, "a confirmation popover adds no modal boundary")
            }
            p.menus.action("showPopover", ["confirm"])
            settle { !p.menus.inTransition }
            XCTAssertTrue(host.presentedViewController is UIAlertController)
            checkBoundary()
            p.menus.action("hidePopover", ["confirm"])
            XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, previous.frames.map { $0.0 })
            p.menus.action("togglePopover", ["confirm"])
            checkBoundary()
            settle { !p.menus.inTransition }
            XCTAssertTrue(host.presentedViewController is UIAlertController)
            checkBoundary()
            p.menus.action("hidePopover", ["confirm"])
            settle { !p.menus.inTransition }
            XCTAssertEqual(p.openTopLayer.modal, previous.modal)
        }
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

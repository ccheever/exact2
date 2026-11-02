#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class ContextMenuKeyboardIOSTests: XCTestCase {
    private var window: UIWindow!

    override func tearDown() {
        window?.endEditing(true)
        window?.isHidden = true
        window = nil
    }

    private func fixture(scene: UIWindowScene? = nil) -> Presenter {
        let p = Presenter()
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        if let scene { window.frame = scene.coordinateSpace.bounds }
        window.rootViewController = UIViewController()
        p.viewport.frame = window.bounds
        window.rootViewController!.view.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 400],
            ["op": "create", "id": 2, "kind": "view", "props": ["tabIndex": "0"], "handlers": ["contextmenu", "key"]],
            ["op": "frame", "id": 2, "x": 20, "y": 40, "w": 200, "h": 80],
            ["op": "create", "id": 3, "kind": "button"],
            ["op": "frame", "id": 3, "x": 20, "y": 140, "w": 100, "h": 40],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
        ]))
        window.makeKeyAndVisible()
        return p
    }

    func testTabReachesAMessageAndBothMenuKeysRunItsActionAfterKeyHandlers() throws {
        let p = fixture(), message = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.moveFocus(backward: false))
        XCTAssertTrue(message.isFirstResponder)
        XCTAssertFalse(message.isButton)
        var heard: [String] = []
        p.onKey = { id, press in heard.append("key:\(id):\(press.chord)") }
        p.onClipboard = { id, kind, line in
            XCTAssertEqual(kind, 10)
            XCTAssertEqual(line.split(separator: ",").prefix(2).compactMap { Double($0) }, [100, 40])
            heard.append("menu:\(id)")
        }
        XCTAssertTrue(p.keyDown(at: message, "ContextMenu"))
        XCTAssertTrue(p.keyDown(at: message, "F10", held: "Shift+"))
        XCTAssertEqual(heard, ["key:2:ContextMenu", "menu:2", "key:2:Shift+F10", "menu:2"])
    }

    func testCancelledModifiedAndReleasedKeysDoNotOpenAMenu() throws {
        let p = fixture(), message = try XCTUnwrap(p.views[2])
        XCTAssertTrue(message.becomeFirstResponder())
        var heard = 0
        p.onClipboard = { _, _, _ in heard += 1 }
        p.onKey = { _, _ in p.defaultPrevented = true }
        XCTAssertTrue(p.keyDown(at: message, "ContextMenu"))
        XCTAssertTrue(p.keyDown(at: message, "F10", held: "Shift+"))
        p.onKey = nil
        XCTAssertFalse(p.keyDown(at: message, "ContextMenu", held: "Control+"))
        XCTAssertFalse(p.keyDown(at: message, "F10"))
        XCTAssertFalse(p.keyDown(at: message, "F10", held: "Shift+Meta+"))
        XCTAssertFalse(p.keyUp(at: message, "ContextMenu"))
        XCTAssertEqual(heard, 0)
        XCTAssertTrue(p.keyDown(at: message, "ContextMenu"))
        XCTAssertTrue(p.keyDown(at: message, "F10", held: "Shift+"))
        XCTAssertEqual(heard, 2, "the unmodified chords still open the menu")
    }

    func testTheMenuUsesTheFocusAfterTheKeyHandlerAndFindsItsAncestor() throws {
        let p = fixture(), message = try XCTUnwrap(p.views[2]), child = try XCTUnwrap(p.views[3])
        p.apply(wireBatch([["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]]]))
        XCTAssertTrue(message.becomeFirstResponder())
        p.onKey = { _, _ in XCTAssertTrue(child.becomeFirstResponder()) }
        var heard: [UInt32] = []
        p.onClipboard = { id, _, _ in heard.append(id) }
        XCTAssertTrue(p.keyDown(at: message, "ContextMenu"))
        XCTAssertEqual(heard, [2])
        XCTAssertTrue(child.isFirstResponder)
    }

    func testAKeyHandlerCannotLeaveAMenuOnAnInertOrRemovedMessage() throws {
        let p = fixture(), message = try XCTUnwrap(p.views[2])
        XCTAssertTrue(message.becomeFirstResponder())
        var heard = 0
        p.onClipboard = { _, _, _ in heard += 1 }
        p.onKey = { _, _ in message.applyProps(set: ["inert": "true"], clear: []) }
        XCTAssertFalse(p.keyDown(at: message, "ContextMenu"))
        XCTAssertEqual(heard, 0)
        message.applyProps(set: [:], clear: ["inert"])
        p.onKey = nil
        XCTAssertTrue(message.becomeFirstResponder())
        XCTAssertTrue(p.keyDown(at: message, "ContextMenu"))
        XCTAssertEqual(heard, 1, "the same source works while eligible")
        p.onKey = { _, _ in p.apply(wireBatch([["op": "destroy", "id": 2]])) }
        XCTAssertFalse(p.keyDown(at: message, "ContextMenu"))
        XCTAssertEqual(heard, 1)
    }

    func testAnEditorsMenuStaysWithTheEditor() throws {
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "input"],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 100, "h": 40],
            ["op": "children", "id": 2, "ids": [4]],
        ]))
        let editor = try XCTUnwrap(p.views[4])
        XCTAssertTrue(try XCTUnwrap(editor.field).becomeFirstResponder())
        var heard = 0
        p.onClipboard = { _, _, _ in heard += 1 }
        XCTAssertFalse(p.keyDown(at: editor, "ContextMenu"))
        XCTAssertFalse(p.keyDown(at: editor, "F10", held: "Shift+"))
        XCTAssertEqual(heard, 0)
        let message = try XCTUnwrap(p.views[2])
        XCTAssertTrue(message.becomeFirstResponder())
        XCTAssertTrue(p.keyDown(at: message, "ContextMenu"))
        XCTAssertTrue(p.keyDown(at: message, "F10", held: "Shift+"))
        XCTAssertEqual(heard, 2, "leaving the editor restores the ancestor's menu")
    }

    func testMenuKeysPresentThePopoverWithAgentModeOff() throws {
        guard let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first else {
            throw missing("scene", "a keyboard menu must present in a real window scene")
        }
        XCTAssertFalse(ExactEnv.agentPaints)
        let p = fixture(scene: scene), message = try XCTUnwrap(p.views[2])
        defer { p.menus.reset() }
        p.apply(wireBatch([
            ["op": "props", "id": 2, "set": ["contextPopover": "actions"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "actions", "popover": "auto", "accessibilityRole": "menu"]],
            ["op": "create", "id": 5, "kind": "button", "handlers": ["press"]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Before the handler"]],
            ["op": "children", "id": 5, "ids": [6]], ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let pop = try XCTUnwrap(p.views[4])
        var heard = 0
        p.onClipboard = { _, _, _ in
            heard += 1
            p.apply(wireBatch([["op": "props", "id": 6, "set": ["text": "Reply after handler"]]]))
        }
        func shows(_ view: UIView) -> Bool {
            guard !view.isHidden, view.alpha > 0 else { return false }
            if (view as? UILabel)?.text == "Reply after handler" { return true }
            return view.subviews.contains(where: shows)
        }
        func until(_ condition: () -> Bool) {
            let limit = Date(timeIntervalSinceNow: 5)
            while !condition(), Date() < limit { _ = RunLoop.main.run(mode: .default, before: limit) }
        }
        for handler in [true, false] {
            if !handler { message.handlers.remove("contextmenu") }
            XCTAssertTrue(message.becomeFirstResponder())
            XCTAssertTrue(p.keyDown(at: message, handler ? "F10" : "ContextMenu", held: handler ? "Shift+" : ""))
            until { p.menus.isOpen(pop) && scene.windows.contains(where: shows) }
            XCTAssertTrue(p.menus.isOpen(pop), "a popover opens with or without an authored contextmenu handler")
            XCTAssertTrue(scene.windows.contains(where: shows), "UIKit displays the action, after the handler updates it")
            XCTAssertTrue(p.menus.context.isOpen(pop), "this is production menu presentation")
            XCTAssertEqual(heard, 1)
            p.menus.context.close(pop)
            until { !scene.windows.contains(where: shows) }
            XCTAssertFalse(p.menus.isOpen(pop))
        }
    }

}
#endif

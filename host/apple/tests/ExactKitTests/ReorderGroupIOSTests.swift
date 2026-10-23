#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactDrag

final class ReorderGroupIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactDrag.install() }
    private var window: UIWindow?
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    private final class Key: UIKey {
        var text = " "
        var code = UIKeyboardHIDUsage.keyboardSpacebar
        override var characters: String { text }
        override var charactersIgnoringModifiers: String { text }
        override var keyCode: UIKeyboardHIDUsage { code }
        override var modifierFlags: UIKeyModifierFlags { [] }
    }
    private final class Press: UIPress {
        let value = Key()
        override var key: UIKey? { value }
    }
    private func key(_ text: String, _ code: UIKeyboardHIDUsage, at node: NodeView) {
        let press = Press()
        press.value.text = text; press.value.code = code
        node.pressesBegan([press], with: nil)
        node.pressesEnded([press], with: nil)
    }

    private func fixture(_ calls: ReorderGroupRecorder) -> (Presenter, NodeView) {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        p.reorderGroupCalls = calls
        let entry = { (list: Int, wrapper: Int, root: Int) -> [String: Any] in
            ["view": list, "revision": 1, "scrollSequence": 0, "count": 1, "totalExtent": 40,
             "rows": [["view": wrapper, "root": root, "epoch": 7]], "correction": NSNull()]
        }
        p.apply(wireBatch([
            ["op": "collections", "items": [entry(1, 2, 3), entry(5, 6, 7)]],
            ["op": "create", "id": 9, "kind": "view"],
            ["op": "create", "id": 1, "kind": "list", "props": ["id": "col-a", "reorderGroup": "cards"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "props": ["reorderFor": "col-a"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "grip"]],
            ["op": "create", "id": 5, "kind": "list", "props": ["id": "col-b", "reorderGroup": "cards"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 6, "kind": "view"],
            ["op": "create", "id": 7, "kind": "view", "props": ["reorderFor": "col-b"]],
            ["op": "children", "id": 9, "ids": [1, 5]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 6, "ids": [7]],
            ["op": "roots", "ids": [9]],
            ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 150.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 150.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 150.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 10.0, "y": 10.0, "w": 100.0, "h": 20.0],
            ["op": "frame", "id": 5, "x": 200.0, "y": 0.0, "w": 150.0, "h": 200.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 150.0, "h": 40.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 150.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 150.0, "h": 200.0],
            ["op": "content", "id": 5, "w": 150.0, "h": 200.0]
        ]))
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        return (p, p.views[3]!)
    }

    func testAGroupedGripIsASequentialHardwareKeyboardStop() throws {
        let (p, grip) = fixture(ReorderGroupRecorder())
        XCTAssertTrue(grip.canBecomeFirstResponder)
        XCTAssertTrue(p.moveFocus(backward: false))
        XCTAssertTrue(grip.isFirstResponder)
    }

    func testHardwareKeysLiftMoveAndDropWithoutAGhost() throws {
        let calls = ReorderGroupRecorder(); calls.hold = false
        let (p, grip) = fixture(calls)
        grip.applyProps(set: ["tabIndex": "0"], clear: [])
        XCTAssertTrue(grip.becomeFirstResponder())
        key(" ", .keyboardSpacebar, at: grip)
        XCTAssertEqual(calls.log, ["begin 3"])
        XCTAssertNil(p.reorderGroup?.ghost)
        key(UIKeyCommand.inputUpArrow, .keyboardUpArrow, at: grip)
        key(UIKeyCommand.inputDownArrow, .keyboardDownArrow, at: grip)
        key(UIKeyCommand.inputLeftArrow, .keyboardLeftArrow, at: grip)
        key(UIKeyCommand.inputRightArrow, .keyboardRightArrow, at: grip)
        key("\r", .keyboardReturnOrEnter, at: grip)
        XCTAssertEqual(calls.log, ["begin 3", "step 1", "step 2", "step 3", "step 4", "drop", "finish"])
        XCTAssertNil(p.reorderGroup)
        XCTAssertTrue(p.views[7]?.isFirstResponder == true, "the landed grip keeps the keyboard")
    }

    func testEscapeCancelsAndAnAuthoredKeyHandlerKeepsItsKeys() throws {
        let calls = ReorderGroupRecorder(), (p, grip) = fixture(calls)
        grip.applyProps(set: ["tabIndex": "0"], clear: [])
        XCTAssertTrue(grip.becomeFirstResponder())
        key(" ", .keyboardSpacebar, at: grip)
        key(UIKeyCommand.inputEscape, .keyboardEscape, at: grip)
        XCTAssertEqual(calls.log, ["begin 3", "cancel", "finish"])
        XCTAssertNil(p.reorderGroup)
        calls.log = []
        grip.handlers = ["key"]
        var heard: [UInt32] = []
        p.onKey = { id, _ in heard.append(id) }
        key(" ", .keyboardSpacebar, at: grip)
        XCTAssertEqual(heard, [3])
        XCTAssertTrue(calls.log.isEmpty)
    }
}
#endif

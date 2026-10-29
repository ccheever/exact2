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

    private func fixture(_ calls: ReorderGroupRecorder, nested: Bool = false) -> (Presenter, NodeView) {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        p.reorderGroupCalls = calls
        let entry = { (list: Int, wrapper: Int, root: Int) -> [String: Any] in
            ["view": list, "revision": 1, "scrollSequence": 0, "count": 1, "totalExtent": 40,
             "rows": [["view": wrapper, "root": root, "epoch": 7]], "correction": NSNull()]
        }
        p.apply(wireBatch([
            ["op": "collections", "items": [entry(1, 2, 3), entry(5, 6, 7)]],
            ["op": "create", "id": 9, "kind": "view", "style": nested ? ["overflow_y": "scroll"] : [:]],
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

    /// At its last pixel read back a ULP off the grid's value (UIKit hands
    /// 5/3 back as 1.6666666666666665), a scroller can scroll no further
    /// toward that edge, so autoscroll passes to its ancestor (PixelGrid.same;
    /// Astra's audit on the smooth-correction livelock).
    func testAScrollerAtItsLastPixelReadBackOffTheGridCannotScrollOn() throws {
        let (p, _) = fixture(ReorderGroupRecorder(), nested: true)
        let inner = try XCTUnwrap(p.views[1]?.scroll)
        inner.traitOverrides.displayScale = 3
        inner.updateTraitsIfNeeded()
        inner.contentInsetAdjustmentBehavior = .never
        inner.contentInset = .zero
        // A grid offset UIKit reads back off the grid (the precondition, shown here), made the last one.
        var last: CGFloat?
        for k in 1...600 {
            let y = CGFloat(k) / 3 // (100 + k/3 is handed back exactly on this simulator; 5/3 is not)
            inner.contentSize.height = inner.bounds.height + y
            inner.contentOffset.y = PixelGrid.offset(CGPoint(x: 0, y: y), in: inner).y
            if inner.contentOffset.y != PixelGrid.offset(CGPoint(x: 0, y: y), in: inner).y { last = y; break }
        }
        guard let y = last else { throw XCTSkip("this display hands every grid offset back exactly at 3x: nothing to drive") }
        let candidate = ReorderScroller(scroll: inner, vertical: true, rect: .zero)
        XCTAssertFalse(candidate.canScroll(toward: 1), "at its last pixel (\(y), read back \(inner.contentOffset.y))")
        XCTAssertTrue(candidate.canScroll(toward: -1))
        window?.isHidden = true
    }

    func testAutoscrollPassesAFractionalInnerLimitToItsAncestor() throws {
        for scale in [2, 3] as [CGFloat] {
            let (p, grip) = fixture(ReorderGroupRecorder(), nested: true)
            let inner = try XCTUnwrap(p.views[1]?.scroll), outer = try XCTUnwrap(p.views[9]?.scroll)
            for scroll in [inner, outer] {
                scroll.traitOverrides.displayScale = scale
                scroll.updateTraitsIfNeeded()
                XCTAssertEqual(scroll.traitCollection.displayScale, scale)
                scroll.contentInsetAdjustmentBehavior = .never
                scroll.contentInset.bottom = 0.3
            }
            inner.contentSize.height = 300
            outer.contentSize.height = 400
            inner.contentOffset.y = 100
            let candidate = ReorderScroller(scroll: inner, vertical: true, rect: .zero)
            XCTAssertFalse(candidate.canScroll(toward: 1), "the last reachable pixel is 100 at both scales")
            XCTAssertFalse(candidate.scroll(by: 10), "a snapped-away request did not move")
            let hold = try XCTUnwrap(ReorderGroupHold(grip, point: CGPoint(x: 75, y: 190), ghost: false))
            XCTAssertTrue(hold.move(CGPoint(x: 75, y: 190)))
            let deadline = Date(timeIntervalSinceNow: 1)
            while outer.contentOffset.y == 0 && Date() < deadline {
                RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
            }
            XCTAssertGreaterThan(outer.contentOffset.y, 0, "the eligible ancestor must autoscroll")
            XCTAssertEqual(inner.contentOffset.y, 100, accuracy: 1e-6)
            hold.cancel()
            window?.isHidden = true
        }
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

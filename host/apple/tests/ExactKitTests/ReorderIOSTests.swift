#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass
import XCTest
@testable import ExactKit
@testable import ExactDrag

/// UIKit recognizes Arrange: a handle whose touch-action keeps the List's
/// pan off starts on a vertical drag; any handle starts on a long press. The
/// recognizer's phases make the web host's sequence (`ReorderMacTests`).
final class ReorderIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactDrag.install() } // LLP 1047.001 D4
    private var window: UIWindow!

    /// A pan whose phase, location and velocity the test sets.
    private final class Drag: UIPanGestureRecognizer {
        var at = CGPoint.zero, speed = CGPoint.zero
        // An unattached recognizer keeps UIKit's own state at .possible.
        private var phase = UIGestureRecognizer.State.possible
        override var state: UIGestureRecognizer.State { get { phase } set { phase = newValue } }
        override func location(in view: UIView?) -> CGPoint { at }
        override func translation(in view: UIView?) -> CGPoint { speed }
        override func velocity(in view: UIView?) -> CGPoint { speed }
    }

    /// A virtualized List (1) whose mounted row wrapper (2) holds a grip (3).
    private func fixture(_ calls: ReorderRecorder, touchAction: String = "none") -> (Presenter, NodeView) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.reorderCalls = calls
        p.apply(wireBatch([
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50,
                "totalExtent": 2000, "rows": [["view": 2, "root": 3, "epoch": 7]], "correction": NSNull()]]],
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "props": ["reorderFor": "items"], "style": ["touch_action": touchAction]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]
        ]))
        window.makeKeyAndVisible()
        return (p, p.views[3]!)
    }

    func testAHandleGetsADragAndALongPressAndTouchActionDecidesTheDrag() throws {
        let (p, grip) = fixture(ReorderRecorder())
        XCTAssertNotNil(grip.reorderPan)
        XCTAssertNotNil(grip.reorderPress)
        XCTAssertEqual(grip.reorderShouldBegin(try XCTUnwrap(grip.reorderPress)), true)
        let drag = Drag(); grip.reorderPan = drag
        drag.speed = CGPoint(x: 2, y: 40)
        XCTAssertEqual(grip.reorderShouldBegin(drag), true, "touch-action none: the handle drag starts")
        drag.speed = CGPoint(x: 40, y: 2)
        XCTAssertEqual(grip.reorderShouldBegin(drag), false, "a horizontal start is not Arrange")
        grip.applyStyle(["touch_action": "auto"])
        drag.speed = CGPoint(x: 2, y: 40)
        XCTAssertEqual(grip.reorderShouldBegin(drag), false, "scroll always wins: only a long press lifts")
        withExtendedLifetime(p) { grip.applyProps(set: [:], clear: ["reorderFor"]) }
        XCTAssertNil(grip.props["reorderFor"])
        XCTAssertNil(grip.reorderPan)
        XCTAssertNil(grip.reorderPress)
    }

    func testRecognizerPhasesCatchFollowAndDropOnceThenFinish() throws {
        let calls = ReorderRecorder()
        let (p, grip) = fixture(calls)
        XCTAssertNotNil(grip.window)
        XCTAssertTrue(SwipeInput.allows(grip))
        XCTAssertEqual(p.collections.owningCollection(3), 1)
        XCTAssertNotNil(p.views[1]?.scroll)
        XCTAssertNotNil(p.collections.geometry(1))
        let drag = Drag()
        drag.at = CGPoint(x: 20, y: 120)
        drag.state = .began
        XCTAssertEqual(drag.state, .began)
        grip.reorderDragged(drag)
        XCTAssertNotNil(grip.reorderHold)
        drag.at.y = 170; drag.state = .changed; grip.reorderDragged(drag)
        XCTAssertEqual(p.views[2]?.layer.zPosition, 0.001, "the lifted row paints above later rows")
        drag.at.y = 180; drag.state = .ended; grip.reorderDragged(drag)
        XCTAssertEqual(calls.log, ["begin 3 top=0.0", "move 7 dy=50.0", "drop 7 dy=60.0"])
        let hold = try XCTUnwrap(p.reorder as? ReorderHold)
        XCTAssertEqual(hold.phase, .settling)
        XCTAssertEqual(p.collections.interaction, 3, "the pin stays while the source returns")
        hold.observe(ReorderRecorder.finished())
        XCTAssertNil(p.reorder)
        XCTAssertEqual(p.views[2]?.layer.zPosition, 0)
    }

    func testACancelledRecognizerCancelsAndAPointerOutsideThePortIsSaid() {
        let calls = ReorderRecorder()
        let (p, grip) = fixture(calls)
        let drag = Drag()
        drag.at = CGPoint(x: 20, y: 120)
        drag.state = .began; grip.reorderDragged(drag)
        drag.at = CGPoint(x: 360, y: 150); drag.state = .changed; grip.reorderDragged(drag)
        drag.state = .cancelled; grip.reorderDragged(drag)
        XCTAssertEqual(calls.log, ["begin 3 top=0.0", "move 7 dy=30.0 outside", "cancel 7"])
        XCTAssertEqual((p.reorder as? ReorderHold)?.phase, .settling)
    }
}
#endif

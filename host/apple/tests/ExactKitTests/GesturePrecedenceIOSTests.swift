#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass
import XCTest
@testable import ExactKit

/// LLP 1057.001 on UIKit: the web's `dblclick` order. Recognizer phases are set
/// by the test (UIKit synthesizes no touches for a unit test), as elsewhere.
final class GesturePrecedenceIOSTests: XCTestCase {
    private var window: UIWindow!

    private final class Taps: UITapGestureRecognizer {
        private var phase = UIGestureRecognizer.State.possible
        override var state: UIGestureRecognizer.State { get { phase } set { phase = newValue } }
    }

    private func host(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch(ops))
        window.makeKeyAndVisible()
        return p
    }
    private func drain() {
        let turn = expectation(description: "a main-queue turn")
        DispatchQueue.main.async { turn.fulfill() }
        wait(for: [turn], timeout: 1)
    }

    func testTheSecondTapStillPressesAndDblclickComesAfterIt() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press", "dblclick"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0]
        ])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        p.onDblclick = { log.append("dblclick \($0)") }
        let node = try XCTUnwrap(p.views[1])
        let recognizer = try XCTUnwrap(node.doubleRecognizer)
        XCTAssertFalse(recognizer.cancelsTouchesInView, "the second tap's touches reach the press")
        XCTAssertFalse(recognizer.delaysTouchesEnded)
        let touch: Set<UITouch> = [UITouch()]
        node.touchesBegan(touch, with: nil); node.touchesEnded(touch, with: nil)
        // UIKit may run the recognizer's action before the view's touchesEnded.
        let taps = Taps(); taps.state = .ended
        node.touchesBegan(touch, with: nil)
        node.doubleClicked(taps)
        node.touchesEnded(touch, with: nil)
        drain()
        XCTAssertEqual(log, ["press 1", "press 1", "dblclick 1"], "the web's click, click, dblclick")
    }
}
#endif

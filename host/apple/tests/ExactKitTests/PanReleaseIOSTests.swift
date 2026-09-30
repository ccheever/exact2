#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass
import XCTest
@testable import ExactKit

/// LLP 1057 §10.6 on UIKit: `layoutPanning` releases once, after the final
/// delta, at UIKit's own velocity; a cancelled pan at rest. Recognizer phases
/// are set by the test (UIKit synthesizes no touches for a unit test).
final class PanReleaseIOSTests: XCTestCase {
    private var window: UIWindow!

    private final class Pan: UIPanGestureRecognizer {
        var phase = UIGestureRecognizer.State.possible
        var moved = CGPoint.zero, speed = CGPoint.zero
        override var state: UIGestureRecognizer.State { get { phase } set { phase = newValue } }
        override func translation(in view: UIView?) -> CGPoint { moved }
        override func velocity(in view: UIView?) -> CGPoint { speed }
    }

    private func host(_ handlers: [String]) -> (Presenter, NodeView) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": handlers],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0]
        ]))
        window.makeKeyAndVisible()
        return (p, p.views[1]!)
    }

    private func drive(_ node: NodeView, _ steps: [(UIGestureRecognizer.State, CGFloat)], speed: CGPoint = .zero) {
        let pan = Pan()
        pan.speed = speed
        for (state, x) in steps { pan.phase = state; pan.moved = CGPoint(x: x, y: 0); node.layoutPanning(pan) }
    }

    func testAnEndedPanReleasesOnceAfterItsFinalDeltaAtUIKitsVelocity() throws {
        let (p, node) = host(["pan", "panrelease"])
        XCTAssertNotNil(node.layoutPanRecognizer)
        var log: [String] = []
        p.onPan = { _, dx, _ in log.append("pan \(dx)") }
        p.onPanRelease = { id, vx, vy in log.append("release \(id) \(vx) \(vy)") }
        drive(node, [(.began, 10), (.changed, 30), (.ended, 45)], speed: CGPoint(x: 800, y: -20))
        XCTAssertEqual(log, ["pan 10.0", "pan 20.0", "pan 15.0", "release 1 800.0 -20.0"])
    }

    func testACancelledPanReleasesAtRest() {
        let (p, node) = host(["pan", "panrelease"])
        var releases: [[Double]] = []
        p.onPanRelease = { _, vx, vy in releases.append([vx, vy]) }
        drive(node, [(.began, 10), (.changed, 20), (.cancelled, 20)], speed: CGPoint(x: 500, y: 0))
        XCTAssertEqual(releases, [[0, 0]])
        drive(node, [(.began, 10), (.failed, 10)])
        XCTAssertEqual(releases, [[0, 0], [0, 0]])
    }

    func testANodeWithoutPanreleaseHearsNone() {
        let (p, node) = host(["pan"])
        var pans = 0, releases = 0
        p.onPan = { _, _, _ in pans += 1 }
        p.onPanRelease = { _, _, _ in releases += 1 }
        drive(node, [(.began, 10), (.ended, 30)], speed: CGPoint(x: 300, y: 0))
        XCTAssertEqual(pans, 2)
        XCTAssertEqual(releases, 0)
    }
    private func admits(_ node: NodeView, action: String?, velocity: CGPoint, translation: CGPoint = .zero) -> Bool {
        node.style["touch_action"] = action.map(BatchValue.string)
        let pan = Pan()
        pan.speed = velocity; pan.moved = translation
        node.layoutPanRecognizer = pan
        return node.gestureRecognizerShouldBegin(pan)
    }

    func testPanYLeavesVerticalContactsToNativeScrolling() {
        let (presenter, node) = host(["pan", "panrelease"])
        withExtendedLifetime(presenter) {
            for y: CGFloat in [-200, 200] {
                XCTAssertFalse(admits(node, action: "pan-y", velocity: CGPoint(x: 12, y: y)))
                XCTAssertTrue(admits(node, action: "pan-y", velocity: CGPoint(x: y, y: 12)))
            }
        }
    }

    func testPanXLeavesHorizontalContactsToNativeScrolling() {
        let (presenter, node) = host(["pan", "panrelease"])
        withExtendedLifetime(presenter) {
            for x: CGFloat in [-200, 200] {
                XCTAssertFalse(admits(node, action: "pan-x", velocity: CGPoint(x: x, y: 12)))
                XCTAssertTrue(admits(node, action: "pan-x", velocity: CGPoint(x: 12, y: x)))
            }
        }
    }

    func testDirectionalAndCombinedTouchActionsKeepTheirNativeDirections() {
        let (presenter, node) = host(["pan", "panrelease"])
        withExtendedLifetime(presenter) {
            for (action, native) in [("pan-left", CGPoint(x: 200, y: 0)), ("pan-right", CGPoint(x: -200, y: 0)),
                                     ("pan-up", CGPoint(x: 0, y: 200)), ("pan-down", CGPoint(x: 0, y: -200))] {
                XCTAssertFalse(admits(node, action: action, velocity: native), action)
                XCTAssertTrue(admits(node, action: action, velocity: CGPoint(x: -native.x, y: -native.y)), action)
            }
            XCTAssertFalse(admits(node, action: "pan-x pan-y", velocity: CGPoint(x: 200, y: 0)))
            XCTAssertFalse(admits(node, action: "pan-y pinch-zoom", velocity: CGPoint(x: 0, y: 200)))
            XCTAssertTrue(admits(node, action: "pan-y pinch-zoom", velocity: CGPoint(x: 200, y: 0)))
        }
    }

    func testDefaultAndNoNativePanRetainExistingLayoutPanAdmission() {
        let (presenter, node) = host(["pan", "panrelease"])
        withExtendedLifetime(presenter) {
            for action: String? in [nil, "auto", "manipulation", "none", "pinch-zoom"] {
                XCTAssertTrue(admits(node, action: action, velocity: CGPoint(x: 200, y: 0)))
                XCTAssertTrue(admits(node, action: action, velocity: CGPoint(x: 0, y: 200)))
            }
        }
    }

    func testZeroVelocityUsesTranslationAndUnknownDirectionDoesNotFail() {
        let (presenter, node) = host(["pan", "panrelease"])
        withExtendedLifetime(presenter) {
            XCTAssertFalse(admits(node, action: "pan-y", velocity: .zero, translation: CGPoint(x: 0, y: 20)))
            XCTAssertTrue(admits(node, action: "pan-y", velocity: .zero, translation: CGPoint(x: 20, y: 0)))
            XCTAssertTrue(admits(node, action: "pan-y", velocity: .zero))
        }
    }

}
#endif

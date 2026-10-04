#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// Under platform timing (LLP 1035.003 D5) a smooth correction is UIKit's
/// scroll animation: a list following its end after an appended row.
/// `clock settle` must not read the fixed point mid-flight, and an
/// interrupted one must not keep it waiting.
final class SettleScrollAnimationIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    private func collections(revision: Int, correction: Any) -> [String: Any] {
        ["op": "collections", "items": [["view": 1, "revision": revision, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": [["view": 10, "root": 10, "epoch": 1]], "correction": correction]]]
    }

    func testASmoothCorrectionIsNativeWorkUntilItEndsOrIsInterrupted() throws {
        let session = ExactApp.shared.makeSession(label: "settle-scroll-animation")
        defer { session.destroy() }
        let p = session.presenter
        // UIKit animates a scroll view only in a window on a screen.
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([collections(revision: 1, correction: NSNull()),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"], ["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        let agent = Agent(session: session)
        let list = try XCTUnwrap(p.views[1]), scroll = try XCTUnwrap(list.scroll)
        XCTAssertFalse(agent.nativeInFlight())
        // The list follows its end smoothly: settle waits while it runs.
        p.apply(wireBatch([collections(revision: 2, correction: ["scrollSequence": "0", "offset": 1700, "smooth": true])]))
        if ExactEnv.agentFreezes { throw XCTSkip("frozen agent timing sets a correction at once") }
        XCTAssertTrue(p.collections.animating.contains(1), "the host animates the correction")
        XCTAssertTrue(agent.nativeInFlight(), "a running end-follow keeps settle waiting")
        // Its end (UIKit's display link may not run in a unit test's window).
        scroll.setContentOffset(CGPoint(x: 0, y: 1700), animated: false)
        list.scrollViewDidEndScrollingAnimation(scroll)
        XCTAssertFalse(agent.nativeInFlight(), "landed: settle may read the fixed point")
        // A wheel before the next one's first tick interrupts it, as a drag does.
        let seq = String(try XCTUnwrap(p.collections.entries[1]?.cursor.sequence))
        p.apply(wireBatch([collections(revision: 3, correction: ["scrollSequence": seq, "offset": 1000, "smooth": true])]))
        XCTAssertTrue(agent.nativeInFlight())
        Agent.scroll(from: scroll, dx: 0, dy: -100)
        XCTAssertFalse(agent.nativeInFlight(), "an interrupted correction does not keep settle waiting")
    }
}
#endif

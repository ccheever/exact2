#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class StickyTransitionIOSTests: XCTestCase {
    func testAPinnedBoxKeepsItsPlacementWhileTranslatingAndScrolling() throws {
        let session = ExactApp.shared.makeSession(label: "sticky-transition")
        defer { session.destroy() }
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_STICKY_TRANSITION_PLAN"])
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: CGSize(width: 300, height: 500)).error)
        let p = session.presenter
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 500))
        defer { window.isHidden = true }
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport); window.makeKeyAndVisible()
        func node(_ name: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == name }) }
        let port = try node("port"), pinned = try node("pinned"), go = try node("move")
        let scroll = try XCTUnwrap(port.scrollView)
        scroll.contentOffset.y = 100
        p.stickies.scrolled(port.id)
        XCTAssertEqual(pinned.stickyOffset.y, 100, accuracy: 0.01)
        session.apply(session.runtime.startOnFrame(true, at: 0))
        session.apply(session.runtime.press(go.id, now: 100))
        session.apply(session.runtime.tick(now: 112, frame: 116))
        session.apply(session.runtime.tick(now: 612, frame: 616))
        XCTAssertEqual(pinned.translate.x, 50, accuracy: 0.01, "the presenter receives the visible translation")
        XCTAssertEqual(pinned.transform.tx, 50, accuracy: 0.01)
        XCTAssertEqual(pinned.transform.ty, 100, accuracy: 0.01)
        XCTAssertFalse((pinned.layer.animationKeys() ?? []).contains { $0.contains("transform.translation") })
        scroll.contentOffset.y = 140
        p.stickies.scrolled(port.id)
        XCTAssertEqual(pinned.transform.ty, 140, accuracy: 0.01, "placement can change during the transition")
        session.apply(session.runtime.tick(now: 1116, frame: 1120))
        XCTAssertEqual(pinned.transform.tx, 100, accuracy: 0.01)
        XCTAssertEqual(pinned.transform.ty, 140, accuracy: 0.01)
    }
}
#endif

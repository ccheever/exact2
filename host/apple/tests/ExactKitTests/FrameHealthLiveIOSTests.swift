#if os(iOS)
import UIKit
import XCTest
@testable import ExactCore
@testable import ExactKit

final class FrameHealthLiveIOSTests: XCTestCase {
    private static let plan: Result<Data, Error> = Result {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FRAME_HEALTH_PLAN"])
        return try Data(contentsOf: URL(fileURLWithPath: path))
    }

    func testPlatformAgentTicksFrameTasksAndUsesTheAsyncOwner() throws {
        guard !ExactEnv.agentFreezes else { throw XCTSkip("this regression runs under platform timing and ordinary timing") }
        guard let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first else { throw missing("scene", "live frame tasks need a scene") }
        let session = ExactApp.shared.makeSession(label: "live-frame-tasks")
        let window = UIWindow(windowScene: scene), controller = UIViewController()
        let view = ExactView(session: session)
        controller.view = view; window.rootViewController = controller; window.frame = scene.screen.bounds; window.isHidden = false
        defer { window.isHidden = true; session.destroy() }
        session.clock = nil
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: window.bounds.size).error)
        let frame = expectation(description: "a real display frame advances every(frame)")
        var heard = false
        session.runtime.observeBatch = { [weak session] _, _ in
            guard let session, !heard,
                  let object = try? JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"state\"}").utf8)) as? [String: Any],
                  ((object["slots"] as? [String: Any])?["ticks"] as? Int ?? 0) > 0 else { return }
            heard = true; frame.fulfill()
        }
        wait(for: [frame], timeout: 10)
        let state = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"state\"}").utf8)) as? [String: Any])
        XCTAssertGreaterThan((state["slots"] as? [String: Any])?["ticks"] as? Int ?? 0, 0)
        XCTAssertTrue(ExactSession.asyncFills)
        session.frames.tasks = false; session.frames.timerSoon = false; session.frames.run(false)
        session.runtime.observeBatch = nil
        let entered = expectation(description: "the existing owner is deliberately held")
        let gate = DispatchSemaphore(value: 0), lock = NSLock()
        var ownerReleased = false
        Owner.shared.post {
            entered.fulfill()
            _ = gate.wait(timeout: .now() + 10)
            lock.lock(); ownerReleased = true; lock.unlock()
        }
        wait(for: [entered], timeout: 5)
        defer { gate.signal() }
        session.frames.submitMotionTick(now: session.now(), frame: nil)
        lock.lock(); let released = ownerReleased; lock.unlock()
        XCTAssertFalse(released, "the motion submission returns while its owner is held")
        XCTAssertTrue(session.tickInFlight)
        gate.signal()
        session.drainFill()
        session.runtime.observeBatch = nil
    }

    func testScrollObserverIsPassiveScopedAndDoesNotOwnTheFrameClock() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("UIUpdateLink requires iOS 18") }
        let session = ExactApp.shared.makeSession(label: "passive-scroll")
        defer { session.destroy() }
        session.clock = nil
        let scroll = UIScrollView(frame: CGRect(x: 0, y: 0, width: 320, height: 480))
        guard let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first else { throw missing("scene", "passive observation requires a mounted view") }
        let window = UIWindow(windowScene: scene), controller = UIViewController()
        controller.view = scroll; window.rootViewController = controller; window.frame = scene.screen.bounds; window.isHidden = false
        defer { window.isHidden = true }
        let clock = FrameClock.shared.linksMade
        session.frameHealth.beginScroll(scroll, session: session)
        let observer = try XCTUnwrap(session.frameHealth.scrollObserver as? FrameHealthScrollObserver)
        let link = try XCTUnwrap(observer.link)
        XCTAssertFalse(link.requiresContinuousUpdates)
        XCTAssertFalse(link.wantsImmediatePresentation)
        XCTAssertFalse(link.wantsLowLatencyEventDispatch)
        XCTAssertEqual(link.preferredFrameRateRange, .default)
        XCTAssertEqual(FrameClock.shared.linksMade, clock)
        let observed = expectation(description: "native scroll updates reach the passive observer")
        var seen = false
        session.frameHealth.changed = {
            if !seen, observer.health.snapshot().callbacks > 0 { seen = true; observed.fulfill() }
        }
        defer { session.frameHealth.changed = nil }
        scroll.contentSize = CGSize(width: 320, height: 4000)
        scroll.setContentOffset(CGPoint(x: 0, y: 100), animated: true)
        wait(for: [observed], timeout: 10)
        XCTAssertEqual(FrameClock.shared.linksMade, clock, "observation did not add display demand")
        scroll.removeFromSuperview()
        XCTAssertFalse(session.frameHealth.nativeScrollActive, "detachment closes observation without waiting for another update")
        XCTAssertNil(observer.link)
        XCTAssertEqual(observer.health.snapshot().running, 0)
    }
}
#endif

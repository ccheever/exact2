#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The frame sampler's run-loop watch (LLP 1079, the overrun amendment)
/// on a real display link.
final class FrameSamplerIOSTests: XCTestCase {
    /// The turns are watched while the sampler's link runs, in the common
    /// modes (a scroll's tracking turns too), and no longer once it stops.
    func testTheTurnsAreWatchedWhileTheLinkRuns() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        sampler.activity()
        XCTAssertEqual(sampler.turnObservers.count, 2)
        for o in sampler.turnObservers { XCTAssertTrue(CFRunLoopContainsObserver(CFRunLoopGetMain(), o, .commonModes)) }
        // Woken first, asleep last: the whole turn, Core Animation's commit in it.
        let watch = sampler.turnObservers.map { (CFRunLoopObserverGetActivities($0), CFRunLoopObserverGetOrder($0)) }
        XCTAssertTrue(watch.contains { $0 == (CFRunLoopActivity.afterWaiting.rawValue, 0) })
        XCTAssertTrue(watch.contains { $0 == (CFRunLoopActivity.beforeWaiting.rawValue, CFIndex.max) })
        let watched = sampler.turnObservers
        sampler.stop()
        XCTAssertTrue(sampler.turnObservers.isEmpty)
        for o in watched { XCTAssertFalse(CFRunLoopContainsObserver(CFRunLoopGetMain(), o, .commonModes)) }
    }

    /// A turn the run loop really times: one that runs past the frame's
    /// target before it sleeps is an overrun, read by the observers alone.
    func testATurnThatRunsPastItsTargetIsSeen() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        sampler.activity()
        // Turns of the run loop with the link running; then a turn held
        // 60 ms, past any frame's target, until the sampler has seen one (a
        // loaded machine may deliver the link late: a few tries).
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.1))
        let before = (sampler.reply()["lifetime"] as? [String: Int])?["overruns"] ?? 0
        var after = before
        for _ in 0..<5 where after <= before {
            sampler.activity()
            DispatchQueue.main.async { let end = CACurrentMediaTime() + 0.06; while CACurrentMediaTime() < end {} }
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.15))
            after = (sampler.reply()["lifetime"] as? [String: Int])?["overruns"] ?? 0
        }
        XCTAssertGreaterThan(after, before)
        sampler.stop()
    }

    /// The agent's clock taking over stops the segment: nothing done under
    /// it is timed against the display.
    func testTheAgentsClockStopsTheWatch() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        sampler.activity()
        XCTAssertFalse(sampler.turnObservers.isEmpty)
        _ = Agent(session: session).clock(["take": true])
        XCTAssertNotNil(session.clock)
        XCTAssertTrue(sampler.turnObservers.isEmpty)
    }
}
#endif

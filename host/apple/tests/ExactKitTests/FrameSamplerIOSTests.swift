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
        let watched = sampler.turnObservers
        sampler.stop()
        XCTAssertTrue(sampler.turnObservers.isEmpty)
        for o in watched { XCTAssertFalse(CFRunLoopContainsObserver(CFRunLoopGetMain(), o, .commonModes)) }
    }
}
#endif

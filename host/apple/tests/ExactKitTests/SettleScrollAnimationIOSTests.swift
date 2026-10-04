#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// Under platform timing (LLP 1035.003 D5) a smooth correction is UIKit's
/// scroll animation: a list following its end after an appended row.
/// `clock settle` must not read the fixed point mid-flight.
final class SettleScrollAnimationIOSTests: XCTestCase {
    func testASmoothCorrectionInFlightIsNativeWork() throws {
        let session = ExactApp.shared.makeSession(label: "settle-scroll-animation")
        defer { session.destroy() }
        let p = session.presenter
        let agent = Agent(session: session)
        XCTAssertFalse(agent.nativeInFlight())
        p.collections.beginAnimation(4242, to: CGPoint(x: 0, y: 480))
        XCTAssertTrue(agent.nativeInFlight(), "a running end-follow keeps settle waiting")
        p.collections.stopAnimation(4242)
        XCTAssertFalse(agent.nativeInFlight())
    }
}
#endif

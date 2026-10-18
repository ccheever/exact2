import XCTest
@testable import ExactKit
#if os(iOS)
import UIKit
#endif

/// LLP 1125 D11: the wake lock is the process's, taken by the first
/// presenter whose batch says it is held and given back with the last; a
/// batch that says nothing leaves it, and a reset (a new host follows) lets go.
final class WakeLockTests: XCTestCase {
    private func batch(_ held: Bool?) -> Batch {
        var json: [String: Any] = ["ops": []]
        if let held { json["wakelock"] = held }
        return Batch.decode(try! JSONSerialization.data(withJSONObject: json))
    }

    func testTheLastHolderGivesItBack() throws {
        #if os(iOS)
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "UIKit's idle timer needs an app's window scene")
        }
        #endif
        XCTAssertEqual(batch(true).wakelock, true)
        XCTAssertNil(batch(nil).wakelock)
        let a = Presenter(), b = Presenter()
        XCTAssertFalse(WakeLock.held)
        a.apply(batch(true))
        XCTAssertTrue(WakeLock.held)
        #if os(iOS)
        XCTAssertTrue(UIApplication.shared.isIdleTimerDisabled)
        #endif
        b.apply(batch(true))
        a.apply(batch(false))
        XCTAssertTrue(WakeLock.held, "another session still holds it")
        b.apply(batch(nil))
        XCTAssertTrue(WakeLock.held, "a batch that says nothing leaves it")
        b.reset()
        XCTAssertFalse(WakeLock.held)
        #if os(iOS)
        // Under the agent the idle timer stays off for the whole drive.
        XCTAssertEqual(UIApplication.shared.isIdleTimerDisabled, ExactEnv.agentMode)
        #endif
    }
}

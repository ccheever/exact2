#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class HeldTransitionRetryIOSTests: XCTestCase {
    private final class Coordinator: NSObject, UIViewControllerTransitionCoordinator {
        var isAnimated = true
        var presentationStyle: UIModalPresentationStyle = .pageSheet
        var initiallyInteractive = false
        var isInterruptible = true
        var isInteractive = false
        var isCancelled = false
        var transitionDuration: TimeInterval = 0.5
        var percentComplete: CGFloat = 0
        var completionVelocity: CGFloat = 1
        var completionCurve: UIView.AnimationCurve = .easeInOut
        let containerView = UIView()
        var targetTransform = CGAffineTransform.identity
        func viewController(forKey key: UITransitionContextViewControllerKey) -> UIViewController? { nil }
        func view(forKey key: UITransitionContextViewKey) -> UIView? { nil }
        func animate(alongsideTransition animation: ((UIViewControllerTransitionCoordinatorContext) -> Void)?,
                     completion: ((UIViewControllerTransitionCoordinatorContext) -> Void)?) -> Bool { false }
        func animateAlongsideTransition(in view: UIView?, animation: ((UIViewControllerTransitionCoordinatorContext) -> Void)?,
                                        completion: ((UIViewControllerTransitionCoordinatorContext) -> Void)?) -> Bool { false }
        func notifyWhenInteractionChanges(_ handler: @escaping (UIViewControllerTransitionCoordinatorContext) -> Void) {}
        func notifyWhenInteractionEnds(_ handler: @escaping (UIViewControllerTransitionCoordinatorContext) -> Void) {}
    }
    private final class Tabs: UITabBarController {
        var running: Coordinator? = Coordinator()
        override var transitionCoordinator: UIViewControllerTransitionCoordinator? { running }
    }

    func testADeclinedCompletionWaitsForTheTimerInsteadOfSyncingOnTheNextTurn() {
        let p = Presenter(), tabs = Tabs()
        let navigation = p.navigation
        navigation.tabController = tabs
        XCTAssertTrue(navigation.realizeWaits)
        navigation.syncAfterTransition()
        let turn = expectation(description: "the queued main turn ran")
        DispatchQueue.main.async { turn.fulfill() }
        wait(for: [turn], timeout: 5)
        XCTAssertEqual(navigation.syncCalls, 0, "a running transition leaves the timer as the only retry")
        let deadline = Date().addingTimeInterval(5)
        while navigation.syncAfterTransitionArmed, Date() < deadline {
            RunLoop.main.run(mode: .default, before: deadline)
        }
        XCTAssertFalse(navigation.syncAfterTransitionArmed)
        XCTAssertEqual(navigation.syncCalls, 1, "one timer, one sync")
        tabs.running = nil
        navigation.syncAfterTransition()
        let end = Date().addingTimeInterval(5)
        while navigation.syncAfterTransitionArmed, Date() < end {
            RunLoop.main.run(mode: .default, before: end)
        }
        XCTAssertEqual(navigation.syncCalls, 2, "a later retry is armed independently")
    }
}
#endif

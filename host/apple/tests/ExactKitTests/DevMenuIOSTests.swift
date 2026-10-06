#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass
import XCTest
@testable import ExactKit

/// The dev menu's triggers and where it presents: four fingers tapped or
/// held, beside every recognizer the app's views carry but never with each
/// other, each journaled; and the sheet over whatever is presented, waiting
/// for a presentation or dismissal under way, never silently refused.
final class DevMenuIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    private func install() throws -> ExactSession {
        let session = ExactApp.shared.makeSession()
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        let root = UIViewController()
        window.rootViewController = root
        DevMenu.install(on: window, session: session, controller: root, planPath: nil)
        return session
    }
    private func journal(_ session: ExactSession, _ line: String) -> Int {
        session.agent(#"{"op":"logs","since":0}"#).components(separatedBy: line).count - 1
    }

    func testFourFingersTapTwiceTapAndHoldBesideTheAppsRecognizersNeverEachOther() throws {
        let session = try install()
        defer { session.destroy() }
        let ours = (window.gestureRecognizers ?? []).filter { $0.delegate === DevMenu.target }
        let taps = ours.compactMap { $0 as? UITapGestureRecognizer }
        XCTAssertEqual(taps.map(\.numberOfTouchesRequired).sorted(), [4, 4])
        XCTAssertEqual(taps.map(\.numberOfTapsRequired).sorted(), [1, 2])
        let press = try XCTUnwrap(ours.compactMap { $0 as? UILongPressGestureRecognizer }.first)
        XCTAssertEqual(press.numberOfTouchesRequired, 4)
        XCTAssertEqual(press.minimumPressDuration, 0.6, accuracy: 1e-9)
        XCTAssertGreaterThanOrEqual(press.allowableMovement, 40, "fingers drift on glass")
        XCTAssertTrue(press.cancelsTouchesInView, "the row under a hold is not pressed on release")
        for r in ours {
            XCTAssertFalse(r.delaysTouchesEnded)
            XCTAssertEqual(r.allowedTouchTypes, [NSNumber(value: UITouch.TouchType.direct.rawValue)])
            // A scroll view's pan or a row's swipe never excludes it ...
            XCTAssertTrue(DevMenu.target.gestureRecognizer(r, shouldRecognizeSimultaneouslyWith: UIPanGestureRecognizer()))
            // ... and a hold is never a tap too.
            for other in ours where other !== r {
                XCTAssertFalse(DevMenu.target.gestureRecognizer(r, shouldRecognizeSimultaneouslyWith: other))
            }
        }
    }

    /// A hold whose state the test sets (one with no view keeps `.possible`).
    private final class Held: UILongPressGestureRecognizer {
        var set: UIGestureRecognizer.State = .possible
        override var state: UIGestureRecognizer.State { get { set } set { set = newValue } }
    }

    func testEachTriggerJournalsAndAHoldTogglesOnce() throws {
        let session = try install()
        defer { session.destroy() }
        let hold = Held()
        hold.state = .began
        DevMenu.target.menuPress(hold)
        hold.state = .ended
        DevMenu.target.menuPress(hold)
        XCTAssertEqual(journal(session, "dev menu: four-finger press"), 1, "only as the hold begins")
        DevMenu.target.menuTap(UITapGestureRecognizer())
        XCTAssertEqual(journal(session, "dev menu: four-finger tap"), 1)
    }

    /// A controller whose presented one the test sets, as UIKit's is once a
    /// presentation lands, and which records what it is asked to present.
    private final class Presenting: UIViewController {
        var shown: UIViewController?
        var asked: [UIViewController] = []
        override var presentedViewController: UIViewController? { shown }
        override func present(_ vc: UIViewController, animated: Bool, completion: (() -> Void)? = nil) { asked.append(vc) }
    }
    private final class Leaving: UIViewController {
        override var isBeingDismissed: Bool { true }
    }

    func testTheSheetOpensOverWhateverIsPresentedOnceADismissalEnds() throws {
        let session = try install()
        defer { session.destroy() }
        let root = Presenting(), modal = Presenting()
        DevMenu.controller = root
        XCTAssertTrue(DevMenu.presenter === root, "nothing presented: the root")
        root.shown = modal
        XCTAssertTrue(DevMenu.presenter === modal, "over a modal route or a UIKit sheet, not refused for it")
        // A sheet over the modal that is leaving: wait for it, then present.
        let leaving = Leaving()
        modal.shown = leaving
        XCTAssertTrue(DevMenu.presenter === modal)
        let alert = UIAlertController(title: "t", message: nil, preferredStyle: .alert)
        DevMenu.present(alert)
        XCTAssertTrue(modal.asked.isEmpty, "not while a dismissal is under way")
        XCTAssertEqual(journal(session, "dev menu: waiting for a presentation to finish"), 1)
        modal.shown = nil
        let deadline = Date().addingTimeInterval(2)
        while modal.asked.isEmpty, Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(modal.asked.first === alert, "once it has gone")
        // A reload drops a presentation still waiting.
        modal.shown = leaving
        DevMenu.present(UIAlertController(title: "u", message: nil, preferredStyle: .alert))
        DevMenu.reload()
        modal.shown = nil
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        XCTAssertEqual(modal.asked.count, 1, "the dropped one never shows")
        // The menu toggled during a dismissal waits; toggled again, it never opens.
        modal.shown = leaving
        DevMenu.toggle()
        XCTAssertNotNil(DevMenu.waiting)
        DevMenu.toggle()
        XCTAssertNil(DevMenu.waiting)
        modal.shown = nil
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        XCTAssertEqual(modal.asked.count, 1, "the cancelled menu never shows")
        // Toggled once more, with nothing under way, it opens.
        DevMenu.toggle()
        XCTAssertTrue(modal.asked.last === DevMenu.sheet)
    }
}
#endif

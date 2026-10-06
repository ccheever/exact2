#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The dev menu's triggers and where it presents: four fingers tapped or
/// held, beside every recognizer the app's views carry, each journaled; and
/// the sheet over whatever is presented, never refused for it.
final class DevMenuIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    private func install() throws -> (ExactSession, UIViewController) {
        let session = ExactApp.shared.makeSession()
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        let root = UIViewController()
        window.rootViewController = root
        window.makeKeyAndVisible()
        DevMenu.install(on: window, session: session, controller: root, planPath: nil)
        return (session, root)
    }

    private func settle(_ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(2)
        while !done(), Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
    }

    func testFourFingersTapTwiceTapAndHoldBesideTheAppsRecognizers() throws {
        let (session, _) = try install()
        defer { session.destroy() }
        let ours = (window.gestureRecognizers ?? []).filter { $0.delegate === DevMenu.target }
        let taps = ours.compactMap { $0 as? UITapGestureRecognizer }
        XCTAssertEqual(taps.map(\.numberOfTouchesRequired).sorted(), [4, 4])
        XCTAssertEqual(taps.map(\.numberOfTapsRequired).sorted(), [1, 2])
        let press = try XCTUnwrap(ours.compactMap { $0 as? UILongPressGestureRecognizer }.first)
        XCTAssertEqual(press.numberOfTouchesRequired, 4)
        XCTAssertGreaterThanOrEqual(press.allowableMovement, 40, "fingers drift on glass")
        XCTAssertFalse(press.cancelsTouchesInView)
        for r in ours {
            XCTAssertFalse(r.delaysTouchesEnded)
            XCTAssertEqual(r.allowedTouchTypes, [NSNumber(value: UITouch.TouchType.direct.rawValue)])
            // A scroll view's pan or a row's swipe never excludes it.
            XCTAssertTrue(DevMenu.target.gestureRecognizer(r, shouldRecognizeSimultaneouslyWith: UIPanGestureRecognizer()))
        }
        // Firing journals a line a trace carries.
        DevMenu.target.menuTap(taps[0])
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("dev menu: four-finger tap"))
        settle { DevMenu.sheet?.presentingViewController != nil }
        DevMenu.sheet?.dismiss(animated: false)
        settle { DevMenu.sheet?.presentingViewController == nil }
    }

    /// A controller whose presented one the test sets, as UIKit's is once a
    /// presentation lands.
    private final class Presenting: UIViewController {
        var shown: UIViewController?
        override var presentedViewController: UIViewController? { shown }
    }

    func testTheSheetOpensOverWhateverIsPresented() throws {
        let (session, _) = try install()
        defer { session.destroy() }
        let root = Presenting(), modal = Presenting(), sheet = Presenting()
        DevMenu.controller = root
        XCTAssertTrue(DevMenu.presenter === root, "nothing presented: the root")
        root.shown = modal
        XCTAssertTrue(DevMenu.presenter === modal, "over a modal route or a UIKit sheet, not refused for it")
        modal.shown = sheet
        XCTAssertTrue(DevMenu.presenter === sheet, "over a sheet over a sheet")
    }
}
#endif

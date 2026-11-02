#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// While the scene is in the background UIKit can size the main navigation
/// container for the iPhone Duo's other display, and nothing lays it out
/// again on return. The host puts it back (`NavigationHost.fitOwner`) when
/// its scene returns to the foreground.
final class NavigationRefitIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func until(_ what: String, _ done: () -> Bool) {
        let deadline = Date(timeIntervalSinceNow: 5)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// The native fixture, booted in the test host's `scene`, and the
    /// controller Exact installed under its root.
    private func fixture(_ scene: UIWindowScene) throws -> (ExactSession, UIViewController) {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")
        let session = ExactApp.shared.makeSession(label: "navigation-refit")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let window = UIWindow(windowScene: scene)
        window.frame = scene.coordinateSpace.bounds
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the fixture's first route settles") { session.presenter.navigation.activeRoute != nil && !Agent(session: session).nativeInFlight() }
        return (session, try XCTUnwrap(session.presenter.navigation.primaryOwner))
    }

    /// A return to the foreground of the view's scene refits the container,
    /// and one of no scene of the view's does not. An activation refits it
    /// through the page's facts, whose batch lays it out (`PageFacts`).
    func testTheContainerIsRefitWhenItsSceneReturnsToTheForeground() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "the container is refit for the scene its view is in") }
        let (session, owner) = try fixture(scene)
        let fitted = session.presenter.navigation.ownerFrame(owner, in: try XCTUnwrap(owner.view.superview))
        let off = fitted.insetBy(dx: 0, dy: 80)
        owner.view.frame = off
        NotificationCenter.default.post(name: UIScene.willEnterForegroundNotification, object: nil)
        XCTAssertEqual(owner.view.frame, off, "a return to the foreground of no scene of the view's")
        for name in [UIScene.willEnterForegroundNotification, UIScene.didActivateNotification] {
            owner.view.frame = off
            NotificationCenter.default.post(name: name, object: scene)
            XCTAssertEqual(owner.view.frame, fitted, "\(name.rawValue) for the view's scene")
        }
    }
}
#endif

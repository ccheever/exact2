#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class AgentBarPointerIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?
    private var animationsWereEnabled = true

    override func setUp() {
        super.setUp()
        animationsWereEnabled = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
    }

    override func tearDown() {
        session?.destroy(); session = nil
        window?.isHidden = true; window = nil
        UIView.setAnimationsEnabled(animationsWereEnabled)
        super.tearDown()
    }

    private func until(_ ready: () -> Bool) {
        let deadline = Date(timeIntervalSinceNow: 5)
        while !ready(), Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02))
        }
        XCTAssertTrue(ready(), "navigation reached its observable ready state")
    }

    private func fixture(_ key: String = "EXACT_FIXTURE_PLAN") throws -> ExactSession {
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment[key])))
        let session = ExactApp.shared.makeSession(label: "bar-pointer")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until {
            session.presenter.navigation.primaryNavigation?.topViewController is RouteController &&
                !Agent(session: session).nativeInFlight()
        }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }

    func testABarSegmentRechecksARIAAfterPointerHandlers() throws {
        let session = try fixture(), p = session.presenter
        let tab = try node(session, "segment-missed"), root = try node(session, "navigation")
        root.handlers.formUnion(["pointerdown", "pointerup"])
        var presses: [UInt32] = [], downs = 0
        p.onPointer = { _, kind, _ in
            if kind == .down {
                downs += 1
                p.apply(wireBatch([["op": "props", "id": tab.id, "set": ["accessibilityDisabled": "true"]]]))
            }
        }
        p.onPress = { presses.append($0) }
        XCTAssertNotNil(p.navigation.barSegment(tab))
        _ = Agent(session: session).tap(["id": Int(tab.id)])
        XCTAssertEqual(downs, 1)
        XCTAssertTrue(tab.ariaUnavailable)
        XCTAssertTrue(presses.isEmpty, "the agent applies the bar segment's eligibility after its observers")
    }

    func testARIAOnABarItemRemainsAnAccessibilityStateAfterPointers() throws {
        let session = try fixture(), p = session.presenter
        let item = try node(session, "compose-home"), root = try node(session, "navigation")
        root.handlers.formUnion(["pointerdown", "pointerup"])
        var presses: [UInt32] = [], downs = 0
        p.onPointer = { _, kind, _ in
            if kind == .down {
                downs += 1
                p.apply(wireBatch([["op": "props", "id": item.id, "set": ["accessibilityDisabled": "true"]]]))
            }
        }
        p.onPress = { presses.append($0) }
        let reply = Agent(session: session).tap(["id": Int(item.id)])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(downs, 1)
        XCTAssertTrue(item.ariaUnavailable)
        XCTAssertEqual(presses, [item.id], "ARIA state leaves the native item pressable")
    }
}
#endif

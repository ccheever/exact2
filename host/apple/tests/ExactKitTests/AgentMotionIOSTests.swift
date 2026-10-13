#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// A picture at rest (LLP 1116.003 D8): a screenshot waits out UIKit's own
/// motion — an animation, a view moved frame by frame, a segment's thumb —
/// and never a repeating one (a caret's blink, a spinner).
final class AgentMotionIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func mounted(_ ops: [[String: Any]]) -> Agent {
        let session = ExactApp.shared.makeSession(label: "agent-motion")
        self.session = session
        let p = session.presenter
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch(ops))
        p.viewport.layoutIfNeeded()
        return Agent(session: session)
    }

    func testAScreenshotWaitsOutUIKitsMotionButNotARepeatingOne() throws {
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
        ])
        let window = try XCTUnwrap(self.window)
        let moving = UIView(frame: CGRect(x: 0, y: 0, width: 40, height: 40))
        window.addSubview(moving)
        UIView.animate(withDuration: 0.3) { moving.frame.origin.x = 200 }
        XCTAssertTrue(Agent.wallMotion(under: window.layer), "UIKit's own motion under way")
        let started = Date()
        XCTAssertTrue(agent.settleMotion(), "it ends within the bound")
        XCTAssertGreaterThan(Date().timeIntervalSince(started), 0.1, "the wait was the animation's")
        XCTAssertFalse(Agent.wallMotion(under: window.layer))
        // Motion with no animation to see, a frame at a time (iOS 26's segment thumb): waited for as well.
        var steps = 0
        let timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60, repeats: true) { t in
            steps += 1; moving.frame.origin.x = CGFloat(200 - 10 * steps)
            if steps == 18 { t.invalidate() }
        }
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02))
        XCTAssertTrue(agent.settleMotion(), "it comes to rest within the bound")
        XCTAssertEqual(moving.frame.origin.x, 20, "at its end, not part way")
        timer.invalidate()
        // A caret's blink or a spinner repeats for ever: never waited for.
        let blink = CABasicAnimation(keyPath: "opacity")
        blink.fromValue = 1; blink.toValue = 0; blink.duration = 0.5; blink.repeatCount = .infinity
        moving.layer.add(blink, forKey: "blink")
        XCTAssertFalse(Agent.wallMotion(under: window.layer), "a repeating animation is not motion to wait out")
        let blinking = Date()
        XCTAssertTrue(agent.settleMotion())
        XCTAssertLessThan(Date().timeIntervalSince(blinking), 1, "and a picture does not wait for it")
        // UIKit's own controls are watched by their pixels too: iOS 26 moves a segment's thumb in the render server.
        XCTAssertNil(Agent.ownControls(in: window))
        let segments = UISegmentedControl(items: ["A", "B"])
        segments.frame = CGRect(x: 20, y: 300, width: 200, height: 32)
        window.addSubview(segments)
        XCTAssertEqual(Agent.ownControls(in: window), CGRect(x: 16, y: 296, width: 208, height: 40))
        segments.selectedSegmentIndex = 1
        XCTAssertTrue(agent.settleMotion(), "the thumb comes to rest within the bound")
        segments.removeFromSuperview()
        moving.removeFromSuperview()
    }
}
#endif

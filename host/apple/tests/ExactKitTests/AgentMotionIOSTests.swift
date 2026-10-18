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

    /// A native module's motion on the wall clock (the euxy port's boot
    /// splash, a view on the window faded with `UIView.animate`): `clock`
    /// waits it out and its completion runs, and a tap waits for the
    /// overlay to go.
    func testTheAppsOwnMotionOnTheWallEndsUnderTheAgentsClock() throws {
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
        ])
        let window = try XCTUnwrap(self.window)
        let node = try XCTUnwrap(agent.session.presenter.views[1])
        let overlay = UIView(frame: window.bounds)
        window.addSubview(overlay)
        XCTAssertTrue(Agent.appOverlay(try XCTUnwrap(window.hitTest(CGPoint(x: 200, y: 400), with: nil))), "the app's view over the screen")
        XCTAssertFalse(Agent.appOverlay(node), "a node is the session's")
        var finished = false
        UIView.animate(withDuration: 0.3) { overlay.alpha = 0 } completion: { _ in overlay.removeFromSuperview(); finished = true }
        XCTAssertTrue(agent.awaitWallMotion(until: Date(timeIntervalSinceNow: 1.5)), "it ends within the bound")
        XCTAssertTrue(finished, "and its completion has run")
        XCTAssertNil(overlay.superview)
        // One longer than the bound is still running when it passes: `clock settle` says so.
        let slow = UIView(frame: CGRect(x: 0, y: 0, width: 40, height: 40))
        window.addSubview(slow)
        UIView.animate(withDuration: 2) { slow.alpha = 0 }
        XCTAssertFalse(agent.awaitWallMotion(until: Date(timeIntervalSinceNow: 0.2)), "still running at the bound")
        slow.layer.removeAllAnimations()
        slow.removeFromSuperview()
        // A moving overlay over the tap's point: the tap waits for it to go.
        let middle = CGPoint(x: 200, y: 400)
        let cover = UIView(frame: window.bounds)
        window.addSubview(cover)
        UIView.animate(withDuration: 0.3) { cover.backgroundColor = .red }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { cover.removeFromSuperview() }
        var tapped = Date()
        XCTAssertTrue(agent.awaitAppOverlay(at: middle, in: window))
        XCTAssertNil(cover.superview)
        XCTAssertLessThan(Date().timeIntervalSince(tapped), 1, "no longer than the overlay stayed")
        // One fading at another point delays nothing; one holding still over
        // the point delays the quiet spell alone (the tap then refuses it).
        let corner = UIView(frame: CGRect(x: 0, y: 0, width: 60, height: 60))
        window.addSubview(corner)
        UIView.animate(withDuration: 1) { corner.alpha = 0 }
        XCTAssertFalse(agent.awaitAppOverlay(at: middle, in: window), "not over the point")
        corner.layer.removeAllAnimations()
        corner.removeFromSuperview()
        let still = UIView(frame: window.bounds)
        window.addSubview(still)
        tapped = Date()
        XCTAssertFalse(agent.awaitAppOverlay(at: middle, in: window), "nothing moves there")
        XCTAssertLessThan(Date().timeIntervalSince(tapped), 0.8, "a still overlay that takes the touch: only the quiet spell")
        still.removeFromSuperview()
        // A full-screen container that takes no touch, a toast moving in its corner: the point is still.
        let container = UIView(frame: window.bounds), toast = UIView(frame: CGRect(x: 0, y: 0, width: 80, height: 40))
        container.isUserInteractionEnabled = false
        container.addSubview(toast)
        window.addSubview(container)
        UIView.animate(withDuration: 1) { toast.alpha = 0.2 }
        tapped = Date()
        agent.awaitAppOverlay(at: middle, in: window)
        XCTAssertLessThan(Date().timeIntervalSince(tapped), 0.3, "a toast elsewhere delays nothing")
        toast.layer.removeAllAnimations()
        container.removeFromSuperview()
        // A completion that starts a second fade: `clock` waits for that one too.
        let first = UIView(frame: CGRect(x: 0, y: 0, width: 40, height: 40))
        window.addSubview(first)
        var second = false
        UIView.animate(withDuration: 0.2) { first.alpha = 0.5 } completion: { _ in
            UIView.animate(withDuration: 0.2) { first.alpha = 0 } completion: { _ in second = true }
        }
        XCTAssertTrue(agent.awaitWallMotion(until: Date(timeIntervalSinceNow: 1.5)))
        XCTAssertTrue(second, "the fade the first one's completion started has ended too")
        first.removeFromSuperview()
    }

    /// Contract motion held at speed 0 (a lowered `@keyframes`, filled both
    /// ways and kept) is the agent clock's to seek: never motion in flight.
    func testContractMotionHeldAtSpeedZeroIsNotInFlight() throws {
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
        ])
        let window = try XCTUnwrap(self.window)
        let node = try XCTUnwrap(agent.session.presenter.views[1])
        let held = CABasicAnimation(keyPath: "transform.translation.x")
        held.fromValue = 0; held.toValue = 10; held.duration = 0.2
        held.speed = 0; held.fillMode = .both; held.isRemovedOnCompletion = false
        node.layer.add(held, forKey: "held")
        XCTAssertFalse(Agent.wallMotion(under: window.layer))
        XCTAssertFalse(agent.nativeGeometryInFlight(), "`clock settle` does not wait on it")
    }

    /// Native motion that is over, or never ends, is not in flight: an
    /// animation kept on its layer past its end (`isRemovedOnCompletion`
    /// false, no begin time of its own) and a geometry animation repeating
    /// for ever make neither `clock` nor `clock settle` wait.
    func testKeptAndEndlessAnimationsAreNotInFlight() throws {
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
        ])
        let window = try XCTUnwrap(self.window)
        let node = try XCTUnwrap(agent.session.presenter.views[1])
        let kept = CABasicAnimation(keyPath: "position.x")
        kept.fromValue = 0; kept.toValue = 10; kept.duration = 0.1
        kept.fillMode = .forwards; kept.isRemovedOnCompletion = false
        node.layer.add(kept, forKey: "kept")
        XCTAssertTrue(agent.nativeGeometryInFlight(), "running")
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.3))
        XCTAssertFalse(Agent.wallMotion(under: window.layer), "kept past its end")
        XCTAssertFalse(agent.nativeGeometryInFlight())
        node.layer.removeAnimation(forKey: "kept")
        let endless = CABasicAnimation(keyPath: "transform.rotation")
        endless.toValue = Double.pi; endless.duration = 1; endless.repeatCount = .infinity
        node.layer.add(endless, forKey: "endless")
        XCTAssertFalse(agent.nativeGeometryInFlight(), "a spinner is not a transition")
        node.layer.removeAnimation(forKey: "endless")
    }

    /// Animations attached together end together: one pass looks at every
    /// one, so two with no begin time yet are first seen at once, not one
    /// per call, which timed them one after the other and held `clock
    /// settle` past their end.
    func testAnimationsAttachedTogetherAreTimedTogether() {
        let root = CALayer(), layers = [CALayer(), CALayer()]
        for (i, layer) in layers.enumerated() {
            root.addSublayer(layer)
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 1; fade.toValue = 0; fade.duration = 0.5
            fade.fillMode = .forwards; fade.isRemovedOnCompletion = false
            layer.add(fade, forKey: "fade\(i)")
        }
        XCTAssertTrue(Agent.wallMotion(under: root), "both running")
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.3))
        XCTAssertTrue(Agent.wallMotion(under: root))
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.35))
        XCTAssertFalse(Agent.wallMotion(under: root), "both over, the second timed from when the first was")
    }
}
#endif

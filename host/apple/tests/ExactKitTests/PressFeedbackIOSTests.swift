#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// Press feedback (LLP 1061 D2): the host eases a pressed node to its
/// `press_scale` and back, folded into the transform the motion engine
/// writes, so neither overwrites the other. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class PressFeedbackIOSTests: XCTestCase {
    private var window: UIWindow!

    private func fixture(style: [String: Any] = ["press_scale": 0.97]) throws -> (Presenter, NodeView) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"], "style": style],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 50.0, "y": 50.0, "w": 200.0, "h": 100.0],
        ]))
        return (p, try XCTUnwrap(p.views[1]))
    }

    func testTheEaseIsCSSCubicBezier() {
        // cubic-bezier(.16, 1, .3, 1), sampled by bisection off-device.
        for (x, y) in [(0.0, 0.0), (0.1, 0.49439), (0.25, 0.82562), (0.5, 0.97178), (0.75, 0.99768), (1.0, 1.0)] {
            XCTAssertEqual(PressFeedback.ease(x), y, accuracy: 1e-4, "x = \(x)")
        }
    }

    func testPressingEasesToThePressScaleAndReleasingEasesBack() throws {
        let (_, v) = try fixture()
        v.pressed = true
        XCTAssertEqual(v.press.to, 0.97)
        let start = v.press.start
        XCTAssertEqual(v.press.factor(at: start), 1, accuracy: 1e-9, "no jump at touch-down")
        XCTAssertEqual(v.press.factor(at: start + PressFeedback.duration), 0.97, accuracy: 1e-9)
        // Released a third of the way in: the way back starts where it was.
        let mid = start + PressFeedback.duration / 3
        let there = v.press.factor(at: mid)
        v.press.aim(1, at: mid)
        XCTAssertEqual(v.press.from, there, accuracy: 1e-12)
        XCTAssertEqual(v.press.factor(at: mid), there, accuracy: 1e-12)
        v.pressed = false
        XCTAssertEqual(v.press.to, 1)
    }

    func testReducedMotionKeepsThePress() throws {
        let previous = DisplayPreferences.agent
        defer { DisplayPreferences.agent = previous }
        DisplayPreferences.agent = (reducedMotion: true, reducedTransparency: false)
        let (_, v) = try fixture()
        v.pressed = true
        XCTAssertEqual(v.press.to, 0.97)
        v.press.start -= 1
        v.applyTransform()
        XCTAssertEqual(v.transform.a, 0.97, accuracy: 1e-9)
        v.pressed = false
        XCTAssertEqual(v.press.to, 1)
    }

    func testANodeWithoutTheRowGivesNoFeedback() throws {
        let (_, v) = try fixture(style: [:])
        v.pressed = true
        XCTAssertTrue(v.press.idle)
        XCTAssertTrue(v.transform.isIdentity)
    }

    func testThePressFoldsIntoTheEnginesScaleAndSurvivesItsWrites() throws {
        let (p, v) = try fixture()
        // Held at the pressed scale.
        v.press = PressFeedback(from: 0.97, to: 0.97, start: 0)
        p.apply(wireBatch([["op": "present", "id": 1, "property": "scale", "x": 2.0]]))
        XCTAssertEqual(v.transform.a, 2 * 0.97, accuracy: 1e-9, "the engine's write keeps the press")
        XCTAssertEqual(v.transform.d, 2 * 0.97, accuracy: 1e-9)
        // A relayout sets the frame untransformed, then the presentation back.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 60.0, "y": 50.0, "w": 200.0, "h": 100.0]]))
        XCTAssertEqual(v.transform.a, 2 * 0.97, accuracy: 1e-9)
        XCTAssertEqual(v.center, CGPoint(x: 160, y: 100), "about the centre")
        // Idle again: exactly the engine's value.
        v.press = PressFeedback()
        v.applyTransform()
        XCTAssertEqual(v.transform, CGAffineTransform(scaleX: 2, y: 2))
    }

    /// D2: a quick tap in a scroll view arrives late, its down and up in one
    /// turn (`delaysContentTouches`). The release waits until the press has
    /// eased in, so the tap shows, as a UIButton's highlight does.
    func testAReleaseBeforeThePressWasSeenWaitsForIt() throws {
        let (_, v) = try fixture()
        v.pressed = true
        v.pressed = false
        XCTAssertTrue(v.press.releaseHeld)
        XCTAssertEqual(v.press.to, 0.97, "still pressing in")
        RunLoop.main.run(until: Date(timeIntervalSinceNow: PressFeedback.duration + 0.05))
        XCTAssertFalse(v.press.releaseHeld)
        XCTAssertEqual(v.press.to, 1, "then released")
        RunLoop.main.run(until: Date(timeIntervalSinceNow: PressFeedback.duration + 0.1))
        XCTAssertTrue(v.press.idle)
        XCTAssertTrue(v.transform.isIdentity)
        // A press already on screen releases at once.
        v.pressed = true
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        v.pressed = false
        XCTAssertFalse(v.press.releaseHeld)
        XCTAssertEqual(v.press.to, 1)
    }

    /// D6: every transform turns about `transform-origin`.
    func testTheTransformTurnsAboutTheTransformOrigin() throws {
        let (p, v) = try fixture(style: ["transform_origin": [["pct": 0], ["pct": 0]]])
        p.apply(wireBatch([["op": "present", "id": 1, "property": "scale", "x": 0.5]]))
        XCTAssertEqual(v.frame, CGRect(x: 50, y: 50, width: 100, height: 50), "the top-left corner stays put")
        p.apply(wireBatch([["op": "present", "id": 1, "property": "rotate", "x": 90.0], ["op": "present", "id": 1, "property": "scale", "x": 1.0]]))
        XCTAssertEqual(v.frame.minX, -50, accuracy: 1e-9, "a quarter turn about it swings the box to its left")
        XCTAssertEqual(v.frame.minY, 50, accuracy: 1e-9)
        p.apply(wireBatch([["op": "style", "id": 1, "style": [:] as [String: Any]]]))
        XCTAssertEqual(v.frame.midX, 150, accuracy: 1e-9, "unset again: about the centre")
    }
}
#endif

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

    private func fixture(style: [String: Any] = ["press_scale": 0.97], handlers: [String] = ["press"]) throws -> (Presenter, NodeView) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "button", "handlers": handlers, "style": style],
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

    /// LLP 1115: a node that declares no feedback gets the platform's, a dim
    /// (UIKit's custom button), never a scale; the dim is an additive
    /// animation, so the model's alpha stays the engine's.
    func testANodeWithoutTheRowDimsAsUIKitDoes() throws {
        let (_, v) = try fixture(style: [:])
        v.pressed = true
        XCTAssertTrue(v.press.idle, "no scale")
        XCTAssertTrue(v.transform.isIdentity)
        XCTAssertEqual(v.alpha, 1, "the model is untouched")
        let hold = try XCTUnwrap(v.layer.animation(forKey: "pressDim") as? CABasicAnimation)
        XCTAssertTrue(hold.isAdditive)
        XCTAssertEqual(hold.keyPath, "opacity")
        XCTAssertEqual(try XCTUnwrap(hold.fromValue as? Float), NodeView.dimmedOpacity - 1, accuracy: 1e-6)
        XCTAssertEqual(try XCTUnwrap(hold.toValue as? Float), NodeView.dimmedOpacity - 1, accuracy: 1e-6)
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.03))
        XCTAssertEqual(try XCTUnwrap(v.layer.presentation()).opacity, NodeView.dimmedOpacity, accuracy: 0.01, "dimmed on screen at once")
        // Released: it fades back, after the shortest flash.
        v.pressed = false
        let back = try XCTUnwrap(v.layer.animation(forKey: "pressDim") as? CABasicAnimation)
        XCTAssertEqual(back.duration, NodeView.dimFade)
        XCTAssertEqual(try XCTUnwrap(back.toValue as? Float), 0)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: NodeView.dimHold + NodeView.dimFade + 0.1))
        CATransaction.flush()
        XCTAssertEqual(try XCTUnwrap(v.layer.presentation()).opacity, 1, accuracy: 1e-6, "faded back")
    }

    /// A quick tap in a scroll view arrives with its down and up in one turn
    /// (`delaysContentTouches`): the dim still flashes.
    func testAQuickTapStillFlashesTheDim() throws {
        let (_, v) = try fixture(style: [:])
        v.pressed = true
        v.pressed = false
        let back = try XCTUnwrap(v.layer.animation(forKey: "pressDim") as? CABasicAnimation)
        XCTAssertEqual(back.fillMode, .backwards, "held dimmed until it begins")
        XCTAssertGreaterThan(back.beginTime, v.layer.convertTime(CACurrentMediaTime(), from: nil) + NodeView.dimHold / 2)
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.03))
        XCTAssertEqual(try XCTUnwrap(v.layer.presentation()).opacity, NodeView.dimmedOpacity, accuracy: 0.01)
    }

    /// The finger leaving the box lifts the dim, re-entering dims again; a
    /// cancel (a pan the scroll view takes) lifts it.
    func testTheDimFollowsTheFingerAndACancel() throws {
        let (_, v) = try fixture(style: [:])
        v.pressed = true
        v.pressFollows(inside: false)
        XCTAssertNil(v.press.dimmedAt)
        v.pressFollows(inside: true)
        XCTAssertNotNil(v.press.dimmedAt)
        v.touchesCancelled([], with: nil)
        XCTAssertFalse(v.pressed)
        XCTAssertNil(v.press.dimmedAt)
    }

    /// Author > platform: any written press scale (1 included) or pointer
    /// handlers of its own are the author's feedback; a scrim over the window
    /// shows none; a native button is UIKit's own.
    func testTheAuthorsFeedbackAndScrimsGetNoDim() throws {
        let (_, scaled) = try fixture(style: ["press_scale": 1])
        scaled.pressed = true
        XCTAssertNil(scaled.layer.animation(forKey: "pressDim"))
        XCTAssertFalse(scaled.platformPressDims)
        let (_, plain) = try fixture(style: [:])
        XCTAssertTrue(plain.platformPressDims)
        let (_, own) = try fixture(style: [:], handlers: ["press", "pointerdown"])
        XCTAssertFalse(own.platformPressDims)
        let (q, scrim) = try fixture(style: [:])
        q.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0]]))
        XCTAssertFalse(scrim.platformPressDims)
        scrim.pressed = true
        XCTAssertNil(scrim.layer.animation(forKey: "pressDim"))
    }

    /// A view taken for another node drops a dim still showing.
    func testRebindingDropsTheDim() throws {
        let (_, v) = try fixture(style: [:])
        v.pressed = true
        v.stopPressEase()
        XCTAssertNil(v.layer.animation(forKey: "pressDim"))
        XCTAssertNil(v.press.dimmedAt)
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

    /// The press is Core Animation's (D2): the model takes the pressed scale
    /// at once and an additive animation eases the difference on the render
    /// server, so no frame of it runs on the main thread. At touch-down the
    /// screen still shows the unpressed box — about the origin, under a
    /// rotation and a translation.
    func testThePressEasesOnTheRenderServerFromWhatShows() throws {
        let (p, v) = try fixture(style: ["press_scale": 0.5, "transform_origin": [["pct": 0], ["pct": 0]]])
        p.apply(wireBatch([["op": "present", "id": 1, "property": "rotate", "x": 30.0],
                           ["op": "present", "id": 1, "property": "translate", "x": 20.0, "y": 10.0]]))
        let unpressed = v.transform
        v.pressed = true
        XCTAssertEqual(v.transform.a, unpressed.a * 0.5, accuracy: 1e-9, "the model is the pressed box at once")
        let ease = try XCTUnwrap(v.layer.animation(forKey: "press") as? CABasicAnimation)
        XCTAssertTrue(ease.isAdditive)
        XCTAssertEqual(ease.duration, PressFeedback.duration)
        let from = try XCTUnwrap(ease.fromValue as? CATransform3D)
        let shown = CATransform3DGetAffineTransform(CATransform3DConcat(from, CATransform3DMakeAffineTransform(v.transform)))
        for (x, y) in [(shown.a, unpressed.a), (shown.b, unpressed.b), (shown.c, unpressed.c), (shown.d, unpressed.d), (shown.tx, unpressed.tx), (shown.ty, unpressed.ty)] {
            XCTAssertEqual(x, y, accuracy: 1e-9, "touch-down shows the unpressed box")
        }
        // Whatever instant Core Animation presents, it is the box at some
        // factor about the origin — composed before the model, not after it.
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.03))
        let presented = try XCTUnwrap(v.layer.presentation()).affineTransform()
        let f = hypot(presented.a, presented.b) / hypot(unpressed.a, unpressed.b)
        let o = v.transformOriginPoint, d = CGPoint(x: o.x - v.bounds.midX, y: o.y - v.bounds.midY)
        let atF = CGAffineTransform(translationX: d.x, y: d.y).scaledBy(x: f, y: f).translatedBy(x: -d.x, y: -d.y).concatenating(unpressed)
        XCTAssertTrue((0.5...0.99).contains(f), "mid-ease: \(f)")
        XCTAssertEqual(presented.tx, atF.tx, accuracy: 1e-3, "Core Animation composes it as the model's, not after it")
        XCTAssertEqual(presented.ty, atF.ty, accuracy: 1e-3)
        // A release mid-ease starts from what shows: no jump.
        let there = v.pressFactor
        v.pressed = false
        XCTAssertEqual(v.transform, unpressed, "released: the model is the engine's alone")
        let back = try XCTUnwrap(v.layer.animation(forKey: "press") as? CABasicAnimation)
        let backFrom = CATransform3DGetAffineTransform(try XCTUnwrap(back.fromValue as? CATransform3D))
        XCTAssertEqual(backFrom.a, there, accuracy: 1e-3, "the factor that showed at release")
    }

    /// The render server's ease follows the box, and only the box: one that
    /// resizes mid-press eases about its new origin with its progress kept,
    /// and a view taken for another node carries none of it.
    func testThePressEaseFollowsTheBoxAndNotItsNextNode() throws {
        let (p, v) = try fixture(style: ["press_scale": 0.5, "transform_origin": [["pct": 0], ["pct": 0]]])
        v.pressed = true
        let first = try XCTUnwrap(v.layer.animation(forKey: "press") as? CABasicAnimation)
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 50.0, "y": 50.0, "w": 400.0, "h": 100.0]]))
        let resized = try XCTUnwrap(v.layer.animation(forKey: "press") as? CABasicAnimation)
        XCTAssertFalse(resized === first, "rebuilt about the new origin")
        XCTAssertEqual(resized.beginTime, first.beginTime, accuracy: 1e-9, "its progress kept")
        let from = CATransform3DGetAffineTransform(try XCTUnwrap(resized.fromValue as? CATransform3D))
        // A factor of 2 about the top left, 200 points left of the centre: it moves the centre 200 right.
        XCTAssertEqual(from.a, 2, accuracy: 1e-9)
        XCTAssertEqual(from.tx, 200, accuracy: 1e-9)
        v.rebind(99)
        XCTAssertNil(v.layer.animation(forKey: "press"), "the next node shows none of it")
        XCTAssertNil(v.press.easedAbout)
    }

    /// A release held for an unseen press goes for that press only: a newer
    /// press on the view (the next node's, after a rebind) waits for its own.
    func testAHeldReleaseIsForItsOwnPress() throws {
        let (_, v) = try fixture(style: ["press_scale": 0.5])
        v.pressed = true
        v.pressed = false
        XCTAssertTrue(v.press.releaseHeld)
        var newer = PressFeedback(from: 1, to: 0.5, start: CACurrentMediaTime() + 0.1)
        newer.releaseHeld = true
        v.press = newer
        RunLoop.main.run(until: Date(timeIntervalSinceNow: PressFeedback.duration + 0.04))
        XCTAssertEqual(v.press.to, 0.5, "the old release did not take the newer press")
        XCTAssertTrue(v.press.releaseHeld)
    }

    /// A box turned out of the screen's plane scales inside its rotation:
    /// the ease's scale about the origin, composed before the model, is the
    /// model at the shown factor, at every factor.
    func testAPressInSpaceEasesInsideItsRotation() throws {
        let (p, v) = try fixture(style: ["press_scale": 0.5, "rotate_axis": [1.0, 0.0, 0.0], "transform_origin": [["pct": 0], ["pct": 0]]])
        p.apply(wireBatch([["op": "present", "id": 1, "property": "rotate", "x": 60.0]]))
        let unpressed = v.layer.transform
        v.pressed = true
        let ease = try XCTUnwrap(v.layer.animation(forKey: "press") as? CABasicAnimation)
        let from = try XCTUnwrap(ease.fromValue as? CATransform3D)
        let shown = CATransform3DConcat(from, v.layer.transform)
        let pressed = v.layer.transform
        func entries(_ m: CATransform3D) -> [CGFloat] { [m.m11, m.m12, m.m13, m.m14, m.m21, m.m22, m.m23, m.m24, m.m31, m.m32, m.m33, m.m34, m.m41, m.m42, m.m43, m.m44] }
        for (a, b) in zip(entries(shown), entries(unpressed)) { XCTAssertEqual(a, b, accuracy: 1e-9, "touch-down shows the unpressed box") }
        // Part way: the scale at 0.75 of the target's, before the pressed
        // model, is the model pressed to 0.75.
        let k: CGFloat = 0.75 / 0.5, o = v.transformOriginPoint, d = CGPoint(x: o.x - v.bounds.midX, y: o.y - v.bounds.midY)
        let mid = CATransform3DConcat(CATransform3DMakeAffineTransform(CGAffineTransform(translationX: d.x, y: d.y).scaledBy(x: k, y: k).translatedBy(x: -d.x, y: -d.y)), pressed)
        let model = CATransform3DConcat(try XCTUnwrap(v.spaceTransform(origin: d, scale: 0.75)), CATransform3DIdentity)
        for (a, b) in zip(entries(mid), entries(model)) { XCTAssertEqual(a, b, accuracy: 1e-9, "inside the rotation part way too") }
    }

    /// A view that flies shows the flight alone: a press, or a held
    /// release, puts no ease on it.
    func testAFlyingViewTakesNoPressEase() throws {
        let (_, v) = try fixture(style: ["press_scale": 0.5])
        v.flightLook = FlightLook(image: v.bounds)
        v.pressed = true
        XCTAssertNil(v.layer.animation(forKey: "press"))
        v.flightLook = nil
    }

    /// A flight begun from a box still easing back from its press starts
    /// where the box shows, not where its model already is.
    func testAFlightStartsFromThePressedBoxAsShown() throws {
        let (_, v) = try fixture(style: ["press_scale": 0.5])
        v.pressed = true
        RunLoop.main.run(until: Date(timeIntervalSinceNow: PressFeedback.duration + 0.05))
        v.pressed = false
        XCTAssertEqual(v.convert(v.bounds, to: nil).width, 200, accuracy: 1e-6, "the model is unpressed at once")
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
        XCTAssertLessThan(Presenter.shownRect(v).width, 190, "the flight's source is the box as it shows")
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

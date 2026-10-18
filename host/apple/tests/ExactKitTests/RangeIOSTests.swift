#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// `input type="range"` is a UISlider (LLP 1069.001 D5), whose thumb moves
/// from step to step as a browser's does.
///   bun host/apple/build.mjs --test --ios
final class RangeIOSTests: XCTestCase {
    private var window: UIWindow!

    private func slider(_ props: [String: String]) throws -> (Presenter, RangeSlider) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "control", "props": ["type": "range"].merging(props) { $1 },
             "handlers": ["input", "change"], "style": [:]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 28.0],
        ]))
        return (p, try XCTUnwrap(p.controls.controls[1] as? RangeSlider))
    }

    /// The thumb moves from step to step with no tick marks drawn, as a
    /// browser's range without a `list` shows none.
    func testTheThumbSnapsToTheStepWithNoTicks() throws {
        let (p, s) = try slider(["min": "65", "max": "85", "step": "1", "value": "72"])
        XCTAssertEqual(s.value, 72)
        if #available(iOS 26.0, *) { XCTAssertEqual(s.trackConfiguration?.ticks.count ?? 0, 0) }
        s.value = 73.4
        p.controls.rangeMoved(s) // what UIKit sends as the thumb moves (a test bundle routes no target-actions)
        XCTAssertEqual(s.value, 73, "on the nearest step")
    }

    /// A span that is no multiple of the step ends at the last step under `max`, as HTML's does.
    func testAStepThatDoesNotDivideTheSpanStopsBelowMax() throws {
        let (p, s) = try slider(["min": "0", "max": "10", "step": "3", "value": "6"])
        s.value = 9.9
        p.controls.rangeMoved(s)
        XCTAssertEqual(s.value, 9, accuracy: 1e-5)
    }

    /// `step="any"` glides; any other step snaps the thumb as it moves.
    func testAnyStepGlidesAndAStepSnapsAsTheThumbMoves() throws {
        let (p, s) = try slider(["min": "0", "max": "100", "step": "any", "value": "40"])
        s.value = 40.25
        p.controls.rangeMoved(s) // what UIKit sends as the thumb moves (a test bundle routes no target-actions)
        XCTAssertEqual(s.value, 40.25)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["min": "0", "max": "5000", "step": "1"]]]))
        s.value = 73.6
        p.controls.rangeMoved(s)
        XCTAssertEqual(s.value, 74, "snapped as it moves")
        // Back to gliding: a value set is the value shown
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["step": "any"]]]))
        s.value = 2.5
        XCTAssertEqual(s.value, 2.5)
    }

    func testTinyStepsKeepTheValue() throws {
        let (p, s) = try slider(["min": "0", "max": "100", "step": "1e-300", "value": "40"])
        XCTAssertEqual(s.value, 40)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["step": "1e-20"]]]))
        XCTAssertEqual(s.value, 40)
    }

    /// A range's accent is its slider's tint, which draws the filled track
    /// as a default slider's and greys with the tint behind a modal, where
    /// an explicit `minimumTrackTintColor` would keep its colour.
    func testTheAccentIsTheSlidersTint() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "control", "props": ["type": "range", "min": "0", "max": "100", "value": "60"],
             "handlers": ["input", "change"], "style": ["accent_color": [255, 59, 48, 255]]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 28.0],
        ]))
        let s = try XCTUnwrap(p.controls.controls[1] as? RangeSlider)
        XCTAssertEqual(s.tintColor.cgColor.components?.map { ($0 * 255).rounded() }, [255, 59, 48, 255])
        XCTAssertNil(s.minimumTrackTintColor, "no colour of its own on the track")
        p.viewport.tintAdjustmentMode = .dimmed
        XCTAssertNotEqual(s.tintColor.cgColor.components?.map { ($0 * 255).rounded() }, [255, 59, 48, 255], "the track greys with the tint")
    }

    /// VoiceOver's swipes move a range as WebKit does and report each move
    /// as a released drag does: a written step is one move, and at an end
    /// a swipe reports nothing.
    func testVoiceOverMovesTheThumbOneWrittenStep() throws {
        let (p, s) = try slider(["min": "65", "max": "85", "step": "1", "value": "84"])
        var sent: [String] = []
        p.onControlValue = { _, value, input, change in sent.append("\(value) \(input) \(change)") }
        s.accessibilityDecrement()
        XCTAssertEqual(s.value, 83)
        s.accessibilityIncrement(); s.accessibilityIncrement(); s.accessibilityIncrement()
        XCTAssertEqual(s.value, 85)
        XCTAssertEqual(sent, ["83 true true", "84 true true", "85 true true"], "nothing past the end")
        XCTAssertEqual(s.accessibilityValue, "85")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["min": "0", "max": "1000", "value": "500"]]]))
        s.accessibilityIncrement()
        XCTAssertEqual(s.value, 501, "one step of a thousand")
    }

    /// Without a written step, or with `any`, a swipe moves a twentieth of
    /// the span, at least the default step of 1.
    func testVoiceOverMovesAnUnsteppedRangeATwentieth() throws {
        let (p, s) = try slider(["min": "0", "max": "100", "value": "40"])
        s.accessibilityIncrement()
        XCTAssertEqual(s.value, 45)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["max": "10", "value": "4"]]]))
        s.accessibilityIncrement()
        XCTAssertEqual(s.value, 5, "at least one")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["max": "1", "step": "any", "value": "0.5"]]]))
        s.accessibilityIncrement()
        XCTAssertEqual(s.value, 0.55, accuracy: 1e-6)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"]]]))
        XCTAssertFalse(s.isEnabled)
        s.accessibilityIncrement()
        XCTAssertEqual(s.value, 0.55, accuracy: 1e-6, "a disabled range does not move")
    }

    /// A step finer than the thumb's `Float` still adds up, whether or not
    /// the app writes the value back, and VoiceOver reads it.
    func testVoiceOverAddsUpStepsFinerThanTheThumb() throws {
        let (p, s) = try slider(["min": "0", "max": "100", "step": "0.000001", "value": "40"])
        var sent: [String] = []
        p.onControlValue = { _, value, _, _ in sent.append(value) }
        s.accessibilityIncrement(); s.accessibilityIncrement()
        XCTAssertEqual(sent, ["40.000001", "40.000002"])
        XCTAssertEqual(s.accessibilityValue, "40.000002")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["value": "40.000002"]]]))
        s.accessibilityIncrement()
        XCTAssertEqual(sent.last, "40.000003", "and after it is written back")
        p.controls.rangeReleased(s)
        XCTAssertEqual(sent.last, "40.000003", "a release that did not move reports the same value")
    }

    /// The finer value lasts until the person moves the thumb, and new
    /// bounds sanitize it again as HTML's do: 40.000001 shows as 40, so
    /// bounds or a drag that take the thumb away and back must not bring it
    /// back.
    func testAFinerValueFollowsBoundsAndDrags() throws {
        let (p, s) = try slider(["min": "0", "max": "100", "step": "0.000001", "value": "40"])
        var sent: [String] = []
        p.onControlValue = { _, value, _, _ in sent.append(value) }
        s.accessibilityIncrement()
        XCTAssertEqual(s.accessibilityValue, "40.000001")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["max": "30"]]]))
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["max": "100", "min": "40"]]]))
        XCTAssertEqual(s.accessibilityValue, "40", "bounds that clamped the thumb drop the finer value")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["value": "40.5"]]]))
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["value": "40"]]]))
        s.accessibilityIncrement()
        s.value = 100; p.controls.rangeMoved(s)
        s.value = 40; p.controls.rangeMoved(s)
        XCTAssertEqual(sent.last, "40", "the thumb dragged back to the minimum")
        // A new minimum keeps a value still in bounds and on the step
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["value": "40.000003"]]]))
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["min": "40.000002"]]]))
        XCTAssertEqual(s.accessibilityValue, "40.000003")
        s.accessibilityIncrement()
        XCTAssertEqual(sent.last, "40.000004")
        // A new minimum above the value clamps it there, not where UIKit
        // clamps the thumb's `Float`
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["min": "0", "value": "40.000001"]]]))
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["min": "40.000002"]]]))
        XCTAssertEqual(s.accessibilityValue, "40.000002")
        p.controls.rangeReleased(s)
        XCTAssertEqual(sent.last, "40.000002", "and a release reports it")
    }
}

#endif

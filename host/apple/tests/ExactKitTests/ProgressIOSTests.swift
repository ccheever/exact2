#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1069.001, amended 2026-10-07, on UIKit: an indeterminate `progress`
/// is a `UIActivityIndicatorView`, `.medium` in the default 20 × 20 box and
/// `.large` from 37 points, centred, in the node's `color`, named by
/// `aria-label`; it turns while it shows and stops when hidden or removed
/// (the agent's clock, which holds it, needs a session these presenters
/// lack). UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class ProgressIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }
    /// A spinner whose style has the red `color` it computes; `written`, its
    /// author wrote that on it (LLP 1115.000 D2), else it only inherits it.
    private func progress(_ id: Int, _ props: [String: String] = [:], side: Double = 20, y: Double = 0, written: Bool = false) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control",
          "props": ["type": "progress", "accessibilityRole": "progressbar", "accessibilityBusy": "true"].merging(props) { $1 },
          "handlers": [], "style": ["text_color": [255, 0, 0, 255]].merging(written ? ["written": ["text_color"]] : [:]) { $1 }],
         ["op": "frame", "id": id, "x": 0.0, "y": y, "w": side, "h": side]]
    }
    private func box(_ id: Int) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "view", "props": [:], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0]]
    }

    /// LLP 1115 D4: a `color` the indicator only inherits is not its own;
    /// it keeps UIKit's. A colour its author wrote on it is its own, the
    /// parent's colour written again and a platform colour included (LLP
    /// 1115.000 D2).
    func testAnInheritedColourLeavesUIKitsIndicatorColour() throws {
        var ops = box(1) + progress(2) + progress(3, written: true) + progress(4, written: true)
        ops[0]["style"] = ["text_color": [255, 0, 0, 255], "written": ["text_color"]]
        let red: [String: Any] = ["sys": "systemRedColor", "c": [[255, 56, 60, 255], [255, 66, 69, 255]]]
        ops[6]["style"] = ["text_color": red, "written": ["text_color"]]
        let p = presenter(ops + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]])
        XCTAssertEqual(try XCTUnwrap(p.controls.spinners[2]).color, ControlHost.platformInk)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        try XCTUnwrap(p.controls.spinners[3]).color.getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertEqual([r, g, b, a], [1, 0, 0, 1], "equal to its parent's, and written")
        // A platform colour its author wrote: UIKit's red for these traits.
        var want: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        let spinner = try XCTUnwrap(p.controls.spinners[4])
        UIColor.systemRed.resolvedColor(with: spinner.traitCollection).getRed(&want.0, green: &want.1, blue: &want.2, alpha: &want.3)
        spinner.color.getRed(&r, green: &g, blue: &b, alpha: &a)
        for (got, want) in zip([r, g, b, a], [want.0, want.1, want.2, want.3]) { XCTAssertEqual(got, want, accuracy: 0.01, "\([r, g, b, a])") }
    }

    /// A colour its author wrote that names the tint reaches the indicator undimmed, also from a
    /// refresh while UIKit dims tints: the indicator keeps the colour it is given
    func testATintColourReachesTheIndicatorUndimmed() throws {
        var ops = progress(2, written: true)
        ops[0]["style"] = ["text_color": ["sys": "@tint", "c": [0, 122, 255, 255]], "written": ["text_color"]]
        let p = presenter(ops + [["op": "roots", "ids": [2]]])
        window.tintColor = UIColor(red: 1, green: 0, blue: 0, alpha: 1)
        let spinner = try XCTUnwrap(p.controls.spinners[2])
        p.viewport.tintAdjustmentMode = .dimmed
        p.controls.syncProgress()
        p.viewport.tintAdjustmentMode = .automatic
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        spinner.color.resolvedColor(with: spinner.traitCollection).getRed(&r, green: &g, blue: &b, alpha: &a)
        for (got, want) in zip([r, g, b, a], [CGFloat(1), 0, 0, 1]) { XCTAssertEqual(got, want, accuracy: 0.01, "the red tint once the dimming ends: \([r, g, b, a])") }
    }

    /// `accent-color: AccentColor` under an authored accent keeps that one,
    /// as the web's `var(--exact-accent, AccentColor)` does (LLP 1121 D9,
    /// stage 9 F1); with none authored above, the role's own.
    func testTheAccentRoleUnderAnAuthoredAccentKeepsIt() throws {
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
        let green: [Double] = [0, 200, 0, 255]
        var ops = box(1) + box(5) + progress(2) + box(6) + progress(3)
        ops[0]["style"] = ["text_color": [0, 0, 0, 255], "accent_color": green]
        ops[2]["style"] = ["text_color": [0, 0, 0, 255], "accent_color": tint]
        ops[4]["style"] = ["text_color": [255, 0, 0, 255], "accent_color": tint]
        ops[6]["style"] = ["text_color": [0, 0, 0, 255], "accent_color": tint]
        ops[8]["style"] = ["text_color": [255, 0, 0, 255], "accent_color": tint]
        let p = presenter(ops + [["op": "children", "id": 1, "ids": [5]], ["op": "children", "id": 5, "ids": [2]],
                                 ["op": "children", "id": 6, "ids": [3]], ["op": "roots", "ids": [1, 6]]])
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        try XCTUnwrap(p.controls.spinners[2]).color.getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertEqual([r, g, b, a].map { ($0 * 255).rounded() }, [0, 200, 0, 255], "the authored accent, kept through the role")
        try XCTUnwrap(p.controls.spinners[3]).color.getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertNotEqual([r, g, b, a].map { ($0 * 255).rounded() }, [0, 200, 0, 255], "none authored above: the role's own")
    }

    /// A system colour is an authored accent (only the bare tint is the
    /// role): under the role, a systemRed ancestor's accent is kept.
    func testASystemColourAboveTheRoleIsAuthored() throws {
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
        let red: [String: Any] = ["sys": "systemRedColor", "c": [[255, 56, 60, 255], [255, 66, 69, 255]]]
        var ops = box(1) + box(5) + progress(2)
        ops[0]["style"] = ["text_color": [0, 0, 0, 255], "accent_color": red]
        ops[2]["style"] = ["text_color": [0, 0, 0, 255], "accent_color": tint]
        ops[4]["style"] = ["text_color": [255, 0, 0, 255], "accent_color": tint]
        let p = presenter(ops + [["op": "children", "id": 1, "ids": [5]], ["op": "children", "id": 5, "ids": [2]], ["op": "roots", "ids": [1]]])
        let spinner = try XCTUnwrap(p.controls.spinners[2])
        var want: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0), r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        UIColor.systemRed.resolvedColor(with: spinner.traitCollection).getRed(&want.0, green: &want.1, blue: &want.2, alpha: &want.3)
        spinner.color.getRed(&r, green: &g, blue: &b, alpha: &a)
        for (got, want) in zip([r, g, b, a], [want.0, want.1, want.2, want.3]) { XCTAssertEqual(got, want, accuracy: 0.02, "\([r, g, b, a])") }
    }

    func testAnInheritedAccentResolvesInTheSpinnersAppearance() throws {
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
        let pair: [Any] = [[200, 0, 0, 255], [0, 200, 0, 255]]
        var ops = box(1) + box(5) + progress(2)
        ops[0]["style"] = ["accent_color": pair]
        ops[2]["style"] = ["accent_color": tint]
        ops[4]["style"] = ["accent_color": tint]
        let p = presenter(ops + [["op": "children", "id": 1, "ids": [5]], ["op": "children", "id": 5, "ids": [2]], ["op": "roots", "ids": [1]]])
        let author = try XCTUnwrap(p.views[1]), requestor = try XCTUnwrap(p.views[2])
        author.applyStyle(author.style.merging(["color_scheme": "light"]) { $1 })
        requestor.applyStyle(requestor.style.merging(["color_scheme": "dark"]) { $1 })
        requestor.updateTraitsIfNeeded()
        p.controls.syncProgress()
        XCTAssertFalse(try XCTUnwrap(p.views[1]).drawsDark)
        XCTAssertTrue(try XCTUnwrap(p.views[2]).drawsDark)
        let color = try XCTUnwrap(p.controls.spinners[2]?.color)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        color.getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertEqual([r, g, b, a].map { ($0 * 255).rounded() }, [0, 200, 0, 255])
    }

    func testHighlightUnderTheRoleUsesTheAuthoringViewsTint() throws {
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
        let highlight: [String: Any] = ["sys": "@tint/0.2", "c": [[0, 136, 255, 51], [0, 145, 255, 51]]]
        var ops = box(1) + box(5) + progress(2)
        ops[0]["style"] = ["accent_color": highlight]
        ops[2]["style"] = ["accent_color": tint]
        ops[4]["style"] = ["accent_color": tint]
        let p = presenter(ops + [["op": "children", "id": 1, "ids": [5]], ["op": "children", "id": 5, "ids": [2]], ["op": "roots", "ids": [1]]])
        p.views[1]?.tintColor = .systemRed
        p.views[2]?.tintColor = .systemBlue
        p.controls.syncProgress()
        let owner = try XCTUnwrap(p.views[1]), requestor = try XCTUnwrap(p.views[2]), spinner = try XCTUnwrap(p.controls.spinners[2])
        let expected = try XCTUnwrap(owner.style["accent_color"]?.channels(dark: requestor.drawsDark, tint: owner.tintColor))
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        spinner.color.getRed(&r, green: &g, blue: &b, alpha: &a)
        for (got, want) in zip([r, g, b, a].map { Double($0) * 255 }, expected) { XCTAssertEqual(got, want, accuracy: 0.01) }
    }

    func testItIsUIKitsActivityIndicatorSizedByItsBox() throws {
        let p = presenter(box(1) + progress(2, ["testId": "busy", "accessibilityLabel": "Loading"], written: true) + progress(3, side: 40, y: 40)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let medium = try XCTUnwrap(p.controls.spinners[2])
        XCTAssertEqual(medium.style, .medium)
        XCTAssertEqual(medium.frame.size, medium.intrinsicContentSize)
        XCTAssertEqual(medium.center, CGPoint(x: 10, y: 10), "centred in its box")
        XCTAssertFalse(medium.hidesWhenStopped, "stopped, it shows its frame")
        XCTAssertEqual(medium.accessibilityLabel, "Loading")
        XCTAssertEqual(medium.accessibilityIdentifier, "busy")
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        medium.color.getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertEqual([r, g, b, a], [1, 0, 0, 1], "its node's color")
        XCTAssertEqual(try XCTUnwrap(p.controls.spinners[3]).style, .large)
        XCTAssertNil(p.controls.controls[2], "an indicator is not one of the form controls")
        let seen = try XCTUnwrap(p.controls.observation(try XCTUnwrap(p.views[2])))
        XCTAssertEqual(seen["view"] as? String, "UIActivityIndicatorView")
        XCTAssertEqual(seen["style"] as? String, "medium")
        if !ExactEnv.agentFreezes { XCTAssertTrue(medium.isAnimating) }
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["accessibilityLabel"]]]))
        XCTAssertNotEqual(medium.accessibilityLabel, "Loading", "a cleared label is cleared")
    }

    /// LLP 1116 D8: a drawn `role="progressbar"` box is one element whose
    /// value is the percentage UIKit's own progress view speaks, or its
    /// `aria-valuetext`; its fill is not a second element.
    func testADrawnProgressbarSpeaksItsValue() throws {
        let p = presenter(box(1) + [["op": "create", "id": 2, "kind": "view", "handlers": [], "style": [:],
                                     "props": ["accessibilityRole": "progressbar", "accessibilityValueNow": "30", "accessibilityLabel": "Download"]],
                                    ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 8.0]]
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let bar = try XCTUnwrap(p.views[2])
        XCTAssertTrue(bar.isAccessibilityElement)
        XCTAssertEqual(bar.accessibilityLabel, "Download")
        XCTAssertEqual(bar.accessibilityValue, "30%")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityValueNow": "2", "accessibilityValueMax": "8"]]]))
        XCTAssertEqual(bar.accessibilityValue, "25%")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityValueText": "2 of 8 glasses"]]]))
        XCTAssertEqual(bar.accessibilityValue, "2 of 8 glasses")
    }

    private func bar(_ id: Int, _ props: [String: String], y: Double = 0) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control",
          "props": ["type": "progress", "accessibilityRole": "progressbar"].merging(props) { $1 },
          "handlers": [], "style": ["accent_color": [255, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": y, "w": 160.0, "h": 16.0]]
    }

    /// LLP 1116 D8: with a `value`, a `progress` is UIKit's progress view
    /// (`.default`) across its content box at its own height, centred, at
    /// HTML's fraction (`max` 1 unsaid; past `max`, full), tinted by its
    /// `accent-color` and named by `aria-label`; VoiceOver hears the
    /// percentage. Its value taken away, it is the activity indicator again.
    func testAValuedProgressIsUIKitsProgressView() throws {
        let p = presenter(box(1) + bar(2, ["value": "0.25", "testId": "water", "accessibilityLabel": "Water"]) + bar(3, ["value": "3", "max": "8"], y: 40)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let water = try XCTUnwrap(p.controls.bars[2])
        XCTAssertEqual(water.progressViewStyle, .default)
        XCTAssertEqual(water.progress, 0.25)
        XCTAssertEqual(water.frame.width, 160, "across its box")
        XCTAssertEqual(water.frame.height, water.intrinsicContentSize.height)
        XCTAssertEqual(water.frame.midY, 8, "centred in its 16-point box")
        XCTAssertNil(p.controls.spinners[2], "no activity indicator")
        XCTAssertFalse(water.isUserInteractionEnabled)
        XCTAssertEqual(water.accessibilityLabel, "Water")
        XCTAssertEqual(water.accessibilityIdentifier, "water")
        XCTAssertEqual(water.accessibilityValue, "25%")
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        try XCTUnwrap(water.progressTintColor).getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertEqual([r, g, b, a], [1, 0, 0, 1], "its accent")
        XCTAssertEqual(try XCTUnwrap(p.controls.bars[3]).progress, 0.375)
        let seen = try XCTUnwrap(p.controls.observation(try XCTUnwrap(p.views[2])))
        XCTAssertEqual(seen["view"] as? String, "UIProgressView")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": "2", "accessibilityValueText": "All of it"]]]))
        XCTAssertEqual(water.progress, 1, "past max: full")
        XCTAssertEqual(water.accessibilityValue, "All of it")
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["value"]]]))
        XCTAssertNil(p.controls.bars[2])
        XCTAssertNil(water.superview)
        XCTAssertNotNil(p.controls.spinners[2], "no value: the indicator")
    }

    func testItStopsWhenHiddenOrRemoved() throws {
        try XCTSkipIf(ExactEnv.agentFreezes, "the agent's frozen clock holds every indicator")
        let p = presenter(box(1) + progress(2) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let spinner = try XCTUnwrap(p.controls.spinners[2])
        XCTAssertTrue(spinner.isAnimating)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["display": "none", "text_color": [0, 0, 0, 255]]]]))
        XCTAssertFalse(spinner.isAnimating, "under a display: none box it stops")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["text_color": [0, 0, 0, 255]]]]))
        XCTAssertTrue(spinner.isAnimating, "shown again, it turns")
        p.apply(wireBatch([["op": "destroy", "id": 2], ["op": "children", "id": 1, "ids": []]]))
        XCTAssertNil(p.controls.spinners[2], "removed with its node")
        XCTAssertNil(spinner.superview)
    }
}
#endif

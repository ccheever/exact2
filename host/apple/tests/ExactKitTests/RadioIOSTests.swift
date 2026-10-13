#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// x2apps survey #2 on UIKit, which has no radio: the SF Symbols pair at
/// the reader's body size with a 44-point target (LLP 1115.000 D13.2),
/// whose tap checks it and unchecks the rest of its group at once, fires
/// `input` then `change` with its value, nothing when it was checked, and
/// shows the bound group after the commit; the arrows move the focus and
/// the check; the agent's tree says radio. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class RadioIOSTests: XCTestCase {
    private var window: UIWindow!
    private var bound = "red"
    private var accepts = true
    private var heard: [String] = []
    private static let values: [UInt32: String] = [2: "red", 3: "green", 4: "gray", 5: "blue"]

    private func group(_ id: UInt32) -> RadioGroup {
        let enabled: [UInt32] = [2, 3, 5]
        let at = enabled.firstIndex(of: id) ?? 0
        return RadioGroup(json: try! JSONSerialization.data(withJSONObject: [
            "group": [2, 3, 4, 5], "next": enabled[(at + 1) % 3], "previous": enabled[(at + 2) % 3]]))
    }

    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 200))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.controls.radioGroup = { [unowned self] in group($0) }
        p.onControlValue = { [unowned self, unowned p] id, value, input, change in
            if input { heard.append("input \(id) \(value)") }
            if change { heard.append("change \(id) \(value)") }
            if change, accepts {
                bound = value
                p.apply(wireBatch(Self.values.map { ["op": "props", "id": Int($0.key), "set": ["checked": $0.value == bound ? "true" : "false"]] }))
            }
        }
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view"]]
        for (id, value) in Self.values.sorted(by: { $0.key < $1.key }) {
            var props = ["type": "radio", "name": "color", "value": value, "checked": value == bound ? "true" : "false",
                         "accessibilityLabel": value.capitalized, "testId": value]
            if id == 4 { props["disabled"] = "true" }
            ops.append(["op": "create", "id": Int(id), "kind": "control", "props": props, "handlers": ["input", "change"], "style": [:]])
            ops.append(["op": "frame", "id": Int(id), "x": Double(id) * 30, "y": 10.0, "w": 16.0, "h": 16.0])
        }
        ops += [["op": "children", "id": 1, "ids": [2, 3, 4, 5]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0]]
        p.apply(wireBatch(ops))
        return p
    }

    private func shown(_ p: Presenter) -> [UInt32] { [2, 3, 4, 5].filter { p.controls.radioShows($0) } }
    /// A tap's registered actions, as a touch up sends them (the test host
    /// delivers no `sendActions` for a touch).
    private func tap(_ control: UIControl) {
        for target in control.allTargets {
            for action in control.actions(forTarget: target, forControlEvent: .touchUpInside) ?? [] {
                _ = (target as NSObject).perform(Selector(action), with: control)
            }
        }
    }

    func testATapChecksTheGroupAndAWriteNothingActionSnapsItBack() throws {
        let p = presenter()
        let green = try XCTUnwrap(p.controls.controls[3] as? ExactRadio)
        XCTAssertEqual(shown(p), [2])
        tap(green)
        XCTAssertEqual(heard, ["input 3 green", "change 3 green"])
        XCTAssertEqual(shown(p), [3])
        XCTAssertTrue(green.accessibilityTraits.contains(.selected))
        XCTAssertNil(green.accessibilityValue, "no checkbox value")
        tap(green)
        XCTAssertEqual(heard.count, 2, "a checked radio's tap fires nothing")
        accepts = false
        tap(try XCTUnwrap(p.controls.controls[5]))
        XCTAssertEqual(heard.suffix(2), ["input 5 blue", "change 5 blue"])
        XCTAssertEqual(shown(p), [3], "an action that wrote nothing: the group shows its bound state")
        XCTAssertFalse(try XCTUnwrap(p.controls.controls[4]).isEnabled)
        XCTAssertEqual(p.controls.observation(try XCTUnwrap(p.views[3]))?["view"] as? String, "radio")
    }

    /// LLP 1115.000 D13.2: a radio is the platform's symbols at the body
    /// text's size, as the checkbox's mark is, where it had been Safari's
    /// 16-point circle at every text size; a finger has 44 points to hit.
    func testARadioIsTheSymbolPairAtBodySizeWithAFingersTarget() throws {
        let p = presenter()
        let red = try XCTUnwrap(p.controls.controls[2] as? ExactRadio), green = try XCTUnwrap(p.controls.controls[3] as? ExactRadio)
        let body = UIFont.preferredFont(forTextStyle: .body, compatibleWith: red.traitCollection)
        let symbol = UIImage.SymbolConfiguration(font: body, scale: .large)
        let size = red.intrinsicContentSize
        XCTAssertGreaterThan(size.width, 18, "the body text's mark, not Safari's 16-point circle")
        XCTAssertEqual(size, ExactCheckbox(frame: .zero).intrinsicContentSize, "the checkbox's size")
        XCTAssertEqual(red.mark(on: true, tint: .systemBlue)?.pngData(), UIImage(systemName: "circle.inset.filled", withConfiguration: symbol)?.withTintColor(.systemBlue, renderingMode: .alwaysOriginal).pngData(), "checked: the filled symbol")
        XCTAssertEqual(green.mark(on: false, tint: .systemGray)?.pngData(), UIImage(systemName: "circle", withConfiguration: symbol)?.withTintColor(.systemGray, renderingMode: .alwaysOriginal).pngData(), "unchecked: the empty circle")
        // It follows the reader's text size, as the body text beside it does.
        let large = ExactRadio(frame: .zero)
        window.addSubview(large)
        large.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
        large.updateTraitsIfNeeded()
        XCTAssertGreaterThan(large.intrinsicContentSize.width, size.width * 1.5)
        large.removeFromSuperview()
        // The mark as drawn: ink at the centre only when checked.
        red.frame = CGRect(origin: .zero, size: size)
        func centre(_ radio: ExactRadio) -> UInt8 {
            let format = UIGraphicsImageRendererFormat(); format.scale = 1
            let image = UIGraphicsImageRenderer(bounds: radio.bounds, format: format).image { _ in radio.draw(radio.bounds) }
            let cg = image.cgImage!, w = cg.width, h = cg.height
            var data = [UInt8](repeating: 0, count: w * h * 4)
            let ctx = CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(),
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
            return data[((h / 2) * w + w / 2) * 4 + 3]
        }
        green.frame = CGRect(origin: .zero, size: size)
        XCTAssertGreaterThan(centre(red), 200, "checked: its centre is filled")
        XCTAssertEqual(centre(green), 0, "unchecked: an empty circle")
        // A finger's target: 44 points each way, though the mark is smaller.
        XCTAssertLessThan(size.width, 44)
        let middle = CGPoint(x: red.bounds.midX, y: red.bounds.midY)
        for (dx, dy) in [(-21.0, 0.0), (0.0, -21.0), (0.0, 21.0)] {
            XCTAssertTrue(red.point(inside: CGPoint(x: middle.x + dx, y: middle.y + dy), with: nil), "\(dx), \(dy) is within 22 points of its middle")
        }
        XCTAssertFalse(red.point(inside: CGPoint(x: middle.x - 23, y: middle.y), with: nil))
        // To its right, where no other radio is: the last of the row.
        let blue = try XCTUnwrap(p.controls.controls[5] as? ExactRadio)
        XCTAssertTrue(blue.point(inside: CGPoint(x: blue.bounds.midX + 21, y: blue.bounds.midY), with: nil))
        // In the page: a touch beside the mark, outside its node's box, reaches it.
        let node = try XCTUnwrap(p.views[2])
        p.controls.sync()
        let beside = red.convert(CGPoint(x: red.bounds.minX - 6, y: red.bounds.midY), to: window)
        XCTAssertFalse(node.convert(node.bounds, to: window).contains(beside), "past the node's box")
        XCTAssertTrue(window.hitTest(beside, with: nil) === red, "and still the radio's")
        // Two radios 30 points apart: their margins overlap, and each touch
        // between them is the nearer mark's.
        let a = red.convert(CGPoint(x: red.bounds.midX, y: red.bounds.midY), to: window), b = green.convert(CGPoint(x: green.bounds.midX, y: green.bounds.midY), to: window)
        XCTAssertEqual(b.x - a.x, 30, accuracy: 0.5)
        XCTAssertTrue(window.hitTest(CGPoint(x: a.x + 13, y: a.y), with: nil) === red, "13 points from red, 17 from green")
        XCTAssertTrue(window.hitTest(CGPoint(x: a.x + 17, y: a.y), with: nil) === green, "17 points from red, 13 from green")
    }

    func testARIAUnavailableRadioKeepsItsTraitAfterRefusedChange() throws {
        let p = presenter()
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["accessibilityDisabled": "true"]]]))
        let green = try XCTUnwrap(p.controls.controls[3] as? ExactRadio)
        XCTAssertTrue(green.accessibilityTraits.contains(.notEnabled))
        accepts = false
        tap(green)
        XCTAssertEqual(shown(p), [2])
        XCTAssertTrue(green.isEnabled, "ARIA state leaves input enabled")
        XCTAssertTrue(green.accessibilityTraits.contains(.notEnabled))
    }

    func testTheArrowsMoveFocusAndCheckAndTheAgentTypesTrue() throws {
        let p = presenter()
        let red = try XCTUnwrap(p.views[2])
        XCTAssertTrue(red.canBecomeFirstResponder)
        XCTAssertTrue(red.becomeFirstResponder())
        XCTAssertTrue(p.controls.radioKey(red, "ArrowDown", held: ""))
        XCTAssertEqual(shown(p), [3])
        XCTAssertTrue(try XCTUnwrap(p.views[3]).isFirstResponder)
        XCTAssertTrue(p.controls.radioKey(try XCTUnwrap(p.views[3]), "ArrowRight", held: ""))
        XCTAssertEqual(shown(p), [5], "the disabled radio is skipped")
        XCTAssertTrue(p.controls.radioKey(try XCTUnwrap(p.views[5]), "ArrowDown", held: ""))
        XCTAssertEqual(shown(p), [2], "wrapping")
        let blue = try XCTUnwrap(p.views[5])
        XCTAssertNotNil(p.controls.type(blue, "false")?["error"])
        XCTAssertEqual(p.controls.type(blue, "true")?["checked"] as? Bool, true)
        XCTAssertEqual(shown(p), [5])
        p.syncAccessibility()
        window.layoutIfNeeded()
        let all = p.axElements(roots: [p.viewport])["elements"] as? [[String: Any]] ?? []
        let radio = try XCTUnwrap(all.first { ($0["native"] as? [String: Any])?["class"] as? String == "ExactRadio" && $0["name"] as? String == "Blue" }, "\(all)")
        XCTAssertEqual(radio["role"] as? String, "radio")
        XCTAssertEqual((radio["states"] as? [String: Any])?["checked"] as? Bool, true)
    }
}
#endif

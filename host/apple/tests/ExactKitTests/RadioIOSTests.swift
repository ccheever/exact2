#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// x2apps survey #2 on UIKit, which has no radio: a drawn circle whose tap
/// checks it and unchecks the rest of its group at once, fires `input` then
/// `change` with its value, nothing when it was checked, and shows the bound
/// group after the commit; the arrows move the focus and the check; the
/// agent's tree says radio. UIKit, so a simulator runs it:
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

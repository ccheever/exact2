#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// x2apps survey #2 on AppKit: `input type="radio"` is AppKit's radio button
/// in a group Exact owns by `name` — a click checks it and unchecks the rest
/// at once, `input` then `change` carry its value, a checked one fires
/// nothing, and after the commit the group shows its bound `checked`; the
/// arrows move the focus and the check through the enabled radios, wrapping.
final class RadioMacTests: XCTestCase {
    private var window: NSWindow!
    /// The app's state: which value the bound radios show.
    private var bound = "red"
    /// Whether the action writes the value it hears.
    private var accepts = true
    private var heard: [String] = []

    override func tearDown() { window?.close(); window = nil }

    private static let values: [UInt32: String] = [2: "red", 3: "green", 4: "gray", 5: "blue"]

    private func group(_ id: UInt32) -> RadioGroup {
        guard Self.values[id] != nil else {
            return RadioGroup(json: try! JSONSerialization.data(withJSONObject: ["group": [id], "next": NSNull(), "previous": NSNull()]))
        }
        // Tree order 2, 3, 4, 5; 4 is disabled, so the arrows skip it.
        let enabled: [UInt32] = [2, 3, 5]
        let at = enabled.firstIndex(of: id) ?? 0
        let next = enabled[(at + 1) % 3], previous = enabled[(at + 2) % 3]
        return RadioGroup(json: try! JSONSerialization.data(withJSONObject: ["group": [2, 3, 4, 5], "next": next, "previous": previous]))
    }

    private func checkedOps() -> [[String: Any]] {
        Self.values.map { ["op": "props", "id": Int($0.key), "set": ["checked": $0.value == bound ? "true" : "false"]] }
    }

    private func presenter(bound checked: Bool = true) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.controls.radioGroup = { [unowned self] in group($0) }
        p.onControlValue = { [unowned self, unowned p] id, value, input, change in
            if input { heard.append("input \(id) \(value)") }
            if change { heard.append("change \(id) \(value)") }
            // The action's commit: the bound radios follow the state it wrote.
            if change, accepts, checked {
                bound = value
                p.apply(wireBatch(checkedOps()))
            }
        }
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view"]]
        for (id, value) in Self.values.sorted(by: { $0.key < $1.key }) {
            var props = ["type": "radio", "name": "color", "value": value, "accessibilityLabel": value.capitalized, "testId": value]
            if checked { props["checked"] = value == bound ? "true" : "false" }
            if id == 4 { props["disabled"] = "true" }
            ops.append(["op": "create", "id": Int(id), "kind": "control", "props": props, "handlers": ["input", "change"], "style": [:]])
            ops.append(["op": "frame", "id": Int(id), "x": Double(id) * 30, "y": 10.0, "w": 13.0, "h": 13.0])
        }
        ops += [["op": "children", "id": 1, "ids": [2, 3, 4, 5]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0]]
        p.apply(wireBatch(ops))
        return p
    }

    private func shown(_ p: Presenter) -> [UInt32] { [2, 3, 4, 5].filter { p.controls.radioShows($0) } }

    func testAClickChecksItsGroupAtOnceAndAWriteNothingActionSnapsItBack() throws {
        let p = presenter()
        let green = try XCTUnwrap(p.controls.controls[3] as? RadioButtonMac)
        XCTAssertEqual(green.cell?.className.contains("RadioCell"), true)
        XCTAssertFalse(green.acceptsFirstResponder, "the node holds the focus")
        XCTAssertEqual(shown(p), [2])
        green.performClick(nil)
        XCTAssertEqual(heard, ["input 3 green", "change 3 green"])
        XCTAssertEqual(shown(p), [3])
        XCTAssertTrue(window.firstResponder === p.views[3], "a click focuses the radio, as Chrome's does")
        green.performClick(nil)
        XCTAssertEqual(heard.count, 2, "a checked radio's click fires nothing")
        XCTAssertEqual(shown(p), [3])
        accepts = false
        try XCTUnwrap(p.controls.controls[5]).performClick(nil)
        XCTAssertEqual(heard.suffix(2), ["input 5 blue", "change 5 blue"])
        XCTAssertEqual(shown(p), [3], "an action that wrote nothing: the whole group shows its bound state")
        XCTAssertFalse(try XCTUnwrap(p.controls.controls[4]).isEnabled)
        XCTAssertFalse(p.controls.checkRadio(4), "a disabled radio takes no click")
        XCTAssertEqual(p.controls.observation(try XCTUnwrap(p.views[3]))?["view"] as? String, "NSButton(radio)")
    }

    func testTheArrowsMoveTheFocusAndTheCheckSkippingADisabledRadioAndWrapping() throws {
        let p = presenter()
        let red = try XCTUnwrap(p.views[2])
        XCTAssertTrue(red.acceptsFirstResponder)
        XCTAssertTrue(red.canBecomeKeyView, "the checked radio is its group's Tab stop")
        XCTAssertFalse(try XCTUnwrap(p.views[3]).canBecomeKeyView, "the rest are not")
        window.makeFirstResponder(red)
        let down = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                                  context: nil, characters: "\u{F701}", charactersIgnoringModifiers: "\u{F701}", isARepeat: false, keyCode: 125))
        red.keyDown(with: down)
        XCTAssertEqual(heard, ["input 3 green", "change 3 green"])
        XCTAssertEqual(shown(p), [3])
        XCTAssertTrue(window.firstResponder === p.views[3])
        XCTAssertTrue(p.controls.radioKey(try XCTUnwrap(p.views[3]), "ArrowRight", held: ""))
        XCTAssertEqual(shown(p), [5], "the disabled radio is skipped")
        XCTAssertTrue(p.controls.radioKey(try XCTUnwrap(p.views[5]), "ArrowDown", held: ""))
        XCTAssertEqual(shown(p), [2], "and the last wraps to the first")
        XCTAssertTrue(p.controls.radioKey(try XCTUnwrap(p.views[2]), "ArrowUp", held: ""))
        XCTAssertEqual(shown(p), [5])
        XCTAssertTrue(window.firstResponder === p.views[5])
        XCTAssertFalse(p.controls.radioKey(try XCTUnwrap(p.views[5]), "ArrowUp", held: "Meta+"), "a chord is not the radio's")
        heard.removeAll()
        XCTAssertTrue(p.controls.radioKey(try XCTUnwrap(p.views[5]), " ", held: ""))
        XCTAssertEqual(heard, [], "Space on a checked radio fires nothing")
    }

    func testAnUnboundGroupKeepsThePersonsCheck() throws {
        let p = presenter(bound: false)
        XCTAssertEqual(shown(p), [])
        XCTAssertTrue(try XCTUnwrap(p.views[2]).canBecomeKeyView, "none checked: the first enabled radio is the stop")
        try XCTUnwrap(p.controls.controls[5]).performClick(nil)
        try XCTUnwrap(p.controls.controls[3]).performClick(nil)
        XCTAssertEqual(shown(p), [3])
        XCTAssertEqual(heard, ["input 5 blue", "change 5 blue", "input 3 green", "change 3 green"])
    }

    func testTheAgentTypesTrueAndTheTreeHasARadio() throws {
        let p = presenter()
        let blue = try XCTUnwrap(p.views[5])
        XCTAssertEqual(p.controls.type(blue, "false")?["error"] as? String,
                       "radio 5 cannot be typed false: a radio is unchecked by checking another of its group")
        XCTAssertEqual(p.controls.type(blue, "true")?["checked"] as? Bool, true)
        XCTAssertEqual(shown(p), [5])
        p.syncAccessibility()
        let all = p.axElements(roots: [p.viewport])["elements"] as? [[String: Any]] ?? []
        let radio = try XCTUnwrap(all.first { $0["testId"] as? String == "blue" }, "\(all)")
        XCTAssertEqual([radio["role"] as? String, radio["name"] as? String], ["radio", "Blue"])
        XCTAssertEqual((radio["native"] as? [String: Any])?["role"] as? String, "AXRadioButton")
        XCTAssertEqual((radio["states"] as? [String: Any])?["checked"] as? Bool, true)
        let red = try XCTUnwrap(all.first { $0["testId"] as? String == "red" })
        XCTAssertEqual((red["states"] as? [String: Any])?["checked"] as? Bool, false)
    }
}
#endif

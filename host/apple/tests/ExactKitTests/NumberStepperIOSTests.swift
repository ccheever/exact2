// @ref LLP 1116.007 D4: `input type="number"` is a field with a UIStepper at its
// trailing edge, stepping the field's text by HTML's rules and saying so as
// `input` then `change`; `appearance="textfield"`, any other `type` and a
// field too narrow for both leave the bare field.
#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class NumberStepperIOSTests: XCTestCase {
    /// A number input in a session, 300×44 as a bordered field's box, its
    /// text starting at `rect`'s edges as the kernel publishes it.
    private func fixture(_ props: [String: String] = [:], width: CGFloat = 300, rect: [Double]? = nil, handlers: Set<String> = ["input", "change"]) -> (node: NodeView, heard: () -> [String], keep: ExactSession) {
        let session = ExactApp.shared.makeSession(label: "number-stepper")
        let p = session.presenter
        let node = NodeView(id: 1, kind: "input", presenter: p)
        node.bounds = CGRect(x: 0, y: 0, width: width, height: 44)
        node.handlers = handlers
        p.views[node.id] = node
        var heard: [String] = []
        p.onInput = { _, value in heard.append("input \(value)") }
        p.onChange = { _, value in heard.append("change \(value)") }
        node.applyProps(set: ["type": "number", "min": "1", "max": "99", "step": "1", "value": "3", "aria-label": "Quantity", "accessibilityLabel": "Quantity", "testId": "qty"].merging(props) { _, new in new }, clear: [])
        node.applyStyle(["font_size": .number(17)])
        node.applyFieldContent(["rect": rect ?? [6.0, 5.0, Double(width) - 12, 34.0]])
        return (node, { heard }, session)
    }

    /// The plus or the minus: the stepper's value moves a step, as UIKit's
    /// does when a finger lifts on either half, and its `valueChanged` action
    /// runs (a test bundle has no application to send it, so it is called, as
    /// `FieldCompositionIOSTests` calls a field's `fieldChanged`).
    private func press(_ node: NodeView, up: Bool) {
        guard let stepper = node.stepper else { return XCTFail("no stepper") }
        stepper.value += up ? stepper.stepValue : -stepper.stepValue
        node.stepperChanged(stepper)
    }

    func testANumberInputHasAStepperAtItsTrailingEdgeAndTheTextGivesWayToIt() throws {
        let (node, _, session) = fixture()
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        XCTAssertTrue(stepper.superview === node)
        let field = try XCTUnwrap(node.field)
        XCTAssertEqual(stepper.minimumValue, 1)
        XCTAssertEqual(stepper.maximumValue, 99)
        XCTAssertEqual(stepper.stepValue, 1)
        XCTAssertEqual(stepper.value, 3, "it shows what the field holds")
        let size = stepper.intrinsicContentSize
        XCTAssertEqual(stepper.frame.size, size)
        XCTAssertEqual(stepper.frame.maxX, 294, "its trailing edge is where the text ends")
        XCTAssertEqual(stepper.frame.midY, node.bounds.midY, accuracy: 0.5, "centered in the input's box")
        let text = field.textRect(forBounds: field.bounds)
        XCTAssertEqual(text.minX, 6)
        XCTAssertEqual(text.maxX, stepper.frame.minX - NodeView.stepperGap, accuracy: 0.001, "the text ends a gap short of the stepper")
        XCTAssertEqual(field.editingRect(forBounds: field.bounds), text)
        XCTAssertEqual(field.placeholderRect(forBounds: field.bounds), text)
        XCTAssertEqual(field.keyboardType, .decimalPad, "direct entry works as before")
        XCTAssertEqual(field.text, "3")
        XCTAssertEqual(stepper.accessibilityLabel, "Quantity", "the stepper is named as the field is")
        XCTAssertEqual(field.accessibilityLabel, "Quantity")
        XCTAssertEqual(stepper.accessibilityIdentifier, "qty-stepper")
        XCTAssertFalse(stepper.isHidden)
    }

    func testNoStepperWhereTheAuthorOrTheTypeSaysNone() throws {
        let (node, _, session) = fixture()
        defer { session.destroy() }
        let field = try XCTUnwrap(node.field)
        let full = field.textRect(forBounds: field.bounds)
        XCTAssertNotNil(node.stepper)
        // `appearance="textfield"`: the bare field, text to the trailing edge.
        node.applyStyle(["appearance": .string("textfield")])
        XCTAssertNil(node.stepper)
        XCTAssertFalse(node.subviews.contains { $0 is UIStepper })
        XCTAssertEqual(field.textRect(forBounds: field.bounds).maxX, full.maxX + node.stepperRoomForTests, accuracy: 0.001)
        XCTAssertEqual(field.borderStyle, .roundedRect, "still the platform's field")
        node.applyStyle(["font_size": .number(17)])
        XCTAssertNotNil(node.stepper, "back to the default, it has one again")
        // `appearance="none"`: the author's own box.
        node.applyStyle(["appearance": .string("none")])
        XCTAssertNil(node.stepper)
        node.applyStyle(["font_size": .number(17)])
        XCTAssertNotNil(node.stepper)
        // Nothing is inferred from the bounds or the keyboard: another type has none.
        for props in [["type": "text"], ["type": "text", "inputMode": "numeric"], ["type": "text", "inputMode": "decimal"], ["type": "tel"], ["type": "email"], ["type": "password"], ["type": "search"]] {
            node.applyProps(set: props, clear: [])
            XCTAssertNil(node.stepper, "\(props)")
            XCTAssertEqual(field.textRect(forBounds: field.bounds).maxX, full.maxX + node.stepperRoomForTests, accuracy: 0.001, "\(props)")
        }
        node.applyProps(set: ["type": "number"], clear: ["inputMode"])
        XCTAssertNotNil(node.stepper)
        // A plain input has none, whatever its bounds.
        let plain = NodeView(id: 2, kind: "input", presenter: node.presenter!)
        plain.bounds = node.bounds
        plain.applyProps(set: ["min": "1", "max": "99", "step": "1", "value": "3"], clear: [])
        plain.applyFieldContent(["rect": [6.0, 5.0, 288.0, 34.0]])
        XCTAssertNil(plain.stepper)
    }

    func testSteppingSetsTheTextWithinTheBoundsAndSaysInputThenChange() throws {
        let (node, heard, session) = fixture()
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        let field = try XCTUnwrap(node.field)
        XCTAssertEqual(stepper.actions(forTarget: node, forControlEvent: .valueChanged), ["stepperChanged:"], "UIKit's own action on the stepper")
        press(node, up: true)
        XCTAssertEqual(field.text, "4")
        XCTAssertEqual(heard(), ["input 4", "change 4"])
        XCTAssertEqual(stepper.value, 4)
        press(node, up: false)
        press(node, up: false)
        XCTAssertEqual(field.text, "2")
        XCTAssertEqual(heard().suffix(4), ["input 3", "change 3", "input 2", "change 2"])
        press(node, up: false)
        XCTAssertEqual(field.text, "1")
        XCTAssertEqual(stepper.value, stepper.minimumValue)
        // At `min`, nothing steps below it and nothing is said.
        let count = heard().count
        press(node, up: false)
        XCTAssertEqual(field.text, "1")
        XCTAssertEqual(heard().count, count)
        // Typing carries the stepper to the typed number; the top bounds it.
        field.text = "98"
        node.fieldChanged()
        XCTAssertEqual(stepper.value, 98)
        press(node, up: true)
        XCTAssertEqual(field.text, "99")
        XCTAssertEqual(stepper.value, stepper.maximumValue)
        let top = heard().count
        press(node, up: true)
        XCTAssertEqual(field.text, "99")
        XCTAssertEqual(heard().count, top)
        // A value above `max` steps down to it, and up does nothing.
        field.text = "150"
        node.fieldChanged()
        press(node, up: true)
        XCTAssertEqual(field.text, "150")
        press(node, up: false)
        XCTAssertEqual(field.text, "99")
    }

    func testAnEmptyOrNonNumberFieldStepsFromZeroAndAWholeStepGivesAWholeNumber() throws {
        let (node, heard, session) = fixture(["value": ""])
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        let field = try XCTUnwrap(node.field)
        press(node, up: false)
        XCTAssertEqual(field.text, "", "from 0, stepping down is below min 1: nothing happens, as the web's")
        XCTAssertEqual(heard(), [])
        press(node, up: true)
        XCTAssertEqual(field.text, "1")
        node.applyProps(set: ["min": "0", "max": "10", "step": "0.5", "value": "1.5"], clear: [])
        XCTAssertEqual(field.text, "1.5")
        XCTAssertEqual(stepper.stepValue, 0.5)
        press(node, up: true)
        XCTAssertEqual(field.text, "2", "a whole step gives a whole number, never 2.0")
        press(node, up: true)
        XCTAssertEqual(field.text, "2.5")
        XCTAssertEqual(heard().suffix(2), ["input 2.5", "change 2.5"])
        // No bounds written: none, as the web's.
        node.applyProps(set: ["value": "-3"], clear: ["min", "max", "step"])
        press(node, up: false)
        XCTAssertEqual(field.text, "-4")
    }

    func testNoHandlersMeansNoEventsButTheTextStillSteps() throws {
        let (node, heard, session) = fixture(handlers: [])
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        press(node, up: true)
        XCTAssertEqual(node.field?.text, "4")
        XCTAssertEqual(heard(), [])
        node.handlers = ["change"]
        press(node, up: true)
        XCTAssertEqual(heard(), ["change 5"])
    }

    func testTheAppsValueMovesTheStepperAndADisabledOrReadonlyFieldStepsNot() throws {
        let (node, heard, session) = fixture()
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        node.applyProps(set: ["value": "42"], clear: [])
        XCTAssertEqual(node.field?.text, "42")
        XCTAssertEqual(stepper.value, 42)
        node.applyProps(set: ["disabled": "true"], clear: [])
        XCTAssertFalse(stepper.isEnabled)
        node.applyProps(set: ["editable": "false"], clear: ["disabled"])
        XCTAssertFalse(stepper.isEnabled, "a readonly field's spin buttons do nothing")
        press(node, up: true)
        XCTAssertEqual(node.field?.text, "42")
        XCTAssertEqual(heard(), [])
        node.applyProps(set: [:], clear: ["editable"])
        XCTAssertTrue(stepper.isEnabled)
    }

    func testTheFieldKeepsPriorityInABoxTooNarrowForBoth() throws {
        let (node, _, session) = fixture(width: 120)
        defer { session.destroy() }
        let field = try XCTUnwrap(node.field)
        let stepper = try XCTUnwrap(node.stepper)
        XCTAssertTrue(stepper.isHidden, "less than 44 pt for text beside the stepper: no stepper")
        XCTAssertEqual(field.textRect(forBounds: field.bounds).maxX, 114, "the text keeps the field")
        node.bounds = CGRect(x: 0, y: 0, width: 200, height: 44)
        node.applyFieldContent(["rect": [6.0, 5.0, 188.0, 34.0]])
        XCTAssertFalse(stepper.isHidden)
        XCTAssertLessThan(field.textRect(forBounds: field.bounds).maxX, 114)
    }

    func testAStepperInAGroupedRowsBorderlessFieldSitsAtTheCellsEdge() throws {
        let (node, _, session) = fixture(width: 338, rect: [0, 0, 338, 22])
        defer { session.destroy() }
        node.bounds = CGRect(x: 0, y: 0, width: 338, height: 22)
        node.applyFieldContent(["rect": [0.0, 0.0, 338.0, 22.0], "cell": true])
        let stepper = try XCTUnwrap(node.stepper)
        XCTAssertEqual(node.field?.borderStyle, UITextField.BorderStyle.none)
        XCTAssertEqual(stepper.frame.maxX, 338)
        XCTAssertEqual(stepper.frame.midY, 11, accuracy: 0.5, "centered on the line, in a row that is taller")
    }

    func testRightToLeftPutsTheStepperAtTheLeadingEdge() throws {
        let (node, _, session) = fixture()
        defer { session.destroy() }
        node.applyStyle(["direction": .string("rtl")])
        let stepper = try XCTUnwrap(node.stepper)
        let field = try XCTUnwrap(node.field)
        XCTAssertEqual(stepper.frame.minX, 6)
        XCTAssertEqual(field.textRect(forBounds: field.bounds).minX, 6 + stepper.frame.width + NodeView.stepperGap, accuracy: 0.001)
        XCTAssertEqual(field.textRect(forBounds: field.bounds).maxX, 294)
    }
}

private extension NodeView {
    /// What the stepper takes of the text at its end, as the tests reckon
    /// the bare field's rectangle back: the stepper's width and gap, for a
    /// view that still had one a moment ago.
    var stepperRoomForTests: CGFloat { UIStepper().intrinsicContentSize.width + NodeView.stepperGap }
}
#endif

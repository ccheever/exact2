// @ref LLP 1116.007 D4: `input type="number"` is a text field with an NSStepper
// beside it, stepping the field's text by HTML's rules and saying so as
// `input` then `change`; `appearance="textfield"`, any other `type` and a
// field too narrow for both leave the bare field.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class NumberStepperMacTests: XCTestCase {
    /// A number input in a session, 300×28, its text starting at `rect`'s edges as the kernel publishes it.
    private func fixture(_ props: [String: String] = [:], width: CGFloat = 300, handlers: Set<String> = ["input", "change"]) -> (node: NodeView, heard: () -> [String], keep: ExactSession) {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "number-stepper")
        let p = session.presenter
        let node = NodeView(id: 1, kind: "input", presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: width, height: 28)
        node.handlers = handlers
        p.views[node.id] = node
        var heard: [String] = []
        p.onInput = { _, value in heard.append("input \(value)") }
        p.onChange = { _, value in heard.append("change \(value)") }
        node.applyProps(set: ["type": "number", "min": "1", "max": "99", "step": "1", "value": "3", "accessibilityLabel": "Quantity", "testId": "qty"].merging(props) { _, new in new }, clear: [])
        node.applyStyle(["font_size": .number(13)])
        node.applyFieldContent(["rect": [4.0, 3.0, Double(width) - 8, 22.0]])
        return (node, { heard }, session)
    }

    /// One click of the up or the down arrow: the stepper's value moves a step and it sends its action.
    private func press(_ stepper: NSStepper, up: Bool) {
        stepper.doubleValue += up ? stepper.increment : -stepper.increment
        _ = stepper.sendAction(stepper.action, to: stepper.target)
    }

    func testANumberInputHasAnNSStepperBesideItsFieldAtTheTrailingEdge() throws {
        let (node, _, session) = fixture()
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(stepper.superview === node)
        XCTAssertEqual(stepper.minValue, 1)
        XCTAssertEqual(stepper.maxValue, 99)
        XCTAssertEqual(stepper.increment, 1)
        XCTAssertEqual(stepper.doubleValue, 3, "it shows what the field holds")
        let size = stepper.fittingSize
        XCTAssertEqual(stepper.frame.size, size)
        XCTAssertEqual(stepper.frame.maxX, node.bounds.maxX, "at the input's trailing edge")
        XCTAssertEqual(stepper.frame.midY, node.bounds.midY, accuracy: 0.5, "centered in the input's box")
        XCTAssertEqual(field.frame.minX, 0)
        XCTAssertEqual(field.frame.maxX, stepper.frame.minX - NodeView.stepperGap, accuracy: 0.001, "the field is narrower by the stepper and a gap")
        XCTAssertTrue(field.isBezeled, "still the platform's field")
        XCTAssertEqual(field.stringValue, "3")
        XCTAssertEqual(stepper.accessibilityLabel(), "Quantity")
        XCTAssertEqual(stepper.accessibilityIdentifier(), "qty-stepper")
        XCTAssertFalse(stepper.isHidden)
    }

    func testNoStepperWhereTheAuthorOrTheTypeSaysNone() throws {
        let (node, _, session) = fixture()
        defer { session.destroy() }
        let field = try XCTUnwrap(node.field)
        XCTAssertNotNil(node.stepper)
        node.applyStyle(["appearance": .string("textfield")])
        XCTAssertNil(node.stepper)
        XCTAssertFalse(node.subviews.contains { $0 is NSStepper })
        XCTAssertEqual(field.frame, node.bounds, "the bare field, as wide as the input")
        XCTAssertTrue(field.isBezeled, "still the platform's field")
        node.applyStyle(["font_size": .number(13)])
        XCTAssertNotNil(node.stepper, "back to the default, it has one again")
        node.applyStyle(["appearance": .string("none")])
        XCTAssertNil(node.stepper)
        node.applyStyle(["font_size": .number(13)])
        XCTAssertNotNil(node.stepper)
        for props in [["type": "text"], ["type": "text", "inputMode": "numeric"], ["type": "text", "inputMode": "decimal"], ["type": "tel"], ["type": "email"], ["type": "password"], ["type": "search"]] {
            node.applyProps(set: props, clear: [])
            XCTAssertNil(node.stepper, "\(props)")
            XCTAssertEqual(node.field?.frame, node.bounds, "\(props)")
        }
        node.applyProps(set: ["type": "number"], clear: ["inputMode"])
        XCTAssertNotNil(node.stepper)
        let plain = NodeView(id: 2, kind: "input", presenter: node.presenter!)
        plain.frame = node.frame
        plain.applyProps(set: ["min": "1", "max": "99", "step": "1", "value": "3"], clear: [])
        plain.applyFieldContent(["rect": [4.0, 3.0, 292.0, 22.0]])
        XCTAssertNil(plain.stepper)
    }

    func testSteppingSetsTheTextWithinTheBoundsAndSaysInputThenChange() throws {
        let (node, heard, session) = fixture()
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        let field = try XCTUnwrap(node.field)
        press(stepper, up: true)
        XCTAssertEqual(field.stringValue, "4")
        XCTAssertEqual(heard(), ["input 4", "change 4"])
        XCTAssertEqual(stepper.doubleValue, 4)
        press(stepper, up: false)
        press(stepper, up: false)
        press(stepper, up: false)
        XCTAssertEqual(field.stringValue, "1")
        XCTAssertEqual(stepper.doubleValue, 1)
        XCTAssertEqual(heard().suffix(2), ["input 1", "change 1"])
        let count = heard().count
        press(stepper, up: false)
        XCTAssertEqual(field.stringValue, "1", "nothing steps below min")
        XCTAssertEqual(heard().count, count)
        field.stringValue = "98"
        node.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification, object: field))
        XCTAssertEqual(stepper.doubleValue, 98, "typing carries the stepper to the typed number")
        press(stepper, up: true)
        XCTAssertEqual(field.stringValue, "99")
        let top = heard().count
        press(stepper, up: true)
        XCTAssertEqual(field.stringValue, "99")
        XCTAssertEqual(heard().count, top)
        field.stringValue = "150"
        node.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification, object: field))
        press(stepper, up: false)
        XCTAssertEqual(field.stringValue, "99", "above max, down returns to it")
    }

    func testAWholeStepGivesAWholeNumberAndAnEmptyFieldStepsFromZero() throws {
        let (node, heard, session) = fixture(["value": ""])
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        let field = try XCTUnwrap(node.field)
        press(stepper, up: true)
        XCTAssertEqual(field.stringValue, "1")
        node.applyProps(set: ["min": "0", "max": "10", "step": "0.5", "value": "1.5"], clear: [])
        XCTAssertEqual(stepper.increment, 0.5)
        press(stepper, up: true)
        XCTAssertEqual(field.stringValue, "2", "never 2.0")
        press(stepper, up: true)
        XCTAssertEqual(field.stringValue, "2.5")
        XCTAssertEqual(heard().suffix(2), ["input 2.5", "change 2.5"])
    }

    func testTheFieldEditorFollowsAStepWhileTheFieldIsBeingEdited() throws {
        let (node, heard, session) = fixture()
        defer { session.destroy() }
        let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = node
        let field = try XCTUnwrap(node.field)
        let stepper = try XCTUnwrap(node.stepper)
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        press(stepper, up: true)
        XCTAssertEqual(editor.string, "4")
        XCTAssertEqual(heard(), ["input 4", "change 4"])
    }

    /// Typing into the field, as AppKit reports it: the text changes and says so.
    private func type(_ node: NodeView, _ text: String) throws {
        let field = try XCTUnwrap(node.field)
        field.stringValue = text
        node.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification, object: field))
    }

    /// LLP 1116.008 D5 (grocery-list, round 10): a typed 250 against a `max` of
    /// 99 saved 250 while the stepper's up arrow was dimmed. The edit commits as
    /// the bound it passed, written into the field and told as a step is:
    /// `input`, then the `change`. What was typed while typing is unchanged.
    func testATypedValuePastABoundCommitsAsTheBoundAndTheFieldShowsIt() throws {
        let (node, heard, session) = fixture()
        defer { session.destroy() }
        let field = try XCTUnwrap(node.field), stepper = try XCTUnwrap(node.stepper)
        try type(node, "250")
        XCTAssertEqual(heard(), ["input 250"], "typing is as it was")
        node.controlTextDidEndEditing(Notification(name: NSControl.textDidEndEditingNotification, object: field))
        XCTAssertEqual(heard(), ["input 250", "input 99", "change 99"])
        XCTAssertEqual(field.stringValue, "99")
        XCTAssertEqual(stepper.doubleValue, 99, "the stepper shows it")
        try type(node, "-4")
        node.controlTextDidEndEditing(Notification(name: NSControl.textDidEndEditingNotification, object: field))
        XCTAssertEqual(heard().suffix(3), ["input -4", "input 1", "change 1"])
        XCTAssertEqual(field.stringValue, "1")
        XCTAssertEqual(stepper.doubleValue, 1)
        // An edit within the bounds, and text that is no number, are as they were.
        try type(node, "42")
        node.controlTextDidEndEditing(Notification(name: NSControl.textDidEndEditingNotification, object: field))
        XCTAssertEqual(field.stringValue, "42")
        XCTAssertEqual(heard().suffix(2), ["input 42", "change 42"])
        try type(node, "abc")
        node.controlTextDidEndEditing(Notification(name: NSControl.textDidEndEditingNotification, object: field))
        XCTAssertEqual(field.stringValue, "abc")
        XCTAssertEqual(heard().suffix(2), ["input abc", "change abc"])
    }

    /// Return commits in the field editor, which shows the bound as it does a step.
    func testReturnCommitsTheBoundIntoTheFieldEditor() throws {
        let (node, heard, session) = fixture()
        defer { session.destroy() }
        let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = node
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        editor.string = "250"
        try type(node, "250")
        XCTAssertTrue(node.control(field, textView: editor, doCommandBy: #selector(NSResponder.insertNewline(_:))))
        XCTAssertEqual(editor.string, "99")
        XCTAssertEqual(heard(), ["input 250", "input 99", "change 99"])
        XCTAssertEqual(try XCTUnwrap(node.stepper).doubleValue, 99)
    }

    func testOnlyAFieldWithAStepperHoldsItsBoundsAndTheHandlersItHasHearIt() throws {
        let end = { (node: NodeView) in node.controlTextDidEndEditing(Notification(name: NSControl.textDidEndEditingNotification, object: node.field)) }
        // `appearance="textfield"`: the bare field, whose bounds are the browser's validation.
        let (bare, bareHeard, bareSession) = fixture()
        defer { bareSession.destroy() }
        bare.applyStyle(["appearance": .string("textfield")])
        XCTAssertNil(bare.stepper)
        try type(bare, "250"); end(bare)
        XCTAssertEqual(bare.field?.stringValue, "250")
        XCTAssertEqual(bareHeard(), ["input 250", "change 250"])
        let (text, textHeard, textSession) = fixture(["type": "text"])
        defer { textSession.destroy() }
        try type(text, "250"); end(text)
        XCTAssertEqual(text.field?.stringValue, "250")
        XCTAssertEqual(textHeard(), ["input 250", "change 250"])
        let (change, changeHeard, changeSession) = fixture(handlers: ["change"])
        defer { changeSession.destroy() }
        try type(change, "250"); end(change)
        XCTAssertEqual(changeHeard(), ["change 99"])
        let (input, inputHeard, inputSession) = fixture(handlers: ["input"])
        defer { inputSession.destroy() }
        try type(input, "250"); end(input)
        XCTAssertEqual(inputHeard(), ["input 250", "input 99"])
        XCTAssertEqual(input.field?.stringValue, "99")
    }

    func testNoHandlersMeansNoEventsAndADisabledOrReadonlyFieldStepsNot() throws {
        let (node, heard, session) = fixture(handlers: [])
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        press(stepper, up: true)
        XCTAssertEqual(node.field?.stringValue, "4")
        XCTAssertEqual(heard(), [])
        node.applyProps(set: ["disabled": "true", "value": "4"], clear: [])
        XCTAssertFalse(stepper.isEnabled)
        node.applyProps(set: ["editable": "false"], clear: ["disabled"])
        XCTAssertFalse(stepper.isEnabled)
        node.handlers = ["input", "change"]
        press(stepper, up: true)
        XCTAssertEqual(node.field?.stringValue, "4")
        XCTAssertEqual(heard(), [])
    }

    func testTheFieldKeepsPriorityInABoxTooNarrowForBoth() throws {
        let (node, _, session) = fixture(width: 60)
        defer { session.destroy() }
        let stepper = try XCTUnwrap(node.stepper)
        XCTAssertTrue(stepper.isHidden, "less than 44 pt for the field beside the stepper: no stepper")
        XCTAssertEqual(node.field?.frame, node.bounds)
        node.frame = NSRect(x: 0, y: 0, width: 200, height: 28)
        node.applyFieldContent(["rect": [4.0, 3.0, 192.0, 22.0]])
        XCTAssertFalse(stepper.isHidden)
        XCTAssertLessThan(try XCTUnwrap(node.field).frame.width, 200)
    }
}
#endif

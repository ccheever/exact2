#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The number pads' bar (LLP 1116.003 D7): the decimal pad has no minus key
/// and no Return key, so a field on it gets, in the keyboard's accessory bar,
/// ± where its `min` is below zero, which flips the sign and fires `input` as
/// a keystroke does (round 5's unit converter), and Done where Return would
/// act (`enterkeyhint="done"`, a `submit` handler), which does what Return
/// does and puts the keyboard away. A field that asks for neither, or a
/// keyboard that has both, gets no bar.
final class PadBarIOSTests: XCTestCase {
    private func field(_ props: [String: String], handlers: Set<String> = ["input"]) -> (Presenter, NodeView, UIWindow) {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        let node = NodeView(id: 1, kind: "input", presenter: p)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 44)
        node.handlers = handlers
        window.addSubview(node); p.views[node.id] = node
        window.makeKeyAndVisible()
        node.applyProps(set: props, clear: [])
        return (p, node, window)
    }
    private func keys(_ node: NodeView) -> [String] {
        (node.field?.inputAccessoryView as? UIToolbar)?.items?.compactMap { $0.action.map { NSStringFromSelector($0) } } ?? []
    }

    func testANegativeMinGetsAPlusMinusKeyThatFlipsTheSignAsInput() throws {
        let (p, node, window) = field(["inputMode": "decimal", "min": "-40", "value": "12.5"])
        defer { window.isHidden = true }
        var heard: [String] = []
        p.onInput = { [unowned node] _, value in
            heard.append(value)
            node.applyProps(set: ["value": value], clear: [])
        }
        let field = try XCTUnwrap(node.field)
        XCTAssertEqual(field.keyboardType, .decimalPad)
        XCTAssertEqual(keys(node), ["toggleSign"], "± alone: nothing asks for Done")
        let sign = try XCTUnwrap((field.inputAccessoryView as? UIToolbar)?.items?.first)
        XCTAssertEqual(sign.title, "\u{00B1}")
        XCTAssertTrue(sign.target === node, "the key is the field's node's")
        node.toggleSign()
        XCTAssertEqual(heard.last, "-12.5", "the app hears the flipped value as input")
        XCTAssertEqual(field.text, "-12.5")
        node.toggleSign()
        XCTAssertEqual(heard.last, "12.5")
        field.text = ""
        node.toggleSign()
        XCTAssertEqual(heard.last, "-", "an empty field starts a negative number")
        // The bar follows the props: it is the field's only while its keyboard lacks a key it asks for.
        node.applyProps(set: ["min": "0"], clear: [])
        XCTAssertNil(field.inputAccessoryView, "min 0: no sign, no bar")
        node.applyProps(set: [:], clear: ["min"])
        XCTAssertNil(field.inputAccessoryView, "no min: no bar")
        node.applyProps(set: ["inputMode": "text", "min": "-1"], clear: [])
        XCTAssertNil(field.inputAccessoryView, "a text keyboard has its minus and its Return")
        node.applyProps(set: ["type": "number", "min": "-273.15"], clear: ["inputMode"])
        XCTAssertEqual(field.keyboardType, .decimalPad)
        XCTAssertEqual(keys(node), ["toggleSign"], "type=number with a negative min, as inputmode=decimal")
    }

    func testDoneIsThereWhereReturnWouldActAndDoesWhatReturnDoes() throws {
        // number-field's converter: a negative min and enterkeyhint="done", ± and Done.
        let (p, node, window) = field(["inputMode": "decimal", "min": "-273.15", "enterKeyHint": "done", "value": "-40"], handlers: ["input", "change"])
        defer { window.isHidden = true }
        XCTAssertEqual(keys(node), ["toggleSign", "padDone"])
        var committed: [String] = []
        p.onChange = { _, value in committed.append(value) }
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field.becomeFirstResponder())
        node.toggleSign() // an edit, as a keystroke's
        node.padDone()
        XCTAssertEqual(committed, ["40"], "Done commits the edit, as Return does")
        XCTAssertFalse(field.isFirstResponder, "and puts the keyboard away")
        // Done alone, without a sign: enterkeyhint="done" on a pad, or a submit handler.
        node.applyProps(set: [:], clear: ["min"])
        XCTAssertEqual(keys(node), ["padDone"])
        node.applyProps(set: [:], clear: ["enterKeyHint"])
        XCTAssertNil(field.inputAccessoryView, "no hint and no submit: no permanent Done")
        node.handlers = ["input", "submit"]
        var submitted = 0
        p.onSubmit = { _ in submitted += 1 }
        XCTAssertTrue(node.textFieldShouldBeginEditing(field), "the bar is made as editing begins")
        XCTAssertEqual(keys(node), ["padDone"], "a submit handler: Return would submit, so Done is there")
        node.padDone()
        XCTAssertEqual(submitted, 1, "and Done submits")
    }
}
#endif

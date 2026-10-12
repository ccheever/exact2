#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A field and a text area composing through an input method (LLP 1115.000
/// §5, ruling 6; LLP 1115.001 D8; `FieldComposition.swift`), as on the Mac
/// (`FieldCompositionMacTests`): the app hears each change of the marked
/// text, with it in the value; the commit of what is marked as it stands is
/// not heard again; the value the editor shows leaves a composition alone
/// and a different one ends it. Driven through `UITextInput`
/// (`setMarkedText`, `unmarkText`). A text view tells its delegate of each
/// of those itself; a field sends its control event only for a keyboard's,
/// so the tests send that by hand. Nobody typed with a real keyboard.
/// UIKit, so a simulator runs it:
///   EXACT_TESTS=1 EXACT_LIB_DIR=<target>/aarch64-apple-ios-sim/release EXACT_LIB=caltrain_apple \
///   xcodebuild test -scheme Exact -destination 'platform=iOS Simulator,name=<iPhone>' \
///     -only-testing:ExactKitTests/FieldCompositionIOSTests
final class FieldCompositionIOSTests: XCTestCase {
    func testInputsApplyPhysicalAndLogicalAlignmentAfterRestyling() throws {
        let session = ExactApp.shared.makeSession(label: "input-alignment")
        defer { session.destroy() }
        let p = session.presenter
        for kind in ["input", "textarea"] {
            let node = NodeView(id: 1, kind: kind, presenter: p)
            node.frame = CGRect(x: 0, y: 0, width: 300, height: 44)
            node.applyProps(set: ["value": "hello"], clear: [])
            for (alignment, direction, expected) in [
                ("center", "ltr", NSTextAlignment.center), ("right", "ltr", .right),
                ("start", "rtl", .right), ("end", "rtl", .left), ("left", "rtl", .left),
                ("start", "ltr", .left),
            ] {
                node.applyStyle(["text_align": .string(alignment), "direction": .string(direction), "line_height": "24px"])
                XCTAssertEqual(node.field?.textAlignment ?? node.textArea?.textAlignment, expected, "\(kind) \(alignment) \(direction)")
                if let editor = node.textArea {
                    let paragraph = try XCTUnwrap(editor.textStorage.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)
                    XCTAssertEqual(paragraph.alignment, expected)
                    XCTAssertEqual(paragraph.minimumLineHeight, 24)
                    XCTAssertEqual((editor.typingAttributes[.paragraphStyle] as? NSParagraphStyle)?.alignment, expected)
                }
            }
            node.applyStyle([:])
            XCTAssertEqual(node.field?.textAlignment ?? node.textArea?.textAlignment, .left)
        }
    }

    /// A node of `kind` in a key window with the focus in its editor, what
    /// its app hears, and UIKit's notice of a change. `echo`: a controlled
    /// input writes back what it hears, or what `echo` makes of it.
    private func fixture(_ kind: String, echo: @escaping (String) -> String = { $0 }) throws -> (node: NodeView, input: UITextInput & UIResponder, heard: () -> [String], changed: () -> Void, keep: [AnyObject]) {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        let node = NodeView(id: 1, kind: kind, presenter: p)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 80)
        node.handlers = ["input"]
        window.addSubview(node); p.views[node.id] = node
        window.makeKeyAndVisible()
        final class Heard { var inputs: [String] = [] }
        let heard = Heard()
        p.onInput = { [unowned node] _, value in
            heard.inputs.append(value)
            node.applyProps(set: ["value": echo(value)], clear: [])
        }
        let input: UITextInput & UIResponder = try XCTUnwrap(node.field ?? node.textArea)
        XCTAssertTrue(input.becomeFirstResponder())
        // A text view has told its delegate already (measured on iOS 27.0).
        let changed = { [unowned node] in if node.field != nil { node.fieldChanged() } }
        return (node, input, { heard.inputs }, changed, [p, window])
    }
    private func text(_ node: NodeView) -> String { node.field?.text ?? node.textArea?.text ?? "" }

    /// にほん, then 日本: each change of the marked text is an `input` with
    /// it in the value, and the app's echo of what it heard leaves the
    /// composition marked (it was held and written back at the commit).
    func testEachChangeOfMarkedTextIsAnInput() throws {
        for kind in ["input", "textarea"] {
            let f = try fixture(kind)
            defer { _ = f.keep }
            for kana in ["に", "にほ", "にほん"] {
                f.input.setMarkedText(kana, selectedRange: NSRange(location: (kana as NSString).length, length: 0))
                f.changed()
                XCTAssertEqual(f.heard().last, kana, "\(kind): heard as it is composed")
                XCTAssertNotNil(f.input.markedTextRange, "\(kind): the echo leaves it marked")
            }
            // The candidate replaces what was marked.
            f.input.setMarkedText("日本", selectedRange: NSRange(location: 2, length: 0))
            f.changed()
            XCTAssertEqual(f.heard(), ["に", "にほ", "にほん", "日本"], kind)
            // Committed as it stands: no text changes, and it is not heard again.
            f.input.unmarkText()
            f.changed()
            XCTAssertNil(f.input.markedTextRange)
            XCTAssertEqual(text(f.node), "日本", kind)
            XCTAssertEqual(f.heard(), ["に", "にほ", "にほん", "日本"], "\(kind): the commit is no second input")
            // A composition cancelled takes its text away, which is a change.
            f.input.setMarkedText("x", selectedRange: NSRange(location: 1, length: 0))
            f.changed()
            XCTAssertEqual(f.heard().last, "日本x", kind)
            f.input.setMarkedText("", selectedRange: NSRange(location: 0, length: 0))
            f.changed()
            XCTAssertEqual(f.heard().last, "日本", "\(kind): the cancelled text is gone from what the app holds")
            // The caret moving inside what is marked changes no text: no input.
            f.input.setMarkedText("かな", selectedRange: NSRange(location: 2, length: 0))
            f.changed()
            f.input.setMarkedText("かな", selectedRange: NSRange(location: 1, length: 0))
            f.changed()
            XCTAssertEqual(f.heard(), ["に", "にほ", "にほん", "日本", "日本x", "日本", "日本かな"], "\(kind): one input for each change")
            // A value from anywhere else (another action) lands on committed text.
            f.node.applyProps(set: ["value": "done"], clear: [])
            XCTAssertEqual(text(f.node), "done", "\(kind): a different value replaces the text")
            XCTAssertNil(f.input.markedTextRange, "\(kind): and ends the composition")
            XCTAssertEqual(f.heard().count, 7, "\(kind): the app's own write is no input: \(f.heard())")
        }
    }

    /// LLP 1115.001 D8's rule: an app commit that sets a different value
    /// while text is marked replaces the text and ends the composition (it
    /// was held until the composition ended), and the app hears no `input`
    /// for its own write.
    func testADifferentValueEndsTheComposition() throws {
        for kind in ["input", "textarea"] {
            // The app keeps three characters.
            let f = try fixture(kind) { String($0.prefix(3)) }
            defer { _ = f.keep }
            f.input.setMarkedText("にほ", selectedRange: NSRange(location: 2, length: 0))
            f.changed()
            XCTAssertNotNil(f.input.markedTextRange, "\(kind): within its limit the composition stands")
            f.input.setMarkedText("にほんご", selectedRange: NSRange(location: 4, length: 0))
            f.changed()
            XCTAssertEqual(f.heard(), ["にほ", "にほんご"], kind)
            XCTAssertEqual(text(f.node), "にほん", "\(kind): the app's value is the text")
            XCTAssertNil(f.input.markedTextRange, "\(kind): and the composition is over")
            XCTAssertEqual(f.heard(), ["にほ", "にほんご"], "\(kind): the app's own write is no input")
        }
    }

    /// #140, a hardware keyboard during a composition (macOS's
    /// `KeyUpMacTests` drive the same through AppKit): a modifier's own key
    /// passes it untouched, any other key is the input method's, and a ⌘
    /// chord's keydown commits it before its handlers run, without hearing
    /// the text twice. A composer's handler then clears the field, and the
    /// cleared value lands.
    func testACommandChordCommitsTheCompositionAndAModifierPassesIt() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        let node = NodeView(id: 1, kind: "input", presenter: p)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 44)
        node.handlers = ["input", "key", "keyup"]
        window.addSubview(node); p.views[node.id] = node
        window.makeKeyAndVisible()
        // A controlled input: the app echoes what it hears.
        var heard: [String] = []
        p.onInput = { [unowned node] _, value in
            heard.append(value)
            node.applyProps(set: ["value": value], clear: [])
        }
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field.becomeFirstResponder())
        field.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0))
        node.fieldChanged()
        XCTAssertEqual(heard, ["한"], "UIKit reports the composed text as it is composed")
        XCTAssertFalse(node.passesComposition("KeyA", command: false, down: true))
        XCTAssertFalse(node.passesComposition("KeyB", command: true, down: false), "a chord's keyup waits for its keydown's commit")
        XCTAssertTrue(node.passesComposition("ShiftLeft", command: false, down: false))
        XCTAssertNotNil(field.markedTextRange, "none of these commits it")
        XCTAssertTrue(node.passesComposition("Enter", command: true, down: true))
        XCTAssertNil(field.markedTextRange)
        XCTAssertEqual(field.text, "한")
        node.fieldChanged()
        XCTAssertEqual(heard, ["한"], "the commit is not heard again, should UIKit say the field changed")
        node.applyProps(set: ["value": ""], clear: [])
        XCTAssertEqual(field.text, "", "the composer's cleared value lands")
        XCTAssertTrue(node.passesComposition("KeyA", command: false, down: true), "no composition: every key")
    }
}
#endif

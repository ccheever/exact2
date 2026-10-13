#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A field and a text area composing through an input method (LLP 1115.000
/// §5, ruling 6; LLP 1045 D5; `FieldComposition.swift`), as on the Mac
/// (`FieldCompositionMacTests`): the app hears each change of the marked
/// text, with it in the value; the commit of what is marked as it stands is
/// not heard again; and the app's value never ends a composition: the value
/// the editor shows changes nothing, and a different one written while text
/// is marked is held and applied when the composition ends. Driven through
/// `UITextInput` (`setMarkedText`, `insertText`, `unmarkText`). A text view
/// tells its delegate of each of those itself; a field sends its control
/// event only for a keyboard's, so the tests send that by hand. Nobody
/// typed with a real keyboard.
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
    /// input writes back what it hears, or what `echo` makes of it; nil
    /// writes nothing.
    private func fixture(_ kind: String, value: String = "", echo: ((String) -> String)? = { $0 }) throws -> (node: NodeView, input: UITextInput & UIResponder, heard: () -> [String], changed: () -> Void, keep: [AnyObject]) {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        let node = NodeView(id: 1, kind: kind, presenter: p)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 80)
        node.handlers = ["input"]
        window.addSubview(node); p.views[node.id] = node
        window.makeKeyAndVisible()
        node.applyProps(set: ["value": value], clear: [])
        final class Heard { var inputs: [String] = [] }
        let heard = Heard()
        p.onInput = { [unowned node] _, value in
            heard.inputs.append(value)
            if let echo { node.applyProps(set: ["value": echo(value)], clear: []) }
        }
        let input: UITextInput & UIResponder = try XCTUnwrap(node.field ?? node.textArea)
        XCTAssertTrue(input.becomeFirstResponder())
        input.selectedTextRange = input.textRange(from: input.endOfDocument, to: input.endOfDocument)
        // A text view has told its delegate already (measured on iOS 27.0).
        let changed = { [unowned node] in if node.field != nil { node.fieldChanged() } }
        return (node, input, { heard.inputs }, changed, [p, window])
    }
    private func text(_ node: NodeView) -> String { node.field?.text ?? node.textArea?.text ?? "" }
    private typealias Fixture = (node: NodeView, input: UITextInput & UIResponder, heard: () -> [String], changed: () -> Void, keep: [AnyObject])
    private func mark(_ f: Fixture, _ text: String) {
        f.input.setMarkedText(text, selectedRange: NSRange(location: (text as NSString).length, length: 0))
        f.changed()
    }
    /// The candidate chosen: the keyboard inserts it over what was marked.
    private func choose(_ f: Fixture, _ text: String) {
        f.input.insertText(text)
        f.changed()
    }
    private func commit(_ f: Fixture) {
        f.input.unmarkText()
        f.changed()
    }
    private func cancel(_ f: Fixture) {
        f.input.setMarkedText("", selectedRange: NSRange(location: 0, length: 0))
        f.changed()
    }

    /// にほん, then 日本: each change of the marked text is an `input` with
    /// it in the value, and the app's echo of what it heard leaves the
    /// composition marked and is not held.
    func testEachChangeOfMarkedTextIsAnInput() throws {
        for kind in ["input", "textarea"] {
            let f = try fixture(kind)
            defer { _ = f.keep }
            for kana in ["に", "にほ", "にほん"] {
                mark(f, kana)
                XCTAssertEqual(f.heard().last, kana, "\(kind): heard as it is composed")
                XCTAssertNotNil(f.input.markedTextRange, "\(kind): the echo leaves it marked")
                XCTAssertNil(f.node.pendingValue, "\(kind): the echo is the text: nothing is held")
            }
            choose(f, "日本")
            XCTAssertNil(f.input.markedTextRange)
            XCTAssertEqual(text(f.node), "日本", "\(kind): the candidate replaces what was marked")
            XCTAssertEqual(f.heard(), ["に", "にほ", "にほん", "日本"], kind)
            // Committed as it stands: no text changes, and it is not heard again.
            mark(f, "ご")
            commit(f)
            XCTAssertNil(f.input.markedTextRange)
            XCTAssertEqual(text(f.node), "日本ご", kind)
            XCTAssertEqual(f.heard(), ["に", "にほ", "にほん", "日本", "日本ご"], "\(kind): the commit is no second input")
            // A composition cancelled takes its text away, which is a change.
            mark(f, "x")
            XCTAssertEqual(f.heard().last, "日本ごx", kind)
            cancel(f)
            XCTAssertEqual(f.heard().last, "日本ご", "\(kind): the cancelled text is gone from what the app holds")
            // The caret moving inside what is marked changes no text: no input.
            mark(f, "かな")
            f.input.setMarkedText("かな", selectedRange: NSRange(location: 1, length: 0))
            f.changed()
            XCTAssertEqual(f.heard(), ["に", "にほ", "にほん", "日本", "日本ご", "日本ごx", "日本ご", "日本ごかな"], "\(kind): one input for each change")
        }
    }

    /// LLP 1045 D5: an action that rewrites what it hears never ends a
    /// composition. Here it upper-cases: each change of にほん is heard, each
    /// rewrite is held while the text stays marked, and the rewrite lands
    /// when the composition ends, by the candidate chosen or as it stands.
    func testAnActionThatRewritesWhatItHearsWaitsForTheCompositionToEnd() throws {
        for kind in ["input", "textarea"] {
            var f = try fixture(kind, value: "ab", echo: { $0.uppercased() })
            for kana in ["に", "にほ", "にほん"] {
                mark(f, kana)
                XCTAssertEqual(text(f.node), "ab" + kana, "\(kind): the text is the person's while it is marked")
                XCTAssertNotNil(f.input.markedTextRange, "\(kind): the app's value does not end the composition")
                XCTAssertEqual(f.node.pendingValue, "AB" + kana, "\(kind): it is held, the last one written")
            }
            XCTAssertEqual(f.heard(), ["abに", "abにほ", "abにほん"], "\(kind): the app heard each change")
            choose(f, "日本")
            XCTAssertEqual(f.heard(), ["abに", "abにほ", "abにほん", "ab日本"], "\(kind): and the commit, which changed the text")
            XCTAssertEqual(text(f.node), "AB日本", "\(kind): its rewrite of that lands at once, nothing being marked")
            XCTAssertNil(f.input.markedTextRange)
            XCTAssertNil(f.node.pendingValue, "\(kind): and nothing older is left to land")

            // Committed as it stands: nothing more is heard, and what was held lands.
            f = try fixture(kind, value: "ab", echo: { $0.uppercased() })
            mark(f, "にほ"); mark(f, "にほん")
            XCTAssertEqual(f.node.pendingValue, "ABにほん", kind)
            commit(f)
            XCTAssertEqual(f.heard(), ["abにほ", "abにほん"], "\(kind): the commit changed no text")
            XCTAssertEqual(text(f.node), "ABにほん", "\(kind): the held value is applied when the composition ends")
            XCTAssertNil(f.node.pendingValue, kind)
            XCTAssertNil(f.input.markedTextRange)
        }
    }

    /// The same when the composition is cancelled: the app hears the text
    /// without what was marked, and what it wrote lands.
    func testAHeldValueLandsWhenTheCompositionIsCancelled() throws {
        for kind in ["input", "textarea"] {
            var f = try fixture(kind, value: "ab", echo: { $0.uppercased() })
            mark(f, "に"); mark(f, "にほ")
            XCTAssertEqual(f.node.pendingValue, "ABにほ", kind)
            XCTAssertNotNil(f.input.markedTextRange)
            cancel(f)
            XCTAssertEqual(f.heard(), ["abに", "abにほ", "ab"], "\(kind): the cancel is a change of text")
            XCTAssertEqual(text(f.node), "AB", "\(kind): and the app's value for it lands")
            XCTAssertNil(f.node.pendingValue, kind)

            // A value from another action while text is marked, and an app that answers nothing to the cancel.
            f = try fixture(kind, echo: nil)
            mark(f, "に")
            f.node.applyProps(set: ["value": "done"], clear: [])
            XCTAssertEqual(text(f.node), "に", "\(kind): held")
            XCTAssertNotNil(f.input.markedTextRange, "\(kind): the composition stands")
            mark(f, "にほ")
            XCTAssertEqual(f.node.pendingValue, "done", "\(kind): still held as the composition goes on")
            cancel(f)
            XCTAssertEqual(f.heard(), ["に", "にほ", ""], kind)
            XCTAssertEqual(text(f.node), "done", "\(kind): applied at the cancel")
            XCTAssertNil(f.node.pendingValue, kind)
        }
    }

    /// An action that clears the field on one value: the clear waits for
    /// the composition, a later echo of the text takes its place with
    /// nothing, and committed at that value the field is cleared.
    func testAnActionThatClearsOnAValueWaitsToo() throws {
        let clears = { (value: String) in value == "にほ" ? "" : value }
        for kind in ["input", "textarea"] {
            var f = try fixture(kind, echo: clears)
            mark(f, "に"); mark(f, "にほ")
            XCTAssertEqual(f.node.pendingValue, "", "\(kind): the clear is held")
            XCTAssertNotNil(f.input.markedTextRange, "\(kind): and the composition goes on")
            mark(f, "にほん")
            XCTAssertNil(f.node.pendingValue, "\(kind): the app's value is the text again: nothing is owed")
            choose(f, "日本")
            XCTAssertEqual(text(f.node), "日本", "\(kind): no stale clear lands at the commit")
            XCTAssertEqual(f.heard(), ["に", "にほ", "にほん", "日本"], kind)

            f = try fixture(kind, echo: clears)
            mark(f, "にほ")
            commit(f)
            XCTAssertEqual(text(f.node), "", "\(kind): committed at that value, the field is cleared")
            XCTAssertEqual(f.heard(), ["にほ"], "\(kind): which the app does not hear as typing")
        }
    }

    /// Nothing marked, a value the app writes is the text at once, and is no `input`.
    func testAValueWrittenWithNothingMarkedAppliesAtOnce() throws {
        for kind in ["input", "textarea"] {
            let f = try fixture(kind, value: "ab")
            defer { _ = f.keep }
            f.node.applyProps(set: ["value": "now"], clear: [])
            XCTAssertEqual(text(f.node), "now", kind)
            XCTAssertNil(f.node.pendingValue, kind)
            XCTAssertTrue(f.heard().isEmpty, kind)
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
        XCTAssertNil(node.pendingValue)
        node.fieldChanged()
        XCTAssertEqual(heard, ["한"], "the commit is not heard again, should UIKit say the field changed")
        node.applyProps(set: ["value": ""], clear: [])
        XCTAssertEqual(field.text, "", "the composer's cleared value lands")
        XCTAssertTrue(node.passesComposition("KeyA", command: false, down: true), "no composition: every key")
    }
}
#endif

#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 §5 (ruling 6) and LLP 1045 D5 on the Mac: a field and a text
/// area composing through an input method (`FieldComposition.swift`). The
/// app hears `input` at each change of the marked text, with it in the
/// value, where AppKit tells a delegate nothing until the commit; the commit
/// of what is marked as it stands is not heard again. The app's value never
/// ends a composition: the value the editor shows changes nothing, and a
/// different one written while text is marked is held and applied when the
/// composition ends, committed or cancelled.
///
/// The editor is driven as an input method drives it, through
/// `NSTextInputClient` (`setMarkedText`, `insertText`, `unmarkText`): kana,
/// then the kanji chosen for it. Nobody typed with a real input method.
final class FieldCompositionMacTests: XCTestCase {
    private var window: NSWindow!
    private var presenters: [Presenter] = [] // a node holds its presenter weakly
    override func tearDown() { close(); presenters = [] }
    private func close() { window?.makeFirstResponder(nil); window?.close(); window = nil }

    private let none = NSRange(location: NSNotFound, length: 0)
    private final class Heard { var inputs: [String] = []; var changes: [String] = [] }

    /// A node of `kind` (2) in a column (1) with the focus in its editor,
    /// which is returned with the caret at its text's end. `echo`: a
    /// controlled input, as most are: the app writes back what it hears, or
    /// what `echo` makes of it; nil writes nothing.
    private func fixture(_ kind: String, props: [String: String] = ["value": ""], echo: ((String) -> String)? = { $0 }) throws -> (NodeView, NSTextView, Heard) {
        _ = NSApplication.shared
        close()
        let p = Presenter()
        presenters.append(p)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": kind, "props": props, "handlers": ["input", "change"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 200],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 200, "h": 60],
        ]))
        let node = try XCTUnwrap(p.views[2]), heard = Heard()
        p.onInput = { [unowned node] _, value in
            heard.inputs.append(value)
            if let echo { node.applyProps(set: ["value": echo(value)], clear: []) }
        }
        p.onChange = { _, value in heard.changes.append(value) }
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(node.field ?? node.textArea)))
        let editor = try XCTUnwrap(window.firstResponder as? NSTextView)
        editor.setSelectedRange(NSRange(location: (editor.string as NSString).length, length: 0))
        return (node, editor, heard)
    }
    private func mark(_ editor: NSTextView, _ text: String) {
        editor.setMarkedText(text, selectedRange: NSRange(location: (text as NSString).length, length: 0), replacementRange: none)
    }
    private func cancel(_ editor: NSTextView) {
        editor.setMarkedText("", selectedRange: NSRange(location: 0, length: 0), replacementRange: none)
    }

    /// にほん, then 日本: each change of the marked text is an `input` with it
    /// in the value, in a field, a search field and a text area alike, and
    /// the app's echo of what it heard leaves the composition and its
    /// selection as they were.
    func testEachChangeOfMarkedTextIsAnInput() throws {
        for (kind, props) in [("input", ["value": "ab"]), ("input", ["value": "ab", "type": "search"]), ("textarea", ["value": "ab"])] {
            let (node, editor, heard) = try fixture(kind, props: props)
            let name = props["type"] ?? kind
            if kind == "input" { XCTAssertTrue(editor is FieldEditor, "\(name): the session's editor, which says what the window's does not") }
            for kana in ["に", "にほ", "にほん"] {
                mark(editor, kana)
                XCTAssertEqual(heard.inputs.last, "ab" + kana, "\(name): heard as it is composed")
                XCTAssertEqual(editor.markedRange(), NSRange(location: 2, length: (kana as NSString).length), "\(name): the echo leaves it marked")
                XCTAssertEqual(editor.selectedRange(), NSRange(location: 2 + (kana as NSString).length, length: 0), "\(name): and the caret where it was")
                XCTAssertNil(node.pendingValue, "\(name): the echo is the text: nothing is held")
            }
            XCTAssertEqual(heard.inputs, ["abに", "abにほ", "abにほん"], name)
            XCTAssertEqual(node.field?.stringValue ?? editor.string, "abにほん", "\(name): the field's value has the marked text in it")
            // The candidate chosen: the input method inserts it over what was marked.
            editor.insertText("日本", replacementRange: none)
            XCTAssertFalse(editor.hasMarkedText())
            XCTAssertEqual(heard.inputs, ["abに", "abにほ", "abにほん", "ab日本"], name)
            // Marked text committed as it stands changes no text: not heard
            // again, though AppKit takes it out and puts it back to do it.
            mark(editor, "ご")
            editor.unmarkText()
            XCTAssertEqual(editor.string, "ab日本ご")
            XCTAssertFalse(editor.hasMarkedText())
            XCTAssertEqual(Array(heard.inputs.suffix(2)), ["ab日本", "ab日本ご"], "\(name): the commit is no second input")
            // A composition cancelled takes its text away, which is a change (AppKit tells a delegate nothing).
            mark(editor, "x")
            XCTAssertEqual(heard.inputs.last, "ab日本ごx", name)
            cancel(editor)
            XCTAssertEqual(heard.inputs.last, "ab日本ご", "\(name): the cancelled text is gone from what the app holds")
            // The caret moving inside what is marked changes no text: no input.
            mark(editor, "かな")
            editor.setMarkedText("かな", selectedRange: NSRange(location: 1, length: 0), replacementRange: none)
            XCTAssertEqual(heard.inputs.last, "ab日本ごかな", name)
            cancel(editor)
            // Typing with no input method is heard as it always was.
            editor.insertText("!", replacementRange: none)
            XCTAssertEqual(heard.inputs, ["abに", "abにほ", "abにほん", "ab日本", "ab日本ご", "ab日本ごx", "ab日本ご", "ab日本ごかな", "ab日本ご", "ab日本ご!"], "\(name): one input for each change")
            XCTAssertTrue(heard.changes.isEmpty, "\(name): `change` is the editing's commit, not a composition's")
            window.makeFirstResponder(nil)
            XCTAssertEqual(heard.changes, ["ab日本ご!"], name)
        }
    }

    /// An app that keeps no copy of the value (it hears `input` and writes
    /// nothing back) composes undisturbed.
    func testAnAppThatWritesNothingBackLeavesTheCompositionAlone() throws {
        let (node, editor, heard) = try fixture("input", echo: nil)
        mark(editor, "に"); mark(editor, "にほん")
        XCTAssertEqual(heard.inputs, ["に", "にほん"])
        XCTAssertEqual(editor.markedRange(), NSRange(location: 0, length: 3))
        editor.insertText("日本", replacementRange: none)
        XCTAssertEqual(heard.inputs, ["に", "にほん", "日本"])
        XCTAssertNil(node.pendingValue)
    }

    /// LLP 1045 D5: an action that rewrites what it hears never ends a
    /// composition. Here it upper-cases: each change of にほん is heard, each
    /// rewrite is held while the text stays marked, and the rewrite lands
    /// when the composition ends, by the candidate chosen or as it stands.
    func testAnActionThatRewritesWhatItHearsWaitsForTheCompositionToEnd() throws {
        for kind in ["input", "textarea"] {
            // にほん through its changes, then 日本 chosen.
            var (node, editor, heard) = try fixture(kind, props: ["value": "ab"], echo: { $0.uppercased() })
            for kana in ["に", "にほ", "にほん"] {
                mark(editor, kana)
                XCTAssertEqual(editor.string, "ab" + kana, "\(kind): the text is the person's while it is marked")
                XCTAssertEqual(editor.markedRange(), NSRange(location: 2, length: (kana as NSString).length), "\(kind): the app's value does not end the composition")
                XCTAssertEqual(node.pendingValue, "AB" + kana, "\(kind): it is held, the last one written")
            }
            XCTAssertEqual(heard.inputs, ["abに", "abにほ", "abにほん"], "\(kind): the app heard each change")
            editor.insertText("日本", replacementRange: none)
            XCTAssertEqual(heard.inputs, ["abに", "abにほ", "abにほん", "ab日本"], "\(kind): and the commit, which changed the text")
            XCTAssertEqual(editor.string, "AB日本", "\(kind): its rewrite of that lands at once, nothing being marked")
            XCTAssertFalse(editor.hasMarkedText())
            XCTAssertNil(node.pendingValue, "\(kind): and nothing older is left to land")
            editor.insertText("c", replacementRange: none)
            XCTAssertEqual(heard.inputs.last, "AB日本c", kind)
            XCTAssertEqual(editor.string, "AB日本C", "\(kind): a write with nothing marked applies at once")
            XCTAssertEqual(heard.inputs.count, 5, "\(kind): the app's own writes are no input: \(heard.inputs)")

            // Committed as it stands: nothing more is heard, and what was held lands.
            (node, editor, heard) = try fixture(kind, props: ["value": "ab"], echo: { $0.uppercased() })
            mark(editor, "にほ"); mark(editor, "にほん")
            XCTAssertEqual(node.pendingValue, "ABにほん", kind)
            editor.unmarkText()
            XCTAssertEqual(heard.inputs, ["abにほ", "abにほん"], "\(kind): the commit changed no text")
            XCTAssertEqual(editor.string, "ABにほん", "\(kind): the held value is applied when the composition ends")
            XCTAssertNil(node.pendingValue, kind)
            XCTAssertFalse(editor.hasMarkedText())
        }
    }

    /// The same when the composition is cancelled: the app hears the text
    /// without what was marked, and what it wrote lands. AppKit posts no
    /// change for a cancel, so a held value used to wait for the next edit.
    func testAHeldValueLandsWhenTheCompositionIsCancelled() throws {
        for kind in ["input", "textarea"] {
            // The upper-casing action: its rewrite of the text the cancel leaves.
            var (node, editor, heard) = try fixture(kind, props: ["value": "ab"], echo: { $0.uppercased() })
            mark(editor, "に"); mark(editor, "にほ")
            XCTAssertEqual(node.pendingValue, "ABにほ", kind)
            XCTAssertTrue(editor.hasMarkedText())
            cancel(editor)
            XCTAssertEqual(heard.inputs, ["abに", "abにほ", "ab"], "\(kind): the cancel is a change of text")
            XCTAssertEqual(editor.string, "AB", "\(kind): and the app's value for it lands")
            XCTAssertNil(node.pendingValue, kind)

            // A value from another action while text is marked, and an app that answers nothing to the cancel.
            (node, editor, heard) = try fixture(kind, echo: nil)
            mark(editor, "に")
            node.applyProps(set: ["value": "done"], clear: [])
            XCTAssertEqual(editor.string, "に", "\(kind): held")
            XCTAssertEqual(editor.markedRange(), NSRange(location: 0, length: 1), "\(kind): the composition stands")
            mark(editor, "にほ")
            XCTAssertEqual(node.pendingValue, "done", "\(kind): still held as the composition goes on")
            cancel(editor)
            XCTAssertEqual(heard.inputs, ["に", "にほ", ""], kind)
            XCTAssertEqual(editor.string, "done", "\(kind): applied at the cancel, with no further edit")
            XCTAssertNil(node.pendingValue, kind)
        }
    }

    /// An action that clears the field on one value: the clear waits for
    /// the composition, a later echo of the text takes its place with
    /// nothing, and committed at that value the field is cleared.
    func testAnActionThatClearsOnAValueWaitsToo() throws {
        let clears = { (value: String) in value == "にほ" ? "" : value }
        for kind in ["input", "textarea"] {
            var (node, editor, heard) = try fixture(kind, echo: clears)
            mark(editor, "に"); mark(editor, "にほ")
            XCTAssertEqual(node.pendingValue, "", "\(kind): the clear is held")
            XCTAssertEqual(editor.markedRange(), NSRange(location: 0, length: 2), "\(kind): and the composition goes on")
            mark(editor, "にほん")
            XCTAssertNil(node.pendingValue, "\(kind): the app's value is the text again: nothing is owed")
            editor.insertText("日本", replacementRange: none)
            XCTAssertEqual(editor.string, "日本", "\(kind): no stale clear lands at the commit")
            XCTAssertEqual(heard.inputs, ["に", "にほ", "にほん", "日本"], kind)

            (node, editor, heard) = try fixture(kind, echo: clears)
            mark(editor, "にほ")
            editor.unmarkText()
            XCTAssertEqual(editor.string, "", "\(kind): committed at that value, the field is cleared")
            XCTAssertEqual(heard.inputs, ["にほ"], "\(kind): which the app does not hear as typing")
            // And the app knows the field is empty: typing the same text again is heard.
            editor.insertText("にほ", replacementRange: none)
            XCTAssertEqual(heard.inputs, ["にほ", "にほ"], kind)
            XCTAssertEqual(editor.string, "", "\(kind): cleared at once, nothing being marked")
            _ = node
        }
    }

    /// Nothing marked, a value the app writes is the text at once, and is no `input`.
    func testAValueWrittenWithNothingMarkedAppliesAtOnce() throws {
        for kind in ["input", "textarea"] {
            let (node, editor, heard) = try fixture(kind, props: ["value": "ab"])
            node.applyProps(set: ["value": "now"], clear: [])
            XCTAssertEqual(editor.string, "now", kind)
            XCTAssertNil(node.pendingValue, kind)
            XCTAssertTrue(heard.inputs.isEmpty, kind)
            // The same value again is nothing at all: the selection stays.
            editor.setSelectedRange(NSRange(location: 1, length: 1))
            node.applyProps(set: ["value": "now"], clear: [])
            XCTAssertEqual(editor.selectedRange(), NSRange(location: 1, length: 1), kind)
        }
    }
}
#endif

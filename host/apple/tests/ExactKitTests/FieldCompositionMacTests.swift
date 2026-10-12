#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 §5 (ruling 6) and LLP 1115.001 D8 on the Mac: a field and a
/// text area composing through an input method (`FieldComposition.swift`).
/// The app hears `input` at each change of the marked text, with it in the
/// value, where AppKit tells a delegate nothing until the commit; the commit
/// of what is marked as it stands is not heard again; the value the editor
/// shows leaves a composition alone and a different one ends it.
///
/// The editor is driven as an input method drives it, through
/// `NSTextInputClient` (`setMarkedText`, `insertText`, `unmarkText`): kana,
/// then the kanji chosen for it. Nobody typed with a real input method.
final class FieldCompositionMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.makeFirstResponder(nil); window?.close(); window = nil }

    private let none = NSRange(location: NSNotFound, length: 0)
    private final class Heard { var inputs: [String] = []; var changes: [String] = [] }

    /// A node of `kind` (2) in a column (1) with the focus in its editor,
    /// which is returned. `echo`: a controlled input, as most are: the app
    /// writes back what it hears, or what `echo` makes of it.
    private func fixture(_ kind: String, props: [String: String] = ["value": ""], echo: ((String) -> String)? = { $0 }) throws -> (Presenter, NodeView, NSTextView, Heard) {
        _ = NSApplication.shared
        let p = Presenter()
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
        return (p, node, try XCTUnwrap(window.firstResponder as? NSTextView), heard)
    }
    private func mark(_ editor: NSTextView, _ text: String) {
        editor.setMarkedText(text, selectedRange: NSRange(location: (text as NSString).length, length: 0), replacementRange: none)
    }

    /// にほん, then 日本: each change of the marked text is an `input` with it
    /// in the value, in a field, a search field and a text area alike, and
    /// the app's echo of what it heard leaves the composition and its
    /// selection as they were.
    func testEachChangeOfMarkedTextIsAnInput() throws {
        for (kind, props) in [("input", ["value": "ab"]), ("input", ["value": "ab", "type": "search"]), ("textarea", ["value": "ab"])] {
            let (p, node, editor, heard) = try fixture(kind, props: props)
            defer { _ = p } // a node holds its presenter weakly
            let name = props["type"] ?? kind
            if kind == "input" { XCTAssertTrue(editor is FieldEditor, "\(name): the session's editor, which says what the window's does not") }
            editor.setSelectedRange(NSRange(location: 2, length: 0))
            for kana in ["に", "にほ", "にほん"] {
                mark(editor, kana)
                XCTAssertEqual(heard.inputs.last, "ab" + kana, "\(name): heard as it is composed")
                XCTAssertEqual(editor.markedRange(), NSRange(location: 2, length: (kana as NSString).length), "\(name): the echo leaves it marked")
                XCTAssertEqual(editor.selectedRange(), NSRange(location: 2 + (kana as NSString).length, length: 0), "\(name): and the caret where it was")
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
            // The app's value for it now lands on committed text, whatever it is.
            mark(editor, "か")
            let told = heard.inputs.count
            node.applyProps(set: ["value": "ab日本ご"], clear: [])
            XCTAssertEqual(editor.string, "ab日本ご", "\(name): a different value replaces the text")
            XCTAssertFalse(editor.hasMarkedText(), "\(name): and ends the composition")
            XCTAssertEqual(heard.inputs.count, told, "\(name): the app's own write is no input")
            // A composition cancelled takes its text away, which is a change (AppKit tells a delegate nothing).
            mark(editor, "x")
            XCTAssertEqual(heard.inputs.last, "ab日本ごx", name)
            editor.setMarkedText("", selectedRange: NSRange(location: 0, length: 0), replacementRange: none)
            XCTAssertEqual(heard.inputs.last, "ab日本ご", "\(name): the cancelled text is gone from what the app holds")
            // The caret moving inside what is marked changes no text: no input.
            mark(editor, "かな")
            editor.setMarkedText("かな", selectedRange: NSRange(location: 1, length: 0), replacementRange: none)
            XCTAssertEqual(heard.inputs.last, "ab日本ごかな", name)
            editor.setMarkedText("", selectedRange: NSRange(location: 0, length: 0), replacementRange: none)
            // Typing with no input method is heard as it always was.
            editor.insertText("!", replacementRange: none)
            XCTAssertEqual(heard.inputs.last, "ab日本ご!", name)
            XCTAssertEqual(heard.inputs, ["abに", "abにほ", "abにほん", "ab日本", "ab日本ご", "ab日本ごか", "ab日本ごx", "ab日本ご", "ab日本ごかな", "ab日本ご", "ab日本ご!"], "\(name): one input for each change")
            XCTAssertTrue(heard.changes.isEmpty, "\(name): `change` is the editing's commit, not a composition's")
            window.makeFirstResponder(nil)
            XCTAssertEqual(heard.changes, ["ab日本ご!"], name)
            tearDown()
        }
    }

    /// An app that keeps no copy of the value (it hears `input` and writes
    /// nothing back) composes undisturbed.
    func testAnAppThatWritesNothingBackLeavesTheCompositionAlone() throws {
        let (p, _, editor, heard) = try fixture("input", echo: nil)
        defer { _ = p }
        mark(editor, "に"); mark(editor, "にほん")
        XCTAssertEqual(heard.inputs, ["に", "にほん"])
        XCTAssertEqual(editor.markedRange(), NSRange(location: 0, length: 3))
        editor.insertText("日本", replacementRange: none)
        XCTAssertEqual(heard.inputs, ["に", "にほん", "日本"])
    }

    /// LLP 1115.001 D8's rule: an app commit that sets a different value
    /// while text is marked replaces the text and ends the composition, and
    /// that write is not typing, so the app hears no `input` for it.
    func testADifferentValueEndsTheComposition() throws {
        for kind in ["input", "textarea"] {
            // The app keeps three characters.
            let (p, node, editor, heard) = try fixture(kind, echo: { String($0.prefix(3)) })
            defer { _ = p }
            mark(editor, "に")
            mark(editor, "にほ")
            XCTAssertEqual(editor.markedRange(), NSRange(location: 0, length: 2), "\(kind): within its limit the composition stands")
            mark(editor, "にほんご")
            XCTAssertEqual(heard.inputs, ["に", "にほ", "にほんご"], kind)
            XCTAssertEqual(editor.string, "にほん", "\(kind): the app's value is the text")
            XCTAssertFalse(editor.hasMarkedText(), "\(kind): and the composition is over")
            XCTAssertEqual(editor.selectedRange(), NSRange(location: 3, length: 0), kind)
            // What follows is ordinary typing over committed text.
            editor.insertText("!", replacementRange: none)
            XCTAssertEqual(heard.inputs, ["に", "にほ", "にほんご", "にほん!"], "\(kind): the app's own writes are no input")
            XCTAssertEqual(editor.string, "にほん", kind)
            _ = node
            tearDown()
        }
    }
}
#endif

// A Markdown editor's link destinations, hidden by the web editor's rules.
// @ref LLP 1045 D1, D5 — the destination takes no space; a caret steps over
// it; deletion, replacement, copy, undo, composition and accessibility keep
// source offsets. No claim about a physical keyboard or a phone's gestures.
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

private let destination = "](t3-citation://v1?text=%E2%80%9CThe%20editor%20draws%20every%20source%20character%E2%80%9D&comment=Why%20three%20lines%3F)"
private let source = "Ask about [Assistant quote" + destination + " and more."
/// The destination's start and end.
private let a = ("Ask about [Assistant quote" as NSString).length, b = a + (destination as NSString).length

#if os(macOS)
import AppKit

final class MarkupHiddenTests: XCTestCase {
    private var presenter: Presenter!
    private var session: ExactSession?

    override func setUp() {
        super.setUp()
        ExactMarkdown.install()
    }
    override func tearDown() {
        session?.destroy()
        session = nil
        super.tearDown()
    }

    private func editor(_ text: String = source, selection: NSRange) -> (NodeView, TextArea, NSWindow) {
        _ = NSApplication.shared
        presenter = Presenter()
        // The styler reads the node's fonts from its session's text engine.
        session = ExactApp.shared.makeSession(label: "markup-hidden")
        presenter.session = session
        let node = NodeView(id: 3, kind: "textarea", presenter: presenter)
        node.frame = NSRect(x: 0, y: 0, width: 500, height: 300)
        node.makeTextArea()
        node.props["markup"] = "markdown"
        node.props["value"] = text
        node.handlers = ["input"]
        node.applyTextArea()
        node.layoutTextArea()
        let window = NSWindow(contentRect: node.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = node
        let f = node.textArea as! TextArea
        window.makeFirstResponder(f)
        f.setSelectedRange(selection)
        node.restyleMarkup()
        f.undoManager?.removeAllActions()
        return (node, f, window)
    }

    func testTheDestinationTakesNoSpaceAndTheSourceStays() throws {
        let (_, f, window) = editor(selection: NSRange(location: 0, length: 0))
        defer { window.close() }
        XCTAssertEqual(f.string, source)
        let manager = try XCTUnwrap(f.textLayoutManager, "still TextKit 2")
        manager.ensureLayout(for: manager.documentRange)
        var lines = 0
        manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { lines += $0.textLineFragments.count; return true }
        XCTAssertEqual(lines, 1, "the destination is out of layout")
        let x = { (p: Int) in f.firstRect(forCharacterRange: NSRange(location: p, length: 0), actualRange: nil).minX }
        XCTAssertEqual(x(a), x(b), accuracy: 0.5)
        XCTAssertGreaterThan(x(b + 1), x(b))
        // Typed text never takes the hidden mark from the parenthesis before it.
        f.setSelectedRange(NSRange(location: b, length: 0))
        XCTAssertNil(f.typingAttributes[.markupHidden])
    }

    func testACaretStepsOverTheDestinationAndSitsAfterIt() {
        let (_, f, window) = editor(selection: NSRange(location: b, length: 0))
        defer { window.close() }
        f.doCommand(by: #selector(NSResponder.moveLeft(_:)))
        XCTAssertEqual(f.selectedRange(), NSRange(location: a - 1, length: 0), "one visible letter left")
        f.doCommand(by: #selector(NSResponder.moveRight(_:)))
        XCTAssertEqual(f.selectedRange(), NSRange(location: b, length: 0), "the label's end sits after the link")
        f.doCommand(by: #selector(NSResponder.moveRight(_:)))
        XCTAssertEqual(f.selectedRange(), NSRange(location: b + 1, length: 0))
        f.setSelectedRange(NSRange(location: b, length: 0))
        f.doCommand(by: #selector(NSResponder.moveWordLeft(_:)))
        XCTAssertEqual(f.selectedRange(), NSRange(location: a - 5, length: 0), "the label's last word")
        f.setSelectedRange(NSRange(location: b, length: 0))
        f.doCommand(by: #selector(NSResponder.moveLeftAndModifySelection(_:)))
        XCTAssertEqual(f.selectedRange(), NSRange(location: a - 1, length: b - a + 1), "a selection takes the letter too")
    }

    func testBackspaceAfterTheLinkDeletesTheLabelsLastLetterAsOneStep() throws {
        let (_, f, window) = editor(selection: NSRange(location: b, length: 0))
        defer { window.close() }
        f.doCommand(by: #selector(NSResponder.deleteBackward(_:)))
        let deleted = source.replacingOccurrences(of: "quote]", with: "quot]")
        XCTAssertEqual(f.string, deleted)
        XCTAssertEqual(f.selectedRange(), NSRange(location: a - 1, length: 0), "typing goes back into the label")
        f.breakUndoCoalescing()
        let manager = try XCTUnwrap(f.undoManager)
        manager.undo()
        XCTAssertEqual(f.string, source)
        manager.redo()
        XCTAssertEqual(f.string, deleted)
        f.setSelectedRange(NSRange(location: a - 1, length: 0))
        f.insertText("e", replacementRange: f.selectedRange())
        XCTAssertEqual(f.string, source)
    }

    func testDeletingAcrossTheDestinationKeepsItAndUndoesInOneStep() throws {
        let (_, f, window) = editor(selection: NSRange(location: a - 3, length: b - a + 4))
        defer { window.close() }
        var changes: [String] = []
        presenter.onInput = { _, value in changes.append(value) }
        f.doCommand(by: #selector(NSResponder.deleteBackward(_:)))
        let kept = "Ask about [Assistant qu" + destination + "and more."
        XCTAssertEqual(f.string, kept, "only the visible letters go")
        XCTAssertEqual(f.selectedRange(), NSRange(location: a - 3, length: 0))
        XCTAssertEqual(changes, [kept])
        try XCTUnwrap(f.undoManager).undo()
        XCTAssertEqual(f.string, source)
        // Typing over a selection of the destination alone lands after it.
        f.setSelectedRange(NSRange(location: a, length: b - a))
        f.insertText("!", replacementRange: f.selectedRange())
        XCTAssertEqual(f.string, "Ask about [Assistant quote" + destination + "! and more.")
    }

    func testDeletingOrReplacingAWholeLinkKeepsItsBracketAndDestination() throws {
        let link = "see [docs](http://x) now"
        let (_, f, window) = editor(link, selection: NSRange(location: 4, length: 16))
        defer { window.close() }
        f.doCommand(by: #selector(NSResponder.deleteBackward(_:)))
        XCTAssertEqual(f.string, "see [](http://x) now", "as the web editor leaves it")
        try XCTUnwrap(f.undoManager).undo()
        XCTAssertEqual(f.string, link)
        f.setSelectedRange(NSRange(location: 4, length: 16))
        f.insertText("Z", replacementRange: f.selectedRange())
        XCTAssertEqual(f.string, "see [Z](http://x) now")
        // Two links side by side: each keeps its own `[` and destination.
        let (_, g, other) = editor("[a](b)[c](d)", selection: NSRange(location: 1, length: 8))
        defer { other.close() }
        g.doCommand(by: #selector(NSResponder.deleteBackward(_:)))
        XCTAssertEqual(g.string, "[](b)[](d)")
    }

    func testDeletingBesideALinksBracketTakesTheGraphemePastIt() throws {
        let (_, f, window) = editor("see [docs](x)", selection: NSRange(location: 5, length: 0))
        defer { window.close() }
        f.doCommand(by: #selector(NSResponder.deleteBackward(_:)))
        XCTAssertEqual(f.string, "see[docs](x)", "Backspace after the `[` takes the space before it")
        XCTAssertEqual(f.selectedRange(), NSRange(location: 3, length: 0))
        try XCTUnwrap(f.undoManager).undo()
        XCTAssertEqual(f.string, "see [docs](x)")
        f.setSelectedRange(NSRange(location: 4, length: 0))
        f.doCommand(by: #selector(NSResponder.deleteForward(_:)))
        XCTAssertEqual(f.string, "see [ocs](x)", "Delete before the `[` takes the letter after it")
        XCTAssertEqual(f.selectedRange(), NSRange(location: 4, length: 0))
    }

    func testCopyOfALabelTakesItsBracketAndDestination() {
        let (_, f, window) = editor("x [a](b) y", selection: NSRange(location: 3, length: 1))
        defer { window.close() }
        f.copy(nil)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "[a](b)")
    }

    func testACompositionOverADestinationKeepsItAndComposes() {
        let (_, f, window) = editor(selection: NSRange(location: a - 3, length: b - a + 4))
        defer { window.close() }
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(f.hasMarkedText(), "the composition goes on")
        f.unmarkText()
        XCTAssertEqual(f.string, "Ask about [Assistant quに" + destination + "and more.")
    }

    func testCopyTakesTheDestinationAtTheSelectionsEdge() {
        let (_, f, window) = editor(selection: NSRange(location: a - 5, length: 5))
        defer { window.close() }
        f.copy(nil)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "quote" + destination)
    }

    func testAccessibilityReadsTheTextWithoutTheDestination() {
        let (_, f, window) = editor(selection: NSRange(location: 0, length: 0))
        defer { window.close() }
        XCTAssertEqual(f.accessibilityValue(), "Ask about [Assistant quote and more.")
        XCTAssertEqual(f.accessibilityString(for: NSRange(location: a - 5, length: b - a + 14)), "quote and more")
        XCTAssertEqual(f.accessibilityAttributedString(for: NSRange(location: a - 5, length: b - a + 9))?.string, "quote and")
    }

    func testComposedTextBesideTheDestinationIsShown() {
        let (node, f, window) = editor(selection: NSRange(location: b, length: 0))
        defer { window.close() }
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertNil(f.textStorage?.attribute(.markupHidden, at: b, effectiveRange: nil), "the composing text is drawn")
        f.unmarkText()
        node.restyleMarkup()
        XCTAssertEqual(f.string, "Ask about [Assistant quote" + destination + "に and more.")
        XCTAssertNotNil(f.textStorage?.attribute(.markupHidden, at: a, effectiveRange: nil))
        XCTAssertNil(f.textStorage?.attribute(.markupHidden, at: b, effectiveRange: nil))
    }
}
#endif

#if os(iOS)
import UIKit

/// The same on UIKit. A simulator runs it: bun host/apple/build.mjs --test --ios
final class MarkupHiddenIOSTests: XCTestCase {
    private var window: UIWindow!
    private var session: ExactSession?

    override func setUp() {
        super.setUp()
        ExactMarkdown.install()
    }
    override func tearDown() {
        session?.destroy()
        session = nil
        super.tearDown()
    }

    private func field(_ text: String = source) -> (Presenter, NodeView, TextArea) {
        // The styler reads the node's fonts from its session's text engine.
        let made = ExactApp.shared.makeSession(label: "markup-hidden")
        session = made
        let p = made.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 600, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": text, "markup": "markdown"], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 600.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 560.0, "h": 120.0],
        ]))
        let node = p.views[2]!, f = node.textArea as! TextArea
        XCTAssertTrue(f.becomeFirstResponder())
        node.restyleMarkup()
        return (p, node, f)
    }

    func testTheDestinationTakesNoSpace() throws {
        let (_, _, f) = field()
        XCTAssertEqual(f.text, source)
        let manager = try XCTUnwrap(f.textLayoutManager, "still TextKit 2")
        manager.ensureLayout(for: manager.documentRange)
        var lines = 0
        manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { lines += $0.textLineFragments.count; return true }
        XCTAssertEqual(lines, 1)
        let x = { (p: Int) in f.caretRect(for: f.position(from: f.beginningOfDocument, offset: p)!).minX }
        XCTAssertEqual(x(a), x(b), accuracy: 0.5)
        XCTAssertEqual(f.accessibilityValue, "Ask about [Assistant quote and more.")
    }

    func testACaretInTheDestinationSitsAfterItAndStepsOverIt() {
        let (_, _, f) = field()
        f.selectedRange = NSRange(location: b, length: 0)
        f.selectedRange = NSRange(location: a + 4, length: 0)
        XCTAssertEqual(f.selectedRange, NSRange(location: b, length: 0), "placed in it, after it")
        // UIKit's one step left from the end lands inside: on over the letter.
        f.selectedRange = NSRange(location: b - 1, length: 0)
        XCTAssertEqual(f.selectedRange, NSRange(location: a - 1, length: 0))
        f.selectedRange = NSRange(location: a - 5, length: 9)
        XCTAssertEqual(f.selectedRange, NSRange(location: a - 5, length: b - a + 5), "a range takes the destination whole")
    }

    func testDeletingOrReplacingAWholeLinkKeepsItsBracketAndDestination() {
        let (_, _, f) = field("see [docs](http://x) now")
        f.selectedRange = NSRange(location: 4, length: 16)
        f.deleteBackward()
        XCTAssertEqual(f.text, "see [](http://x) now")
        let (_, _, g) = field("see [docs](http://x) now")
        g.selectedRange = NSRange(location: 4, length: 16)
        g.insertText("Z")
        XCTAssertEqual(g.text, "see [Z](http://x) now")
    }

    func testBackspaceBesideALinksBracketTakesTheGraphemeBeforeIt() {
        let (_, _, f) = field("see [docs](x)")
        f.selectedRange = NSRange(location: 5, length: 0)
        f.deleteBackward()
        XCTAssertEqual(f.text, "see[docs](x)")
        XCTAssertEqual(f.selectedRange, NSRange(location: 3, length: 0))
    }

    func testACompositionOverADestinationKeepsItAndComposes() {
        let (_, _, f) = field()
        f.selectedRange = NSRange(location: a - 3, length: b - a + 4)
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0))
        XCTAssertNotNil(f.markedTextRange, "the composition goes on")
        f.unmarkText()
        XCTAssertEqual(f.text, "Ask about [Assistant quに" + destination + "and more.")
    }

    func testBackspaceAfterTheLinkDeletesTheLabelsLastLetter() throws {
        let (_, _, f) = field()
        f.selectedRange = NSRange(location: b, length: 0)
        f.deleteBackward()
        XCTAssertEqual(f.text, source.replacingOccurrences(of: "quote]", with: "quot]"))
        f.insertText("e")
        XCTAssertEqual(f.text, source, "typing goes back into the label")
        // A selection across it deletes only what is shown.
        f.selectedRange = NSRange(location: a - 3, length: b - a + 4)
        f.deleteBackward()
        XCTAssertEqual(f.text, "Ask about [Assistant qu" + destination + "and more.")
        let manager = try XCTUnwrap(f.undoManager)
        let deadline = Date().addingTimeInterval(2)
        while manager.groupingLevel > 0 && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        manager.undo()
        XCTAssertEqual(f.text, source, "the editor's edit undoes as the native one would")
    }
}
#endif

#if os(macOS)
import AppKit
import CoreText
import XCTest
@testable import ExactKit

/// A press on a `button` leaves the reader's text selection as it is, as
/// Chrome does, so the button's action reads it; a click on selectable text,
/// into a field, on another pressable or on the ground still clears it (#132).
final class TextSelectionMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.makeFirstResponder(nil); window?.close(); window = nil }

    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                          styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "handlers": ["selectionchange"], "props": ["text": "Alpha beta gamma"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Cite"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Cite"]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Another paragraph"]],
            ["op": "create", "id": 6, "kind": "input", "props": ["value": ""]],
            ["op": "create", "id": 7, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2, 3, 5, 6, 7]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 300, "h": 20],
            ["op": "frame", "id": 3, "x": 10, "y": 50, "w": 80, "h": 30],
            ["op": "frame", "id": 4, "x": 10, "y": 5, "w": 60, "h": 20],
            ["op": "frame", "id": 5, "x": 10, "y": 100, "w": 300, "h": 20],
            ["op": "frame", "id": 6, "x": 10, "y": 140, "w": 200, "h": 30],
            ["op": "frame", "id": 7, "x": 10, "y": 190, "w": 100, "h": 30],
        ]))
        return p
    }

    private func mouse(_ type: NSEvent.EventType, _ point: NSPoint, clicks: Int = 1) -> NSEvent {
        NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                           windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: clicks, pressure: type == .leftMouseUp ? 0 : 1)!
    }
    /// One line of text a session lays out (a bare presenter has no text
    /// engine), in the window; the session is the caller's to destroy.
    private func laidOut(_ text: String) -> (ExactSession, NodeView) {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-selection")
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 100)
        window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        let node = NodeView(id: 1, kind: "text", presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: 380, height: 20)
        node.props = ["text": text]
        node.style = ["font_size": 16.0, "line_height": "20px"]
        p.root.addSubview(node); p.views[1] = node
        return (session, node)
    }
    /// Where the character at `index` of a one-line paragraph is drawn: its
    /// leading edge and its width, in the node's space.
    private func glyph(_ node: NodeView, _ index: Int) throws -> (x: CGFloat, width: CGFloat) {
        let line = try XCTUnwrap(node.paragraphLayout()?.lines.first)
        let from = CTLineGetOffsetForStringIndex(line, index, nil), to = CTLineGetOffsetForStringIndex(line, index + 1, nil)
        return (node.paragraphBox().minX + from, to - from)
    }
    /// A click routed as NSWindow routes a person's: the view under the
    /// point takes the focus first, then hears the mouse; the release is a
    /// later turn of the run loop, and so is what the test reads.
    private func click(_ node: NSView, at local: NSPoint? = nil) {
        let point = node.convert(local ?? NSPoint(x: node.bounds.midX, y: node.bounds.midY), to: nil)
        let hit = window.contentView?.hitTest(point)
        if let hit, hit.acceptsFirstResponder { window.makeFirstResponder(hit) }
        hit?.mouseDown(with: mouse(.leftMouseDown, point))
        turn()
        hit?.mouseUp(with: mouse(.leftMouseUp, point))
        turn()
    }
    private func turn() { RunLoop.current.run(until: Date().addingTimeInterval(0.01)) }
    /// Drag across the first paragraph, from above its first line (offset 0)
    /// to just below its last (its end).
    private func select(_ p: Presenter) throws {
        let text = try XCTUnwrap(p.views[2])
        let from = text.convert(NSPoint(x: 1, y: -1), to: nil), to = text.convert(NSPoint(x: 299, y: 21), to: nil)
        text.mouseDown(with: mouse(.leftMouseDown, from))
        text.mouseDragged(with: mouse(.leftMouseDragged, to))
        text.mouseUp(with: mouse(.leftMouseUp, to))
        XCTAssertEqual(p.selection.selectedText(), "Alpha beta gamma")
        XCTAssertTrue(window.firstResponder === text)
    }

    func testAButtonPressKeepsTheSelectionForItsAction() throws {
        let p = fixture()
        var heard: [String] = []
        p.onSelectionChange = { _, text, start, end in heard.append("\(start)-\(end):\(text)") }
        var read: [String] = []
        p.onPress = { _ in read.append(p.selection.selectedText()) }
        try select(p)
        XCTAssertEqual(heard.last, "0-16:Alpha beta gamma")
        let count = heard.count
        let button = try XCTUnwrap(p.views[3]), label = try XCTUnwrap(p.views[4])
        // The button's own padding, then its label.
        for (target, at) in [(button as NSView, NSPoint(x: 4, y: 15)), (label, nil)] {
            click(target, at: at)
            XCTAssertEqual(p.selection.selectedText(), "Alpha beta gamma", "the press leaves the selection")
            XCTAssertEqual(p.selection.range(try XCTUnwrap(p.views[2])), NSRange(location: 0, length: 16), "and its highlight")
        }
        XCTAssertEqual(read, ["Alpha beta gamma", "Alpha beta gamma"], "the button's action reads the selection")
        XCTAssertEqual(heard.count, count, "no selectionchange on the press")
        // The action hides the button: the focus goes to the window.
        button.isHidden = true
        window.makeFirstResponder(nil)
        turn()
        XCTAssertEqual(p.selection.selectedText(), "Alpha beta gamma", "a pressed button that hides leaves the selection")
        XCTAssertEqual(heard.count, count)
    }

    func testAClickOnTextOrIntoAFieldStillClears() throws {
        let p = fixture()
        var heard: [String] = []
        p.onSelectionChange = { _, text, start, end in heard.append("\(start)-\(end):\(text)") }
        try select(p)
        click(try XCTUnwrap(p.views[5]), at: NSPoint(x: 2, y: 10))
        XCTAssertEqual(p.selection.selectedText(), "", "a click on other text collapses the selection")
        XCTAssertEqual(heard.last, "0-0:")
        try select(p)
        click(try XCTUnwrap(p.views[3]))
        XCTAssertEqual(p.selection.selectedText(), "Alpha beta gamma")
        let field = try XCTUnwrap(p.views[6]?.field)
        XCTAssertTrue(window.makeFirstResponder(field), "focus moves into the field from the button")
        turn()
        XCTAssertEqual(p.selection.selectedText(), "", "editing a field takes the page's selection")
        XCTAssertEqual(heard.last, "0-0:")
        for (target, at, what) in [(p.views[7], nil, "a pressable that is not a button"), (p.views[1], NSPoint(x: 350, y: 280), "the ground")] {
            try select(p)
            click(try XCTUnwrap(target), at: at)
            XCTAssertEqual(p.selection.selectedText(), "", "a click on \(what) clears the selection, as on the web")
            XCTAssertEqual(heard.last, "0-0:")
        }
    }

    /// LLP 1115 D8: on the Mac `user-select: auto` is AppKit's. A control's
    /// or the chrome's label is not selectable, so ⌘A and a drag skip it;
    /// an `article`'s text (its own header too) and plain content are, and
    /// an authored `user-select` decides over both.
    func testAutoLeavesControlAndChromeLabelsUnselectable() {
        _ = NSApplication.shared
        let p = Presenter()
        func text(_ id: Int, _ value: String) -> [String: Any] { ["op": "create", "id": id, "kind": "text", "props": ["text": value]] }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["semanticTag": "header"]], text(3, "Window title"),
            ["op": "create", "id": 4, "kind": "view", "props": ["semanticTag": "article"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["semanticTag": "header"]], text(6, "Article title"), text(7, "Body"),
            ["op": "create", "id": 8, "kind": "button", "handlers": ["press"]], text(9, "Cite"),
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "tablist"]], text(11, "Tab label"),
            ["op": "create", "id": 12, "kind": "view", "handlers": ["press"]], text(13, "Pressable row"),
            text(14, "Plain content"),
            ["op": "create", "id": 15, "kind": "view", "style": ["user_select": "none"]], text(16, "Opted out"),
            ["op": "create", "id": 17, "kind": "view", "props": ["semanticTag": "nav"]],
            ["op": "create", "id": 18, "kind": "view", "style": ["user_select": "text"]], text(19, "Opted in"),
            ["op": "create", "id": 20, "kind": "view", "props": ["accessibilityRole": "link"]], text(21, "Link"),
            ["op": "children", "id": 1, "ids": [2, 4, 8, 10, 12, 14, 15, 17, 20]],
            ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 4, "ids": [5, 7]], ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 8, "ids": [9]], ["op": "children", "id": 10, "ids": [11]], ["op": "children", "id": 12, "ids": [13]],
            ["op": "children", "id": 15, "ids": [16]], ["op": "children", "id": 17, "ids": [18]], ["op": "children", "id": 18, "ids": [19]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "roots", "ids": [1]],
        ]))
        XCTAssertEqual(p.selection.paragraphs.map(\.id), [6, 7, 14, 19])
        p.selection.selectAll()
        XCTAssertEqual(p.selection.selectedText(), "Article title\n\nBody\n\nPlain content\n\nOpted in")
        XCTAssertNil(p.selection.range(p.views[3]!), "the header's title is chrome")
        XCTAssertNil(p.selection.range(p.views[9]!), "a button's label is the control's")
    }

    /// A double click selects a word by the text system's boundaries, as an
    /// `NSTextView` does: without the punctuation beside it (the run between
    /// two spaces took "hello," whole), and a Japanese word, not its
    /// sentence. The word is the one under the pointer, which in a letter's
    /// trailing half is not the nearest caret's.
    func testADoubleClickSelectsTheWordUnderThePointer() throws {
        let sentence = "今日は良い天気ですね", text = "Say hello, world. " + sentence
        let (session, node) = laidOut(text)
        defer { session.destroy() }
        let p = session.presenter
        func doubleClick(_ index: Int, _ fraction: CGFloat) throws -> String {
            let at = try glyph(node, index)
            p.selection.begin(node, event: mouse(.leftMouseDown, node.convert(NSPoint(x: at.x + at.width * fraction, y: 10), to: nil), clicks: 2))
            return p.selection.selectedText()
        }
        XCTAssertEqual(try doubleClick(5, 0.5), "hello", "a word, without its comma")
        XCTAssertEqual(try doubleClick(8, 0.8), "hello", "its last letter's trailing half is still the word")
        XCTAssertEqual(try doubleClick(9, 0.5), ",", "the comma is its own")
        XCTAssertEqual(try doubleClick(11, 0.2), "world", "its first letter's leading half")
        XCTAssertEqual(try doubleClick(15, 0.8), "world", "without its full stop")
        // The system's own answer for the second character of the sentence
        // ("今日" where this was written), from its trailing half.
        let system = NSAttributedString(string: text).doubleClick(at: 19)
        let word = try doubleClick(19, 0.8)
        XCTAssertEqual(word, (text as NSString).substring(with: system))
        XCTAssertLessThan((word as NSString).length, (sentence as NSString).length, "a word of the sentence, not the sentence")
        p.selection.begin(node, event: mouse(.leftMouseDown, node.convert(NSPoint(x: 40, y: 10), to: nil), clicks: 3))
        XCTAssertEqual(p.selection.selectedText(), text, "a triple click is still the paragraph")
    }

    /// A secondary click on selected text, with no authored `contextmenu`,
    /// is a read-only text view's menu (LLP 1115 D8); elsewhere, none.
    func testASecondaryClickOnSelectedTextIsTheTextMenu() throws {
        let p = fixture()
        let text = try XCTUnwrap(p.views[2]), other = try XCTUnwrap(p.views[5])
        let point = text.convert(NSPoint(x: 20, y: 10), to: nil)
        let right = NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        XCTAssertNil(text.menu(for: right), "no selection, no menu")
        try select(p)
        let menu = try XCTUnwrap(text.menu(for: right))
        XCTAssertEqual(menu.items.filter { !$0.isSeparatorItem }.map(\.title), ["Look Up “Alpha beta gamma”", "Copy", "Speech"])
        XCTAssertNil(other.menu(for: right), "a paragraph outside the selection has none")
        XCTAssertTrue(text.validRequestor(forSendType: .string, returnType: nil) as AnyObject? === text, "Services can take the selection")
        let board = NSPasteboard(name: .init("exact.test.services"))
        XCTAssertTrue(text.writeSelection(to: board, types: [.string]))
        XCTAssertEqual(board.string(forType: .string), "Alpha beta gamma")
        let copy = try XCTUnwrap(menu.items.first { $0.title == "Copy" })
        NSPasteboard.general.clearContents()
        _ = (copy.target as? NSObject)?.perform(copy.action, with: copy)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "Alpha beta gamma")
    }
}
#endif

#if os(macOS)
import AppKit
import CExact
import CoreText
import XCTest
@testable import ExactKit

/// A press on a `button` leaves the reader's text selection as it is, as
/// Chrome does, so the button's action reads it; a click on selectable text,
/// into a field, on another pressable or on the ground still clears it (#132).
final class TextSelectionMacTests: XCTestCase {
    private var window: NSWindow!
    private var session: ExactSession?
    override func tearDown() { window?.makeFirstResponder(nil); window?.close(); window = nil; session?.destroy(); session = nil }

    /// `measured`: a session's presenter, whose text engine lays the text
    /// out, so a point finds the word under it (a bare one finds offset 0).
    private func fixture(measured: Bool = false) -> Presenter {
        _ = NSApplication.shared
        if measured { session = ExactApp.shared.makeSession(label: "text-selection") }
        let p = session?.presenter ?? Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                          styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "handlers": ["selectionchange"], "props": ["text": "Alpha beta gamma"], "style": ["font_size": 16.0]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Cite"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Cite"]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Another paragraph"]],
            ["op": "create", "id": 6, "kind": "input", "props": ["value": ""]],
            ["op": "create", "id": 7, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 8, "kind": "view", "handlers": ["contextmenu"]],
            ["op": "create", "id": 9, "kind": "text", "handlers": ["selectionchange"], "props": ["text": "Row words"]],
            ["op": "create", "id": 10, "kind": "text", "handlers": ["selectionchange", "contextmenu"], "props": ["text": "Menu words"]],
            ["op": "children", "id": 1, "ids": [2, 3, 5, 6, 7, 8, 10]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 8, "ids": [9]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 300, "h": 20],
            ["op": "frame", "id": 3, "x": 10, "y": 50, "w": 80, "h": 30],
            ["op": "frame", "id": 4, "x": 10, "y": 5, "w": 60, "h": 20],
            ["op": "frame", "id": 5, "x": 10, "y": 100, "w": 300, "h": 20],
            ["op": "frame", "id": 6, "x": 10, "y": 140, "w": 200, "h": 30],
            ["op": "frame", "id": 7, "x": 10, "y": 190, "w": 100, "h": 30],
            ["op": "frame", "id": 8, "x": 10, "y": 230, "w": 300, "h": 24],
            ["op": "frame", "id": 9, "x": 0, "y": 2, "w": 300, "h": 20],
            ["op": "frame", "id": 10, "x": 10, "y": 265, "w": 300, "h": 20],
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
    /// Drag across a paragraph, the first unless named, from above its
    /// first line (offset 0) to just below its last (its end).
    private func select(_ p: Presenter, _ id: UInt32 = 2, _ expected: String = "Alpha beta gamma") throws {
        let text = try XCTUnwrap(p.views[id])
        let from = text.convert(NSPoint(x: 1, y: -1), to: nil), to = text.convert(NSPoint(x: 299, y: 21), to: nil)
        text.mouseDown(with: mouse(.leftMouseDown, from))
        text.mouseDragged(with: mouse(.leftMouseDragged, to))
        text.mouseUp(with: mouse(.leftMouseUp, to))
        XCTAssertEqual(p.selection.selectedText(), expected)
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
        // A contact delivered to the scroll view itself clears page selection
        // while the first responder stays put, as AppKit leaves it on a
        // window's empty ground (LLP 1115.000 D7.1).
        try select(p)
        let ground = p.viewport.convert(NSPoint(x: 350, y: 280), to: nil)
        p.viewport.mouseDown(with: mouse(.leftMouseDown, ground))
        XCTAssertEqual(p.selection.selectedText(), "")
        XCTAssertTrue(window.firstResponder === p.views[2])
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

    /// X76 by MAC-16's rule: a secondary click outside the selection takes
    /// the word a double click at the same point takes, by the text
    /// system's boundaries ("hello" without its comma, a word of a Japanese
    /// sentence), so the two never disagree.
    func testASecondaryClickSelectsTheWordADoubleClickSelects() throws {
        let text = "Say hello, world. 今日は良い天気ですね"
        let (session, node) = laidOut(text)
        defer { session.destroy() }
        let p = session.presenter
        for (index, fraction) in [(5, 0.5), (8, 0.8), (9, 0.5), (11, 0.2), (15, 0.8), (19, 0.8)] as [(Int, CGFloat)] {
            let at = try glyph(node, index)
            let point = node.convert(NSPoint(x: at.x + at.width * fraction, y: 10), to: nil)
            p.selection.begin(node, event: mouse(.leftMouseDown, point, clicks: 2))
            let word = p.selection.selectedText()
            p.selection.clear()
            let right = try XCTUnwrap(NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                         windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1))
            XCTAssertTrue(p.selection.secondaryClick(node, event: right, answered: {}), "a laid-out paragraph places the click at once")
            XCTAssertFalse(word.isEmpty)
            XCTAssertEqual(p.selection.selectedText(), word, "character \(index) at \(fraction) of its width")
        }
    }

    /// A secondary click on selected text, with no authored `contextmenu`,
    /// is a read-only text view's menu (LLP 1115 D8) for the selection.
    func testASecondaryClickOnSelectedTextIsTheTextMenu() throws {
        let p = fixture()
        let text = try XCTUnwrap(p.views[2])
        let point = text.convert(NSPoint(x: 20, y: 10), to: nil)
        let right = NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        try select(p)
        let menu = try XCTUnwrap(text.menu(for: right))
        XCTAssertEqual(menu.items.filter { !$0.isSeparatorItem }.map(\.title), ["Look Up “Alpha beta gamma”", "Copy", "Speech"])
        XCTAssertTrue(text.validRequestor(forSendType: .string, returnType: nil) as AnyObject? === text, "Services can take the selection")
        let board = NSPasteboard(name: .init("exact.test.services"))
        XCTAssertTrue(text.writeSelection(to: board, types: [.string]))
        XCTAssertEqual(board.string(forType: .string), "Alpha beta gamma")
        let copy = try XCTUnwrap(menu.items.first { $0.title == "Copy" })
        NSPasteboard.general.clearContents()
        _ = (copy.target as? NSObject)?.perform(copy.action, with: copy)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "Alpha beta gamma")
    }

    /// X76: a secondary click on a word outside the selection selects that
    /// word, the one a double click selects, before the text menu opens for
    /// it, as Chrome and an `NSTextView` do on a Mac; inside the selection
    /// it keeps it. Unselectable text keeps what is selected and opens
    /// nothing, and an authored `contextmenu` on the text or above it runs
    /// instead and selects nothing, as in Chrome (the web host prevents the
    /// browser's menu, and Chrome selects only for its own).
    func testASecondaryClickSelectsTheWordUnderItThenOpensTheTextMenu() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent opens no native menu")
        let p = fixture(measured: true)
        window.orderFrontRegardless()
        var heard: [String] = []
        p.onSelectionChange = { id, text, start, end in heard.append("\(id) \(start)-\(end):\(text)") }
        var authored: [UInt32] = []
        p.onClipboard = { id, kind, _ in if kind == 10 { authored.append(id) } }
        var menus: [(titles: [String], copy: Bool, heard: String?)] = []
        let center = NotificationCenter.default
        let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu else { return }
            menus.append((menu.items.filter { !$0.isSeparatorItem }.map(\.title), menu.items.first { $0.title == "Copy" }?.isEnabled == true, heard.last))
            RunLoop.current.perform(inModes: [.eventTracking, .default]) { menu.cancelTracking() }
        }
        defer { center.removeObserver(observer) }
        // The button down and up through the window, as a mouse's are routed.
        func rightClick(_ id: UInt32, at local: NSPoint) throws {
            let point = try XCTUnwrap(p.views[id]).convert(local, to: nil)
            for type in [NSEvent.EventType.rightMouseDown, .rightMouseUp] {
                window.sendEvent(try XCTUnwrap(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                                  windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1,
                                                                  pressure: type == .rightMouseDown ? 1 : 0)))
            }
            turn()
        }
        let text = try XCTUnwrap(p.views[2]), layout = try XCTUnwrap(text.paragraphLayout())
        // The middle of the characters `range` covers in "Alpha beta gamma".
        func on(_ location: Int, _ length: Int) throws -> NSPoint {
            let rect = try XCTUnwrap(layout.selectionRects(NSRange(location: location, length: length), align: text.paragraphSpec().align,
                                                           in: text.paragraphBox(), dirty: text.bounds).first)
            return NSPoint(x: rect.midX, y: rect.midY)
        }
        try rightClick(2, at: on(1, 1))
        XCTAssertEqual(p.selection.selectedText(), "Alpha", "the word under the pointer")
        XCTAssertTrue(window.firstResponder === text, "which takes the focus, as a double click does")
        XCTAssertEqual(menus.last?.titles, ["Look Up “Alpha”", "Copy", "Speech"])
        XCTAssertEqual(menus.last?.copy, true)
        XCTAssertEqual(menus.last?.heard, "2 0-5:Alpha", "its selectionchange before the menu opens")
        try rightClick(2, at: on(3, 1))
        XCTAssertEqual(p.selection.selectedText(), "Alpha", "inside the selection it stays")
        XCTAssertEqual(heard, ["2 0-5:Alpha"])
        XCTAssertEqual(menus.count, 2)
        try rightClick(2, at: on(13, 1))
        XCTAssertEqual(p.selection.selectedText(), "gamma", "another word outside it")
        XCTAssertEqual(heard, ["2 0-5:Alpha", "2 11-16:gamma"])
        XCTAssertEqual(menus.last?.titles.first, "Look Up “gamma”")
        try select(p)
        try rightClick(2, at: on(7, 1))
        XCTAssertEqual(p.selection.selectedText(), "Alpha beta gamma", "a word inside a longer selection keeps it")
        XCTAssertEqual(menus.last?.titles.first, "Look Up “Alpha beta gamma”")
        XCTAssertEqual(menus.count, 4)
        try rightClick(4, at: NSPoint(x: 10, y: 10))
        XCTAssertEqual(p.selection.selectedText(), "Alpha beta gamma", "a button's label selects nothing")
        XCTAssertEqual(menus.count, 4, "and opens nothing")
        let count = heard.count
        try rightClick(9, at: NSPoint(x: 10, y: 10))
        try rightClick(10, at: NSPoint(x: 10, y: 10))
        XCTAssertEqual(authored, [8, 10], "an ancestor's contextmenu, then the text's own")
        XCTAssertEqual(p.selection.selectedText(), "Alpha beta gamma", "neither selects a word")
        XCTAssertEqual(heard.count, count)
        try select(p, 9, "Row words")
        try rightClick(9, at: NSPoint(x: 10, y: 10))
        XCTAssertEqual(authored, [8, 10, 8], "selected text under an authored contextmenu hears it too")
        XCTAssertEqual(p.selection.selectedText(), "Row words")
        XCTAssertEqual(menus.count, 4, "and no text menu opens over it")
    }

    /// X76 on a reader paragraph whose caret cache misses the point (here a
    /// line outside the band its worker rastered; in use, a line too long
    /// for that cache): the click opens no menu itself, the raster worker
    /// places it, and then the word under it is selected, or the selection
    /// under it kept, and the menu opens for that.
    func testASecondaryClickTheReaderMustPlaceOpensTheMenuOncePlaced() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent opens no native menu")
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-selection-reader")
        self.session = session
        let p = session.presenter
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400),
                          styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        window.orderFrontRegardless()
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 600, height: 400))
        let node = NodeView(id: 12345, kind: "text", presenter: p)
        node.applyStyle(["font_size": 16.0, "line_height": "24px"])
        node.applyProps(set: ["text": String(repeating: "reader scroll ahead words ", count: 4000)], clear: [])
        node.frame = NSRect(x: 0, y: 0, width: 600, height: 100000)
        node.prepareToMount()
        scroll.documentView = node; p.root.addSubview(scroll)
        var request = ExactMeasureRequest()
        request.view = node.id; request.strut.font_size = 16
        request.strut.has_line_height = 1; request.strut.line_height = 24
        let reader = RegionReaderParagraph(request, bytes: 100000)
        session.text.readerParagraphs[node.id] = reader
        reader.update(node)
        // Each wait is for its event; the bound only stops one that never comes.
        func until(_ seconds: TimeInterval, _ done: () -> Bool) {
            let hang = Date(timeIntervalSinceNow: seconds)
            while !done() && Date() < hang { RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.001)) }
        }
        until(300) { reader.raster != nil }
        let height: CGFloat = try XCTUnwrap(reader.accepted?.metadata.height)
        var menus: [String] = []
        let center = NotificationCenter.default
        let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu else { return }
            menus.append(menu.items.first?.title ?? "")
            RunLoop.current.perform(inModes: [.eventTracking, .default]) { menu.cancelTracking() }
        }
        defer { center.removeObserver(observer) }
        // The first word of the line at `fraction` of the text's height.
        func rightClick(_ fraction: CGFloat) -> NSMenu? {
            let box = node.contentBox()
            let local = NSPoint(x: box.minX + 4, y: box.minY + floor(height * fraction / 24) * 24 + 12)
            XCTAssertNil(reader.offset(at: local, node: node), "the caret cache misses the point")
            return node.menu(for: NSEvent.mouseEvent(with: .rightMouseDown, location: node.convert(local, to: nil), modifierFlags: [],
                                                     timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                                     context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!)
        }
        XCTAssertNil(rightClick(0.5), "nothing is known yet to open a menu over")
        until(60) { menus.count == 1 }
        let word = p.selection.selectedText(), first = p.selection.range(node)
        XCTAssertTrue(["reader", "scroll", "ahead", "words"].contains(word), "the word the worker placed it on, not \"\(word)\"")
        XCTAssertEqual(menus, ["Look Up “\(word)”"], "its menu opens once it is placed")
        XCTAssertNil(rightClick(0.5))
        until(60) { menus.count == 2 }
        XCTAssertEqual(p.selection.range(node), first, "inside the selection it stays")
        XCTAssertEqual(menus.last, "Look Up “\(word)”")
        XCTAssertNil(rightClick(0.75), "the selection elsewhere opens no menu for itself")
        until(60) { menus.count == 3 }
        let second = try XCTUnwrap(p.selection.range(node))
        XCTAssertGreaterThan(second.location, NSMaxRange(try XCTUnwrap(first)), "the word under the later point")
        XCTAssertEqual(menus.last, "Look Up “\(p.selection.selectedText())”")
    }
}
#endif

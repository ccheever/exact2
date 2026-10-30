#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

/// LLP 1001 §1: CSS `direction` is a plain field's paragraph base direction,
/// as it is Apple text's (`TextEngine.setBaseDirection`) and the Markdown
/// editor's (LLP 1045 D11): an `input` (plain, password, search), a
/// `textarea` of either appearance, and a Markdown editor's text once its
/// `markup` goes. No first strong character picks it. An unsaid
/// `text-align` is `start`, the right under `rtl`, as on the web.
final class FieldDirectionMacTests: XCTestCase {
    private var window: NSWindow?
    private var sessions: [ExactSession] = []
    override func setUp() { super.setUp(); ExactMarkdown.install() }
    override func tearDown() { window?.close(); window = nil; sessions.forEach { $0.destroy() }; sessions = []; super.tearDown() }

    /// A presenter whose node 2 is a `kind` with `props` and `style`.
    private func field(_ kind: String, _ value: String, props: [String: Any] = [:], style: [String: Any]) -> (Presenter, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        // A session for the text engine a field styles with.
        let session = ExactApp.shared.makeSession(label: "field-direction")
        sessions.append(session)
        p.session = session
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        w.contentView = p.viewport
        window = w
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": kind, "props": props.merging(["value": value]) { a, _ in a }, "handlers": ["input", "change"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": kind == "input" ? 24.0 : 120.0],
            ["op": "style", "id": 2, "style": style],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        return (p, p.views[2]!)
    }
    /// The text view editing node's text: a text area's own, an input's field editor once it has the focus.
    private func editor(_ node: NodeView) throws -> NSTextView {
        if let area = node.textArea { return area }
        let field = try XCTUnwrap(node.field)
        if field.currentEditor() == nil { window?.makeFirstResponder(field) }
        return try XCTUnwrap(field.currentEditor() as? NSTextView)
    }
    private func base(_ tv: NSTextView, at index: Int = 0) -> NSWritingDirection? {
        (tv.textStorage?.attribute(.paragraphStyle, at: index, effectiveRange: nil) as? NSParagraphStyle)?.baseWritingDirection
    }
    /// Where character `index` is drawn, from the window's left: asked as an
    /// input method asks, which leaves a text area on TextKit 2 (reading its
    /// `layoutManager` would put it on TextKit 1).
    private func x(_ tv: NSTextView, _ index: Int) throws -> CGFloat {
        var actual = NSRange()
        return tv.firstRect(forCharacterRange: NSRange(location: index, length: 1), actualRange: &actual).minX - (tv.window?.frame.minX ?? 0)
    }

    /// Tests 1 and 2: each field kind takes the node's direction as its
    /// base, whichever script its text opens with.
    func testEachFieldTakesTheNodesDirectionAsItsBase() throws {
        let kinds: [(String, [String: Any], [String: Any])] = [
            ("input", [:], [:]), ("input", ["type": "search"], [:]),
            ("textarea", [:], [:]), ("textarea", [:], ["appearance": "none"]),
        ]
        for (kind, props, look) in kinds {
            let name = "\(kind) \(props) \(look)"
            // `rtl` under text that opens left to right: the Latin run sits right of the Arabic.
            var (_, node) = field(kind, "Hello مرحبا", props: props, style: look.merging(["direction": "rtl"]) { a, _ in a })
            var tv = try editor(node)
            XCTAssertEqual(base(tv), .rightToLeft, name)
            XCTAssertEqual((tv.typingAttributes[.paragraphStyle] as? NSParagraphStyle)?.baseWritingDirection, .rightToLeft, name)
            XCTAssertGreaterThan(try x(tv, 0), try x(tv, 6), "\(name): H right of م")
            window?.close()
            // `ltr` under text that opens right to left: the Arabic sits left of the Latin.
            (_, node) = field(kind, "مرحبا Hello", props: props, style: look.merging(["direction": "ltr"]) { a, _ in a })
            tv = try editor(node)
            XCTAssertEqual(base(tv), .leftToRight, name)
            XCTAssertLessThan(try x(tv, 0), try x(tv, 6), "\(name): م left of H")
            window?.close()
        }
    }

    /// A password field's editor (AppKit's secure one) and a text area's
    /// second paragraph opening in the other script take the base too.
    func testAPasswordFieldAndEveryParagraphTakeTheBase() throws {
        var (_, node) = field("input", "abc", props: ["type": "password"], style: ["direction": "rtl"])
        let secure = try editor(node)
        XCTAssertEqual(base(secure), .rightToLeft)
        XCTAssertEqual(node.field?.baseWritingDirection, .rightToLeft)
        window?.close()
        for look: [String: Any] in [[:], ["appearance": "none"]] {
            (_, node) = field("textarea", "مرحبا\nHello مرحبا", style: look.merging(["direction": "rtl"]) { a, _ in a })
            let tv = try editor(node)
            XCTAssertEqual(base(tv, at: 0), .rightToLeft, "\(look)")
            XCTAssertEqual(base(tv, at: 6), .rightToLeft, "\(look): the second paragraph, opening in Latin")
            window?.close()
        }
    }

    /// Test 3: an unsaid `text-align` is `start`, the right under `rtl`;
    /// one said stands.
    func testAnUnsaidAlignmentIsStart() throws {
        var (_, node) = field("input", "Hi", style: ["direction": "rtl"])
        XCTAssertEqual(node.field?.alignment, .right, "an input")
        XCTAssertEqual((try editor(node).typingAttributes[.paragraphStyle] as? NSParagraphStyle)?.alignment, .right)
        window?.close()
        (_, node) = field("input", "Hi", style: ["direction": "rtl", "text_align": "left"])
        XCTAssertEqual(node.field?.alignment, .left, "an input said left")
        window?.close()
        (_, node) = field("textarea", "", style: ["direction": "rtl"])
        XCTAssertEqual(node.textArea?.defaultParagraphStyle?.alignment, .right, "an empty textarea's default paragraph")
        XCTAssertEqual(node.textArea?.defaultParagraphStyle?.baseWritingDirection, .rightToLeft)
        window?.close()
        for look: [String: Any] in [[:], ["appearance": "none"]] {
            (_, node) = field("textarea", "Hi", style: look.merging(["direction": "rtl"]) { a, _ in a })
            XCTAssertGreaterThan(try x(try editor(node), 0), 150, "a textarea \(look) sits at the right")
            XCTAssertNil(node.textArea?.textLayoutManager == nil ? "TextKit 1" : nil, "still TextKit 2")
            window?.close()
        }
        for look: [String: Any] in [[:], ["appearance": "none"]] {
            (_, node) = field("textarea", "Hi", style: look.merging(["direction": "rtl", "text_align": "left"]) { a, _ in a })
            XCTAssertLessThan(try x(try editor(node), 0), 20, "a textarea \(look) said left")
            window?.close()
        }
    }

    /// Test 4: a Markdown editor whose `markup` goes keeps the field's base
    /// (the QUEUE's example: `مرحبا Hello` left to right in an `ltr` field).
    func testAMarkdownEditorWhoseMarkupGoesKeepsTheBase() throws {
        let (p, node) = field("textarea", "مرحبا Hello", props: ["markup": "markdown"], style: ["direction": "ltr"])
        node.restyleMarkup()
        let tv = try editor(node)
        XCTAssertEqual(base(tv), .leftToRight, "the Markdown editor's")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["markup": "none"]]]))
        XCTAssertNil((tv as? TextArea)?.markup)
        XCTAssertEqual(base(tv), .leftToRight, "the plain field's, once markup goes")
        XCTAssertEqual((tv.typingAttributes[.paragraphStyle] as? NSParagraphStyle)?.baseWritingDirection, .leftToRight)
        XCTAssertLessThan(try x(tv, 0), try x(tv, 6), "م left of H")
    }

    /// A Markdown editor's `markup` going under `rtl` and a said `left`:
    /// the plain field's side, not the editor's plain look's.
    func testAMarkdownEditorWhoseMarkupGoesKeepsASaidSide() throws {
        let (p, node) = field("textarea", "Hello", props: ["markup": "markdown"], style: ["direction": "rtl", "text_align": "left"])
        node.restyleMarkup()
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["markup": "none"]]]))
        let tv = try editor(node)
        XCTAssertEqual((tv.textStorage?.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)?.alignment, .left)
        XCTAssertEqual(base(tv), .rightToLeft)
        XCTAssertLessThan(try x(tv, 0), 20)
    }

    /// An undo brings back the text it removed in the field's direction now,
    /// not the one it had then; one step, as before.
    func testAnUndoAfterADirectionChangeTakesTheDirectionNow() throws {
        for (kind, look) in [("textarea", [String: Any]()), ("input", [:]), ("input", ["appearance": "none"])] {
            let (p, node) = field(kind, "abc", style: look.merging(["direction": "ltr"]) { a, _ in a })
            let tv = try editor(node)
            if kind == "textarea" { window?.makeFirstResponder(tv) }
            tv.setSelectedRange(NSRange(location: 0, length: 3))
            tv.deleteBackward(nil)
            XCTAssertEqual(tv.string, "", kind)
            p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "rtl"]]]))
            let manager = try XCTUnwrap(tv.undoManager, kind)
            manager.undo()
            XCTAssertEqual(tv.string, "abc", kind)
            XCTAssertEqual(base(tv), .rightToLeft, "\(kind): the restored text takes the field's direction now")
            if kind == "input" { XCTAssertEqual((tv.textStorage?.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)?.alignment, .right, "\(look): and its side") }
            XCTAssertFalse(manager.canUndo, "\(kind): the restyle is no step of its own")
            window?.close()
        }
    }

    /// A `direction` that comes while an input method composes applies once
    /// the composition ends, committed or not; the marked text is left alone.
    func testADirectionThatComesWhileComposingAppliesAtItsEnd() throws {
        for kind in ["textarea", "input"] {
            let (p, node) = field(kind, "abc", style: ["direction": "rtl"])
            let tv = try editor(node)
            window?.makeFirstResponder(kind == "input" ? node.field : tv)
            let live = try editor(node)
            live.setSelectedRange(NSRange(location: 3, length: 0))
            live.setMarkedText("か", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
            p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "ltr"]]]))
            XCTAssertTrue(live.hasMarkedText(), kind)
            live.unmarkText()
            XCTAssertEqual(base(live), .leftToRight, "\(kind): at the composition's end")
            XCTAssertEqual((live.typingAttributes[.paragraphStyle] as? NSParagraphStyle)?.baseWritingDirection, .leftToRight, kind)
            // Cancelled (no text marked any more, and AppKit posts no change): the same.
            live.setSelectedRange(NSRange(location: (live.string as NSString).length, length: 0))
            live.setMarkedText("か", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
            p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "rtl"]]]))
            live.setMarkedText("", selectedRange: NSRange(location: 0, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
            XCTAssertFalse(live.hasMarkedText(), kind)
            XCTAssertEqual(base(live), .rightToLeft, "\(kind): at a cancelled composition's end")
            window?.close()
        }
    }

    /// A composition cancelled in one input whose `input` handler moves the
    /// focus to another leaves the editor styled for the field that has it.
    func testACancelThatMovesTheFocusStylesTheEditorForItsNewField() throws {
        _ = NSApplication.shared
        let p = Presenter()
        let session = ExactApp.shared.makeSession(label: "field-direction")
        sessions.append(session)
        p.session = session
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        w.contentView = p.viewport
        window = w
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "input", "props": ["value": "abc"], "handlers": ["input"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["value": "xyz"], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 24.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 300.0, "h": 24.0],
            ["op": "style", "id": 2, "style": ["direction": "rtl"]],
            ["op": "style", "id": 3, "style": ["direction": "ltr"]],
        ]))
        let a = try XCTUnwrap(p.views[2]?.field), b = try XCTUnwrap(p.views[3]?.field)
        w.makeFirstResponder(a)
        let shared = try XCTUnwrap(a.currentEditor() as? NSTextView)
        shared.setSelectedRange(NSRange(location: 3, length: 0))
        shared.setMarkedText("か", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        let forward = p.onInput
        p.onInput = { id, value in forward?(id, value); if id == 2 { w.makeFirstResponder(b) } }
        defer { p.onInput = forward }
        shared.setMarkedText("", selectedRange: NSRange(location: 0, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        let now = try XCTUnwrap(b.currentEditor() as? NSTextView)
        XCTAssertEqual(base(now), .leftToRight, "the editor is the second field's, styled for it")
    }

    /// An undo on another thread is no field editor's: its notification is passed over.
    func testAnUndoOffTheMainThreadIsPassedOver() throws {
        let (_, node) = field("input", "abc", style: ["direction": "rtl"])
        _ = try editor(node)
        let done = expectation(description: "posted")
        DispatchQueue.global().async {
            NotificationCenter.default.post(name: .NSUndoManagerDidUndoChange, object: UndoManager())
            done.fulfill()
        }
        wait(for: [done], timeout: 5)
    }

    /// A search field lent to the window's toolbar, which no page chrome
    /// styles, takes the node's side and base, and a change of them.
    func testAFieldLentToTheToolbarTakesTheDirection() throws {
        let (p, node) = field("input", "Hello", props: ["type": "search"], style: ["direction": "ltr"])
        let bar = try XCTUnwrap(node.lendFieldToBar())
        XCTAssertEqual(bar.baseWritingDirection, .leftToRight)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "rtl"]]]))
        XCTAssertEqual(bar.alignment, .right)
        XCTAssertEqual(bar.baseWritingDirection, .rightToLeft)
    }

    /// Test 5: a change of `direction` restyles the text there and what is
    /// typed next, and leaves the selection.
    func testADirectionChangeRestylesAndKeepsTheSelection() throws {
        for kind in ["input", "textarea"] {
            let (p, node) = field(kind, "Hello مرحبا", style: ["direction": "rtl"])
            let tv = try editor(node)
            tv.setSelectedRange(NSRange(location: 3, length: 2))
            p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "ltr"]]]))
            let now = try editor(node)
            XCTAssertEqual(base(now), .leftToRight, kind)
            XCTAssertEqual(now.selectedRange(), NSRange(location: 3, length: 2), kind)
            now.setSelectedRange(NSRange(location: (now.string as NSString).length, length: 0))
            now.insertText("x", replacementRange: now.selectedRange())
            XCTAssertEqual(base(now, at: (now.string as NSString).length - 1), .leftToRight, "\(kind): what is typed next")
            if kind == "input" {
                // The field itself takes the new side once its editing ends.
                XCTAssertTrue(window?.makeFirstResponder(nil) ?? false)
                XCTAssertNil(node.field?.currentEditor())
                RunLoop.main.run(until: Date().addingTimeInterval(0.05))
                XCTAssertEqual(node.field?.alignment, .left, "the field, after its editing")
                XCTAssertEqual(node.field?.baseWritingDirection, .leftToRight)
            }
            window?.close()
        }
    }

    /// Test 6: a plain text area's placeholder sits at its start, the right
    /// under `rtl`.
    func testAPlainTextAreasPlaceholderSitsAtItsStart() throws {
        for (direction, right) in [("rtl", true), ("ltr", false)] {
            let (_, node) = field("textarea", "", props: ["placeholder": "Type here"], style: ["direction": direction])
            let tv = try XCTUnwrap(node.textArea)
            let rep = try XCTUnwrap(tv.bitmapImageRepForCachingDisplay(in: tv.bounds))
            tv.cacheDisplay(in: tv.bounds, to: rep)
            // The placeholder's ink: pixels apart from the field's ground (light or dark), above its empty bottom.
            let ground = rep.colorAt(x: rep.pixelsWide / 2, y: rep.pixelsHigh - 6)?.usingColorSpace(.deviceRGB)?.brightnessComponent ?? 1
            var xs: [Int] = []
            for py in 2..<min(rep.pixelsHigh, 40) {
                let ink = (4..<(rep.pixelsWide - 4)).filter { px in
                    guard let c = rep.colorAt(x: px, y: py)?.usingColorSpace(.deviceRGB) else { return false }
                    return abs(c.brightnessComponent - ground) > 0.2
                }
                if ink.count < rep.pixelsWide / 2 { xs += ink }
            }
            XCTAssertFalse(xs.isEmpty, "\(direction): the placeholder drew")
            let middle = xs.isEmpty ? 0 : xs.reduce(0, +) / xs.count
            if right { XCTAssertGreaterThan(middle, rep.pixelsWide / 2, "rtl: at the right, its ink's middle \(middle) of \(rep.pixelsWide)") }
            else { XCTAssertLessThan(middle, rep.pixelsWide / 2, "ltr: at the left, its ink's middle \(middle) of \(rep.pixelsWide)") }
            window?.close()
        }
    }
}
#endif

// Authored CSS line boxes remain fractional before rasterization.
// @ref LLP 1008 §3, LLP 1001 §5
import XCTest
import CoreText
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

final class TextGeometryTests: XCTestCase {
    private let engine = TextEngine(resolve: { _ in nil })

    func testTextCacheRecencyAndCheckpointRemainIndependent() {
        var cache = TextCache<Int, String>()
        for i in 0..<4096 { cache.put(i, "value-\(i)") }
        var checkpoint = cache
        for i in 0..<512 { XCTAssertEqual(cache.get(i), "value-\(i)") }
        cache.put(4096, "new")
        XCTAssertEqual(cache.get(0), "value-0")
        XCTAssertNil(cache.get(512), "the oldest untouched entries are evicted")
        XCTAssertEqual(checkpoint.get(512), "value-512")
        XCTAssertNil(checkpoint.get(4096))
        cache.put(0, "changed")
        XCTAssertEqual(checkpoint.get(0), "value-0")
        XCTAssertLessThanOrEqual(cache.count, 4096)
    }

    #if os(macOS)
    func testEmptyContainerReleasesPaintAndKeepsItsChildren() {
        let presenter = Presenter()
        let container = NodeView(id: 1, kind: "view", presenter: presenter)
        let child = NodeView(id: 2, kind: "text", presenter: presenter)
        container.addSubview(child)
        container.applyStyle(["background_color": [1.0, 0.0, 0.0, 1.0]])
        XCTAssertFalse(container.wantsUpdateLayer)
        // The previous bitmap must not survive removal of the decoration.
        container.layer?.contents = NSImage(size: NSSize(width: 20, height: 20))
        container.applyStyle([:])
        XCTAssertTrue(container.wantsUpdateLayer)
        container.updateLayer()
        XCTAssertNil(container.layer?.contents)
        XCTAssertTrue(child.superview === container)
        container.applyStyle(["border_width_left": 2.0])
        XCTAssertFalse(container.wantsUpdateLayer)
        XCTAssertFalse(child.wantsUpdateLayer)
        for kind in ["image", "canvas", "iframe"] {
            let node = NodeView(id: 3, kind: kind, presenter: presenter)
            node.applyStyle([:])
            XCTAssertFalse(node.wantsUpdateLayer)
        }
    }

    func testInlineTextDefersLayersAndCanBecomeAParagraphAgain() {
        let presenter = Presenter()
        let clipping: [[Any]] = [["M", [0.0, 0.0]], ["L", [100.0, 0.0]], ["L", [0.0, 30.0]], ["Z", [Double]()]]
        let style: [String: Any] = ["z_index": 3.0, "clip_path": clipping]
        func apply(_ ops: [[String: Any]]) {
            presenter.apply(Batch(ops: ops, timers: false, motion: false, clock: nil, error: nil))
        }
        apply([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text"],
            ["op": "create", "id": 3, "kind": "text", "style": style, "props": ["text": "retained run"]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
        ])
        let inline = presenter.views[3]!
        XCTAssertFalse(inline.wantsLayer)
        XCTAssertNil(inline.layer)
        XCTAssertEqual(presenter.views[2]!.paragraphSpec().runs.first?.text, "retained run")
        apply([["op": "style", "id": 3, "style": style]])
        XCTAssertNil(inline.layer, "restyling an inline run must not allocate a layer")
        apply([
            ["op": "children", "id": 2, "ids": []],
            ["op": "children", "id": 1, "ids": [2, 3]],
        ])
        XCTAssertTrue(presenter.views[3] === inline)
        XCTAssertTrue(inline.wantsLayer)
        XCTAssertEqual(inline.layer?.zPosition, 3)
        XCTAssertNotNil(inline.layer?.mask)
        XCTAssertTrue(inline.isParagraph)
        apply([
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
        ])
        XCTAssertNil(inline.layer)
        XCTAssertEqual(presenter.views[2]!.paragraphSpec().runs.first?.text, "retained run")
    }

    func testScrollPaintOnlyInvalidatesNewlyExposedText() {
        let original = NSRect(x: 0, y: 0, width: 300, height: 200)
        XCTAssertEqual(Presenter.exposedTextRects(original, after: nil), [original])
        XCTAssertTrue(Presenter.exposedTextRects(original, after: original).isEmpty)
        for delta in [NSPoint(x: 0, y: 20), NSPoint(x: 0, y: -20), NSPoint(x: 30, y: 25), NSPoint(x: 400, y: 500)] {
            let next = original.offsetBy(dx: delta.x, dy: delta.y)
            let exposed = Presenter.exposedTextRects(next, after: original)
            let overlap = next.intersection(original)
            let overlapArea = overlap.isEmpty ? 0 : overlap.width * overlap.height
            XCTAssertEqual(exposed.reduce(0) { $0 + $1.width * $1.height }, next.width * next.height - overlapArea)
            for (i, rect) in exposed.enumerated() {
                XCTAssertTrue(next.contains(rect))
                XCTAssertFalse(rect.intersects(original))
                for other in exposed.dropFirst(i + 1) { XCTAssertFalse(rect.intersects(other)) }
            }
        }
        XCTAssertTrue(Presenter.exposedTextRects(original.insetBy(dx: 10, dy: 10), after: original).isEmpty)
        XCTAssertTrue(Presenter.exposedTextRects(.zero, after: original).isEmpty)
    }

    func testLogicalSelectionSurvivesRetiredRowsAndClearsWhenKeysDisappear() {
        let presenter = Presenter()
        let list = NodeView(id: 1, kind: "list", presenter: presenter)
        presenter.root.addSubview(list)
        presenter.views[1] = list
        var keys = ["s:first": 0, "s:middle": 1, "s:last": 2]
        presenter.onListIndex = { _, key in keys[key] }
        var copiedAll = false
        presenter.onListText = { _, first, last in
            copiedAll = first == nil && last == nil
            if let first, let last {
                XCTAssertEqual(first.0, "s:first"); XCTAssertEqual(first.2, 0)
                XCTAssertEqual(last.0, "s:last"); XCTAssertEqual(last.2, 4)
            }
            return "first\n\nmiddle\n\nlast"
        }
        func row(_ id: UInt32, _ key: String, _ position: Int, _ text: String) -> NodeView {
            let wrapper = NodeView(id: id, kind: "view", presenter: presenter)
            wrapper.applyProps(set: ["listItemKey": key, "accessibilityPosInSet": String(position)], clear: [])
            list.container.addSubview(wrapper)
            let node = NodeView(id: id + 1, kind: "text", presenter: presenter)
            node.applyProps(set: ["text": text], clear: [])
            node.frame = NSRect(x: 0, y: 0, width: 200, height: 20)
            wrapper.addSubview(node)
            return node
        }
        weak var retired: NodeView?
        autoreleasepool {
            let first = row(10, "s:first", 1, "first")
            retired = first
            let down = NSEvent.mouseEvent(with: .leftMouseDown, location: first.convert(.zero, to: nil), modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
            presenter.selection.begin(first, event: down)
            first.forget()
            first.superview?.removeFromSuperview()
            presenter.selection.structureChanged()
        }
        XCTAssertNil(retired, "selection stores a logical endpoint, not a retained view")
        let last = row(20, "s:last", 3, "last")
        presenter.selection.structureChanged()
        let drag = NSEvent.mouseEvent(with: .leftMouseDragged, location: last.convert(NSPoint(x: 20, y: 21), to: nil), modifierFlags: [], timestamp: 1, windowNumber: 0, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        presenter.selection.drag(drag)
        XCTAssertEqual(presenter.selection.range(last), NSRange(location: 0, length: 4))
        XCTAssertEqual(presenter.selection.selectedText(), "first\n\nmiddle\n\nlast")
        XCTAssertFalse(copiedAll)
        presenter.selection.selectAll()
        XCTAssertEqual(presenter.selection.selectedText(), "first\n\nmiddle\n\nlast")
        XCTAssertTrue(copiedAll)
        keys.removeAll()
        presenter.selection.structureChanged()
        XCTAssertNil(presenter.selection.range(last))
        XCTAssertEqual(presenter.selection.selectedText(), "")
    }

    func testParagraphVisibilityTracksScrollWithoutIndexingAgain() {
        let presenter = Presenter()
        presenter.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        presenter.root.frame = NSRect(x: 0, y: 0, width: 400, height: 30_000)
        var nodes: [NodeView] = []
        for i in 0..<1000 {
            let node = NodeView(id: UInt32(i + 1), kind: "text", presenter: presenter)
            node.frame = NSRect(x: 0, y: i * 30, width: 300, height: 25)
            presenter.root.addSubview(node)
            nodes.append(node)
        }
        let index = TextViewportIndex(nodes)
        XCTAssertEqual(index.candidates().map(\.id), Array(1...10).map(UInt32.init))
        presenter.viewport.contentView.scroll(to: NSPoint(x: 0, y: 15_000))
        XCTAssertEqual(index.candidates().map(\.id), Array(501...510).map(UInt32.init))
        presenter.viewport.contentView.scroll(to: NSPoint(x: 0, y: 29_700))
        XCTAssertEqual(index.candidates().map(\.id), Array(991...1000).map(UInt32.init))
        presenter.viewport.contentView.scroll(to: .zero)
        XCTAssertEqual(index.candidates().map(\.id), Array(1...10).map(UInt32.init))
    }

    func testParagraphVisibilityIncludesTallOverlapsAndNestedScrollers() {
        let presenter = Presenter()
        presenter.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        presenter.root.frame = NSRect(x: 0, y: 0, width: 400, height: 4000)
        let tall = NodeView(id: 1, kind: "text", presenter: presenter)
        tall.frame = NSRect(x: 0, y: 0, width: 200, height: 3000)
        presenter.root.addSubview(tall)
        let short = NodeView(id: 2, kind: "text", presenter: presenter)
        short.frame = NSRect(x: 0, y: 100, width: 200, height: 25)
        presenter.root.addSubview(short)
        let nested = NSScrollView(frame: NSRect(x: 0, y: 1600, width: 300, height: 100))
        let document = FlippedView(frame: NSRect(x: 0, y: 0, width: 300, height: 2000))
        nested.documentView = document
        presenter.root.addSubview(nested)
        let inner = NodeView(id: 3, kind: "text", presenter: presenter)
        inner.frame = NSRect(x: 0, y: 1000, width: 200, height: 25)
        document.addSubview(inner)
        let index = TextViewportIndex([tall, short, inner])
        presenter.viewport.contentView.scroll(to: NSPoint(x: 0, y: 1500))
        XCTAssertEqual(index.candidates().map(\.id), [1])
        nested.contentView.scroll(to: NSPoint(x: 0, y: 1000))
        XCTAssertEqual(Set(index.candidates().map(\.id)), [1, 3])
        presenter.viewport.contentView.scroll(to: NSPoint(x: 0, y: 2000))
        XCTAssertEqual(index.candidates().map(\.id), [1])
    }
    #endif

    private func spec(_ text: String) -> Spec {
        Spec(runs: [Run(text: text, size: 13.25, weight: 400, family: 0,
                        italic: false, lineHeight: 18.125, letterSpacing: 0)],
             align: 0, lineClamp: 0, color: [0, 0, 0, 255])
    }

    func testNormalIncludesTheShapedFallbackFontsMetrics() {
        var input = spec("🧙🏽‍♀️👨‍👩‍👧‍👦")
        input.runs[0].lineHeight = nil
        let p = engine.paragraph(input, width: 500)
        var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
        _ = CTLineGetTypographicBounds(p.lines[0], &ascent, &descent, &leading)
        XCTAssertGreaterThanOrEqual(p.height, ceil(ascent + descent + leading))
    }

    func testFixedMixedFontLinesKeepSharedBaselineExtents() {
        let base = Run(text: "", size: 20, weight: 400, family: 0, italic: false, lineHeight: 20, letterSpacing: 0)
        var child = base; child.text = "Larger"; child.size = 40
        let spec = Spec(runs: [child], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: base)
        let p = engine.paragraph(spec, width: 500)
        XCTAssertGreaterThan(p.height, 20)
        XCTAssertEqual(p.lineBottoms.last!, p.height, accuracy: 0.02)
    }

    func testCoalescedGlyphRunsPreserveEveryAuthoredLineHeight() {
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: 20, letterSpacing: 0)
        for heights: [CGFloat] in [[20, 60], [60, 20]] {
            var first = base; first.text = "before "; first.lineHeight = heights[0]
            var second = base; second.text = "after"; second.lineHeight = heights[1]
            var input = Spec(runs: [first, second], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: base)
            let joined = engine.paragraph(input, width: 500)
            XCTAssertEqual((CTLineGetGlyphRuns(joined.lines[0]) as! [CTRun]).count, 1)
            XCTAssertEqual(joined.height, 60, accuracy: 0.02)
            input.runs[1].color = [255, 0, 0, 255]
            let colored = engine.paragraph(input, width: 500)
            XCTAssertEqual((CTLineGetGlyphRuns(colored.lines[0]) as! [CTRun]).count, 2)
            XCTAssertEqual(colored.height, joined.height, accuracy: 0.02)
            XCTAssertEqual(colored.firstBaseline, joined.firstBaseline, accuracy: 0.02)
        }
    }

    func testNormalParagraphPreservesFractionalExplicitChildBoxes() {
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
        for (text, count) in [("Child", 1), ("First\nSecond", 2)] {
            var child = base; child.text = text; child.lineHeight = 60.25
            let input = Spec(runs: [child], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: base)
            let paragraph = engine.paragraph(input, width: 500)
            XCTAssertEqual(paragraph.height, 60.25 * CGFloat(count), accuracy: 0.02)
            XCTAssertEqual(paragraph.lineBottoms.last!, paragraph.height, accuracy: 0.02)
        }
        var child = base; child.text = "Child"
        var input = Spec(runs: [child], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: base)
        let normal = engine.paragraph(input, width: 500).height
        input.runs[0].lineHeight = 10
        XCTAssertEqual(engine.paragraph(input, width: 500).height, normal, accuracy: 0.02)
    }

    func testLineHeightKindsAndParagraphMinimum() {
        let session = ExactApp.shared.makeSession(label: "line-height")
        defer { session.destroy() }
        let node = NodeView(id: 999, kind: "text", presenter: session.presenter)
        node.style = ["font_size": 20.0, "line_height": 1.5]
        XCTAssertEqual(node.usedLineHeight, 30)
        node.style["line_height"] = "24px"
        XCTAssertEqual(node.usedLineHeight, 24)
        node.style["line_height"] = "normal"
        XCTAssertNil(node.usedLineHeight)
        for height: CGFloat in [0, 25.25] {
            let run = Run(text: "hello", size: 16, weight: 400, family: 0, italic: false, lineHeight: height, letterSpacing: 0)
            var spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run)
            XCTAssertEqual(engine.paragraph(spec, width: 500).height, height, accuracy: 0.02)
            if height > 0 {
                spec.runs[0].size = 8
                spec.runs[0].lineHeight = 12
                XCTAssertEqual(engine.paragraph(spec, width: 500).height, height, accuracy: 0.02)
            }
        }
    }

    func testAuthoredLineHeightSurvivesEmptyMultilineAndClampedText() {
        for (text, count) in [("", 1), ("Fraction", 1), ("First\nSecond", 2)] {
            let value = spec(text)
            let paragraph = engine.paragraph(value, width: 300)
            XCTAssertEqual(paragraph.height, 18.125 * CGFloat(count), accuracy: 0.000001)
            XCTAssertEqual(engine.paragraph(value, width: 300).height, paragraph.height)
        }
        var clamped = spec("First\nSecond\nThird")
        clamped.lineClamp = 2
        XCTAssertEqual(engine.paragraph(clamped, width: 300).height, 36.25, accuracy: 0.000001)
    }

    func testFractionalLineHeightEditsAndColorChangesKeepTheirGeometry() {
        let value = spec("First\nSecond")
        let before = engine.paragraph(value, width: 300)
        var edited = value
        edited.runs[0].lineHeight! += 0.25
        XCTAssertEqual(engine.paragraph(edited, width: 300).height - before.height, 0.5, accuracy: 0.000001)
        edited.color = [200, 20, 30, 255]
        XCTAssertEqual(engine.paragraph(edited, width: 300).height, 36.75, accuracy: 0.000001)
    }

    func testOverflowWrappingAndIntrinsicWidthsKeepDistinctPolicies() {
        var value = spec(String(repeating: "W", count: 30))
        let normal = engine.paragraph(value, width: 80)
        XCTAssertEqual(normal.lines.count, 1)
        XCTAssertGreaterThan(normal.width, 80)
        let minimum = engine.minContentWidth(value)
        value.overflowWrap = 1
        let broken = engine.paragraph(value, width: 80)
        XCTAssertGreaterThan(broken.lines.count, 1)
        XCTAssertLessThanOrEqual(broken.width, 80)
        XCTAssertEqual(engine.minContentWidth(value), minimum)
        value.overflowWrap = 2
        XCTAssertEqual(engine.paragraph(value, width: 80).height, broken.height)
        XCTAssertLessThan(engine.minContentWidth(value), minimum / 10)
        value.overflowWrap = 0
        XCTAssertTrue(engine.paragraph(value, width: 80) === normal)
    }

    func testEmergencyWrapKeepsComposedCharactersAndOrdinaryWordBreaks() {
        for text in ["👨‍👩‍👧‍👦👨‍👩‍👧‍👦👨‍👩‍👧‍👦", "e\u{301}e\u{301}e\u{301}e\u{301}"] {
            var value = spec(text)
            value.overflowWrap = 2
            let string = text as NSString
            for line in engine.paragraph(value, width: 15).lines {
                let range = CTLineGetStringRange(line)
                let end = range.location + range.length
                if end < string.length {
                    XCTAssertEqual(string.rangeOfComposedCharacterSequence(at: end).location, end)
                }
            }
        }
        let value = spec("One two three four")
        var wrapped = value; wrapped.overflowWrap = 1
        XCTAssertEqual(engine.paragraph(value, width: 65).lines.map { CTLineGetStringRange($0).length },
                       engine.paragraph(wrapped, width: 65).lines.map { CTLineGetStringRange($0).length })
    }

    private func ellipses(_ line: CTLine) -> Int {
        (CTLineGetGlyphRuns(line) as! [CTRun]).reduce(0) { result, run in
            let attrs = CTRunGetAttributes(run) as NSDictionary
            let font = attrs[kCTFontAttributeName] as! CTFont
            var character: UniChar = 0x2026, glyph: CGGlyph = 0
            guard CTFontGetGlyphsForCharacters(font, &character, &glyph, 1) else { return result }
            var glyphs = [CGGlyph](repeating: 0, count: CTRunGetGlyphCount(run))
            CTRunGetGlyphs(run, CFRange(location: 0, length: 0), &glyphs)
            return result + glyphs.filter { $0 == glyph }.count
        }
    }

    func testClampedTextAddsOneEllipsisOnlyWhenContentIsHidden() {
        for text in ["A quoted reply wraps within its padding and keeps every line beside the border.",
                     "First\nSecond\nThird", "Hello 👨‍👩‍👧‍👦 café 世界. More text follows the family emoji."] {
            var value = spec(text)
            value.lineClamp = 2
            let paragraph = engine.paragraph(value, width: 145)
            XCTAssertEqual(paragraph.lines.count, 2)
            XCTAssertEqual(ellipses(paragraph.lines[0]), 0)
            XCTAssertEqual(ellipses(paragraph.lines[1]), 1, text)
            XCTAssertEqual(paragraph.height, 36.25, accuracy: 0.000001)
            XCTAssertLessThanOrEqual(CTLineGetTypographicBounds(paragraph.lines[1], nil, nil, nil), 145)
        }
        for text in ["", "Fits", "First\nSecond"] {
            var value = spec(text)
            value.lineClamp = 2
            XCTAssertEqual(engine.paragraph(value, width: 300).lines.reduce(0) { $0 + ellipses($1) }, 0)
        }
        var narrow = spec("Narrow text to hide")
        narrow.lineClamp = 1
        XCTAssertEqual(ellipses(engine.paragraph(narrow, width: 6).lines[0]), 0)
    }

    func testClampedInlineRunsKeepOriginalIndicesAndRecomputeAfterWidening() {
        var value = spec("First\nSecond colored link with more words to hide")
        let first = value.runs[0]
        value.runs[0].text = "First\n"
        var colored = first
        colored.text = "Second colored link with more words to hide"
        colored.color = [255, 0, 0, 255]
        colored.weight = 600
        colored.href = "example.md"
        value.runs.append(colored)
        value.lineClamp = 2
        let narrow = engine.paragraph(value, width: 145)
        XCTAssertEqual(ellipses(narrow.lines[1]), 1)
        for run in CTLineGetGlyphRuns(narrow.lines[1]) as! [CTRun] {
            XCTAssertGreaterThanOrEqual(CTRunGetStringRange(run).location, 6)
        }
        XCTAssertEqual(ellipses(engine.paragraph(value, width: 500).lines[1]), 0)
        XCTAssertEqual(ellipses(engine.paragraph(value, width: 145).lines[1]), 1)
        value.lineClamp = 0
        XCTAssertEqual(engine.paragraph(value, width: 145).lines.reduce(0) { $0 + ellipses($1) }, 0)
    }

    #if os(macOS)
    func testSelectionCommandSelectsTheEditorCreatedInItsBatch() throws {
        final class Delegate: ExactSessionDelegate {
            var commands: [String] = []
            func exactSession(_ session: ExactSession, command name: String, args: [Any]) { commands.append(name) }
        }
        _ = NSApplication.shared
        let delegate = Delegate()
        let session = ExactApp.shared.makeSession(delegate: delegate, label: "selection-command")
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 100),
                              styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = session.presenter.viewport
        defer { session.destroy(); window.close() }
        let value = "First line\nThird café 👩🏽‍💻"
        // A command may precede its target in the wire batch: delivery waits
        // for the complete batch, then stays inside this session.
        session.apply(Batch(ops: [
            ["op": "command", "name": "selectText", "args": ["message"]],
            ["op": "create", "id": 1, "kind": "textarea", "props": ["id": "message", "editable": "false", "value": value]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
            ["op": "command", "name": "productAction", "args": []],
        ], timers: false, motion: false, clock: nil, error: nil))
        let editor = try XCTUnwrap(session.presenter.views[1]?.textArea)
        XCTAssertFalse(editor.isEditable)
        XCTAssertTrue(window.firstResponder === editor)
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 0, length: (value as NSString).length))
        XCTAssertEqual(delegate.commands, ["productAction"])
    }

    func testPaddedParagraphWrapsLikeTextInsideTheEquivalentContainer() throws {
        let session = ExactApp.shared.makeSession(label: "text-padding")
        defer { session.destroy() }
        let node = NodeView(id: 1, kind: "text", presenter: session.presenter)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 80)
        node.props = ["text": "A quoted reply wraps within its padding and keeps every line beside the border."]
        node.style = ["font_size": 16.0, "line_height": "20px", "padding_left": 8.0,
                      "padding_right": 12.0, "padding_top": 11.0, "padding_bottom": 7.0,
                      "border_width": 2.0]
        func breaks(_ paragraph: Paragraph) -> [Int] {
            paragraph.lines.map { CTLineGetStringRange($0).length }
        }
        let first = try XCTUnwrap(node.paragraphLayout())
        XCTAssertEqual(first.lines.count, 3)
        XCTAssertEqual(breaks(first), breaks(session.text.paragraph(node.paragraphSpec(), width: 276)))
        // The outer frame is unchanged; the cache must still use the new content width.
        node.style["padding_left"] = 24.0
        let changed = try XCTUnwrap(node.paragraphLayout())
        XCTAssertFalse(first === changed)
        XCTAssertEqual(breaks(changed), breaks(session.text.paragraph(node.paragraphSpec(), width: 260)))
        node.style["padding_left"] = 8.0
        XCTAssertEqual(breaks(try XCTUnwrap(node.paragraphLayout())), breaks(first))
        node.style = ["font_size": 16.0, "line_height": "20px"]
        XCTAssertEqual(breaks(try XCTUnwrap(node.paragraphLayout())),
                       breaks(session.text.paragraph(node.paragraphSpec(), width: 300)))
    }

    func testPaddedAlignedLinksAndSelectionUseThePaintedLine() throws {
        final class Delegate: ExactSessionDelegate {
            var urls: [String] = []
            func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
                if name == "openURL", let url = args.first as? String { urls.append(url) }
            }
        }
        let delegate = Delegate()
        let session = ExactApp.shared.makeSession(delegate: delegate, label: "text-hit")
        defer { session.destroy() }
        let node = NodeView(id: 1, kind: "text", presenter: session.presenter)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 100)
        node.props = ["text": "First line\nSecond link", "href": "example.md"]
        session.presenter.root.addSubview(node)
        for (alignment, flush) in [("left", 0.0), ("center", 0.5), ("right", 1.0)] {
            node.style = ["font_size": 16.0, "line_height": "20px", "text_align": alignment,
                          "padding_left": 20.0, "padding_top": 25.0, "padding_right": 30.0,
                          "border_width": 2.0]
            node.invalidateText()
            let paragraph = try XCTUnwrap(node.paragraphLayout())
            let line = paragraph.lines[1]
            let x = 22 + CGFloat(CTLineGetPenOffsetForFlush(line, flush, 246))
            func click(_ point: CGPoint, count: Int = 1) throws {
                let event = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown,
                    location: node.convert(point, to: nil), modifierFlags: [], timestamp: 0,
                    windowNumber: 0, context: nil, eventNumber: 0, clickCount: count, pressure: 1))
                session.presenter.selection.begin(node, event: event)
                session.presenter.selection.end(node, event: event)
            }
            let before = delegate.urls.count
            try click(CGPoint(x: 2, y: 57)) // Left border/padding.
            try click(CGPoint(x: x + 4, y: 10)) // Above the first line.
            XCTAssertEqual(delegate.urls.count, before)
            try click(CGPoint(x: x + 4, y: 57)) // Second line's actual ink.
            XCTAssertEqual(delegate.urls.count, before + 1)
            try click(CGPoint(x: x + 4, y: 57), count: 2)
            XCTAssertEqual(session.presenter.selection.range(node), NSRange(location: 11, length: 6))
        }
    }
    #endif
}

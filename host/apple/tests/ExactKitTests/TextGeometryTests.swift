// Authored CSS line boxes remain fractional before rasterization.
// @ref LLP 1008 §3, LLP 1001 §5
import XCTest
import CoreText
import CExact
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

final class TextGeometryTests: XCTestCase {
    private let engine = TextEngine(resolve: { _ in nil })

    func testTextCacheRecencyAndCheckpointRemainIndependent() {
        // Trunk's fixture named the retired generic cache. Exercise the current
        // residency's bounded identity LRU and copy-on-write scalar checkpoint.
        func value(_ text: String) -> Spec {
            var result = spec(text); result.strut = result.runs[0]
            return result
        }
        var cache = TextResidency()
        let identities = (0..<4096).map { i in
            let identity = cache.identity(value("value-\(i)"))
            cache.putMinimum(identity, width: CGFloat(i))
            return identity
        }
        func get(_ cache: inout TextResidency, _ text: String) -> TextIdentity? {
            var answer: TextIdentity?
            withBorrowedRequest(value(text)) { answer = cache.borrowedIdentity($0) }
            return answer
        }
        var checkpoint = cache
        for i in 0..<512 { XCTAssertTrue(get(&cache, "value-\(i)") === identities[i]) }
        let added = cache.identity(value("new"))
        XCTAssertTrue(get(&cache, "value-0") === identities[0])
        XCTAssertNil(get(&cache, "value-512"), "the oldest untouched entries are evicted")
        XCTAssertTrue(get(&checkpoint, "value-512") === identities[512])
        XCTAssertNil(get(&checkpoint, "new"))
        XCTAssertTrue(get(&cache, "new") === added)
        cache.putMinimum(identities[0], width: -1)
        XCTAssertEqual(cache.minimum(identities[0]), -1)
        XCTAssertEqual(checkpoint.minimum(identities[0]), 0)
        XCTAssertLessThanOrEqual(cache.stats.identityEntries, 4096)
    }

    #if os(macOS)
    func testEmptyContainerReleasesPaintAndKeepsItsChildren() {
        let presenter = Presenter()
        let container = NodeView(id: 1, kind: "view", presenter: presenter)
        container.frame = NSRect(x: 0, y: 0, width: 100, height: 40)
        let child = NodeView(id: 2, kind: "text", presenter: presenter)
        container.addSubview(child)
        container.applyStyle(["background_color": [1.0, 0.0, 0.0, 1.0]])
        // A background is the layer's own colour, not a bitmap (`BoxLayerMac.swift`).
        XCTAssertTrue(container.wantsUpdateLayer)
        container.applyStyle(["background_color": [1.0, 0.0, 0.0, 1.0], "border_width_left": 2.0, "border_width_top": 1.0,
                              "border_color_left": [0.0, 0.0, 255.0, 255.0], "border_color_top": [255.0, 0.0, 0.0, 255.0]])
        XCTAssertFalse(container.wantsUpdateLayer, "sides in two colours draw")
        // The previous bitmap must not survive removal of the decoration.
        container.layer?.contents = NSImage(size: NSSize(width: 20, height: 20))
        container.applyStyle([:])
        XCTAssertTrue(container.wantsUpdateLayer)
        container.updateLayer()
        XCTAssertNil(container.layer?.contents)
        XCTAssertTrue(child.superview === container)
        container.applyStyle(["border_width_left": 2.0, "border_color_left": [0.0, 0.0, 255.0, 255.0], "border_radius_top_left": 4.0])
        XCTAssertFalse(container.wantsUpdateLayer, "a rounded one-sided border draws")
        XCTAssertFalse(child.wantsUpdateLayer)
        for kind in ["canvas", "iframe"] {
            let node = NodeView(id: 3, kind: kind, presenter: presenter)
            node.applyStyle([:])
            XCTAssertFalse(node.wantsUpdateLayer)
        }
    }

    func testInlineTextIsDataAndCanBecomeAParagraphAgain() {
        let presenter = Presenter()
        let clipping: [String: Any] = ["rule": "nonzero", "commands": [["M", [0.0, 0.0]], ["L", [100.0, 0.0]], ["L", [0.0, 30.0]], ["Z", [Double]()]]]
        // A positioned box: CSS's used `z-index` is the row's (LLP 1074 T1).
        let style: [String: Any] = ["position_type": "relative", "z_index": 3.0, "clip_path": clipping]
        let run: [String: Any] = ["id": 3, "parent": 2, "paint": true, "style": style,
                                  "props": ["text": "retained run", "testId": "run"], "handlers": ["press"]]
        func apply(_ ops: [[String: Any]]) {
            presenter.apply(batchFixture(ops: ops, timers: false, motion: false, clock: nil, error: nil))
        }
        apply([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text"],
            ["op": "paragraph", "id": 2, "runs": [run]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
        ])
        XCTAssertNil(presenter.views[3])
        XCTAssertEqual(presenter.inlineText(3)?.props["testId"], "run")
        XCTAssertEqual(presenter.inlineText(3)?.handlers, ["press"])
        XCTAssertEqual(presenter.views[2]!.paragraphSpec().runs.first?.text, "retained run")
        apply([["op": "paragraph", "id": 2, "runs": [run]]])
        XCTAssertNil(presenter.views[3], "restyling cannot allocate a run view")
        apply([
            ["op": "paragraph", "id": 2, "runs": []],
            ["op": "create", "id": 3, "kind": "text", "style": style, "props": ["text": "retained run"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "rank", "id": 3, "rank": 6],
        ])
        let paragraph = presenter.views[3]!
        XCTAssertNil(presenter.inlineText(3))
        XCTAssertTrue(paragraph.wantsLayer)
        XCTAssertEqual(paragraph.layer?.zPosition, 0.001)
        XCTAssertNotNil(paragraph.layer?.mask)
        XCTAssertTrue(paragraph.isParagraph)
        apply([
            ["op": "destroy", "id": 3],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "paragraph", "id": 2, "runs": [run]],
        ])
        XCTAssertNil(presenter.views[3])
        XCTAssertEqual(presenter.views[2]!.paragraphSpec().runs.first?.text, "retained run")
        apply([["op": "destroy", "id": 2]])
        XCTAssertNil(presenter.inlineText(3))
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

    func testUpcomingParagraphsAreOrderedByDistanceInsteadOfHeight() {
        let presenter = Presenter()
        presenter.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        presenter.root.frame = NSRect(x: 0, y: 0, width: 400, height: 4000)
        func node(_ id: UInt32, _ top: CGFloat, _ height: CGFloat) -> NodeView {
            let node = NodeView(id: id, kind: "text", presenter: presenter)
            node.frame = NSRect(x: 0, y: top, width: 200, height: height)
            presenter.root.addSubview(node)
            return node
        }
        let above = node(1, 970, 20)
        let near = node(2, 1320, 20)
        let far = node(3, 1500, 1000)
        let index = TextViewportIndex([far, near, above])
        presenter.viewport.contentView.scroll(to: NSPoint(x: 0, y: 1000))
        XCTAssertEqual(index.candidates(reach: 1600).map(\.id), [1, 2, 3])
        // Reverse movement changes priority without rebuilding the index.
        presenter.viewport.contentView.scroll(to: NSPoint(x: 0, y: 1400))
        XCTAssertEqual(index.candidates(reach: 1600).map(\.id), [3, 2, 1])
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

    func spec(_ text: String) -> Spec {
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
        session.apply(batchFixture(ops: [
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

    func testBlockLinksOpenWithoutAHandlerAndRespectAuthoredPressAndDisabled() {
        final class Delegate: ExactSessionDelegate {
            var urls: [String] = []
            func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
                if name == "openURL", let url = args.first as? String { urls.append(url) }
            }
        }
        let delegate = Delegate()
        let session = ExactApp.shared.makeSession(delegate: delegate, label: "block-link")
        defer { session.destroy() }
        let p = session.presenter
        let link = NodeView(id: 9001, kind: "button", presenter: p)
        p.root.addSubview(link); p.views[link.id] = link
        link.applyProps(set: ["href": "https://example.test/article"], clear: [])
        p.press(link.id)
        XCTAssertEqual(delegate.urls, ["https://example.test/article"])
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        link.handlers = ["press"]
        p.press(link.id)
        XCTAssertEqual(presses, [link.id])
        XCTAssertEqual(delegate.urls.count, 1)
        link.handlers = []
        link.applyProps(set: ["disabled": "true"], clear: [])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.count, 1)
        link.applyProps(set: ["inert": "true"], clear: ["disabled"])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.count, 1)
        link.applyProps(set: ["href": "//example.test/relative"], clear: ["inert"])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.last, "//example.test/relative")
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
            // The line break is a preserved segment break (CSS; LLP 1053 G5).
            node.style = ["font_size": 16.0, "line_height": "20px", "text_align": .string(alignment),
                          "padding_left": 20.0, "padding_top": 25.0, "padding_right": 30.0,
                          "border_width": 2.0, "white_space": "pre-wrap"]
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

// Viewport culling must be indistinguishable from painting every shaped line.
extension TextGeometryTests {
    private func checkedStrutParagraph(_ input: Spec, width: CGFloat) -> (Paragraph, Int) {
        let e = TextEngine(resolve: { _ in nil })
        #if STRUT_EXTENTS_SENTINEL
        strutExtentComputations = 0
        #endif
        let p = e.paragraph(input, width: width)
        var calls = 0
        #if STRUT_EXTENTS_SENTINEL
        calls = strutExtentComputations
        // The standalone fixture copies the unchanged original layout body;
        // no counter or reference entry is present in the production module.
        let original = e.originalStrutParagraph(input, width: width)
        func equal(_ a: CGFloat, _ b: CGFloat) {
            if a.isNaN { XCTAssertTrue(b.isNaN) }
            else { XCTAssertEqual(Double(a).bitPattern, Double(b).bitPattern) }
        }
        equal(p.width, original.width); equal(p.height, original.height)
        XCTAssertEqual(p.baselines.count, original.baselines.count)
        for (a, b) in zip(p.baselines, original.baselines) { equal(a, b) }
        XCTAssertEqual(p.lineBottoms.count, original.lineBottoms.count)
        for (a, b) in zip(p.lineBottoms, original.lineBottoms) { equal(a, b) }
        XCTAssertEqual(p.lines.count, original.lines.count)
        for (a, b) in zip(p.lines, original.lines) {
            let x = CTLineGetStringRange(a), y = CTLineGetStringRange(b)
            XCTAssertEqual(x.location, y.location); XCTAssertEqual(x.length, y.length)
            XCTAssertEqual(CTLineGetGlyphCount(a), CTLineGetGlyphCount(b))
            for offset: CGFloat in [0, 8.25, 60, 180] {
                XCTAssertEqual(CTLineGetStringIndexForPosition(a, CGPoint(x: offset, y: 0)),
                               CTLineGetStringIndexForPosition(b, CGPoint(x: offset, y: 0)))
            }
        }
        if p.height.isFinite && p.baselines.allSatisfy({ $0.isFinite }) {
            let box = CGRect(x: 12, y: 9, width: width, height: p.height)
            let clip = CGRect(x: 0, y: 0, width: 360, height: 180)
            XCTAssertEqual(inkBitmap(p, spec: input, bounds: box, clip: clip, exhaustive: false),
                           inkBitmap(original, spec: input, bounds: box, clip: clip, exhaustive: true))
        }
        #endif
        return (p, calls)
    }

    func testExplicitStrutExtentsAreComputedOnceAcrossLinesAndPaintRuns() {
        let input = spec("alpha\nbeta\ngamma\ndelta")
        let (p, calls) = checkedStrutParagraph(input, width: 220)
        XCTAssertEqual(p.lines.count, 4)
        XCTAssertTrue(p.lines.allSatisfy { (CTLineGetGlyphRuns($0) as! [CTRun]).count == 1 })
        XCTAssertEqual(p.height, 4 * 18.125)
        XCTAssertEqual(p.lineBottoms, [18.125, 36.25, 54.375, 72.5])
        #if STRUT_EXTENTS_SENTINEL
        print("uniform extents computations \(calls), baseline expected 5, candidate expected 1")
        XCTAssertEqual(calls, 1, "four lines must reuse the already computed matching strut extents")
        #endif
        var styled = input
        var a = input.runs[0]; a.text = "Café e\u{301} "
        var b = a; b.text = "אבג 👩🏽‍💻"; b.letterSpacing = 0.25
        b.color = [200, 30, 80, 255]; b.decoration = "underline"; b.href = "second"
        styled.runs = [a, b]; styled.strut = input.runs[0]
        for width: CGFloat in [75.25, 220] {
            let (painted, _) = checkedStrutParagraph(styled, width: width)
            XCTAssertGreaterThan(painted.lines.count, 0)
            XCTAssertEqual(painted.height, CGFloat(painted.lines.count) * 18.125)
            assertInkMatches(painted, spec: styled,
                             bounds: CGRect(x: 12, y: 9, width: width, height: painted.height),
                             clip: CGRect(x: 0, y: 0, width: 360, height: 180))
        }
    }

    func testExplicitStrutReuseRequiresEveryFontAndHeightField() {
        let base = spec("plain").runs[0]
        for field in 0..<5 {
            var child = base
            switch field {
            case 0: child.size += 2
            case 1: child.weight = 700
            case 2: child.family = 5
            case 3: child.italic = true
            default: child.lineHeight! += 5.25
            }
            var input = spec(""); input.runs = [child]; input.strut = base
            let (p, calls) = checkedStrutParagraph(input, width: 220)
            XCTAssertEqual(p.lines.count, 1)
            XCTAssertGreaterThanOrEqual(p.height, base.lineHeight!)
            #if STRUT_EXTENTS_SENTINEL
            XCTAssertEqual(calls, 2, "mismatched field \(field) must evaluate the authored extents")
            #endif
        }
        for height: CGFloat in [0, -0.0, .infinity, -.infinity, .nan] {
            var child = base; child.lineHeight = height
            var strut = child
            if height == 0 { strut.lineHeight = Double(height).sign == .minus ? 0 : -0.0 }
            var input = spec(""); input.runs = [child]; input.strut = strut
            let (p, calls) = checkedStrutParagraph(input, width: 220)
            XCTAssertEqual(p.lines.count, 1)
            XCTAssertEqual(CTLineGetStringRange(p.lines[0]).length, child.text.utf16.count)
            #if STRUT_EXTENTS_SENTINEL
            XCTAssertEqual(calls, 2, "signed-zero mismatch/nonfinite height must not reuse minimum")
            #endif
        }
        var zero = base; zero.size = 0
        var negativeZero = zero; negativeZero.size = -0.0
        var input = spec(""); input.runs = [negativeZero]; input.strut = zero
        let (p, calls) = checkedStrutParagraph(input, width: 220)
        XCTAssertEqual(p.lines.count, 1)
        #if STRUT_EXTENTS_SENTINEL
        XCTAssertEqual(calls, 2, "font key signed zero must remain distinct")
        #endif
    }

    func testStrutReusePreservesNormalFallbackCoalescingAndClampedSuffix() {
        var input = spec("👩🏽‍💻 café\nאבג\nthird")
        input.runs[0].lineHeight = nil
        let (normal, _) = checkedStrutParagraph(input, width: 95)
        XCTAssertGreaterThan(normal.height, 0)
        input.strut = input.runs[0]
        input.runs[0].lineHeight = 60.25
        let (explicit, _) = checkedStrutParagraph(input, width: 95)
        XCTAssertEqual(explicit.height, CGFloat(explicit.lines.count) * 60.25)
        input = spec("before ")
        var second = input.runs[0]; second.text = "after"; second.lineHeight = 60.25
        input.runs.append(second)
        let (coalesced, _) = checkedStrutParagraph(input, width: 220)
        XCTAssertEqual((CTLineGetGlyphRuns(coalesced.lines[0]) as! [CTRun]).count, 1)
        XCTAssertEqual(coalesced.height, 60.25)
        input = spec("First\nSecond\nHidden")
        input.lineClamp = 1
        var hidden = input.runs[0]; hidden.text = " suffix"; hidden.lineHeight = 100
        input.runs.append(hidden)
        let (clamped, _) = checkedStrutParagraph(input, width: 100)
        XCTAssertEqual(clamped.lines.count, 1)
        XCTAssertEqual(clamped.height, 18.125)
        let (empty, calls) = checkedStrutParagraph(spec(""), width: 100)
        XCTAssertTrue(empty.lines.isEmpty)
        XCTAssertEqual(empty.height, 18.125)
        #if STRUT_EXTENTS_SENTINEL
        XCTAssertEqual(calls, 1)
        #endif
    }

    func testObsoleteUnpinnedWidthsRetireBeforeTheNextWidth() {
        let engine = TextEngine(resolve: { _ in nil })
        let value = spec(String(repeating: "Café e\u{301} 🦀 東京 paragraph. ", count: 120))
        var history: [WeakParagraphForResidency] = []
        for width in 200..<224 {
            autoreleasepool {
                let paragraph = engine.paragraph(value, width: CGFloat(width))
                history.append(WeakParagraphForResidency(paragraph))
                XCTAssertEqual(paragraph.lines.last.map { CTLineGetStringRange($0) }.map { $0.location + $0.length },
                               value.runs[0].text.utf16.count)
            }
            XCTAssertEqual(history.dropLast().filter { $0.value != nil }.count, 0,
                           "Old widths with no presenter/checkpoint owner must not accumulate")
        }
    }

    func testIndependentAcceptedWidthsRemainReusableDuringResize() {
        let engine = TextEngine(resolve: { _ in nil })
        let value = spec(String(repeating: "two visible owners 🦀 ", count: 80))
        let first = engine.paragraph(value, width: 180)
        let second = engine.paragraph(value, width: 260)
        for width in 300..<316 {
            autoreleasepool { _ = engine.paragraph(value, width: CGFloat(width)) }
        }
        XCTAssertTrue(engine.paragraph(value, width: 180) === first)
        XCTAssertTrue(engine.paragraph(value, width: 260) === second)
        XCTAssertTrue(first !== second)
        XCTAssertEqual(first.lines.last.map { CTLineGetStringRange($0) }.map { $0.location + $0.length },
                       value.runs[0].text.utf16.count)
    }

    func testResidencyCheckpointRestoresWidthsAndFontNamespace() {
        let engine = TextEngine(resolve: { _ in nil })
        let value = spec("checkpoint full source 🦀")
        weak var original: Paragraph?
        autoreleasepool { original = engine.paragraph(value, width: 180) }
        let saved = engine.checkpoint()
        autoreleasepool { _ = engine.paragraph(value, width: 200) }
        XCTAssertTrue(original != nil, "Checkpoint owns its accepted cache state")
        engine.install(nil)
        let candidate = engine.paragraph(value, width: 180)
        XCTAssertTrue(candidate !== original)
        XCTAssertTrue(candidate.shape!.identity.catalog !== original!.shape!.identity.catalog)
        engine.restore(saved)
        XCTAssertTrue(engine.paragraph(value, width: 180) === original)
        XCTAssertTrue(engine.paragraph(value, width: 180) !== candidate)
    }

    private func inkBitmap(_ paragraph: Paragraph, spec: Spec, bounds: CGRect,
                           clip: CGRect, exhaustive: Bool) -> Data {
        let width = 360, height = 180, stride = width * 4
        let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                bytesPerRow: stride, space: CGColorSpaceCreateDeviceRGB(),
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.translateBy(x: 0, y: CGFloat(height))
        context.scaleBy(x: 1, y: -1)
        context.clip(to: clip)
        if exhaustive {
            // Deliberately independent of TextEngine.draw and its candidate index.
            // y flips in the CTM at each baseline: a flipped text matrix inverts
            // glyphs' vertical offsets (fallback faces' cursive attachment).
            let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
            context.textMatrix = .identity
            for (line, baseline) in zip(paragraph.lines, paragraph.baselines) {
                let x = CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(bounds.width)))
                context.saveGState()
                context.translateBy(x: bounds.minX + x, y: bounds.minY + baseline.rounded())
                context.scaleBy(x: 1, y: -1)
                context.textPosition = .zero
                CTLineDraw(line, context)
                context.restoreGState()
            }
        } else {
            TextEngine.draw(paragraph, spec: spec, in: bounds, context: context, dirty: clip)
        }
        return Data(bytes: context.data!, count: stride * height)
    }

    func assertInkMatches(_ paragraph: Paragraph, spec: Spec, bounds: CGRect,
                                  clip: CGRect, file: StaticString = #filePath, line: UInt = #line) {
        let expected = inkBitmap(paragraph, spec: spec, bounds: bounds, clip: clip, exhaustive: true)
        XCTAssertTrue(expected.contains { $0 != 0 }, "Oracle clip must contain ink", file: file, line: line)
        let actual = inkBitmap(paragraph, spec: spec, bounds: bounds, clip: clip, exhaustive: false)
        XCTAssertTrue(actual == expected, "Viewport differs from exhaustive paint: \(clip)", file: file, line: line)
    }

    func testViewportInkMatchesExhaustiveAtBeginningMiddleAndEnd() {
        let engine = TextEngine(resolve: { _ in nil })
        let run = Run(text: String(repeating: "Café e\u{301} 🦀 東京 — complete paragraph. ", count: 180),
                      size: 16, weight: 400, family: 0, italic: false,
                      lineHeight: 23.125, letterSpacing: 0)
        let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [30, 60, 90, 255])
        let paragraph = engine.paragraph(spec, width: 230)
        XCTAssertGreaterThan(paragraph.lines.count, 100)
        let coverage = paragraph.lines.map { CTLineGetStringRange($0) }
        XCTAssertEqual(coverage.last!.location + coverage.last!.length, run.text.utf16.count)
        let clip = CGRect(x: 12.5, y: 8.25, width: 320, height: 150)
        for index in [0, paragraph.lines.count / 2, paragraph.lines.count - 1, 0] {
            let top = paragraph.baselines[index]
            assertInkMatches(paragraph, spec: spec,
                             bounds: CGRect(x: 28.25, y: 48.5 - top, width: 230, height: paragraph.height), clip: clip)
        }
        XCTAssertEqual(paragraph.lines.map { CTLineGetStringRange($0).location }, coverage.map(\.location))
        XCTAssertEqual(paragraph.lines.map { CTLineGetStringRange($0).length }, coverage.map(\.length))
    }

    func testViewportInkKeepsOverlappingFractionalLinesFontsAndStyledLinks() {
        let engine = TextEngine(resolve: { _ in nil })
        for height: CGFloat in [0, 0.25, 5.125, 24.25] {
            var small = Run(text: "flair café\n", size: 13.25, weight: 400, family: 3,
                            italic: true, lineHeight: height, letterSpacing: 0.125)
            small.color = [210, 30, 60, 170]
            small.href = "https://example.com/first"
            var large = small
            large.text = "🧙🏽‍♀️ 東京 fj\n"
            large.size = 39.5; large.family = 0; large.weight = 700
            large.color = [20, 100, 220, 160]; large.decoration = "underline line-through"
            var last = small
            last.text = "last link\n"; last.family = 5; last.size = 21.25
            var strut = small; strut.text = ""; strut.size = 16
            for alignment in [0, 1, 2] {
                let spec = Spec(runs: [small, large, last, small, large, last], align: alignment,
                                lineClamp: 0, color: [0, 0, 0, 255], strut: strut)
                let paragraph = engine.paragraph(spec, width: 180)
                for index in [0, paragraph.lines.count / 2, paragraph.lines.count - 1] {
                    let top = paragraph.baselines[index]
                    let bounds = CGRect(x: 72.25, y: 75.5 - top, width: 180, height: paragraph.height)
                    for clip in [CGRect(x: 10.25, y: 25.5, width: 330, height: 100.25),
                                 CGRect(x: 70.5, y: 72.25, width: 200, height: 10.5)] {
                        assertInkMatches(paragraph, spec: spec, bounds: bounds, clip: clip)
                    }
                }
            }
        }
    }

    func testViewportInkRetainsAlignedOverflowOutsideTheContentBox() {
        let engine = TextEngine(resolve: { _ in nil })
        let run = Run(text: String(repeating: "f", count: 32), size: 19.25, weight: 400,
                      family: 3, italic: true, lineHeight: 24.25, letterSpacing: 0)
        let spec = Spec(runs: [run], align: 2, lineClamp: 0, color: [0, 0, 0, 255])
        let paragraph = engine.paragraph(spec, width: 45)
        XCTAssertEqual(paragraph.lines.count, 1)
        XCTAssertGreaterThan(paragraph.width, 120)
        assertInkMatches(paragraph, spec: spec,
                         bounds: CGRect(x: 240, y: 35, width: 45, height: paragraph.height),
                         clip: CGRect(x: 0, y: 20, width: 220, height: 80))
    }

    func testViewportInkIndexIsReusedAndKeepsZeroHeightPaintOrder() {
        let engine = TextEngine(resolve: { _ in nil })
        var run = Run(text: String(repeating: "line\n", count: 1024), size: 16, weight: 400,
                      family: 0, italic: false, lineHeight: 24, letterSpacing: 0)
        var spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let paragraph = engine.paragraph(spec, width: 200)
        XCTAssertTrue(paragraph.cachedInk == nil)
        let index = paragraph.inkBounds()
        XCTAssertTrue(paragraph.inkBounds() === index)
        XCTAssertEqual(index.storageBytes, 2048 * 2 * MemoryLayout<CGFloat>.stride)
        var visible: [Int] = []
        index.forEachLine(from: 12000, through: 12048) { visible.append($0) }
        XCTAssertGreaterThan(visible.count, 0)
        XCTAssertTrue(visible.count < 6)
        XCTAssertEqual(visible, visible.sorted())

        // Many lines may paint the same pixels. No binary search on baselines,
        // line-height division, or one-candidate shortcut may omit any of them.
        run.lineHeight = 0
        spec.runs = [run]
        let overlapping = engine.paragraph(spec, width: 200)
        var all: [Int] = []
        overlapping.inkBounds().forEachLine(from: -100, through: 100) { all.append($0) }
        XCTAssertEqual(all, Array(overlapping.lines.indices))
        XCTAssertTrue(overlapping.inkBounds() !== index)
        XCTAssertTrue(paragraph.inkBounds() === index)
    }

    func testTextCacheDistinguishesCanonicallyEquivalentSourceRanges() {
        let engine = TextEngine(resolve: { _ in nil })
        let composed = "Café 🦀 東京"
        let decomposed = "Cafe\u{301} 🦀 東京"
        XCTAssertEqual(composed, decomposed) // Swift String equality is canonical.
        XCTAssertTrue(composed.utf16.count != decomposed.utf16.count)
        let first = Run(text: composed, size: 16, weight: 400, family: 0,
                        italic: false, lineHeight: 24.25, letterSpacing: 0)
        var second = first; second.text = decomposed
        XCTAssertTrue(first != second) // CoreText indexes the actual UTF16 source.
        var bridged = first; bridged.text = NSString(string: composed) as String
        XCTAssertEqual(first, bridged)
        XCTAssertEqual(first.hashValue, bridged.hashValue)
        let spec = Spec(runs: [first], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        var other = spec; other.runs = [second]
        let a = engine.paragraph(spec, width: 60)
        let b = engine.paragraph(other, width: 60)
        XCTAssertTrue(a !== b)
        for (paragraph, source) in [(a, composed), (b, decomposed)] {
            var end = 0
            for line in paragraph.lines {
                let range = CTLineGetStringRange(line)
                XCTAssertEqual(range.location, end)
                end += range.length
            }
            XCTAssertEqual(end, source.utf16.count)
        }
        XCTAssertTrue(engine.paragraph(spec, width: 60) === a)
        XCTAssertTrue(engine.paragraph(other, width: 60) === b)
    }

    func testViewportInkKeepsPaintOrderWhenBaselinesGoBackwards() {
        let engine = TextEngine(resolve: { _ in nil })
        let baselines: [CGFloat] = [50.125, 15.25, 50.125, -8.5, 110.25, 0.25]
        var specs: [Spec] = []
        let lines = baselines.indices.map { i -> CTLine in
            var run = Run(text: "fj café 🦀 \(i)", size: 24.25, weight: 400,
                          family: 3, italic: true, lineHeight: 0, letterSpacing: 0)
            run.color = i.isMultiple(of: 2) ? [220, 20, 50, 150] : [20, 60, 220, 170]
            run.href = "example.md#\(i)"
            let spec = Spec(runs: [run], align: 1, lineClamp: 0, color: [0, 0, 0, 255])
            specs.append(spec)
            return CTLineCreateWithAttributedString(engine.attributed(spec))
        }
        let paragraph = Paragraph(lines: lines, baselines: baselines, width: 180, height: 130)
        for clip in [CGRect(x: 0, y: 20, width: 360, height: 140),
                     CGRect(x: 30.25, y: 75.5, width: 300, height: 12.25)] {
            assertInkMatches(paragraph, spec: specs[0],
                             bounds: CGRect(x: 72.25, y: 40.5, width: 180, height: 130), clip: clip)
        }
    }
}

private final class WeakParagraphForResidency {
    weak var value: Paragraph?
    init(_ value: Paragraph) { self.value = value }
}


extension TextGeometryTests {
    func testDroppingAcceptedLeaseAlsoReleasesItsSourceAndShaperWithoutAnotherLookup() {
        let engine = TextEngine(resolve: { _ in nil })
        weak var source: TextIdentity?
        weak var shape: TextShape?
        autoreleasepool {
            let paragraph = engine.paragraph(spec(String(repeating: "complete source 🦀 ", count: 120)), width: 190)
            source = paragraph.shape!.identity
            shape = paragraph.shape
            engine.accepted(paragraph)
        }
        XCTAssertTrue(shape == nil)
        XCTAssertTrue(source == nil, "Weak lookup keys must not retain a dead paragraph's full source")
        XCTAssertEqual(engine.residencyStats.coldEntries, 0)
    }

    func testAcceptedWidthsUseWeakLookupAndReleaseWithTheirLastOwner() {
        let engine = TextEngine(resolve: { _ in nil })
        let value = spec(String(repeating: "accepted source 🦀 ", count: 90))
        var first: Paragraph? = engine.paragraph(value, width: 180)
        var second: Paragraph? = engine.paragraph(value, width: 270)
        engine.accepted(first!); engine.accepted(second!)
        let weakFirst = WeakParagraphForResidency(first!), weakSecond = WeakParagraphForResidency(second!)
        for width in 300..<320 {
            autoreleasepool { _ = engine.paragraph(value, width: CGFloat(width)) }
        }
        autoreleasepool {
            XCTAssertTrue(engine.paragraph(value, width: 180) === first)
            XCTAssertTrue(engine.paragraph(value, width: 270) === second)
        }
        first = nil; second = nil
        XCTAssertTrue(weakFirst.value == nil)
        XCTAssertTrue(weakSecond.value == nil, "Weak cache hits must not re-pin an accepted width")
    }

    func testColdBudgetRetiresDistinctSourcesAndAllowsOneOversizeWorkingValue() {
        let engine = TextEngine(resolve: { _ in nil }, coldTextTargetBytes: 4096)
        var history: [WeakParagraphForResidency] = []
        for i in 0..<16 {
            autoreleasepool {
                let value = spec(String(repeating: "whole large source \(i) 🦀 ", count: 90))
                let paragraph = engine.paragraph(value, width: 180)
                history.append(WeakParagraphForResidency(paragraph))
                XCTAssertEqual(paragraph.lines.last.map { CTLineGetStringRange($0) }.map { $0.location + $0.length },
                               value.runs[0].text.utf16.count)
            }
            XCTAssertEqual(history.dropLast().filter { $0.value != nil }.count, 0)
            XCTAssertEqual(engine.residencyStats.coldEntries, 1)
            XCTAssertGreaterThan(engine.residencyStats.coldEstimatedBytes, engine.residencyStats.softTargetBytes,
                                 "Oversize current work is observable, not rejected or truncated")
            XCTAssertEqual(engine.residencyStats.coldOverageBytes,
                           engine.residencyStats.coldEstimatedBytes - engine.residencyStats.softTargetBytes)
            XCTAssertGreaterThan(engine.residencyStats.coldOwnedPayloadBytes, 0)
        }
    }

    func testPaintReplacementSharesIdentityAndRetiresUnownedMeasurementLines() {
        let engine = TextEngine(resolve: { _ in nil })
        let geometry = spec(String(repeating: "styled link café 🦀 ", count: 80))
        weak var measured: Paragraph?
        var ranges: [Int] = []
        autoreleasepool {
            let paragraph = engine.paragraph(geometry, width: 190)
            measured = paragraph
            ranges = paragraph.lines.map { CTLineGetStringRange($0).length }
        }
        let identity = measured!.shape!.identity
        var paint = geometry
        paint.color = [180, 30, 70, 255]
        paint.runs[0].href = "example.md#café"
        paint.runs[0].decoration = "underline line-through"
        let colored = engine.paragraph(paint, width: 190)
        XCTAssertTrue(colored.shape!.identity === identity)
        XCTAssertEqual(colored.lines.map { CTLineGetStringRange($0).length }, ranges)
        XCTAssertTrue(measured == nil, "Colored pixels must not keep a redundant black CTLine array")
        XCTAssertEqual(colored.shape!.spec.runs[0].href, paint.runs[0].href)
        XCTAssertEqual(engine.residencyStats.liveParagraphs, 1)
        assertInkMatches(colored, spec: paint, bounds: CGRect(x: 15, y: 20, width: 190, height: colored.height),
                         clip: CGRect(x: 0, y: 0, width: 340, height: 160))
    }

    func testIntrinsicMeasurementRetainsScalarsNotFullLineArrays() {
        let engine = TextEngine(resolve: { _ in nil })
        let value = String(repeating: "Café e\u{301} 🦀 東京 ", count: 100)
        let bytes = Array(value.utf8)
        bytes.withUnsafeBufferPointer { bytes in
            var run = ExactTextRun()
            run.text = bytes.baseAddress; run.len = bytes.count
            run.font_size = 13.25; run.font_weight = 400
            run.has_line_height = 1; run.line_height = 18.125
            withUnsafePointer(to: &run) { pointer in
                var request = ExactMeasureRequest()
                request.runs = pointer; request.count = 1; request.strut = pointer.pointee
                request.strut.text = nil; request.strut.len = 0
                for width in [Float(EXACT_MAX_CONTENT), Float(EXACT_MIN_CONTENT)] {
                    request.width = width
                    let first = engine.measure(request)
                    XCTAssertGreaterThan(first.width, 0)
                    XCTAssertGreaterThan(first.height, 0)
                    XCTAssertEqual(engine.residencyStats.liveParagraphs, 0)
                    let hits = engine.measureHits
                    let repeated = engine.measure(request)
                    XCTAssertEqual(repeated.width, first.width)
                    XCTAssertEqual(repeated.height, first.height)
                    XCTAssertEqual(repeated.baseline, first.baseline)
                    XCTAssertEqual(engine.measureHits, hits + 1)
                }
            }
        }
        XCTAssertEqual(engine.residencyStats.scalarEntries, 3)
    }

    func testRevisitedMeasurementWidthsReuseScalarsWithoutRetainingOldParagraphs() {
        let engine = TextEngine(resolve: { _ in nil })
        let bytes = Array(String(repeating: "Café e\u{301} 🦀 東京 paragraph. ", count: 20).utf8)
        bytes.withUnsafeBufferPointer { bytes in
            var run = ExactTextRun()
            run.text = bytes.baseAddress; run.len = bytes.count
            run.font_size = 13.25; run.font_weight = 400
            run.has_line_height = 1; run.line_height = 18.125
            withUnsafePointer(to: &run) { pointer in
                var request = ExactMeasureRequest()
                request.runs = pointer; request.count = 1; request.strut = pointer.pointee
                request.strut.text = nil; request.strut.len = 0
                var expected: [Float: ExactMetrics] = [:]
                for width: Float in [640, 0, 180] {
                    request.width = width
                    expected[width] = engine.measure(request)
                    XCTAssertLessThanOrEqual(engine.residencyStats.liveParagraphs, 1)
                }
                let hits = engine.measureHits
                for width: Float in [640, 0, 180, 640, 0, 180] {
                    request.width = width
                    let actual = engine.measure(request), first = expected[width]!
                    XCTAssertEqual(actual.width, first.width)
                    XCTAssertEqual(actual.height, first.height)
                    XCTAssertEqual(actual.baseline, first.baseline)
                    XCTAssertLessThanOrEqual(engine.residencyStats.liveParagraphs, 1)
                }
                XCTAssertEqual(engine.measureHits, hits + 6, "Exploratory widths need scalar reuse, not full CTLine history")
                engine.install(nil)
                let before = engine.measureHits
                request.width = 640
                _ = engine.measure(request)
                XCTAssertEqual(engine.measureHits, before, "A replacement font catalog must not reuse old scalars")
            }
        }
    }
}


extension TextGeometryTests {
    func testShortRowMaintenanceDoesNotWalkUnrelatedColdHistory() {
        var visits: [UInt64] = []
        // Cross both entry caps, as a table traversal does.
        for history in [10, 100, 1000, 10_000] {
            let engine = TextEngine(resolve: { _ in nil })
            var visible: [Paragraph] = []
            for row in 0..<history {
                autoreleasepool {
                    let input = spec("ROW\(row) café e\u{301} 🦀")
                    _ = engine.minContentWidth(input)
                    let p = engine.paragraph(input, width: 180)
                    engine.accepted(p)
                    visible.append(p)
                    if visible.count > 8 { visible.removeFirst() }
                }
            }
            let before = engine.residencyStats.maintenanceVisits
            XCTAssertTrue(engine.paragraph(spec("ROW\(history - 1) café e\u{301} 🦀"), width: 180) === visible.last)
            let fresh = engine.paragraph(spec("NEW unrelated café"), width: 180)
            engine.accepted(fresh)
            let work = engine.residencyStats.maintenanceVisits - before
            visits.append(work)
            print("cache-maintenance history=\(history) visits=\(work)")
            XCTAssertLessThanOrEqual(work, 256, "An unrelated hit/miss/accept cannot scan all historical rows")
            XCTAssertEqual(visible.count, 8)
        }
        XCTAssertLessThanOrEqual(visits.last!, visits.first! + 128)
    }

    func testResidencyReportsSharedPayloadAndLazyInkExactlyAcrossCheckpoint() {
        let engine = TextEngine(resolve: { _ in nil })
        let p = engine.paragraph(spec("FIRST\nSECOND café e\u{301} 🦀"), width: 180)
        let shape = p.shape!, source = shape.identity
        let before = engine.residencyStats
        XCTAssertEqual(before.coldEntries, 2)
        XCTAssertEqual(before.coldOwnedPayloadBytes, source.ownedBytes + shape.ownedBytes + p.ownedPayloadBytes)
        XCTAssertEqual(before.coldCoreTextEstimateBytes, shape.opaqueEstimate + p.coreTextEstimateBytes)
        let checkpoint = engine.checkpoint()
        let ink = p.inkBounds()
        XCTAssertEqual(engine.residencyStats.coldOwnedPayloadBytes, before.coldOwnedPayloadBytes + ink.storageBytes)
        engine.accepted(p)
        XCTAssertEqual(engine.residencyStats.coldEntries, 0)
        _ = engine.paragraph(spec("unrelated replacement"), width: 240)
        engine.restore(checkpoint)
        XCTAssertTrue(engine.paragraph(spec("FIRST\nSECOND café e\u{301} 🦀"), width: 180) === p)
        XCTAssertEqual(engine.residencyStats.coldOwnedPayloadBytes, before.coldOwnedPayloadBytes + ink.storageBytes)
    }

    func testDeadWeakMetadataAndTinyColdEntriesHaveExplicitBounds() {
        let engine = TextEngine(resolve: { _ in nil })
        var visible: [Paragraph] = []
        for row in 0..<(TextResidency.maxLookupEntries + 64) {
            autoreleasepool {
                let p = engine.paragraph(spec("accepted \(row)"), width: 180)
                engine.accepted(p)
                visible.append(p)
                if visible.count > 8 { visible.removeFirst() }
            }
        }
        var stats = engine.residencyStats
        XCTAssertLessThanOrEqual(stats.metadataEntries, TextResidency.maxLookupEntries)
        XCTAssertLessThanOrEqual(stats.identityEntries, TextResidency.maxIdentities)
        XCTAssertLessThanOrEqual(stats.geometryEntries, TextResidency.maxLookupEntries)
        XCTAssertEqual(stats.coldEntries, 0)
        for row in 0..<(TextResidency.maxColdEntries + 64) {
            _ = engine.minContentWidth(spec("cold \(row)"))
        }
        stats = engine.residencyStats
        XCTAssertLessThanOrEqual(stats.coldEntries, TextResidency.maxColdEntries)
        XCTAssertLessThanOrEqual(stats.metadataEntries, TextResidency.maxLookupEntries)
        XCTAssertLessThanOrEqual(stats.identityEntries, TextResidency.maxIdentities)
        XCTAssertEqual(stats.softTargetBytes, 64 * 1024 * 1024)
        XCTAssertLessThanOrEqual(stats.coldEstimatedBytes, stats.softTargetBytes)
        XCTAssertEqual(visible.count, 8)
    }
}

extension TextGeometryTests {
    func testMemoryPressureDropsColdTextButNotWhatAViewHolds() {
        let engine = TextEngine(resolve: { _ in nil })
        let held = engine.paragraph(spec("held by a view"), width: 180)
        engine.accepted(held)
        for i in 0..<32 { _ = engine.paragraph(spec("cold \(i)"), width: 180) }
        XCTAssertGreaterThan(engine.residencyStats.coldEntries, 32)
        engine.dropCold()
        XCTAssertEqual(engine.residencyStats.coldEntries, 0)
        XCTAssertEqual(engine.residencyStats.coldEstimatedBytes, 0)
        XCTAssertTrue(engine.paragraph(spec("held by a view"), width: 180) === held)
    }

    func testScalarAndParagraphChargesShareSourceAndReleaseIndependently() {
        let engine = TextEngine(resolve: { _ in nil })
        let input = spec("shared scalar paragraph café e\u{301} 🦀")
        _ = engine.minContentWidth(input)
        let paragraph = engine.paragraph(input, width: 180)
        let shape = paragraph.shape!, source = shape.identity
        let scalarBytes = MemoryLayout<ExactMetrics>.stride + MemoryLayout<CGFloat?>.stride
        let stats = engine.residencyStats
        XCTAssertEqual(stats.coldEntries, 3)
        XCTAssertEqual(stats.coldOwnedPayloadBytes,
                       source.ownedBytes + shape.ownedBytes + paragraph.ownedPayloadBytes + scalarBytes)
        XCTAssertEqual(stats.coldCoreTextEstimateBytes, shape.opaqueEstimate + paragraph.coreTextEstimateBytes)
        XCTAssertGreaterThanOrEqual(stats.coldAdmissionBytes, stats.coldEstimatedBytes)
        engine.accepted(paragraph)
        XCTAssertEqual(engine.residencyStats.coldEntries, 1)
        XCTAssertEqual(engine.residencyStats.coldOwnedPayloadBytes, source.ownedBytes + scalarBytes)
        XCTAssertEqual(engine.residencyStats.coldCoreTextEstimateBytes, 0)
        XCTAssertTrue(engine.paragraph(input, width: 180) === paragraph)
        XCTAssertEqual(engine.residencyStats.coldEntries, 1, "Weak accepted hit cannot repin its paragraph/shape")
    }

    func testDefault64MiBSoftTargetEvictsColdWorkWithoutTruncation() {
        let engine = TextEngine(resolve: { _ in nil })
        var old: [WeakParagraphForResidency] = []
        for i in 0..<6 {
            autoreleasepool {
                let input = spec(String(repeating: "FULL SOURCE \(i) café\n", count: 12000))
                let p = engine.paragraph(input, width: 180)
                old.append(WeakParagraphForResidency(p))
                let last = CTLineGetStringRange(p.lines.last!)
                XCTAssertEqual(last.location + last.length, input.runs[0].text.utf16.count)
                XCTAssertLessThanOrEqual(engine.residencyStats.coldAdmissionBytes, 64 * 1024 * 1024)
                _ = p.inkBounds()
                XCTAssertLessThanOrEqual(engine.residencyStats.coldEstimatedBytes,
                                         engine.residencyStats.coldAdmissionBytes)
            }
        }
        XCTAssertTrue(old.first!.value == nil, "Cold history must release under the actual default64MiB target")
        XCTAssertTrue(old.last!.value != nil)
    }

    func testLookupCapDoesNotDestroyExternallyAcceptedParagraphs() {
        let engine = TextEngine(resolve: { _ in nil })
        var leases: [Paragraph] = []
        for i in 0..<(TextResidency.maxLookupEntries / 2 + 32) {
            let p = engine.paragraph(spec("live owner \(i)"), width: 180)
            engine.accepted(p)
            leases.append(p)
        }
        let stats = engine.residencyStats
        XCTAssertLessThanOrEqual(stats.metadataEntries, TextResidency.maxLookupEntries)
        XCTAssertLessThanOrEqual(stats.identityEntries, TextResidency.maxIdentities)
        XCTAssertLessThanOrEqual(stats.geometryEntries, TextResidency.maxLookupEntries)
        XCTAssertEqual(stats.coldEntries, 0)
        XCTAssertEqual(leases.first!.shape!.identity.geometry.runs[0].text, "live owner 0")
        XCTAssertGreaterThan(leases.first!.inkBounds().storageBytes, 0)
        let last = leases.last!
        XCTAssertTrue(engine.paragraph(spec("live owner \(leases.count - 1)"), width: 180) === last)
        let weakFirst = WeakParagraphForResidency(leases.first!)
        leases.removeFirst()
        XCTAssertTrue(weakFirst.value == nil, "Evicted lookup metadata cannot own a former view's paragraph")
    }

    func testAcceptedParagraphKeepsSourceAndPaintAcrossScalarReuseAndCatalogRestore() {
        let engine = TextEngine(resolve: { _ in nil })
        let original = spec("Ée\u{301} 🧪漢字 linked source\nsecond line")
        let measured = engine.paragraph(original, width: 190)
        var painted = original
        painted.color = [180, 30, 70, 255]
        painted.runs[0].href = "first.md#É"
        let accepted = engine.paragraph(painted, width: 190)
        engine.accepted(accepted)
        let ranges = accepted.lines.map { CTLineGetStringRange($0) }
        let bytes = Array(accepted.shape!.spec.runs[0].text.utf8)
        XCTAssertEqual(accepted.height, measured.height)
        XCTAssertEqual(accepted.baselines, measured.baselines)
        XCTAssertTrue(engine.paragraph(painted, width: 190) === accepted)
        let cp = engine.checkpoint()
        engine.install(nil)
        let candidate = engine.paragraph(painted, width: 190)
        XCTAssertTrue(candidate.shape!.identity.catalog !== accepted.shape!.identity.catalog)
        engine.restore(cp)
        XCTAssertTrue(engine.paragraph(painted, width: 190) === accepted)
        var changed = painted
        changed.color = [20, 80, 220, 255]
        changed.runs[0].href = "second.md#e\u{301}"
        let replacement = engine.paragraph(changed, width: 190)
        XCTAssertTrue(replacement !== accepted)
        XCTAssertEqual(replacement.height, accepted.height)
        XCTAssertEqual(replacement.baselines, accepted.baselines)
        XCTAssertEqual(replacement.lines.map { CTLineGetStringRange($0).location }, ranges.map { $0.location })
        XCTAssertEqual(replacement.lines.map { CTLineGetStringRange($0).length }, ranges.map { $0.length })
        XCTAssertEqual(Array(replacement.shape!.spec.runs[0].text.utf8), bytes)
        XCTAssertEqual(replacement.shape!.spec.runs[0].href, changed.runs[0].href)
        XCTAssertEqual(accepted.shape!.spec.runs[0].href, painted.runs[0].href)
        XCTAssertEqual(ranges.last!.location + ranges.last!.length, original.runs[0].text.utf16.count)
        assertInkMatches(replacement, spec: changed,
                         bounds: CGRect(x: 15, y: 20, width: 190, height: replacement.height),
                         clip: CGRect(x: 0, y: 0, width: 340, height: 160))
    }

}

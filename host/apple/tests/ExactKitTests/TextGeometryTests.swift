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

// Viewport culling must be indistinguishable from painting every shaped line.
extension TextGeometryTests {
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
            context.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
            let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
            for (line, baseline) in zip(paragraph.lines, paragraph.baselines) {
                let x = CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(bounds.width)))
                context.textPosition = CGPoint(x: bounds.minX + x, y: bounds.minY + baseline.rounded())
                CTLineDraw(line, context)
            }
        } else {
            TextEngine.draw(paragraph, spec: spec, in: bounds, context: context, dirty: clip)
        }
        return Data(bytes: context.data!, count: stride * height)
    }

    private func assertInkMatches(_ paragraph: Paragraph, spec: Spec, bounds: CGRect,
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

    func testTextCacheHitKeepsCheckpointEvictionOrderIndependent() {
        var cache = TextCache<Int, Int>()
        for i in 0..<4096 { cache.put(i, i) }
        let saved = cache
        XCTAssertEqual(cache.get(0), 0)
        cache.put(4096, 4096)
        XCTAssertEqual(cache.get(0), 0)
        var restored = saved
        restored.put(4096, 4096)
        XCTAssertTrue(restored.get(0) == nil)
        XCTAssertEqual(restored.get(4095), 4095)
        XCTAssertEqual(cache.get(4095), 4095)
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

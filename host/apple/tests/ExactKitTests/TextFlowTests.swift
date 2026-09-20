// @ref LLP 1043.000 §3 D4–D7 — source ownership, geometry and retained preparation.
import XCTest
import CoreText
import CExact
@testable import ExactKit

final class TextFlowTests: XCTestCase {
    private let engine = TextEngine(resolve: { _ in nil })
    private func spec(_ text: String, family: Int = 0) -> Spec {
        Spec(runs: [Run(text: text, size: 16, weight: 400, family: family, italic: false,
                        lineHeight: 24, letterSpacing: 0)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
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

    private func shape(_ spec: Spec) -> TextShape { engine.paragraph(spec, width: .infinity).shape! }
    private let circle = TextFlowShape(kind: 0, x: 180, y: 96, a: 52)
    private let prose = String(repeating: "The garden leaves room for the light. We watch the quiet river carry its story beyond the trees. ", count: 10)

    /// One tokenizer serves every paragraph. It must find what a new one
    /// finds, whatever script the paragraph before it was in.
    func testTheSharedLineBreakerFindsWhatANewOneFinds() {
        let texts = [
            "東京都は日本の首都です。人口は約一千四百万人で、世界有数の大都市です。",
            "A plain sentence, with a hyphen-ated word and https://example.com/a/long/path?query=1.",
            "ภาษาไทยไม่มีช่องว่างระหว่างคำ จึงต้องใช้พจนานุกรมในการตัดคำ",
            "",
            "Emoji 👨‍👩‍👧‍👦 and e\u{301}, then 中文 mixed with Latin and a\u{00A0}no-break space.",
            "one\ntwo\r\nthree\u{2028}four",
            "A plain sentence, with a hyphen-ated word and https://example.com/a/long/path?query=1.",
        ]
        for text in texts + texts.reversed() {
            let string = text as NSString, length = string.length
            let fresh = CFStringTokenizerCreate(nil, string as CFString, CFRange(location: 0, length: length), kCFStringTokenizerUnitLineBreak, nil)!
            var expected: [Int] = []
            while CFStringTokenizerAdvanceToNextToken(fresh).rawValue != 0 {
                let range = CFStringTokenizerGetCurrentTokenRange(fresh)
                expected.append(range.location + range.length)
            }
            if expected.last != length { expected.append(length) }
            XCTAssertEqual(engine.lineBoundaries(string, length: length), expected, text)
        }
    }

    func testCachedUnicodeOpportunitiesPreserveFreshLayoutAcrossWidths() {
        let texts = [
            "", "longwordwithoutbreaks", "A hyphen-ated word and https://example.invalid/long/path.",
            "東京都は日本の首都です。中文段落沿着河边展开。",
            "ภาษาไทยไม่มีช่องว่างระหว่างคำ จึงต้องใช้พจนานุกรมในการตัดคำ",
            "Emoji 👨‍👩‍👧‍👦 e\u{301} é العربية שלום soft\u{ad}hyphen no\u{a0}break",
            "one\ntwo\r\nthree\u{2028}four"
        ]
        for text in texts {
            for mode in 0...2 {
                var input = spec(text); input.overflowWrap = mode
                let source = shape(input)
                XCTAssertNil(source.lineBreakBoundaries, "Intrinsic layout needs no boundary scan")
                for width: CGFloat in [1, 70, 280, 113, 400, 70] {
                    let cached = engine.paragraph(input, width: width)
                    let fresh = TextEngine(resolve: { _ in nil }).paragraph(input, width: width)
                    XCTAssertTrue(cached.shape === source)
                    XCTAssertEqual(cached.lines.map { CTLineGetStringRange($0).location }, fresh.lines.map { CTLineGetStringRange($0).location })
                    XCTAssertEqual(cached.lines.map { CTLineGetStringRange($0).length }, fresh.lines.map { CTLineGetStringRange($0).length })
                    XCTAssertEqual(cached.baselines, fresh.baselines)
                    XCTAssertEqual(cached.lineBottoms, fresh.lineBottoms)
                    XCTAssertEqual(cached.width, fresh.width)
                    XCTAssertEqual(cached.height, fresh.height)
                }
                if mode == 0 {
                    XCTAssertEqual(source.lineBreakBoundaries, engine.lineBoundaries(text as NSString, length: text.utf16.count))
                } else { XCTAssertNil(source.lineBreakBoundaries, "Emergency wrapping needs no Unicode opportunity array") }
            }
        }
    }

    func testLazySourceChargesCatchUpWhenRestoringAnEarlierCheckpoint() {
        let input = spec(String(repeating: "Unicode opportunities 👨‍👩‍👧‍👦 中文 e\u{301} ", count: 12))
        let source = shape(input)
        let before = engine.residencyStats.coldOwnedPayloadBytes
        let originalShapeBytes = source.ownedBytes
        let saved = engine.checkpoint()
        _ = engine.layout(source, width: 180)
        let boundaryBytes = source.lineBreakBoundaries!.count * MemoryLayout<Int>.stride
        XCTAssertGreaterThan(boundaryBytes, 0)
        XCTAssertEqual(source.ownedBytes - originalShapeBytes, boundaryBytes)
        XCTAssertEqual(engine.residencyStats.coldOwnedPayloadBytes, before + boundaryBytes)
        engine.restore(saved)
        XCTAssertEqual(engine.residencyStats.coldOwnedPayloadBytes, before + boundaryBytes)
        // Existing lazy flow preparation shares the same checkpoint lifetime.
        _ = engine.paragraph(input, width: 360, flow: [circle])
        let growth = source.ownedBytes - originalShapeBytes
        XCTAssertGreaterThan(growth, boundaryBytes)
        engine.restore(saved)
        XCTAssertEqual(engine.residencyStats.coldOwnedPayloadBytes, before + growth)
        XCTAssertEqual(engine.lineBoundaries(input.runs[0].text as NSString, length: source.identity.utf16Count), source.lineBreakBoundaries)
    }

    func testUnicodeBoundaryStorageLeavesWithTheRetiredShape() {
        let local = TextEngine(resolve: { _ in nil }, coldTextTargetBytes: 1024)
        weak var first: TextShape?
        autoreleasepool {
            let p = local.paragraph(spec(String(repeating: "First source 👨‍👩‍👧‍👦 ", count: 90)), width: 180)
            first = p.shape
            XCTAssertNotNil(first?.lineBreakBoundaries)
        }
        autoreleasepool {
            _ = local.paragraph(spec(String(repeating: "Replacement 中文 ", count: 90)), width: 240)
        }
        XCTAssertNil(first, "No separate source/width history may retain the boundary array")
    }

    func testSoftHyphenPaintsDashBesideHole() {
        let flowed = engine.layoutFlow(shape(spec("ab\u{ad}cdefghij")), width: 400,
            flow: [TextFlowShape(kind: 2, x: 64, y: 0, a: 160, b: 100)])
        XCTAssertEqual(flowed.fragments[0].end, 4)
        XCTAssertEqual(flowed.fragments[0].hyphenated, 1)
        let ordinary = engine.paragraph(spec("ab-"), width: .infinity)
        XCTAssertEqual(CTLineGetTypographicBounds(flowed.lines[0], nil, nil, nil),
                       CTLineGetTypographicBounds(ordinary.lines[0], nil, nil, nil), accuracy: 0.02)
        let clear = engine.layoutFlow(shape(spec("ab\u{ad}cdefghij")), width: 400, flow: [])
        XCTAssertEqual(clear.fragments[0].hyphenated, 0)
    }

    func testCSSWhiteSpacePreservesOnlyWhenRequested() {
        var input = spec("A    B\nC")
        let normal = engine.layoutFlow(shape(input), width: 500, flow: [])
        let control = engine.layoutFlow(shape(spec("A B C")), width: 500, flow: [])
        XCTAssertEqual(normal.fragments.count, 1)
        XCTAssertEqual(normal.fragments[0].width, control.fragments[0].width, accuracy: 0.02)
        input.whiteSpace = 1
        let preserved = engine.layoutFlow(shape(input), width: 500, flow: [])
        XCTAssertEqual(preserved.fragments.count, 2)
        let pair = engine.layoutFlow(shape(spec("A B")), width: 500, flow: [])
        XCTAssertGreaterThan(preserved.fragments[0].width, pair.fragments[0].width)
    }

    func testAdvanceClustersMatchCoreTextOffsetsAcrossScripts() {
        for (text, family) in [("Latin letters and words", 0), ("office affinity ffi fi fl", 3),
                               ("مرحبا بالعالم العربية لَا", 0), ("שלום עולם בעברית", 0),
                               ("中文日本語漢字かな", 0), ("क्षि", 0), ("👨‍👩‍👧‍👦 🧙🏽‍♀️ 🇺🇸 e\u{301}", 0)] {
            let source = shape(spec(text, family: family)).preparedFlow()
            XCTAssertEqual(source.advances.count, (text as NSString).length)
            XCTAssertEqual(Double(source.advances.reduce(0, +)), CTLineGetTypographicBounds(source.line, nil, nil, nil), accuracy: 0.02, text)
            // CoreText places carets inside kerning pairs (and ligatures),
            // so individual caret deltas are not glyph advances. At word/run
            // boundaries the complete advances must agree, in both directions.
            let words = text as NSString
            let expression = try! NSRegularExpression(pattern: #"\S+"#)
            for match in expression.matches(in: text, range: NSRange(location: 0, length: words.length)) {
                let a = match.range.location, b = NSMaxRange(match.range)
                var secondaryA: CGFloat = 0, secondaryB: CGFloat = 0
                let primaryA = CTLineGetOffsetForStringIndex(source.line, a, &secondaryA)
                let primaryB = CTLineGetOffsetForStringIndex(source.line, b, &secondaryB)
                let advance = CGFloat(source.advances[a..<b].reduce(0, +))
                let error = [primaryA, secondaryA].flatMap { x in [primaryB, secondaryB].map { abs(abs(x - $0) - advance) } }.min()!
                XCTAssertLessThan(error, 0.03, "\(text) word \(a)..<\(b)")
            }
        }
    }

    func testActualLigaturesAssignAdvanceToTheirLowestStringIndex() throws {
        let font = CTFontCreateWithName("HoeflerText-Regular" as CFString, 16, nil)
        let text = "office affinity fi ffi fl"
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: text,
            attributes: [.font: font, .ligature: 2]))
        guard CTLineGetGlyphCount(line) < text.utf16.count else { throw XCTSkip("ligature font unavailable") }
        let table = TextFlowSource.advances(line, text: text)
        XCTAssertEqual(Double(table.reduce(0, +)), CTLineGetTypographicBounds(line, nil, nil, nil), accuracy: 0.01)
        var starts = Set<Int>()
        for run in CTLineGetGlyphRuns(line) as! [CTRun] {
            var indices = [CFIndex](repeating: 0, count: CTRunGetGlyphCount(run))
            CTRunGetStringIndices(run, CFRange(location: 0, length: 0), &indices)
            starts.formUnion(indices)
        }
        XCTAssertLessThan(starts.count, table.count)
        for i in table.indices where !starts.contains(i) { XCTAssertEqual(table[i], 0) }
        XCTAssertGreaterThan(table[1], 0) // ffi in office starts at its lowest index.
        XCTAssertEqual(table[2], 0)
        XCTAssertEqual(table[3], 0)
        let words = try NSRegularExpression(pattern: #"\S+"#)
        for word in words.matches(in: text, range: NSRange(location: 0, length: text.utf16.count)) {
            let start = word.range.location, end = NSMaxRange(word.range)
            let offsets = CTLineGetOffsetForStringIndex(line, end, nil) - CTLineGetOffsetForStringIndex(line, start, nil)
            XCTAssertEqual(CGFloat(table[start..<end].reduce(0, +)), abs(offsets), accuracy: 0.02)
        }
    }

    func testCircleFragmentsCoverTheSourceOnceAndAvoidTheCircle() {
        let p = engine.paragraph(spec(prose), width: 360, flow: [circle])
        XCTAssertFalse(p.fragments.isEmpty) // fails if flow returns nothing
        XCTAssertTrue(zip(p.fragments, p.fragments.dropFirst()).contains { $0.line == $1.line })
        var end = 0
        for f in p.fragments {
            XCTAssertEqual(f.start, end)
            end = f.end
            let dy = max(0, max(Double(f.y - circle.y), Double(circle.y - f.y - Float(p.flowLineHeight))))
            if dy < Double(circle.a) {
                let half = sqrt(Double(circle.a * circle.a) - dy * dy)
                XCTAssertTrue(Double(f.x + f.width) <= Double(circle.x) - half + 0.01 || Double(f.x) >= Double(circle.x) + half - 0.01)
            }
        }
        XCTAssertEqual(end, prose.utf8.count)
        XCTAssertEqual(p.lines.count, p.origins.count)
        XCTAssertNil(p.residencyKey)
        let clear = engine.layoutFlow(p.shape!, width: 360, flow: [])
        XCTAssertFalse(zip(clear.fragments, clear.fragments.dropFirst()).contains { $0.line == $1.line })
        XCTAssertNotEqual(p.fragments.map(\.end), clear.fragments.map(\.end))
    }

    func testNoShapesMatchesOrdinaryBreaksForSimpleMonospaceControl() {
        let input = spec(String(repeating: "one two three four five six seven eight nine ten ", count: 5), family: 5)
        let ordinary = engine.paragraph(input, width: 300)
        let flowed = engine.layoutFlow(ordinary.shape!, width: 300, flow: [])
        XCTAssertEqual(flowed.fragments.map(\.utf16_end), ordinary.lines.map { let r = CTLineGetStringRange($0); return r.location + r.length })
        XCTAssertEqual(flowed.height, ordinary.height, accuracy: 0.02)
        XCTAssertTrue(engine.paragraph(input, width: 300, flow: []) === ordinary)
    }

    func testPrepareOnceAndNoParagraphHistoryAcrossOneHundredMoves() {
        let input = spec(prose)
        var current: Paragraph? = engine.paragraph(input, width: 360, flow: [circle])
        let shape = current!.shape!, prepared = shape.preparedFlow()
        let before = engine.residencyStats
        for i in 0..<100 {
            weak let old = current
            var moved = circle; moved.x = Float(90 + i)
            current = engine.paragraph(input, width: 360, flow: [moved])
            XCTAssertNil(old)
            XCTAssertTrue(current!.shape === shape)
            XCTAssertTrue(shape.preparedFlow() === prepared)
            XCTAssertNil(current!.residencyKey)
        }
        XCTAssertEqual(shape.prepareCount, 1)
        XCTAssertEqual(engine.residencyStats.metadataEntries, before.metadataEntries)
    }

    func testSelectionAndHitsAcrossTwoFragmentsInOneBand() {
        let input = spec(prose), p = engine.paragraph(input, width: 360, flow: [circle])
        let index = p.fragments.indices.dropLast().first { p.fragments[$0].line == p.fragments[$0 + 1].line }!
        let a = p.fragments[index], b = p.fragments[index + 1]
        let range = NSRange(location: a.paint_start, length: b.paint_end - a.paint_start)
        let bounds = CGRect(x: 9, y: 12, width: 360, height: p.height)
        let rects = p.selectionRects(range, align: 0, in: bounds, dirty: bounds)
        XCTAssertEqual(rects.count, 2)
        XCTAssertEqual(rects[0].minY, rects[1].minY, accuracy: 0.01)
        XCTAssertLessThan(rects[0].maxX, rects[1].minX)
        XCTAssertEqual(p.lineIndex(at: CGPoint(x: p.origins[index + 1] + 4, y: CGFloat(b.y) + 2), align: 0, width: 360), index + 1)
    }

    func testClampCountsBandsAndEllipsisFitsTheLastInterval() {
        var input = spec(prose); input.lineClamp = 4; input.align = 2
        let p = engine.paragraph(input, width: 360, flow: [circle])
        XCTAssertGreaterThan(p.lines.count, 4)
        XCTAssertLessThanOrEqual(p.fragments.last!.line, 3)
        XCTAssertEqual(p.height, 96)
        XCTAssertLessThan(p.fragments.last!.end, prose.utf8.count)
        let f = p.fragments.last!
        XCTAssertLessThanOrEqual(CTLineGetTypographicBounds(p.lines.last!, nil, nil, nil), Double(f.available) + 0.01)
        XCTAssertGreaterThanOrEqual(p.origins.last!, CGFloat(f.x))
    }

    func testUnicodeMappingAndTwentyThousandUnitObstruction() {
        var input = spec(String(repeating: "é e\u{301} 👨‍👩‍👧‍👦 中文 العربية שלום ", count: 25)); input.overflowWrap = 2
        let p = engine.paragraph(input, width: 360, flow: [circle])
        let text = input.runs[0].text
        let bytes = Array(text.utf8)
        for f in p.fragments {
            XCTAssertEqual(String(decoding: bytes[..<f.start], as: UTF8.self).utf16.count, f.utf16_start)
            XCTAssertEqual(String(decoding: bytes[..<f.end], as: UTF8.self).utf16.count, f.utf16_end)
            XCTAssertLessThanOrEqual(f.paint_start, f.paint_end)
        }
        XCTAssertEqual(p.fragments.last?.end, bytes.count)
        input.runs[0].text = String(repeating: "x", count: 20_000)
        let wall = TextFlowShape(kind: 2, x: 0, y: 0, a: 360, b: 1e9)
        let blocked = engine.paragraph(input, width: 360, flow: [wall])
        XCTAssertFalse(blocked.lines.isEmpty)
        XCTAssertTrue(blocked.flowIncomplete)
        XCTAssertTrue(blocked.height.isFinite)
        let clear = engine.layoutFlow(blocked.shape!, width: 360, flow: [])
        XCTAssertEqual(clear.fragments.last?.end, 20_000)
    }

    func testTallWallCompletesAndFailedPreparationFallsBack() {
        let input = spec("word"), source = shape(input)
        let wall = TextFlowShape(kind: 2, x: 0, y: 0, a: 400, b: 30_000)
        let p = engine.layoutFlow(source, width: 400, flow: [wall])
        XCTAssertEqual(p.fragments.count, 1)
        XCTAssertGreaterThanOrEqual(p.fragments[0].y, 30_000)
        exact_textflow_free(source.preparedFlow().handle)
        let fallback = engine.layoutFlow(source, width: 400, flow: [wall])
        XCTAssertTrue(fallback.flowIncomplete)
        XCTAssertFalse(fallback.lines.isEmpty)
    }

    func testWalkerCoreTextFiftyParagraphBreakDifferenceRate() {
        let corpus = [
            "The morning river carries a quiet reflection past the old stone bridge. Every ripple changes the light. ",
            "An office offers efficient filing, fine coffee, and an affinity for difficult questions. ",
            "We measured 12.5 metres, then 3,200 steps; the map said: turn left—carefully! ",
            "A page holds words, and the spaces between those words hold the pace of a story. ",
            "中文段落沿着河边展开。每一行文字都有自己的节奏，日本語もあります。 ",
            "مرحبا بالعالم هذه كلمات عربية تتدفق حول الأشكال في الصفحة. ",
            "שלום עולם מילים על הדף נעות סביב צורה ושומרות על הסיפור. ",
            "The family 👨‍👩‍👧‍👦 walked with a 🧙🏽‍♀️ past café signs and e\u{301}lan. ",
            "First comes one thought.\nThen a second thought follows.\n\nA new beginning. ",
            "Unbreakable_words_and_punctuation belong beside ordinary short words and longer descriptions. "
        ]
        var different = 0, differences = 0, boundaries = 0
        for i in 0..<50 {
            let input = spec(String(repeating: corpus[i % 10], count: 3 + i % 4), family: i % 3 == 0 ? 3 : 0)
            let width = CGFloat(240 + (i % 5) * 53)
            let ordinary = engine.paragraph(input, width: width)
            let flowed = engine.layoutFlow(ordinary.shape!, width: width, flow: [])
            let a = Set(ordinary.lines.map { let r = CTLineGetStringRange($0); return r.location + r.length })
            let b = Set(flowed.fragments.map(\.utf16_end))
            if a != b { different += 1 }
            differences += a.symmetricDifference(b).count; boundaries += a.union(b).count
            XCTAssertEqual(flowed.fragments.last?.utf16_end, input.runs[0].text.utf16.count)
        }
        print("TEXTFLOW BREAK CORPUS: \(different)/50 paragraphs differ (\(Double(different) * 2)%); \(differences)/\(boundaries) union break positions differ (\(100 * Double(differences) / Double(boundaries))%)")
    }

    func testDemoSixHundredFramePerformance() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let contract = try String(contentsOf: root.appendingPathComponent("apps/textflow/app.contract"), encoding: .utf8)
        let regex = try NSRegularExpression(pattern: #": "([^"]*)"\) testId="ball-prose""#)
        let match = try XCTUnwrap(regex.firstMatch(in: contract, range: NSRange(contract.startIndex..., in: contract)))
        let text = (contract as NSString).substring(with: match.range(at: 1)).replacingOccurrences(of: #"\n"#, with: "\n")
        var input = spec(text, family: 3)
        input.whiteSpace = 1
        let shape = shape(input)
        var current = engine.layoutFlow(shape, width: 656, flow: [circle])
        let start = CFAbsoluteTimeGetCurrent()
        for i in 0..<600 {
            let x = Float(130 + (i * 3) % 380), y = Float(65 + (i * 2) % 300)
            current = engine.layoutFlow(shape, width: 656, flow: [TextFlowShape(kind: 0, x: x, y: y, a: 66)])
        }
        let us = (CFAbsoluteTimeGetCurrent() - start) * 1_000_000 / 600
        XCTAssertEqual(shape.prepareCount, 1)
        XCTAssertFalse(current.lines.isEmpty)
        print("TEXTFLOW CORETEXT: \(us) us/frame, 600 frames, \(text.utf16.count) UTF16 units, \(current.lines.count) final fragments, prepares=\(shape.prepareCount)")
    }
}

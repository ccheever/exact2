#if os(macOS)
import XCTest
import AppKit
import CExact
@testable import ExactKit

/// Chrome on the same Mac (TextParityCases.swift). Not pinned: Chrome
/// resolves `ui-monospace` to its default font (Times), and gives SF Arabic a
/// 1.09 px line gap CoreText's fallback does not, a line 1 px taller.
final class TextParityMacTests: XCTestCase {
    func testNormalLineHeightIsChromes() {
        assertChromeNormal([
            (400, 11, 13, 11), (400, 12, 15, 12), (400, 13, 16, 13), (400, 14, 17, 14), (400, 15, 18, 15),
            (400, 16, 18, 15), (400, 17, 20, 16), (400, 20, 23, 19), (400, 24, 28, 23), (400, 28, 33, 27),
            (400, 34, 40, 33), (600, 13, 16, 13), (600, 15, 18, 15), (600, 16, 18, 15), (600, 17, 20, 16),
        ])
    }

    func testAFallbackFacesLineBoxIsChromes() {
        assertChromeFallback([
            ("สวัสดี", 13, 18, 14), ("明天", 13, 16, 13), ("नमस्ते", 13, 20, 14), ("สวัสดี", 15, 20, 16),
            ("明天", 15, 18, 15), ("नमस्ते", 15, 23, 16), ("สวัสดี", 16, 22, 17), ("明天", 16, 18, 15),
            ("नमस्ते", 16, 24, 17), ("สวัสดี", 21, 29, 23), ("明天", 21, 24, 20), ("नमस्ते", 21, 32, 23),
        ])
    }

    func testASetLineHeightCentresChromesContentArea() {
        assertChromeExplicit([
            ("20px", 13, 20, 15), ("20px", 16, 20, 16), ("20px", 20, 20, 17), ("24px", 13, 24, 17),
            ("24px", 20, 24, 19), ("17px", 13, 17, 13), ("17px", 16, 17, 14), ("17px", 20, 17, 16),
            ("1.2", 13, 15.5938, 12), ("1.2", 16, 19.1875, 15), ("1.2", 17, 20.3906, 16), ("1.5", 13, 19.5, 14),
            ("1.5", 17, 25.5, 18), ("25.3px", 13, 25.2969, 17), ("25.3px", 17, 25.2969, 18), ("25.3px", 20, 25.2969, 20),
        ])
    }

    func testIntrinsicWidthsAreChromes() { assertChromeWidths() }

    /// LLP 1093 D6: a paragraph in a multi-column flow breaks once, at the
    /// column width, and the kernel cuts between its line boxes. Chrome 154
    /// on this Mac, `column-count: 3; column-gap: 24px; width: 600px; font:
    /// 14px/20px system-ui`: the first paragraph's lines, four in each of two
    /// columns (each word's client rect). The line boxes the hook answers
    /// are the ones the kernel cuts between: 20px apart.
    func testAParagraphInColumnsBreaksAsChromesColumns() {
        let engine = TextEngine(resolve: { _ in nil })
        let text = "One two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twenty-one twenty-two twenty-three twenty-four twenty-five twenty-six."
        let run = Run(text: text, size: 14, weight: 400, family: 0, italic: false, lineHeight: 20, letterSpacing: 0)
        let p = engine.paragraph(Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run), width: 184)
        let chrome = ["One two three four five six", "seven eight nine ten eleven", "twelve thirteen fourteen", "fifteen sixteen seventeen",
                      "eighteen nineteen twenty", "twenty-one twenty-two", "twenty-three twenty-four", "twenty-five twenty-six."]
        let source = text as NSString
        XCTAssertEqual(p.lines.count, chrome.count)
        for (line, words) in zip(p.lines, chrome) {
            let r = CTLineGetStringRange(line)
            XCTAssertEqual(source.substring(with: NSRange(location: r.location, length: r.length)).trimmingCharacters(in: .whitespaces), words)
        }
        XCTAssertEqual(p.lineBottoms, (1...8).map { CGFloat($0 * 20) })
    }

    /// The reader diary's book typography, against Chrome 154 on this Mac at
    /// scale 1 (`font: 16px/24px system-ui`, Range client rects): justified
    /// lines end at the box's edge but the last; a line broken at a soft
    /// hyphen shows one; an installed family's italic and bold are its faces.
    func testJustifiedLinesFillTheBoxButTheLastAsChromes() {
        let engine = TextEngine(resolve: { _ in nil })
        let text = "Justified: It was the best of times, it was the worst of times, it was the age of wisdom, it was the age of foolishness, it was the epoch of belief."
        let run = Run(text: text, size: 16, weight: 400, family: 0, italic: false, lineHeight: 24, letterSpacing: 0)
        let p = engine.paragraph(Spec(runs: [run], align: 3, lineClamp: 0, color: [0, 0, 0, 255], strut: run), width: 300)
        let source = text as NSString
        let chrome: [(String, CGFloat)] = [("Justified: It was the best of times, it was", 300), ("the worst of times, it was the age of", 300),
                                           ("wisdom, it was the age of foolishness, it", 300), ("was the epoch of belief.", 171.984375)]
        XCTAssertEqual(p.lines.count, chrome.count)
        for (line, (words, right)) in zip(p.lines, chrome) {
            let r = CTLineGetStringRange(line)
            XCTAssertEqual(source.substring(with: NSRange(location: r.location, length: r.length)).trimmingCharacters(in: .whitespaces), words)
            let end = r.location + (words as NSString).length
            XCTAssertEqual(CTLineGetOffsetForStringIndex(line, end, nil), right, accuracy: 1.0 / 64 + 1e-6, words)
        }
        XCTAssertEqual(p.height, 96)
        XCTAssertEqual(p.origin(0, align: 3, width: 300), 0)
    }

    func testALineBrokenAtASoftHyphenShowsOneAsChromes() {
        let engine = TextEngine(resolve: { _ in nil })
        let text = "Soft hyphens: an extra\u{AD}ordinarily long word, and an incom\u{AD}prehensibly long one."
        let run = Run(text: text, size: 16, weight: 400, family: 0, italic: false, lineHeight: 24, letterSpacing: 0)
        let p = engine.paragraph(Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run), width: 150)
        let source = text as NSString
        let lines = p.lines.map { line -> String in
            let r = CTLineGetStringRange(line)
            return source.substring(with: NSRange(location: r.location, length: r.length))
        }
        XCTAssertEqual(lines, ["Soft hyphens: an ", "extra\u{AD}ordinarily long ", "word, and an incom\u{AD}", "prehensibly long ", "one."])
        // Chrome: the chosen SHY's client rect is at 141.61; its hyphen follows it.
        let hyphenated = p.lines[2], shy = CTLineGetStringRange(hyphenated).location + 18
        XCTAssertEqual(CTLineGetOffsetForStringIndex(hyphenated, shy, nil), 141.609375, accuracy: 1.0 / 64 + 1e-6)
        let font = CTFontCreateUIFontForLanguage(.system, 16, nil)!
        let dash = CTLineGetTypographicBounds(CTLineCreateWithAttributedString(NSAttributedString(
            string: "-", attributes: [.font: font])), nil, nil, nil)
        XCTAssertEqual(CTLineGetTypographicBounds(hyphenated, nil, nil, nil), 141.609375 + dash, accuracy: 0.05)
        // An unchosen SHY stays invisible and adds nothing.
        XCTAssertEqual(CTLineGetTypographicBounds(p.lines[1], nil, nil, nil) - CTLineGetTrailingWhitespaceWidth(p.lines[1]), 138.828125, accuracy: 1.0 / 64 + 1e-6)
    }

    /// `text-indent: 32px` and `hyphens: auto` (`lang="en"`) in Chrome 154,
    /// each character's client rect: where each line starts and ends.
    func testTextIndentAndAutoHyphensBreakAsChromes() {
        let engine = TextEngine(resolve: { _ in nil })
        func spec(_ text: String) -> Spec {
            let run = Run(text: text, size: 16, weight: 400, family: 0, italic: false, lineHeight: 24, letterSpacing: 0)
            return Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run)
        }
        var indented = spec("Indented: It was the best of times, it was the worst of times, it was the age of wisdom.")
        indented.textIndent = 32
        let p = engine.paragraph(indented, width: 300)
        let chrome: [(Int, CGFloat, CGFloat)] = [(39, 32, 290.8125), (42, 0, 290.640625), (7, 0, 60.484375)]
        XCTAssertEqual(p.lines.count, chrome.count)
        for (i, (count, left, right)) in chrome.enumerated() where i < p.lines.count {
            let r = CTLineGetStringRange(p.lines[i])
            XCTAssertEqual(r.length, count, "line \(i)")
            let x = p.origin(i, align: 0, width: 300)
            XCTAssertEqual(x, left, accuracy: 1.0 / 64, "line \(i) starts")
            // A line's end space hangs: Chrome gives it no width.
            let hangs = i + 1 < chrome.count ? 1 : 0
            XCTAssertEqual(x + CTLineGetOffsetForStringIndex(p.lines[i], r.location + r.length - hangs, nil), right, accuracy: 1.0 / 64 + 1e-6, "line \(i) ends")
        }
        var auto = spec("Auto: characteristically incomprehensible typographical considerations.")
        auto.hyphens = 2; auto.language = "en"
        auto.hyphenateAuto()
        let q = engine.paragraph(auto, width: 150), text = auto.runs[0].text as NSString
        let lines = q.lines.map { line -> String in
            let r = CTLineGetStringRange(line)
            return text.substring(with: NSRange(location: r.location, length: r.length))
        }
        XCTAssertEqual(lines.map { $0.replacingOccurrences(of: "\u{AD}", with: "") },
                       ["Auto: characteristi", "cally incomprehen", "sible typographical ", "considerations."])
        XCTAssertTrue(lines[0].hasSuffix("\u{AD}") && lines[1].hasSuffix("\u{AD}"), "broken at the inserted soft hyphens")
        // The source map takes them back out: the "c" of "cally" is source offset 19.
        let second = CTLineGetStringRange(q.lines[1]).location
        XCTAssertEqual(auto.source.source(second), 19)
        XCTAssertEqual(auto.source.collapsed(19), second)
    }

    func testAnInstalledFamilysItalicAndBoldAreItsOwnFacesAsChromes() throws {
        let installed = CTFontDescriptorCreateMatchingFontDescriptors(
            CTFontDescriptorCreateWithAttributes([kCTFontFamilyNameAttribute: "Georgia"] as CFDictionary),
            NSSet(object: kCTFontFamilyNameAttribute) as CFSet) as? [CTFontDescriptor] ?? []
        guard installed.count >= 4 else { throw XCTSkip("Georgia's four faces are not installed") }
        let engine = TextEngine(resolve: { _ in nil })
        let family = Array("Georgia".utf8)
        family.withUnsafeBufferPointer { f in
            var face = ExactFontFace()
            face.family = f.baseAddress; face.family_len = f.count
            face.source = nil; face.source_len = 0; face.stack = 8; face.weight = 0; face.italic = 0
            withUnsafePointer(to: &face) { row in
                var catalog = ExactFontCatalog(); catalog.faces = row; catalog.count = 1
                withUnsafePointer(to: &catalog) { engine.install($0) }
            }
        }
        // `font: <style> <weight> 20px Georgia` nowrap span widths in Chrome.
        for (text, weight, italic, width, name) in [("Georgia italic", 400, true, 122.65625, "Georgia-Italic"),
                                                    ("Georgia bold", 700, false, 132.0625, "Georgia-Bold"),
                                                    ("Georgia italic", 400, false, 117.609375, "Georgia"),
                                                    ("Georgia bold", 600, true, 0, "Georgia-BoldItalic")] {
            XCTAssertEqual(CTFontCopyPostScriptName(engine.font(size: 20, weight: weight, family: 8, italic: italic) as CTFont) as String, name)
            guard width > 0 else { continue }
            let run = Run(text: text, size: 20, weight: weight, family: 8, italic: italic, lineHeight: nil, letterSpacing: 0)
            let p = engine.paragraph(Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run), width: .infinity)
            XCTAssertEqual(p.width, width, accuracy: 1.0 / 64 + 1e-6, text)
        }
    }

    /// Min-content is the widest word, each rounded up to the layout unit.
    func testIntrinsicWordScalarsKeepFontMetricsAndExactUnicodeSource() {
        let engine = TextEngine(resolve: { _ in nil })
        var small = Run(text: "Café", size: 13.25, weight: 400, family: 0, italic: false, lineHeight: 18.125, letterSpacing: 0)
        small.text = String(repeating: "Café Cafe\u{301} 🦀 ", count: 80)
        var large = small; large.size = 31.25; large.family = 5
        let input = Spec(runs: [small, large], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        var expected: CGFloat = 0
        for run in [small, large] {
            for word in ["Café", "Cafe\u{301}", "🦀"] {
                var one = input; var r = run; r.text = word; one.runs = [r]
                let line = CTLineCreateWithAttributedString(engine.attributed(one))
                expected = max(expected, ceil(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)) * 64) / 64)
            }
        }
        XCTAssertEqual(engine.minContentWidth(input), expected)
        XCTAssertEqual(engine.minContentWidth(input), expected)
        XCTAssertEqual(engine.residencyStats.liveParagraphs, 0)
        XCTAssertEqual(engine.residencyStats.liveShapes, 0)
        XCTAssertEqual(engine.residencyStats.scalarEntries, 1)
    }
}
#endif

#if os(macOS)
import XCTest
import AppKit
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

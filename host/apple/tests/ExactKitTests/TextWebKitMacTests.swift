#if os(macOS)
import XCTest
import AppKit
@testable import ExactKit

/// WKWebView on macOS 27 (TextWebKitCases.swift): macOS WebKit rounds a
/// face's ascent, descent and line gap to the nearest whole pixel. It also
/// floors a set line height to a whole pixel, which is not followed (Chrome
/// keeps the fraction; LLP 1001 §6), so no set-height case is pinned here.
final class TextWebKitMacTests: XCTestCase {
    func testNormalLineHeightIsWebKitsForTheSystemFaces() {
        assertWebKitNormal([
            (0, 400, 11, 13, 11), (0, 400, 12, 15, 12), (0, 400, 13, 16, 13), (0, 400, 15, 18, 15), (0, 400, 16, 18, 15),
            (0, 400, 17, 20, 16), (0, 400, 20, 23, 19), (0, 400, 24, 28, 23), (0, 400, 28, 33, 27), (0, 400, 34, 40, 33),
            (0, 600, 13, 16, 13), (0, 600, 16, 18, 15), (5, 400, 13, 16, 13), (5, 400, 16, 18, 15), (5, 600, 17, 20, 16),
        ])
    }

    func testAFallbackFacesLineBoxIsWebKits() {
        assertWebKitFallback([
            ("مرحبا", 13, 16, 13), ("สวัสดี", 13, 18, 14), ("明天", 13, 16, 13), ("مرحبا", 15, 19, 15),
            ("สวัสดี", 15, 20, 16), ("明天", 15, 18, 15), ("مرحبا", 16, 20, 16), ("สวัสดี", 16, 22, 17),
            ("明天", 16, 18, 15), ("مرحبا", 21, 26, 21), ("สวัสดี", 21, 29, 23), ("明天", 21, 24, 20),
        ])
    }

    func testIntrinsicWidthsAreWebKitsAdvances() { assertWebKitWidths() }

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

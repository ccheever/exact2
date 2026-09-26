// Chrome is the parity oracle (the web smoke, `browser_cases.rs`): Apple
// text lays out as the web host does in Chrome. Each table is captured with
// `getBoundingClientRect` in headless Chrome 154 on macOS 26.6, 2026-09-26,
// for `<div style="font: <weight> <size>px system-ui; line-height: <lh>">
// text<span style="display:inline-block"></span></div>`: the div's height and
// the empty inline-block's bottom (the first baseline). At device scale 1,
// as the web smoke runs Chrome: at 3x Chrome rounds a face's metrics in
// device pixels instead (a 16 px line is 18.67, not 18).
import XCTest
import CoreText
@testable import ExactKit

/// (weight, size, height, baseline) for "Hxg" in `system-ui`.
typealias ChromeNormalCase = (Int, CGFloat, CGFloat, CGFloat)
/// (text after "H ", size, height, baseline) in `system-ui`: a fallback face's line box.
typealias ChromeFallbackCase = (String, CGFloat, CGFloat, CGFloat)
/// (line-height, size, height, baseline) for "Hxg" in `system-ui`.
typealias ChromeExplicitCase = (String, CGFloat, CGFloat, CGFloat)
/// (text, size, weight, width): a `nowrap` span's width, in 1/64 px layout units.
let chromeWidths: [(String, CGFloat, Int, CGFloat)] = [
    ("2", 13, 600, 8.125), ("33", 13, 600, 16.875), ("Heavy list", 17, 600, 77.890625),
    ("Live: off", 13, 400, 48.734375), ("20d ago", 13, 400, 49.921875),
]

extension XCTestCase {
    func paragraph(_ engine: TextEngine, _ text: String, size: CGFloat, weight: Int = 400,
                   lineHeight: CGFloat? = nil) -> Paragraph {
        let run = Run(text: text, size: size, weight: weight, family: 0, italic: false,
                      lineHeight: lineHeight, letterSpacing: 0)
        return engine.paragraph(Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run), width: .infinity)
    }

    func assertChromeNormal(_ cases: [ChromeNormalCase], baselines: Bool = true,
                            file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (weight, size, height, baseline) in cases {
            let p = paragraph(engine, "Hxg", size: size, weight: weight)
            XCTAssertEqual(p.height, height, "height, \(weight) \(size)px", file: file, line: line)
            if baselines { XCTAssertEqual(p.firstBaseline, baseline, "baseline, \(weight) \(size)px", file: file, line: line) }
        }
    }

    func assertChromeFallback(_ cases: [ChromeFallbackCase], file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (text, size, height, baseline) in cases {
            let p = paragraph(engine, "H " + text, size: size)
            XCTAssertEqual(p.height, height, "height, \(text) \(size)px", file: file, line: line)
            XCTAssertEqual(p.firstBaseline, baseline, "baseline, \(text) \(size)px", file: file, line: line)
        }
    }

    func assertChromeExplicit(_ cases: [ChromeExplicitCase], file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (value, size, height, baseline) in cases {
            let used = value.hasSuffix("px") ? CGFloat(Double(value.dropLast(2))!) : CGFloat(Double(value)!) * size
            let p = paragraph(engine, "Hxg", size: size, lineHeight: used)
            // Chrome holds a length in 1/64 px layout units.
            XCTAssertEqual(p.height, height, accuracy: 1.0 / 64, "height, \(value) \(size)px", file: file, line: line)
            XCTAssertEqual(p.firstBaseline, baseline, accuracy: 1.0 / 64, "baseline, \(value) \(size)px", file: file, line: line)
        }
    }

    /// An intrinsic width is the face's advance at the CSS weight in 1/64 px
    /// layout units. Chrome sums HarfBuzz's positions, so its last unit can
    /// differ by one ("33": 16.875 against CoreText's 16.876 rounded up).
    func assertChromeWidths(file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (text, size, weight, width) in chromeWidths {
            XCTAssertEqual(paragraph(engine, text, size: size, weight: weight).width, width, accuracy: 1.0 / 64 + 1e-9,
                           "width of \(text)", file: file, line: line)
        }
    }
}

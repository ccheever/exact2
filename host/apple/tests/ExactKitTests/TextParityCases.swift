// Chrome is the parity oracle (the web smoke, `browser_cases.rs`): Apple
// text lays out as the web host does in Chrome. Each table is captured with
// `getBoundingClientRect` in headless Chrome on macOS, for `<div style="font:
// <weight> <size>px system-ui; line-height: <lh>">text<span
// style="display:inline-block"></span></div>`: the div's height and the empty
// inline-block's top (the first baseline). Chrome rounds a face's metrics and
// floors the half-leading in device pixels, so each table names its device
// scale (`--force-device-scale-factor`): 1, as the web smoke runs Chrome
// (Chrome 154, 2026-09-26), and 2, a Retina Mac's (Chrome 156, 2026-11-04).
// The engine rounds at its `lineScale` (LLP 1008 §3).
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
/// At 600 Exact draws the platform's semibold where Chrome sets `wght` 600
/// (LLP 1019 §13 D11, a declared deviation): that text is the named
/// instance's width, within 0.4% of Chrome's.
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

    /// An engine whose line boxes round at `scale` device pixels a point.
    func chromeEngine(scale: CGFloat) -> TextEngine {
        let engine = TextEngine(resolve: { _ in nil })
        engine.setLineScale(scale)
        return engine
    }

    func assertChromeNormal(_ cases: [ChromeNormalCase], scale: CGFloat = 1, baselines: Bool = true,
                            file: StaticString = #filePath, line: UInt = #line) {
        let engine = chromeEngine(scale: scale)
        for (weight, size, height, baseline) in cases {
            let p = paragraph(engine, "Hxg", size: size, weight: weight)
            XCTAssertEqual(p.height, height, "height, \(weight) \(size)px at \(scale)x", file: file, line: line)
            if baselines { XCTAssertEqual(p.firstBaseline, baseline, "baseline, \(weight) \(size)px at \(scale)x", file: file, line: line) }
        }
    }

    func assertChromeFallback(_ cases: [ChromeFallbackCase], scale: CGFloat = 1, file: StaticString = #filePath, line: UInt = #line) {
        let engine = chromeEngine(scale: scale)
        for (text, size, height, baseline) in cases {
            let p = paragraph(engine, "H " + text, size: size)
            XCTAssertEqual(p.height, height, "height, \(text) \(size)px at \(scale)x", file: file, line: line)
            XCTAssertEqual(p.firstBaseline, baseline, "baseline, \(text) \(size)px at \(scale)x", file: file, line: line)
        }
    }

    func assertChromeExplicit(_ cases: [ChromeExplicitCase], scale: CGFloat = 1, file: StaticString = #filePath, line: UInt = #line) {
        let engine = chromeEngine(scale: scale)
        for (value, size, height, baseline) in cases {
            let used = value.hasSuffix("px") ? CGFloat(Double(value.dropLast(2))!) : CGFloat(Double(value)!) * size
            let p = paragraph(engine, "Hxg", size: size, lineHeight: used)
            // Chrome holds a length in 1/64 px layout units.
            XCTAssertEqual(p.height, height, accuracy: 1.0 / 64, "height, \(value) \(size)px at \(scale)x", file: file, line: line)
            XCTAssertEqual(p.firstBaseline, baseline, accuracy: 1.0 / 64, "baseline, \(value) \(size)px at \(scale)x", file: file, line: line)
        }
    }

    /// An intrinsic width is the face's advance at the CSS weight in 1/64 px
    /// layout units. Chrome sums HarfBuzz's positions, so its last unit can
    /// differ by one ("33": 16.875 against CoreText's 16.876 rounded up).
    func assertChromeWidths(file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (text, size, weight, width) in chromeWidths {
            let ours = paragraph(engine, text, size: size, weight: weight).width
            guard weight != 400 else {
                XCTAssertEqual(ours, width, accuracy: 1.0 / 64 + 1e-9, "width of \(text)", file: file, line: line)
                continue
            }
            let named = PlatformFont.systemFont(ofSize: size, weight: TextEngine.namedWeight(weight))
            let typeset = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: named]))
            let platform = (CGFloat(CTLineGetTypographicBounds(typeset, nil, nil, nil)) * 64).rounded(.up) / 64
            XCTAssertEqual(ours, platform, accuracy: 1.0 / 64 + 1e-9, "width of \(text), the platform's weight", file: file, line: line)
            XCTAssertEqual(ours, width, accuracy: width * 0.004, "width of \(text) against Chrome's", file: file, line: line)
        }
    }
}

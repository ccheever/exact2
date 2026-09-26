// The web is the parity oracle (CLAUDE.md): Apple text lays out as WebKit
// lays out the same CSS. Each table is captured from `getBoundingClientRect`
// in WebKit itself, 2026-09-26 — Safari on the iOS 27 simulator (iPhone 17
// Pro) and WKWebView on macOS 27 — for `<div style="font: <weight> <size>px
// <family>; line-height: <lh>">text<span style="display:inline-block"></span>`:
// the div's height and the empty inline-block's bottom (the first baseline).
import XCTest
import CoreText
@testable import ExactKit

/// (family, weight, size, height, baseline): family 0 is `system-ui`, 5 `ui-monospace`.
typealias WebKitNormalCase = (Int, Int, CGFloat, CGFloat, CGFloat)
/// (text after "H ", size, height, baseline) in `system-ui`: a fallback face's line box.
typealias WebKitFallbackCase = (String, CGFloat, CGFloat, CGFloat)
/// (line-height, size, height, baseline) for "Hxg" in `system-ui`.
typealias WebKitExplicitCase = (String, CGFloat, CGFloat, CGFloat)
/// (text, size, weight, width): `getBoundingClientRect().width` of a `nowrap` span.
let webKitWidths: [(String, CGFloat, Int, CGFloat)] = [
    ("2", 13, 600, 8.112283706665039), ("33", 13, 600, 16.876256942749023), ("😮", 14, 400, 20),
    ("Heavy list", 17, 600, 77.87490844726562), ("Live: off", 13, 400, 48.73095703125), ("20d ago", 13, 400, 49.91796875),
]

extension XCTestCase {
    private func paragraph(_ engine: TextEngine, _ text: String, size: CGFloat, weight: Int = 400, family: Int = 0,
                           lineHeight: CGFloat? = nil) -> Paragraph {
        let run = Run(text: text, size: size, weight: weight, family: family, italic: false,
                      lineHeight: lineHeight, letterSpacing: 0)
        return engine.paragraph(Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run), width: .infinity)
    }

    func assertWebKitNormal(_ cases: [WebKitNormalCase], file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (family, weight, size, height, baseline) in cases {
            let p = paragraph(engine, "Hxg", size: size, weight: weight, family: family)
            XCTAssertEqual(p.height, height, "height, family \(family) \(weight) \(size)px", file: file, line: line)
            XCTAssertEqual(p.firstBaseline, baseline, "baseline, family \(family) \(weight) \(size)px", file: file, line: line)
        }
    }

    func assertWebKitFallback(_ cases: [WebKitFallbackCase], file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (text, size, height, baseline) in cases {
            let p = paragraph(engine, "H " + text, size: size)
            XCTAssertEqual(p.height, height, "height, \(text) \(size)px", file: file, line: line)
            XCTAssertEqual(p.firstBaseline, baseline, "baseline, \(text) \(size)px", file: file, line: line)
        }
    }

    func assertWebKitExplicit(_ cases: [WebKitExplicitCase], file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (value, size, height, baseline) in cases {
            let used = value.hasSuffix("px") ? CGFloat(Double(value.dropLast(2))!) : CGFloat(Double(value)!) * size
            let p = paragraph(engine, "Hxg", size: size, lineHeight: used)
            // WebKit holds a length in 1/64 px layout units.
            XCTAssertEqual(p.height, height, accuracy: 1.0 / 64, "height, \(value) \(size)px", file: file, line: line)
            XCTAssertEqual(p.firstBaseline, baseline, accuracy: 1.0 / 64, "baseline, \(value) \(size)px", file: file, line: line)
        }
    }

    /// An intrinsic width is the browser's advance, rounded up to a layout unit.
    func assertWebKitWidths(file: StaticString = #filePath, line: UInt = #line) {
        let engine = TextEngine(resolve: { _ in nil })
        for (text, size, weight, width) in webKitWidths {
            let p = paragraph(engine, text, size: size, weight: weight)
            XCTAssertEqual(p.width, ceil(width * 64) / 64, accuracy: 1.0 / 64, "width of \(text)", file: file, line: line)
        }
    }
}

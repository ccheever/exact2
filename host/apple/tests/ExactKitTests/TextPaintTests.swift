// One line painter for every Apple text path: glyphs where CoreText measured
// them, fallback faces with vertical glyph offsets included, as the web paints.
import XCTest
import CoreText
@testable import ExactKit

final class TextPaintTests: XCTestCase {
    let engine = TextEngine(resolve: { _ in nil })
    private let arabic = "مرحبا، سنلتقي في المحطة غداً."

    private func run(_ text: String, weight: Int = 400) -> Run {
        Run(text: text, size: 16, weight: weight, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
    }

    /// Premultiplied RGBA, rows top-down, at scale 1, painted y-down as a view does.
    private func paint(_ p: Paragraph, _ spec: Spec, size: CGSize) -> (bytes: [UInt8], width: Int, height: Int) {
        let width = Int(size.width), height = Int(size.height)
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        bytes.withUnsafeMutableBytes { raw in
            let ctx = CGContext(data: raw.baseAddress, width: width, height: height, bitsPerComponent: 8,
                                bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            ctx.translateBy(x: 0, y: CGFloat(height)); ctx.scaleBy(x: 1, y: -1)
            TextEngine.draw(p, spec: spec, in: CGRect(x: 0, y: 20, width: size.width, height: p.height), context: ctx)
        }
        return (bytes, width, height)
    }

    func testFallbackGlyphOffsetsPaintInsideTheMeasuredLineBox() {
        // SF Arabic's cursive attachment moves glyphs vertically. Flipped by
        // the text matrix those offsets were inverted and the ink fell below
        // the line box: the last line of m478 in the heavy list was clipped.
        let spec = Spec(runs: [run("notes ", weight: 600), run(arabic)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let p = engine.paragraph(spec, width: 318)
        XCTAssertEqual(p.lines.count, 1)
        var lowest = CGFloat.infinity
        for glyphRun in CTLineGetGlyphRuns(p.lines[0]) as! [CTRun] {
            lowest = min(lowest, CTRunGetImageBounds(glyphRun, nil, CFRange()).minY)
        }
        let shot = paint(p, spec, size: CGSize(width: 340, height: 80))
        var bottom = 0
        for y in 0..<shot.height { for x in 0..<shot.width where shot.bytes[(y * shot.width + x) * 4 + 3] > 16 { bottom = max(bottom, y) } }
        // The ink ends where CoreText's geometry says, inside the line box.
        let expected = 20 + p.baselines[0].rounded() - lowest
        XCTAssertLessThanOrEqual(CGFloat(bottom), expected + 1)
        XCTAssertLessThanOrEqual(CGFloat(bottom), 20 + p.height)
    }
}

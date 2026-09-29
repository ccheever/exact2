// One line painter for every Apple text path: glyphs where CoreText measured
// them (fallback faces with vertical glyph offsets included), and an inline
// box's background under each of its line fragments, as the web paints both.
import XCTest
import CoreText
#if os(macOS)
import IOSurface
#endif
@testable import ExactKit

final class TextPaintTests: XCTestCase {
    let engine = TextEngine(resolve: { _ in nil })
    private let arabic = "مرحبا، سنلتقي في المحطة غداً."

    private func run(_ text: String, weight: Int = 400, background: [Double]? = nil) -> Run {
        Run(text: text, size: 16, weight: weight, family: 0, italic: false, lineHeight: nil,
            letterSpacing: 0, background: background)
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

    func testAnInlineBackgroundCoversEachLineFragmentOfItsRun() throws {
        let gray: [Double] = [242, 242, 247, 255]
        let spec = Spec(runs: [run("plain "), run("list_viewport code run", background: gray), run(" after")],
                        align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let p = engine.paragraph(spec, width: 150)
        XCTAssertGreaterThan(p.lines.count, 1, "the run wraps")
        let fragments = p.lines.map { TextLinePaint.backgrounds($0, at: .zero) }
        XCTAssertEqual(fragments.filter { !$0.isEmpty }.count, 2, "one background per line fragment of the run")
        let font = engine.font(size: 16, weight: 400, family: 0, italic: false)
        let first = try XCTUnwrap(fragments[0].first?.0)
        let (ascent, descent) = CSSLineBox.content(font as CTFont)
        XCTAssertEqual(first.minY, -ascent, accuracy: 0.01, "the content area of the run's font, whole pixels as Chrome's")
        XCTAssertEqual(first.height, ascent + descent, accuracy: 0.01)
        let plainWidth = CGFloat(CTLineGetOffsetForStringIndex(p.lines[0], 6, nil))
        XCTAssertEqual(first.minX, plainWidth, accuracy: 0.5, "starts where the run does")

        // Painted: the fragment's corner is the background, the text before it is not.
        let shot = paint(p, spec, size: CGSize(width: 160, height: 100))
        func pixel(_ x: CGFloat, _ y: CGFloat) -> [UInt8] {
            let i = (Int(y) * shot.width + Int(x)) * 4
            return Array(shot.bytes[i..<i + 4])
        }
        let top = 20 + p.baselines[0].rounded() - font.ascender
        XCTAssertEqual(pixel(first.minX + 1, top + 1), [242, 242, 247, 255])
        XCTAssertEqual(pixel(1, top + 1)[3], 0, "no background under the plain run")
    }

    func testBackgroundIsPaintNotMetrics() {
        let plain = Spec(runs: [run("a "), run("code")], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        var painted = plain
        painted.runs[1].background = [242, 242, 247, 255]
        XCTAssertEqual(plain.geometry, painted.geometry)
        XCTAssertNotEqual(TextPaint(plain), TextPaint(painted))
        XCTAssertEqual(engine.paragraph(plain, width: 300).height, engine.paragraph(painted, width: 300).height)
        XCTAssertNil(engine.attributed(plain).attribute(.exactBackground, at: 3, effectiveRange: nil))
        XCTAssertNotNil(engine.attributed(painted).attribute(.exactBackground, at: 3, effectiveRange: nil))
    }

    func testAnInlineRowCarriesItsLightAndDarkBackground() throws {
        let row = try BatchFields(["id": 7, "parent": 1, "props": .object(["text": "code"]), "paint": true,
            "style": .object(["background_color": [[242, 242, 247, 255], [44, 44, 46, 255]]])]).inline()
        XCTAssertEqual(row.run(dark: false).background, [242, 242, 247, 255])
        XCTAssertEqual(row.run(dark: true).background, [44, 44, 46, 255])
        XCTAssertTrue(row.hasSchemeColor)
    }

    /// A `line-clamp` paragraph rasters from its published geometry (LLP
    /// 1071 §8.1): a worker shapes the lines, the last one again from the
    /// range it broke at, ending in "…". The same pixels as layout's lines.
    func testAClampedParagraphRastersFromItsGeometryAsLayoutPaintsIt() throws {
        let text = "Maybe family sounds draft later scroll deadline picnic thanks soon a meeting at the station"
        for align in [0, 1, 2] {
            let spec = Spec(runs: [run(text)], align: align, lineClamp: 2, color: [30, 60, 90, 255])
            let p = engine.paragraph(spec, width: 180)
            XCTAssertEqual(p.lines.count, 2)
            let clamped = try XCTUnwrap(p.clampedRange, "the last line was clamped")
            let geometry = try XCTUnwrap(engine.measuredBreaks(spec, width: 180))
            XCTAssertEqual(geometry.clamped?.location, clamped.location)
            XCTAssertEqual(geometry.ranges.last?.length, clamped.length, "the range it broke at, not the ellipsized line's")
            let box = CGRect(x: 0, y: 0, width: 180, height: p.height)
            func job(_ clamp: CFRange?) -> TextRasterJob {
                TextRasterJob(source: engine.attributed(spec), ranges: geometry.ranges, baselines: geometry.baselines,
                              flush: align == 1 ? 0.5 : align == 2 ? 1 : 0, box: box, size: box.size, scale: 2, clamped: clamp)
            }
            let shaped = try XCTUnwrap(job(geometry.clamped).render())
            let laidOut = try XCTUnwrap(job(geometry.clamped).render(lines: p.lines))
            XCTAssertEqual(shaped.frame, laidOut.frame)
            XCTAssertEqual(rasterBytes(shaped), rasterBytes(laidOut), "align \(align)")
            XCTAssertNotEqual(rasterBytes(try XCTUnwrap(job(nil).render())), rasterBytes(laidOut), "the clamp paints its ellipsis")
        }
    }

    private func rasterBytes(_ r: TextRasterImage) -> Data {
        #if os(iOS)
        return (r.image.dataProvider?.data as Data?) ?? Data()
        #else
        r.surface.lock(options: .readOnly, seed: nil)
        defer { r.surface.unlock(options: .readOnly, seed: nil) }
        return Data(bytes: r.surface.baseAddress, count: r.surface.allocationSize)
        #endif
    }
}

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

    /// LLP 1077 D3/D7 per inline run: a run's own computed `text-shadow`
    /// and `-webkit-text-stroke` cross on its row (`none` and a zero width
    /// as no row and 0), `currentcolor` is the run's own colour, and a
    /// shadow only some runs have is theirs, drawn around their glyphs.
    func testEachRunCarriesItsOwnShadowAndStroke() throws {
        func row(_ id: UInt32, _ text: String, _ style: NodeStyle) throws -> InlineText {
            try BatchFields(["id": .number(Double(id)), "parent": 1, "props": .object(["text": .string(text)]), "paint": true,
                             "style": .object(style)]).inline()
        }
        let red: BatchValue = [229, 57, 53, 255]
        let plain = try row(2, "Pl ", ["text_color": [33, 33, 33, 255], "text_stroke_width": 0, "text_stroke_color": "currentcolor"])
        let glow = try row(3, "Glow", ["text_color": [[33, 33, 33, 255], [238, 238, 238, 255]],
                                       "text_shadow": ["o": [0, 0], "b": 6, "c": red],
                                       "text_stroke_width": 2, "text_stroke_color": "currentcolor"])
        XCTAssertNil(plain.run(dark: false).shadow)
        XCTAssertNil(plain.run(dark: false).stroke, "a zero width is no stroke")
        XCTAssertEqual(glow.run(dark: false).shadow, [0, 0, 6, 229, 57, 53, 255])
        XCTAssertEqual(glow.run(dark: false).stroke, [2, 33, 33, 33, 255], "currentcolor is the run's colour")
        XCTAssertEqual(glow.run(dark: true).stroke, [2, 238, 238, 238, 255], "in its appearance")

        var spec = Spec(runs: [plain.run(dark: false), glow.run(dark: false)], align: 0, lineClamp: 0, color: [33, 33, 33, 255])
        spec.gatherShadows()
        XCTAssertNil(spec.shadow, "the runs differ: each keeps its own")
        let attributed = engine.attributed(spec)
        XCTAssertNil(attributed.attribute(.exactShadow, at: 0, effectiveRange: nil))
        XCTAssertNotNil(attributed.attribute(.exactShadow, at: 4, effectiveRange: nil))
        XCTAssertNil(attributed.attribute(.strokeWidth, at: 0, effectiveRange: nil))
        XCTAssertEqual(try XCTUnwrap(attributed.attribute(.strokeWidth, at: 4, effectiveRange: nil) as? Double), -2 / 16 * 100, accuracy: 1e-9)
        XCTAssertNotEqual(TextPaint(spec), TextPaint(Spec(runs: [plain.run(dark: false), plain.run(dark: false)], align: 0, lineClamp: 0, color: [33, 33, 33, 255])))

        // Painted: the glow reaches past "Glow" only, not around "Pl ".
        let p = engine.paragraph(spec, width: 300)
        let shot = paint(p, spec, size: CGSize(width: 120, height: 60))
        func reddish(_ x: Int, _ y: Int) -> Bool {
            let i = (y * shot.width + x) * 4
            return shot.bytes[i + 3] > 0 && shot.bytes[i] > shot.bytes[i + 1] + 40
        }
        let split = Int(CTLineGetOffsetForStringIndex(p.lines[0], 3, nil).rounded())
        let rows = 0..<shot.height
        XCTAssertTrue((split..<shot.width).contains { x in rows.contains { reddish(x, $0) } }, "Glow casts its shadow")
        XCTAssertFalse((0..<max(0, split - 10)).contains { x in rows.contains { reddish(x, $0) } }, "Pl casts none")

        // Runs that agree keep the paragraph's one shadow and no attribute.
        var same = spec
        same.runs[0].shadow = same.runs[1].shadow
        same.gatherShadows()
        XCTAssertEqual(same.shadow, [0, 0, 6, 229, 57, 53, 255])
        XCTAssertNil(engine.attributed(same).attribute(.exactShadow, at: 4, effectiveRange: nil))
    }

    /// A run's own shadow past the raster's 256-point ink allowance stays in
    /// its pixels, as the layer shadow a uniform paragraph casts would, up
    /// to `maxShadowReach`; a huge one is cut there (LLP 1077 D3).
    func testARunShadowFarPastTheBoxStaysInTheRasterUpToItsCap() throws {
        func frame(_ dx: Double) throws -> CGRect {
            var far = run("Cd", weight: 800)
            far.shadow = [dx, 0, 0, 255, 0, 0, 255]
            var spec = Spec(runs: [run("Ab ", weight: 800), far], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
            spec.gatherShadows()
            XCTAssertNil(spec.shadow, "mixed runs: the raster carries the shadow")
            let p = engine.paragraph(spec, width: 200)
            let box = CGRect(x: 0, y: 0, width: 200, height: 60)
            let job = TextRasterJob(source: engine.attributed(spec), ranges: p.lines.map { CTLineGetStringRange($0) },
                                    baselines: p.baselines, flush: 0, box: box, size: box.size, scale: 2)
            return try XCTUnwrap(job.render()).frame
        }
        let cd = CGFloat(CTLineGetOffsetForStringIndex(engine.paragraph(
            Spec(runs: [run("Ab ", weight: 800), run("Cd", weight: 800)], align: 0, lineClamp: 0, color: [0, 0, 0, 255]),
            width: 200).lines[0], 3, nil))
        XCTAssertGreaterThan(try frame(400).maxX, cd + 400, "the shadow of Cd, 400 points on, is in the pixels")
        let cap = 200 + TextRasterJob.maxShadowReach
        XCTAssertEqual(try frame(Double(cap - cd - 5)).maxX, cap, accuracy: 1, "a shadow across the cap is cut there")
        XCTAssertLessThan(try frame(5000).maxX, cap, "one wholly past it adds nothing")
    }

    /// A `line-clamp` paragraph rasters from its published geometry (LLP
    /// 1072 §8.1): a worker shapes the lines, the last one again from the
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

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
        let first = try XCTUnwrap(fragments[0].first?.rect)
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

    /// An inline box's `border-radius` (issue #725), sliced as CSS's
    /// `box-decoration-break: slice` slices it: its start corners on its
    /// first fragment, its end corners on its last, each box its own.
    func testAnInlineBoxsRadiusRoundsOnlyItsOwnEnds() throws {
        let teal: [Double] = [0, 202, 177, 255]
        func box(_ text: String, _ id: UInt32) -> Run {
            var r = run(text, background: teal)
            r.backgroundBox = id
            r.backgroundRadii = Array(repeating: InlineRadius(px: 4), count: 4)
            return r
        }
        func corners(_ fill: TextLinePaint.Fill) throws -> [Bool] {
            let path = try XCTUnwrap(fill.path, "a rounded fragment is an outline")
            let r = fill.rect, d: CGFloat = 0.4
            // Whether each corner's own point is filled: false where it is rounded.
            return [CGPoint(x: r.minX + d, y: r.minY + d), CGPoint(x: r.maxX - d, y: r.minY + d),
                    CGPoint(x: r.maxX - d, y: r.maxY - d), CGPoint(x: r.minX + d, y: r.maxY - d)].map { !path.contains($0) }
        }
        let fills = { (spec: Spec, width: CGFloat) in self.engine.paragraph(spec, width: width).lines.map { TextLinePaint.backgrounds($0, at: .zero) } }

        // One line: every corner rounded.
        let one = try XCTUnwrap(fills(Spec(runs: [run("left "), box("marked", 7), run(" right")], align: 0, lineClamp: 0, color: [0, 0, 0, 255]), 400).first)
        XCTAssertEqual(one.count, 1)
        XCTAssertEqual(try corners(one[0]), [true, true, true, true])

        // Two boxes of one colour side by side stay two, each rounded.
        let pair = try XCTUnwrap(fills(Spec(runs: [box("one", 1), box("two", 2)], align: 0, lineClamp: 0, color: [0, 0, 0, 255]), 400).first)
        XCTAssertEqual(pair.count, 2, "two boxes, not one joined")
        XCTAssertEqual(pair[0].rect.maxX, pair[1].rect.minX, accuracy: 0.5)
        for fill in pair { XCTAssertEqual(try corners(fill), [true, true, true, true]) }
        // Told apart by character, not by attributes: one CTRun, so they
        // paint as wide as they measured (a ligature across them included).
        let joined = Spec(runs: [box("f", 1), box("i", 2)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let painted = engine.paragraph(joined, width: 400).lines[0], measured = engine.paragraph(joined.geometry, width: 400).lines[0]
        XCTAssertEqual(CTLineGetTypographicBounds(painted, nil, nil, nil), CTLineGetTypographicBounds(measured, nil, nil, nil))

        // Wrapped: the first fragment is open at its end, the last at its start.
        let words = "a box wrapped across lines"
        let wrapped = fills(Spec(runs: [run("plain "), box(words, 3)], align: 0, lineClamp: 0, color: [0, 0, 0, 255]), 120).filter { !$0.isEmpty }
        XCTAssertGreaterThan(wrapped.count, 1, "the box wraps")
        XCTAssertEqual(try corners(wrapped[0][0]), [true, false, false, true], "start corners: the left under ltr")
        XCTAssertEqual(try wrapped.last.map { try corners($0[0]) }, [false, true, true, false])
        if wrapped.count > 2 { XCTAssertNil(wrapped[1][0].path, "a middle fragment is open at both sides") }

        // Right to left, the start is the right.
        var rtl = Spec(runs: [run("plain "), box(words, 4)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        rtl.direction = 1
        // (A wrapped line's trailing space is a piece of its own there, at
        // the line's left end: the words' piece is the widest.)
        let backwards = fills(rtl, 120).filter { !$0.isEmpty }.map { $0.max { $0.rect.width < $1.rect.width }! }
        XCTAssertGreaterThan(backwards.count, 1)
        XCTAssertEqual(try corners(backwards[0]), [false, true, true, false], "start corners: the right under rtl")
        XCTAssertEqual(try backwards.last.map(corners), [true, false, false, true])

        // Bidi: the box's pieces on a line are joined in visual order, so its
        // start is the leftmost piece's left and its end the rightmost's right.
        let mixed = try XCTUnwrap(fills(Spec(runs: [run("abc "), box("אבג 12", 5), run(" 3 דהו xyz")], align: 0, lineClamp: 0,
                                             color: [0, 0, 0, 255]), 400).first)
        let pieces = try mixed.map { $0.path == nil ? [false, false, false, false] : try corners($0) }
        XCTAssertEqual(pieces.first?[0], true); XCTAssertEqual(pieces.first?[3], true)
        XCTAssertEqual(pieces.last?[1], true); XCTAssertEqual(pieces.last?[2], true)
        if pieces.count > 1 { XCTAssertEqual(pieces.first?[1], false); XCTAssertEqual(pieces.last?[0], false) }

        // An ellipsis stands for the cut end: the box stays open there.
        let long = Spec(runs: [box("abcdefghijklmnopqrstuvwxyz", 6)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let source = engine.attributed(long)
        let cut = try XCTUnwrap(TextEngine.ellipsis(engine.paragraph(long, width: 400).lines[0], range: NSRange(location: 0, length: source.length),
                                                    width: 80, source: source))
        let clipped = TextLinePaint.backgrounds(cut, at: .zero)
        XCTAssertEqual(try clipped.first.map(corners), [true, false, false, true])

        // A box's ends count where they hold no ink: a zero-width first
        // character, a last space the line trims.
        let zero = try XCTUnwrap(fills(Spec(runs: [box("\u{200B}hello", 8)], align: 0, lineClamp: 0, color: [0, 0, 0, 255]), 400).first)
        XCTAssertEqual(try zero.first.map(corners), [true, true, true, true])
        let trimmed = fills(Spec(runs: [box("abc ", 9), run("restless")], align: 0, lineClamp: 0, color: [0, 0, 0, 255]), 50)
        XCTAssertGreaterThan(trimmed.count, 1, "the plain word wraps")
        XCTAssertEqual(try trimmed[0].first.map(corners), [true, true, true, true])

        // One glyph for characters of two boxes (a ligature across them) is
        // cut where the caret would stand: both boxes paint.
        let four = Array(repeating: InlineRadius(px: 4), count: 4)
        let table = InlineBoxes([.init(extent: NSRange(location: 0, length: 1), radii: four),
                                 .init(extent: NSRange(location: 1, length: 1), radii: four)], rtl: false)
        let times = CTFontCreateWithName("Times-Roman" as CFString, 32, nil)
        let fill = InlineBackground(color: CGColor(red: 0, green: 0.8, blue: 0.7, alpha: 1), ascent: 30, descent: 8, boxes: table)
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: "fi", attributes: [.font: times, .exactBackground: fill]))
        let halves = TextLinePaint.backgrounds(line, at: .zero)
        XCTAssertEqual(halves.count, 2, "each box its own piece")
        for half in halves { XCTAssertEqual(try corners(half), [true, true, true, true]) }
        XCTAssertEqual(halves.map(\.rect.width).reduce(0, +), CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)), accuracy: 0.01)
        // Lam-alef, one glyph right to left: shared in its direction, the
        // two pieces side by side and the whole glyph covered.
        let lamAlef = CTLineCreateWithAttributedString(NSAttributedString(string: "لا", attributes: [.font: times, .exactBackground: fill]))
        let sides = TextLinePaint.backgrounds(lamAlef, at: .zero).sorted { $0.rect.minX < $1.rect.minX }
        XCTAssertEqual(sides.count, 2)
        if sides.count == 2 {
            XCTAssertEqual(sides[0].rect.maxX, sides[1].rect.minX, accuracy: 0.01, "no overlap and no gap")
            XCTAssertEqual(sides[1].rect.maxX - sides[0].rect.minX, CGFloat(CTLineGetTypographicBounds(lamAlef, nil, nil, nil)), accuracy: 0.01)
        }
    }

    func testAnInlineRowCarriesItsRadiusAndItsBox() throws {
        let row = try BatchFields(["id": 7, "parent": 1, "props": .object(["text": "marked"]), "paint": true,
            "style": .object(["background_color": [0, 202, 177, 255], "border_radius_top_left": 3, "border_radius_top_right": 3,
                              "border_radius_bottom_right": .object(["pct": 50, "px": 0]), "border_radius_bottom_left": 0])]).inline()
        let run = row.run(dark: false)
        XCTAssertEqual(run.backgroundBox, 7)
        XCTAssertEqual(run.backgroundRadii, [InlineRadius(px: 3), InlineRadius(px: 3), InlineRadius(pct: 50), InlineRadius()])
        let square = try BatchFields(["id": 8, "parent": 1, "props": .object(["text": "x"]), "paint": true,
            "style": .object(["border_radius_top_left": 3])]).inline()
        XCTAssertEqual(square.run(dark: false).backgroundBox, 0, "no background: no box to round")
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

    func testShadowedRunsKeepTheWholeLinesDecorations() throws {
        let font = engine.font(size: 32, weight: 400, family: 1, italic: false)
        let attributes: [NSAttributedString.Key: Any] = [
            .font: font, .foregroundColor: PlatformColor.black,
            .underlineStyle: 1, .strikethroughStyle: 1,
            .underlineColor: PlatformColor.red, .strikethroughColor: PlatformColor.blue,
        ]
        let plain = NSAttributedString(string: "MM  WW  II", attributes: attributes)
        let shadowed = NSMutableAttributedString(attributedString: plain)
        let shadow = TextRunShadow([0, 8, 0, 0, 0, 0, 0])
        shadowed.addAttribute(.exactShadow, value: shadow, range: NSRange(location: 4, length: 4))
        func capture(_ string: NSAttributedString, pass: TextLinePaint.Pass = .all) throws -> [UInt8] {
            let ctx = try XCTUnwrap(CGContext(data: nil, width: 240, height: 60, bitsPerComponent: 8,
                bytesPerRow: 240 * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            TextLinePaint.draw(CTLineCreateWithAttributedString(string), at: CGPoint(x: 10, y: 40), in: ctx, pass: pass)
            return Array(UnsafeBufferPointer(start: try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self), count: 240 * 60 * 4))
        }
        let reference = try capture(plain)
        for pass: TextLinePaint.Pass in [.all, .ink] {
            let actual = try capture(shadowed, pass: pass)
            let error = zip(reference, actual).reduce(0) { $0 + abs(Int($1.0) - Int($1.1)) }
            XCTAssertLessThan(error, 500, "a transparent run shadow keeps glyphs, underline and line-through")
        }
        let hidden = NSMutableAttributedString(attributedString: plain)
        hidden.addAttribute(.exactHidden, value: true, range: NSRange(location: 0, length: 4))
        hidden.addAttribute(.exactHidden, value: true, range: NSRange(location: 8, length: 2))
        XCTAssertTrue(try capture(shadowed, pass: .caster(shadow)) == capture(hidden),
                       "the separate shadow caster includes decorations and excludes neighbouring runs")
    }

    func testRunShadowClipsPreserveItalicInkAndThickStroke() throws {
        let font = engine.font(size: 32, weight: 400, family: 1, italic: true)
        let plain = NSAttributedString(string: "f   MM   f", attributes: [
            .font: font, .foregroundColor: PlatformColor.black,
            .strokeWidth: -25, .strokeColor: PlatformColor.blue,
            .underlineStyle: 1, .strikethroughStyle: 1, .underlineColor: PlatformColor.red,
        ])
        let shadowed = NSMutableAttributedString(attributedString: plain)
        shadowed.addAttribute(.exactShadow, value: TextRunShadow([0, 8, 0, 0, 0, 0, 0]),
                              range: NSRange(location: 4, length: 2))
        func capture(_ source: NSAttributedString, pass: TextLinePaint.Pass, reference: Bool = false) throws -> [UInt8] {
            let ctx = try XCTUnwrap(CGContext(data: nil, width: 240, height: 80, bitsPerComponent: 8,
                bytesPerRow: 240 * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            let line = CTLineCreateWithAttributedString(source)
            if reference {
                ctx.translateBy(x: 20, y: 50)
                ctx.scaleBy(x: 1, y: -1)
                ctx.textPosition = .zero
                CTLineDraw(line, ctx)
            } else {
                TextLinePaint.draw(line, at: CGPoint(x: 20, y: 50), in: ctx, pass: pass)
            }
            return Array(UnsafeBufferPointer(start: try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self), count: 240 * 80 * 4))
        }
        let reference = try capture(shadowed, pass: .all, reference: true)
        let end = 20 + CTLineGetTypographicBounds(CTLineCreateWithAttributedString(shadowed), nil, nil, nil)
        for pass: TextLinePaint.Pass in [.all, .ink] {
            let actual = try capture(shadowed, pass: pass)
            var error = 0, ink = 0
            for y in 0..<80 { for x in 0..<240 where x < 20 || Double(x) > end {
                let i = (y * 240 + x) * 4
                ink += Int(reference[i + 3])
                for channel in 0..<4 { error += abs(Int(reference[i + channel]) - Int(actual[i + channel])) }
            } }
            XCTAssertGreaterThan(ink, 10_000, "the reference has stroked italic ink beyond its advances")
            XCTAssertLessThan(error, 500, "the run clips preserve the line's overhanging ink")
        }
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

    /// A tall paragraph rasters in bands clipped 32 points past its box; a
    /// run's own shadow cast far sideways widens the band, under the cap,
    /// and the band's height pays for the width (LLP 1077 D3).
    func testABandAdmitsARunShadowCastSidewaysPastItsClip() throws {
        var far = run("Cd", weight: 800)
        far.shadow = [400, 0, 0, 255, 0, 0, 255]
        var spec = Spec(runs: [run("Ab ", weight: 800), far], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        spec.gatherShadows()
        let plain = Spec(runs: [run("Ab ", weight: 800), run("Cd", weight: 800)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let port = CGRect(x: 0, y: 0, width: 100, height: 800)
        let budget: CGFloat = 16 * 1024 * 1024
        let before = TextRasterJob.band(plain, width: 100, port: port, scale: 3, maximumBytes: budget)
        XCTAssertEqual(before.minX, -32, "no run shadow: the band as it was")
        XCTAssertEqual(before.width, 164)
        let band = TextRasterJob.band(spec, width: 100, port: port, scale: 3, maximumBytes: budget)
        XCTAssertEqual(band.minX, -32, accuracy: 0.01, "a shadow cast right reaches no further left")
        XCTAssertGreaterThan(band.maxX, 100 + 400)
        XCTAssertLessThan(band.height, before.height, "a wider band is shorter for the same bytes")

        let p = engine.paragraph(spec, width: 100)
        let box = CGRect(x: 0, y: 0, width: 100, height: 5000)
        let job = TextRasterJob(source: engine.attributed(spec), ranges: p.lines.map { CTLineGetStringRange($0) },
                                baselines: p.baselines, flush: 0, box: box, size: box.size, scale: 3, clip: band, crop: true)
        let cd = CGFloat(CTLineGetOffsetForStringIndex(engine.paragraph(plain, width: 100).lines[0], 3, nil))
        XCTAssertGreaterThan(try XCTUnwrap(job.render()).frame.maxX, cd + 400, "the shadow is in the band's pixels")

        far.shadow = [5000, 0, 40, 255, 0, 0, 255]
        spec.runs[1] = far
        let capped = TextRasterJob.band(spec, width: 100, port: port, scale: 3, maximumBytes: budget)
        XCTAssertEqual(capped.maxX, 100 + 32 + TextRasterJob.maxShadowReach, accuracy: 0.01, "under the cap")
    }

    /// A band a worker paints for a port that travels has three quarters of
    /// its room ahead of the port, so it is ready before the port leaves the
    /// band shown; one painted now is centred on the port.
    func testABandLeadsThePortItTravelsWith() {
        let spec = Spec(runs: [run("Ab", weight: 400)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let port = CGRect(x: 0, y: 4000, width: 100, height: 800)
        let budget: CGFloat = 48 * 1024 * 1024
        let down = TextRasterJob.band(spec, width: 100, port: port, scale: 3, maximumBytes: budget, down: true)
        let up = TextRasterJob.band(spec, width: 100, port: port, scale: 3, maximumBytes: budget, down: false)
        let centred = TextRasterJob.band(spec, width: 100, port: port, scale: 3, maximumBytes: budget)
        XCTAssertEqual(down.height, 1600, "two ports, under the budget")
        XCTAssertEqual(down.maxY - port.maxY, 600, "travelling down, the room is ahead")
        XCTAssertEqual(port.minY - down.minY, 200)
        XCTAssertEqual(port.minY - up.minY, 600, "travelling up, ahead is above")
        XCTAssertEqual(port.minY - centred.minY, 400)
    }

    /// A band is kept while the port rests in it or moves on its way, until
    /// the port is within half a port of its edge ahead. The way is the
    /// port's own: a band leading down lies mostly below the port, which
    /// does not turn it up, and nor does a move back of less than a quarter
    /// port. A port that turns back gets a band ahead of it.
    func testABandIsKeptUntilThePortNearsItsEdgeAhead() {
        let spec = Spec(runs: [run("Ab", weight: 400)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let budget: CGFloat = 48 * 1024 * 1024, height: CGFloat = 100_000
        var travel = PortTravel(), port = CGRect(x: 0, y: 4000, width: 100, height: 800)
        let keeps = { (band: CGRect) in TextRasterJob.keeps(band, port: port, down: travel.follow(port), height: height) }
        let down = TextRasterJob.band(spec, width: 100, port: port, scale: 3, maximumBytes: budget, down: travel.follow(port))
        for _ in 0..<3 { XCTAssertTrue(keeps(down), "the port at rest") }
        port.origin.y += 10
        XCTAssertTrue(keeps(down), "a small move on")
        port.origin.y += 200
        XCTAssertFalse(keeps(down), "within half a port of the edge ahead")
        port.origin.y -= 10
        XCTAssertTrue(keeps(down), "a finger's jitter back keeps its way")
        XCTAssertTrue(travel.down)
        port.origin.y = 4000
        XCTAssertFalse(keeps(down), "turned back, with a quarter port above")
        let up = TextRasterJob.band(spec, width: 100, port: port, scale: 3, maximumBytes: budget, down: travel.down)
        XCTAssertFalse(travel.down)
        XCTAssertTrue(keeps(up), "the new band, at rest")
    }

    /// A `line-clamp` paragraph rasters from its published geometry (LLP
    /// 1072 §8.1): a worker shapes the lines, the last one again from the
    /// range it broke at, ending in "…". The same pixels as layout's lines.
    func testAClampedParagraphRastersFromItsGeometryAsLayoutPaintsIt() throws {
        let text = "Maybe family sounds draft later scroll deadline picnic thanks soon a meeting at the station"
        for (clamp, align) in [1, 2].flatMap({ clamp in [0, 1, 2].map { (clamp, $0) } }) {
            let spec = Spec(runs: [run(text)], align: align, lineClamp: clamp, color: [30, 60, 90, 255])
            let p = engine.paragraph(spec, width: 180)
            XCTAssertEqual(p.lines.count, clamp)
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
            let laidOut = try XCTUnwrap(job(nil).render(lines: p.lines))
            XCTAssertEqual(shaped.frame, laidOut.frame)
            XCTAssertEqual(rasterBytes(shaped), rasterBytes(laidOut), "align \(align)")
            // Mounted paragraphs reuse the engine's plain range lines, not
            // paragraph.lines (which already ends with the clamp's ellipsis).
            let reused = engine.rasterLines(spec, ranges: geometry.ranges, width: box.width).1
            let firstPixels = try XCTUnwrap(job(geometry.clamped).render(lines: reused))
            XCTAssertEqual(firstPixels.frame, laidOut.frame)
            XCTAssertEqual(rasterBytes(firstPixels), rasterBytes(laidOut), "first pixels: clamp \(clamp), align \(align)")
            XCTAssertNotEqual(rasterBytes(try XCTUnwrap(job(nil).render())), rasterBytes(laidOut), "the clamp paints its ellipsis")
        }
    }

    func testAClampTokensPaintChoosesTheRasterFormatAndCasters() throws {
        let hdr: BatchValue = ["cs": [["s": "srgb-linear", "v": [4, 4, 4, 1]]], "c": [255, 255, 255, 255]]
        let bright = try XCTUnwrap(hdr.textChannels(dark: false))
        let black: [Double] = [0, 0, 0, 255]
        let wide: [Double] = [255, 0, 0, 255, 1, 1, 0, 0, 1]
        func raster(_ blockColor: [Double], shadow: [Double]? = nil) throws -> TextRasterImage {
            var red = run("Red words fill the line and carry on to another line")
            red.color = [255, 0, 0, 255]
            var spec = Spec(runs: [red], align: 0, lineClamp: 1, color: blockColor)
            spec.strut = run("")
            spec.strut?.color = blockColor
            spec.strut?.shadow = shadow
            spec.gatherShadows()
            let paragraph = engine.paragraph(spec, width: 100)
            let geometry = try XCTUnwrap(engine.measuredBreaks(spec, width: 100))
            let box = CGRect(x: 0, y: 0, width: 100, height: 40)
            var job = TextRasterJob(source: engine.attributed(spec), ranges: geometry.ranges, baselines: paragraph.baselines,
                                    flush: 0, box: box, size: box.size, scale: 1, clamped: geometry.clamped)
            #if os(iOS) || os(tvOS)
            job.tints = true
            #endif
            return try XCTUnwrap(job.render())
        }
        let ordinary = try raster(black)
        XCTAssertNil(ordinary.tint, "red text and a black token cannot share one tint")
        if ordinary.tint == nil {
            let pixels = rasterPixels(ordinary)
            XCTAssertGreaterThan(pixels.filter { $0.w > 0.5 && max($0.x, $0.y, $0.z) < 0.05 }.count, 5,
                                 "the ellipsis paints black while all source glyphs paint red")
        }
        let p3 = try raster(wide)
        #if os(macOS)
        let space = IOSurfaceCopyValue(p3.surface, kIOSurfaceColorSpace).flatMap { CGColorSpace(propertyListPlist: $0) }
        #else
        let space = p3.image.colorSpace
        #endif
        XCTAssertEqual(space?.name, CGColorSpace.displayP3, "the token alone requires wide storage")
        let light = try raster(bright)
        XCTAssertEqual(light.headroom, 4, accuracy: 0.01)
        XCTAssertGreaterThan(rasterPixels(light).map(\.x).max() ?? 0, 1, "the token's raster retains HDR light")
        let shadowed = try raster(black, shadow: [12, 0, 0] + bright)
        let cast = try XCTUnwrap(shadowed.cast, "the token alone casts this HDR shadow")
        XCTAssertEqual(shadowed.castHeadroom, 4, accuracy: 0.01)
        let castImage = TextRasterImage(cast, frame: shadowed.frame, covered: shadowed.covered, headroom: shadowed.castHeadroom)
        XCTAssertGreaterThan(rasterPixels(castImage).map(\.x).max() ?? 0, 1, "the shadow has HDR pixels")
    }

    private func rasterPixels(_ image: TextRasterImage) -> [SIMD4<Float>] {
        #if os(macOS)
        let width = image.surface.width, height = image.surface.height, row = image.surface.bytesPerRow
        let deep = image.surface.bytesPerElement == 8
        #else
        let width = image.image.width, height = image.image.height, row = image.image.bytesPerRow
        let deep = image.image.bitsPerComponent == 16
        #endif
        let data = rasterBytes(image)
        return data.withUnsafeBytes { raw in
            (0..<height).flatMap { y in (0..<width).map { x in
                if let tint = image.tint?.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .relativeColorimetric, options: nil)?.components {
                    let alpha = Float(raw.load(fromByteOffset: y * row + x, as: UInt8.self)) / 255
                    return SIMD4(Float(tint[0]) * alpha, Float(tint[1]) * alpha, Float(tint[2]) * alpha, alpha)
                }
                let channels: [Float] = (0..<4).map { channel in
                    if deep {
                        return Float(Float16(bitPattern: raw.loadUnaligned(fromByteOffset: y * row + x * 8 + channel * 2, as: UInt16.self)))
                    }
                    return Float(raw.load(fromByteOffset: y * row + x * 4 + (channel == 3 ? 3 : 2 - channel), as: UInt8.self)) / 255
                }
                return SIMD4(channels[0], channels[1], channels[2], channels[3])
            } }
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

    /// LLP 1100 D2: a paragraph whose colour is wide rasterizes into Display
    /// P3, so its ink is not clipped to sRGB; any other stays sRGB.
    func testAWideColouredParagraphRastersInDisplayP3() throws {
        func space(_ color: [Double]) throws -> String? {
            let spec = Spec(runs: [run("Wide")], align: 0, lineClamp: 0, color: color)
            let p = engine.paragraph(spec, width: 200)
            let box = CGRect(x: 0, y: 0, width: 200, height: 40)
            let job = TextRasterJob(source: engine.attributed(spec), ranges: p.lines.map { CTLineGetStringRange($0) },
                                    baselines: p.baselines, flush: 0, box: box, size: box.size, scale: 2)
            let image = try XCTUnwrap(job.render())
            #if os(macOS)
            let profile = IOSurfaceCopyValue(image.surface, kIOSurfaceColorSpace)
            return profile.flatMap { CGColorSpace(propertyListPlist: $0) }?.name as String?
            #else
            return image.image.colorSpace?.name as String?
            #endif
        }
        XCTAssertEqual(try space([255, 0, 0, 255, 1, 1, 0, 0, 1]), CGColorSpace.displayP3 as String)
        XCTAssertEqual(try space([255, 0, 0, 255]), CGColorSpace.sRGB as String)
    }
}

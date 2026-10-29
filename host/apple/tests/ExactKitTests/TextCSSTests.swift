// LLP 1053 §0 G4, G5: CSS white space, `nowrap`, `pre-line`, `text-overflow: ellipsis`
// without `line-clamp`, and `tabular-nums`, as the browser renders them.
import XCTest
import CoreText
import CExact
@testable import ExactKit

final class TextCSSTests: XCTestCase {
    let engine = TextEngine(resolve: { _ in nil })

    func testRuntimeInstalledFamiliesSelectTheirOwnFacesAndWidths() throws {
        guard !TextEngine.installedFaces("Arial").isEmpty,
              !TextEngine.installedFaces("Menlo").isEmpty else { throw missing("fonts", "Arial or Menlo unavailable") }
        let engine = TextEngine(resolve: { _ in nil })
        let arial = Array("Arial".utf8), menlo = Array("Menlo".utf8)
        arial.withUnsafeBufferPointer { a in
            menlo.withUnsafeBufferPointer { m in
                var rows = [ExactFontFace(), ExactFontFace()]
                rows[0].family = a.baseAddress; rows[0].family_len = a.count; rows[0].stack = 8
                rows[1].family = m.baseAddress; rows[1].family_len = m.count; rows[1].stack = 9
                rows.withUnsafeMutableBufferPointer { buffer in
                    var catalog = ExactFontCatalog()
                    catalog.faces = UnsafePointer(buffer.baseAddress); catalog.count = buffer.count
                    withUnsafePointer(to: &catalog) { engine.install($0) }
                }
            }
        }
        let a = engine.font(size: 20, weight: 400, family: 8, italic: false) as CTFont
        let m = engine.font(size: 20, weight: 400, family: 9, italic: false) as CTFont
        XCTAssertEqual(CTFontCopyFamilyName(a) as String, "Arial")
        XCTAssertEqual(CTFontCopyFamilyName(m) as String, "Menlo")
        let text = "The quick brown fox 0123456789"
        let width = { (font: CTFont) in CTLineGetTypographicBounds(
            CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: font])), nil, nil, nil) }
        XCTAssertNotEqual(width(a), width(m))
    }

    func testHTMLMaxlengthCountsUTF16AndAllowsDeletingAnAuthoredLongValue() {
        let props = ["maxlength": "3"]
        XCTAssertEqual(TextInputLimit.prefix("a😀b", props: props), "a😀")
        XCTAssertFalse(TextInputLimit.allows("abc", range: NSRange(location: 3, length: 0), replacement: "d", props: props))
        XCTAssertTrue(TextInputLimit.allows("long authored", range: NSRange(location: 4, length: 1), replacement: "", props: props))
        XCTAssertEqual(TextInputLimit.prefix("12345", props: ["type": "number", "maxlength": "3"]), "12345")
    }

    private func run(_ text: String, numeric: Int = 0) -> Run {
        Run(text: text, size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0, numeric: numeric)
    }
    private func spec(_ runs: [Run], whiteSpace: Int = 0) -> Spec {
        Spec(runs: runs, align: 0, lineClamp: 0, color: [0, 0, 0, 255], whiteSpace: whiteSpace)
    }

    /// At rest (`ExactSession.rest`) cold shaped text goes; what a view
    /// holds and the cold measurements stay.
    func testRestDropsColdShapedTextButKeepsMeasurements() {
        let engine = TextEngine(resolve: { _ in nil })
        let held = engine.paragraph(spec([run("held by a view")]), width: 180)
        engine.accepted(held)
        let input = spec([run("measured, then passed")])
        let width = engine.minContentWidth(input)
        _ = engine.paragraph(input, width: 180)
        for i in 0..<16 { _ = engine.paragraph(spec([run("cold \(i)")]), width: 180) }
        XCTAssertGreaterThan(engine.residencyStats.coldCoreTextEstimateBytes, 0)
        engine.dropColdShaped()
        XCTAssertEqual(engine.residencyStats.coldEntries, 1, "the measurement stays")
        XCTAssertEqual(engine.residencyStats.coldCoreTextEstimateBytes, 0)
        XCTAssertEqual(engine.minContentWidth(input), width)
        XCTAssertEqual(engine.residencyStats.coldEntries, 1, "answered by the kept measurement")
        XCTAssertTrue(engine.paragraph(spec([run("held by a view")]), width: 180) === held)
    }

    func testCollapsingIsTheMeasurersAndMapsBackToTheSource() {
        // Chrome's innerText for each source under `white-space: normal`.
        for (source, rendered) in [("a\r\nb", "a b"), ("  lead", "lead"), ("trail  ", "trail"), ("\t a\t\tb ", "a b"),
                                   ("a\u{200b}\nb", "a\u{200b}b"), ("a\u{a0}\u{a0}b", "a\u{a0}\u{a0}b"), ("中\n文", "中 文")] {
            var runs = [run(source)]
            _ = SourceMap.collapse(&runs, whiteSpace: 0)
            XCTAssertEqual(runs[0].text, rendered, source)
        }
        var runs = [run("a "), run(" b"), run("  c ")]
        let map = SourceMap.collapse(&runs, whiteSpace: 0)
        XCTAssertEqual(runs.map(\.text), ["a ", "b", " c"], "the first space stays in its own run")
        // "a  b  c " → "a b c": shaped offsets back to source ones and round trip.
        XCTAssertEqual(map.source(2), 3)   // b
        XCTAssertEqual(map.source(4), 6)   // c
        XCTAssertEqual(map.source(5), 8)   // the end, past the trailing space
        for shaped in 0...5 { XCTAssertEqual(map.collapsed(map.source(shaped)), shaped) }
        var clean = [run("already clean")]
        XCTAssertTrue(SourceMap.collapse(&clean, whiteSpace: 0).edits.isEmpty)
    }

    func testNowrapTakesNoSoftBreakAndMinContentIsMaxContent() {
        let text = "a nowrap label that is much wider than its box"
        let nowrap = spec([run(text)], whiteSpace: 2)
        let natural = engine.paragraph(nowrap, width: .infinity)
        let narrow = engine.paragraph(nowrap, width: 60)
        XCTAssertEqual(narrow.lines.count, 1)
        XCTAssertEqual(narrow.height, natural.height)
        XCTAssertGreaterThan(narrow.width, 60)
        XCTAssertEqual(engine.minContentWidth(nowrap), natural.width)
        let normal = spec([run(text)])
        XCTAssertGreaterThan(engine.paragraph(normal, width: 60).lines.count, 1)
        XCTAssertLessThan(engine.minContentWidth(normal), natural.width)
    }

    /// CSS Text 3 §4.1.3 removes collapsible spaces at a soft line break and
    /// hangs `pre-wrap`'s there; Core Text keeps their advance in the line's
    /// typographic width. Before a forced break `pre-wrap`'s spaces hang only
    /// as far as they would not fit, and `pre`'s count.
    func testAWrappedLinesTrailingSpaceTakesNoWidth() {
        let first = width("word word", numeric: 0)
        // Room for "word word" and less than the space after it
        let room = first + 1
        for whiteSpace in [0, 1, 3] {
            let p = engine.paragraph(spec([run("word word word")], whiteSpace: whiteSpace), width: room)
            XCTAssertEqual(p.lines.count, 2, "white-space \(whiteSpace)")
            XCTAssertEqual(p.width, first, accuracy: 1.0 / 64, "white-space \(whiteSpace)")
        }
        let word = width("word", numeric: 0)
        let spaced = engine.paragraph(spec([run("word   ")], whiteSpace: 4), width: .infinity).width
        XCTAssertEqual(engine.paragraph(spec([run("word   \nx")], whiteSpace: 1), width: spaced + 10).width, spaced, accuracy: 1.0 / 64,
                       "pre-wrap with room for them: the spaces count")
        XCTAssertEqual(engine.paragraph(spec([run("word   \nx")], whiteSpace: 1), width: word + 1).width, word + 1, accuracy: 1.0 / 64,
                       "pre-wrap without: the spaces past the room hang")
        XCTAssertEqual(engine.paragraph(spec([run("word   \nx")], whiteSpace: 4), width: word + 1).width, spaced, accuracy: 1.0 / 64,
                       "pre: they count")
        // The room is the line's, after its indent
        let indented = Spec(runs: [run("word   \nx")], align: 0, lineClamp: 0, color: [0, 0, 0, 255], whiteSpace: 1, textIndent: 20)
        XCTAssertEqual(engine.paragraph(indented, width: word + 21).width, word + 21, accuracy: 1.0 / 64,
                       "pre-wrap after an indent: the spaces past the line's room hang")
    }

    func testPreLineKeepsLineFeedsWrapsAndSizesByForcedLines() {
        // Chrome's innerText for each source under `white-space: pre-line`.
        for (source, rendered) in [("a    b", "a b"), ("a  \n  b", "a\nb"), ("a\n\nb", "a\n\nb"), ("\nfirst", "\nfirst"),
                                   ("  lead\n  mid  \ntrail  ", "lead\nmid\ntrail"), ("a\r\nb", "a\nb"), ("a\tb\t\nc", "a b\nc")] {
            var runs = [run(source)]
            _ = SourceMap.collapse(&runs, whiteSpace: 3)
            XCTAssertEqual(runs[0].text, rendered, source)
        }
        var runs = [run("one two three  \n  four")]
        _ = SourceMap.collapse(&runs, whiteSpace: 3)
        let s = spec(runs, whiteSpace: 3)
        let natural = engine.paragraph(s, width: .infinity)
        XCTAssertEqual(natural.lines.count, 2, "the line feed is a forced break")
        XCTAssertEqual(natural.width, width("one two three", numeric: 0), accuracy: 0.01)
        XCTAssertEqual(engine.minContentWidth(s), width("three", numeric: 0), accuracy: 0.01)
        XCTAssertEqual(engine.paragraph(s, width: width("one two", numeric: 0) + 1).lines.count, 3, "and it wraps")
    }

    /// `white-space: pre` (LLP 1053 G5): preserve × nowrap. Chrome's widths
    /// (13px system-ui, headless Chrome 154, `getBoundingClientRect` of an
    /// inline-block, scripts/fixtures/pre.contract's cases): kept spaces,
    /// trailing ones included, and lines only where a line feed is.
    func testPreKeepsEverySpaceAndBreaksOnlyAtLineFeeds() {
        func pre(_ text: String) -> Spec {
            var runs = [Run(text: text, size: 13, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)]
            XCTAssertTrue(SourceMap.collapse(&runs, whiteSpace: 4).edits.isEmpty, "pre collapses nothing")
            XCTAssertEqual(runs[0].text, text)
            return Spec(runs: runs, align: 0, lineClamp: 0, color: [0, 0, 0, 255], whiteSpace: 4)
        }
        for (text, width, lines) in [("  lead   inner  ", 81.015625, 1), ("one\n\nthree  ", 38.765625, 3),
                                     ("let a_very_long_line = compute(alpha, beta, gamma, delta);", 360.015625, 1)] {
            let s = pre(text)
            let natural = engine.paragraph(s, width: .infinity)
            XCTAssertEqual(natural.width, width, accuracy: 1.0 / 64 + 1e-9, text)
            XCTAssertEqual(natural.lines.count, lines, text)
            // A narrow box wraps nothing: the line overflows it.
            let narrow = engine.paragraph(s, width: 40)
            XCTAssertEqual(narrow.lines.count, lines, text)
            XCTAssertEqual(narrow.width, natural.width, text)
            XCTAssertEqual(engine.minContentWidth(s), natural.width, "min-content is max-content: \(text)")
        }
    }

    /// Tab stops every eight spaces of the face (CSS `tab-size: 8`), from the
    /// line's start. Chrome's widths, 13px DejaVu Sans (the pinned fixture
    /// face, as a web font): system-ui's are declared apart (LLP 1053 §0 r3).
    func testTabsStopEveryEightSpacesAsChromeDoes() throws {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").standardized
        let data = try Data(contentsOf: url)
        let d = (CTFontManagerCreateFontDescriptorsFromData(data as CFData) as! [CTFontDescriptor])[0]
        let font = CTFontCreateWithFontDescriptor(d, 13, nil)
        let paragraph = NSMutableParagraphStyle()
        paragraph.tabStops = []
        paragraph.defaultTabInterval = TextEngine.tabInterval(font, letterSpacing: 0)
        for (text, chrome) in [("abcdefghij\tk", 106.71875), ("a\tb", 41.328125), ("\tc", 40.21875), ("\t\tx", 73.828125)] {
            let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: font, .paragraphStyle: paragraph]))
            XCTAssertEqual(CSSLineBox.layoutWidth(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil))), chrome, accuracy: 1.0 / 64 + 1e-9, text)
        }
        // Through the engine: a preserved tab in the system monospace face
        // lands on the stop after "abcdefghij" (ten advances, past two stops).
        let runs = [Run(text: "abcdefghij\tk", size: 13, weight: 400, family: 5, italic: false, lineHeight: nil, letterSpacing: 0)]
        let mono = engine.font(runs[0]) as CTFont
        let interval = TextEngine.tabInterval(mono, letterSpacing: 0)
        let k = CTLineGetTypographicBounds(CTLineCreateWithAttributedString(NSAttributedString(string: "k", attributes: [.font: mono])), nil, nil, nil)
        for whiteSpace in [1, 4] {
            let p = engine.paragraph(Spec(runs: runs, align: 0, lineClamp: 0, color: [0, 0, 0, 255], whiteSpace: whiteSpace), width: .infinity)
            XCTAssertEqual(p.width, CSSLineBox.layoutWidth(2 * interval + CGFloat(k)), accuracy: 1.0 / 64 + 1e-9, "white-space \(whiteSpace)")
        }
    }

    /// A declared variable face draws at its declared weight on the `wght`
    /// axis, as a browser draws an `@font-face` (the Bluesky clone's Inter,
    /// whose first named instance is Thin); a static face is left as it is.
    func testADeclaredVariableFaceTakesItsWeightOnTheAxis() throws {
        let fonts = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../")
        let inter = try Data(contentsOf: fonts.appendingPathComponent("host/apple/tests/fonts/InterVariable.ttf").standardized)
        let first = (CTFontManagerCreateFontDescriptorsFromData(inter as CFData) as! [CTFontDescriptor])[0]
        func wght(_ d: CTFontDescriptor) -> Double? {
            (CTFontCopyVariation(CTFontCreateWithFontDescriptor(d, 16, nil)) as? [NSNumber: NSNumber])?[0x77676874 as NSNumber]?.doubleValue
        }
        XCTAssertEqual(wght(first), 100, "the file's first named instance is Thin")
        // CoreText leaves an axis at its default (Inter's 400) out of the variation.
        XCTAssertEqual(wght(TextEngine.declaredFace(first, weight: 400)) ?? 400, 400)
        XCTAssertEqual(wght(TextEngine.declaredFace(first, weight: 600)), 600)
        XCTAssertEqual(wght(TextEngine.declaredFace(first, weight: 1000)), 900, "clamped to the axis")
        let dejavu = try Data(contentsOf: fonts.appendingPathComponent("scripts/fixtures/fonts/assets/DejaVuSans.ttf").standardized)
        let plain = (CTFontManagerCreateFontDescriptorsFromData(dejavu as CFData) as! [CTFontDescriptor])[0]
        XCTAssertTrue(TextEngine.declaredFace(plain, weight: 700) === plain)
        // Through registration: one file declared at 400 and 600, as an app's
        // `font "Inter"` block does, draws each weight on the axis.
        let engine = TextEngine(resolve: { _ in nil }, read: { _ in inter })
        let family = Array("Inter".utf8), source = Array("inter.ttf".utf8)
        family.withUnsafeBufferPointer { f in
            source.withUnsafeBufferPointer { src in
                var faces = [400, 600].map { w -> ExactFontFace in
                    var face = ExactFontFace()
                    face.family = f.baseAddress; face.family_len = f.count
                    face.source = src.baseAddress; face.source_len = src.count
                    face.stack = 8; face.weight = UInt16(w); face.italic = 0
                    return face
                }
                faces.withUnsafeMutableBufferPointer { rows in
                    var catalog = ExactFontCatalog()
                    catalog.faces = UnsafePointer(rows.baseAddress); catalog.count = rows.count
                    withUnsafePointer(to: &catalog) { engine.install($0) }
                }
            }
        }
        func drawn(_ weight: Int) -> Double? {
            (CTFontCopyVariation(engine.font(size: 16, weight: weight, family: 8, italic: false) as CTFont) as? [NSNumber: NSNumber])?[0x77676874 as NSNumber]?.doubleValue
        }
        XCTAssertEqual(drawn(400) ?? 400, 400)
        XCTAssertEqual(drawn(600), 600)
        XCTAssertEqual(drawn(700), 600, "700 matches the 600 face, drawn at its declared weight")
    }

    /// LLP 1019 D9: `font-variation-settings` draws after the weight, the
    /// width and the size, each axis it names winning; `font-stretch` on a
    /// platform face is its `wdth`, as Chrome and WebKit set San Francisco's;
    /// `font-optical-sizing: none` leaves `opsz` to the face.
    func testFontVariationSettingsWinAndAPlatformFaceStretches() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../")
        let flex = try Data(contentsOf: root.appendingPathComponent("host/linux/tests/fonts/RobotoFlex-subset.ttf").standardized)
        let engine = TextEngine(resolve: { _ in nil }, read: { _ in flex })
        let family = Array("Flex".utf8), source = Array("flex.ttf".utf8)
        family.withUnsafeBufferPointer { f in
            source.withUnsafeBufferPointer { src in
                var face = ExactFontFace()
                face.family = f.baseAddress; face.family_len = f.count
                face.source = src.baseAddress; face.source_len = src.count
                face.stack = 8; face.weight = 100; face.weight_max = 1000
                face.stretch_min = 250; face.stretch_max = 1510; face.italic = 0
                var catalog = ExactFontCatalog()
                withUnsafePointer(to: &face) { row in
                    catalog.faces = row; catalog.count = 1
                    withUnsafePointer(to: &catalog) { engine.install($0) }
                }
            }
        }
        func tag(_ s: String) -> UInt32 { s.utf8.reduce(0) { $0 << 8 | UInt32($1) } }
        func width(_ family: Int, _ weight: Int, _ stretch: CGFloat, _ axes: [FontAxis] = []) -> CGFloat {
            let font = engine.font(size: 48, weight: weight, family: family, italic: false, stretch: stretch, variations: axes)
            let line = CTLineCreateWithAttributedString(NSAttributedString(string: "EXACT", attributes: [.font: font]))
            return CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil))
        }
        XCTAssertEqual(width(8, 700, 100, [FontAxis(tag: tag("wdth"), value: 50)]), width(8, 700, 50))
        XCTAssertEqual(width(8, 700, 100, [FontAxis(tag: tag("wght"), value: 300)]), width(8, 300, 100))
        XCTAssertEqual(width(8, 700, 100, [FontAxis(tag: tag("GRAD"), value: 150)]), width(8, 700, 100), "a grade keeps the advances")
        // system-ui (stack 0): San Francisco's `wdth` is the stretch.
        XCTAssertLessThan(width(0, 400, 75), width(0, 400, 100))
        XCTAssertLessThan(width(0, 400, 100), width(0, 400, 125))
        XCTAssertEqual(width(0, 400, 75), width(0, 400, 100, [FontAxis(tag: tag("wdth"), value: 75)]))
        // `font-optical-sizing: none` keeps the face's own `opsz` (Roboto
        // Flex's default is 14) rather than the size, and San Francisco's
        // text cut rather than the display cut CoreText picks at 48.
        let own = [FontAxis.ownOpticalSize]
        XCTAssertNotEqual(width(8, 700, 100, own), width(8, 700, 100))
        XCTAssertEqual(width(8, 700, 100, own), width(8, 700, 100, [FontAxis(tag: tag("opsz"), value: 14)]))
        XCTAssertEqual(width(8, 700, 100), width(8, 700, 100, [FontAxis(tag: tag("opsz"), value: 48)]))
        XCTAssertGreaterThan(width(0, 400, 100, own), width(0, 400, 100))
    }

    /// LLP 1019 D2, D8: a face declared over a weight range and a width range
    /// draws each request on `wght` and `wdth`, clamped to the ranges, as a
    /// browser draws an `@font-face` with range descriptors; matching treats
    /// a range as its value nearest the request (CSS Fonts 4 §5.2).
    func testADeclaredRangeDrawsEachWeightAndWidthOnTheAxes() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../")
        let flex = try Data(contentsOf: root.appendingPathComponent("host/linux/tests/fonts/RobotoFlex-subset.ttf").standardized)
        let engine = TextEngine(resolve: { _ in nil }, read: { _ in flex })
        let family = Array("Flex".utf8), source = Array("flex.ttf".utf8)
        family.withUnsafeBufferPointer { f in
            source.withUnsafeBufferPointer { src in
                var face = ExactFontFace()
                face.family = f.baseAddress; face.family_len = f.count
                face.source = src.baseAddress; face.source_len = src.count
                face.stack = 8; face.weight = 100; face.weight_max = 1000
                face.stretch_min = 250; face.stretch_max = 1510; face.italic = 0
                var catalog = ExactFontCatalog()
                withUnsafePointer(to: &face) { row in
                    catalog.faces = row; catalog.count = 1
                    withUnsafePointer(to: &catalog) { engine.install($0) }
                }
            }
        }
        // CoreText reports no variation for this file's fonts, so each is
        // measured against the file drawn at explicit axes.
        let file = (CTFontManagerCreateFontDescriptorsFromData(flex as CFData) as! [CTFontDescriptor])[0]
        func line(_ font: CTFont) -> CGFloat {
            let line = CTLineCreateWithAttributedString(NSAttributedString(string: "EXACT", attributes: [.font: font]))
            return CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil))
        }
        func width(_ weight: Int, _ stretch: CGFloat) -> CGFloat {
            line(engine.font(size: 48, weight: weight, family: 8, italic: false, stretch: stretch) as CTFont)
        }
        func reference(wght: Double, wdth: Double, opsz: Double = 48) -> CGFloat {
            let axes: [NSNumber: NSNumber] = [0x77676874: wght as NSNumber, 0x77647468: wdth as NSNumber, 0x6f70737a: opsz as NSNumber]
            let d = CTFontDescriptorCreateCopyWithAttributes(file, [kCTFontVariationAttribute: axes] as CFDictionary)
            return line(CTFontCreateWithFontDescriptor(d, 48, nil))
        }
        XCTAssertEqual(width(650, 75), reference(wght: 650, wdth: 75))
        XCTAssertEqual(width(1000, 200), reference(wght: 1000, wdth: 151), "clamped to the declared range")
        XCTAssertNotEqual(reference(wght: 650, wdth: 75), reference(wght: 650, wdth: 75, opsz: 14),
                          "opsz is the size, as `font-optical-sizing: auto` sets it")
        XCTAssertLessThan(width(700, 50), width(700, 100))
        XCTAssertLessThan(width(700, 100), width(700, 125))
        XCTAssertLessThan(width(300, 100), width(900, 100))
        let one = engine.font(size: 48, weight: 650, family: 8, italic: false) as CTFont
        XCTAssertNil(CTFontCopyAttribute(one, kCTFontCascadeListAttribute), "a one-family stack falls back to nothing of its own")

        let regular = RegisteredFace(family: "Flex", generic: nil, weight: 400, italic: false, descriptor: file)
        let ranged = RegisteredFace(family: "Flex", generic: nil, weights: 500...900, stretches: 100...100, italic: false, descriptor: file)
        XCTAssertEqual(TextEngine.matched([regular, ranged], weight: 750, italic: false).weights, 500...900)
        XCTAssertEqual(TextEngine.matched([regular, ranged], weight: 420, italic: false).weights, 500...900,
                       "for 420, weights up to 500 come before lighter ones")
        XCTAssertEqual(TextEngine.matched([regular, ranged], weight: 380, italic: false).weights, 400...400)
        let condensed = RegisteredFace(family: "Flex", generic: nil, weights: 400...400, stretches: 75...75, italic: false, descriptor: file)
        XCTAssertEqual(TextEngine.matched([regular, condensed], weight: 400, italic: false, stretch: 80).stretches, 75...75,
                       "at or below normal width, narrower faces come first")
        XCTAssertEqual(TextEngine.matched([regular, condensed], weight: 400, italic: false, stretch: 110).stretches, 100...100)
    }

    func testEllipsisEndsAnOverWideLineOnlyWhereItPaints() {
        var s = spec([run("a nowrap label that is much wider than its box")], whiteSpace: 2)
        s.ellipsis = true
        let p = engine.paragraph(s, width: 100)
        let shown = p.ellipsized(0, spec: s, width: 100)
        XCTAssertLessThanOrEqual(CGFloat(CTLineGetTypographicBounds(shown, nil, nil, nil)), 100.5)
        XCTAssertGreaterThan(CGFloat(CTLineGetTypographicBounds(p.lines[0], nil, nil, nil)), 100, "the measured line is kept")
        XCTAssertEqual(s.geometry, spec(s.runs, whiteSpace: 2).geometry, "ellipsis is paint, never metrics")
        let fits = engine.paragraph(s, width: 1000)
        XCTAssertTrue(fits.ellipsized(0, spec: s, width: 1000) === fits.lines[0])
    }

    private func width(_ text: String, numeric: Int) -> CGFloat {
        engine.paragraph(spec([run(text, numeric: numeric)]), width: .infinity).width
    }

    func testTabularNumsIsTheFacesTnumFeature() {
        // The system face: proportional figures by default, `tnum` present.
        XCTAssertLessThan(width("111", numeric: 0), width("888", numeric: 0))
        XCTAssertEqual(width("111", numeric: 1), width("888", numeric: 1))
        XCTAssertNotEqual(spec([run("1", numeric: 1)]).geometry, spec([run("1")]).geometry, "a metric identity")
        // A face without `tnum` (DejaVu Sans, tabular already) is unchanged by it.
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").standardized
        let data = try! Data(contentsOf: url)
        let d = (CTFontManagerCreateFontDescriptorsFromData(data as CFData) as! [CTFontDescriptor])[0]
        let plain = CTFontCreateWithFontDescriptor(d, 16, nil)
        let settings = [[kCTFontOpenTypeFeatureTag: "tnum", kCTFontOpenTypeFeatureValue: 1]] as CFArray
        let tnum = CTFontCreateCopyWithAttributes(plain, 16, nil, CTFontDescriptorCreateWithAttributes([kCTFontFeatureSettingsAttribute: settings] as CFDictionary))
        func advance(_ f: CTFont, _ s: String) -> Double {
            CTLineGetTypographicBounds(CTLineCreateWithAttributedString(NSAttributedString(string: s, attributes: [.font: f])), nil, nil, nil)
        }
        XCTAssertEqual(advance(plain, "111"), advance(tnum, "111"))
        XCTAssertEqual(advance(plain, "111"), advance(plain, "888"))
    }

    #if os(macOS)
    func testLinksAddressTheSourceWhileLinesAddressTheShapedText() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "collapse-links")
        defer { session.destroy() }
        let p = session.presenter
        let node = NodeView(id: 1, kind: "text", presenter: p)
        p.views[1] = node
        node.frame = NSRect(x: 0, y: 0, width: 400, height: 60)
        p.applyParagraphFixture(1, [
            ["id": 2, "parent": 1, "paint": true, "props": ["text": "Plain  \n\t "]],
            ["id": 3, "parent": 1, "paint": true, "props": ["text": "link", "href": "https://example.com"]],
        ])
        // Source ranges: "Plain  \n\t " is 10 units; the link follows it.
        XCTAssertEqual(p.inlineText(3)?.range, NSRange(location: 10, length: 4))
        let spec = node.paragraphSpec()
        XCTAssertEqual(spec.runs.map(\.text), ["Plain ", "link"])
        XCTAssertEqual(spec.source.collapsed(10), 6)
        XCTAssertEqual(spec.source.source(6), 10)
        let paragraph = try XCTUnwrap(node.paragraphLayout())
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(3)))
        let start = CGFloat(CTLineGetOffsetForStringIndex(paragraph.lines[0], 6, nil))
        XCTAssertEqual(try XCTUnwrap(rects.first).minX, node.contentBox().minX + start, accuracy: 0.5)
        let hit = CGPoint(x: node.contentBox().minX + start + 4, y: node.contentBox().minY + 8)
        XCTAssertEqual(node.inlineLink(at: hit), "https://example.com")
    }
    #endif

    /// A clamp's last line wider than its box, one word longer than the
    /// line, ends in "…" as UILabel's tail truncation does (notes at the
    /// largest text size, round 22: "Packing" ran over the row's time);
    /// layout keeps the word's width, as CSS measures it.
    func testLineClampTruncatesAWordLongerThanItsLastLine() {
        let clamped = Spec(runs: [run("Packing")], align: 0, lineClamp: 1, color: [0, 0, 0, 255])
        let natural = engine.paragraph(clamped, width: .infinity).width
        let p = engine.paragraph(clamped, width: natural / 2)
        XCTAssertNotNil(p.clampedRange)
        XCTAssertLessThanOrEqual(CTLineGetTypographicBounds(p.lines[0], nil, nil, nil), Double(natural / 2) + 0.5)
        XCTAssertEqual(p.width, natural, accuracy: 1.0 / 64)
        // One that fits takes no ellipsis.
        XCTAssertNil(engine.paragraph(clamped, width: natural + 1).clampedRange)
    }
}

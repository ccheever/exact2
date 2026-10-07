// LLP 1053 §0 G4, G5: CSS white space, `nowrap`, `pre-line`, `text-overflow: ellipsis`
// without `line-clamp`, and `tabular-nums`, as the browser renders them.
import XCTest
import CoreText
import CExact
@testable import ExactKit

final class TextCSSTests: XCTestCase {
    let engine = TextEngine(resolve: { _ in nil })

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
}

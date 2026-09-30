import Foundation
import CoreText
import XCTest
import CExact
@testable import ExactKit

/// LLP 1072 §8.1: the kernel measures on the owner thread while main paints
/// the same paragraphs. The two engines of a session share nothing but the
/// published breaks, so this runs under the Thread Sanitizer clean, and what
/// each lays out agrees. Before the split, one engine's residency was shaped
/// from both threads at once and a scrolling list crashed within minutes.
/// Both platforms; the iOS simulator runs it as an `IOSTests` class.
final class OwnerTextIOSTests: XCTestCase {
    private func spec(_ i: Int, clamp: Int = 0) -> Spec {
        let run = Run(text: "Paragraph \(i): " + String(repeating: "several words wrap here ", count: 8 + i % 5),
                      size: 15 + CGFloat(i % 3), weight: i % 2 == 0 ? 400 : 600, family: 0, italic: false,
                      lineHeight: nil, letterSpacing: 0)
        return Spec(runs: [run], align: 0, lineClamp: clamp, color: [0, 0, 0, 255], strut: run)
    }

    /// The kernel's request for `spec` at `width` (negative: an intrinsic
    /// offer), its buffers alive for `body`.
    private func withRequest(_ spec: Spec, width: Float, _ body: (ExactMeasureRequest) -> Void) {
        func raw(_ r: Run, _ text: UnsafePointer<UInt8>?, _ len: Int) -> ExactTextRun {
            var out = ExactTextRun()
            out.text = text; out.len = len
            out.font_size = Float(r.size); out.font_weight = UInt16(r.weight); out.font_family = UInt16(r.family)
            out.italic = r.italic ? 1 : 0; out.has_line_height = r.lineHeight == nil ? 0 : 1
            out.line_height = Float(r.lineHeight ?? 0); out.letter_spacing = Float(r.letterSpacing)
            out.font_variant_numeric = UInt8(r.numeric)
            return out
        }
        let texts = spec.runs.map { Array($0.text.utf8) }
        let buffers = texts.map { bytes -> UnsafeMutablePointer<UInt8> in
            let b = UnsafeMutablePointer<UInt8>.allocate(capacity: max(1, bytes.count))
            b.initialize(from: bytes, count: bytes.count)
            return b
        }
        defer { buffers.forEach { $0.deallocate() } }
        let runs = spec.runs.indices.map { raw(spec.runs[$0], UnsafePointer(buffers[$0]), texts[$0].count) }
        runs.withUnsafeBufferPointer { pointer in
            var request = ExactMeasureRequest()
            request.runs = pointer.baseAddress; request.count = pointer.count
            request.strut = raw(spec.strut ?? spec.runs[0], nil, 0)
            request.width = width; request.height = -1; request.align = UInt8(spec.align)
            request.line_clamp = UInt32(spec.lineClamp); request.overflow_wrap = UInt8(spec.overflowWrap)
            request.direction = UInt8(spec.direction); request.white_space = UInt8(spec.whiteSpace)
            body(request)
        }
    }
    private func same(_ a: LineGeometry?, _ b: LineGeometry?) -> Bool {
        guard let a, let b else { return a == nil && b == nil }
        func plain(_ r: CFRange?) -> [Int] { r.map { [$0.location, $0.length] } ?? [] }
        return a.ranges.map { plain($0) } == b.ranges.map { plain($0) } && a.baselines == b.baselines
            && plain(a.clamped) == plain(b.clamped)
    }
    /// The painter's copy of a paragraph: its lengths through the batch's
    /// decimal text, as `Presenter` gets them.
    private func throughTheWire(_ spec: Spec) -> Spec {
        func wire(_ v: CGFloat) -> CGFloat { CGFloat(Double("\(Float(v))")!) }
        var out = spec
        for i in out.runs.indices {
            out.runs[i].size = wire(out.runs[i].size); out.runs[i].letterSpacing = wire(out.runs[i].letterSpacing)
            out.runs[i].lineHeight = out.runs[i].lineHeight.map(wire)
        }
        out.strut = out.runs[0]
        out.color = [10, 20, 30, 255]
        return out
    }

    /// What the measurer answered once it answers again without typesetting,
    /// after its residency let everything go; and the painter finds that
    /// paragraph's breaks though its lengths came through decimal text.
    func testAMeasuredParagraphIsAnsweredAndBrokenFromThePublishedAnswer() {
        let painter = TextEngine.pair(resolve: { _ in nil })
        let measurer = painter.measuring
        var s = spec(3, clamp: 2)
        s.runs[0].letterSpacing = CGFloat(Float(0.41)); s.runs[0].lineHeight = CGFloat(Float(18.2))
        s.strut = s.runs[0]
        let width = Float(301) / 3
        for offer in [width, -1, -2] {
            withRequest(s, width: offer) { r in
                let first = measurer.measure(r)
                measurer.dropCold()
                XCTAssertEqual(measurer.residencyStats.coldEntries, 0)
                let hits = measurer.measureHits
                let again = measurer.measure(r)
                XCTAssertEqual([again.width, again.height, again.baseline], [first.width, first.height, first.baseline])
                XCTAssertEqual(measurer.measureHits, hits + 1)
                XCTAssertEqual(measurer.residencyStats.coldEntries, 0, "answered without shaping it again")
            }
        }
        // A fresh engine typesets the same paragraph: its breaks are the published ones.
        let fresh = TextEngine(resolve: { _ in nil })
        let expected = LineGeometry(fresh.paragraph(s, width: CGFloat(width)))
        XCTAssertNotNil(expected.clamped, "the fixture clamps")
        let painted = throughTheWire(s)
        XCTAssertNotEqual(painted.runs[0].letterSpacing, s.runs[0].letterSpacing, "the decimal round trip moves the double")
        let published = painter.measuredBreaks(painted, width: CGFloat(Double("\(width)")!))
        XCTAssertNotNil(published)
        XCTAssertTrue(same(published, expected))
        XCTAssertEqual(painter.answers?.observation.paragraphs, 1)
        XCTAssertEqual(painter.answers?.observation.offers, 3)
    }

    /// The table is bounded, and what it let go is only typeset again: the
    /// measurer answers the same metrics and the painter paints the same
    /// lines as an engine that never had a table.
    func testAnEvictedAnswerChangesNothingMeasuredOrPainted() {
        let painter = TextEngine.pair(resolve: { _ in nil }, answerBytes: 16 * 1024)
        let measurer = painter.measuring
        let fresh = TextEngine(resolve: { _ in nil })
        let specs = (0..<160).map { spec($0, clamp: $0 % 5 == 0 ? 2 : 0) }
        let width: Float = 233.5
        var first: [[Float]] = []
        for s in specs { withRequest(s, width: width) { let m = measurer.measure($0); first.append([m.width, m.height, m.baseline]) } }
        let stats = painter.answers!.observation
        XCTAssertLessThan(stats.paragraphs, specs.count, "the byte limit evicted the oldest")
        XCTAssertLessThanOrEqual(stats.bytes, 16 * 1024)
        // What the raster job is made from (`TextRasterizer.ensure`): the
        // published breaks, else the painter's own typesetting.
        var evicted = 0
        for (i, s) in specs.enumerated() {
            let expected = LineGeometry(fresh.paragraph(s, width: CGFloat(width)))
            let published = painter.measuredBreaks(s, width: CGFloat(width))
            if published == nil { evicted += 1 }
            let used = published ?? LineGeometry(painter.paragraph(s, width: CGFloat(width)))
            XCTAssertTrue(same(used, expected), "paragraph \(i)")
        }
        XCTAssertGreaterThan(evicted, 0)
        XCTAssertLessThan(evicted, specs.count)
        // Measured again, evicted or not: the same answers, published again.
        for (i, s) in specs.enumerated() {
            withRequest(s, width: width) { r in
                let m = measurer.measure(r)
                XCTAssertEqual([m.width, m.height, m.baseline], first[i], "paragraph \(i)")
            }
            XCTAssertTrue(same(painter.measuredBreaks(s, width: CGFloat(width)), LineGeometry(fresh.paragraph(s, width: CGFloat(width)))), "paragraph \(i)")
        }
    }

    /// A font catalog change leaves no answer of the old catalog's: the
    /// table's metrics feed layout, so a paragraph in a declared family
    /// measured before its face was installed is measured by the face once
    /// it is, whichever engine resets first, and although the measurer can
    /// still publish an old answer between the painter's reset and its own.
    func testANewFontCatalogLeavesNoAnswerOfTheOldOne() throws {
        // A face to declare: a monospaced system face's own file, as bytes.
        let mono = CTFontCreateWithName("Courier New" as CFString, 16, nil)
        guard let url = CTFontCopyAttribute(mono, kCTFontURLAttribute) as? URL, let data = try? Data(contentsOf: url) else {
            throw XCTSkip("no readable file for the fixture face")
        }
        var s = spec(4)
        s.runs[0].family = 1; s.strut = s.runs[0]
        let width: Float = 240
        func withCatalog(_ body: (UnsafePointer<ExactFontCatalog>) -> Void) {
            let family = Array("Fixture".utf8), source = Array("fixture.ttf".utf8)
            family.withUnsafeBufferPointer { f in
                source.withUnsafeBufferPointer { src in
                    var face = ExactFontFace()
                    face.family = f.baseAddress; face.family_len = f.count
                    face.source = src.baseAddress; face.source_len = src.count
                    face.stack = 1; face.weight = 400; face.italic = 0
                    withUnsafePointer(to: &face) { row in
                        var catalog = ExactFontCatalog()
                        catalog.faces = row; catalog.count = 1
                        withUnsafePointer(to: &catalog, body)
                    }
                }
            }
        }
        func metrics(_ e: TextEngine, _ offer: Float) -> [Float] {
            var out: [Float] = []
            withRequest(s, width: offer) { let m = e.measure($0); out = [m.width, m.height, m.baseline] }
            return out
        }
        // What the declared face answers, from an engine that never measured without it.
        let fresh = TextEngine(resolve: { _ in nil }, read: { _ in data })
        withCatalog { fresh.install($0) }
        let expected = [width, -1, -2].map { metrics(fresh, $0) }
        let expectedLines = LineGeometry(fresh.paragraph(s, width: CGFloat(width)))
        for painterFirst in [true, false] {
            let painter = TextEngine.pair(resolve: { _ in nil }, read: { _ in data })
            let measurer = painter.measuring
            let before = [width, -1, -2].map { metrics(measurer, $0) }
            XCTAssertNotEqual(before, expected, "the fixture face measures apart from the fallback")
            XCTAssertEqual(painter.answers?.observation.offers, 3)
            withCatalog { catalog in
                if painterFirst {
                    painter.install(catalog)
                    XCTAssertEqual(painter.answers?.observation.offers, 0)
                    // The measurer, not yet reset, answers and publishes by the old catalog.
                    XCTAssertEqual([width, -1, -2].map { metrics(measurer, $0) }, before)
                    measurer.install(catalog)
                } else {
                    measurer.install(catalog)
                    XCTAssertEqual(metrics(measurer, width), expected[0])
                    painter.install(catalog)
                }
                XCTAssertEqual(painter.answers?.observation.offers, 0, "each engine's reset empties the table")
            }
            XCTAssertEqual([width, -1, -2].map { metrics(measurer, $0) }, expected, painterFirst ? "painter first" : "measurer first")
            XCTAssertTrue(same(painter.measuredBreaks(s, width: CGFloat(width)), expectedLines))
            // And answered again from the table, the same.
            let hits = measurer.measureHits
            XCTAssertEqual([width, -1, -2].map { metrics(measurer, $0) }, expected)
            XCTAssertEqual(measurer.measureHits, hits + 3)
        }
    }

    /// The owner measures through the kernel's callback while main takes
    /// the same paragraphs' breaks from the table, which is small enough to
    /// evict throughout: under the Thread Sanitizer the table is the only
    /// state the two threads share, and every answer main reads is a fresh
    /// typesetting's.
    func testTheOwnerPublishesAnswersWhileMainReadsThem() {
        let painter = TextEngine.pair(resolve: { _ in nil }, answerBytes: 24 * 1024)
        let measurer = painter.measuring
        let fresh = TextEngine(resolve: { _ in nil })
        let specs = (0..<96).map { spec($0, clamp: $0 % 7 == 0 ? 2 : 0) }
        let widths: [Float] = [180, 240.25, 320]
        let expected = specs.map { s in widths.map { LineGeometry(fresh.paragraph(s, width: CGFloat($0))) } }
        let done = expectation(description: "the owner measured")
        Owner.shared.post { [self] in
            for round in 0..<24 {
                for (i, s) in specs.enumerated() {
                    withRequest(s, width: widths[(i + round) % widths.count]) { _ = measurer.measure($0) }
                    if i % 9 == 0 { withRequest(s, width: -1) { _ = measurer.measure($0) } }
                }
                if round % 5 == 3 { measurer.dropCold() }
            }
            DispatchQueue.main.async { done.fulfill() }
        }
        var read = 0
        for round in 0..<24 {
            for (i, s) in specs.enumerated() {
                let w = (i + round) % widths.count
                if let lines = painter.measuredBreaks(s, width: CGFloat(widths[w])) {
                    read += 1
                    XCTAssertTrue(same(lines, expected[i][w]), "paragraph \(i) at \(widths[w])")
                }
            }
        }
        wait(for: [done], timeout: 60)
        Owner.shared.sync {}
        for (i, s) in specs.enumerated().suffix(8) {
            XCTAssertTrue(same(painter.measuredBreaks(s, width: CGFloat(widths[(i + 23) % widths.count])), expected[i][(i + 23) % widths.count]))
        }
        _ = read
    }

    func testTheOwnerMeasuresWhileMainPaintsTheSameText() {
        let painter = TextEngine.pair(resolve: { _ in nil })
        let measurer = painter.measuring
        XCTAssertFalse(painter === measurer)
        let specs = (0..<48).map { spec($0, clamp: $0 % 7 == 0 ? 2 : 0) }
        let widths: [CGFloat] = [180, 240, 320]
        let done = expectation(description: "the owner measured")
        var measured: [Int: Int] = [:]
        Owner.shared.post {
            for round in 0..<30 {
                for (i, s) in specs.enumerated() {
                    let p = measurer.paragraph(s, width: widths[(i + round) % widths.count])
                    if round == 0, (i + round) % widths.count == 0 { measured[i] = p.lines.count }
                }
                if round % 7 == 3 { measurer.dropCold() }
                if round % 11 == 5 { measurer.fitShaped(visibleParagraphs: 8) }
            }
            DispatchQueue.main.async { done.fulfill() }
        }
        var painted: [Int: Int] = [:]
        for round in 0..<30 {
            for (i, s) in specs.enumerated() {
                let width = widths[(i + round) % widths.count]
                let p = painter.paragraph(s, width: width)
                _ = painter.measuredBreaks(s, width: width)
                if round == 0, (i + round) % widths.count == 0 { painted[i] = p.lines.count }
            }
            if round % 5 == 2 { painter.dropCold() }
        }
        wait(for: [done], timeout: 60)
        Owner.shared.sync {}
        XCTAssertFalse(measured.isEmpty)
        XCTAssertEqual(measured, painted, "each engine lays out the same lines")
    }
}

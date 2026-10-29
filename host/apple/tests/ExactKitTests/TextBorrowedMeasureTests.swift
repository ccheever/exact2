import XCTest
import CoreText
import CExact
@testable import ExactKit

extension TextGeometryTests {
    func withBorrowedRequest(_ value: Spec, width: Float = 160, height: Float = -1,
                                     raw: [[UInt8]]? = nil, _ body: (ExactMeasureRequest) -> Void) {
        let strut = value.strut ?? value.runs.first!
        let values = value.runs + [strut]
        let bytes = (raw ?? value.runs.map { Array($0.text.utf8) }) + [Array(strut.text.utf8)]
        var buffers: [UnsafeMutablePointer<UInt8>] = []
        defer { buffers.forEach { $0.deallocate() } }
        var runs: [ExactTextRun] = []
        for (i, v) in values.enumerated() {
            let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: max(1, bytes[i].count))
            bytes[i].withUnsafeBufferPointer { if !$0.isEmpty { buffer.initialize(from: $0.baseAddress!, count: $0.count) } }
            buffers.append(buffer)
            var r = ExactTextRun()
            r.text = UnsafePointer(buffer); r.len = bytes[i].count
            r.font_size = Float(v.size); r.font_weight = UInt16(v.weight); r.font_family = UInt16(v.family)
            r.italic = v.italic ? 1 : 0; r.has_line_height = v.lineHeight == nil ? 0 : 1
            r.line_height = Float(v.lineHeight ?? 0); r.letter_spacing = Float(v.letterSpacing)
            r.font_variant_numeric = UInt8(v.numeric); r.font_stretch = Float(v.stretch)
            runs.append(r)
        }
        let s = runs.removeLast()
        runs.withUnsafeBufferPointer { pointer in
            var request = ExactMeasureRequest()
            request.runs = pointer.baseAddress; request.count = pointer.count; request.strut = s
            request.width = width; request.height = height; request.align = UInt8(value.align)
            request.line_clamp = UInt32(value.lineClamp); request.overflow_wrap = UInt8(value.overflowWrap)
            request.direction = UInt8(value.direction); request.white_space = UInt8(value.whiteSpace)
            body(request)
        }
    }

    private func assertMetrics(_ actual: ExactMetrics, _ expected: ExactMetrics) {
        XCTAssertEqual(actual.width, expected.width)
        XCTAssertEqual(actual.height, expected.height)
        XCTAssertEqual(actual.baseline, expected.baseline)
    }

    func testBorrowedCachedAnswersBypassDecoderAndPreserveCounters() {
        let e = TextEngine(resolve: { _ in nil })
        let value = spec("Café e\u{301} 👩🏽‍💻 אבג repeated text")
        for width in [Float(-2), -1, 160] {
            withBorrowedRequest(value, width: width) { r in
                let expected = e.measure(r), count = e.measureCount, hits = e.measureHits
                let seconds = e.measureSeconds
                #if BORROWED_LOOKUP_SENTINEL
                let decoded = borrowedDecodedRequests
                #endif
                assertMetrics(e.measure(r), expected)
                XCTAssertEqual(e.measureCount, count + 1)
                XCTAssertEqual(e.measureHits, hits + 1)
                XCTAssertGreaterThanOrEqual(e.measureSeconds, seconds)
                #if BORROWED_LOOKUP_SENTINEL
                XCTAssertEqual(borrowedDecodedRequests, decoded, "cached callback must bypass actual Run/Spec construction")
                #endif
            }
        }
        #if BORROWED_LOOKUP_SENTINEL
        let before = borrowedDecodedRequests
        withBorrowedRequest(spec("new source")) { _ = e.measure($0) }
        withBorrowedRequest(value, width: 131) { _ = e.measure($0) }
        XCTAssertEqual(borrowedDecodedRequests, before + 1, "only a cold source needs decoding; a new width reuses its matched identity")
        #endif
    }

    #if os(iOS)
    func testEmergencyHyphenatedAnswersRemainReachableFromRawRequests() {
        let painter = TextEngine(resolve: { _ in nil }), measurer = TextEngine(resolve: { _ in nil })
        painter.connect(measurer: measurer, answerBytes: TextAnswers.defaultLimit)
        var value = spec("Work antidisestablishmentarianism")
        value.overflowWrap = 1
        value.strut = value.runs[0]
        for width: Float in [-2, -1, 140] {
            withBorrowedRequest(value, width: width) { request in
                let prepared = measurer.requestSpec(request)
                XCTAssertFalse(prepared.emergencyBreaks.isEmpty)
                let first = measurer.measure(request), hits = measurer.measureHits
                for _ in 0..<3 { assertMetrics(measurer.measure(request), first) }
                XCTAssertEqual(measurer.measureHits, hits + 3, "identical callbacks reuse the published raw-request answer")
                if width >= 0 {
                    XCTAssertNotNil(painter.measuredBreaks(prepared, width: CGFloat(width)), "prepared geometry remains available to the painter")
                    let p = painter.paragraph(prepared, width: CGFloat(width))
                    XCTAssertEqual(first.height, Float(p.height))
                    XCTAssertEqual(first.baseline, Float(p.firstBaseline))
                    XCTAssertEqual(CTLineGetStringRange(p.lines[0]).length, 5, "ordinary word boundaries keep priority")
                }
            }
        }
    }
    #endif

    func testBorrowedMetricsMatchOriginalCallbackAcrossOffersAndStyles() {
        var values = [spec("É e\u{301} 🧑🏿‍🚀 אבג\nsecond line"), spec(""), spec("longwordwithoutbreaks")]
        var styled = values[0]
        styled.runs[0].italic = true; styled.runs[0].weight = 700; styled.runs[0].letterSpacing = 0.25
        styled.runs.append(Run(text: " extra", size: 17.5, weight: 500, family: 0,
                               italic: false, lineHeight: 0, letterSpacing: -0.125))
        styled.strut = styled.runs[1]; styled.strut?.text = "ignored strut source"
        values.append(styled)
        for var value in values {
            for policy in 0...2 {
                value.overflowWrap = policy; value.lineClamp = policy == 1 ? 2 : 0; value.align = policy
                let a = TextEngine(resolve: { _ in nil })
                #if BORROWED_LOOKUP_SENTINEL
                let b = TextEngine(resolve: { _ in nil })
                #endif
                for width in [Float(-2), -1, -0.0, 0, 97.25, Float(97.25).nextUp, 160, 97.25] {
                    for height in [Float(-2), -1, 0, 73.5] {
                        withBorrowedRequest(value, width: width, height: height) { r in
                            #if BORROWED_LOOKUP_SENTINEL
                            let expected = b.originalMeasure(r)
                            #else
                            let expected = TextEngine(resolve: { _ in nil }).measure(r)
                            #endif
                            assertMetrics(a.measure(r), expected)
                            assertMetrics(a.measure(r), expected)
                        }
                    }
                }
            }
        }
    }

    func testBorrowedIdentityUsesExactFieldsSourceAndRunBoundaries() {
        var value = spec("É e\u{301} emoji🙂")
        value.strut = value.runs[0]; value.strut?.text = "strut text is not geometry"
        var cache = TextResidency()
        let original = cache.identity(value)
        withBorrowedRequest(value) { r in
            XCTAssertEqual(TextMetricKey.hash(r), TextMetricKey.hash(value.geometry))
            XCTAssertTrue(cache.borrowedIdentity(r) === original)
        }
        var variants: [Spec] = []
        func change(_ edit: (inout Spec) -> Void) { var v = value; edit(&v); variants.append(v) }
        change { $0.runs[0].text = "E\u{301} é emoji🙂" }
        change { $0.runs[0].size += 1 }; change { $0.runs[0].weight = 600 }
        change { $0.runs[0].family = 1 }; change { $0.runs[0].italic = true }
        change { $0.runs[0].lineHeight = nil }; change { $0.runs[0].lineHeight = 0 }
        change { $0.runs[0].letterSpacing = 0.5 }; change { $0.align = 2 }
        change { $0.lineClamp = 1 }; change { $0.overflowWrap = 2 }
        change { $0.direction = 1 }; change { $0.whiteSpace = 1 }
        change { $0.strut?.size += 2 }; change { $0.strut?.lineHeight = 0 }
        change { $0.strut?.weight = 600 }; change { $0.strut?.family = 1 }
        change { $0.strut?.italic = true }; change { $0.strut?.lineHeight = nil }
        change { $0.strut?.letterSpacing = 0.5 }
        change { v in var tail = v.runs[0]; tail.text = "emoji🙂"; v.runs[0].text = "É e\u{301} "; v.runs.append(tail) }
        // Also compiled with every metric hash forced to collide. Exact equality,
        // not the fingerprint or canonical String equality, must decide reuse.
        let retained = variants.map { cache.identity($0) }
        for (i, variant) in variants.enumerated() {
            withBorrowedRequest(variant) { r in
                XCTAssertFalse(TextMetricKey.matches(r, original.geometry))
                XCTAssertTrue(cache.borrowedIdentity(r) === retained[i])
            }
        }
        var paint = value
        paint.color = [20, 30, 40, 255]; paint.runs[0].color = [1, 2, 3, 255]
        paint.runs[0].href = "new.md"; paint.runs[0].decoration = "underline"
        paint.strut?.text = "different ignored bytes"; paint.strut?.href = "strut.md"
        XCTAssertTrue(cache.identity(paint) === original)
        withBorrowedRequest(paint) { XCTAssertTrue(cache.borrowedIdentity($0) === original) }
        var noStrut = value; noStrut.strut = nil
        XCTAssertFalse(noStrut.geometry == original.geometry)
        withBorrowedRequest(value) { XCTAssertFalse(TextMetricKey.matches($0, noStrut.geometry)) }
    }

    /// `-webkit-font-smoothing` is paint (LLP 1019 D12): the kernel's request
    /// never carries it, so the paragraph it measured is the one an
    /// `antialiased` paint finds, and the paint keeps the flag.
    func testAnAntialiasedPaintSharesTheMeasuredIdentity() {
        var measured = spec("Settings")
        measured.strut = measured.runs[0]
        var painted = measured; painted.antialiased = true
        var cache = TextResidency()
        let identity = cache.identity(measured)
        XCTAssertTrue(cache.identity(painted) === identity)
        withBorrowedRequest(measured) { XCTAssertTrue(cache.borrowedIdentity($0) === identity) }
        XCTAssertTrue(TextPaint(painted).applying(to: identity).antialiased)
        XCTAssertNotEqual(TextPaint(painted), TextPaint(measured), "a shape painted each way is its own")
    }

    func testBorrowedSignedZeroNaNAndMalformedUTF8Fallback() {
        var value = spec("replacement �")
        value.strut = value.runs[0]; value.runs[0].letterSpacing = -0.0
        var cache = TextResidency(); let identity = cache.identity(value)
        value.runs[0].letterSpacing = 0.0
        withBorrowedRequest(value, width: -0.0) { r in
            XCTAssertEqual(TextMetricKey.hash(r), TextMetricKey.hash(identity.geometry))
            XCTAssertTrue(cache.borrowedIdentity(r) === identity)
            var invalid = r; invalid.strut.font_size = .nan
            XCTAssertFalse(TextMetricKey.matches(invalid, identity.geometry))
            XCTAssertTrue(cache.borrowedIdentity(invalid) == nil)
        }
        let e = TextEngine(resolve: { _ in nil }), reference = TextEngine(resolve: { _ in nil })
        let malformed: [UInt8] = [0x72, 0x65, 0x70, 0x6c, 0x61, 0x63, 0x65, 0x6d, 0x65, 0x6e, 0x74, 0x20, 0xff]
        withBorrowedRequest(value) { _ = e.measure($0) }
        withBorrowedRequest(value, raw: [malformed]) { r in
            XCTAssertTrue(cache.borrowedIdentity(r) == nil)
            #if BORROWED_LOOKUP_SENTINEL
            let decoded = borrowedDecodedRequests
            let expected = reference.originalMeasure(r)
            #else
            let expected = reference.measure(r)
            #endif
            assertMetrics(e.measure(r), expected)
            assertMetrics(e.measure(r), expected)
            #if BORROWED_LOOKUP_SENTINEL
            XCTAssertEqual(borrowedDecodedRequests, decoded + 2, "malformed bytes keep existing replacement decoding")
            #endif
        }
    }

    func testBorrowedCatalogRestorePaintAndIndependentBufferLifetime() {
        let e = TextEngine(resolve: { _ in nil })
        var value = spec("two owners Ée\u{301}🙂"); value.strut = value.runs[0]
        var first = ExactMetrics()
        withBorrowedRequest(value) { first = e.measure($0) }
        var painted = value; painted.runs[0].href = "kept.md"; painted.color = [210, 10, 30, 255]
        let accepted = e.paragraph(painted, width: 160); e.accepted(accepted)
        let checkpoint = e.checkpoint(), catalog = accepted.shape!.identity.catalog
        // First allocation has been freed. An independent allocation with the
        // same exact source can reuse geometry; no borrowed pointer is an ID.
        withBorrowedRequest(value) { assertMetrics(e.measure($0), first) }
        e.install(nil)
        let next = e.paragraph(painted, width: 160)
        XCTAssertTrue(next.shape!.identity.catalog !== catalog)
        e.restore(checkpoint)
        withBorrowedRequest(value) { assertMetrics(e.measure($0), first) }
        XCTAssertTrue(e.paragraph(painted, width: 160) === accepted)
        XCTAssertEqual(accepted.shape!.spec.runs[0].href, "kept.md")
        XCTAssertEqual(Array(accepted.shape!.identity.geometry.runs[0].text.utf8), Array(value.runs[0].text.utf8))
        assertInkMatches(accepted, spec: painted, bounds: CGRect(x: 15, y: 20, width: 160, height: accepted.height),
                         clip: CGRect(x: 0, y: 0, width: 340, height: 160))
    }

    func testBorrowedWeakIdentityEvictionAndMetadataBounds() {
        var value = spec("weak identity"); value.strut = value.runs[0]
        var cache = TextResidency()
        var held: TextIdentity? = cache.identity(value)
        weak var weakIdentity = held
        withBorrowedRequest(value) { XCTAssertTrue(cache.borrowedIdentity($0) === held) }
        held = nil
        XCTAssertTrue(weakIdentity == nil)
        weakIdentity = nil // End the test-owned weak storage after proving last-owner release.
        withBorrowedRequest(value) { XCTAssertTrue(cache.borrowedIdentity($0) == nil) }
        for i in 0..<(TextResidency.maxIdentities + 16) {
            var item = spec("identity \(i)"); item.strut = item.runs[0]
            _ = cache.identity(item)
            withBorrowedRequest(item) { XCTAssertTrue(cache.borrowedIdentity($0) == nil) }
        }
        XCTAssertLessThanOrEqual(cache.stats.identityEntries, TextResidency.maxIdentities)
    }
}


extension TextGeometryTests {
    func testMeasuredGlyphsCrossTheOwnerBoundaryAndPaintWithoutReshaping() throws {
        let painter = TextEngine(resolve: { _ in nil }), measurer = TextEngine(resolve: { _ in nil })
        painter.connect(measurer: measurer, answerBytes: TextAnswers.defaultLimit)
        var before: [Double] = [], after: [Double] = []
        for text in ["office affine e\u{301} wrapped words ", "אבג Latin 日本語 "] {
            for align in 0...2 {
                var value = spec(String(repeating: text, count: 5))
                value.align = align
                value.runs[0].letterSpacing = 0.25
                value.strut = value.runs[0]
                withBorrowedRequest(value, width: 180) { request in
                    Owner.shared.sync { _ = measurer.measure(request) }
                }
                value.color = [200, 20, 70, 128]
                let geometry = try XCTUnwrap(painter.measuredBreaks(value, width: 180))
                let bytes = try XCTUnwrap(painter.answers).observation.bytes
                let glyphs = try XCTUnwrap(painter.measuredGlyphs(value, width: 180))
                XCTAssertLessThan(painter.answers!.observation.bytes, bytes, "taking the glyph copy releases its cache charge")
                XCTAssertNil(painter.measuredGlyphs(value, width: 180), "one raster owns the copy")
                XCTAssertNotNil(painter.measuredBreaks(value, width: 180), "scalar and break answers remain")
                let size = CGSize(width: 190, height: (geometry.baselines.last ?? 0) + 30)
                let box = CGRect(x: 5, y: 5, width: 180, height: size.height - 10)
                for crop in [false, true] {
                    var job = TextRasterJob(source: painter.attributed(value), ranges: geometry.ranges,
                        baselines: geometry.baselines, flush: align == 1 ? 0.5 : align == 2 ? 1 : 0,
                        box: box, size: size, scale: 2, crop: crop)
                    let start = ProcessInfo.processInfo.systemUptime
                    let expected = try XCTUnwrap(job.render())
                    before.append((ProcessInfo.processInfo.systemUptime - start) * 1000)
                    job.measuredGlyphs = glyphs
                    job.measuredColor = TextEngine.color(value.color).cgColor
                    // Empty source makes accidental typesetting unable to reproduce the image.
                    job.source = NSAttributedString(string: "")
                    let next = ProcessInfo.processInfo.systemUptime
                    let actual = try XCTUnwrap(job.render())
                    after.append((ProcessInfo.processInfo.systemUptime - next) * 1000)
                    XCTAssertEqual(actual.frame, expected.frame)
                    XCTAssertEqual(actual.covered, expected.covered)
                    XCTAssertEqual(glyphPixels(actual), glyphPixels(expected), "glyph positions and translucent ink: \(text), align \(align)")
                }
            }
        }
        before.sort(); after.sort()
        print("W822 raster p50 before=\(before[before.count / 2]) after=\(after[after.count / 2]) ms; cases=\(before.count)")
    }

    func testFreshGlyphsRemainPaintableAfterTheAnswerCacheFills() throws {
        let painter = TextEngine(resolve: { _ in nil }), measurer = TextEngine(resolve: { _ in nil })
        let budget = 64 * 1024
        painter.connect(measurer: measurer, answerBytes: budget)
        func value(_ i: Int) -> Spec {
            var value = spec("Row \(i): One short paragraph for a list.")
            value.strut = value.runs[0]
            return value
        }
        for i in 0..<512 { withBorrowedRequest(value(i)) { request in Owner.shared.sync { _ = measurer.measure(request) } } }
        XCTAssertLessThan(painter.answers!.observation.paragraphs, 512, "the scalar budget is saturated")
        var misses = 0, paint: [Double] = []
        for i in 512..<768 {
            let value = value(i)
            withBorrowedRequest(value) { request in Owner.shared.sync { _ = measurer.measure(request) } }
            let geometry = try XCTUnwrap(painter.measuredBreaks(value, width: 160))
            let glyphs = painter.measuredGlyphs(value, width: 160)
            if glyphs == nil { misses += 1 }
            let height = (geometry.baselines.last ?? 0) + 30
            var job = TextRasterJob(source: painter.attributed(value), ranges: geometry.ranges,
                baselines: geometry.baselines, flush: 0, box: CGRect(x: 0, y: 0, width: 160, height: height),
                size: CGSize(width: 160, height: height), scale: 2, crop: false)
            if let glyphs {
                job.measuredGlyphs = glyphs
                job.measuredColor = TextEngine.color(value.color).cgColor
                job.source = NSAttributedString(string: "")
            }
            let began = ProcessInfo.processInfo.systemUptime
            XCTAssertNotNil(job.render())
            paint.append((ProcessInfo.processInfo.systemUptime - began) * 1000)
            XCTAssertLessThanOrEqual(painter.answers!.observation.bytes, budget)
        }
        paint.sort()
        print("W822 saturated: glyph misses=\(misses)/256; raster p50=\(paint[128]) ms, p95=\(paint[243]) ms")
        XCTAssertEqual(misses, 0, "fresh copies survive scalar-answer eviction")
        let last = value(900)
        for width in 120..<128 { withBorrowedRequest(last, width: Float(width)) { _ = measurer.measure($0) } }
        XCTAssertNotNil(painter.measuredGlyphs(last, width: 127), "one paragraph's older offers must not evict its newest glyph copy")
        painter.dropColdShaped()
        for _ in 0..<100 { painter.answers!.discardGlyphs() }
        XCTAssertLessThanOrEqual(painter.answers!.observation.bytes, budget)
    }

    func testMeasuredGlyphsDeclinePaintThatNeedsCoreTextAndExpireWithAnswers() throws {
        let painter = TextEngine(resolve: { _ in nil }), measurer = TextEngine(resolve: { _ in nil })
        painter.connect(measurer: measurer, answerBytes: TextAnswers.defaultLimit)
        var plain = spec("ordinary words")
        plain.strut = plain.runs[0]
        withBorrowedRequest(plain) { _ = measurer.measure($0) }
        var decorated = plain
        decorated.runs[0].underlineColor = [255, 0, 0, 255]
        XCTAssertNil(painter.measuredGlyphs(decorated, width: 160))
        decorated = plain; decorated.ellipsis = true
        XCTAssertNil(painter.measuredGlyphs(decorated, width: 160))
        XCTAssertNotNil(painter.measuredGlyphs(plain, width: 160), "declines do not consume the plain glyph copy")
        var emoji = spec("colour 👩🏽‍💻")
        emoji.strut = emoji.runs[0]
        withBorrowedRequest(emoji) { _ = measurer.measure($0) }
        XCTAssertNil(painter.measuredGlyphs(emoji, width: 160))
        var arabic = spec("مرحبا، سنلتقي في المحطة غداً.")
        arabic.strut = arabic.runs[0]
        withBorrowedRequest(arabic) { _ = measurer.measure($0) }
        XCTAssertNil(painter.measuredGlyphs(arabic, width: 160), "vertical cursive attachment retains CoreText's paint")
        withBorrowedRequest(plain, width: 150) { _ = measurer.measure($0) }
        painter.dropColdShaped()
        XCTAssertNil(painter.measuredGlyphs(plain, width: 150))
        XCTAssertNotNil(painter.measuredBreaks(plain, width: 150), "rest drops glyph storage but keeps scalar answers")
    }

    private func glyphPixels(_ raster: TextRasterImage) -> Data {
        #if os(macOS)
        let surface = raster.surface
        surface.lock(options: .readOnly, seed: nil)
        defer { surface.unlock(options: .readOnly, seed: nil) }
        let width = surface.width * surface.bytesPerElement, height = surface.height, row = surface.bytesPerRow
        let base = surface.baseAddress
        #else
        let image = raster.image
        let data = image.dataProvider!.data!
        let width = image.width * image.bitsPerPixel / 8, height = image.height, row = image.bytesPerRow
        let base = UnsafeRawPointer(CFDataGetBytePtr(data))!
        #endif
        var out = Data()
        for y in 0..<height { out.append(base.advanced(by: y * row).assumingMemoryBound(to: UInt8.self), count: width) }
        return out
    }
}

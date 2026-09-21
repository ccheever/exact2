import Foundation
// Actual ordinary TextEngine is the independent geometry oracle.
import XCTest
import CoreText
@testable import ExactKit

@MainActor final class RegionWorkerTests: XCTestCase {
    func testAuthoredRunIndexPreservesCoalescedAndEmptyRunBoundaries() {
        let engine = TextEngine(resolve: { _ in nil })
        let runs: [Run] = (0..<1024).map { (i: Int) -> Run in
            let height = CGFloat(20 + i % 3)
            return Run(text: i % 7 == 0 ? "" : "ab", size: 16, weight: 400, family: 0,
                italic: false, lineHeight: height, letterSpacing: 0,
                href: "https://example.com/\(i)")
        }
        let source = RegionTextSource.capture(Spec(runs: runs, align: 0, lineClamp: 0,
            color: [0,0,0,255]), engine: engine)
        for start in stride(from: 0, to: source.utf16Count, by: 13) {
            let range = NSRange(location: start, length: 19)
            let expected = source.runs.filter {
                NSMaxRange($0.range) > range.location && $0.range.location < NSMaxRange(range)
            }
            XCTAssertEqual(source.runs(overlapping: range).map(\.range), expected.map(\.range))
            let linked = source.runs.first { NSLocationInRange(start, $0.range) }?.href
            XCTAssertEqual(source.link(at: start), linked)
        }
        XCTAssertNil(source.link(at: source.utf16Count))
    }
    func testNumericPaintIndexMatchesOriginalCoreTextSpansAndEveryBoundary() {
        let evidence = numericIndexEvidence()
        XCTAssertEqual(evidence.error, "")
        XCTAssertGreaterThan(evidence.actual.count, 20)
        XCTAssertEqual(evidence.actual, evidence.expected, "every ordinal retains exact rounded-baseline and two-point ink padding")
        XCTAssertEqual(evidence.queries, evidence.expectedQueries, "closed-overlap queries retain ordinal paint order")
        XCTAssertGreaterThan(evidence.queries.count, evidence.actual.count)
        XCTAssertGreaterThan(evidence.zeroHeightLines, 2)
        XCTAssertGreaterThan(evidence.coincidentSpans, 1)
        XCTAssertEqual(evidence.intrinsicLineCount, 0)
        XCTAssertEqual(evidence.intrinsicMetadataCount, 0)
        XCTAssertEqual(evidence.intrinsicIndexCount, 0)
    }
    #if EXACT_INDEX_QUERY_PROBE
    func testNumericPaintIndexDoesNotRepeatCapturedCoreTextQueries() {
        let evidence = numericIndexEvidence()
        XCTAssertEqual(evidence.error, "")
        XCTAssertGreaterThan(evidence.actual.count, 20, "real multi-line index, not an empty fast path")
        XCTAssertEqual(evidence.inkCalls, 0, "index must reuse previously captured glyph-path bounds")
        XCTAssertEqual(evidence.metricCalls, 0, "index must reuse previously captured typographic metrics")
        print("index-work lines=\(evidence.actual.count) ink=\(evidence.inkCalls) metrics=\(evidence.metricCalls)")
    }
    #endif
    func testCompactBreakingMatchesCoreTextForWhitespaceUnicodeAndOverflow() {
        let engine = TextEngine(resolve: { _ in nil })
        let texts = ["alpha     beta\n\nnext\n", "    alpha\tbeta  ", "one\u{00a0}two three",
                     "漢字の段落、句読点。次の行。", "ffi e\u{301} العربية אבג 👨‍👩‍👧‍👦 end",
                     "longwordwithoutbreaks then short words", "a\r\nb\r\n\r\nc"]
        for text in texts {
            for wrap in [0, 1, 2] {
                let run = Run(text: text, size: 16, weight: 400, family: 0, italic: false,
                              lineHeight: 26, letterSpacing: 0)
                let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [0,0,0,255],
                                overflowWrap: wrap, whiteSpace: 1, strut: run)
                let source = RegionTextSource.capture(spec, engine: engine)
                for width: CGFloat in [0, 1, 15, 61.25, 125, 300] {
                    let ordinary = engine.paragraph(spec, width: width)
                    let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
                    DispatchQueue.global().async {
                        box.value = RegionWorkerLayout.shape(source, width: width, compact: true).metadata
                        done.signal()
                    }
                    done.wait()
                    let ranges = ordinary.lines.map { CTLineGetStringRange($0) }.map {
                        NSRange(location: $0.location, length: $0.length)
                    }
                    XCTAssertEqual(box.value!.lines.map(\.range), ranges, "\(text.debugDescription), wrap \(wrap), width \(width)")
                    XCTAssertEqual(box.value!.height, ordinary.height)
                    XCTAssertEqual(box.value!.width, ordinary.width)
                }
            }
        }
    }
    func testShapeDoesNotEagerlyExpandEveryCaret() {
        let run = Run(text: String(repeating: "ffi אבג words ", count: 40), size: 16,
            weight: 400, family: 0, italic: false, lineHeight: 26, letterSpacing: 0)
        let source = RegionTextSource.capture(Spec(runs: [run], align: 0, lineClamp: 0,
            color: [0,0,0,255], strut: run), engine: TextEngine(resolve: { _ in nil }))
        let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
        DispatchQueue(label: "region-lean-shape-test").async {
            box.value = RegionWorkerLayout.shape(source, width: 120).metadata
            done.signal()
        }
        done.wait()
        XCTAssertGreaterThan(box.value!.lines.count, 10)
        XCTAssertTrue(box.value!.lines.allSatisfy { $0.carets.isEmpty },
            "shape must preserve full lines/extents without eagerly expanding all UTF16 carets")
        XCTAssertEqual(box.value!.copy(NSRange(location: 0,length: source.utf16Count)), source.text)
    }
    func testWorkerMetricsAndOwnedHitsMatchOrdinary() {
        let engine = TextEngine(resolve: { _ in nil })
        let values = ["", "Latin e\u{301} العربية אבג 👨‍👩‍👧‍👦 ffi\nnext", String(repeating: "ordinary glyphs ", count: 120)]
        for text in values {
            let run = Run(text: text, size: 16, weight: 400, family: 0, italic: false,
                          lineHeight: 26, letterSpacing: 0, decoration: "underline")
            let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run)
            let source = RegionTextSource.capture(spec, engine: engine)
            for width: CGFloat in [0, 165.25, 600] {
                let ordinary = engine.paragraph(spec, width: width)
                let box = RegionTestBox()
                let done = DispatchSemaphore(value: 0)
                DispatchQueue(label: "region-test").async {
                    box.value = denseRegionReference(RegionWorkerLayout.shape(source, width: width, compact: true))
                    done.signal()
                }
                done.wait()
                let result = box.value!
                XCTAssertFalse(result.shapedOnMainThread)
                XCTAssertEqual(result.width, ordinary.width)
                XCTAssertEqual(result.height, ordinary.height)
                XCTAssertEqual(result.baselines, ordinary.baselines)
                XCTAssertEqual(result.copy(NSRange(location: 0, length: source.utf16Count)), text)
                for (i, line) in ordinary.lines.enumerated() {
                    for x in stride(from: CGFloat(-2), through: max(4, ordinary.width + 2), by: 2.25) {
                        XCTAssertEqual(result.lines[i].index(at: x), CTLineGetStringIndexForPosition(line, CGPoint(x: x, y: 0)))
                    }
                }
            }
        }
    }
    func testStationaryFractionalPhasePixelsMatchUnprunedOrdinary() {
        let engine = TextEngine(resolve: { _ in nil })
        let run = Run(text: String(repeating: "Underlined ffi e\u{301} 👩🏽‍🚀 אבג\n", count: 8), size: 16,
                      weight: 400, family: 0, italic: true, lineHeight: 26.25, letterSpacing: 0,
                      decoration: "underline line-through")
        let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [40,60,80,255], strut: run)
        let source = RegionTextSource.capture(spec, engine: engine)
        let ordinary = engine.paragraph(spec, width: 320)
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let profile = NativeProfile.capture(original: space, data: space.copyICCData(), account: NativeProfileAccount()).owner!
        for format in [CGImageAlphaInfo.premultipliedLast.rawValue, RegionRasterRequest.compositedFormat] {
            for background: [CGFloat] in [[1,1,1,1], [0,0,0,0]] {
                for scale in [1, 2] {
                    for scroll: CGFloat in [0, 0.375, 27.25, 103.875] {
                        let large = background[3] == 0 && scale == 2
                        let size = CGSize(width: large ? 1200 : 397, height: large ? 500 : 100)
                        let width = Int(size.width) * scale, height = Int(size.height) * scale
                        let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                            bytesPerRow: width * 4, space: space, bitmapInfo: format)!
                        ctx.setFillColor(CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(), components: background)!)
                        ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
                        ctx.translateBy(x: 0, y: CGFloat(height)); ctx.scaleBy(x: CGFloat(scale), y: -CGFloat(scale))
                        TextEngine.draw(ordinary, spec: spec, in: CGRect(x: 7.25, y: 0.125 - scroll, width: 320, height: ordinary.height), context: ctx)
                        ctx.flush()
                        let expected = Data(bytes: ctx.data!, count: width * height * 4)
                        let request = RegionRasterRequest(serial: 1, publication: 1, generation: 1,
                            rows: [RegionPaintRow(artifact: 7, box: CGRect(x: 7.25, y: 0.125, width: 320, height: ordinary.height))],
                            scroll: CGPoint(x: 0, y: scroll), size: size, scale: scale,
                            profile: profile, format: format, background: background, selectionColor: [0.2,0.4,0.8,0.45],
                            pixelLimit: large ? RegionRasterRequest.maximumPixelLimit : 8 * 1024 * 1024)
                        if large {
                            var surface = request; surface.pixelLimit = 8 * 1024 * 1024
                            XCTAssertNil(surface.bytes, "the registered surface keeps its existing admission")
                            XCTAssertNotNil(request.bytes, "a reader admits its whole visible viewport")
                        }
                        let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
                        DispatchQueue(label: "region-pixel-test").async {
                            do {
                                let layout = RegionWorkerLayout.shape(source, width: 320, compact: true)
                                XCTAssertTrue(layout.lines.isEmpty, "glyphs materialize only for the viewport")
                                let index = try RegionPaintIndex(request: request, lookup: { $0 == 7 ? layout : nil }, account: InkAccount())
                                box.raster = try index.render(request, account: RegionPixelAccount(), hits: RegionHitAccount())
                            } catch { box.error = String(describing: error) }
                            done.signal()
                        }
                        done.wait()
                        XCTAssertEqual(box.error, "")
                        let image = box.raster!.image()!
                        let actual = image.dataProvider!.data! as Data
                        for row in 0..<height {
                            XCTAssertEqual(actual[(row * image.bytesPerRow)..<(row * image.bytesPerRow + width * 4)],
                                           expected[(row * width * 4)..<((row + 1) * width * 4)])
                        }
                        XCTAssertTrue(box.raster!.accepts(image, size: request.size, scale: scale, profile: profile))
                        XCTAssertFalse(box.raster!.accepts(image, size: request.size, scale: scale == 1 ? 2 : 1, profile: profile))
                        XCTAssertFalse(box.raster!.accepts(image, size: CGSize(width: 401,height: 100), scale: scale, profile: profile))

                    }
                }
            }
        }
    }
    func testResetWhileWorkerBlockedHasOneActiveAndOnlyLatestRecovery() {
        let engine = TextEngine(resolve: { _ in nil })
        let run = Run(text: "worker reset barrier", size: 16, weight: 400, family: 0, italic: false, lineHeight: 26, letterSpacing: 0)
        let source = RegionTextSource.capture(Spec(runs: [run], align: 0, lineClamp: 0, color: [0,0,0,255], strut: run), engine: engine)
        let gate = RegionTestGate()
        let received = RegionTestDelivery()
        let lifetime = RegionServiceLifetime(beforeShape: { gate.enter() }) { answer in
            if case .shape(let artifact) = answer { received.ids.append(artifact.id) }
        }
        func request(_ n: UInt64) -> RegionJob {
            .shape(RegionShapeRequest(id: n, sourceID: n, source: source, width: 300, height: -1, generation: Int(n)))
        }
        lifetime.service.submit(request(1)); gate.started.wait()
        for i in 2...40 { lifetime.reset(); lifetime.service.submit(request(UInt64(i))) }
        XCTAssertEqual(gate.started.wait(timeout: .now() + 0.05), .timedOut)
        XCTAssertEqual(gate.count, 1, "old blocked allocation still owns the only worker turn")
        gate.release.signal()
        let deadline = Date().addingTimeInterval(3)
        while received.ids.isEmpty && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.005)) }
        XCTAssertEqual(received.ids, [40])
        XCTAssertEqual(gate.count, 2)
        lifetime.service.close()
    }
    func testVisibleWitnessRejectsResizeBeforeNewRaster() {
        let witness = RegionVisibleWitness(publication: 7, size: CGSize(width: 400,height: 300), scroll: CGPoint(x: 0,y: 0.375), profile: Data([1,2]), scale: 1)
        XCTAssertTrue(witness.matches(publication: 7, size: CGSize(width: 400,height: 300), scroll: CGPoint(x: 0,y: 0.375), profile: Data([1,2]), scale: 1))
        XCTAssertFalse(witness.matches(publication: 7, size: CGSize(width: 401,height: 300), scroll: CGPoint(x: 0,y: 0.375), profile: Data([1,2]), scale: 1))
        XCTAssertFalse(witness.matches(publication: 7, size: CGSize(width: 400,height: 300), scroll: CGPoint(x: 0,y: 0.5), profile: Data([1,2]), scale: 1))
        XCTAssertFalse(witness.matches(publication: 7, size: CGSize(width: 400,height: 300), scroll: CGPoint(x: 0,y: 0.375), profile: Data([1,2]), scale: 2))
        XCTAssertFalse(witness.matches(publication: 8, size: CGSize(width: 400,height: 300), scroll: CGPoint(x: 0,y: 0.375), profile: Data([1,2]), scale: 1))
    }
    func testGiantParagraphHitUsesBoundsNotMidpoint() {
        let heading = RegionFrame(key: 1, id: 1, kind: "text", box: CGRect(x: 20,y: 20,width: 600,height: 40), extent: .zero, artifact: 1)
        let giant = RegionFrame(key: 2, id: 2, kind: "text", box: CGRect(x: 20,y: 80,width: 600,height: 1450000), extent: .zero, artifact: 2)
        let point = CGPoint(x: 100,y: 100)
        XCTAssertLessThan(giant.hitDistance(point), heading.hitDistance(point))
        XCTAssertEqual(giant.hitDistance(point), 0)
        XCTAssertLessThan(heading.hitDistance(CGPoint(x: 100,y: 30)), giant.hitDistance(CGPoint(x: 100,y: 30)))
        XCTAssertEqual(giant.hitDistance(CGPoint(x: 621,y: 100)), 1)
    }
    func testCapturedSourceDigestDistinguishesEqualCounts() {
        let engine = TextEngine(resolve: { _ in nil })
        func capture(_ text: String) -> RegionTextSource {
            let run = Run(text: text, size: 16, weight: 400, family: 0, italic: false, lineHeight: 26, letterSpacing: 0)
            return RegionTextSource.capture(Spec(runs: [run], align: 0, lineClamp: 0, color: [0,0,0,255], strut: run), engine: engine)
        }
        func digest(_ source: RegionTextSource) -> String {
            let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
            DispatchQueue(label: "source-digest-test").async {
                box.value = RegionWorkerLayout.shape(source,width: 100).metadata
                done.signal()
            }
            done.wait()
            return box.value!.sourceSHA256
        }
        let first = capture("abc"), changed = capture("cba")
        let firstDigest = digest(first), changedDigest = digest(changed)
        XCTAssertEqual(firstDigest, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        XCTAssertEqual(first.utf16Count, changed.utf16Count)
        XCTAssertFalse(firstDigest == changedDigest)
    }
    func testIntrinsicOffersUseWorkerFontsAndScalarOnly() {
        let engine = TextEngine(resolve: { _ in nil })
        let run = Run(text: "one longlongword two 👩🏽‍🚀", size: 16, weight: 400, family: 0,
                      italic: false, lineHeight: 26, letterSpacing: 0)
        let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [0,0,0,255], strut: run)
        let source = RegionTextSource.capture(spec, engine: engine)
        let expected = engine.minContentWidth(spec)
        let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
        DispatchQueue(label: "region-intrinsic-test").async {
            box.width = RegionWorkerLayout.minimumWidth(source)
            box.value = RegionWorkerLayout.shape(source, width: box.width, retainHits: false).metadata
            done.signal()
        }
        done.wait()
        XCTAssertEqual(box.width, expected)
        XCTAssertTrue(box.value!.lines.isEmpty)
        XCTAssertGreaterThan(box.value!.height, 0)
    }

    private func rangeSpec(clamp: Int = 0) -> Spec {
        let texts = ["", "before ", "after ffi e\u{301} ", "🧑🏿‍🚀 אבג العربية ", "", "東京 ไทย\n",
                     String(decoding: [0x65, 0xcc, 0x81, 0xff], as: UTF8.self) + " tail "]
        let runs = texts.enumerated().map { i, text in
            Run(text: text, size: 16, weight: 400, family: 0, italic: true,
                lineHeight: i == 2 ? nil : i == 1 ? 18.25 : 34.5, letterSpacing: 0,
                decoration: "underline line-through", href: "range.md")
        }
        var strut = runs[0]; strut.lineHeight = 12.25
        return Spec(runs: runs, align: 0, lineClamp: clamp, color: [40, 60, 80, 255], strut: strut)
    }

    func testCapturedUTF16RangesPreserveStrictIntersections() {
        let engine = TextEngine(resolve: { _ in nil }), spec = rangeSpec()
        let source = RegionTextSource.capture(spec, engine: engine)
        var offset = 0
        var oldRanges: [NSRange] = []
        for (authored, captured) in zip(spec.runs, source.runs) {
            let old = NSRange(location: offset, length: (authored.text as NSString).length)
            XCTAssertEqual(captured.range, old)
            oldRanges.append(old); offset = NSMaxRange(old)
        }
        XCTAssertEqual(offset, source.utf16Count)
        XCTAssertGreaterThan(source.utf16Count, source.text.count)
        // Include empty and touching intervals, not just ordinary glyph ranges.
        for start in 0...offset { for length in 0...(offset - start) {
            let end = start + length
            let old = oldRanges.enumerated().filter { $0.element.location < end && NSMaxRange($0.element) > start }.map(\.offset)
            let captured = source.runs.enumerated().filter { $0.element.range.location < end && NSMaxRange($0.element.range) > start }.map(\.offset)
            XCTAssertEqual(captured, old)
        } }
    }

    func testMixedRunWorkerGlyphsMetricsCaretsAndClampMatchOrdinary() {
        let engine = TextEngine(resolve: { _ in nil })
        var coalesced = false
        for clamp in [0, 2] { for wrap in [0, 1, 2] {
            var spec = rangeSpec(clamp: clamp); spec.overflowWrap = wrap
            let source = RegionTextSource.capture(spec, engine: engine)
            for width: CGFloat in [0, 83.25, 213.5] {
                let ordinary = engine.paragraph(spec, width: width)
                let expectedGlyphs = ordinary.lines.map(RegionGlyphEvidence.init)
                let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
                DispatchQueue(label: "region-range-parity").async {
                    let layout = RegionWorkerLayout.shape(source, width: width)
                    box.value = denseRegionReference(layout)
                    box.glyphs = layout.lines.map(RegionGlyphEvidence.init)
                    done.signal()
                }
                done.wait()
                let result = box.value!
                XCTAssertFalse(result.shapedOnMainThread)
                XCTAssertEqual(result.width, ordinary.width)
                XCTAssertEqual(result.height, ordinary.height)
                XCTAssertEqual(result.baselines, ordinary.baselines)
                XCTAssertEqual(result.lineBottoms, ordinary.lineBottoms)
                XCTAssertEqual(result.lines.count, ordinary.lines.count)
                XCTAssertEqual(box.glyphs, expectedGlyphs)
                XCTAssertEqual(result.copy(NSRange(location: 0, length: source.utf16Count)), source.text)
                for (i, line) in ordinary.lines.enumerated() {
                    let range = CTLineGetStringRange(line)
                    XCTAssertEqual(result.lines[i].range, NSRange(location: range.location, length: range.length))
                    for index in range.location...(range.location + range.length) {
                        var secondary: CGFloat = 0
                        let primary = CTLineGetOffsetForStringIndex(line, index, &secondary)
                        XCTAssertEqual(result.lines[i].offsets(at: index).primary, primary)
                        XCTAssertEqual(result.lines[i].offsets(at: index).secondary, secondary)
                    }
                    for x in stride(from: CGFloat(-2), through: ordinary.width + 2, by: 2.25) {
                        XCTAssertEqual(result.lines[i].index(at: x), CTLineGetStringIndexForPosition(line, CGPoint(x: x, y: 0)))
                    }
                    for run in CTLineGetGlyphRuns(line) as! [CTRun] {
                        let r = CTRunGetStringRange(run)
                        if source.runs.filter({ $0.range.location < r.location + r.length && NSMaxRange($0.range) > r.location }).count > 1 {
                            coalesced = true
                        }
                    }
                }
            }
        } }
        XCTAssertTrue(coalesced, "fixture must exercise one glyph run intersecting multiple authored line-height spans")
    }

    func testSharedPreparationUnicodeBoundariesAcrossIntrinsicAndFiniteWidths() {
        let texts = ["", "unbrokenword", "a\n\nb\r\nc",
            "東京都は日本の首都です。中文段落沿着河边展开。",
            "ภาษาไทยไม่มีช่องว่างระหว่างคำ จึงต้องใช้พจนานุกรมในการตัดคำ",
            "👨‍👩‍👧‍👦 e\u{301} العربية שלום soft\u{ad}hyphen no\u{00a0}break"]
        let engine = TextEngine(resolve: { _ in nil })
        for text in texts {
            let run = Run(text: text, size: 16, weight: 400, family: 0, italic: false,
                          lineHeight: 22, letterSpacing: 0)
            let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [20,40,60,255], strut: run)
            let source = RegionTextSource.capture(spec, engine: engine)
            for width: CGFloat in [0, 30, 83.25, 260, 525, 90] {
                let ordinary = engine.paragraph(spec, width: width)
                let expectedGlyphs = ordinary.lines.map(RegionGlyphEvidence.init)
                let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
                DispatchQueue(label: "region-unicode-reuse").async {
                    // Intrinsic preparation must still acquire finite-width
                    // boundaries, and later widths must retain the same breaks.
                    let intrinsic = RegionWorkerLayout.shape(source, width: .infinity)
                    let first = RegionWorkerLayout.shape(source, width: 311, preparation: intrinsic.preparation)
                    let layout = RegionWorkerLayout.shape(source, width: width, preparation: first.preparation)
                    box.preparationShared = intrinsic.preparation === layout.preparation
                    box.value = denseRegionReference(layout)
                    box.glyphs = layout.lines.map(RegionGlyphEvidence.init)
                    done.signal()
                }
                done.wait()
                let result = box.value!
                XCTAssertTrue(box.preparationShared)
                XCTAssertEqual(result.width, ordinary.width)
                XCTAssertEqual(result.height, ordinary.height)
                XCTAssertEqual(result.baselines, ordinary.baselines)
                XCTAssertEqual(result.lineBottoms, ordinary.lineBottoms)
                XCTAssertEqual(box.glyphs, expectedGlyphs)
                XCTAssertEqual(result.copy(NSRange(location: 0, length: source.utf16Count)), text)
            }
        }
    }

    func testSharedPreparationGlyphsMetricsCaretsAndClampMatchOrdinary() {
        let engine = TextEngine(resolve: { _ in nil })
        var coalesced = false
        for clamp in [0, 2] { for wrap in [0, 1, 2] {
            var spec = rangeSpec(clamp: clamp); spec.overflowWrap = wrap
            let source = RegionTextSource.capture(spec, engine: engine)
            for width: CGFloat in [0, 83.25, 213.5] {
                let ordinary = engine.paragraph(spec, width: width)
                let expectedGlyphs = ordinary.lines.map(RegionGlyphEvidence.init)
                let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
                DispatchQueue(label: "region-range-parity").async {
                    let first = RegionWorkerLayout.shape(source,width: 311)
                    let layout = RegionWorkerLayout.shape(source, width: width, preparation: first.preparation)
                    box.preparationShared = first.preparation === layout.preparation
                    box.value = denseRegionReference(layout)
                    box.glyphs = layout.lines.map(RegionGlyphEvidence.init)
                    done.signal()
                }
                done.wait()
                let result = box.value!
                XCTAssertTrue(box.preparationShared)
                XCTAssertFalse(result.shapedOnMainThread)
                XCTAssertEqual(result.width, ordinary.width)
                XCTAssertEqual(result.height, ordinary.height)
                XCTAssertEqual(result.baselines, ordinary.baselines)
                XCTAssertEqual(result.lineBottoms, ordinary.lineBottoms)
                XCTAssertEqual(result.lines.count, ordinary.lines.count)
                XCTAssertEqual(box.glyphs, expectedGlyphs)
                XCTAssertEqual(result.copy(NSRange(location: 0, length: source.utf16Count)), source.text)
                for (i, line) in ordinary.lines.enumerated() {
                    let range = CTLineGetStringRange(line)
                    XCTAssertEqual(result.lines[i].range, NSRange(location: range.location, length: range.length))
                    for index in range.location...(range.location + range.length) {
                        var secondary: CGFloat = 0
                        let primary = CTLineGetOffsetForStringIndex(line, index, &secondary)
                        XCTAssertEqual(result.lines[i].offsets(at: index).primary, primary)
                        XCTAssertEqual(result.lines[i].offsets(at: index).secondary, secondary)
                    }
                    for x in stride(from: CGFloat(-2), through: ordinary.width + 2, by: 2.25) {
                        XCTAssertEqual(result.lines[i].index(at: x), CTLineGetStringIndexForPosition(line, CGPoint(x: x, y: 0)))
                    }
                    for run in CTLineGetGlyphRuns(line) as! [CTRun] {
                        let r = CTRunGetStringRange(run)
                        if source.runs.filter({ $0.range.location < r.location + r.length && NSMaxRange($0.range) > r.location }).count > 1 {
                            coalesced = true
                        }
                    }
                }
            }
        } }
        XCTAssertTrue(coalesced, "fixture must exercise one glyph run intersecting multiple authored line-height spans")
    }

    func testMixedRunStationaryPixelsMatchOrdinary() {
        let engine = TextEngine(resolve: { _ in nil })
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let profile = NativeProfile.capture(original: space, data: space.copyICCData(), account: NativeProfileAccount()).owner!
        for clamp in [0, 2] {
            let spec = rangeSpec(clamp: clamp), source = RegionTextSource.capture(rangeSpec(clamp: clamp), engine: engine)
            let ordinary = engine.paragraph(spec, width: 163.25)
            for scale in [1, 2] { for scroll: CGFloat in [0, 27.375] {
                let width = 240 * scale, height = 120 * scale
                let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                    bytesPerRow: width * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
                ctx.setFillColor(CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(), components: [1, 1, 1, 1])!)
                ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
                ctx.translateBy(x: 0, y: CGFloat(height)); ctx.scaleBy(x: CGFloat(scale), y: -CGFloat(scale))
                TextEngine.draw(ordinary, spec: spec,
                    in: CGRect(x: 7.25, y: 0.125 - scroll, width: 163.25, height: ordinary.height), context: ctx)
                ctx.flush()
                let expected = Data(bytes: ctx.data!, count: width * height * 4)
                let request = RegionRasterRequest(serial: 1, publication: 1, generation: 1,
                    rows: [RegionPaintRow(artifact: 7, box: CGRect(x: 7.25, y: 0.125, width: 163.25, height: ordinary.height))],
                    scroll: CGPoint(x: 0, y: scroll), size: CGSize(width: 240, height: 120), scale: scale,
                    profile: profile, format: CGImageAlphaInfo.premultipliedLast.rawValue,
                    background: [1, 1, 1, 1], selectionColor: [0.2, 0.4, 0.8, 0.45])
                let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
                DispatchQueue(label: "region-range-pixels").async {
                    do {
                        let layout = RegionWorkerLayout.shape(source, width: 163.25)
                        let index = try RegionPaintIndex(request: request, lookup: { $0 == 7 ? layout : nil }, account: InkAccount())
                        box.raster = try index.render(request, account: RegionPixelAccount(), hits: RegionHitAccount())
                    } catch { box.error = String(describing: error) }
                    done.signal()
                }
                done.wait()
                XCTAssertEqual(box.error, "")
                XCTAssertEqual(box.raster!.image()!.dataProvider!.data! as Data, expected)
            } }
        }
    }

    func testSharedPreparationStationaryPixelsMatchOrdinary() {
        let engine = TextEngine(resolve: { _ in nil })
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let profile = NativeProfile.capture(original: space, data: space.copyICCData(), account: NativeProfileAccount()).owner!
        for clamp in [0, 2] {
            let spec = rangeSpec(clamp: clamp), source = RegionTextSource.capture(rangeSpec(clamp: clamp), engine: engine)
            let ordinary = engine.paragraph(spec, width: 163.25)
            for scale in [1, 2] { for scroll: CGFloat in [0, 27.375] {
                let width = 240 * scale, height = 120 * scale
                let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                    bytesPerRow: width * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
                ctx.setFillColor(CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(), components: [1, 1, 1, 1])!)
                ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
                ctx.translateBy(x: 0, y: CGFloat(height)); ctx.scaleBy(x: CGFloat(scale), y: -CGFloat(scale))
                TextEngine.draw(ordinary, spec: spec,
                    in: CGRect(x: 7.25, y: 0.125 - scroll, width: 163.25, height: ordinary.height), context: ctx)
                ctx.flush()
                let expected = Data(bytes: ctx.data!, count: width * height * 4)
                let request = RegionRasterRequest(serial: 1, publication: 1, generation: 1,
                    rows: [RegionPaintRow(artifact: 7, box: CGRect(x: 7.25, y: 0.125, width: 163.25, height: ordinary.height))],
                    scroll: CGPoint(x: 0, y: scroll), size: CGSize(width: 240, height: 120), scale: scale,
                    profile: profile, format: CGImageAlphaInfo.premultipliedLast.rawValue,
                    background: [1, 1, 1, 1], selectionColor: [0.2, 0.4, 0.8, 0.45])
                let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
                DispatchQueue(label: "region-range-pixels").async {
                    do {
                        let first = RegionWorkerLayout.shape(source,width: 311)
                        let layout = RegionWorkerLayout.shape(source, width: 163.25, preparation: first.preparation)
                        box.preparationShared = first.preparation === layout.preparation
                        let index = try RegionPaintIndex(request: request, lookup: { $0 == 7 ? layout : nil }, account: InkAccount())
                        box.raster = try index.render(request, account: RegionPixelAccount(), hits: RegionHitAccount())
                    } catch { box.error = String(describing: error) }
                    done.signal()
                }
                done.wait()
                XCTAssertEqual(box.error, "")
                XCTAssertTrue(box.preparationShared)
                XCTAssertEqual(box.raster!.image()!.dataProvider!.data! as Data, expected)
            } }
        }
    }
}
// Glyph/layout objects are read only on their owning executor; only these
// owned scalar arrays cross the semaphore handoff for the parity assertion.
private struct RegionGlyphEvidence: Sendable, Equatable {
    struct RunEvidence: Sendable, Equatable {
        let location: Int, length: Int
        let glyphs: [CGGlyph], indices: [CFIndex], positions: [CGPoint], advances: [CGSize]
        let font: String
        let size: CGFloat
    }
    let runs: [RunEvidence]
    init(_ line: CTLine) {
        runs = (CTLineGetGlyphRuns(line) as! [CTRun]).map { run in
            let count = CTRunGetGlyphCount(run), range = CTRunGetStringRange(run)
            var glyphs = [CGGlyph](repeating: 0, count: count)
            var indices = [CFIndex](repeating: 0, count: count)
            var positions = [CGPoint](repeating: .zero, count: count)
            var advances = [CGSize](repeating: .zero, count: count)
            CTRunGetGlyphs(run, CFRange(location: 0, length: 0), &glyphs)
            CTRunGetStringIndices(run, CFRange(location: 0, length: 0), &indices)
            CTRunGetPositions(run, CFRange(location: 0, length: 0), &positions)
            CTRunGetAdvances(run, CFRange(location: 0, length: 0), &advances)
            let font = (CTRunGetAttributes(run) as NSDictionary)[kCTFontAttributeName] as! CTFont
            return RunEvidence(location: range.location, length: range.length, glyphs: glyphs,
                indices: indices, positions: positions, advances: advances,
                font: CTFontCopyPostScriptName(font) as String, size: CTFontGetSize(font))
        }
    }
}
// Test-only handoff: writer completes before semaphore establishes reader access.
private final class RegionTestBox: @unchecked Sendable {
    var value: RegionParagraph?
    var glyphs: [RegionGlyphEvidence] = []
    var preparationShared = false
    var width: CGFloat = 0
    var raster: RegionRaster?
    var error = ""
}

@MainActor private final class RegionTestDelivery { var ids: [UInt64] = [] }
private final class RegionTestGate: @unchecked Sendable {
    let started = DispatchSemaphore(value: 0), release = DispatchSemaphore(value: 0)
    private let lock = NSLock()
    private var value = 0
    var count: Int { lock.lock(); defer { lock.unlock() }; return value }
    func enter() {
        lock.lock(); value += 1; let first = value == 1; lock.unlock()
        started.signal()
        if first { release.wait() }
    }
}

// Preserve the original exhaustive owned-metadata oracle on the worker. The
// production shape now intentionally publishes only scalar line geometry.
private func denseRegionReference(_ layout: RegionWorkerLayout) -> RegionParagraph {
    let p = layout.metadata
    return RegionParagraph(source: p.source,sourceSHA256: p.sourceSHA256,lines: (0..<layout.lineCount).map { layout.line(at: $0) },baselines: p.baselines,
        width: p.width,height: p.height,lineBottoms: p.lineBottoms,offeredWidth: p.offeredWidth)
}

// All CTLine and index introspection stays on the worker. Only numeric evidence
// crosses the semaphore; no production getter or second index storage is added.
private final class NumericIndexEvidence: @unchecked Sendable {
    var error = ""
    var actual: [[Double]] = [], expected: [[Double]] = []
    var queries: [[Int32]] = [], expectedQueries: [[Int32]] = []
    var zeroHeightLines = 0, coincidentSpans = 0
    var intrinsicLineCount = -1, intrinsicMetadataCount = -1, intrinsicIndexCount = -1
    var inkCalls = -1, metricCalls = -1
}
@MainActor private func numericIndexEvidence() -> NumericIndexEvidence {
    let engine = TextEngine(resolve: { _ in nil })
    func source(_ text: String, height: CGFloat, clamp: Int = 0) -> RegionTextSource {
        let a = Run(text: text,size: 16,weight: 400,family: 0,italic: true,
            lineHeight: height,letterSpacing: 0,decoration: "underline")
        return RegionTextSource.capture(Spec(runs: [a],align: 2,lineClamp: clamp,
            color: [0,0,0,255],strut: a),engine: engine)
    }
    let samples = [source("",height: 0),
        source(String(repeating: "ffi e\u{301} אבג 👩🏽‍🚀 words\n",count: 8),height: 26.25),
        source("",height: 0),
        source(String(repeating: "zero overlapping words\n",count: 6),height: 0),
        source(String(repeating: "clamped fallback 👨‍👩‍👧‍👦 words ",count: 5),height: 29.75,clamp: 2)]
    let space = CGColorSpace(name: CGColorSpace.sRGB)!
    let profile = NativeProfile.capture(original: space,data: space.copyICCData(),account: NativeProfileAccount()).owner!
    let box = NumericIndexEvidence(), done = DispatchSemaphore(value: 0)
    DispatchQueue(label: "numeric-paint-index-test").async {
        defer { done.signal() }
        do {
            let layouts = samples.map { RegionWorkerLayout.shape($0,width: 103.25) }
            let origins: [CGFloat] = [10,7.375,10,-8.625,3.125]
            let rows = layouts.enumerated().map { i,l in
                RegionPaintRow(artifact: UInt64(i+1),box: CGRect(x: 2.5,y: origins[i],width: 103.25,height: l.metadata.height))
            }
            let request = RegionRasterRequest(serial: 1,publication: 1,generation: 1,rows: rows,
                scroll: .zero,size: CGSize(width: 160,height: 100),scale: 1,profile: profile,
                format: CGImageAlphaInfo.premultipliedLast.rawValue,background: [1,1,1,1],selectionColor: [0,0,0,0])
            // Independent original CoreText formula, including exact operand order.
            for (p,l) in layouts.enumerated() {
                for (i,line) in l.lines.enumerated() {
                    let ink = CTLineGetBoundsWithOptions(line,.useGlyphPathBounds)
                    var a: CGFloat = 0, d: CGFloat = 0, leading: CGFloat = 0
                    _ = CTLineGetTypographicBounds(line,&a,&d,&leading)
                    let above = max(a + max(leading,0),ink.isNull ? 0 : ink.maxY)
                    let below = max(d + max(leading,0),ink.isNull ? 0 : -ink.minY)
                    let y = rows[p].box.minY + l.baselines[i].rounded()
                    box.expected.append([Double(y - above - 2),Double(y + below + 2)])
                }
            }
            box.zeroHeightLines = layouts[3].metadata.height == 0 ? layouts[3].lines.count : 0
            #if EXACT_INDEX_QUERY_PROBE
            NumericIndexCTProbe.reset()
            #endif
            let paint = try RegionPaintIndex(request: request,lookup: { id in
                id > 0 && id <= UInt64(layouts.count) ? layouts[Int(id)-1] : nil
            },account: InkAccount())
            #if EXACT_INDEX_QUERY_PROBE
            box.inkCalls = NumericIndexCTProbe.inkCalls
            box.metricCalls = NumericIndexCTProbe.metricCalls
            #endif
            guard let index = Mirror(reflecting: paint).children.first(where: { $0.label == "index" })?.value as? WorkerInkIndex,
                  let spans = Mirror(reflecting: index).children.first(where: { $0.label == "spans" })?.value as? UnsafeMutablePointer<InkSpan> else {
                box.error = "test-only private numeric index inspection unavailable"; return
            }
            box.actual = (0..<index.count).map { [spans[$0].top,spans[$0].bottom] }
            box.coincidentSpans = box.actual.count - Set(box.actual.map { "\($0[0])/\($0[1])" }).count
            // Every endpoint, its immediate neighbors, and gaps/whole-range query.
            let points = Array(Set(box.expected.flatMap { $0.flatMap { [$0.nextDown,$0,$0.nextUp] } })).sorted()
            var ranges = points.map { [$0,$0] }
            for i in 1..<points.count { ranges.append([points[i-1],points[i]]) }
            ranges.append([-10000,100000])
            for range in ranges {
                index.query(top: range[0],bottom: range[1]) { ids,_ in box.queries.append(Array(ids)) }
                box.expectedQueries.append(box.expected.enumerated().compactMap { i,s in
                    s[0] <= range[1] && s[1] >= range[0] ? Int32(i) : nil
                })
            }
            let intrinsic = RegionWorkerLayout.shape(samples[1],width: .infinity,retainHits: false)
            box.intrinsicLineCount = intrinsic.lines.count
            box.intrinsicMetadataCount = intrinsic.metadata.lines.count
            let emptyRequest = RegionRasterRequest(serial: 2,publication: 2,generation: 1,
                rows: [RegionPaintRow(artifact: 9,box: CGRect(x: 0,y: 0,width: .infinity,height: intrinsic.metadata.height))],
                scroll: .zero,size: CGSize(width: 160,height: 100),scale: 1,profile: profile,
                format: CGImageAlphaInfo.premultipliedLast.rawValue,background: [1,1,1,1],selectionColor: [0,0,0,0])
            let emptyPaint = try RegionPaintIndex(request: emptyRequest,lookup: { _ in intrinsic },account: InkAccount())
            box.intrinsicIndexCount = (Mirror(reflecting: emptyPaint).children.first(where: { $0.label == "index" })?.value as? WorkerInkIndex)?.count ?? -1
            withExtendedLifetime([paint,emptyPaint]) {}
        } catch { box.error = String(describing: error) }
    }
    done.wait()
    return box
}

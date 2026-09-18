import Foundation
// Actual ordinary TextEngine is the independent geometry oracle.
import XCTest
import CoreText
@testable import ExactKit

@MainActor final class RegionWorkerTests: XCTestCase {
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
                    box.value = RegionWorkerLayout.shape(source, width: width).metadata
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
        for scale in [1, 2] {
            for scroll: CGFloat in [0, 0.375, 27.25, 103.875] {
                let width = 400 * scale, height = 100 * scale
                let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                    bytesPerRow: width * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
                ctx.setFillColor(CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(), components: [1,1,1,1])!)
                ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
                ctx.translateBy(x: 0, y: CGFloat(height)); ctx.scaleBy(x: CGFloat(scale), y: -CGFloat(scale))
                TextEngine.draw(ordinary, spec: spec, in: CGRect(x: 7.25, y: 0.125 - scroll, width: 320, height: ordinary.height), context: ctx)
                ctx.flush()
                let expected = Data(bytes: ctx.data!, count: width * height * 4)
                let request = RegionRasterRequest(serial: 1, publication: 1, generation: 1,
                    rows: [RegionPaintRow(artifact: 7, box: CGRect(x: 7.25, y: 0.125, width: 320, height: ordinary.height))],
                    scroll: CGPoint(x: 0, y: scroll), size: CGSize(width: 400, height: 100), scale: scale,
                    profile: profile, format: CGImageAlphaInfo.premultipliedLast.rawValue, background: [1,1,1,1], selectionColor: [0.2,0.4,0.8,0.45])
                let box = RegionTestBox(), done = DispatchSemaphore(value: 0)
                DispatchQueue(label: "region-pixel-test").async {
                    do {
                        let layout = RegionWorkerLayout.shape(source, width: 320)
                        let index = try RegionPaintIndex(request: request, layouts: [7: layout], account: InkAccount())
                        box.raster = try index.render(request, account: RegionPixelAccount())
                    } catch { box.error = String(describing: error) }
                    done.signal()
                }
                done.wait()
                XCTAssertEqual(box.error, "")
                let image = box.raster!.image()!
                XCTAssertEqual(image.dataProvider!.data! as Data, expected)
                XCTAssertTrue(box.raster!.accepts(image, size: request.size, scale: scale, profile: profile))
                XCTAssertFalse(box.raster!.accepts(image, size: request.size, scale: scale == 1 ? 2 : 1, profile: profile))
                XCTAssertFalse(box.raster!.accepts(image, size: CGSize(width: 401,height: 100), scale: scale, profile: profile))

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
        let first = capture("abc"), changed = capture("cba")
        XCTAssertEqual(first.sourceSHA256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        XCTAssertEqual(first.utf16Count, changed.utf16Count)
        XCTAssertFalse(first.sourceSHA256 == changed.sourceSHA256)
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
}
// Test-only handoff: writer completes before semaphore establishes reader access.
private final class RegionTestBox: @unchecked Sendable {
    var value: RegionParagraph?
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

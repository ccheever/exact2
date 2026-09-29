// Raster workers beside a session's two text engines (LLP 1072 §8.1): the
// painter on main, the measurer on the owner thread. A worker's job holds
// only plain line geometry and its own copy of the source; neither engine's
// caches (shapes, paragraphs, residency) are touched off its own thread.
// Run on both hosts (`swift test`; on iOS, `build.mjs --test --ios` picks
// up *IOSTests); under Thread Sanitizer it is the race check for that rule.
import XCTest
import CoreText
#if os(macOS)
import IOSurface
#endif
@testable import ExactKit

final class TextRasterRaceIOSTests: XCTestCase {
    private func run(_ text: String) -> Run {
        Run(text: text, size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
    }

    private func bytes(_ r: TextRasterImage?) -> Data {
        guard let r else { return Data() }
        #if os(iOS)
        return (r.image.dataProvider?.data as Data?) ?? Data()
        #else
        r.surface.lock(options: .readOnly, seed: nil)
        defer { r.surface.unlock(options: .readOnly, seed: nil) }
        return Data(bytes: r.surface.baseAddress, count: r.surface.allocationSize)
        #endif
    }

    /// Workers render clamped and unclamped paragraphs from their geometry
    /// while main lays the same paragraphs out again at other widths, asks
    /// their geometry, paints them and lets its caches go, and the owner
    /// measures them too. Every worker's pixels match what main made of the
    /// same job first.
    func testRasterWorkersRunBesideBothEngines() throws {
        let engine = TextEngine.pair(resolve: { _ in nil })
        let measurer = engine.measuring
        let texts = [
            "Maybe family sounds draft later scroll deadline picnic thanks soon a meeting at the station",
            "日本語のテキストと English words mixed together until the line has to wrap twice or more",
            "Short",
            "العربية שלום and Latin, then a long tail that the clamp will cut off with its ellipsis at last",
        ]
        var jobs: [(TextRasterJob, Data)] = []
        for (i, text) in texts.enumerated() {
            for clamp in [0, 1, 2, 3] {
                let spec = Spec(runs: [run(text)], align: i % 3, lineClamp: clamp, color: [30, 60, 90, 255])
                let width: CGFloat = 140 + CGFloat(i * 20)
                let p = engine.paragraph(spec, width: width)
                let geometry = try XCTUnwrap(engine.measuredBreaks(spec, width: width))
                // As `TextRasterizer.ensure` makes it: the shape's source, copied.
                let source = (p.shape?.attributed ?? engine.attributed(spec)).copy() as! NSAttributedString
                let box = CGRect(x: 0, y: 0, width: width, height: p.height)
                let job = TextRasterJob(source: source, ranges: geometry.ranges, baselines: geometry.baselines,
                                        flush: i % 3 == 1 ? 0.5 : i % 3 == 2 ? 1 : 0, box: box, size: box.size,
                                        scale: 2, crop: true, clamped: geometry.clamped)
                jobs.append((job, bytes(job.render())))
            }
        }
        let lock = NSLock()
        var mismatches = 0, rendered = 0
        let done = DispatchGroup()
        done.enter()
        DispatchQueue.global(qos: .userInitiated).async {
            DispatchQueue.concurrentPerform(iterations: 8) { worker in
                for round in 0..<200 {
                    let (job, expected) = jobs[(worker * 7 + round) % jobs.count]
                    let got = self.bytes(job.render())
                    lock.lock(); rendered += 1; if got != expected { mismatches += 1 }; lock.unlock()
                }
            }
            done.leave()
        }
        // The owner: the measurer's own work on the same text meanwhile.
        let measured = expectation(description: "the owner measured")
        Owner.shared.post {
            for round in 0..<60 {
                for (i, text) in texts.enumerated() {
                    let spec = Spec(runs: [self.run(text)], align: i % 3, lineClamp: round % 4, color: [0, 0, 0, 255])
                    _ = measurer.paragraph(spec, width: CGFloat(100 + (round * 17 + i * 13) % 200))
                }
                if round % 9 == 4 { measurer.dropColdShaped() }
            }
            DispatchQueue.main.async { measured.fulfill() }
        }
        // Main: the painter's own work on the same paragraphs meanwhile.
        let context = CGContext(data: nil, width: 400, height: 200, bitsPerComponent: 8, bytesPerRow: 0,
                                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        var turns = 0
        while done.wait(timeout: .now()) != .success {
            let text = texts[turns % texts.count]
            let spec = Spec(runs: [run(text)], align: turns % 3, lineClamp: turns % 4, color: [30, 60, 90, 255])
            let width = CGFloat(100 + (turns * 13) % 200)
            let p = engine.paragraph(spec, width: width)
            _ = engine.measuredBreaks(spec, width: width)
            _ = engine.attributed(spec)
            TextEngine.draw(p, spec: spec, in: CGRect(x: 0, y: 0, width: width, height: p.height), context: context)
            if turns % 16 == 15 { engine.dropColdShaped() }
            turns += 1
        }
        wait(for: [measured], timeout: 60)
        Owner.shared.sync {}
        XCTAssertEqual(rendered, 8 * 200)
        XCTAssertEqual(mismatches, 0, "a worker's pixels differ from main's for the same job")
        XCTAssertGreaterThan(turns, 0)
    }
}

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class OneColorCoverageIOSTests: XCTestCase {
    func testRasterJobsAlphaOnlyImageMasksTheTintWithItsOriginalOpacity() throws {
        let engine = TextEngine(resolve: { _ in nil })
        let spec = Spec(runs: [Run(text: "Ink", size: 32, weight: 700, family: 0, italic: false, lineHeight: 38, letterSpacing: 0)],
                        align: 0, lineClamp: 0, color: [255, 0, 0, 128])
        let paragraph = engine.paragraph(spec, width: 160)
        let geometry = try XCTUnwrap(engine.measuredBreaks(spec, width: 160))
        let box = CGRect(x: 8, y: 8, width: 160, height: paragraph.height)
        let size = CGSize(width: 180, height: paragraph.height + 16)
        let job = TextRasterJob(source: engine.attributed(spec), ranges: geometry.ranges,
            baselines: geometry.baselines, flush: 0, box: box, size: size, scale: 1, tints: true,
            measuredGlyphs: try XCTUnwrap(MeasuredGlyphs(paragraph, spec: spec, width: 160)),
            measuredColor: TextEngine.color(spec.color).cgColor)
        let result = try XCTUnwrap(job.render())
        XCTAssertTrue(result.image.isMask, "Core Graphics publishes the alpha-only bitmap as a coverage mask")
        XCTAssertEqual(result.image.bitsPerPixel, 8)
        let tint = try XCTUnwrap(result.tint)
        XCTAssertEqual(tint.alpha, 1, "the colour's opacity is carried by coverage")
        let ink = TintInk()
        ink.show(result.image, tint: tint, frame: result.frame, scale: 1)
        let width = Int(size.width), height = Int(size.height.rounded(.up))
        let output = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
            bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ink.render(in: output)
        let bytes = try XCTUnwrap(output.data).assumingMemoryBound(to: UInt8.self)
        XCTAssertEqual(bytes[3], 0, "an uncovered corner stays clear")
        let alpha = stride(from: 3, to: width * height * 4, by: 4).map { bytes[$0] }
        XCTAssertGreaterThan(alpha.filter { $0 > 16 }.count, 50, "the alpha-only glyphs paint")
        let peak = try XCTUnwrap(alpha.indices.max(by: { alpha[$0] < alpha[$1] }))
        XCTAssertLessThanOrEqual(abs(Int(alpha[peak]) - 128), 1)
        XCTAssertLessThanOrEqual(abs(Int(bytes[peak * 4]) - Int(alpha[peak])), 1, "covered pixels carry the premultiplied red tint")
        XCTAssertEqual(bytes[peak * 4 + 1], 0)
        XCTAssertEqual(bytes[peak * 4 + 2], 0)
    }
}
#endif

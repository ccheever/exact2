import XCTest
import CoreGraphics
@testable import ExactKit
@testable import ExactSurfaces

/// LLP 1100 D12a: a `display-p3` canvas's list bytes are Display P3, and its
/// bitmap keeps what sRGB can't hold; a `float16` one is half floats.
final class Canvas2DSpaceTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    private func list(_ records: [(Canvas2DOp, [Double])]) -> Data {
        var d = Data()
        func u32(_ v: UInt32) { withUnsafeBytes(of: v.littleEndian) { d.append(contentsOf: $0) } }
        func f64(_ v: Double) { withUnsafeBytes(of: v.bitPattern.littleEndian) { d.append(contentsOf: $0) } }
        u32(0x4432_4345); u32(1)
        for (op, n) in records { u32(op.rawValue); u32(UInt32(n.count)); n.forEach(f64) }
        return d
    }

    /// The bitmap's centre pixel in extended linear sRGB.
    private func centre(_ image: CGImage) throws -> [Double] {
        let space = try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedLinearSRGB))
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 1, height: 1, bitsPerComponent: 32, bytesPerRow: 16, space: space,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder32Little.rawValue))
        ctx.interpolationQuality = .none
        ctx.draw(image, in: CGRect(x: -CGFloat(image.width / 2), y: -CGFloat(image.height / 2), width: CGFloat(image.width), height: CGFloat(image.height)))
        let px = try XCTUnwrap(ctx.data).assumingMemoryBound(to: Float.self)
        return (0..<3).map { Double(px[$0]) }
    }

    private func paint(_ space: Canvas2DSpace, _ records: [(Canvas2DOp, [Double])]) throws -> CGImage {
        let r = Canvas2DReplayer(width: 4, height: 4, scale: 1, lifetime: 1, generation: 0, space: space)
        XCTAssertTrue(r.apply(list(records)))
        return try XCTUnwrap(r.image())
    }

    func testADisplayP3CanvasesColoursAndPixelsAreP3() throws {
        let red: [(Canvas2DOp, [Double])] = [(.fillColor, [255, 0, 0, 1]), (.fillRect, [0, 0, 4, 4])]
        let p3 = try centre(paint(Canvas2DSpace(p3: true), red))
        XCTAssertGreaterThan(p3[0], 1.05, "P3 red is redder than sRGB's: \(p3)")
        XCTAssertLessThan(p3[1], -0.01, "and outside sRGB: \(p3)")
        let srgb = try centre(paint(.srgb8, red))
        XCTAssertEqual(srgb[0], 1, accuracy: 0.01)
        // sRGB red recorded as P3 bytes (234 51 35, the recorders' numbers) is sRGB red.
        let same = try centre(paint(Canvas2DSpace(p3: true), [(.fillColor, [234, 51, 35, 1]), (.fillRect, [0, 0, 4, 4])]))
        for c in 0..<3 { XCTAssertEqual(same[c], [1, 0, 0][c], accuracy: 0.01, "\(same)") }
        let put = try centre(paint(Canvas2DSpace(p3: true), [(.putImageData, [0, 0, 4, 4] + Array(repeating: Double(0xFF00_00FF), count: 16))]))
        XCTAssertGreaterThan(put[0], 1.05, "putImageData's bytes are P3 too: \(put)")
    }

    func testAFloat16CanvasIsHalfFloats() throws {
        let image = try paint(Canvas2DSpace(p3: true, float16: true), [(.fillColor, [255, 0, 0, 1]), (.fillRect, [0, 0, 4, 4])])
        XCTAssertEqual(image.bitsPerComponent, 16)
        XCTAssertTrue(image.bitmapInfo.contains(.floatComponents))
        XCTAssertGreaterThan(try centre(image)[0], 1.05)
    }
}

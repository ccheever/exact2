import CoreGraphics
import IOSurface
import Metal
import XCTest
@testable import ExactKit

/// A live filter picture's Metal chain (LLP 1055.000 D14) against the Core
/// Image chain, which the SVG smoke holds to Chrome: F3's CSS chain over a
/// scene of a card, a disc and a square, the same pixels to a fraction of a
/// level.
class SvgFilterMetalTests: XCTestCase {
    func testTheCSSChainMatchesCoreImage() throws {
        guard let fm = SvgFilterMetal.shared, let queue = SvgFilterGPU.metal?.queue else { throw XCTSkip("no Metal") }
        let (w, h) = (240, 180)
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: space,
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.setFillColor(CGColor(srgbRed: 0.31, green: 0.27, blue: 0.9, alpha: 1)); ctx.fill(CGRect(x: 30, y: 30, width: 180, height: 120))
        ctx.setFillColor(CGColor(srgbRed: 0.99, green: 0.88, blue: 0.28, alpha: 1)); ctx.fillEllipse(in: CGRect(x: 80, y: 50, width: 70, height: 70))
        ctx.setFillColor(CGColor(srgbRed: 0.13, green: 0.83, blue: 0.93, alpha: 0.8)); ctx.fill(CGRect(x: 150, y: 100, width: 40, height: 40))
        let image = ctx.makeImage()!
        let sub: [Float] = [0, 0, Float(w), Float(h)]
        let saturate: [Float] = [0.2126 + 0.7874 * 1.8, 0.7152 - 0.7152 * 1.8, 0.0722 - 0.0722 * 1.8, 0, 0,
                                 0.2126 - 0.2126 * 1.8, 0.7152 + 0.2848 * 1.8, 0.0722 - 0.0722 * 1.8, 0, 0,
                                 0.2126 - 0.2126 * 1.8, 0.7152 - 0.7152 * 1.8, 0.0722 + 0.9278 * 1.8, 0, 0,
                                 0, 0, 0, 1, 0]
        let turn: [Float] = [-0.574, 1.43, 0.144, 0, 0, 0.426, 0.43, 0.144, 0, 0, -0.574, 0.43, 1.144, 0, 0, 0, 0, 0, 1, 0]
        var program: [Float] = sub + [4]
        program += [0, -1, -3] + sub + [0, 2, 2]
        program += [6, 0, -3] + sub + [0, 6, 6, 0, 10, 0, 0, 0, 0.45]
        program += [5, 1, -3] + sub + [0] + saturate
        program += [5, 2, -3] + sub + [0] + turn
        let steps = try XCTUnwrap(SvgFilterMetal.steps(program, scale: 1))
        XCTAssertEqual(steps.count, 4)
        let ci = try XCTUnwrap(SvgFilterGPU.run(program, source: image, origin: .zero, scale: CGSize(width: 1, height: 1)))
        // The Metal chain, from a texture of the same pixels, into a surface.
        let loader = try XCTUnwrap(MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: w, height: h, mipmapped: false) as MTLTextureDescriptor?)
        loader.usage = [.shaderRead]
        let src = try XCTUnwrap(fm.device.makeTexture(descriptor: loader))
        src.replace(region: MTLRegionMake2D(0, 0, w, h), mipmapLevel: 0, withBytes: ctx.data!, bytesPerRow: w * 4)
        let surface = try XCTUnwrap(IOSurface(properties: [.width: w, .height: h, .bytesPerElement: 4, .pixelFormat: 0x4247_5241]))
        let cb = try XCTUnwrap(queue.makeCommandBuffer())
        XCTAssertTrue(fm.encode(steps, source: src, into: surface, on: cb))
        cb.commit(); cb.waitUntilCompleted()
        // Both as premultiplied RGBA bytes.
        let out = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: space,
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        out.draw(ci, in: CGRect(x: 0, y: 0, width: w, height: h))
        let a = out.data!.assumingMemoryBound(to: UInt8.self)
        surface.lock(options: .readOnly, seed: nil)
        defer { surface.unlock(options: .readOnly, seed: nil) }
        let b = surface.baseAddress.assumingMemoryBound(to: UInt8.self)
        var sum = 0.0
        for y in 0..<h { for x in 0..<w {
            let i = (y * w + x) * 4, j = y * surface.bytesPerRow + x * 4
            // BGRA against RGBA.
            sum += Double(abs(Int(a[i]) - Int(b[j + 2]))) + Double(abs(Int(a[i + 1]) - Int(b[j + 1])))
                + Double(abs(Int(a[i + 2]) - Int(b[j]))) + Double(abs(Int(a[i + 3]) - Int(b[j + 3])))
        } }
        let mean = sum / Double(w * h * 4)
        XCTAssertLessThan(mean, 1.5, "mean |Δ| \(mean)/255")
    }
}

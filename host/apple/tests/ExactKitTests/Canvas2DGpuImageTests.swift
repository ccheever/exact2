import XCTest
import CoreGraphics
import CoreText
@testable import ExactSurfaces

final class Canvas2DGpuImageTests: XCTestCase {
    private final class Images: Canvas2DEnv {
        var current: CGImage?
        func canvasFont(_ font: Canvas2DFont) -> CTFont? { nil }
        func canvasImage(_ src: String) -> CGImage? { current }
    }

    func testProductionCallbackBoundsDistinctDecodedImages() throws {
        Canvas2DGpuPixels.shared.trim()
        defer { Canvas2DGpuPixels.shared.trim() }
        let env = Images(), callbacks = Canvas2DGpuCallbacks(env: env, p3: false)
        let context = Unmanaged.passUnretained(callbacks).toOpaque()
        for i in 0..<400 {
            try autoreleasepool {
                let c = try XCTUnwrap(CGContext(data: nil, width: 512, height: 512, bitsPerComponent: 8, bytesPerRow: 2048,
                    space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
                c.setFillColor(CGColor(red: CGFloat(i % 251) / 250, green: 0, blue: 0, alpha: 1))
                c.fill(CGRect(x: 0, y: 0, width: 512, height: 512))
                env.current = try XCTUnwrap(c.makeImage())
                var w: UInt32 = 0, h: UInt32 = 0
                var identity: UInt64 = 0
                let bytes = Array("review-\(i)".utf8)
                let pixels = bytes.withUnsafeBufferPointer { Canvas2DGpuCallbacks.image(context, $0.baseAddress, $0.count, &w, &h, &identity) }
                XCTAssertNotNil(pixels)
                XCTAssertEqual(w, 512); XCTAssertEqual(h, 512)
            }
        }
        let retained = Canvas2DGpuPixels.shared.retainedBytes
        print("D3 production images=400 host_retained_bytes=\(retained)")
        XCTAssertLessThanOrEqual(retained, 32 << 20)
    }
    func testReplacementAndTrimKeepOnlyTheCallbackLeaseAlive() throws {
        let cache = Canvas2DGpuPixels.shared
        cache.trim(); defer { cache.trim() }
        let env = Images(), callbacks = Canvas2DGpuCallbacks(env: env, p3: false)
        let context = Unmanaged.passUnretained(callbacks).toOpaque()
        func image(_ red: CGFloat) throws -> CGImage {
            let c = try XCTUnwrap(CGContext(data: nil, width: 512, height: 512, bitsPerComponent: 8, bytesPerRow: 2048,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            c.setFillColor(CGColor(red: red, green: 0, blue: 0, alpha: 1)); c.fill(CGRect(x: 0, y: 0, width: 512, height: 512))
            return try XCTUnwrap(c.makeImage())
        }
        func call() -> (UnsafePointer<UInt8>?, UInt64) {
            var w: UInt32 = 0, h: UInt32 = 0, identity: UInt64 = 0
            let pixels = Array("replacement".utf8).withUnsafeBufferPointer {
                Canvas2DGpuCallbacks.image(context, $0.baseAddress, $0.count, &w, &h, &identity)
            }
            return (pixels, identity)
        }
        env.current = try image(1)
        let first = call()
        XCTAssertEqual(call().1, first.1, "a cache hit preserves the upload identity")
        weak var old = cache.rgba("replacement", try XCTUnwrap(env.current), p3: false)
        env.current = try image(0)
        let second = call()
        XCTAssertNotEqual(first.1, second.1, "replacement cannot hit an old Rust upload")
        XCTAssertNil(old, "replacement releases the old allocation and its CGImage")
        weak var leased = cache.rgba("replacement", try XCTUnwrap(env.current), p3: false)
        cache.trim()
        XCTAssertEqual(cache.retainedBytes, 0)
        XCTAssertNotNil(leased, "the callback owns its result until its next call")
        XCTAssertEqual(try XCTUnwrap(second.0)[3], 255, "trim does not invalidate a concurrent replay's read")
        let third = call()
        XCTAssertNotEqual(second.1, third.1, "trim never recycles an upload identity")
        XCTAssertNil(leased)
        cache.trim(); env.current = nil
        _ = call()
        XCTAssertEqual(cache.retainedBytes, 0)
    }

}

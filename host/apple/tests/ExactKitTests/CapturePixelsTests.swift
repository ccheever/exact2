import XCTest
import CoreGraphics

final class CapturePixelsTests: XCTestCase {
    static func clearedContext(width: Int, height: Int) throws -> CGContext {
        let context = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.clear(CGRect(x: 0, y: 0, width: width, height: height))
        return context
    }

    func testPaintRequiresAlphaAndOpaqueRequiresFullAlpha() {
        let rgba: [UInt8] = [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 255, 255, 255, 255, 255]
        XCTAssertFalse(CapturePixels.painted(rgba, at: 0))
        XCTAssertTrue(CapturePixels.painted(rgba, at: 4))
        XCTAssertTrue(CapturePixels.painted(rgba, at: 8))
        XCTAssertTrue(CapturePixels.painted(rgba, at: 12))
        XCTAssertFalse(CapturePixels.opaque(rgba, at: 0))
        XCTAssertFalse(CapturePixels.opaque(rgba, at: 4))
        XCTAssertTrue(CapturePixels.opaque(rgba, at: 8))
    }
}

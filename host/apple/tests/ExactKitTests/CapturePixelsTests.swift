import CoreGraphics
import XCTest
#if os(macOS)
import AppKit
#endif

final class CapturePixelsTests: XCTestCase {
    static func clearedContext(width: Int, height: Int) throws -> CGContext {
        let context = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.clear(CGRect(x: 0, y: 0, width: width, height: height))
        return context
    }

    func testPaintRequiresAlphaAndOpaqueRequiresFullAlpha() throws {
        let context = try Self.clearedContext(width: 4, height: 1)
        for (x, gray, alpha) in [(1, CGFloat(0), CGFloat(1) / 255), (2, 0, 1), (3, 1, 1)] {
            context.setFillColor(CGColor(gray: gray, alpha: alpha))
            context.fill(CGRect(x: x, y: 0, width: 1, height: 1))
        }
        let rgba = BorderParity.bytes(context)
        XCTAssertEqual(rgba, [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 255, 255, 255, 255, 255])
        XCTAssertFalse(CapturePixels.painted(rgba, at: 0))
        XCTAssertTrue(CapturePixels.painted(rgba, at: 4))
        XCTAssertTrue(CapturePixels.painted(rgba, at: 8))
        XCTAssertTrue(CapturePixels.painted(rgba, at: 12))
        XCTAssertFalse(CapturePixels.opaque(rgba, at: 0))
        XCTAssertFalse(CapturePixels.opaque(rgba, at: 4))
        XCTAssertTrue(CapturePixels.opaque(rgba, at: 8))
        XCTAssertFalse(CapturePixels.hasPaint([UInt8]()))
        XCTAssertFalse(CapturePixels.hasPaint(Array(rgba.prefix(4))))
        XCTAssertTrue(CapturePixels.hasPaint(rgba[4..<8]))
        XCTAssertTrue(CapturePixels.hasPaint(rgba))
    }

    func testBorderComparisonRejectsTransparentPictures() throws {
        let context = try Self.clearedContext(width: 100, height: 70)
        let clear = BorderParity.bytes(context)
        XCTAssertTrue(clear.allSatisfy { $0 == 0 })
        context.setFillColor(CGColor(gray: 0, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: 100, height: 70))
        let black = BorderParity.bytes(context)
        XCTAssertEqual(black, Array(repeating: [UInt8(0), 0, 0, 255], count: 100 * 70).flatMap { $0 })
        for alpha: UInt8 in [0, 128, 254] {
            var broken = black
            broken[broken.count - 1] = alpha
            for (a, b) in [(black, black), (clear, black), (black, clear), (clear, clear),
                           (broken, black), (black, broken), (broken, broken)] {
                let difference = BorderParity.compare(a, (100, 70, b), at: 0, 0)
                XCTAssertEqual(difference.0, a == black && b == black ? 0 : 255)
                XCTAssertEqual(difference.1, a == black && b == black ? 0 : 100)
            }
        }
    }

    func testCapturedBytesExcludeRowPadding() throws {
        let context = try Self.clearedContext(width: 2, height: 2)
        let clear = CapturePixels.bytes(try XCTUnwrap(context.makeImage()))
        XCTAssertFalse(CapturePixels.hasPaint(clear))
        context.setFillColor(CGColor(gray: 0, alpha: 1))
        context.fill(CGRect(x: 1, y: 1, width: 1, height: 1))
        let painted = CapturePixels.bytes(try XCTUnwrap(context.makeImage()))
        XCTAssertTrue(CapturePixels.hasPaint(painted))
        for pixels in [clear, painted] {
            let bytes = pixels.prefix(8) + Data(repeating: 255, count: 8) + pixels.suffix(8) + Data(repeating: 255, count: 8)
            let provider = try XCTUnwrap(CGDataProvider(data: bytes as CFData))
            let image = try XCTUnwrap(CGImage(width: 2, height: 2, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: 16,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent))
            XCTAssertEqual(CapturePixels.bytes(image), pixels)
        }
    }

    #if os(macOS)
    func testPaintedColorRejectsTransparentBlack() throws {
        let context = try Self.clearedContext(width: 3, height: 1)
        context.setFillColor(CGColor(gray: 0, alpha: 1))
        context.fill(CGRect(x: 1, y: 0, width: 1, height: 1))
        context.setFillColor(CGColor(gray: 0, alpha: 0.5))
        context.fill(CGRect(x: 2, y: 0, width: 1, height: 1))
        let picture = NSBitmapImageRep(cgImage: try XCTUnwrap(context.makeImage()))
        XCTAssertNil(picture.paintedColorAt(x: 0, y: 0))
        XCTAssertEqual(picture.paintedColorAt(x: 1, y: 0)?.alphaComponent, 1)
        XCTAssertEqual(try XCTUnwrap(picture.paintedColorAt(x: 2, y: 0)).alphaComponent, 0.5, accuracy: 1.0 / 255)
    }
    #endif
}

import XCTest
import ImageIO
import CoreGraphics
@testable import ExactKit

final class RasterImageTests: XCTestCase {
    private func fixture(_ width: Int, _ height: Int, type: String = "public.png", orientation: Int = 1) throws -> Data {
        let ctx = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
            bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(CGColor(red: 0.25, green: 0.5, blue: 0.75, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let bytes = NSMutableData()
        let destination = try XCTUnwrap(CGImageDestinationCreateWithData(bytes, type as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try XCTUnwrap(ctx.makeImage()), [kCGImagePropertyOrientation: orientation] as CFDictionary)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        return bytes as Data
    }
    func testPlanReconstructedFromAdmittedDimensionsIsIdentical() throws {
        for orientation in [1, 6] {
            let metadata = try RasterMetadata.validated(width: 1448, height: 1086,
                orientation: orientation, encodedBytes: 100, headerBytes: 100)
            for pixel in [1, 64, 100, 364, 365, 1024] {
                let plan = try RasterDecodePlan(metadata: metadata, maxPixel: pixel)
                let rebuilt = try RasterDecodePlan(metadata: metadata, maxPixel: max(plan.width, plan.height))
                XCTAssertEqual(max(plan.width, plan.height), pixel)
                XCTAssertEqual(rebuilt.width, plan.width)
                XCTAssertEqual(rebuilt.height, plan.height)
                XCTAssertEqual(rebuilt.peakBytes, plan.peakBytes)
            }
        }
    }
    func testReducedRasterKeepsOriginalNaturalDimensionsAndFit() throws {
        let bytes = try fixture(4000, 2000)
        let metadata = try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count)
        let plan = try RasterDecodePlan(metadata: metadata, maxPixel: 100)
        let image = try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: TestRasterCharge())
        XCTAssertEqual(image.image.width, 100); XCTAssertEqual(image.image.height, 50)
        XCTAssertEqual(image.naturalSize, CGSize(width: 4000, height: 2000))
        XCTAssertEqual(image.residentBytes, image.image.bytesPerRow * image.image.height)
        let box = CGRect(x: 0, y: 0, width: 200, height: 200)
        XCTAssertEqual(RasterGeometry.rect(natural: image.naturalSize, content: box, fit: "none").size, image.naturalSize)
        XCTAssertEqual(RasterGeometry.rect(natural: image.naturalSize, content: box, fit: "scale-down").size, CGSize(width: 200, height: 100))
        XCTAssertEqual(RasterGeometry.rect(natural: image.naturalSize, content: box, fit: "cover").size, CGSize(width: 400, height: 200))
    }
    /// A tint draws the bitmap as a template: every pixel takes the tint at
    /// the pixel's own alpha; no tint draws the pixels themselves.
    func testATintDrawsTheBitmapsAlphaInTheTint() throws {
        let space = CGColorSpace(name: CGColorSpace.sRGB)!, info = CGImageAlphaInfo.premultipliedLast.rawValue
        let art = try XCTUnwrap(CGContext(data: nil, width: 3, height: 1, bitsPerComponent: 8, bytesPerRow: 12, space: space, bitmapInfo: info))
        art.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)); art.fill(CGRect(x: 0, y: 0, width: 1, height: 1))
        art.setFillColor(CGColor(srgbRed: 0, green: 1, blue: 0, alpha: 0.5)); art.fill(CGRect(x: 1, y: 0, width: 1, height: 1))
        let image = try XCTUnwrap(art.makeImage())
        func paint(_ tint: CGColor?) throws -> [UInt8] {
            let ctx = try XCTUnwrap(CGContext(data: nil, width: 3, height: 1, bitsPerComponent: 8, bytesPerRow: 12, space: space, bitmapInfo: info))
            RasterGeometry.draw(ctx, image, in: CGRect(x: 0, y: 0, width: 3, height: 1), tint: tint)
            let bytes = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
            return Array(UnsafeBufferPointer(start: bytes, count: 12))
        }
        XCTAssertEqual(try paint(CGColor(srgbRed: 0, green: 0, blue: 1, alpha: 1)), [0, 0, 255, 255, 0, 0, 128, 128, 0, 0, 0, 0])
        XCTAssertEqual(try paint(nil), [255, 0, 0, 255, 0, 128, 0, 128, 0, 0, 0, 0])
    }
    func testOrientationAndSupportedFirstFrameFormats() throws {
        for type in ["public.png", "public.jpeg", "com.compuserve.gif", "public.tiff", "com.microsoft.bmp"] {
            let bytes = try fixture(120, 60, type: type)
            let metadata = try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count)
            let image = try RasterImage.decode(bytes, metadata: metadata,
                plan: RasterDecodePlan(metadata: metadata, maxPixel: 30), charge: TestRasterCharge())
            XCTAssertEqual(image.naturalSize, CGSize(width: 120, height: 60))
            XCTAssertEqual(image.image.width, 30); XCTAssertEqual(image.image.height, 15)
        }
        let bytes = try fixture(120, 60, type: "public.jpeg", orientation: 6)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        let image = try RasterImage.decode(bytes, metadata: metadata,
            plan: RasterDecodePlan(metadata: metadata, maxPixel: 30), charge: TestRasterCharge())
        XCTAssertEqual(metadata.sourceWidth, 120); XCTAssertEqual(metadata.sourceHeight, 60)
        XCTAssertEqual(image.naturalSize, CGSize(width: 60, height: 120))
        XCTAssertEqual(image.image.width, 15); XCTAssertEqual(image.image.height, 30)
    }
    func testInputLimitsAndCheckedPlanningRejectBeforePixels() throws {
        let bytes = try fixture(40, 20)
        XCTAssertThrowsError(try RasterMetadata.read(prefix: bytes, encodedBytes: RasterMetadata.encodedLimit + 1))
        XCTAssertThrowsError(try RasterMetadata.read(prefix: Data(repeating: 0, count: RasterMetadata.headerLimit + 1), encodedBytes: RasterMetadata.headerLimit + 1))
        XCTAssertThrowsError(try RasterMetadata.read(prefix: Data(bytes.prefix(4)), encodedBytes: bytes.count))
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        XCTAssertThrowsError(try RasterDecodePlan(metadata: metadata, maxPixel: 0))
        XCTAssertThrowsError(try RasterMetadata.validated(width: 100000, height: 100000, orientation: 1, encodedBytes: 20, headerBytes: 20))
    }
    func testProviderRetainsChargeUntilLastImageAliasDropsOnWorker() throws {
        let bytes = try fixture(120, 60)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        let counter = RasterDropCounter()
        let alias: RasterImageAlias = try autoreleasepool {
            let image = try RasterImage.decode(bytes, metadata: metadata,
                plan: RasterDecodePlan(metadata: metadata, maxPixel: 30), charge: TestRasterCharge(counter))
            return RasterImageAlias(image.image)
        }
        XCTAssertEqual(counter.value, 0)
        let done = expectation(description: "worker drops final provider alias")
        DispatchQueue.global().async { alias.drop(); done.fulfill() }
        wait(for: [done], timeout: 2)
        XCTAssertEqual(counter.value, 1)
    }
    func testReplacedSourceOfEqualByteCountCannotUseOldAdmission() throws {
        var old = try fixture(4000, 2000)
        var replacement = try fixture(4000, 4000)
        let count = max(old.count, replacement.count)
        old.append(Data(repeating: 0, count: count - old.count))
        replacement.append(Data(repeating: 0, count: count - replacement.count))
        let metadata = try RasterMetadata.read(prefix: old.prefix(RasterMetadata.headerLimit), encodedBytes: count)
        XCTAssertThrowsError(try RasterImage.decode(replacement, metadata: metadata,
            plan: RasterDecodePlan(metadata: metadata, maxPixel: 100), charge: TestRasterCharge()))
    }
}

private final class RasterDropCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    var value: Int { lock.lock(); defer { lock.unlock() }; return count }
    func increment() { lock.lock(); count += 1; lock.unlock() }
}
private final class TestRasterCharge: RasterBackingCharge, @unchecked Sendable {
    let counter: RasterDropCounter?
    init(_ counter: RasterDropCounter? = nil) { self.counter = counter }
    deinit { counter?.increment() }
}
private final class RasterImageAlias: @unchecked Sendable {
    private var image: CGImage?
    init(_ image: CGImage) { self.image = image }
    func drop() { image = nil }
}

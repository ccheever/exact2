import XCTest
import ImageIO
import CoreGraphics
#if os(macOS)
import AppKit
#else
import UIKit
#endif
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
    func testHalfAlphaPNGViewOverWhiteMatchesChrome() throws {
        // Unassociated RGBA(80, 120, 160, 128), encoded independently of ImageIO.
        let source = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMIqFjQAAAEbQHpbOjxOAAAAABJRU5ErkJggg=="
        #if os(macOS)
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 40, height: 40), styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        #else
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 40, height: 40))
        #endif
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 40, height: 40)
        #if os(macOS)
        window.contentView = p.viewport
        #else
        let controller = UIViewController()
        controller.view = p.viewport
        window.rootViewController = controller
        #endif
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": [:], "handlers": []],
            ["op": "create", "id": 2, "kind": "image", "props": [:], "handlers": []],
            ["op": "style", "id": 1, "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 40, "h": 40],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 40, "h": 40],
            ["op": "roots", "ids": [1]]
        ]))
        let root = try XCTUnwrap(p.views[1]), node = try XCTUnwrap(p.views[2])
        let loader = RasterLoader()
        let resolver = AssetResolver(root: URL(fileURLWithPath: "/"))
        defer {
            loader.shutdown(); node.raster = nil
            #if os(macOS)
            window.close()
            #else
            window.isHidden = true
            #endif
        }
        XCTAssertTrue(loader.load(node, source: source, resolver: resolver))
        let deadline = Date().addingTimeInterval(5)
        while node.raster == nil, Date() < deadline {
            loader.reconcile()
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        }
        XCTAssertNotNil(node.raster)
        let capture = try XCTUnwrap(Capture.bitmap(of: root, scale: 1))
        #if os(macOS)
        let image = try XCTUnwrap(capture.cgImage)
        #else
        let image = try XCTUnwrap(capture.context.makeImage())
        #endif
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 40, height: 40, bitsPerComponent: 8,
            bytesPerRow: 160, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: 40, height: 40))
        let pixels = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        let actual = Array(UnsafeBufferPointer(start: pixels + (20 * 40 + 20) * 4, count: 4))
        // Chrome's screenshot of this PNG at 40 CSS pixels over white.
        for (got, want) in zip(actual, [167, 187, 207, 255]) {
            XCTAssertEqual(Int(got), want, accuracy: 1)
        }
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
    func testJPEGThumbnailRoundingDoesNotRequireResampling() throws {
        let bytes = try fixture(1448, 1086, type: "public.jpeg")
        let metadata = try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count)
        let source = try XCTUnwrap(CGImageSourceCreateWithData(bytes as CFData, nil))
        var copies = 0
        let started = CACurrentMediaTime()
        for pixel in [65, 99, 101, 363, 365, 511] {
            let plan = try RasterDecodePlan(metadata: metadata, maxPixel: pixel)
            let thumbnail = try XCTUnwrap(CGImageSourceCreateThumbnailAtIndex(source, 0, RasterImage.thumbnailOptions(plan)))
            let image = try XCTUnwrap(RasterImage.normalized(thumbnail, plan: plan))
            if image !== thumbnail { copies += 1 }
            XCTAssertTrue(image === thumbnail, "keep ImageIO's pixels within the admitted dimensions")
            XCTAssertLessThanOrEqual(image.bytesPerRow * image.height, plan.outputBytes)
        }
        print("QUEUE 670 JPEG thumbnails=6 resamples=\(copies) ms=\((CACurrentMediaTime() - started) * 1000)")
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
    /// A whole file no decoder here reads is its format, which the image's
    /// `error` names (LLP 1011 §4); a prefix too short to size stays the
    /// header limit.
    /// An SVG file is a picture (LLP 1011.001): its header gives the natural
    /// size in CSS pixels, the plan may exceed that size (a 3× screen gets a
    /// 3× drawing), under `fill` the plan is the box's own ratio with the
    /// picture letterboxed inside as the browser does, and the pixels are the
    /// file's shapes at the plan's size, the top row the picture's top.
    func testAnSVGIsAPictureDrawnAtThePlannedSize() throws {
        let text = "<?xml version=\"1.0\"?>\n<!-- a wordmark -->\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"10\" viewBox=\"0 0 20 10\"><rect width=\"10\" height=\"10\" fill=\"#ff0000\"/><script>bad()</script></svg>"
        let bytes = Data(text.utf8)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        XCTAssertTrue(metadata.svg)
        XCTAssertEqual(metadata.naturalSize, CGSize(width: 20, height: 10))
        XCTAssertEqual(metadata.variant, RasterVariant.own8)
        var plan = try RasterDecodePlan(metadata: metadata, maxPixel: 200)
        XCTAssertEqual([plan.width, plan.height], [200, 100], "above the natural size")
        plan.scale = 2
        let image = try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: TestRasterCharge())
        XCTAssertEqual([image.image.width, image.image.height], [200, 100])
        XCTAssertEqual(image.naturalSize, CGSize(width: 20, height: 10))
        let pixels = try rgba(image.image)
        XCTAssertEqual(pixels(10, 50), [255, 0, 0, 255], "the rect fills the left half")
        XCTAssertEqual(pixels(190, 50)[3], 0, "the right half is clear")
        XCTAssertEqual(pixels(10, 2), [255, 0, 0, 255], "the picture's top is the first row")
        // Under `fill` the plan takes the box's ratio and the file letterboxes inside (xMidYMid meet).
        let filled = try RasterDecodePlan(metadata: metadata, maxPixel: 200, aspect: (200, 200))
        XCTAssertEqual([filled.width, filled.height], [200, 200])
        // The worker rebuilds the plan from the work's own size (the box's), as it reconstructs a bitmap's from its longest side.
        let rebuilt = try RasterDecodePlan(metadata: metadata, maxPixel: 200, variant: filled.variant, aspect: (filled.width, filled.height))
        XCTAssertEqual([rebuilt.width, rebuilt.height, rebuilt.peakBytes], [filled.width, filled.height, filled.peakBytes])
        let square = try rgba(try RasterImage.decode(bytes, metadata: metadata, plan: filled, charge: TestRasterCharge()).image)
        XCTAssertEqual(square(50, 100), [255, 0, 0, 255], "the rect, centred vertically")
        XCTAssertEqual(square(50, 10)[3], 0, "the letterbox above it is clear")
        // A viewBox alone is the natural size (the declared deviation); a size alone the default ratio; neither the default.
        for (root, size) in [("viewBox=\"0 0 4 2\"", CGSize(width: 4, height: 2)), ("width=\"1in\" viewBox=\"0 0 4 2\"", CGSize(width: 96, height: 48)), ("", CGSize(width: 300, height: 150))] {
            let bytes = Data("<svg xmlns=\"http://www.w3.org/2000/svg\" \(root)/>".utf8)
            XCTAssertEqual(try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count).naturalSize, size, root)
        }
        // Refused by the parser: the reason, with its line, is the image's error.
        let broken = Data("<svg xmlns=\"http://www.w3.org/2000/svg\">\n<g>\n</svg>".utf8)
        let brokenMetadata = try RasterMetadata.read(prefix: broken, encodedBytes: broken.count)
        XCTAssertThrowsError(try RasterImage.decode(broken, metadata: brokenMetadata, plan: try RasterDecodePlan(metadata: brokenMetadata, maxPixel: 10), charge: TestRasterCharge())) { error in
            XCTAssertEqual(String(describing: error), "not an SVG this host reads: line 3: `</svg>` closes `<g>` from line 2")
        }
        // Bytes decide, not a name: a PNG stays a PNG.
        let png = try fixture(40, 20)
        XCTAssertFalse(try RasterMetadata.read(prefix: png.prefix(RasterMetadata.headerLimit), encodedBytes: png.count).svg)
    }
    private func rgba(_ image: CGImage) throws -> (Int, Int) -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: image.width * image.height * 4)
        let ctx = try XCTUnwrap(CGContext(data: &bytes, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        // Core Graphics' rows run bottom up: row 0 of the picture is the last.
        return { x, y in let at = ((image.height - 1 - y) * image.width + x) * 4; return Array(bytes[at..<at + 4]) }
    }
    func testAWholeFileNoDecoderReadsIsItsFormat() throws {
        for text in ["not an image", "<html><svg/></html>"] {
            let bytes = Data(text.utf8)
            XCTAssertThrowsError(try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)) { error in
                XCTAssertEqual(error as? RasterFailure, .format)
                XCTAssertEqual(String(describing: error), "not an image format this host decodes")
            }
        }
        let bytes = try fixture(40, 20)
        XCTAssertThrowsError(try RasterMetadata.read(prefix: Data(bytes.prefix(4)), encodedBytes: bytes.count)) { error in
            XCTAssertEqual(error as? RasterFailure, .headerLimit)
        }
        XCTAssertEqual(String(describing: RasterHTTPStatus(code: 404)), "HTTP 404")
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

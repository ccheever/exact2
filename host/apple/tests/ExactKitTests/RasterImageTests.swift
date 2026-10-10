import XCTest
import ImageIO
import CoreGraphics
@testable import ExactKit
#if os(iOS)
import UIKit
import WebKit
#endif

final class RasterImageTests: XCTestCase {
    func testSVGImageKeepsNaturalSizeAndPaintsNegativeViewBox() throws {
        let bytes = Data("<svg xmlns='http://www.w3.org/2000/svg' width='110' height='130' viewBox='-5 -100 110 130' fill='#808080'><path d='M-5-100H105V30H-5Z'/></svg>".utf8)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        XCTAssertTrue(metadata.svg)
        XCTAssertEqual(metadata.naturalSize, CGSize(width: 110, height: 130))
        let plan = try RasterDecodePlan(metadata: metadata, maxPixel: 13)
        let image = try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: TestRasterCharge())
        XCTAssertEqual(image.image.width, 11); XCTAssertEqual(image.image.height, 13)
        let enlarged = try RasterDecodePlan(metadata: metadata, maxPixel: 390)
        XCTAssertEqual(enlarged.width, 330); XCTAssertEqual(enlarged.height, 390)
        let largeImage = try RasterImage.decode(bytes, metadata: metadata, plan: enlarged, charge: TestRasterCharge())
        XCTAssertEqual(largeImage.image.width, 330); XCTAssertEqual(largeImage.image.height, 390)
        XCTAssertEqual(largeImage.naturalSize, metadata.naturalSize)
        let context = try XCTUnwrap(CGContext(data: nil, width: 11, height: 13, bitsPerComponent: 8,
            bytesPerRow: 44, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(image.image, in: CGRect(x: 0, y: 0, width: 11, height: 13))
        let pixels = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
        XCTAssertEqual(Array(UnsafeBufferPointer(start: pixels, count: 4)), [128, 128, 128, 255])
        XCTAssertThrowsError(try RasterMetadata.read(prefix: bytes.prefix(40), encodedBytes: bytes.count))
        let external = Data("<svg xmlns='http://www.w3.org/2000/svg'><image href='/etc/passwd'/></svg>".utf8)
        XCTAssertThrowsError(try RasterMetadata.read(prefix: external, encodedBytes: external.count)) { error in
            XCTAssertEqual(String(describing: error), "unsupported SVG image feature")
        }
    }

    func testSVGSourceChangeIsRefusedAndReservationHasNoImageIOStaging() throws {
        let bytes = Data("<svg xmlns='http://www.w3.org/2000/svg' width='5' height='7'/>".utf8)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        let plan = try RasterDecodePlan(metadata: metadata, maxPixel: 70)
        XCTAssertEqual(plan.peakBytes, 5 * plan.outputBytes + 64 * 1024)
        let changed = Data("<svg xmlns='http://www.w3.org/2000/svg' width='6' height='7'/>".utf8)
        XCTAssertThrowsError(try RasterImage.decode(changed, metadata: metadata, plan: plan, charge: TestRasterCharge())) { error in
            XCTAssertEqual(String(describing: error), "actual exceeds reservation")
        }
        let bom = Data([0xef, 0xbb, 0xbf, 32, 10]) + bytes
        XCTAssertTrue(try RasterMetadata.read(prefix: bom, encodedBytes: bom.count).svg)
    }

    func testSVGFractionalNaturalSizeKeepsUniformPixelsAndCeilPadding() throws {
        let bytes = Data("<svg xmlns='http://www.w3.org/2000/svg' width='5.2' height='7'><rect width='2.6' height='7' fill='red'/><rect x='2.6' width='2.6' height='7' fill='blue'/></svg>".utf8)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        XCTAssertEqual(metadata.naturalSize.width, 5.2, accuracy: 0.000001)
        XCTAssertEqual(metadata.naturalSize.height, 7)
        let plan = try RasterDecodePlan(metadata: metadata, maxPixel: 70)
        XCTAssertEqual(plan.width, 52); XCTAssertEqual(plan.height, 70)
        let image = try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: TestRasterCharge())
        XCTAssertEqual(try pixel(image.image, x: 24, y: 35), [0, 0, 255, 255])
        XCTAssertEqual(try pixel(image.image, x: 27, y: 35), [255, 0, 0, 255])
        let reduced = try RasterImage.decode(bytes, metadata: metadata,
            plan: RasterDecodePlan(metadata: metadata, maxPixel: 7), charge: TestRasterCharge())
        let rect = reduced.displayRect(CGRect(origin: .zero, size: metadata.naturalSize))
        XCTAssertEqual(rect.width, 6, accuracy: 0.000001)
        XCTAssertEqual(rect.height, 7, accuracy: 0.000001)
        XCTAssertLessThan(try pixel(reduced.image, x: 5, y: 3)[3], 80)
    }

    func testSVGConcreteFillViewportKeepsAspectAndTopBottomColors() throws {
        let bytes = Data("<svg xmlns='http://www.w3.org/2000/svg' width='100' height='100' viewBox='0 0 100 100'><rect width='100' height='50' fill='red'/><rect y='50' width='100' height='50' fill='blue'/></svg>".utf8)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        let image = try RasterImage.decode(bytes, metadata: metadata,
            plan: RasterDecodePlan(metadata: metadata, maxPixel: 200, svgViewport: CGSize(width: 200, height: 100)), charge: TestRasterCharge())
        XCTAssertEqual(image.image.width, 200); XCTAssertEqual(image.image.height, 100)
        XCTAssertEqual(image.naturalSize, CGSize(width: 100, height: 100))
        XCTAssertEqual(try pixel(image.image, x: 10, y: 25), [0, 0, 0, 0])
        XCTAssertEqual(try pixel(image.image, x: 100, y: 25), [0, 0, 255, 255])
        XCTAssertEqual(try pixel(image.image, x: 100, y: 75), [255, 0, 0, 255])
        let percentage = Data("<svg xmlns='http://www.w3.org/2000/svg' width='100%' height='50%' viewBox='0 0 200 200'/>".utf8)
        XCTAssertEqual(try RasterMetadata.read(prefix: percentage, encodedBytes: percentage.count).naturalSize, CGSize(width: 150, height: 150))
    }

    func testSVGIntrinsicMetadataMatchesRootAttributesAndAspectRatioNone() throws {
        let cases: [(String, CGSize)] = [
            ("<svg width='100' height='100' style='width:10px;height:20px'/>", CGSize(width: 100, height: 100)),
            ("<svg viewBox='0 0 100 100' style='width:10px;height:20px'/>", CGSize(width: 150, height: 150)),
            ("<svg class='art' viewBox='0 0 100 100'><style>.art {width:10px;height:20px}</style></svg>", CGSize(width: 150, height: 150)),
            ("<svg width='80' height='250' viewBox='0 0 200 200' preserveAspectRatio='none'/>", CGSize(width: 80, height: 250)),
        ]
        for (source, expected) in cases {
            let bytes = Data(source.utf8)
            let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
            XCTAssertTrue(metadata.svg)
            XCTAssertEqual(metadata.naturalSize, expected, source)
        }
    }

    func testSVGWithoutIntrinsicRatioHasANamedRefusal() {
        for attrs in ["", "width='100%' height='50%'", "width='80'", "height='100'", "viewBox='0 0 200 200' preserveAspectRatio='none'"] {
            let bytes = Data("<svg xmlns='http://www.w3.org/2000/svg' \(attrs)/>".utf8)
            XCTAssertThrowsError(try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count), attrs) { error in
                XCTAssertEqual(error as? RasterFailure, .unsupportedSvgIntrinsicSize)
                XCTAssertEqual(String(describing: error), "unsupported SVG image intrinsic sizing")
            }
        }
    }

    #if os(iOS)
    func testSVGWithoutRatioWithOnlyCSSWidthUsesBrowserFallbackAndNativeRefusal() throws {
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='100%' height='50%'><rect width='100%' height='100%' fill='red'/></svg>"
        let url = "data:image/svg+xml;base64," + Data(svg.utf8).base64EncodedString()
        let browser = WKWebView(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        var geometry: [Double]?
        let sized = expectation(description: "browser geometry")
        let navigation = SVGImageNavigation {
            browser.evaluateJavaScript("[document.getElementById('art').width, document.getElementById('art').height]") { value, error in
                XCTAssertNil(error)
                geometry = value as? [Double]
                sized.fulfill()
            }
        }
        browser.navigationDelegate = navigation
        browser.loadHTMLString("<meta name='viewport' content='width=device-width'><img id='art' style='width:120px;height:auto' src='\(url)'>", baseURL: nil)
        wait(for: [sized], timeout: 10)
        XCTAssertEqual(geometry, [120, 150], "the fallback 300x150 does not imply a 2:1 intrinsic ratio")

        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("svg-no-ratio-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        try Data(svg.utf8).write(to: root.appendingPathComponent("art.svg"))
        let resolver = AssetResolver(root: root), presenter = Presenter(), loader = RasterLoader()
        let node = NodeView(id: 1, kind: "image", presenter: presenter)
        presenter.views[node.id] = node; presenter.viewport.addSubview(node)
        node.style = ["width": 120]; node.frame = CGRect(x: 0, y: 0, width: 120, height: 0)
        node.loadGeneration = 1
        var intrinsic: [CGSize] = []
        presenter.onIntrinsic = { sizes in intrinsic += sizes.compactMap { $0.1 } }
        defer { loader.shutdown(); node.raster = nil; try? FileManager.default.removeItem(at: root) }
        XCTAssertTrue(loader.load(node, source: "art.svg", resolver: resolver))
        func failure() -> String? { (loader.diagnostics["images"] as? [[String: Any]])?.first?["failure"] as? String }
        let nativeDeadline = Date(timeIntervalSinceNow: 5)
        while failure()?.isEmpty != false && Date() < nativeDeadline {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        }
        XCTAssertEqual(failure(), "unsupported SVG image intrinsic sizing")
        XCTAssertTrue(intrinsic.isEmpty, "no fallback pair may reach the kernel and imply a ratio")
        XCTAssertNil(node.raster)
        XCTAssertEqual(node.frame.size, CGSize(width: 120, height: 0))
        XCTAssertEqual(loader.diagnostics["decoded"] as? Int, 0)
    }
    #endif

    func testSVGClippedGroupsPaintAtLargeDecodeScalesWithinReservation() throws {
        let bytes = Data("<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64' viewBox='0 0 64 64'><defs><clipPath id='c'><circle cx='32' cy='32' r='24'/></clipPath></defs><g clip-path='url(#c)'><rect width='64' height='32' fill='red'/><rect y='32' width='64' height='32' fill='blue'/></g></svg>".utf8)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        for size in [160, 640] {
            let plan = try RasterDecodePlan(metadata: metadata, maxPixel: size)
            XCTAssertEqual(plan.scratchBytes, 4 * plan.outputBytes + 64 * 1024)
            XCTAssertLessThanOrEqual(plan.peakBytes, 32 * 1024 * 1024)
            let image = try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: TestRasterCharge())
            XCTAssertEqual(try pixel(image.image, x: size / 2, y: size / 4), [0, 0, 255, 255])
            XCTAssertEqual(try pixel(image.image, x: size / 2, y: size * 3 / 4), [255, 0, 0, 255])
            XCTAssertEqual(try pixel(image.image, x: size / 40, y: size / 40), [0, 0, 0, 0])
        }
        XCTAssertThrowsError(try RasterDecodePlan(metadata: metadata, maxPixel: 1600)) { error in
            XCTAssertEqual(error as? RasterFailure, .tooLarge)
        }
    }

    func testSVGTextReservesBoundedFontStorageAndPaintsGlyphs() throws {
        let bytes = Data("<svg xmlns='http://www.w3.org/2000/svg' width='80' height='24'><text x='2' y='18' font-family='Arial' font-size='16' fill='red'>Hi</text></svg>".utf8)
        let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
        XCTAssertEqual(metadata.svgFontBytes, 8 * 1024 * 1024)
        let plan = try RasterDecodePlan(metadata: metadata, maxPixel: 80)
        XCTAssertEqual(plan.peakBytes, 5 * plan.outputBytes + 64 * 1024 + metadata.svgFontBytes)
        let image = try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: TestRasterCharge())
        let pixels = try XCTUnwrap(image.image.dataProvider?.data) as Data
        XCTAssertTrue(pixels.enumerated().contains { $0.offset % 4 == 3 && $0.element > 0 }, "CoreText-resolved glyphs must paint")
        let plain = Data("<svg xmlns='http://www.w3.org/2000/svg' width='80' height='24'><rect width='80' height='24'/></svg>".utf8)
        let shapeMetadata = try RasterMetadata.read(prefix: plain, encodedBytes: plain.count)
        XCTAssertEqual(shapeMetadata.svgFontBytes, 0)
        let shapePlan = try RasterDecodePlan(metadata: shapeMetadata, maxPixel: 80)
        XCTAssertEqual(shapePlan.peakBytes, 5 * shapePlan.outputBytes + 64 * 1024)
    }

    func testSVGReplacementCannotAddUnreservedFontStaging() throws {
        var original = Data("<svg xmlns='http://www.w3.org/2000/svg' width='80' height='24'><rect width='80' height='24'/></svg>".utf8)
        var replacement = Data("<svg xmlns='http://www.w3.org/2000/svg' width='80' height='24'><text x='2' y='18'>Hi</text></svg>".utf8)
        let count = max(original.count, replacement.count)
        original.append(Data(repeating: 32, count: count - original.count))
        replacement.append(Data(repeating: 32, count: count - replacement.count))
        let metadata = try RasterMetadata.read(prefix: original, encodedBytes: count)
        let plan = try RasterDecodePlan(metadata: metadata, maxPixel: 80)
        XCTAssertThrowsError(try RasterImage.decode(replacement, metadata: metadata, plan: plan, charge: TestRasterCharge())) { error in
            XCTAssertEqual(error as? RasterFailure, .reservation)
        }
    }

    private func pixel(_ image: CGImage, x: Int, y: Int) throws -> [UInt8] {
        let data = try XCTUnwrap(image.dataProvider?.data) as Data
        let offset = y * image.bytesPerRow + x * 4
        return Array(data[offset..<offset + 4])
    }

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
    /// A whole file no decoder here reads is its format, which the image's
    /// `error` names (LLP 1011 §4); a prefix too short to size stays the
    /// header limit.
    func testAWholeFileNoDecoderReadsIsItsFormat() throws {
        for text in ["not an image", "{\"oops\":1}"] {
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

#if os(iOS)
private final class SVGImageNavigation: NSObject, WKNavigationDelegate {
    let finish: () -> Void
    init(finish: @escaping () -> Void) { self.finish = finish }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { finish() }
}
#endif

import XCTest
import ImageIO
import CoreGraphics
import CoreImage
import QuartzCore
@testable import ExactKit

/// LLP 1100 D5, D8: an HDR picture decodes as HDR when its display can
/// show it and `dynamic-range-limit` allows it; its layer asks for that
/// range; anything else is SDR and never marked.
final class HDRImageTests: XCTestCase {
    func testUnknownPQAndHLGHeadroomUsesReferenceDefault() throws {
        guard #available(iOS 18, macOS 15, tvOS 18, *) else { throw XCTSkip("headroom tag needs 18/15") }
        for name in [CGColorSpace.itur_2100_PQ, CGColorSpace.itur_2100_HLG] {
            let space = try XCTUnwrap(CGColorSpace(name: name))
            let data = Data(repeating: 128, count: 4)
            let provider = try XCTUnwrap(CGDataProvider(data: data as CFData))
            let image = try XCTUnwrap(CGImage(width: 1, height: 1, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: 4,
                space: space, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue), provider: provider,
                decode: nil, shouldInterpolate: false, intent: .defaultIntent))
            let unknown = try XCTUnwrap(CGImageCreateCopyWithContentHeadroom(0, image))
            // Some OS versions immediately synthesize the BT.2100 default
            // on a zero-tagged CGImage; exercise an actual reported zero too.
            XCTAssertEqual(RasterImage.headroom(reported: 0, space: space), 1000 / 203, accuracy: 0.001)
            XCTAssertEqual(RasterImage.headroom(reported: 1, space: space), 1)
            XCTAssertEqual(RasterImage.headroom(reported: 0, space: CGColorSpace(name: CGColorSpace.sRGB)), 0)
            XCTAssertEqual(RasterImage.headroom(of: unknown), 1000 / 203, accuracy: 0.001)
            XCTAssertNoThrow(try HDRPlatformProbe.requireHDRHeadroom(0, space: space, file: "untagged BT.2100"))
            XCTAssertThrowsError(try HDRPlatformProbe.requireHDRHeadroom(1, space: space, file: "SDR headroom"))
        }
    }

    private struct Patch: Decodable { let x: Int; let w: Int; let linearSRGB: [Double]?; let source: [Double]? }
    private struct Expectation: Decodable { let patches: [Patch] }
    private static let dir = Bundle.module.resourceURL!.appendingPathComponent("ColorFixtures")

    override func tearDown() { DisplayRange.pinned = nil }

    private func header(_ file: String) throws -> (Data, RasterMetadata) {
        let bytes = try Data(contentsOf: Self.dir.appendingPathComponent(file))
        return (bytes, try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count))
    }
    private func decode(_ file: String, variant: UInt32?) throws -> (RasterMetadata, RasterDecodePlan, RasterImage) {
        let (bytes, metadata) = try header(file)
        var pixel = Int(max(metadata.naturalSize.width, metadata.naturalSize.height))
        var plan = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: variant)
        while plan == nil, pixel > 1 { pixel /= 2; plan = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: variant) }
        let fitted = try XCTUnwrap(plan)
        return (metadata, fitted, try RasterImage.decode(bytes, metadata: metadata, plan: fitted, charge: HDRTestCharge()))
    }

    func testEveryHDRFixtureIsKnownFromItsHeader() throws {
        for file in ["gainmap-iso.jpg", "gainmap-iso.heic", "gainmap-huge.jpg", "gainmap-orient6.heic", "pq.heic", "pq.avif", "hlg.heic", "pq-cicp.png"] {
            XCTAssertTrue(try header(file).1.hdr, "\(file): HDR from the header")
        }
        for file in ["p3-camera.jpg", "srgb-ramp.png", "linear.exr"] {
            XCTAssertFalse(try header(file).1.hdr, "\(file): not HDR")
        }
    }

    /// PQ, decoded as HDR: kept in its own BT.2100 PQ space, each patch at
    /// its cd/m² over BT.2408's 203.
    func testAnHDRDecodeKeepsLightAboveSDRWhite() throws {
        for (file, tolerance) in [("pq.heic", 0.06), ("pq.avif", 0.06), ("pq-cicp.png", 0.04)] {
            let (metadata, _, image) = try decode(file, variant: RasterVariant.hdr)
            XCTAssertTrue(image.isHDR, "\(file): tagged HDR")
            XCTAssertGreaterThan(image.headroom, 3.5, file)
            // 10 bits packed; the 16-bit PNG at 16.
            XCTAssertEqual(image.image.colorSpace?.name, CGColorSpace.itur_2100_PQ, file)
            XCTAssertEqual(image.image.bitsPerComponent, file.hasSuffix(".png") ? 16 : 10, file)
            if #available(macOS 15, iOS 18, tvOS 18, *) { XCTAssertGreaterThan(image.image.contentHeadroom, 3.5, "\(file): the bitmap carries it") }
            // HEIC and AVIF declare their 4000 cd/m² peak (make.swift); PNG
            // can't, and takes PQ's 1000 cd/m² default.
            XCTAssertEqual(image.headroom, file.hasSuffix(".png") ? 4.93 : 19.7, accuracy: 0.05, "\(file): the declared peak")
            let e = try JSONDecoder().decode(Expectation.self, from: Data(contentsOf: Self.dir.appendingPathComponent(file + ".json")))
            var brightest = 0.0
            for p in e.patches {
                guard let want = p.linearSRGB, want.allSatisfy({ $0 > 0.05 }), want.allSatisfy({ $0 < 4.5 }) else { continue }
                let got = try HDRPlatformProbe.sample(image.image, x: p.x + p.w / 2, sourceWidth: metadata.sourceWidth)
                brightest = max(brightest, got.max() ?? 0)
                for c in 0..<3 {
                    XCTAssertEqual(got[c], want[c], accuracy: max(0.02, want[c] * tolerance), "\(file) patch at \(p.x): \(got) vs \(want)")
                }
            }
            XCTAssertGreaterThan(brightest, 1.8, "\(file): light above SDR white survives")
        }
    }

    /// A gain map, decoded as HDR. Its HDR rendition is the encoder's
    /// approximation of the original (an 8-bit base, a reduced 8-bit map), so
    /// the check is its shape, not the original's numbers: white stays near
    /// 1, light above it is recovered, brighter patches stay brighter, and
    /// the headroom is the one it was made with (4), or BT.2100's default
    /// when ImageIO doesn't report the gain map's headroom.
    func testAGainMapRecoversLightAboveSDRWhite() throws {
        for file in ["gainmap-iso.jpg", "gainmap-iso.heic", "gainmap-orient6.heic"] {
            let (metadata, plan, image) = try decode(file, variant: RasterVariant.hdr)
            let probe = try HDRPlatformProbe.rasterThumbnail(Self.dir.appendingPathComponent(file), plan: plan, hdr: metadata.hdr)
            var headroom: Float = 0
            if #available(iOS 18, macOS 15, tvOS 18, *) { headroom = probe.contentHeadroom }
            if headroom == 0, probe.colorSpace.map(CGColorSpaceUsesITUR_2100TF) == true { headroom = 1000 / 203 }
            XCTAssertEqual(image.headroom, headroom, accuracy: 0.001, "\(file): stored headroom follows ImageIO")
            XCTAssertEqual(image.isHDR, headroom > 1, file)
            let w = metadata.sourceWidth / 6
            let reference = try (0..<4).map { try HDRPlatformProbe.sample(probe, x: $0 * w + w / 2, sourceWidth: metadata.sourceWidth, rotated: metadata.orientation == 6) }
            let stored = try (0..<4).map { try HDRPlatformProbe.sample(image.image, x: $0 * w + w / 2, sourceWidth: metadata.sourceWidth, rotated: metadata.orientation == 6) }
            for (got, want) in zip(stored, reference) {
                for c in 0..<3 {
                    XCTAssertEqual(got[c], want[c], accuracy: max(0.02, abs(want[c]) * 0.02), "\(file): normalization preserves ImageIO's neutral pixels")
                }
            }
            if try HDRPlatformProbe.declined("\(file): HDR rendition", {
                if #available(iOS 18, macOS 15, tvOS 18, *) {
                    try HDRPlatformProbe.requireHDRHeadroom(probe.contentHeadroom, space: probe.colorSpace, file: file)
                }
            }) != nil { continue }
            XCTAssertTrue(image.isHDR, file)
            let referenceHeadroom: Float = 1000 / 203
            let expectedHeadroom: Float = abs(headroom - referenceHeadroom) < 0.01 ? referenceHeadroom : 4
            XCTAssertEqual(image.headroom, expectedHeadroom, accuracy: 0.05, file)
            XCTAssertTrue(image.image.colorSpace.map(CGColorSpaceUsesITUR_2100TF) == true, file)
            guard metadata.orientation == 1 else { continue }
            let neutral = stored.map { $0[1] }
            XCTAssertEqual(neutral[1], 1, accuracy: 0.1, "\(file): white \(neutral)")
            XCTAssertGreaterThan(neutral[3], 2.5, "\(file): light above SDR white \(neutral)")
            XCTAssertEqual(neutral, neutral.sorted(), "\(file): brighter stays brighter")
        }
    }

    /// The same files as SDR: ImageIO's SDR rendition, never the raw PQ
    /// signal, nothing above 1.
    func testTheSDRRenditionStaysWithinSDR() throws {
        for file in ["gainmap-iso.jpg", "pq.heic", "pq.avif", "hlg.heic", "pq-cicp.png"] {
            let (metadata, plan, image) = try decode(file, variant: nil)
            let probe = try HDRPlatformProbe.rasterThumbnail(Self.dir.appendingPathComponent(file), plan: plan, hdr: metadata.hdr)
            XCTAssertFalse(image.isHDR, file)
            let space = try XCTUnwrap(image.image.colorSpace)
            XCTAssertEqual(space, probe.colorSpace, "\(file): normalization preserves ImageIO's space")
            if try HDRPlatformProbe.declined("\(file): SDR profile", {
                try HDRPlatformProbe.requireSDRSpace(probe, file: file)
            }) == nil {
                XCTAssertFalse(isHDRSpace(space), "\(file): stored as SDR")
                if file != "gainmap-iso.jpg" { XCTAssertEqual(colorSpaceName(space), "display-p3", file) }
            }
            let e = try JSONDecoder().decode(Expectation.self, from: Data(contentsOf: Self.dir.appendingPathComponent(file + ".json")))
            let neutrals = e.patches.filter { p in
                guard let rgb = p.linearSRGB ?? p.source else { return false }
                return (rgb.max() ?? 0) - (rgb.min() ?? 0) < 0.0001
            }
            XCTAssertFalse(neutrals.isEmpty, "\(file): neutral patches measure light without gamut excursions")
            let reference = try neutrals.map { try HDRPlatformProbe.sample(probe, x: $0.x + $0.w / 2, sourceWidth: metadata.sourceWidth) }
            let stored = try neutrals.map { try HDRPlatformProbe.sample(image.image, x: $0.x + $0.w / 2, sourceWidth: metadata.sourceWidth) }
            for (got, want) in zip(stored, reference) {
                for c in 0..<3 {
                    XCTAssertEqual(got[c], want[c], accuracy: max(0.02, abs(want[c]) * 0.02), "\(file): normalization preserves ImageIO's pixels")
                }
            }
            if try HDRPlatformProbe.declined("\(file): SDR pixel ceiling", {
                try HDRPlatformProbe.requireSDRPixels(probe, peak: reference.flatMap { $0 }.max() ?? 0, file: file)
            }) == nil {
                for got in stored { XCTAssertLessThanOrEqual(got.max() ?? 0, 1.001, "\(file): \(got)") }
            }
        }
    }

    func testTheDisplayAndTheLimitDecideWhetherHDRShows() {
        let view = PlatformView(frame: .zero)
        DisplayRange.pinned = 4
        XCTAssertTrue(DisplayRange.showsHDR(view, limit: nil), "initially no-limit")
        XCTAssertTrue(DisplayRange.showsHDR(view, limit: "constrained"))
        XCTAssertFalse(DisplayRange.showsHDR(view, limit: "standard"))
        DisplayRange.pinned = 1
        XCTAssertFalse(DisplayRange.showsHDR(view, limit: nil), "an SDR display")
    }

    func testOnlyAnHDRBitmapsLayerIsMarked() {
        let layer = CALayer()
        layer.applyDynamicRange(hdr: false, headroom: 0, limit: nil)
        XCTAssertEqual(layer.dynamicRangeFacts["dynamicRange"] as? String, "standard")
        layer.applyDynamicRange(hdr: true, headroom: 4, limit: nil)
        XCTAssertEqual(layer.dynamicRangeFacts["dynamicRange"] as? String, "high")
        layer.applyDynamicRange(hdr: true, headroom: 4, limit: "standard")
        XCTAssertEqual(layer.dynamicRangeFacts["dynamicRange"] as? String, "standard")
        if #available(macOS 26, iOS 26, tvOS 26, *) {
            layer.applyDynamicRange(hdr: true, headroom: 4, limit: "constrained")
            XCTAssertEqual(layer.dynamicRangeFacts["dynamicRange"] as? String, "constrained")
            XCTAssertEqual(layer.contentsHeadroom, 4)
        }
        layer.applyDynamicRange(hdr: false, headroom: 0, limit: "constrained")
        XCTAssertEqual(layer.dynamicRangeFacts["dynamicRange"] as? String, "standard", "SDR content is never marked")
    }

    /// The budget's ladder: an HDR plan costs 8 bytes a pixel; the 4096²
    /// fixture can't have one at full size, and its SDR rendition is 4.
    func testAnHDRPlanCostsEightBytesAPixel() throws {
        let metadata = try header("gainmap-huge.jpg").1
        let hdr = try RasterDecodePlan(metadata: metadata, maxPixel: 1024, variant: RasterVariant.hdr)
        let sdr = try RasterDecodePlan(metadata: metadata, maxPixel: 1024)
        XCTAssertEqual(hdr.bytesPerPixel, 8)
        XCTAssertEqual(sdr.bytesPerPixel, 4)
        XCTAssertThrowsError(try RasterDecodePlan(metadata: metadata, maxPixel: 4096, variant: RasterVariant.hdr))
    }
}

private final class HDRTestCharge: RasterBackingCharge, @unchecked Sendable {}

/// Probe Apple APIs independently of Exact so a host regression still fails
/// whenever the runtime provides the capability the test needs.
enum HDRPlatformProbe {
    struct Declined: Error { let reason: String; var finalizations = 0 }

    /// A declined capability omits only its assertion group, not the rest of the test.
    static func declined(_ behavior: String, _ probe: () throws -> Void) throws -> Declined? {
        do { try probe(); return nil }
        catch let decline as Declined {
            let message = "Skipped \(behavior): \(decline.reason)"
            XCTContext.runActivity(named: message) { _ in }
            return decline
        }
    }

    static func requireContentsFormat(_ format: CALayerContentsFormat) throws {
        let layer = CALayer()
        layer.contentsFormat = format
        guard layer.contentsFormat == format else {
            throw Declined(reason: "CALayer.contentsFormat reports \(layer.contentsFormat.rawValue) for requested \(format.rawValue) (a hint, per https://developer.apple.com/documentation/quartzcore/calayer/contentsformat)")
        }
    }

    static func rasterThumbnail(_ url: URL, plan: RasterDecodePlan, hdr: Bool) throws -> CGImage {
        let source = try XCTUnwrap(CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary))
        var options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: plan.maxPixel,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceShouldAllowFloat: plan.deep,
        ]
        if plan.variant == RasterVariant.hdr {
            options[kCGImageSourceDecodeRequest] = kCGImageSourceDecodeToHDR
        } else if hdr {
            options[kCGImageSourceDecodeRequest] = kCGImageSourceDecodeToSDR
        }
        return try XCTUnwrap(CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary))
    }

    static func requireHDRHeadroom(_ headroom: Float, space: CGColorSpace?, file: String) throws {
        // Zero is unknown, not SDR; BT.2100 still identifies an HDR rendition.
        // https://developer.apple.com/videos/play/wwdc2024/10177/
        if headroom <= 1 && (headroom > 0 || space.map(CGColorSpaceUsesITUR_2100TF) != true) {
            throw Declined(reason: "CGImageSourceCreateThumbnailAtIndex with kCGImageSourceDecodeToHDR reports headroom \(headroom) in \(String(describing: space?.name)) for \(file)")
        }
    }

    static func requireSDRSpace(_ image: CGImage, file: String) throws {
        if let space = image.colorSpace, CGColorSpaceUsesITUR_2100TF(space) {
            throw Declined(reason: "CGImageSourceCreateThumbnailAtIndex with kCGImageSourceDecodeToSDR retains \(String(describing: space.name)) for \(file)")
        }
    }

    static func requireSDRPixels(_ image: CGImage, peak: Double, file: String) throws {
        // A zero tag is unknown; measure the neutral pixels in that case.
        // https://developer.apple.com/documentation/coreimage/ciimage/contentheadroom
        let headroom: Float
        if #available(iOS 18, macOS 15, tvOS 18, *) { headroom = CIImage(cgImage: image).contentHeadroom }
        else { headroom = 0 }
        guard headroom == 1 || (headroom == 0 && peak <= 1.001) else {
            throw Declined(reason: "CGImageSourceCreateThumbnailAtIndex with kCGImageSourceDecodeToSDR yields CIImage.contentHeadroom \(headroom) and neutral peak \(peak) for \(file)")
        }
    }

    /// Extended linear sRGB, read at a patch's centre column, middle row.
    static func sample(_ image: CGImage, x: Int, sourceWidth: Int, rotated: Bool = false) throws -> [Double] {
        let space = try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedLinearSRGB))
        let w = image.width, h = image.height
        let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 32, bytesPerRow: w * 16, space: space,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder32Little.rawValue))
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        let px = try XCTUnwrap(ctx.data).assumingMemoryBound(to: Float.self)
        let i = rotated ? ((x * h / sourceWidth) * w + w / 2) * 4 : ((h / 2) * w + x * w / sourceWidth) * 4
        return (0..<3).map { Double(px[i + $0]) / max(Double(px[i + 3]), 1e-6) }
    }

    @available(iOS 18, macOS 15, tvOS 18, *)
    private static func jpeg(_ url: URL, hdr: Bool,
                             thumbnail: (CGImageSource, Int, CFDictionary?) -> CGImage? = CGImageSourceCreateThumbnailAtIndex) throws -> CGImageSource {
        let source = try XCTUnwrap(CGImageSourceCreateWithURL(url as CFURL, nil))
        let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any])
        let w = try XCTUnwrap(properties[kCGImagePropertyPixelWidth] as? Int)
        let h = try XCTUnwrap(properties[kCGImagePropertyPixelHeight] as? Int)
        let decode: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: max(w, h),
            kCGImageSourceDecodeRequest: hdr ? kCGImageSourceDecodeToHDR : kCGImageSourceDecodeToSDR,
        ]
        let requested = thumbnail(source, 0, decode as CFDictionary)
        if hdr && requested == nil {
            var sdr = decode
            sdr[kCGImageSourceDecodeRequest] = kCGImageSourceDecodeToSDR
            _ = try XCTUnwrap(thumbnail(source, 0, sdr as CFDictionary), "\(url.lastPathComponent): neither HDR nor SDR thumbnail could be decoded")
            throw Declined(reason: "kCGImageSourceDecodeToHDR returns no thumbnail while SDR decodes from \(url.lastPathComponent)")
        }
        let image = try XCTUnwrap(requested)
        let bytes = NSMutableData()
        let destination = try XCTUnwrap(CGImageDestinationCreateWithData(bytes, "public.jpeg" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, [
            kCGImageDestinationLossyCompressionQuality: 0.9,
            kCGImageDestinationEncodeRequest: hdr ? kCGImageDestinationEncodeToISOGainmap : kCGImageDestinationEncodeToSDR,
        ] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else {
            throw Declined(reason: "CGImageDestinationFinalize fails for \(hdr ? "ISO gain-map" : "SDR") JPEG from \(url.lastPathComponent)", finalizations: 1)
        }
        return try XCTUnwrap(CGImageSourceCreateWithData(bytes, nil))
    }

    static func requireGainMapJPEG(_ url: URL,
                                  thumbnail: (CGImageSource, Int, CFDictionary?) -> CGImage? = CGImageSourceCreateThumbnailAtIndex) throws {
        guard #available(iOS 18, macOS 15, tvOS 18, *) else { throw Declined(reason: "ISO gain-map encoding needs iOS 18 / macOS 15") }
        let jpeg = try jpeg(url, hdr: true, thumbnail: thumbnail)
        guard CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap) != nil else {
            throw Declined(reason: "kCGImageDestinationEncodeToISOGainmap produces no ISO gain map from \(url.lastPathComponent)", finalizations: 1)
        }
    }

    @available(iOS 18, macOS 15, tvOS 18, *)
    static func requireSDRJPEG(_ url: URL) throws {
        let jpeg = try jpeg(url, hdr: false)
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(jpeg, 0, nil))
        let headroom = CIImage(cgImage: image).contentHeadroom
        guard CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap) == nil,
              CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeHDRGainMap) == nil,
              image.colorSpace.map(CGColorSpaceUsesITUR_2100TF) != true,
              headroom == 0 || headroom == 1 else {
            throw Declined(reason: "ImageIO's SDR JPEG retains \(String(describing: image.colorSpace?.name)), headroom \(headroom), or a gain map from \(url.lastPathComponent)", finalizations: 1)
        }
    }
}

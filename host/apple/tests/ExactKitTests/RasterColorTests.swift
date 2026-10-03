import XCTest
import ImageIO
import CoreGraphics
@testable import ExactKit

/// LLP 1100 §9 V3: each color fixture decodes to the class, storage and
/// colors its `.json` says. The expected values come from the standards'
/// numbers (`scripts/fixtures/color/make.swift`), so these hold the
/// platform's decode and conversion to the standard, not to itself.
final class RasterColorTests: XCTestCase {
    private struct Patch: Decodable { let x: Int; let y: Int; let w: Int; let h: Int; let linearSRGB: [Double]? }
    private struct Expectation: Decodable {
        let file: String; let id: String; let `class`: String; let variant: String
        let space: String; let bitsPerComponent: Int; let note: String; let patches: [Patch]
        let tolerance: Double?
    }
    private static let dir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("scripts/fixtures/color")

    private func expectations() throws -> [Expectation] {
        let names = try FileManager.default.contentsOfDirectory(atPath: Self.dir.path).filter { $0.hasSuffix(".json") }.sorted()
        XCTAssertGreaterThanOrEqual(names.count, 20, "the fixture set (run make.swift)")
        return try names.map { try JSONDecoder().decode(Expectation.self, from: Data(contentsOf: Self.dir.appendingPathComponent($0))) }
    }
    private func decode(_ file: String, variant: UInt32? = nil, maxPixel: Int? = nil) throws -> (RasterMetadata, RasterDecodePlan, RasterImage) {
        let bytes = try Data(contentsOf: Self.dir.appendingPathComponent(file))
        let metadata = try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count)
        // Full size, or as the loader does past the 32 MiB ceiling, half until it fits.
        var pixel = maxPixel ?? Int(max(metadata.naturalSize.width, metadata.naturalSize.height))
        var fitted = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: variant)
        while fitted == nil, pixel > 1 { pixel /= 2; fitted = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: variant) }
        let plan = try XCTUnwrap(fitted)
        return (metadata, plan, try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: ColorTestCharge()))
    }
    /// The image's pixels as Core Graphics converts them to extended linear
    /// sRGB: what the platform would show, in the space the expectations use.
    private func linear(_ image: CGImage) throws -> (Int, Int, [Float]) {
        let space = try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedLinearSRGB))
        let w = image.width, h = image.height
        let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 32, bytesPerRow: w * 16, space: space,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder32Little.rawValue))
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        let data = try XCTUnwrap(ctx.data).assumingMemoryBound(to: Float.self)
        return (w, h, Array(UnsafeBufferPointer(start: data, count: w * h * 4)))
    }
    /// sRGB's transfer, extended to negatives and values above 1: errors
    /// are compared in encoded steps, as a person sees them.
    private func encoded(_ v: Double) -> Double {
        let a = abs(v); let e = a <= 0.0031308 ? a * 12.92 : 1.055 * pow(a, 1 / 2.4) - 0.055; return v < 0 ? -e : e
    }

    /// V3.1, V3.2: class, storage and patch colors for every fixture.
    func testEveryFixtureDecodesToItsClassStorageAndColors() throws {
        for e in try expectations() {
            let decoded: (RasterMetadata, RasterDecodePlan, RasterImage)
            do { decoded = try decode(e.file) } catch { XCTFail("\(e.id) \(e.file): \(error)"); continue }
            let (metadata, plan, image) = decoded
            let names: [String: UInt32] = ["own8": RasterVariant.own8, "deep": RasterVariant.deep]
            XCTAssertEqual(plan.variant, names[e.variant], "\(e.id) \(e.file): variant")
            XCTAssertEqual(metadata.deep, e.variant == "deep", "\(e.id) \(e.file): deep")
            let stored = image.image
            XCTAssertEqual(stored.colorSpace.map(colorSpaceName), e.space, "\(e.id) \(e.file): stored space")
            XCTAssertEqual(stored.bitsPerComponent, plan.deep ? 16 : 8, "\(e.id) \(e.file): stored depth")
            XCTAssertEqual(image.residentBytes, plan.outputBytes, "\(e.id) \(e.file): charged bytes")
            if plan.deep { XCTAssertTrue(stored.bitmapInfo.contains(.floatComponents), "\(e.id): deep is half float") }
            guard e.class != "hdr" else { continue }   // HDRImageTests checks these
            let (w, h, px) = try linear(stored)
            let lossy = e.file.hasSuffix(".jpg") || e.file.hasSuffix(".heic")
            let band = (e.tolerance ?? (lossy ? 6.0 : 2.0)) / 255
            for p in e.patches {
                guard let want = p.linearSRGB else { continue }
                // The patch's centre, in the stored bitmap's coordinates (row 0 at the top).
                let sx = (p.x + p.w / 2) * w / Int(metadata.sourceWidth), sy = (p.y + p.h / 2) * h / Int(metadata.sourceHeight)
                guard metadata.orientation == 1, sx < w, sy < h else { continue }
                let i = (sy * w + sx) * 4
                let a = Double(px[i + 3])
                let got = (0..<3).map { Double(px[i + $0]) / max(a, 1e-6) }
                for c in 0..<3 {
                    XCTAssertEqual(encoded(got[c]), encoded(want[c]), accuracy: band,
                        "\(e.id) \(e.file) patch at \(p.x): channel \(c) \(got) vs \(want)")
                }
            }
        }
    }

    /// V3.3: 8-bit sRGB is stored as ImageIO decoded it.
    func testAnSRGBPictureIsStoredAsImageIODecodedIt() throws {
        let bytes = try Data(contentsOf: Self.dir.appendingPathComponent("srgb-ramp.png"))
        let (_, plan, image) = try decode("srgb-ramp.png")
        XCTAssertEqual(plan.bytesPerPixel, 4)
        let source = try XCTUnwrap(CGImageSourceCreateWithData(bytes as CFData, nil))
        let reference = try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))
        XCTAssertEqual(try linear(image.image).2, try linear(reference).2)
        XCTAssertEqual(image.image.colorSpace?.name, CGColorSpace.sRGB)
    }

    /// V3.4: an opaque 8-bit P3 picture is adopted, not redrawn.
    func testAnOpaqueP3PictureIsAdoptedWithoutARedraw() throws {
        for file in ["p3-primaries.png", "p3-camera.jpg", "adobe-rgb.jpg"] {
            let bytes = try Data(contentsOf: Self.dir.appendingPathComponent(file))
            let metadata = try RasterMetadata.read(prefix: bytes, encodedBytes: bytes.count)
            let plan = try RasterDecodePlan(metadata: metadata, maxPixel: 256)
            let source = try XCTUnwrap(CGImageSourceCreateWithData(bytes as CFData, nil))
            let thumbnail = try XCTUnwrap(CGImageSourceCreateThumbnailAtIndex(source, 0, RasterImage.thumbnailOptions(plan)))
            guard [.noneSkipFirst, .noneSkipLast].contains(thumbnail.alphaInfo), thumbnail.bytesPerRow <= plan.stride else { continue }
            XCTAssertTrue(RasterImage.normalized(thumbnail, plan: plan) === thumbnail, "\(file): kept as ImageIO made it")
        }
    }

    /// V3.4a: a 16-bit picture keeps its bits. Every column of the P3 ramp
    /// is distinct when stored deep; at 8 bits neighbours merge, which is
    /// what the deep store is for.
    func testADeepPictureKeepsEveryStep() throws {
        func greens(_ variant: UInt32?) throws -> [Float] {
            let (_, plan, image) = try decode("p3-16bit-ramp.png", variant: variant)
            XCTAssertEqual(plan.width, 1024)
            let (w, _, px) = try linear(image.image)
            return (0..<w).map { px[$0 * 4 + 1] }
        }
        let deep = try greens(nil)
        XCTAssertEqual(Set(deep).count, 1024, "every step distinct at 16 bits")
        XCTAssertEqual(deep, deep.sorted(), "and in order")
        let reduced = try greens(RasterVariant.reduced8)
        XCTAssertLessThan(Set(reduced).count, 300, "8 bits can't hold 1024 steps")
    }

    /// V3.4a: the plan prices a deep picture at 8 bytes a pixel, and the
    /// budget's last resort at 4, in the picture's own space.
    func testADeepPlanCostsEightBytesAPixelAndTheReducedOneFour() throws {
        let (metadata, deep, _) = try decode("prophoto-16.tif")
        XCTAssertTrue(metadata.deep)
        XCTAssertEqual(deep.bytesPerPixel, 8)
        XCTAssertGreaterThanOrEqual(deep.stride, deep.width * 8)
        let (_, reduced, image) = try decode("prophoto-16.tif", variant: RasterVariant.reduced8)
        XCTAssertEqual(reduced.bytesPerPixel, 4)
        XCTAssertEqual(deep.outputBytes, reduced.outputBytes * 2)
        XCTAssertEqual(image.image.bitsPerComponent, 8)
        XCTAssertEqual(image.image.colorSpace.map(colorSpaceName), "prophoto-rgb", "reduced, still in its own space")
    }

    /// F20: orientation holds on the wide path.
    func testOrientationHoldsForAWidePicture() throws {
        let (metadata, _, image) = try decode("p3-orient6.jpg")
        XCTAssertEqual(metadata.naturalSize, CGSize(width: 64, height: 256))
        XCTAssertEqual(image.image.width, 64); XCTAssertEqual(image.image.height, 256)
        XCTAssertEqual(image.image.colorSpace.map(colorSpaceName), "display-p3")
    }

    /// F18, F19: damaged color data never fails a picture.
    func testDamagedColorDataStillDecodes() throws {
        XCTAssertEqual(try decode("gainmap-broken.jpg").2.image.colorSpace.map(colorSpaceName), "display-p3")
        XCTAssertEqual(try decode("profile-broken.jpg").2.image.colorSpace.map(colorSpaceName), "srgb")
    }

    /// A gain-map JPEG past the header limit: ImageIO reads nothing from its
    /// prefix, so the frame header gives the size.
    func testAPartialGainMapJPEGStillHasItsSizeAndOrientation() throws {
        let huge = try Data(contentsOf: Self.dir.appendingPathComponent("gainmap-huge.jpg"))
        XCTAssertGreaterThan(huge.count, RasterMetadata.headerLimit)
        let metadata = try RasterMetadata.read(prefix: huge.prefix(RasterMetadata.headerLimit), encodedBytes: huge.count)
        XCTAssertEqual(metadata.naturalSize, CGSize(width: 4096, height: 4096))
        let rotated = try Data(contentsOf: Self.dir.appendingPathComponent("p3-orient6.jpg"))
        let header = try XCTUnwrap(jpegHeader(rotated))
        XCTAssertEqual(header.width, 256); XCTAssertEqual(header.height, 64)
        XCTAssertEqual(header.orientation, 6); XCTAssertEqual(header.depth, 8)
        XCTAssertNil(jpegHeader(Data([0xff, 0xd8, 0xff])))
    }

    /// The agent's names for spaces are the standards' (LLP 1100 D1).
    func testColorSpacesAreNamedByTheirStandards() {
        XCTAssertEqual(colorSpaceName(CGColorSpace(name: CGColorSpace.extendedDisplayP3)!), "display-p3")
        XCTAssertEqual(colorSpaceName(CGColorSpace(name: CGColorSpace.dcip3)!), "--dci-p3")
        XCTAssertEqual(colorSpaceName(CGColorSpace(name: CGColorSpace.itur_2100_PQ)!), "rec2100-pq")
        XCTAssertEqual(colorSpaceName(CGColorSpaceCreateDeviceRGB()), "icc")
    }
}

private final class ColorTestCharge: RasterBackingCharge, @unchecked Sendable {}

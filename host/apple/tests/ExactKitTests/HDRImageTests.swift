import XCTest
import ImageIO
import CoreGraphics
import QuartzCore
@testable import ExactKit

/// LLP 1100 D5, D8: an HDR picture decodes as HDR when its display can
/// show it and `dynamic-range-limit` allows it; its layer asks for that
/// range; anything else is SDR and never marked.
final class HDRImageTests: XCTestCase {
    private struct Patch: Decodable { let x: Int; let w: Int; let linearSRGB: [Double]? }
    private struct Expectation: Decodable { let patches: [Patch] }
    private static let dir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("scripts/fixtures/color")

    override func tearDown() { DisplayRange.pinned = nil }

    private func header(_ file: String) throws -> (Data, RasterMetadata) {
        let bytes = try Data(contentsOf: Self.dir.appendingPathComponent(file))
        return (bytes, try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count))
    }
    private func decode(_ file: String, variant: UInt32?) throws -> (RasterMetadata, RasterImage) {
        let (bytes, metadata) = try header(file)
        var pixel = Int(max(metadata.naturalSize.width, metadata.naturalSize.height))
        var plan = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: variant)
        while plan == nil, pixel > 1 { pixel /= 2; plan = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: variant) }
        let fitted = try XCTUnwrap(plan)
        return (metadata, try RasterImage.decode(bytes, metadata: metadata, plan: fitted, charge: HDRTestCharge()))
    }
    /// Extended linear sRGB, read at a patch's centre column, middle row.
    private func sample(_ image: CGImage, x: Int, sourceWidth: Int) throws -> [Double] {
        let space = try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedLinearSRGB))
        let w = image.width, h = image.height
        let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 32, bytesPerRow: w * 16, space: space,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder32Little.rawValue))
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        let px = try XCTUnwrap(ctx.data).assumingMemoryBound(to: Float.self)
        let i = ((h / 2) * w + x * w / sourceWidth) * 4
        return (0..<3).map { Double(px[i + $0]) / max(Double(px[i + 3]), 1e-6) }
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
            let (metadata, image) = try decode(file, variant: RasterVariant.hdr)
            XCTAssertTrue(image.isHDR, "\(file): tagged HDR")
            XCTAssertGreaterThan(image.headroom, 3.5, file)
            // 10 bits packed; the 16-bit PNG at 16.
            XCTAssertEqual(image.image.colorSpace?.name, CGColorSpace.itur_2100_PQ, file)
            XCTAssertEqual(image.image.bitsPerComponent, file.hasSuffix(".png") ? 16 : 10, file)
            if #available(macOS 15, iOS 18, *) { XCTAssertGreaterThan(image.image.contentHeadroom, 3.5, "\(file): the bitmap carries it") }
            // HEIC and AVIF declare their 4000 cd/m² peak (make.swift); PNG
            // can't, and takes PQ's 1000 cd/m² default.
            XCTAssertEqual(image.headroom, file.hasSuffix(".png") ? 4.93 : 19.7, accuracy: 0.05, "\(file): the declared peak")
            let e = try JSONDecoder().decode(Expectation.self, from: Data(contentsOf: Self.dir.appendingPathComponent(file + ".json")))
            var brightest = 0.0
            for p in e.patches {
                guard let want = p.linearSRGB, want.allSatisfy({ $0 > 0.05 }), want.allSatisfy({ $0 < 4.5 }) else { continue }
                let got = try sample(image.image, x: p.x + p.w / 2, sourceWidth: metadata.sourceWidth)
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
    /// the headroom is the one it was made with (4).
    func testAGainMapRecoversLightAboveSDRWhite() throws {
        for file in ["gainmap-iso.jpg", "gainmap-iso.heic", "gainmap-orient6.heic"] {
            let (metadata, image) = try decode(file, variant: RasterVariant.hdr)
            XCTAssertTrue(image.isHDR, file)
            XCTAssertEqual(image.headroom, 4, accuracy: 0.05, file)
            XCTAssertTrue(image.image.colorSpace.map(CGColorSpaceUsesITUR_2100TF) == true, file)
            guard metadata.orientation == 1 else { continue }
            let w = metadata.sourceWidth / 6
            let neutral = try (0..<4).map { try sample(image.image, x: $0 * w + w / 2, sourceWidth: metadata.sourceWidth)[1] }
            XCTAssertEqual(neutral[1], 1, accuracy: 0.1, "\(file): white \(neutral)")
            XCTAssertGreaterThan(neutral[3], 2.5, "\(file): light above SDR white \(neutral)")
            XCTAssertEqual(neutral, neutral.sorted(), "\(file): brighter stays brighter")
        }
    }

    /// The same files as SDR: ImageIO's SDR rendition, never the raw PQ
    /// signal, nothing above 1.
    func testTheSDRRenditionStaysWithinSDR() throws {
        for file in ["gainmap-iso.jpg", "pq.heic", "pq.avif", "hlg.heic", "pq-cicp.png"] {
            let (metadata, image) = try decode(file, variant: nil)
            XCTAssertFalse(image.isHDR, file)
            let space = try XCTUnwrap(image.image.colorSpace)
            XCTAssertFalse(isHDRSpace(space), "\(file): stored as SDR")
            if file != "gainmap-iso.jpg" { XCTAssertEqual(colorSpaceName(space), "display-p3", file) }
            let got = try sample(image.image, x: metadata.sourceWidth * 5 / 12, sourceWidth: metadata.sourceWidth)
            XCTAssertLessThanOrEqual(got.max() ?? 0, 1.001, "\(file): \(got)")
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
        if #available(macOS 26, iOS 26, *) {
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

import XCTest
import CoreGraphics
import CoreText
import IOSurface
@testable import ExactKit

/// LLP 1100 D3: a profile's colour is made by Core Graphics in the profile's
/// space — a standard one by name, an app's from its ICC file — and a reader
/// that needs sRGB asks Core Graphics to convert it.
final class ProfileColorTests: XCTestCase {
    func testAStandardSpaceIsCoreGraphicsOwn() throws {
        let row: BatchValue = ["cs": [["s": "cg:kCGColorSpaceDCIP3", "i": "relative-colorimetric", "v": [1, 0, 0, 1]]]]
        let c = try XCTUnwrap(row.cgColor(dark: false))
        XCTAssertEqual(c.colorSpace?.name, CGColorSpace.dcip3)
        XCTAssertEqual(c.components, [1, 0, 0, 1])
        let srgb = try XCTUnwrap(row.channels(dark: false), "Core Graphics converts it for a channel reader")
        XCTAssertEqual(srgb[0], 255, accuracy: 1)
        XCTAssertEqual(try XCTUnwrap(row.textChannels(dark: false)).count, 9, "text carries Core Graphics' extended sRGB")
    }

    private func root() throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
        return root
    }

    private func row(_ resolver: AssetResolver, intent: String = "relative-colorimetric", values: [Double] = [1, 0, 0, 1]) throws -> BatchValue {
        let wire: [String: Any] = ["ops": [["op": "style", "id": 1, "style": ["text_color": ["cs": [["s": "icc:assets/brand.icc", "i": intent, "v": values]]]]]]]
        let batch = Batch.decode(try JSONSerialization.data(withJSONObject: wire), resolver: resolver)
        XCTAssertNil(batch.error)
        return try XCTUnwrap(batch.ops.first?.style["text_color"])
    }

    func testProfilesAreBoundToTheirSessionAndAssetBytes() throws {
        let a = try root(), b = try root()
        defer { try? FileManager.default.removeItem(at: a); try? FileManager.default.removeItem(at: b) }
        let srgb = try XCTUnwrap(CGColorSpace(name: CGColorSpace.sRGB)?.copyICCData()) as Data
        let p3 = try XCTUnwrap(CGColorSpace(name: CGColorSpace.displayP3)?.copyICCData()) as Data
        try srgb.write(to: a.appendingPathComponent("assets/brand.icc"))
        try p3.write(to: b.appendingPathComponent("assets/brand.icc"))
        let ra = AssetResolver(root: a), rb = AssetResolver(root: b)
        let first = try row(ra), second = try row(rb)
        XCTAssertEqual(try XCTUnwrap(first.cgColor(dark: false)?.components)[0], 1, accuracy: 0.002)
        XCTAssertGreaterThan(try XCTUnwrap(second.cgColor(dark: false)?.components)[0], 1.05)
        XCTAssertEqual(try row(ra), first, "another session cannot change this session's profile")
        try p3.write(to: a.appendingPathComponent("assets/brand.icc"))
        let updated = try row(ra)
        XCTAssertEqual(updated, second, "same bytes give the same immutable identity")
        XCTAssertNotEqual(updated, first, "a replaced file cannot hit the old style/profile cache")
        XCTAssertEqual(try XCTUnwrap(first.cgColor(dark: false)?.components)[0], 1, accuracy: 0.002, "old views retain their profile")
        let missing: BatchValue = ["cs": [["s": "icc:assets/missing.icc", "v": [1, 0, 0, 1]]]]
        XCTAssertNil(missing.cgColor(dark: false))
    }

    func testICCConversionUsesTheAuthoredRenderingIntent() throws {
        let root = try root()
        defer { try? FileManager.default.removeItem(at: root) }
        let profile = try XCTUnwrap(CGColorSpace(name: CGColorSpace.genericCMYK))
        try (XCTUnwrap(profile.copyICCData()) as Data).write(to: root.appendingPathComponent("assets/brand.icc"))
        let resolver = AssetResolver(root: root), target = try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedSRGB))
        var foundDifferentTransforms = false
        for values: [Double] in [[0, 1, 1, 0, 1], [0.1, 0.8, 0.2, 0.05, 1], [0, 0, 0, 0, 1]] {
            let color = try XCTUnwrap(CGColor(colorSpace: profile, components: values.map { CGFloat($0) }))
            var results: [[CGFloat]] = []
            for (name, intent) in [("perceptual", CGColorRenderingIntent.perceptual), ("relative-colorimetric", .relativeColorimetric), ("absolute-colorimetric", .absoluteColorimetric), ("saturation", .saturation)] {
                let expected = try XCTUnwrap(color.converted(to: target, intent: intent, options: nil)?.components)
                let actual = try XCTUnwrap(row(resolver, intent: name, values: values).cgColor(dark: false)?.components)
                for (a, e) in zip(actual, expected) { XCTAssertEqual(a, e, accuracy: 0.0001, name) }
                results.append(expected)
            }
            foundDifferentTransforms = foundDifferentTransforms || results.dropFirst().contains { r in zip(r, results[0]).contains { abs($0 - $1) > 0.001 } }
        }
        XCTAssertTrue(foundDifferentTransforms, "the fixture must distinguish intents")
    }

    func testProfiledTextPreservesGamutBeyondSRGBAndP3() throws {
        for name in ["cg:kCGColorSpaceDCIP3", "cg:kCGColorSpaceROMMRGB"] {
            let value: BatchValue = ["cs": [["s": .string(name), "v": [0, 1, 0, 1]]]]
            let channels = try XCTUnwrap(value.textChannels(dark: false))
            let color = TextEngine.color(channels).cgColor
            XCTAssertTrue(ColorRange.needsExtendedComponents(color))
            let source = NSAttributedString(string: "Wide", attributes: [.font: CTFontCreateWithName("Helvetica" as CFString, 24, nil), .foregroundColor: color])
            let box = CGRect(x: 0, y: 0, width: 100, height: 40)
            let image = try XCTUnwrap(TextRasterJob(source: source, ranges: [CFRange(location: 0, length: 4)], baselines: [28], flush: 0, box: box, size: box.size, scale: 2).render())
            #if os(macOS)
            XCTAssertEqual(IOSurfaceGetBytesPerElement(image.surface), 8, "profile outside P3 needs float storage")
            #else
            XCTAssertEqual(image.image.bitsPerComponent, 16)
            #endif
        }
    }
}

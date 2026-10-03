import XCTest
import CoreGraphics
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

    func testAnAppsProfileIsReadFromItsICCFile() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let icc = try XCTUnwrap(CGColorSpace(name: CGColorSpace.genericCMYK)?.copyICCData()) as Data
        try icc.write(to: root.appendingPathComponent("assets/cmyk.icc"))
        let resolver = AssetResolver(root: root)
        ProfileSpaces.resolver = resolver
        let row: BatchValue = ["cs": [["s": "icc:assets/cmyk.icc", "i": "perceptual", "v": [0, 1, 1, 0, 1]]]]
        let c = try XCTUnwrap(row.cgColor(dark: false))
        XCTAssertEqual(c.colorSpace?.model, .cmyk)
        XCTAssertEqual(c.numberOfComponents, 5)
        let srgb = try XCTUnwrap(row.channels(dark: false))
        XCTAssertGreaterThan(srgb[0], 150, "CMYK magenta+yellow reads red in sRGB: \(srgb)")
        let wrong: BatchValue = ["cs": [["s": "icc:assets/cmyk.icc", "v": [0, 1, 1]]]]
        XCTAssertNil(wrong.cgColor(dark: false), "the profile's component count")
        withExtendedLifetime(resolver) {}
    }
}

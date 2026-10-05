import XCTest
import ImageIO
import CoreGraphics
@testable import ExactKit

/// LLP 1100 D13: a photo the picker converts to JPEG keeps its color
/// profile and, where the system can encode one, its gain map.
final class PickerColorTests: XCTestCase {
    private static let dir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("scripts/fixtures/color")

    private func convert(_ file: String) throws -> CGImageSource {
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        try Picker.jpeg(from: Self.dir.appendingPathComponent(file), to: out)
        return try XCTUnwrap(CGImageSourceCreateWithData(try Data(contentsOf: out) as CFData, nil))
    }

    func testAnHDRPhotoKeepsItsGainMapAndProfile() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain maps encode from macOS 15 / iOS 18") }
        for file in ["gainmap-iso.heic", "pq.heic"] {
            let jpeg = try convert(file)
            XCTAssertTrue(Picker.isHDR(jpeg), "\(file): still HDR as JPEG")
            XCTAssertNotNil(CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap), file)
        }
        let p3 = try convert("p3.heic")
        XCTAssertFalse(Picker.isHDR(p3))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(p3, 0, nil))
        XCTAssertEqual(image.colorSpace.map(colorSpaceName), "display-p3", "an SDR photo keeps its profile")
    }

    func testOrientationIsAppliedNotCopied() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain maps encode from macOS 15 / iOS 18") }
        let jpeg = try convert("gainmap-orient6.heic")
        let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(jpeg, 0, nil) as? [CFString: Any])
        XCTAssertEqual(properties[kCGImagePropertyPixelWidth] as? Int, 64)
        XCTAssertEqual(properties[kCGImagePropertyPixelHeight] as? Int, 256)
        XCTAssertEqual(properties[kCGImagePropertyOrientation] as? Int ?? 1, 1)
        XCTAssertTrue(Picker.isHDR(jpeg))
    }
}

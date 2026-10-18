import AppKit
import ImageIO
import XCTest

// Raw image facts for snapshotDraftSize; chip palette and paint belong to TS.
final class ComposerImageChipTests: XCTestCase {
    func testTheAccentIsTheAlphaWeightedAverage() throws {
        func image(_ fill: (CGContext) -> Void) throws -> CGImage {
            let context = try XCTUnwrap(CGContext(data: nil, width: 32, height: 32, bitsPerComponent: 8, bytesPerRow: 0,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            fill(context)
            return try XCTUnwrap(context.makeImage())
        }
        // Left half opaque red, right half fully transparent blue: the transparent half does not count.
        let halves = try image { context in
            context.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: 16, height: 32))
        }
        let average = try XCTUnwrap(T3ComposerImageChip.average(halves))
        XCTAssertEqual(average.0, 255, accuracy: 1); XCTAssertEqual(average.1, 0, accuracy: 1); XCTAssertEqual(average.2, 0, accuracy: 1)
        let mixed = try XCTUnwrap(T3ComposerImageChip.average(try image { context in
            context.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: 16, height: 32))
            context.setFillColor(CGColor(srgbRed: 0, green: 0, blue: 1, alpha: 1)); context.fill(CGRect(x: 16, y: 0, width: 16, height: 32))
        }))
        XCTAssertEqual(mixed.0, 128, accuracy: 2); XCTAssertEqual(mixed.2, 128, accuracy: 2)
        XCTAssertNil(T3ComposerImageChip.average(try image { _ in }), "a fully transparent image keeps the default accent")
    }

    func testDraftFileDecodeReturnsRGBOrNoFact() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-image-facts-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("draft.png")
        let context = try XCTUnwrap(CGContext(data: nil, width: 4, height: 4, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
        let image = try XCTUnwrap(context.makeImage())
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(file as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        XCTAssertEqual(T3ComposerImageChip.average(at: file), [255, 0, 0])
        try Data("invalid".utf8).write(to: file)
        XCTAssertNil(T3ComposerImageChip.average(at: file))
        XCTAssertNil(T3ComposerImageChip.average(at: directory.appendingPathComponent("missing.png")))
    }
}

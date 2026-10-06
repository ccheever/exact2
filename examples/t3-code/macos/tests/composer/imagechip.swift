import AppKit
import XCTest

// Image chips (T3ComposerImageChip.swift): ContextChip's colour mixing, the
// averaged accent and the thumbnail a draft image gives its chip.
final class ComposerImageChipTests: XCTestCase {
    func testPaletteMixesTheAccentAsContextChipDoes() {
        // The fixed kinds' palettes (T3ComposerUnderlay.palette) come from the same tokens.
        XCTAssertEqual(T3ComposerImageChip.palette(accent: T3ComposerImageChip.oklch(0.62, 0.136, 237)), T3ComposerUnderlay.palette["file"])
        XCTAssertEqual(T3ComposerImageChip.palette(accent: T3ComposerImageChip.oklch(0.62, 0.16, 16)), T3ComposerUnderlay.palette["image"])
    }

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

    func testADraftImageGivesItsChipASquareThumbnail() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-chip-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 40, pixelsHigh: 20, bitsPerSample: 8, samplesPerPixel: 4,
            hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        for x in 0..<40 { for y in 0..<20 { rep.setColor(NSColor(srgbRed: 0.2, green: 0.5, blue: 0.9, alpha: 1), atX: x, y: y) } }
        let id = UUID().uuidString.lowercased()
        try rep.representation(using: .png, properties: [:])!.write(to: directory.appendingPathComponent("\(id).png"))
        let entry = try XCTUnwrap(T3ComposerImageChip.entry(directory: directory, id: id))
        XCTAssertEqual(entry.thumbnail.width, 20); XCTAssertEqual(entry.thumbnail.height, 20)
        XCTAssertEqual(entry.palette.count, 5)
        XCTAssertTrue(entry.palette[2].hasSuffix("1c"), "the fill is the accent at 11%")
        XCTAssertNil(T3ComposerImageChip.entry(directory: directory, id: UUID().uuidString), "a missing draft draws the plain image chip")
        XCTAssertNil(T3ComposerImageChip.entry(directory: directory, id: "../escape"))
    }

    func testTheSuffixIsTheSizeAloneAndTheImageIsLookedUpByDraftId() {
        let styler = T3ComposerStyler()
        let view = NSTextView(frame: NSRect(x: 0, y: 0, width: 300, height: 40))
        styler.attach(view)
        view.string = "![photo.png](t3-context://v1/image/image_x) "
        styler.contexts = ["image/image_x": "image\t2 KB\tnot-a-uuid"]
        styler.restyle()
        let chip = styler.chips.first
        XCTAssertNotNil(chip)
        if let chip {
            XCTAssertEqual(styler.chipKind(chip), "image")
            XCTAssertEqual(styler.chipSuffix(chip), "2 KB")
            XCTAssertNil(styler.chipImage(chip))
        }
        styler.detach()
    }
}

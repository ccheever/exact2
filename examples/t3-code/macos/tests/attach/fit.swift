import AppKit
import ImageIO
import UniformTypeIdentifiers
import XCTest

// T3ImageFit, which fits SnapShot's draft images: images longer than the client's edge
// (IMAGE_ATTACHMENT_MAX_EDGE, 2000) are scaled to it as T3 Code's imageCompression.ts
// fitImageToMaxEdge scales them (its "attachment max edge" tests).
final class ImageFitTests: XCTestCase {
    private var root: URL!

    override func setUp() {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-fit-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: root.appendingPathComponent("attach"), withIntermediateDirectories: true)
    }
    override func tearDown() { try? FileManager.default.removeItem(at: root) }

    /// An image of `type`, stored `width` by `height`; `noise` fills it with random pixels so PNG cannot compress it.
    private func write(_ name: String, _ type: UTType, width: Int, height: Int, orientation: Int? = nil, noise: Bool = false) throws -> URL {
        let context = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        if noise, let pixels = context.data { arc4random_buf(pixels, width * height * 4); for i in stride(from: 3, to: width * height * 4, by: 4) { pixels.storeBytes(of: 255, toByteOffset: i, as: UInt8.self) } }
        else { context.setFillColor(CGColor(srgbRed: 0.2, green: 0.4, blue: 0.9, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: width, height: height)) }
        let url = root.appendingPathComponent("attach/\(name)")
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, type.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try XCTUnwrap(context.makeImage()), orientation.map { [kCGImagePropertyOrientation: $0] as CFDictionary })
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        return url
    }

    /// T3ImageFit.fit over the file at `url`: the PNG it writes and its size, or nil when it keeps the original.
    private func fit(_ url: URL, _ maxEdge: Int) throws -> (data: Data, width: Int, height: Int)? {
        guard let fitted = T3ImageFit.fit(try Data(contentsOf: url), maxEdge: maxEdge) else { return nil }
        let rep = try XCTUnwrap(NSBitmapImageRep(data: fitted.data))
        XCTAssertEqual([rep.pixelsWide, rep.pixelsHigh], [fitted.width, fitted.height])
        XCTAssertTrue(fitted.data.starts(with: [0x89, 0x50, 0x4e, 0x47]))
        return (fitted.data, rep.pixelsWide, rep.pixelsHigh)
    }

    func testFittedSizeKeepsTheAspectRatioAndRoundsAsMathRound() {
        XCTAssertTrue(T3ImageFit.fittedSize(width: 3024, height: 1964, maxEdge: 2000)! == (2000, 1299))
        XCTAssertTrue(T3ImageFit.fittedSize(width: 3024, height: 4032, maxEdge: 2000)! == (1500, 2000))
        XCTAssertTrue(T3ImageFit.fittedSize(width: 2560, height: 1600, maxEdge: 2000)! == (2000, 1250))
        XCTAssertTrue(T3ImageFit.fittedSize(width: 9000, height: 1, maxEdge: 2000)! == (2000, 1))
        XCTAssertNil(T3ImageFit.fittedSize(width: 2000, height: 1500, maxEdge: 2000))
        XCTAssertNil(T3ImageFit.fittedSize(width: 3000, height: 1000, maxEdge: 0))
    }

    func testALargePngScreenshotScalesAndAnImageAtTheEdgeIsKept() throws {
        let shot = try XCTUnwrap(try fit(try write("a-shot.png", .png, width: 3024, height: 1964), 2000))
        XCTAssertEqual([shot.width, shot.height], [2000, 1299])
        XCTAssertNil(try fit(try write("b-edge.png", .png, width: 2000, height: 1200), 2000))
    }

    func testAJpegScalesByItsDisplayedSize() throws {
        // Stored landscape, displayed portrait after the EXIF rotation (orientation 6).
        let photo = try XCTUnwrap(try fit(try write("photo.jpg", .jpeg, width: 400, height: 300, orientation: 6), 200))
        XCTAssertEqual([photo.width, photo.height], [150, 200])
    }

    func testAGifAndARequestWithoutAnEdgeKeepTheirSize() throws {
        let gif = try write("anim.gif", .gif, width: 300, height: 104)
        XCTAssertNil(try fit(gif, 200))
        XCTAssertFalse(T3ImageFit.downscalable(try Data(contentsOf: gif)))
        let wide = try write("wide.png", .png, width: 300, height: 104)
        XCTAssertEqual(try fit(wide, 200)?.width, 200)
        XCTAssertTrue(T3ImageFit.downscalable(try Data(contentsOf: wide)))
        XCTAssertNil(try fit(wide, 0))
    }
}

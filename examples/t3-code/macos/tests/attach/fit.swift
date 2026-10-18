import AppKit
import ImageIO
import UniformTypeIdentifiers
import XCTest

// T3ImageFit and the attach staging over it: images longer than the client's edge
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

    private func pick(_ request: [String: Any]) -> [[String: Any]] {
        let attach = T3ComposerAttach(dataRoot: root, agent: true)
        let done = expectation(description: "reply")
        var files: [[String: Any]] = []
        attach.pick(request) { reply in
            files = (reply["value"] as? [String: Any])?["files"] as? [[String: Any]] ?? []
            done.fulfill()
        }
        wait(for: [done], timeout: 60)
        return files
    }

    private func draft(_ file: [String: Any]) throws -> (data: Data, width: Int, height: Int) {
        let id = try XCTUnwrap(file["id"] as? String)
        let data = try Data(contentsOf: root.appendingPathComponent("snapshots/drafts/\(id).png"))
        let rep = try XCTUnwrap(NSBitmapImageRep(data: data))
        return (data, rep.pixelsWide, rep.pixelsHigh)
    }

    func testFittedSizeKeepsTheAspectRatioAndRoundsAsMathRound() {
        XCTAssertTrue(T3ImageFit.fittedSize(width: 3024, height: 1964, maxEdge: 2000)! == (2000, 1299))
        XCTAssertTrue(T3ImageFit.fittedSize(width: 3024, height: 4032, maxEdge: 2000)! == (1500, 2000))
        XCTAssertTrue(T3ImageFit.fittedSize(width: 2560, height: 1600, maxEdge: 2000)! == (2000, 1250))
        XCTAssertTrue(T3ImageFit.fittedSize(width: 9000, height: 1, maxEdge: 2000)! == (2000, 1))
        XCTAssertNil(T3ImageFit.fittedSize(width: 2000, height: 1500, maxEdge: 2000))
        XCTAssertNil(T3ImageFit.fittedSize(width: 3000, height: 1000, maxEdge: 0))
    }

    func testPickScalesALargePngScreenshotAndKeepsAnImageAtTheEdgeByteForByte() throws {
        _ = try write("a-shot.png", .png, width: 3024, height: 1964)
        let exact = try write("b-edge.png", .png, width: 2000, height: 1200)
        let original = try Data(contentsOf: exact)
        let files = pick(["generation": 1, "imageMaxEdge": 2000])
        XCTAssertEqual(files.map { $0["kind"] as? String }, ["image", "image"])
        let shot = try draft(files[0])
        XCTAssertEqual([shot.width, shot.height], [2000, 1299])
        XCTAssertEqual(files[0]["name"] as? String, "a-shot.png")
        XCTAssertEqual(files[0]["sizeBytes"] as? Int, shot.data.count)
        XCTAssertEqual(try draft(files[1]).data, original)
    }

    func testAJpegScalesByItsDisplayedSize() throws {
        // Stored landscape, displayed portrait after the EXIF rotation (orientation 6).
        _ = try write("photo.jpg", .jpeg, width: 400, height: 300, orientation: 6)
        let files = pick(["generation": 1, "imageMaxEdge": 200])
        let photo = try draft(try XCTUnwrap(files.first))
        XCTAssertEqual([photo.width, photo.height], [150, 200])
        XCTAssertEqual(files.first?["name"] as? String, "photo.png")
    }

    func testAGifAndARequestWithoutAnEdgeKeepTheirSize() throws {
        _ = try write("anim.gif", .gif, width: 300, height: 104)
        _ = try write("wide.png", .png, width: 300, height: 104)
        let gif = pick(["generation": 1, "imageMaxEdge": 200])
        XCTAssertEqual(try draft(gif[0]).width, 300)
        XCTAssertEqual(gif[0]["downscalable"] as? Bool, false)
        XCTAssertEqual(try draft(gif[1]).width, 200)
        XCTAssertEqual(gif[1]["downscalable"] as? Bool, true)
        _ = try write("wide.png", .png, width: 300, height: 104)
        XCTAssertEqual(try draft(pick(["generation": 2])[0]).width, 300)
    }

    func testAPngOverTheByteLimitIsAcceptedOnceScaled() throws {
        // 1800 x 1800 noise is about 13 MB as a PNG: over the 10 MB image limit until it is scaled.
        let url = try write("noise.png", .png, width: 1800, height: 1800, noise: true)
        XCTAssertGreaterThan(try Data(contentsOf: url).count, T3ComposerAttach.maxImageBytes)
        let files = pick(["generation": 1, "imageMaxEdge": 900])
        XCTAssertEqual(files.first?["kind"] as? String, "image")
        XCTAssertEqual(try draft(try XCTUnwrap(files.first)).width, 900)
        _ = try write("noise.png", .png, width: 1800, height: 1800, noise: true)
        XCTAssertEqual(pick(["generation": 2]).first?["kind"] as? String, "too-large")
    }
}

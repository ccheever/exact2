import XCTest
import ImageIO
import CoreGraphics
@testable import ExactKit

/// LLP 1011.000: Chrome's schedule for an animated image, its clock across
/// visibility, and the frames decoded at the display size.
final class AnimatedRasterTests: XCTestCase {
    func testTenMillisecondsOrLessShowsForOneHundred() {
        let s = AnimationSchedule(seconds: [0, 0.01, 0.011, 0.02, 0.3], plays: 0)
        XCTAssertEqual(s.durations, [100, 100, 11, 20, 300])
    }

    func testFramesByElapsedTimeAndFinitePlaysHoldTheLastFrame() {
        let s = AnimationSchedule(seconds: [0.1, 0.2, 0.1, 0.3], plays: 0)
        XCTAssertEqual(s.at(0).index, 0); XCTAssertEqual(s.at(0).next, 100)
        XCTAssertEqual(s.at(250).index, 1); XCTAssertEqual(s.at(300).index, 2)
        XCTAssertEqual(s.at(650).index, 3)
        XCTAssertEqual(s.at(710).index, 0); XCTAssertEqual(s.at(710).next, 800)
        let twice = AnimationSchedule(seconds: [0.2, 0.2, 0.2], plays: 2)
        XCTAssertEqual(twice.at(650).index, 0)
        XCTAssertEqual(twice.at(1100).index, 2); XCTAssertNil(twice.at(1100).next)
        XCTAssertEqual(twice.at(99_000).index, 2); XCTAssertNil(twice.at(99_000).next)
        let once = AnimationSchedule(seconds: [0.15, 0.15, 0.15], plays: 1)
        XCTAssertEqual(once.at(350).index, 2); XCTAssertNil(once.at(350).next)
    }

    func testAPlayStartsWhenSeenAndCatchesUpUnlessItsFirstPlayEndedUnseen() {
        let s = AnimationSchedule(seconds: [0.1, 0.1, 0.1], plays: 0)
        var c = AnimationClock(s)
        c.hide(at: 5000) // never seen: no clock yet
        XCTAssertEqual(c.show(at: 10_000), 0)
        XCTAssertEqual(c.show(at: 10_150), 1)
        // Unseen past the end of its first play: it starts over.
        c.hide(at: 10_150)
        XCTAssertEqual(c.show(at: 10_450), 0)
        XCTAssertEqual(c.show(at: 10_550), 1)
        // Unseen in a later play: where the time says.
        c.hide(at: 10_800)
        XCTAssertEqual(c.show(at: 11_070), 0)
        XCTAssertEqual(c.show(at: 11_250), 2)
        // More than five minutes behind: the frame it left, from now.
        c.hide(at: 11_250)
        XCTAssertEqual(c.show(at: 411_250), 2)
        XCTAssertEqual(c.next(after: 411_250), 411_350)
    }

    private func gif(_ colors: [(CGFloat, CGFloat, CGFloat)], delays: [Double], loop: Int?, size: Int = 64) throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("exact-anim-\(UUID().uuidString).gif")
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, "com.compuserve.gif" as CFString, colors.count, nil))
        if let loop { CGImageDestinationSetProperties(destination, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFLoopCount: loop]] as CFDictionary) }
        for (c, delay) in zip(colors, delays) {
            let ctx = try XCTUnwrap(CGContext(data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: size * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            ctx.setFillColor(CGColor(srgbRed: c.0, green: c.1, blue: c.2, alpha: 1)); ctx.fill(CGRect(x: 0, y: 0, width: size, height: size))
            CGImageDestinationAddImage(destination, try XCTUnwrap(ctx.makeImage()),
                [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFDelayTime: delay, kCGImagePropertyGIFUnclampedDelayTime: delay]] as CFDictionary)
        }
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        return url
    }

    private func decode(_ url: URL, maxPixel: Int) throws -> RasterImage {
        let bytes = try Data(contentsOf: url)
        let metadata = try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count)
        return try RasterImage.decode(bytes, metadata: metadata, plan: RasterDecodePlan(metadata: metadata, maxPixel: maxPixel),
                                      charge: AnimatedTestCharge(), url: url)
    }

    private func pixel(_ image: CGImage, _ x: Int, _ y: Int) throws -> [UInt8] {
        let ctx = try XCTUnwrap(CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        let p = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        let o = (y * image.width + x) * 4 // row 0 is the top
        return [p[o], p[o + 1], p[o + 2], p[o + 3]]
    }

    func testAnAnimatedGIFCarriesItsScheduleAndDecodesEachFrameAtTheDisplaySize() throws {
        let url = try gif([(1, 0, 0), (0, 0, 1), (0, 1, 0)], delays: [0.1, 0.02, 0], loop: 2)
        defer { try? FileManager.default.removeItem(at: url) }
        let image = try decode(url, maxPixel: 16)
        let animation = try XCTUnwrap(image.animation)
        XCTAssertEqual(animation.count, 3)
        XCTAssertEqual(animation.schedule.durations, [100, 20, 100])
        XCTAssertEqual(animation.schedule.plays, 2) // ImageIO writes and reads plays: NETSCAPE 1
        XCTAssertEqual(image.image.width, 16)
        let frames = try XCTUnwrap(animation.frames())
        for (i, want) in [[255, 0, 0], [0, 0, 255], [0, 255, 0]].enumerated() {
            let frame = try XCTUnwrap(frames.frame(i))
            XCTAssertEqual(frame.width, 16); XCTAssertEqual(frame.height, 16)
            XCTAssertEqual(Array(try pixel(frame, 8, 8).prefix(3)).map(Int.init), want)
        }
        // Back to the start after the last, as a loop reads them.
        XCTAssertEqual(Array(try pixel(try XCTUnwrap(frames.frame(0)), 8, 8).prefix(3)), [255, 0, 0])
    }

    func testAStillGIFAndAGIFWithoutALoopCount() throws {
        let still = try gif([(1, 0, 0)], delays: [0.1], loop: nil)
        let once = try gif([(1, 0, 0), (0, 0, 1)], delays: [0.1, 0.1], loop: nil)
        defer { try? FileManager.default.removeItem(at: still); try? FileManager.default.removeItem(at: once) }
        XCTAssertNil(try decode(still, maxPixel: 16).animation)
        XCTAssertEqual(try XCTUnwrap(try decode(once, maxPixel: 16).animation).schedule.plays, 1)
    }

    /// The gallery's WebP: three frames, the second and third cropped to a
    /// corner and not blended, played twice.
    func testAWebPsFramesComposeOverTheCanvas() throws {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../../../apps/motion-gallery/assets/twice.webp").standardizedFileURL
        let image = try decode(url, maxPixel: 48)
        let animation = try XCTUnwrap(image.animation)
        XCTAssertTrue(animation.webp)
        XCTAssertEqual(animation.count, 3); XCTAssertEqual(animation.schedule.plays, 2)
        let frames = try XCTUnwrap(animation.frames())
        let blue: [UInt8] = [37, 99, 235], red: [UInt8] = [220, 38, 38], green: [UInt8] = [22, 163, 74]
        func at(_ i: Int, _ x: Int, _ y: Int) throws -> [UInt8] { Array(try pixel(try XCTUnwrap(frames.frame(i)), x, y).prefix(3)) }
        XCTAssertEqual(try at(0, 4, 4), blue); XCTAssertEqual(try at(0, 44, 44), blue)
        XCTAssertEqual(try at(1, 4, 4), red); XCTAssertEqual(try at(1, 44, 44), blue)
        XCTAssertEqual(try at(2, 4, 4), red); XCTAssertEqual(try at(2, 44, 44), green)
        XCTAssertEqual(try at(1, 44, 44), blue) // composing again from the first
    }
}

private final class AnimatedTestCharge: RasterBackingCharge, @unchecked Sendable {}

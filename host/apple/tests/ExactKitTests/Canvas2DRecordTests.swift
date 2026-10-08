import XCTest
import CoreGraphics
import QuartzCore
@testable import ExactKit
@testable import ExactSurfaces

/// Canvas 2D recording (LLP 1056 §8.4): the tracker's covers and refusals,
/// replay from the kept lists' start matching a bitmap drawn throughout, and
/// the host's policy.
final class Canvas2DRecordTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    /// A list writer: records as (opcode, operands).
    private func list(_ records: [(Canvas2DOp, [Double])]) -> Data {
        var d = Data()
        func u32(_ v: UInt32) { withUnsafeBytes(of: v.littleEndian) { d.append(contentsOf: $0) } }
        func f64(_ v: Double) { withUnsafeBytes(of: v.bitPattern.littleEndian) { d.append(contentsOf: $0) } }
        u32(0x4432_4345); u32(1)
        for (op, n) in records { u32(op.rawValue); u32(UInt32(n.count)); n.forEach(f64) }
        return d
    }

    private let cover: [(Canvas2DOp, [Double])] = [(.fillColor, [11, 16, 32, 1]), (.fillRect, [0, 0, 150, 100])]

    private func tracker() -> Canvas2DReplayer {
        Canvas2DReplayer(trackingWidth: 300, height: 200, scale: 2, lifetime: 1, generation: 0)
    }

    func testAnOpaqueFullFillRectCoversAndStyleCarriesAcrossLists() {
        let t = tracker()
        XCTAssertTrue(t.track(list(cover)).covers)
        // The fill colour set in the list before still holds.
        XCTAssertTrue(t.track(list([(.fillRect, [0, 0, 150, 100])])).covers)
        XCTAssertFalse(t.track(list([(.fillRect, [0, 0, 149, 100])])).covers, "short of the right edge")
        XCTAssertFalse(t.track(list([(.globalAlpha, [0.5]), (.fillRect, [0, 0, 150, 100])])).covers, "translucent")
        XCTAssertFalse(t.track(list([(.globalAlpha, [1]), (.beginPath, []), (.clip, [0]), (.fillRect, [0, 0, 150, 100])])).covers, "under a clip")
        XCTAssertFalse(t.track(list([(.fillRect, [0, 0, 150, 100])])).covers, "the clip carried from the list before")
        XCTAssertTrue(t.track(list([(.reset, []), (.fillRect, [0, 0, 1, 1])])).covers, "a reset first")
        XCTAssertFalse(t.track(list([(.fillRect, [0, 0, 1, 1]), (.fillColor, [0, 0, 0, 1]), (.fillRect, [0, 0, 150, 100])])).covers,
                       "a paint before the cover")
    }

    func testWhatARecordingDrawsDifferentlyIsRefused() {
        XCTAssertTrue(tracker().track(list([(.shadowColor, [0, 0, 0, 1]), (.shadowBlur, [4]), (.fillRect, [0, 0, 10, 10])])).refused)
        XCTAssertTrue(tracker().track(list([(.conicGradient, [1, 0, 50, 50])])).refused)
        XCTAssertTrue(tracker().track(list([(.composite, [1]), (.fillRect, [0, 0, 10, 10])])).refused, "source-in")
        XCTAssertFalse(tracker().track(list([(.composite, [11]), (.fillRect, [0, 0, 10, 10])])).refused, "multiply draws the same")
        XCTAssertFalse(tracker().track(list(cover)).refused)
    }

    /// The pixels of `lists` replayed into a bitmap throughout, and of the
    /// kept lists replayed from their start as a recording replays them.
    private func pixels(_ r: Canvas2DReplayer) -> [UInt8] {
        let image = r.image()!
        var px = [UInt8](repeating: 0, count: 300 * 200 * 4)
        let c = CGContext(data: &px, width: 300, height: 200, bitsPerComponent: 8, bytesPerRow: 1200,
                          space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        c.draw(image, in: CGRect(x: 0, y: 0, width: 300, height: 200))
        return px
    }

    func testTheKeptListsFromTheirStartDrawWhatTheBitmapHolds() {
        let lists = [
            // A gradient made before the cover and used after it; a save
            // left open; a path carried into the next list.
            list([(.linearGradient, [7, 0, 0, 150, 0]), (.colorStop, [7, 0, 255, 0, 0, 1]), (.colorStop, [7, 1, 0, 0, 255, 1]),
                  (.save, []), (.setTransform, [2, 0, 0, 2, 0, 0])]),
            list(cover + [(.fillGradient, [7]), (.beginPath, []), (.moveTo, [0, 0]), (.lineTo, [60, 0]), (.lineTo, [60, 40])]),
            list([(.lineTo, [0, 40]), (.closePath, []), (.fill, [0]), (.restore, []), (.fillColor, [255, 255, 255, 0.5]),
                  (.fillRect, [100, 50, 30, 30])]),
        ]
        let whole = Canvas2DReplayer(width: 300, height: 200, scale: 2, lifetime: 1, generation: 0)
        for l in lists { XCTAssertTrue(whole.apply(l)) }
        let kept = Canvas2DKept(tracker())
        for l in lists { kept.keep(l) }
        XCTAssertEqual(kept.lists.count, 2, "the cover starts the kept lists over")
        let replay = Canvas2DReplayer(width: 300, height: 200, scale: 2, lifetime: 1, generation: 0)
        replay.restore(kept.start!)
        for l in kept.lists { XCTAssertTrue(replay.apply(l)) }
        XCTAssertEqual(pixels(replay), pixels(whole))
    }

    func testKeptListsAreBounded() {
        let kept = Canvas2DKept(tracker())
        let small = list([(.fillColor, [255, 0, 0, 1]), (.fillRect, [0, 0, 1, 1])])
        for _ in 0...Canvas2DKept.maxLists { kept.keep(small) }
        XCTAssertFalse(kept.bounded)
        kept.drop()
        kept.keep(small)
        XCTAssertNil(kept.start, "after a drop only a cover starts again")
        XCTAssertTrue(kept.lists.isEmpty)
        kept.keep(list(cover))
        XCTAssertNotNil(kept.start)
        XCTAssertEqual(kept.lists.count, 1)
    }

    private func op(_ lists: [Data], fresh: Bool, stretch: Bool = false, animating: Bool = true) -> [String: Any] {
        ["lifetime": 1, "generation": 0, "fresh": fresh, "w": 300, "h": 200, "scale": 2.0,
         "stretch": stretch, "animating": animating, "box": [0.0, 0.0, 150.0, 100.0], "radii": [0.0, 0.0, 0.0, 0.0], "lists": lists]
    }

    func testAnAnimatingCanvasIsRecordedAndAShadowReturnsItToTheBitmap() throws {
        let host = Canvas2DHost(), parent = CALayer()
        host.apply(7, op([list(cover)], fresh: true), layer: parent)
        for _ in 0..<3 { host.apply(7, op([list(cover)], fresh: false), layer: parent); host.waitForReplays(timeout: 5) }
        let layer = try XCTUnwrap(parent.sublayers?.first)
        let recorder = try XCTUnwrap(layer.sublayers?.compactMap { $0 as? Canvas2DRecordLayer }.first, "a canvas that asks for frames is recorded")
        XCTAssertNil(layer.contents, "no bitmap while recorded")
        XCTAssertFalse(recorder.isHidden)
        host.apply(7, op([list([(.shadowColor, [0, 0, 0, 1]), (.shadowBlur, [4]), (.fillRect, [10, 10, 20, 20])])], fresh: false), layer: parent)
        host.waitForReplays(timeout: 5)
        XCTAssertTrue(showsBitmap(layer), "a shadow draws in the bitmap, from the kept lists")
        XCTAssertTrue(recorder.isHidden)
    }

    private func showsBitmap(_ layer: CALayer) -> Bool {
        layer.contents.map { CFGetTypeID($0 as CFTypeRef) == CGImage.typeID } ?? false
    }

    func testACanvasThatDoesNotAnimateKeepsItsBitmap() throws {
        let host = Canvas2DHost(), parent = CALayer()
        host.apply(7, op([list(cover)], fresh: true, animating: false), layer: parent)
        host.waitForReplays(timeout: 5)
        XCTAssertTrue(showsBitmap(try XCTUnwrap(parent.sublayers?.first)))
    }

    func testAnExplicitBitmapIsNeverRecorded() throws {
        let host = Canvas2DHost(), parent = CALayer()
        host.apply(7, op([list(cover)], fresh: true, stretch: true), layer: parent)
        for _ in 0..<3 { host.apply(7, op([list(cover)], fresh: false, stretch: true), layer: parent); host.waitForReplays(timeout: 5) }
        XCTAssertTrue(showsBitmap(try XCTUnwrap(parent.sublayers?.first)))
    }
}

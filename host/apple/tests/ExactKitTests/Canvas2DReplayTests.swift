import XCTest
import CoreGraphics
import QuartzCore
@testable import ExactKit

/// Canvas 2D replay off the main thread (LLP 1056 §8.3): a canvas drawn
/// every frame faster than it replays still presents, and its backlog is
/// one replay waiting, not one per frame.
final class Canvas2DReplayTests: XCTestCase {
    /// A list: `fillColor` then `count` full-bitmap `fillRect`s.
    private func list(_ rgb: (Double, Double, Double), rects count: Int = 1) -> Data {
        var d = Data()
        func u32(_ v: UInt32) { withUnsafeBytes(of: v.littleEndian) { d.append(contentsOf: $0) } }
        func f64(_ v: Double) { withUnsafeBytes(of: v.bitPattern.littleEndian) { d.append(contentsOf: $0) } }
        u32(0x4432_4345); u32(1)
        u32(Canvas2DOp.fillColor.rawValue); u32(4); f64(rgb.0); f64(rgb.1); f64(rgb.2); f64(1)
        for _ in 0..<count { u32(Canvas2DOp.fillRect.rawValue); u32(4); f64(0); f64(0); f64(150); f64(100) }
        return d
    }

    private func op(_ lists: [Data], fresh: Bool) -> [String: Any] {
        ["lifetime": 1, "generation": 0, "fresh": fresh, "w": 300, "h": 200, "scale": 2.0,
         "stretch": true, "box": [0.0, 0.0, 150.0, 100.0], "radii": [0.0, 0.0, 0.0, 0.0], "lists": lists]
    }

    /// The shown bitmap's first pixel, as RGB bytes.
    private func pixel(_ layer: CALayer) throws -> [UInt8] {
        let image = try XCTUnwrap(layer.contents.map { $0 as! CGImage })
        var px = [UInt8](repeating: 0, count: 4)
        let c = try XCTUnwrap(CGContext(data: &px, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                        space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        c.draw(image, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        return Array(px[0..<3])
    }

    func testAReplayDoneShowsThoughNewerListsArrivedMeanwhile() throws {
        let host = Canvas2DHost(), parent = CALayer()
        host.apply(7, op([list((255, 0, 0))], fresh: true), layer: parent)
        Thread.sleep(forTimeInterval: 0.3)  // the red replay ends; its showing waits for the main thread
        // A slow list arrives before the red one's result reaches the main thread.
        host.apply(7, op([list((0, 0, 255), rects: 20_000)], fresh: false), layer: parent)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02))
        let layer = try XCTUnwrap(parent.sublayers?.first)
        XCTAssertNotNil(layer.contents, "the finished replay shows while a newer one runs")
        host.waitForReplays(timeout: 30)
        XCTAssertEqual(try pixel(layer), [0, 0, 255], "then the newest")
    }

    func testFramesFasterThanReplayWaitAsOneReplayAndHoldTheFrameRequest() throws {
        let host = Canvas2DHost(), parent = CALayer()
        var held: [Bool] = []
        host.onHeld = { view, h in XCTAssertEqual(view, 7); held.append(h) }
        host.apply(7, op([list((255, 0, 0), rects: 20_000)], fresh: true), layer: parent)
        for i in 1...100 { host.apply(7, op([list((0, Double(i), 0))], fresh: false), layer: parent) }
        XCTAssertLessThanOrEqual(host.loadingCount, 2, "one replay running, one waiting: frames join it")
        XCTAssertEqual(held.first, true, "the runner holds the canvas's frame request")
        host.waitForReplays(timeout: 30)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02))
        XCTAssertEqual(host.loadingCount, 0)
        XCTAssertEqual(held.last, false, "and releases it once the replay caught up")
        XCTAssertEqual(try pixel(try XCTUnwrap(parent.sublayers?.first)), [0, 100, 0], "every list applied, the last on top")
    }

    func testAFreshBitmapSupersedesTheListsWaitingBeforeIt() throws {
        let host = Canvas2DHost(), parent = CALayer()
        host.apply(7, op([list((255, 0, 0), rects: 2_000)], fresh: true), layer: parent)
        for _ in 0..<10 { host.apply(7, op([list((0, 255, 0))], fresh: false), layer: parent) }
        host.apply(7, op([], fresh: true), layer: parent)
        host.waitForReplays(timeout: 30)
        let px = try pixel(try XCTUnwrap(parent.sublayers?.first))
        XCTAssertEqual(px, [0, 0, 0], "a new bitmap is cleared; the lists before it are dropped")
    }

    /// The main thread's font scan skips every other record's operands and
    /// finds the fonts a full decode finds, in order.
    func testTheFontScanFindsWhatAFullReadFinds() {
        var d = Data()
        func u32(_ v: UInt32) { withUnsafeBytes(of: v.littleEndian) { d.append(contentsOf: $0) } }
        func f64(_ v: Double) { withUnsafeBytes(of: v.bitPattern.littleEndian) { d.append(contentsOf: $0) } }
        func font(_ size: Double, _ family: String) {
            let cps = family.unicodeScalars.map { Double($0.value) }
            u32(Canvas2DOp.font.rawValue); u32(UInt32(9 + cps.count))
            [size, 700, 0, 100, 0, 0, 0, 0, 0].forEach(f64); cps.forEach(f64)
        }
        u32(0x4432_4345); u32(1)
        u32(Canvas2DOp.fillRect.rawValue); u32(4); [0, 0, 10, 10].forEach(f64)
        font(12, "serif")
        u32(Canvas2DOp.beginPath.rawValue); u32(0)
        u32(Canvas2DOp.cubicTo.rawValue); u32(6); [1, 2, 3, 4, 5, 6].forEach(f64)
        font(30, "Exposure Sans,monospace")
        var full: [Canvas2DFont] = []
        XCTAssertTrue(Canvas2DReplayer.read(d) { op, n, count in if op == .font { full.append(Canvas2DFont(record: n, count: count)) } })
        XCTAssertEqual(full.count, 2)
        XCTAssertEqual(Canvas2DReplayer.fonts(in: d), full)
        XCTAssertEqual(Canvas2DReplayer.fonts(in: d.prefix(d.count - 8)), [full[0]], "a truncated record ends the scan")
    }
}

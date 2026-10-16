import XCTest
@testable import ExactKit

/// SVG islands retain their coverage and phase when tiled or cropped
/// (LLP 1055.000 D7, D10, D14), within `SvgIsland.cap` pixels.
class SvgIslandTests: XCTestCase {
    func testRotatedPatternHasNoSeamsBetweenOpaqueTiles() throws {
        let rect = CGRect(x: 0, y: 0, width: 32, height: 32)
        let cell: [String: Any] = ["id": 1, "n": 7, "o": 1,
                                   "p": [0, 0, 0, 1, 8, 0, 1, 8, 8, 1, 0, 8, 3],
                                   "f": [51, 65, 85, 255]]
        for angle in [CGFloat.pi / 4, -.pi / 6] {
            let t = CGAffineTransform(rotationAngle: angle)
            for scale: CGFloat in [1, 2] {
                for opacity in [1.0, 0.5] {
                    let pattern: [String: Any] = ["pt": [0, 0, 8, 8], "c": [cell], "o": opacity,
                                                  "t": [t.a, t.b, t.c, t.d, 0, 0]]
                    let layer = try XCTUnwrap(SvgIsland.pattern(pattern, rect: rect, scale: scale, dark: false, fonts: nil))
                    let image = try XCTUnwrap(layer.contents) as! CGImage
                    let data = try XCTUnwrap(image.dataProvider?.data)
                    let bytes = try XCTUnwrap(CFDataGetBytePtr(data))
                    var worst = 0
                    for y in 0..<image.height {
                        for x in 0..<image.width {
                            let alpha = Int(bytes[y * image.bytesPerRow + x * 4 + 3])
                            worst = max(worst, abs(alpha - Int((255 * opacity).rounded())))
                        }
                    }
                    XCTAssertLessThanOrEqual(worst, 1, "angle \(angle), scale \(scale), opacity \(opacity)")
                }
            }
        }
    }

    func testPatternPhaseFollowsItsTileOriginAndTransformAcrossCrops() throws {
        func paint(origin: CGPoint, rect: CGRect) throws -> CGImage {
            let x = origin.x, y = origin.y
            let cell: [String: Any] = ["id": 1, "n": 7, "o": 1,
                                       "p": [0, x, y, 1, x + 4, y, 1, x + 4, y + 8, 1, x, y + 8, 3],
                                       "f": [51, 65, 85, 255]]
            let t = CGAffineTransform(rotationAngle: .pi / 4).translatedBy(x: -x, y: -y)
            let pattern: [String: Any] = ["pt": [x, y, 8, 8], "c": [cell],
                                          "t": [t.a, t.b, t.c, t.d, t.tx, t.ty]]
            let layer = try XCTUnwrap(SvgIsland.pattern(pattern, rect: rect, scale: 1, dark: false, fonts: nil))
            return try XCTUnwrap(layer.contents) as! CGImage
        }
        let rect = CGRect(x: 0, y: 0, width: 32, height: 32)
        let whole = try paint(origin: .zero, rect: rect)
        let shifted = try paint(origin: CGPoint(x: 2, y: 3), rect: rect)
        XCTAssertEqual(whole.dataProvider?.data, shifted.dataProvider?.data)
        let part = try paint(origin: .zero, rect: CGRect(x: 5, y: 7, width: 16, height: 16))
        let a = try XCTUnwrap(whole.dataProvider?.data), b = try XCTUnwrap(part.dataProvider?.data)
        let pa = try XCTUnwrap(CFDataGetBytePtr(a)), pb = try XCTUnwrap(CFDataGetBytePtr(b))
        var worst = 0
        // Compare the requested paint bounds, excluding the raster's one-unit gutter.
        for y in 1..<(part.height - 1) {
            for x in 4..<((part.width - 1) * 4) {
                worst = max(worst, abs(Int(pa[(y + 7) * whole.bytesPerRow + 5 * 4 + x]) - Int(pb[y * part.bytesPerRow + x])))
            }
        }
        XCTAssertLessThanOrEqual(worst, 1)
    }

    func testUnderTheCapIsTheWholeRegion() {
        let rect = CGRect(x: -12, y: -8, width: 144, height: 96)
        let e = SvgIsland.extent(rect, k: 3, seen: CGRect(x: 0, y: 0, width: 10, height: 10))
        XCTAssertEqual(e?.rect, rect)
        XCTAssertEqual(e?.w, 432)
        XCTAssertEqual(e?.h, 288)
        XCTAssertEqual(e?.k, 3)
    }

    /// The feature benchmark's repro: a 200% drop-shadow region on a group
    /// spanning a 1600 × 1000 view box shown with `slice` in a 1032 pt box
    /// at 2x, 24 M pixels whole.
    func testOverTheCapIsCutToWhatShowsOnTheSameGrid() throws {
        let rect = CGRect(x: -780, y: -470, width: 3120, height: 1920)
        let seen = CGRect(x: 284, y: 0, width: 1032, height: 1000).insetBy(dx: -10, dy: -10)
        let e = try XCTUnwrap(SvgIsland.extent(rect, k: 2, seen: seen))
        XCTAssertEqual(e.k, 2)
        XCTAssertEqual(e.w, 2104)
        XCTAssertEqual(e.h, 2040)
        XCTAssertEqual(e.rect, CGRect(x: 274, y: -10, width: 1052, height: 1020))
        XCTAssertLessThanOrEqual(e.w * e.h, SvgIsland.cap)
    }

    func testStillOverTheCapIsDrawnAtFewerPixels() throws {
        let rect = CGRect(x: 0, y: 0, width: 4000, height: 4000)
        let e = try XCTUnwrap(SvgIsland.extent(rect, k: 3, seen: nil))
        XCTAssertEqual(e.rect, rect)
        XCTAssertLessThanOrEqual(e.w * e.h, SvgIsland.cap)
        XCTAssertGreaterThan(e.w * e.h, SvgIsland.cap * 99 / 100)
        XCTAssertEqual(e.k, 3 * CGFloat(e.w) / 12000, accuracy: 0.001)
    }

    func testNothingShowsIsNothing() {
        let rect = CGRect(x: 0, y: 0, width: 4000, height: 4000)
        XCTAssertNil(SvgIsland.extent(rect, k: 3, seen: CGRect(x: 5000, y: 0, width: 10, height: 10)))
    }
}

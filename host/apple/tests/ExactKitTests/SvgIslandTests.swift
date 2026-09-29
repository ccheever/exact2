import XCTest
@testable import ExactKit

/// An SVG island's extent (LLP 1055.000 D10, D14): at most `SvgIsland.cap`
/// pixels, never dropped. An island under the cap is drawn whole, as
/// before; one over it is cut to what can show, on its own pixel grid, and
/// then drawn at fewer pixels per unit when still over.
class SvgIslandTests: XCTestCase {
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

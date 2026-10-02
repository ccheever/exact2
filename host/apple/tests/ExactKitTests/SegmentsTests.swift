// The viewport segments a fold makes, as a pure function of the viewport
// and UIKit's division frames (LLP 1076 D5), and the even split a host
// without a fold makes for `prefer segments` (D7, D10): no window, no
// simulator, no 27.1 — the numbers the Duo reports, held as assertions.
import CoreGraphics
import XCTest
@testable import ExactKit

final class SegmentsTests: XCTestCase {
    /// The Duo at 130°: the inner panel's division is a 40-point band at
    /// x = 455.5 with 20-point margins, so the frame (455.5, 0, 40, 669)
    /// splits a 951 × 669 cover viewport into two columns around it.
    func testADivisionSplitsTheViewportIntoTwoColumnsAroundTheBand() {
        let fold = Segments.split(viewport: CGSize(width: 951, height: 669), dividers: [CGRect(x: 455.5, y: 0, width: 40, height: 669)])
        XCTAssertEqual(fold.posture, "folded")
        XCTAssertEqual(fold.cols, 2)
        XCTAssertEqual(fold.rows, 1)
        XCTAssertEqual(fold.rects, [CGRect(x: 0, y: 0, width: 455.5, height: 669), CGRect(x: 495.5, y: 0, width: 455.5, height: 669)])
        let env = fold.env
        XCTAssertEqual(env["device-posture"] as? String, "folded")
        XCTAssertEqual(env["horizontal-viewport-segments"] as? Int, 2)
        XCTAssertEqual((env["viewport-segments"] as? [[Double]])?[1], [495.5, 0, 455.5, 669])
    }

    /// A non-cover viewport is the panel less the safe area (867 × 635 at
    /// the panel's origin): the division's frame converted into the
    /// viewport's space, and clipped to it.
    func testTheDivisionIsConvertedIntoANonCoverViewportAndClipped() {
        let fold = Segments.split(viewport: CGSize(width: 867, height: 635), dividers: [CGRect(x: 455.5, y: -40, width: 40, height: 740)])
        XCTAssertEqual(fold.cols, 2)
        XCTAssertEqual(fold.rects, [CGRect(x: 0, y: 0, width: 455.5, height: 635), CGRect(x: 495.5, y: 0, width: 371.5, height: 635)])
    }

    func testNoActiveDivisionIsOneContinuousSegment() {
        let flat = Segments.split(viewport: CGSize(width: 951, height: 669), dividers: [])
        XCTAssertEqual(flat, .flat)
        XCTAssertEqual(flat.env["viewport-segments"] as? [[Double]], [])
    }

    func testAWideBandCutsRowsAndABandOutsideTheViewportCutsNothing() {
        let rows = Segments.split(viewport: CGSize(width: 400, height: 900), dividers: [CGRect(x: 0, y: 430, width: 400, height: 40)])
        XCTAssertEqual(rows.cols, 1); XCTAssertEqual(rows.rows, 2)
        XCTAssertEqual(rows.rects, [CGRect(x: 0, y: 0, width: 400, height: 430), CGRect(x: 0, y: 470, width: 400, height: 430)])
        let outside = Segments.split(viewport: CGSize(width: 466, height: 678), dividers: [CGRect(x: 900, y: 0, width: 40, height: 678)])
        XCTAssertEqual(outside.cols, 1); XCTAssertEqual(outside.rows, 1)
        XCTAssertEqual(outside.rects, [])
        XCTAssertEqual(outside.posture, "folded", "a band the viewport does not meet still says the hinge is bent")
        // Two bands make a 2 × 2 grid, row-major.
        let grid = Segments.split(viewport: CGSize(width: 100, height: 100), dividers: [CGRect(x: 40, y: 0, width: 20, height: 100), CGRect(x: 0, y: 40, width: 100, height: 20)])
        XCTAssertEqual(grid.cols, 2); XCTAssertEqual(grid.rows, 2)
        XCTAssertEqual(grid.rects.map { $0.origin }, [CGPoint(x: 0, y: 0), CGPoint(x: 60, y: 0), CGPoint(x: 0, y: 60), CGPoint(x: 60, y: 60)])
    }

    func testTheEvenSplitCentresTheGapAndRefusesByName() throws {
        XCTAssertEqual(try Segments.even(viewport: CGSize(width: 951, height: 669), cols: 2, rows: 1, gap: 40),
                       [CGRect(x: 0, y: 0, width: 455.5, height: 669), CGRect(x: 495.5, y: 0, width: 455.5, height: 669)])
        XCTAssertEqual(try Segments.even(viewport: CGSize(width: 951, height: 669), cols: 1, rows: 1, gap: 40), [])
        XCTAssertEqual(try Segments.even(viewport: CGSize(width: 100, height: 100), cols: 2, rows: 2, gap: 0).count, 4)
        XCTAssertThrowsError(try Segments.even(viewport: CGSize(width: 100, height: 100), cols: 0, rows: 1, gap: 0))
        XCTAssertThrowsError(try Segments.even(viewport: CGSize(width: 100, height: 100), cols: 1, rows: 0, gap: 0))
        XCTAssertThrowsError(try Segments.even(viewport: CGSize(width: 100, height: 100), cols: 2, rows: 1, gap: 100)) { error in
            XCTAssertTrue("\(error)".contains("wider than the viewport"), "\(error)")
        }
    }
}

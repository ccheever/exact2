// What `layout` says about a scroller that is past its own ends.
//
// A stretched rubber band and an ordinary scroll position are the same
// number in `scroll x,y`, and the same picture in a screenshot. That is how
// a band that never released survived a driven check and a screenshot both
// (LLP 1033 D4a). `layout` now reports how far past its ends a scroller
// sits, and this states the arithmetic of that report — no window, no
// animation, no timing.
//
// @ref LLP 1012 (the eight operations), LLP 1033 D4a
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class OverscrollReportTests: XCTestCase {
    /// A 300×300 viewport over a 300×700 document: 400 points of travel on y
    /// and none on x.
    private let document = CGSize(width: 300, height: 700)
    private let visible = CGSize(width: 300, height: 300)

    private func past(_ x: CGFloat, _ y: CGFloat) -> (CGFloat, CGFloat) {
        Agent.overscroll(origin: CGPoint(x: x, y: y), document: document, visible: visible)
    }

    func testWithinItsBoundsThereIsNoOverscroll() {
        for y in [CGFloat(0), 1, 200, 400] {
            let (ox, oy) = past(0, y)
            XCTAssertEqual(ox, 0, "x at y=\(y)")
            XCTAssertEqual(oy, 0, "y at y=\(y)")
        }
    }

    /// Past the top is negative — the sign says which end, which is what
    /// tells a reader whether the band is at the top or the bottom.
    func testPastTheTopReportsHowFarAndWhichWay() {
        XCTAssertEqual(past(0, -83).1, -83)
    }

    /// Past the end is positive, and measured from the end rather than zero.
    func testPastTheEndIsMeasuredFromTheEnd() {
        XCTAssertEqual(past(0, 460).1, 60)
    }

    /// An axis with nothing to scroll still overscrolls: that is exactly the
    /// case a page whose content fits can get stuck in, and the one that
    /// dragged this app's chrome around (LLP 1033 D4).
    func testAnAxisWithNoTravelStillReportsAStretch() {
        XCTAssertEqual(past(-40, 0).0, -40)
    }

    /// Half a point of slack: a fractional layout is not an overscroll, or
    /// every rounded frame would report one.
    func testAFractionOfAPointIsNotAnOverscroll() {
        XCTAssertEqual(past(0, -0.25).1, 0)
        XCTAssertEqual(past(0, 400.25).1, 0)
    }
}
#endif

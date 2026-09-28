// Where a wheel event goes.
//
// This target exists for one class of defect: a host routing decision that
// no other check in this repository can see. The five checks compile and run
// Rust; the agent drives the app through the eight operations and its wheel
// carries a delta and no gesture phase; a screenshot shows a stretched rubber
// band and a scrolled pane as the same picture. So a lift dropped before
// AppKit could see it — the zero-delta `.ended` that ends every trackpad
// scroll — left a band stretched forever, passed every check, drove
// perfectly under the agent, and was found by a person scrolling.
//
// `ChainingScrollView.routing` is a pure function of the event and the
// geometry, so these are assertions about a decision: no window, no run
// loop, no animation, no clock. Nothing here can flake on timing.
//
// @ref LLP 1033 D4a, LLP 1010 (nested chaining), `rules/RULES.md` §Loop shape
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class ScrollRoutingTests: XCTestCase {
    /// A scroller with both axes able to move, and 400 points of room on each.
    private func scroller(containX: Bool = false, containY: Bool = false) -> ChainingScrollView {
        let sv = ChainingScrollView(frame: NSRect(x: 0, y: 0, width: 300, height: 300))
        sv.scrollsX = true
        sv.scrollsY = true
        sv.containX = containX
        sv.containY = containY
        return sv
    }

    /// 400 points of travel on each axis, sitting at `origin`.
    private func extent(at origin: NSPoint = .zero) -> ChainingScrollView.Extent {
        ChainingScrollView.Extent(maxX: 400, maxY: 400, origin: origin)
    }

    /// The regression this file exists for: the lift that ends a trackpad
    /// gesture carries no delta, and a contained scroller must still see it
    /// or AppKit never learns the gesture is over and holds the stretch.
    func testAZeroDeltaLiftReachesAppKitOnAContainedAxis() {
        let sv = scroller(containY: true)
        XCTAssertEqual(sv.routing(dx: 0, dy: 0, phased: true, in: extent(at: NSPoint(x: 0, y: 400))), .appKit)
        // Even sitting past the end, which is where a stretched band is.
        XCTAssertEqual(sv.routing(dx: 0, dy: 0, phased: true, in: extent(at: NSPoint(x: 0, y: -60))), .appKit)
    }

    /// An `auto` scroller is driven by hand and has no use for phases; a
    /// delta-less event must not become a chained event either.
    func testAZeroDeltaEventIsDroppedWhenNoAxisIsContained() {
        XCTAssertEqual(scroller().routing(dx: 0, dy: 0, phased: true, in: extent()), .drop)
        // And a delta-less event with no phase at all is nobody's.
        XCTAssertEqual(scroller(containY: true).routing(dx: 0, dy: 0, phased: false, in: extent()), .drop)
    }

    /// A contained axis hands a *gesture* to AppKit — its scrolling, its
    /// momentum, and its rubber band — rather than clamping by hand.
    func testAContainedAxisWithRoomGivesAGestureToAppKit() {
        let sv = scroller(containY: true)
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: true, in: extent()), .appKit)
        // At the end of its travel too: that is when the band forms.
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: true, in: extent(at: NSPoint(x: 0, y: 400))), .appKit)
    }

    /// A wheel with no phases is a mouse wheel, and a mouse wheel does not
    /// rubber-band on this platform. It is clamped here — which also keeps it
    /// synchronous, and synchronous is what an agent can read back without
    /// racing AppKit's animation.
    func testAPhaselessWheelOnAContainedAxisScrollsHere() {
        let sv = scroller(containY: true)
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: false, in: extent()), .here)
        // Still contained: at its end it stops rather than moving the page.
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: false, in: extent(at: NSPoint(x: 0, y: 400))), .drop)
    }

    /// `contain` forbids passing the rest of a gesture to an ancestor, so a
    /// contained axis with nothing to scroll stops the gesture rather than
    /// chaining it.
    func testAContainedAxisWithNoRoomDropsRatherThanChains() {
        let sv = scroller(containY: true)
        let none = ChainingScrollView.Extent(maxX: 0, maxY: 0, origin: .zero)
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: true, in: none), .drop)
    }

    /// The default is CSS scroll chaining, which Caltrain's fixture depends
    /// on: what this view can take it takes, and the remainder goes up.
    func testAnAutoAxisScrollsHereWithRoomAndChainsAtItsEnd() {
        let sv = scroller()
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: false, in: extent()), .here)
        // Scrolled to the bottom, a further downward delta belongs to the page.
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: false, in: extent(at: NSPoint(x: 0, y: 400))), .chain)
        // And at the top, an upward one does.
        XCTAssertEqual(sv.routing(dx: 0, dy: 50, phased: false, in: extent()), .chain)
    }

    /// An axis that does not scroll at all chains, whatever the room.
    func testAnAxisThatDoesNotScrollChains() {
        let sv = scroller()
        sv.scrollsY = false
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: false, in: extent()), .chain)
    }

    /// A gesture is one thing: the larger delta decides which axis's rules
    /// apply, so a sideways gesture over a vertically-contained pane still
    /// chains.
    func testTheDominantAxisDecides() {
        let vertical = scroller(containY: true)
        XCTAssertEqual(vertical.routing(dx: -80, dy: -10, phased: true, in: extent()), .here)
        XCTAssertEqual(vertical.routing(dx: -10, dy: -80, phased: true, in: extent()), .appKit)

        let horizontal = scroller(containX: true)
        XCTAssertEqual(horizontal.routing(dx: -80, dy: -10, phased: true, in: extent()), .appKit)
        XCTAssertEqual(horizontal.routing(dx: -10, dy: -80, phased: true, in: extent()), .here)
    }

    /// An equal-magnitude gesture is vertical, so a pane contained only on y
    /// keeps it. Stated because "dominant" needs a tie-break and this is it.
    func testATieIsVertical() {
        XCTAssertEqual(scroller(containY: true).routing(dx: -50, dy: -50, phased: true, in: extent()), .appKit)
        XCTAssertEqual(scroller(containX: true).routing(dx: -50, dy: -50, phased: true, in: extent()), .here)
    }

    /// A phase-less tick is not split (LLP 1070 G2, Chrome's rule, its
    /// §2): over a strip that scrolls only sideways, a mostly-vertical
    /// diagonal still scrolls the strip by its x and drops its y, and a
    /// purely vertical tick chains to the feed.
    func testAPhaselessTickGoesToTheDeepestScrollerThatCanTakeAnyOfIt() {
        let strip = scroller()
        strip.scrollsY = false
        let sideways = ChainingScrollView.Extent(maxX: 400, maxY: 0, origin: .zero)
        XCTAssertEqual(strip.routing(dx: -60, dy: -100, phased: false, in: sideways), .here)
        XCTAssertEqual(strip.routing(dx: 0, dy: -120, phased: false, in: sideways), .chain)
        // At its end the strip takes no more x: the tick chains.
        let end = ChainingScrollView.Extent(maxX: 400, maxY: 0, origin: NSPoint(x: 400, y: 0))
        XCTAssertEqual(strip.routing(dx: -60, dy: -100, phased: false, in: end), .chain)
        // A gesture still goes by its dominant axis (latching is LLP 1070 G1).
        XCTAssertEqual(strip.routing(dx: -60, dy: -100, phased: true, in: sideways), .chain)
    }

    /// The readers' panes, as they are declared: `overflow-x: hidden` with
    /// `overscroll-behavior: contain` (LLP 1033 D4). A sideways gesture must
    /// not escape to the page, and must not scroll the pane either.
    func testAReaderPaneNeitherScrollsSidewaysNorLeaks() {
        let sv = scroller(containX: true, containY: true)
        sv.scrollsX = false
        let onlyVertical = ChainingScrollView.Extent(maxX: 0, maxY: 400, origin: .zero)
        XCTAssertEqual(sv.routing(dx: -80, dy: 0, phased: true, in: onlyVertical), .drop)
        XCTAssertEqual(sv.routing(dx: 0, dy: -80, phased: true, in: onlyVertical), .appKit)
        XCTAssertEqual(sv.routing(dx: 0, dy: -80, phased: false, in: onlyVertical), .here)
    }
}
#endif

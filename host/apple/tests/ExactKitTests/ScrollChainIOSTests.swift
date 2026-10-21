#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// CSS scroll chaining at a touch's start (LLP 1070 G1; Q4, ruled
/// provisionally pending a real-finger feel test): an inner scroller at its
/// edge hands an outward drag to the scroller around it under
/// `overscroll-behavior: auto`; `contain` keeps UIKit's rubber band, and
/// `none` keeps the gesture with no band. `handsOff` is a decision over the
/// velocity and the geometry, checked without a finger:
///   bun host/apple/build.mjs --test --ios
final class ScrollChainIOSTests: XCTestCase {
    /// A page that scrolls vertically (it holds the rest: keep it alive), holding a node whose scroller scrolls
    /// on `horizontal`'s axis, 1,000 points of content in a 300-point port.
    private func nested(horizontal: Bool, behavior: String? = nil) -> (ScrollView, ScrollView) {
        let page = ScrollView(frame: CGRect(x: 0, y: 0, width: 300, height: 600))
        page.scrollsX = false
        page.contentSize = CGSize(width: 300, height: 3000)
        page.contentOffset = CGPoint(x: 0, y: 500)
        let p = Presenter()
        let owner = NodeView(id: 1, kind: "list", presenter: p)
        owner.frame = CGRect(x: 0, y: 0, width: 300, height: 300)
        if let behavior { owner.style[horizontal ? "overscroll_behavior_x" : "overscroll_behavior_y"] = .string(behavior) }
        page.addSubview(owner)
        let inner = ScrollView(frame: owner.bounds)
        inner.scrollsX = horizontal
        inner.scrollsY = !horizontal
        inner.contentSize = horizontal ? CGSize(width: 1000, height: 300) : CGSize(width: 300, height: 1000)
        owner.addSubview(inner)
        return (page, inner)
    }

    func testAnInnerListAtItsStartHandsAnOutwardDragToThePage() {
        let (page, inner) = nested(horizontal: false)
        defer { withExtendedLifetime(page) {} }
        // At its top, a finger moving down pulls toward the start: the page takes it.
        XCTAssertTrue(inner.handsOff(CGPoint(x: 0, y: 400)))
        // Moving up, the inner list has room: it keeps the drag.
        XCTAssertFalse(inner.handsOff(CGPoint(x: 0, y: -400)))
        // Scrolled in, it has room both ways.
        inner.contentOffset = CGPoint(x: 0, y: 200)
        XCTAssertFalse(inner.handsOff(CGPoint(x: 0, y: 400)))
        // At its end, a finger moving up goes to the page.
        inner.contentOffset = CGPoint(x: 0, y: 700)
        XCTAssertTrue(inner.handsOff(CGPoint(x: 0, y: -400)))
    }

    func testAnUnknownDirectionOrAContainedAxisKeepsTheGesture() {
        let (page_inner, inner) = nested(horizontal: false)
        defer { withExtendedLifetime(page_inner) {} }
        XCTAssertFalse(inner.handsOff(.zero), "a pan whose direction is unknown is never failed")
        let (page_contained, contained) = nested(horizontal: false, behavior: "contain")
        defer { withExtendedLifetime(page_contained) {} }
        XCTAssertFalse(contained.handsOff(CGPoint(x: 0, y: 400)))
        XCTAssertTrue(contained.bounces, "`contain` keeps the rubber band")
        let (page_none, none) = nested(horizontal: false, behavior: "none")
        defer { withExtendedLifetime(page_none) {} }
        XCTAssertFalse(none.handsOff(CGPoint(x: 0, y: 400)))
        XCTAssertFalse(none.bounces, "`none` has no band")
        // A style that leaves `none` gets its band back at the next drag.
        none.superview.flatMap { $0 as? NodeView }?.style["overscroll_behavior_y"] = .string("auto")
        _ = none.handsOff(CGPoint(x: 0, y: 400))
        XCTAssertTrue(none.bounces, "leaving `none` restores the band")
        // A sideways flick does not give the band back to a vertical `none`.
        none.superview.flatMap { $0 as? NodeView }?.style["overscroll_behavior_y"] = .string("none")
        _ = none.handsOff(CGPoint(x: 400, y: 0))
        XCTAssertFalse(none.bounces, "a vertical `none` keeps no band under a horizontal drag")
    }

    /// Forced vertical bounce (`fitScroll`): a short page keeps it through a
    /// stray point of width, and a carousel, as tall as its content, does not
    /// take it.
    func testAShortPageBouncesThroughAStrayPointOfWidthAndACarouselDoesNot() {
        let p = Presenter()
        func scroller(content: CGSize, row: Bool = false, rowChild: Bool = false) -> ScrollView {
            let node = NodeView(id: 1, kind: "scroll", presenter: p)
            if row { node.style["display"] = .string("flex") }
            node.frame = CGRect(x: 0, y: 0, width: 300, height: 600)
            let sv = ScrollView(frame: node.bounds)
            sv.scrollsX = true
            sv.scrollsY = true
            node.addSubview(sv)
            if rowChild {
                let strip = NodeView(id: 2, kind: "view", presenter: p)
                strip.style["display"] = .string("flex")
                strip.frame = CGRect(origin: .zero, size: content)
                sv.addSubview(strip)
            }
            node.scroll = sv
            node.content = content
            node.fitScroll()
            return sv
        }
        XCTAssertTrue(scroller(content: CGSize(width: 300, height: 200)).alwaysBounceVertical, "a short page")
        XCTAssertTrue(scroller(content: CGSize(width: 301.5, height: 200)).alwaysBounceVertical, "a short page a point too wide")
        XCTAssertTrue(scroller(content: CGSize(width: 900, height: 1200)).alwaysBounceVertical, "vertical travel")
        XCTAssertTrue(scroller(content: CGSize(width: 320, height: 200)).alwaysBounceVertical, "a short page a few points too wide")
        XCTAssertFalse(scroller(content: CGSize(width: 900, height: 600)).alwaysBounceVertical, "a carousel")
        XCTAssertFalse(scroller(content: CGSize(width: 900, height: 180)).alwaysBounceVertical, "a carousel shorter than its box")
        XCTAssertFalse(scroller(content: CGSize(width: 360, height: 180), row: true).alwaysBounceVertical, "a short flex-row strip")
        XCTAssertFalse(scroller(content: CGSize(width: 360, height: 180), rowChild: true).alwaysBounceVertical, "a block port around a flex-row strip")
        XCTAssertTrue(scroller(content: CGSize(width: 301.5, height: 600)).alwaysBounceVertical, "a page that fills its port, a point too wide")
    }

    func testNothingAroundToTakeItKeepsTheBand() {
        // A strip at its start, dragged right: the page cannot scroll on x.
        let (page_strip, strip) = nested(horizontal: true)
        defer { withExtendedLifetime(page_strip) {} }
        XCTAssertFalse(strip.handsOff(CGPoint(x: 400, y: 0)))
        // A vertical drag on a strip is the page's already (UIKit: the strip
        // has no vertical travel); the rule does not claim it.
        XCTAssertFalse(strip.handsOff(CGPoint(x: 0, y: 400)))
        // The page at its own top cannot take a downward drag either.
        let (page, inner) = nested(horizontal: false)
        defer { withExtendedLifetime(page) {} }
        page.contentOffset = .zero
        XCTAssertFalse(inner.handsOff(CGPoint(x: 0, y: 400)))
    }
}
#endif

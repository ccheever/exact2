import CoreGraphics
import XCTest
@testable import ExactKit

/// LLP 1021 §5, "Placement": the painted top layers' `position-area`, the
/// margin box aligned to the invoker as CSS aligns it.
final class PositionAreaTests: XCTestCase {
    private let anchor = CGRect(x: 100, y: 200, width: 80, height: 30)
    private let size = CGSize(width: 120, height: 60)
    private let layer = CGRect(x: 0, y: 0, width: 400, height: 600)

    func testEachAreaPlacesTheBorderBoxWithoutMargins() {
        func at(_ area: String) -> CGPoint { PositionArea.origin(area, anchor: anchor, size: size, in: layer) }
        XCTAssertEqual(at("none"), CGPoint(x: 100, y: 230), "D2: top-left at the invoker's bottom-left")
        XCTAssertEqual(at("bottom span-right"), at("none"))
        XCTAssertEqual(at("bottom"), CGPoint(x: 80, y: 230), "below, centred")
        XCTAssertEqual(at("bottom span-all"), at("bottom"))
        XCTAssertEqual(at("top span-right"), CGPoint(x: 100, y: 140), "bottom-left at the invoker's top-left")
        XCTAssertEqual(at("top"), CGPoint(x: 80, y: 140))
        XCTAssertEqual(at("center"), CGPoint(x: 80, y: 185), "centred over it")
        XCTAssertEqual(at("right span-bottom"), CGPoint(x: 180, y: 200), "a submenu: top-left at the invoker's top-right")
    }

    /// `position-area="top" margin=0 margin-bottom=12`: CSS aligns the
    /// margin box, so the border box ends 12 points above the invoker.
    func testTheMarginBoxIsWhatIsAligned() {
        func at(_ area: String, _ m: PositionArea.Margins) -> CGPoint {
            PositionArea.origin(area, anchor: anchor, size: size, margins: m, in: layer)
        }
        XCTAssertEqual(at("top", .init(bottom: 12)), CGPoint(x: 80, y: 128), "a 12-point gap above the invoker")
        XCTAssertEqual(at("none", .init(top: 8, left: 4)), CGPoint(x: 104, y: 238), "inside its margins below")
        XCTAssertEqual(at("bottom", .init(right: 20)), CGPoint(x: 70, y: 230), "the wider margin box centred")
        XCTAssertEqual(at("center", .init(top: 10)), CGPoint(x: 80, y: 190), "the taller margin box centred")
    }

    /// CSS shifts an anchor-centred axis, and an overflowing edge back over
    /// the top and left edges; the native default retains its clamp.
    func testCentredAxesAndStartEdgesShift() {
        let high = CGRect(x: 0, y: 20, width: 40, height: 30)
        XCTAssertEqual(PositionArea.origin("top", anchor: high, size: size, margins: .init(bottom: 12, left: 6), overflow: .absolute(page: layer), in: layer),
                       CGPoint(x: 6, y: 0), "no option fits: the margin box is shifted down to the top edge")
        let low = CGRect(x: 380, y: 570, width: 20, height: 30)
        XCTAssertEqual(PositionArea.origin("none", anchor: low, size: size, margins: .init(right: 5, bottom: 5), in: layer),
                       CGPoint(x: 275, y: 535), "held at the bottom and right edges, its margins inside them")
    }

    /// A painted popover whose author wrote no area takes the web's UA
    /// default (neutral.css): below from the inline start, flipping above or
    /// inline where that doesn't fit (Harmony's emoji button at the bottom-left
    /// opens above it, not held over it), and shifted into its limit only when
    /// nothing fits. Authored fallbacks, `none` included, replace the flips;
    /// an authored area keeps its own.
    func testAPaintedPopoverWithNoAreaFlipsAsTheWebsDefault() {
        func at(_ anchor: CGRect, _ size: CGSize, area: String? = nil, fallbacks: String? = nil, rtl: Bool = false, _ m: PositionArea.Margins = .init()) -> CGPoint {
            let placed = PositionArea.painted(area: area, fallbacks: fallbacks, rtl: rtl)
            return PositionArea.origin(placed.area, anchor: anchor, size: size, margins: m, fallbacks: placed.fallbacks, rtl: rtl, in: layer)
        }
        let emoji = CGRect(x: 12, y: 560, width: 30, height: 30), picker = CGSize(width: 300, height: 200)
        XCTAssertEqual(at(emoji, picker), CGPoint(x: 12, y: 360), "above, from its left edge")
        XCTAssertEqual(at(anchor, size), CGPoint(x: 100, y: 230), "room below: as before")
        XCTAssertEqual(at(CGRect(x: 380, y: 570, width: 20, height: 30), size, .init(right: 5, bottom: 5)), CGPoint(x: 280, y: 510),
                       "bottom-right: above and leftward, its margins flipped too")
        XCTAssertEqual(at(CGRect(x: 200, y: 100, width: 30, height: 30), CGSize(width: 120, height: 60), rtl: true), CGPoint(x: 110, y: 130),
                       "right to left: below, spanning left from its inline start")
        XCTAssertEqual(at(emoji, CGSize(width: 300, height: 700)).y, 0, "nothing fits: shifted into the bounds")
        XCTAssertEqual(at(emoji, picker, fallbacks: "none"), CGPoint(x: 12, y: 400), "authored `none`: no flips, shifted up into the bounds")
        XCTAssertEqual(at(emoji, picker, area: "none"), PositionArea.origin("none", anchor: emoji, size: picker, in: layer), "an authored area keeps its own")
    }

    func testOrderedFlipsAtEveryEdgeAndNoFittingFallback() {
        let viewport = CGRect(x: 0, y: 0, width: 420, height: 600)
        let box = CGSize(width: 322, height: 122)
        let tries = "flip-block, flip-inline, flip-block flip-inline"
        func at(_ area: String, _ anchor: CGRect, _ fallback: String = "none", _ bounds: CGRect? = nil) -> CGPoint {
            PositionArea.origin(area, anchor: anchor, size: box, fallbacks: fallback, overflow: .absolute(page: bounds ?? viewport), in: bounds ?? viewport)
        }
        let low = CGRect(x: 332, y: 560, width: 80, height: 32)
        XCTAssertEqual(at("bottom span-right", low), CGPoint(x: 332, y: 592))
        XCTAssertEqual(at("bottom span-right", low, "flip-block, flip-inline"), CGPoint(x: 332, y: 592), "neither single flip fits")
        XCTAssertEqual(at("bottom span-right", low, tries), CGPoint(x: 90, y: 438))
        XCTAssertEqual(at("bottom span-right", low, tries, CGRect(x: 0, y: 0, width: 800, height: 800)), CGPoint(x: 332, y: 592), "a fresh opening uses the fitting base")
        XCTAssertEqual(at("top span-left", CGRect(x: 8, y: 8, width: 80, height: 32), tries), CGPoint(x: 8, y: 40))
        XCTAssertEqual(at("left span-bottom", CGRect(x: 8, y: 200, width: 80, height: 32), tries), CGPoint(x: 88, y: 200))
        XCTAssertEqual(at("right span-top", low, tries), CGPoint(x: 10, y: 470))
        let middle = CGRect(x: 8, y: 300, width: 80, height: 32)
        XCTAssertEqual(at("bottom span-left", middle, "flip-block flip-inline, flip-inline"), CGPoint(x: 8, y: 178), "first fitting attempt wins")
        XCTAssertEqual(at("bottom span-left", middle, "flip-inline, flip-block flip-inline"), CGPoint(x: 8, y: 332))
    }

    func testFlipsExchangeMarginsAndFitTheWholeMarginBox() {
        let low = CGRect(x: 332, y: 560, width: 80, height: 32)
        let box = CGSize(width: 322, height: 122)
        let viewport = CGRect(x: 0, y: 0, width: 420, height: 600)
        let at = PositionArea.origin("bottom span-right", anchor: low, size: box,
                                     margins: .init(top: 10, left: 5),
                                     fallbacks: "flip-block, flip-inline, flip-block flip-inline", in: viewport)
        XCTAssertEqual(at, CGPoint(x: 85, y: 428))
        let tooWide = PositionArea.origin("bottom span-right", anchor: low, size: box,
                                          margins: .init(left: 110), fallbacks: "flip-block flip-inline", overflow: .absolute(page: viewport), in: viewport)
        XCTAssertEqual(tooWide, CGPoint(x: 442, y: 592), "the border box alone fitting is insufficient")
    }

    func testSuccessfulChoiceSurvivesResizeAndResetsWithTheStyle() {
        var history = PositionArea.History()
        let low = CGRect(x: 332, y: 560, width: 80, height: 32)
        let size = CGSize(width: 322, height: 122)
        let tries = "flip-block, flip-inline, flip-block flip-inline"
        func at(_ width: CGFloat, _ height: CGFloat, _ list: String) -> CGPoint {
            PositionArea.origin("bottom span-right", anchor: low, size: size, fallbacks: list, overflow: .absolute(page: CGRect(x: 0, y: 0, width: width, height: height)), history: &history,
                                in: CGRect(x: 0, y: 0, width: width, height: height))
        }
        XCTAssertEqual(at(420, 600, tries), CGPoint(x: 90, y: 438))
        XCTAssertEqual(at(800, 800, tries), CGPoint(x: 90, y: 438))
        XCTAssertEqual(at(100, 100, tries), CGPoint(x: 90, y: 438), "if every choice overflows, retain the current one")
        XCTAssertEqual(at(800, 800, "none"), CGPoint(x: 332, y: 592), "a changed list clears the remembered option")
    }

    /// `flip-start` mirrors across the top-left/bottom-right diagonal, and an
    /// attempt's flips apply in the order written. Chrome 155's rectangles.
    func testFlipStartComposesInOrder() {
        let viewport = CGRect(x: 0, y: 0, width: 420, height: 600)
        let box = CGSize(width: 122, height: 122)
        func at(_ area: String, _ x: CGFloat, _ y: CGFloat, _ fallback: String, _ m: PositionArea.Margins = .init()) -> CGPoint {
            PositionArea.origin(area, anchor: CGRect(x: x, y: y, width: 80, height: 32), size: box, margins: m,
                                fallbacks: fallback, overflow: .absolute(page: viewport), in: viewport)
        }
        XCTAssertEqual(at("bottom span-right", 8, 500, "flip-start flip-block"), CGPoint(x: 88, y: 410), "right span-top")
        XCTAssertEqual(at("bottom span-right", 8, 500, "flip-block flip-start"), CGPoint(x: 8, y: 532), "left span-bottom does not fit")
        XCTAssertEqual(at("bottom span-right", 8, 500, "flip-start flip-block", .init(top: 1, right: 2, bottom: 3, left: 4)),
                       CGPoint(x: 89, y: 406), "top and left margins exchange, then top and bottom")
        XCTAssertEqual(at("bottom", 8, 540, "flip-start"), CGPoint(x: 88, y: 478), "right, its centred axis shifted")
        XCTAssertEqual(at("top", 8, 8, "flip-start, flip-block"), CGPoint(x: 0, y: 40))
        XCTAssertEqual(at("bottom span-right", 332, 500, "flip-inline flip-start flip-block, flip-start flip-inline flip-block"),
                       CGPoint(x: 210, y: 410), "the second order fits")
    }

    /// No option fits: a fixed box is shifted inside every edge, an absolute
    /// one inside the top and left edges only (Chrome 155).
    func testNoFittingOptionShiftsIntoTheContainingBlock() {
        let viewport = CGRect(x: 0, y: 0, width: 420, height: 600)
        let square = CGSize(width: 122, height: 122), wide = CGSize(width: 322, height: 122)
        let low = CGRect(x: 332, y: 560, width: 80, height: 32)
        XCTAssertEqual(PositionArea.origin("top span-left", anchor: CGRect(x: 200, y: 8, width: 80, height: 32), size: square,
                                           margins: .init(top: 3, left: 5), fallbacks: "flip-start", overflow: .absolute(page: viewport), in: viewport),
                       CGPoint(x: 158, y: 3))
        XCTAssertEqual(PositionArea.origin("left", anchor: CGRect(x: 8, y: 300, width: 80, height: 32), size: square,
                                           fallbacks: "flip-start", overflow: .absolute(page: viewport), in: viewport), CGPoint(x: 0, y: 178))
        XCTAssertEqual(PositionArea.origin("bottom span-right", anchor: low, size: wide, overflow: .absolute(page: viewport), in: viewport), CGPoint(x: 332, y: 592))
        XCTAssertEqual(PositionArea.origin("bottom span-right", anchor: low, size: wide, in: viewport), CGPoint(x: 98, y: 478))
        XCTAssertEqual(PositionArea.origin("right", anchor: low, size: square, in: viewport), CGPoint(x: 298, y: 478))
        XCTAssertEqual(PositionArea.origin("bottom span-right", anchor: CGRect(x: -10, y: 200, width: 80, height: 32), size: square, in: viewport),
                       CGPoint(x: -10, y: 232), "the limit holds the area an invoker past the edge gives")
        XCTAssertEqual(PositionArea.origin("left", anchor: CGRect(x: -60, y: 300, width: 80, height: 32), size: square, in: viewport),
                       CGPoint(x: -60, y: 255), "the grid's outer line reaches the invoker")
        XCTAssertEqual(PositionArea.origin("top", anchor: CGRect(x: 200, y: -10, width: 80, height: 32), size: square, in: viewport),
                       CGPoint(x: 179, y: -10))
        let scrolled = CGRect(x: 0, y: -100, width: 420, height: 1200), high = CGRect(x: 200, y: 8, width: 80, height: 32)
        XCTAssertEqual(PositionArea.origin("top", anchor: high, size: square, overflow: .absolute(page: scrolled), in: viewport),
                       CGPoint(x: 179, y: -100), "an absolute box is held at the page's top, not the window's")
        XCTAssertEqual(PositionArea.origin("top", anchor: high, size: square, in: viewport), CGPoint(x: 179, y: 0))
        XCTAssertEqual(PositionArea.origin("bottom span-right", anchor: low, size: wide, overflow: .platform, in: viewport),
                       CGPoint(x: 332, y: 592), "a native menu's launch point is left to AppKit to fit")
    }

    /// A right-to-left document: `flip-start` mirrors across the diagonal from
    /// the top-right corner, and the right edge holds an absolute box (Chrome 155).
    func testRightToLeftFlipStartAndStartEdge() {
        let viewport = CGRect(x: 0, y: 0, width: 420, height: 600), square = CGSize(width: 122, height: 122)
        func at(_ area: String, _ x: CGFloat, _ y: CGFloat, _ fallback: String = "none", _ m: PositionArea.Margins = .init()) -> CGPoint {
            PositionArea.origin(area, anchor: CGRect(x: x, y: y, width: 80, height: 32), size: square, margins: m, fallbacks: fallback,
                                overflow: .absolute(page: viewport), rtl: true, in: viewport)
        }
        XCTAssertEqual(at("bottom span-left", 332, 500, "flip-start flip-block"), CGPoint(x: 210, y: 410), "left span-top")
        XCTAssertEqual(at("bottom span-left", 332, 500, "flip-start flip-block", .init(top: 1, right: 2, bottom: 3, left: 4)),
                       CGPoint(x: 209, y: 408), "top and right margins exchange, then top and bottom")
        XCTAssertEqual(at("right", 332, 300), CGPoint(x: 298, y: 255))
        XCTAssertEqual(at("left", 8, 300), CGPoint(x: -114, y: 255))
    }

    /// Changing `position` clears the remembered option (§6.5.1).
    func testAPositionChangeClearsTheRememberedOption() {
        var history = PositionArea.History()
        let low = CGRect(x: 332, y: 560, width: 80, height: 32), size = CGSize(width: 322, height: 122)
        let tries = "flip-block, flip-inline, flip-block flip-inline"
        func at(_ side: CGFloat, _ overflow: PositionArea.Overflow) -> CGPoint {
            let bounds = CGRect(x: 0, y: 0, width: side == 0 ? 420 : side, height: side == 0 ? 600 : side)
            return PositionArea.origin("bottom span-right", anchor: low, size: size, fallbacks: tries, overflow: overflow,
                                       history: &history, in: bounds)
        }
        XCTAssertEqual(at(0, .fixed), CGPoint(x: 90, y: 438))
        XCTAssertEqual(at(800, .fixed), CGPoint(x: 90, y: 438), "kept while it fits")
        XCTAssertEqual(at(800, .absolute(page: CGRect(x: 0, y: 0, width: 800, height: 800))), CGPoint(x: 332, y: 592), "the base fits again")
    }

    /// An option's region is its cell of the grid the anchor's edges make with
    /// the block's, the outer lines reaching the anchor past the block; a
    /// centred cell is the anchor's, an unsaid axis the whole block.
    func testAnOptionsRegionIsItsGridCell() {
        let block = CGRect(x: 0, y: 0, width: 400, height: 600), anchor = CGRect(x: 150, y: 100, width: 40, height: 20)
        XCTAssertEqual(PositionArea.region(1, 2, anchor: anchor, in: block), CGRect(x: 150, y: 120, width: 250, height: 480), "bottom span-right")
        XCTAssertEqual(PositionArea.region(-2, 0, anchor: anchor, in: block), CGRect(x: 0, y: 0, width: 150, height: 600), "left")
        XCTAssertEqual(PositionArea.region(3, 3, anchor: anchor, in: block), anchor, "center: the anchor's cell")
        let past = CGRect(x: 380, y: 100, width: 40, height: 20)
        XCTAssertEqual(PositionArea.region(-1, 2, anchor: past, in: block), CGRect(x: 0, y: 120, width: 420, height: 480), "span-left reaches the anchor past the block")
        XCTAssertEqual(PositionArea.region(1, 2, anchor: past, in: block), CGRect(x: 380, y: 120, width: 40, height: 480))
    }

    /// `place` measures each option in its region and takes the first whose
    /// margin box fits it: a box that narrows to fit its base keeps it, where
    /// one laid out once at the block's width would flip; the box is the
    /// region's fit-content, as `measure` answers.
    func testPlaceMeasuresEachOptionInItsRegion() {
        let block = CGRect(x: 0, y: 0, width: 400, height: 600), anchor = CGRect(x: 300, y: 100, width: 40, height: 20)
        // Six 50x18 boxes in a wrapping row: fit-content in the region's width.
        func measure(_ region: CGSize) -> PositionArea.Measured {
            let across = max(1, min(6, Int(region.width / 50)))
            return .init(size: CGSize(width: CGFloat(across * 50), height: CGFloat((6 + across - 1) / across * 18)))
        }
        var history = PositionArea.History(), asked: [CGSize] = []
        let placed = PositionArea.place("bottom span-right", anchor: anchor, fallbacks: "flip-inline", history: &history, in: block) {
            asked.append($0); return measure($0)
        }
        XCTAssertEqual(placed, PositionArea.Placement(origin: CGPoint(x: 300, y: 120), size: CGSize(width: 100, height: 54),
                                                      region: CGRect(x: 300, y: 120, width: 100, height: 480)), "the base, narrowed")
        XCTAssertEqual(asked, [block.size, CGSize(width: 100, height: 480)], "the whole block (the history's), then the base, which fits: no other option measured")
        XCTAssertEqual(history.choice, 0)
        // Its min-content wider than the base's region: the flip, measured in its own.
        let past = CGRect(x: 380, y: 100, width: 40, height: 20)
        let flipped = PositionArea.place("bottom span-right", anchor: past, fallbacks: "flip-inline", history: &history, in: block, measure: measure)
        XCTAssertEqual(flipped.region, CGRect(x: 0, y: 120, width: 420, height: 480))
        XCTAssertEqual(flipped.origin, CGPoint(x: 120, y: 120), "spanning left from the anchor's end")
        XCTAssertEqual(history.choice, 1)
    }

    /// A box that changes size starts its options again, as `origin`'s does:
    /// one that flipped at 150 wide fits its base again at 80 (Astra round 1).
    func testPlaceStartsAgainWhenTheBoxChangesSize() {
        let block = CGRect(x: 0, y: 0, width: 400, height: 600), anchor = CGRect(x: 300, y: 100, width: 40, height: 20)
        var history = PositionArea.History()
        func at(_ width: CGFloat) -> PositionArea.Placement {
            PositionArea.place("bottom span-right", anchor: anchor, fallbacks: "flip-inline", history: &history, in: block) { _ in
                .init(size: CGSize(width: width, height: 20))
            }
        }
        XCTAssertEqual(at(150).origin, CGPoint(x: 190, y: 120), "flipped")
        XCTAssertEqual(history.choice, 1)
        XCTAssertEqual(at(80).origin, CGPoint(x: 300, y: 120), "the base again")
        XCTAssertEqual(history.choice, 0)
    }

    /// Each option flips the margins the layout resolved: `flip-start` makes
    /// a top margin a left one, so a 120-point box with a 100-point top
    /// margin fits the flipped region as 220 by 120 (Astra round 1); and a
    /// percentage margin is what the layout made of it, an offset and no
    /// part of the box.
    func testPlaceFlipsTheMarginsTheLayoutResolved() {
        let block = CGRect(x: 0, y: 0, width: 400, height: 600)
        var history = PositionArea.History()
        let flipped = PositionArea.place("bottom span-right", anchor: CGRect(x: 100, y: 450, width: 40, height: 20),
                                         margins: .init(top: 100), fallbacks: "flip-start", history: &history, in: block) { _ in
            .init(size: CGSize(width: 120, height: 120), margins: .init(top: 100))
        }
        XCTAssertEqual(history.choice, 1)
        XCTAssertEqual(flipped, PositionArea.Placement(origin: CGPoint(x: 240, y: 450), size: CGSize(width: 120, height: 120),
                                                       region: CGRect(x: 140, y: 450, width: 260, height: 150)))
        var fresh = PositionArea.History()
        let percent = PositionArea.place("bottom span-right", anchor: CGRect(x: 150, y: 100, width: 40, height: 20),
                                         history: &fresh, in: block) { region in
            .init(size: CGSize(width: 100, height: 20), margins: .init(left: region.width / 10))
        }
        XCTAssertEqual(percent.origin, CGPoint(x: 175, y: 120), "10% of the 250-point region")
        XCTAssertEqual(percent.size, CGSize(width: 100, height: 20))
    }

    /// The chosen option keeps the scroll it was chosen at, as CSS remembers
    /// it: a scroll since moves the box with its anchor and does not size it
    /// again (WPT position-area-scrolling-002: a 50% box stays its size);
    /// when the moved box no longer fits, the next option is chosen where the
    /// anchor is now (Astra round 3).
    func testPlaceMovesTheChosenOptionWithAScroll() {
        let block = CGRect(x: 0, y: 0, width: 400, height: 600)
        var history = PositionArea.History()
        func at(_ scroll: CGPoint) -> PositionArea.Placement {
            // The anchor at (100, 100) unscrolled, moved up and left by the scroll.
            let anchor = CGRect(x: 100 - scroll.x, y: 100 - scroll.y, width: 40, height: 20)
            return PositionArea.place("bottom span-right", anchor: anchor, fallbacks: "flip-block", scroll: scroll, history: &history, in: block) { region in
                .init(size: CGSize(width: region.width / 2, height: region.height / 2))
            }
        }
        XCTAssertEqual(at(.zero), PositionArea.Placement(origin: CGPoint(x: 100, y: 120), size: CGSize(width: 150, height: 240),
                                                         region: CGRect(x: 100, y: 120, width: 300, height: 480)))
        let scrolled = at(CGPoint(x: 40, y: 60))
        XCTAssertEqual(scrolled.origin, CGPoint(x: 60, y: 60), "moved with the anchor")
        XCTAssertEqual(scrolled.size, CGSize(width: 150, height: 240), "not sized again")
        XCTAssertEqual(scrolled.region.size, CGSize(width: 300, height: 480), "in the region it was chosen in")
        XCTAssertEqual(history.choice, 0)
        // Scrolled back past where the moved box fits below: the flip, sized where the anchor is.
        let flipped = at(CGPoint(x: 0, y: -300))
        XCTAssertEqual(history.choice, 1)
        XCTAssertEqual(flipped, PositionArea.Placement(origin: CGPoint(x: 100, y: 200), size: CGSize(width: 150, height: 200),
                                                       region: CGRect(x: 100, y: 0, width: 300, height: 400)))
        XCTAssertEqual(history.scroll, CGPoint(x: 0, y: -300), "the scroll it was chosen at")
        // No fallback: the box keeps its size and follows its anchor out of the window, aligned
        // where it was chosen, then moved (Astra's round 3 re-check).
        var alone = PositionArea.History()
        for scroll in [CGPoint.zero, CGPoint(x: 0, y: -300)] {
            let anchor = CGRect(x: 100 - scroll.x, y: 100 - scroll.y, width: 40, height: 20)
            let placed = PositionArea.place("bottom span-right", anchor: anchor, fallbacks: "none", scroll: scroll, history: &alone, in: block) { region in
                .init(size: CGSize(width: region.width / 2, height: region.height / 2))
            }
            XCTAssertEqual(placed.origin, CGPoint(x: 100, y: 120 - scroll.y))
            XCTAssertEqual(placed.size, CGSize(width: 150, height: 240))
        }
        XCTAssertEqual(alone.choice, 0)
        // Held at a scrolling page's start edge, it scrolls with the page (the re-check's case).
        var page = PositionArea.History()
        for scroll in [CGPoint.zero, CGPoint(x: 0, y: 20)] {
            let anchor = CGRect(x: 100, y: 10 - scroll.y, width: 40, height: 20)
            let document = CGRect(x: 0, y: -scroll.y, width: 400, height: 2000)
            let placed = PositionArea.place("top span-right", anchor: anchor, fallbacks: "none", overflow: .absolute(page: document), scroll: scroll, pageScroll: scroll,
                                            history: &page, in: block) { _ in .init(size: CGSize(width: 50, height: 100)) }
            XCTAssertEqual(placed.origin.y, -scroll.y, "at the page's top, as it scrolls")
        }
        // An anchor in a scroller of its own, the page still: aligned as chosen, it follows the anchor.
        var inner = PositionArea.History()
        for scroll in [CGPoint.zero, CGPoint(x: 0, y: 20)] {
            let anchor = CGRect(x: 100, y: 60 - scroll.y, width: 40, height: 20)
            let placed = PositionArea.place("top span-right", anchor: anchor, fallbacks: "none", overflow: .absolute(page: CGRect(x: 0, y: 0, width: 400, height: 2000)),
                                            scroll: scroll, history: &inner, in: block) { _ in .init(size: CGSize(width: 50, height: 100)) }
            XCTAssertEqual(placed.origin.y, -scroll.y, "moved with the anchor")
        }
    }

    /// A second placement in the same batch measures nothing; a new batch
    /// measures again, as does a moved layout stamp (a layout whose batch was
    /// empty), and a refusal is asked again (theme2's cache test).
    func testMeasuresAreKeptForOneBatch() {
        var cache = PositionArea.MeasureCache(), asked = 0
        let region = CGSize(width: 100, height: 480)
        func measure(_ batch: Int, stamp: UInt64 = 1, _ view: UInt32 = 7, _ size: CGSize = region, refuse: Bool = false) -> PositionArea.Measured? {
            cache.measured(view, size, batch: batch, stamp: stamp) {
                asked += 1
                return refuse ? nil : .init(size: CGSize(width: 100, height: 54))
            }
        }
        XCTAssertEqual(measure(1), .init(size: CGSize(width: 100, height: 54)))
        XCTAssertEqual(measure(1), .init(size: CGSize(width: 100, height: 54)))
        XCTAssertEqual(asked, 1, "the same batch: measured once")
        _ = measure(1, 8); _ = measure(1, 7, CGSize(width: 120, height: 480))
        XCTAssertEqual(asked, 3, "another view, another region: each its own")
        _ = measure(2)
        XCTAssertEqual(asked, 4, "a new batch measures again")
        _ = measure(2, stamp: 2)
        XCTAssertEqual(asked, 5, "a moved stamp in the same batch measures again")
        XCTAssertNil(measure(3, refuse: true)); XCTAssertNil(measure(3, refuse: true))
        XCTAssertEqual(asked, 7, "a refusal is not kept")
    }

    /// LLP 1021 D2: a box asked for at a point sits there; on an axis
    /// without room it goes the other way from the point, as the web's sheet
    /// opens it; where neither fits it is held in the bounds.
    func testABoxAtAPointGoesTheOtherWayWhereItDoesNotFit() {
        let bounds = CGRect(x: 0, y: 0, width: 400, height: 300), size = CGSize(width: 120, height: 80)
        XCTAssertEqual(PositionArea.at(CGPoint(x: 30, y: 8), size: size, in: bounds), CGPoint(x: 30, y: 8))
        XCTAssertEqual(PositionArea.at(CGPoint(x: 390, y: 290), size: size, in: bounds), CGPoint(x: 270, y: 210), "up and to the left of it")
        XCTAssertEqual(PositionArea.at(CGPoint(x: 390, y: 8), size: size, in: bounds), CGPoint(x: 270, y: 8), "only the axis without room")
        XCTAssertEqual(PositionArea.at(CGPoint(x: 100, y: 40), size: CGSize(width: 120, height: 280), in: bounds), CGPoint(x: 100, y: 20), "neither way: held in")
    }
}

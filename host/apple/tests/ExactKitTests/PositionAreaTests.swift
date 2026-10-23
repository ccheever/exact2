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
                       CGPoint(x: 280, y: 510), "no area: the web's default flips above and leftward, off its invoker, margins flipped too")
    }

    /// No authored area takes the web's UA default (neutral.css): below from
    /// the inline start, flipping above where that doesn't fit, so a popover
    /// at the bottom-left (Harmony's emoji button) opens above its invoker
    /// instead of being held over it; when nothing fits it is shifted into
    /// the bounds, as an authored area is.
    func testNoAreaFlipsAsTheWebsDefault() {
        let emoji = CGRect(x: 12, y: 560, width: 30, height: 30), picker = CGSize(width: 300, height: 200)
        XCTAssertEqual(PositionArea.origin("none", anchor: emoji, size: picker, in: layer), CGPoint(x: 12, y: 360), "above, from its left edge")
        XCTAssertEqual(PositionArea.origin("none", anchor: emoji, size: picker, rtl: true, in: layer), CGPoint(x: 12, y: 360),
                       "right to left: spanning left from its inline start doesn't fit, so it flips inline too")
        let huge = CGSize(width: 300, height: 700)
        XCTAssertEqual(PositionArea.origin("none", anchor: emoji, size: huge, in: layer).y, 0, "nothing fits: held in the bounds from the top")
        XCTAssertEqual(PositionArea.origin("none", anchor: anchor, size: size, in: layer), CGPoint(x: 100, y: 230), "room below: as before")
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
}

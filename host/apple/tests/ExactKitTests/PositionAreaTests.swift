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

    /// CSS shifts an anchor-centred axis; the native default retains its clamp.
    func testCentredAxesShiftButAnAuthoredEdgeCanOverflow() {
        let high = CGRect(x: 0, y: 20, width: 40, height: 30)
        XCTAssertEqual(PositionArea.origin("top", anchor: high, size: size, margins: .init(bottom: 12, left: 6), in: layer),
                       CGPoint(x: 6, y: -52), "only the centred horizontal axis shifts into the viewport")
        let low = CGRect(x: 380, y: 570, width: 20, height: 30)
        XCTAssertEqual(PositionArea.origin("none", anchor: low, size: size, margins: .init(right: 5, bottom: 5), in: layer),
                       CGPoint(x: 275, y: 535), "held at the bottom and right edges, its margins inside them")
    }
    func testOrderedFlipsAtEveryEdgeAndNoFittingFallback() {
        let viewport = CGRect(x: 0, y: 0, width: 420, height: 600)
        let box = CGSize(width: 322, height: 122)
        let tries = "flip-block, flip-inline, flip-block flip-inline"
        func at(_ area: String, _ anchor: CGRect, _ fallback: String = "none", _ bounds: CGRect? = nil) -> CGPoint {
            PositionArea.origin(area, anchor: anchor, size: box, fallbacks: fallback, in: bounds ?? viewport)
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
                                          margins: .init(left: 110), fallbacks: "flip-block flip-inline", in: viewport)
        XCTAssertEqual(tooWide, CGPoint(x: 442, y: 592), "the border box alone fitting is insufficient")
    }

    func testSuccessfulChoiceSurvivesResizeAndResetsWithTheStyle() {
        var history = PositionArea.History()
        let low = CGRect(x: 332, y: 560, width: 80, height: 32)
        let size = CGSize(width: 322, height: 122)
        let tries = "flip-block, flip-inline, flip-block flip-inline"
        func at(_ width: CGFloat, _ height: CGFloat, _ list: String) -> CGPoint {
            PositionArea.origin("bottom span-right", anchor: low, size: size, fallbacks: list, history: &history,
                                in: CGRect(x: 0, y: 0, width: width, height: height))
        }
        XCTAssertEqual(at(420, 600, tries), CGPoint(x: 90, y: 438))
        XCTAssertEqual(at(800, 800, tries), CGPoint(x: 90, y: 438))
        XCTAssertEqual(at(100, 100, tries), CGPoint(x: 90, y: 438), "if every choice overflows, retain the current one")
        XCTAssertEqual(at(800, 800, "none"), CGPoint(x: 332, y: 592), "a changed list clears the remembered option")
    }

}

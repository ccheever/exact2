import CoreGraphics
import XCTest
@testable import ExactKit

/// LLP 1021 §5, "Placement": the painted top layers' `position-area`, the
/// margin box aligned to the invoker as CSS aligns it, then clamped.
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

    /// The margin box is clamped into the layer, never flipped.
    func testTheMarginBoxIsClampedToTheLayer() {
        let high = CGRect(x: 0, y: 20, width: 40, height: 30)
        XCTAssertEqual(PositionArea.origin("top", anchor: high, size: size, margins: .init(bottom: 12, left: 6), in: layer),
                       CGPoint(x: 6, y: 0), "held at the top and left edges, inside its margins")
        let low = CGRect(x: 380, y: 570, width: 20, height: 30)
        XCTAssertEqual(PositionArea.origin("none", anchor: low, size: size, margins: .init(right: 5, bottom: 5), in: layer),
                       CGPoint(x: 275, y: 535), "held at the bottom and right edges, its margins inside them")
    }
}

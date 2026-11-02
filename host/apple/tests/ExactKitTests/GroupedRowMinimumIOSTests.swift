#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

final class GroupedRowMinimumIOSTests: GroupedListFixture {
    /// A row UIKit's cell configuration sizes keeps the `min-height` its author
    /// wrote (the only one the host sends, `kept_geometry`), and follows it as it
    /// changes: of the whole row under the sheet's `border-box`, of its content
    /// under an authored `content-box`, its padding added.
    func testAStandardRowKeepsItsAuthoredMinimumHeight() throws {
        let p = presenter { self.model() }
        let natural = try cell(p, 11).bounds.height
        func minimum(_ points: Double?, sizing: String = "border-box") {
            var style: [String: Any] = ["text_color": [0, 0, 0, 255], "box_sizing": sizing, "padding_top": 10.0, "padding_bottom": 10.0]
            if let points { style["min_height"] = points }
            p.apply(wireBatch([["op": "style", "id": 11, "style": style]]))
        }
        minimum(natural + 20)
        XCTAssertEqual(try cell(p, 11).bounds.height, natural + 20, accuracy: 0.5)
        minimum(natural + 30)
        XCTAssertEqual(try cell(p, 11).bounds.height, natural + 30, accuracy: 0.5, "a changed minimum reaches the shown cell")
        minimum(natural + 30, sizing: "content-box")
        XCTAssertEqual(try cell(p, 11).bounds.height, natural + 50, accuracy: 0.5, "content-box: the padding adds to it")
        minimum(nil)
        XCTAssertEqual(try cell(p, 11).bounds.height, natural, accuracy: 0.5, "a removed one gives UIKit's height back")
    }

    /// Under `content-box` a percentage padding added to the minimum is of
    /// the row's containing block's width, its card's, not the row's.
    func testAContentBoxMinimumAddsAPercentagePaddingOfTheCardsWidth() throws {
        let p = presenter { self.model() }
        let natural = try cell(p, 11).bounds.height
        p.apply(wireBatch([
            ["op": "frame", "id": 4, "x": 16.0, "y": 0.0, "w": 370.0, "h": 200.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "style", "id": 11, "style": ["text_color": [0, 0, 0, 255], "box_sizing": "content-box", "min_height": natural + 20,
                                               "padding_top": ["pct": 10], "padding_bottom": ["pct": 10]]],
        ]))
        XCTAssertEqual(try cell(p, 11).bounds.height, natural + 20 + 74, accuracy: 0.5, "10% of the card's 370 above and below")
    }
}
#endif

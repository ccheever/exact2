#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// LLP 1121 stage 9 B (D10) in a standard grouped row, which draws the row's
/// symbol as the cell's image: untinted, a colour its author wrote is its
/// tint, as the same row's image is on the web and the Mac; a written tint
/// still wins, a destructive row's red goes over it, and a change of the
/// colour configures the row again.
final class SymbolColorGroupedIOSTests: GroupedListFixture {
    private func rgb(_ p: Presenter, _ id: UInt32) throws -> [CGFloat]? {
        guard let tint = (try cell(p, id).contentConfiguration as? UIListContentConfiguration)?.imageProperties.tintColor else { return nil }
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        tint.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b, a]
    }

    func testAWrittenColourTintsARowsSymbol() throws {
        let p = presenter { self.model() }
        let symbol: [String: Any] = ["imageSource": "symbol:sf/person.circle", "symbolColorWritten": "1"]
        p.apply(wireBatch([
            ["op": "create", "id": 31, "kind": "image", "props": symbol, "style": ["text_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 33, "kind": "text", "props": ["text": "Profile"], "style": [:]],
            ["op": "children", "id": 10, "ids": [31, 33]],
        ]))
        XCTAssertEqual(try rgb(p, 10), [1, 0, 0, 1], "the written colour")
        p.apply(wireBatch([["op": "style", "id": 31, "style": ["text_color": [0, 0, 255, 255]]]]))
        XCTAssertEqual(try rgb(p, 10), [0, 0, 1, 1], "a new colour configures the row again")
        p.apply(wireBatch([["op": "style", "id": 31, "style": ["text_color": [0, 0, 255, 255], "tint_color": [0, 255, 0, 255]]]]))
        XCTAssertEqual(try rgb(p, 10), [0, 1, 0, 1], "a written tint wins")
        p.apply(wireBatch([["op": "style", "id": 31, "style": ["text_color": [0, 0, 255, 255]]],
                           ["op": "props", "id": 31, "set": [:], "clear": ["symbolColorWritten"]]]))
        XCTAssertNil(try rgb(p, 10), "unwritten: UIKit's")
    }
}
#endif

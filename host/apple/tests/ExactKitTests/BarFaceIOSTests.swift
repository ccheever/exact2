#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A header button's face as its author painted it (`ChipFace`, LLP 1115
/// §2): a dot beside a label, or a label in the author's font, is the bar
/// item's drawn image; a plain label is UIKit's title, the platform's.
final class BarFaceIOSTests: XCTestCase {
    private func button(_ children: [[String: Any]]) -> (Presenter, NodeView) {
        let p = Presenter()
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"], "props": [:], "style": ["text_color": [0, 0, 0, 255]]],
            ["op": "create", "id": 2, "kind": "view", "handlers": [], "props": [:], "style": ["column_gap": 5]]]
        ops += children
        ops += [["op": "children", "id": 2, "ids": children.map { $0["id"]! }], ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        return (p, p.views[1]!)
    }
    private let dot: [String: Any] = ["op": "create", "id": 3, "kind": "view", "handlers": [], "props": [:],
                                      "style": ["background_color": [52, 199, 89, 255], "width": 7, "height": 7]]
    private func label(_ style: [String: Any]) -> [String: Any] {
        ["op": "create", "id": 4, "kind": "text", "handlers": [], "props": ["text": "Clock"],
         "style": ["text_color": [142, 142, 147, 255], "font_size": 12].merging(style) { $1 }]
    }

    func testADotBesideALabelIsDrawnAsPainted() throws {
        let (p, node) = button([dot, label([:])])
        let face = try XCTUnwrap(ChipFace(node, engine: TextEngine(resolve: { _ in nil })))
        withExtendedLifetime(p) {}
        XCTAssertEqual(face.parts.count, 2)
        guard case .dot(let size, _, _) = face.parts[0], case .text(let text, let font, let color) = face.parts[1] else { return XCTFail("\(face.parts)") }
        XCTAssertEqual(size, CGSize(width: 7, height: 7))
        XCTAssertEqual(text, "Clock")
        XCTAssertEqual(font.pointSize, 12)
        XCTAssertEqual(color[0], [142, 142, 147, 255])
        XCTAssertEqual(face.gap, 5)
        let image = face.image
        XCTAssertEqual(image.renderingMode, .alwaysOriginal)
        XCTAssertGreaterThan(image.size.width, 7 + 5)
    }

    func testALabelInTheAuthorsFontIsDrawnAndAPlainOneIsTheBarsTitle() {
        let engine = TextEngine(resolve: { _ in nil })
        // The presenter is the views' owner: kept alive while the face is read.
        let (fonted, own) = button([label(["written": ["font_size"]])])
        XCTAssertNotNil(ChipFace(own, engine: engine), "its own font")
        let (plain, page) = button([label([:])])
        XCTAssertNil(ChipFace(page, engine: engine), "the page's type: UIKit's title")
        withExtendedLifetime((fonted, plain)) {}
    }
}
#endif

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
                                      "style": ["background_color": [52, 199, 89, 255], "width": 14, "height": 6]]
    private func label(_ style: [String: Any]) -> [String: Any] {
        ["op": "create", "id": 4, "kind": "text", "handlers": [], "props": ["text": "Clock"],
         "style": ["text_color": [142, 142, 147, 255], "font_size": 12].merging(style) { $1 }]
    }

    func testADotBesideALabelIsDrawnAsPainted() throws {
        let (p, node) = button([dot, label(["written": ["text_color"]])])
        let face = try XCTUnwrap(ChipFace(node, engine: TextEngine(resolve: { _ in nil })))
        withExtendedLifetime(p) {}
        XCTAssertEqual(face.parts.count, 2)
        guard case .dot(let size, _, _) = face.parts[0], case .text(let text, let font, let color) = face.parts[1] else { return XCTFail("\(face.parts)") }
        XCTAssertEqual(size, CGSize(width: 14, height: 6), "the authored size the host sends (kept_geometry)")
        XCTAssertEqual(text, "Clock")
        XCTAssertEqual(font.pointSize, 12)
        XCTAssertEqual(color?[0], [142, 142, 147, 255], "the colour its author wrote")
        XCTAssertEqual(face.gap, 5)
        let image = face.image(tint: .systemBlue)
        XCTAssertEqual(image.renderingMode, .alwaysOriginal)
        XCTAssertGreaterThan(image.size.width, 14 + 5)
    }

    /// A colour only inherited from the page is unsaid: the bar's ink, as a
    /// title's (LLP 1115 D4), never the page's grey or red.
    func testAnInheritedColourIsTheBarsInk() throws {
        let (p, node) = button([dot, label(["written": ["font_size"]])])
        let face = try XCTUnwrap(ChipFace(node, engine: TextEngine(resolve: { _ in nil })))
        withExtendedLifetime(p) {}
        guard case .text(_, _, let color) = face.parts[1] else { return XCTFail("\(face.parts)") }
        XCTAssertNil(color, "inherited: unsaid")
    }

    /// Bold Text weights the system font, an authored weight included (LLP
    /// 1115.000 D12.3): the kernel's weights say it, so the face draws the
    /// weight its rows give (600 where Bold Text took a 400), and adds no
    /// step of its own; its source says the setting, so the bar draws it
    /// again when the setting changes.
    func testBoldTextIsTheWeightTheRowsGive() throws {
        let engine = TextEngine(resolve: { _ in nil })
        let (p, node) = button([dot, label(["written": ["font_size"]])])
        let plain = try XCTUnwrap(ChipFace(node, engine: engine))
        node.traitOverrides.legibilityWeight = .bold
        node.updateTraitsIfNeeded()
        let bold = try XCTUnwrap(ChipFace(node, engine: engine))
        let (q, heavy) = button([dot, label(["written": ["font_size"], "font_weight": 600])])
        let rows = try XCTUnwrap(ChipFace(heavy, engine: engine))
        withExtendedLifetime((p, q)) {}
        guard case .text(_, let regular, _) = plain.parts[1], case .text(_, let same, _) = bold.parts[1],
              case .text(_, let heavier, _) = rows.parts[1] else { return XCTFail() }
        func width(_ f: UIFont) -> CGFloat { ("Clock" as NSString).size(withAttributes: [.font: f]).width }
        XCTAssertEqual(width(same), width(regular), "no step of the face's own")
        XCTAssertGreaterThan(width(heavier), width(regular), "the rows' heavier weight sets wider")
        XCTAssertNotEqual(plain.source, bold.source, "the bar draws it again")
    }

    func testALabelInTheAuthorsFontIsDrawnAndAPlainOneIsTheBarsTitle() {
        let engine = TextEngine(resolve: { _ in nil })
        // The presenter is the views' owner: kept alive while the face is read.
        let (fonted, own) = button([label(["written": ["font_size"]])])
        let (unsized, bare) = button([["op": "create", "id": 3, "kind": "view", "handlers": [], "props": [:], "style": ["background_color": [52, 199, 89, 255]]], label([:])])
        XCTAssertNil(ChipFace(bare, engine: engine), "a box with no authored size draws nothing, as in CSS")
        XCTAssertNotNil(ChipFace(own, engine: engine), "its own font")
        let (plain, page) = button([label([:])])
        XCTAssertNil(ChipFace(page, engine: engine), "the page's type: UIKit's title")
        withExtendedLifetime((fonted, plain, unsized)) {}
    }

    /// Each part is drawn at its authored opacity (a view's alpha), and the
    /// face's source says it, so the bar draws it again when it changes.
    func testAPartsOpacityIsDrawnAndInTheSource() throws {
        let (p, node) = button([dot, label(["written": ["font_size"]])])
        let engine = TextEngine(resolve: { _ in nil })
        let shown = try XCTUnwrap(ChipFace(node, engine: engine))
        try XCTUnwrap(p.views[4]).alpha = 0
        let faded = try XCTUnwrap(ChipFace(node, engine: engine))
        withExtendedLifetime(p) {}
        XCTAssertEqual(shown.opacities.last, 1)
        XCTAssertEqual(faded.opacities.last, 0, "opacity 0: drawn clear, its room kept")
        XCTAssertNotEqual(shown.source, faded.source)
    }
}
#endif

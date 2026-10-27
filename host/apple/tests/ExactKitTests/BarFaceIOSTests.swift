#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A header button's face as its author painted it (`ChipFace`, LLP 1115
/// §2): a dot beside a label, or a label in the author's font, is the bar
/// item's drawn image; a plain label is UIKit's title, the platform's.
final class BarFaceIOSTests: XCTestCase {
    private func button(_ children: [[String: Any]], in p: Presenter = Presenter()) -> (Presenter, NodeView) {
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

    func testABadgeIsDrawnForThePagesSchemeAtTheSamePointSize() throws {
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "button", "props": ["testId": "badge"]],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [[255, 0, 0, 255], [0, 0, 255, 255]]]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "A"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
        ]))
        let box = try XCTUnwrap(p.views[2])
        box.traitOverrides.userInterfaceStyle = .light
        box.updateTraitsIfNeeded()
        let light = try XCTUnwrap(BadgeFace(box: box))
        box.traitOverrides.userInterfaceStyle = .dark
        box.updateTraitsIfNeeded()
        let dark = try XCTUnwrap(BadgeFace(box: box))
        XCTAssertNotEqual(light.source, dark.source, "a scheme change redraws the bar item's face")
        XCTAssertEqual(light.image.size, CGSize(width: 36, height: 36))
        XCTAssertEqual(dark.image.size, light.image.size)
        let pixel = try XCTUnwrap(SharePictureCase.rgba(try XCTUnwrap(dark.image.cgImage), x: 4, y: 4))
        XCTAssertGreaterThan(pixel.2, 240, "the installed image itself is blue in dark mode")
        XCTAssertLessThan(pixel.0, 15)
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

    func testASymbolOnlyUsesColourWrittenInsideTheBarButton() throws {
        let symbol: [String: Any] = ["op": "create", "id": 5, "kind": "image",
            "props": ["imageSource": "symbol:sf/star", "symbolName": "star"],
            "style": ["text_color": [240, 40, 120, 255]]]
        let (p, node) = button([dot, symbol])
        defer { p.reset() }
        let engine = TextEngine(resolve: { _ in nil })
        func colour() throws -> [[Double]]? {
            let face = try XCTUnwrap(ChipFace(node, engine: engine))
            guard case .symbol(_, _, let ink) = face.parts[1] else { XCTFail(); return nil }
            return ink
        }
        XCTAssertNil(try colour(), "computed page ink leaves the bar's ink")
        p.apply(wireBatch([["op": "style", "id": 5, "style": ["text_color": [240, 40, 120, 255], "written": ["text_color"]]]]))
        XCTAssertEqual(try colour()?[0], [240, 40, 120, 255])
        p.apply(wireBatch([["op": "style", "id": 5, "style": ["text_color": [240, 40, 120, 255]]],
                           ["op": "style", "id": 1, "style": ["text_color": [240, 40, 120, 255], "written": ["text_color"]]]]))
        XCTAssertEqual(try colour()?[0], [240, 40, 120, 255], "the enclosing button's authored colour applies")
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

    /// A native button's title size and weight, written on it or on its
    /// text, are its bar item's title font (LLP 1069.011.001 D4): the "●
    /// Clock" glass chip at 12 pt, in every state UIKit draws it in. A
    /// native button whose face says nothing of its font leaves UIKit's.
    func testANativeButtonsAuthoredFontIsItsBarTitlesFont() throws {
        let p = Presenter()
        defer { p.reset() }
        var clock = ButtonFace()
        clock.title = "● Clock"
        clock.style = "glass"
        clock.rows.title = ["font_size": .number(12)]
        var heavy = ButtonFace()
        heavy.title = "Done"
        heavy.rows.title = ["font_weight": .number(600)]
        var plain = ButtonFace()
        plain.title = "Edit"
        p.buttonFace = { [1: clock, 2: heavy][$0] ?? plain }
        let header = NodeView(id: 100, kind: "view", presenter: p)
        header.props["semanticTag"] = "header"
        let buttons = (1...3).map { id -> NodeView in
            let button = NodeView(id: UInt32(id), kind: "control", presenter: p)
            button.props["type"] = "button"
            header.container.addSubview(button)
            return button
        }
        let shape = buttons.map { HeaderShape.Item($0) }
        XCTAssertEqual(shape.map(\.title), ["● Clock", "Done", "Edit"])
        XCTAssertTrue(shape[0].ownGlass)
        let items = p.navigation.barItems(shape, RouteController(header)).filter { $0.title != nil }
        XCTAssertEqual(items.count, 3)
        let unsaid = HeaderShape.Item.titleFont(bold: false)
        for state: UIControl.State in [.normal, .highlighted, .disabled] {
            let font = try XCTUnwrap(items[0].titleTextAttributes(for: state)?[.font] as? UIFont, "state \(state.rawValue)")
            XCTAssertEqual(font.pointSize, 12, "the author's size")
            XCTAssertEqual(TextEngine.controlWeight(font), TextEngine.controlWeight(unsaid), "the bar's weight, unsaid")
        }
        let done = try XCTUnwrap(items[1].titleTextAttributes(for: .normal)?[.font] as? UIFont)
        XCTAssertEqual(done.pointSize, unsaid.pointSize, "a size left unsaid is UIKit's")
        XCTAssertEqual(TextEngine.controlWeight(done), 600, "the author's weight")
        XCTAssertNil(items[2].titleTextAttributes(for: .normal), "nothing written: UIKit's title")
        clock.rows.title = ["font_size": .number(13)]
        p.apply(facesBatch())
        XCTAssertNotEqual(HeaderShape.Item(buttons[0]).source, shape[0].source, "a new size makes the item again")
        heavy.rows.title = ["font_weight": .number(650)]
        p.apply(facesBatch())
        XCTAssertNotEqual(HeaderShape.Item(buttons[1]).source, shape[1].source, "and so does a new weight in the same face")
    }

    /// An unchanged chip item is drawn again for each projection before iOS
    /// 26, where its picture is in the bar's tint, which the item does not
    /// hold; from iOS 26, drawn in the label colour, it is kept as drawn. A
    /// chip whose face changes is drawn again. A session's presenter, whose
    /// text engine draws the face.
    func testAKeptChipItemIsDrawnAgainOnlyInTheBarsTint() throws {
        let session = ExactApp.shared.makeSession(label: "kept-chip")
        defer { session.destroy() }
        let (p, node) = button([dot, label(["written": ["text_color"]])], in: session.presenter)
        let header = NodeView(id: 100, kind: "view", presenter: p)
        header.props["semanticTag"] = "header"
        header.container.addSubview(node)
        let route = RouteController(header)
        XCTAssertNotNil(HeaderShape.Item(node).chip)
        let item = try XCTUnwrap(p.navigation.barItems([HeaderShape.Item(node)], route).first)
        let first = try XCTUnwrap(item.image)
        let kept = try XCTUnwrap(p.navigation.barItems([HeaderShape.Item(node)], route).first)
        XCTAssertTrue(kept === item, "the item is changed in place")
        if #available(iOS 26.0, *) { XCTAssertTrue(kept.image === first, "its face kept as drawn") }
        else { XCTAssertFalse(kept.image === first, "its face drawn again in the tint") }
        let shown = try XCTUnwrap(kept.image)
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["background_color": [255, 59, 48, 255], "width": 14, "height": 6]]]))
        let recoloured = try XCTUnwrap(p.navigation.barItems([HeaderShape.Item(node)], route).first)
        XCTAssertTrue(recoloured === item)
        XCTAssertFalse(recoloured.image === shown, "a new dot colour: its face drawn again")
    }

    /// A kept item whose author removes its font has UIKit's again, in
    /// every state, and is still the same item.
    func testAKeptItemLosesARemovedFont() throws {
        let p = Presenter()
        defer { p.reset() }
        var done = ButtonFace()
        done.title = "Done"
        done.rows.title = ["font_size": .number(12)]
        p.buttonFace = { _ in done }
        let header = NodeView(id: 100, kind: "view", presenter: p)
        header.props["semanticTag"] = "header"
        let button = NodeView(id: 1, kind: "control", presenter: p)
        button.props["type"] = "button"
        header.container.addSubview(button)
        let route = RouteController(header)
        let item = try XCTUnwrap(p.navigation.barItems([HeaderShape.Item(button)], route).first)
        XCTAssertEqual((item.titleTextAttributes(for: .normal)?[.font] as? UIFont)?.pointSize, 12)
        done.rows.title = [:]
        p.apply(facesBatch())
        let kept = try XCTUnwrap(p.navigation.barItems([HeaderShape.Item(button)], route).first)
        XCTAssertTrue(kept === item, "the item is changed in place")
        for state: UIControl.State in [.normal, .highlighted, .disabled, .selected, .focused] {
            XCTAssertNil(kept.titleTextAttributes(for: state)?[.font], "state \(state.rawValue)")
        }
    }

    /// Under Bold Text an authored weight is the kernel's, stepped already
    /// (an authored 400 arrives as 600) and drawn as it comes; a size alone
    /// keeps the bar's weight, which Bold Text makes heavier, as it does
    /// UIKit's own titles beside it.
    func testBoldTextKeepsTheRowsWeightAndStepsTheBarsOwn() throws {
        let p = Presenter()
        defer { p.reset() }
        var sized = ButtonFace()
        sized.title = "● Clock"
        sized.rows.title = ["font_size": .number(12)]
        var weighted = ButtonFace()
        weighted.title = "Edit"
        weighted.rows.title = ["font_weight": .number(600)]
        p.buttonFace = { $0 == 1 ? sized : weighted }
        let buttons = (1...2).map { id -> NodeView in
            let button = NodeView(id: UInt32(id), kind: "control", presenter: p)
            button.props["type"] = "button"
            button.traitOverrides.legibilityWeight = .bold
            button.updateTraitsIfNeeded()
            return button
        }
        let clock = try XCTUnwrap(HeaderShape.Item(buttons[0]).font)
        XCTAssertEqual(clock.pointSize, 12)
        XCTAssertEqual(TextEngine.controlWeight(clock), TextEngine.controlWeight(HeaderShape.Item.titleFont(bold: true)))
        XCTAssertGreaterThan(TextEngine.controlWeight(clock), TextEngine.controlWeight(HeaderShape.Item.titleFont(bold: false)))
        let edit = try XCTUnwrap(HeaderShape.Item(buttons[1]).font, "an authored 400, stepped, is the author's")
        XCTAssertEqual(TextEngine.controlWeight(edit), 600)
    }

    /// The font a row left unsaid keeps (`titleFont`) is the one UIKit
    /// draws a plain bar item's title in, read from a laid-out bar, with
    /// Bold Text and without.
    func testTheUnsaidTitleFontIsUIKits() throws {
        for bold in [false, true] {
            let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
            if bold { window.traitOverrides.legibilityWeight = .bold }
            let root = UIViewController()
            root.navigationItem.rightBarButtonItem = UIBarButtonItem(title: "Edit", style: .plain, target: nil, action: nil)
            let nav = UINavigationController(rootViewController: root)
            window.rootViewController = nav
            window.isHidden = false
            defer { window.isHidden = true }
            nav.view.layoutIfNeeded()
            nav.navigationBar.layoutIfNeeded()
            func labels(_ view: UIView) -> [UILabel] { (view as? UILabel).map { [$0] } ?? view.subviews.flatMap(labels) }
            let edit = try XCTUnwrap(labels(nav.navigationBar).first { $0.text == "Edit" }, "the item's title label")
            let expected = HeaderShape.Item.titleFont(bold: bold)
            XCTAssertEqual(edit.traitCollection.legibilityWeight == .bold, bold)
            XCTAssertEqual(edit.font.pointSize, expected.pointSize, "bold \(bold)")
            XCTAssertEqual(edit.font.fontName, expected.fontName, "bold \(bold)")
            XCTAssertEqual(TextEngine.controlWeight(edit.font), TextEngine.controlWeight(expected), "bold \(bold)")
        }
    }
}
#endif

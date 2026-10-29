#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A header's readout (`role="status"`, LLP 1075.003 §9.15) over hand-built
/// routes: what counts as one, what its face holds, and what its bar item's
/// view does with direction, `aria-hidden` and the text size. The native
/// fixture's readout, pushed and pressed, is NavigationBasicsIOSTests'.
final class BarReadoutIOSTests: XCTestCase {
    private var window: UIWindow!
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    /// A route whose header holds a level-2 heading "Log" (id 3) and then
    /// `rest`, created as given; `kids` are the header's children after the
    /// heading, `more` any further `children` ops.
    private func route(_ rest: [[String: Any]], kids: [Int], more: [[String: Any]] = []) -> Presenter {
        // A session's presenter: a face's texts are set by its text engine.
        let session = ExactApp.shared.makeSession(label: "readout-\(UUID().uuidString)")
        sessions.append(session)
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationKey": "log"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["semanticTag": "header"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Log", "accessibilityRole": "heading"]],
        ]
        ops += rest + more
        ops += [["op": "children", "id": 2, "ids": [3] + kids], ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        return p
    }

    private func text(_ id: Int, _ words: String, _ props: [String: String] = [:], style: [String: Any] = [:]) -> [String: Any] {
        ["op": "create", "id": id, "kind": "text", "props": ["text": words].merging(props) { $1 }, "style": style]
    }

    private func status(_ id: Int, _ props: [String: String] = [:], style: [String: Any] = [:]) -> [String: Any] {
        ["op": "create", "id": id, "kind": "view", "props": ["accessibilityRole": "status"].merging(props) { $1 }, "style": style]
    }

    private let dot: [String: Any] = ["background_color": [52, 199, 89, 255], "width": 7, "height": 7]

    func testAReadoutInABoxHiddenAsAuthoredShowsNothing() throws {
        let p = route([["op": "create", "id": 4, "kind": "view", "style": ["display": "none"]], text(5, "Saved", ["accessibilityRole": "status"])],
                      kids: [4], more: [["op": "children", "id": 4, "ids": [5]]])
        let shape = try XCTUnwrap(HeaderShape(route: try XCTUnwrap(p.views[1]), back: nil, projected: true))
        XCTAssertTrue(shape.trailing.isEmpty, "hidden by the box around it")
        XCTAssertFalse(shape.placed.contains { $0.id == 5 })
    }

    /// A row of a symbol and a status after the heading is no glyph line:
    /// the status is a readout, never the subtitle too, and the text after
    /// the row is the subtitle.
    func testAStatusInARowIsNeverTheSubtitle() throws {
        let p = route([["op": "create", "id": 4, "kind": "view"],
                       ["op": "create", "id": 5, "kind": "image", "props": ["imageSource": "symbol:sf/checkmark", "symbolName": "checkmark"]],
                       text(6, "Saved", ["accessibilityRole": "status"]), text(7, "3 entries")],
                      kids: [4, 7], more: [["op": "children", "id": 4, "ids": [5, 6]]])
        let shape = try XCTUnwrap(HeaderShape(route: try XCTUnwrap(p.views[1]), back: nil, projected: true))
        XCTAssertEqual(shape.trailing.map(\.id), [6])
        XCTAssertEqual(shape.group?.subtitle, "3 entries")
        XCTAssertTrue(shape.group?.glyphs.isEmpty ?? true)
    }

    /// A disabled button is UIKit's disabled bar item, shown dimmed, though
    /// nothing presses it (the euxy port's "● Clock" written as one); one
    /// neither pressed nor disabled is still the page's.
    func testADisabledButtonIsADisabledItemThoughNothingPressesIt() throws {
        let button = { (id: Int, props: [String: String]) -> [String: Any] in
            ["op": "create", "id": id, "kind": "control", "props": ["type": "button", "accessibilityRole": "button"].merging(props) { $1 }]
        }
        let p = route([button(4, ["disabled": "true", "accessibilityLabel": "Clock"]), button(5, ["accessibilityLabel": "Idle"]),
                       button(6, ["disabled": "false", "accessibilityLabel": "Enabled"])], kids: [4, 5, 6])
        let shape = try XCTUnwrap(HeaderShape(route: try XCTUnwrap(p.views[1]), back: nil, projected: true))
        XCTAssertEqual(shape.trailing.map(\.id), [4])
        XCTAssertTrue(shape.trailing.first?.disabled ?? false)
        XCTAssertTrue(shape.placed.contains { $0.id == 4 })
        XCTAssertFalse(shape.placed.contains { $0.id == 5 || $0.id == 6 })
    }

    /// With no `aria-label` a readout says the words it shows: a hidden
    /// text in it is not spoken.
    func testItsWordsAreTheOnesItShows() throws {
        let p = route([status(4), text(5, "Connected"), text(6, "Disconnected", style: ["display": "none"])],
                      kids: [4], more: [["op": "children", "id": 4, "ids": [5, 6]]])
        let shape = try XCTUnwrap(HeaderShape(route: try XCTUnwrap(p.views[1]), back: nil, projected: true))
        XCTAssertEqual(shape.trailing.first?.label, "Connected")
    }

    /// A row of glyphs beside a readout is still the glyph line, the
    /// readout left out of it; beside a hidden status it is the glyph line
    /// whole, the status the page's.
    func testAGlyphLineBesideAReadoutStaysOne() throws {
        for hidden in [false, true] {
            let p = route([["op": "create", "id": 4, "kind": "view"],
                           ["op": "create", "id": 5, "kind": "image", "props": ["imageSource": "symbol:sf/bell.slash", "symbolName": "bell.slash"]],
                           text(6, "Muted"), text(7, "Live", ["accessibilityRole": "status"], style: hidden ? ["display": "none"] : [:])],
                          kids: [4], more: [["op": "children", "id": 4, "ids": [5, 6, 7]]])
            let shape = try XCTUnwrap(HeaderShape(route: try XCTUnwrap(p.views[1]), back: nil, projected: true))
            XCTAssertEqual(shape.group?.glyphs.map(\.symbol), ["bell.slash", nil], "hidden: \(hidden)")
            XCTAssertEqual(shape.group?.subtitle, "Muted")
            XCTAssertEqual(shape.trailing.map(\.id), hidden ? [] : [7])
        }
    }

    /// A dot alone, named by its `aria-label`, is a readout's whole face,
    /// drawn at the status's own opacity.
    func testADotAloneIsAFaceAtTheReadoutsOpacity() throws {
        let p = route([status(4, ["accessibilityLabel": "Connected"], style: dot)], kids: [4],
                      more: [["op": "present", "id": 4, "property": "opacity", "x": 0.4, "y": 0.0]])
        let shape = try XCTUnwrap(HeaderShape(route: try XCTUnwrap(p.views[1]), back: nil, projected: true))
        let item = try XCTUnwrap(shape.trailing.first)
        let face = try XCTUnwrap(item.readout, "a dot is something to draw")
        guard case .dot(let size, _, _) = try XCTUnwrap(face.parts.first) else { return XCTFail("\(face.parts)") }
        XCTAssertEqual(size, CGSize(width: 7, height: 7))
        XCTAssertEqual(Double(face.opacities[0]), 0.4, accuracy: 0.001, "the status's own opacity")
        XCTAssertEqual(item.label, "Connected")
        XCTAssertTrue(shape.placed.contains { $0.id == 4 })
    }

    /// A readout with words and no padding written is as wide as its face
    /// plus a title item's inset each side, as a hand-built
    /// `UIBarButtonItem(title:)` is, so the capsule's ends clear its words; a
    /// side its author wrote is the author's, and a dot alone takes none.
    func testUnsaidPaddingBesideWordsIsATitleItemsInset() throws {
        func view(_ style: [String: Any], words: Bool = true) throws -> ReadoutView {
            let p = route([status(4, style: style), ["op": "create", "id": 5, "kind": "view", "style": dot]] + (words ? [text(6, "Clock")] : []),
                          kids: [4], more: [["op": "children", "id": 4, "ids": words ? [5, 6] : [5]]])
            let route = RouteController(try XCTUnwrap(p.views[1]))
            let shape = try XCTUnwrap(HeaderShape(route: route.node, back: nil, projected: true))
            let view = try XCTUnwrap(p.navigation.barItems(shape.trailing, route).first?.customView as? ReadoutView)
            window.addSubview(view)
            p.navigation.showReadouts(shape, in: route)
            view.layoutIfNeeded()
            return view
        }
        let inset = HeaderShape.Item.titleInset
        XCTAssertEqual(inset, 12)
        let unsaid = try view([:])
        let face = unsaid.stack.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize).width
        XCTAssertGreaterThan(face, 7 + 20, "a dot and a word")
        XCTAssertEqual(unsaid.intrinsicContentSize.width, face + 2 * inset, accuracy: 0.01)
        XCTAssertEqual(unsaid.stack.frame.minX, inset, accuracy: 0.01)
        let written = try view(["padding_left": 4, "padding_right": 0, "written": ["padding_left", "padding_right"]])
        XCTAssertEqual(written.intrinsicContentSize.width, face + 4, accuracy: 0.01, "the author's sides")
        let dotAlone = try view([:], words: false)
        XCTAssertEqual(dotAlone.intrinsicContentSize.width, 7, accuracy: 0.01, "no words, no inset")
    }

    /// A bar item's own colour that names the tint takes its button's tint, undimmed while UIKit dims
    /// tints: the item keeps the colour it is given
    func testABarItemsTintColourIsItsButtonsUndimmed() throws {
        let p = route([["op": "create", "id": 4, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Edit"],
                        "style": ["text_color": ["sys": "@tint", "c": [0, 122, 255, 255]], "written": ["text_color"]]],
                       text(5, "Edit")], kids: [4], more: [["op": "children", "id": 4, "ids": [5]]])
        window.tintColor = UIColor(red: 1, green: 0, blue: 0, alpha: 1)
        func tint() throws -> [Double] {
            let shape = try XCTUnwrap(HeaderShape(route: try XCTUnwrap(p.views[1]), back: nil, projected: true))
            let item = try XCTUnwrap(shape.trailing.first { $0.id == 4 }, "the button is a bar item: \(shape.trailing.map(\.id))")
            return try XCTUnwrap(item.tint?.first, "the item has a colour of its own")
        }
        for (got, want) in zip(try tint(), [255.0, 0, 0, 255]) { XCTAssertEqual(got, want, accuracy: 2, "the button's red tint") }
        window.tintAdjustmentMode = .dimmed
        defer { window.tintAdjustmentMode = .automatic }
        for (got, want) in zip(try tint(), [255.0, 0, 0, 255]) { XCTAssertEqual(got, want, accuracy: 2, "the red tint, undimmed, while dimmed") }
    }

    /// A status's symbol keeps its colour behind a sheet or an alert, as its words keep theirs: UIKit
    /// would dim it as a tint (TintDimmingIOS.swift)
    func testAStatusSymbolKeepsItsColourWhileDimmed() throws {
        let green: [Double] = [52, 199, 89, 255]
        let inked: [String: Any] = ["text_color": green, "written": ["text_color"]]
        let p = route([status(4, style: inked), ["op": "create", "id": 5, "kind": "image", "props": ["imageSource": "symbol:sf/lock.fill", "symbolName": "lock.fill"], "style": inked],
                       text(6, "Locked", style: inked)],
                      kids: [4], more: [["op": "children", "id": 4, "ids": [5, 6]]])
        let route = RouteController(try XCTUnwrap(p.views[1]))
        let shape = try XCTUnwrap(HeaderShape(route: route.node, back: nil, projected: true))
        let view = try XCTUnwrap(p.navigation.barItems(shape.trailing, route).first?.customView as? ReadoutView)
        window.addSubview(view)
        p.navigation.showReadouts(shape, in: route)
        let symbol = try XCTUnwrap(view.stack.arrangedSubviews.compactMap { $0 as? UIImageView }.first, "the readout shows its symbol")
        view.tintAdjustmentMode = .dimmed
        defer { view.tintAdjustmentMode = .automatic }
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        XCTAssertTrue(symbol.tintColor.resolvedColor(with: symbol.traitCollection).getRed(&r, green: &g, blue: &b, alpha: &a))
        for (got, want) in zip([r, g, b, a].map { Double($0) * 255 }, green) { XCTAssertEqual(got, want, accuracy: 2, "the symbol keeps its green while dimmed") }
    }

    /// The bar item's view: the author's direction with padding kept on
    /// its physical side, no accessibility element when `aria-hidden`, and
    /// the platform's body type following the text size.
    func testTheViewTakesDirectionAriaHiddenAndTheTextSize() throws {
        let p = route([status(4, ["accessibilityElementsHidden": "true"], style: ["direction": "rtl", "padding_left": 12, "written": ["padding_left"]]),
                       ["op": "create", "id": 5, "kind": "view", "style": dot], text(6, "Live")],
                      kids: [4], more: [["op": "children", "id": 4, "ids": [5, 6]]])
        let route = RouteController(try XCTUnwrap(p.views[1]))
        let shape = try XCTUnwrap(HeaderShape(route: route.node, back: nil, projected: true))
        let items = p.navigation.barItems(shape.trailing, route)
        let view = try XCTUnwrap(items.first?.customView as? ReadoutView)
        window.addSubview(view)
        p.navigation.showReadouts(shape, in: route)
        XCTAssertEqual(view.stack.semanticContentAttribute, .forceRightToLeft, "the author's direction")
        view.layoutIfNeeded()
        XCTAssertEqual(view.stack.frame.minX, 12, "padding-left stays on the left")
        XCTAssertFalse(view.isAccessibilityElement, "aria-hidden")
        let label = try XCTUnwrap(view.stack.arrangedSubviews.compactMap { $0 as? UILabel }.first)
        XCTAssertTrue(label.adjustsFontForContentSizeCategory, "no font written: the platform's body, which follows the text size")
        let before = label.font.pointSize
        view.traitOverrides.preferredContentSizeCategory = .accessibilityExtraLarge
        view.updateTraitsIfNeeded()
        let after = try XCTUnwrap(view.stack.arrangedSubviews.compactMap { $0 as? UILabel }.first).font.pointSize
        XCTAssertGreaterThan(after, before, "read again at the larger size")
    }
}
#endif

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

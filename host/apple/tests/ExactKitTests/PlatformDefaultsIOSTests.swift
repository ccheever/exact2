#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1115 wave 1 on UIKit: what the author leaves unsaid is the
/// platform's. A colour a control only inherits is not its own; an unset
/// background is `systemBackground`; the keyboard's Return does what its
/// label says; the status bar scrolls one scroller. UIKit, so a simulator
/// runs it: bun host/apple/build.mjs --test --ios
final class PlatformDefaultsIOSTests: XCTestCase {
    private var window: UIWindow!

    /// Each node has a prop, so it stays a view, not a flat leaf (LLP 1068 §6.1).
    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }

    /// §2, "inherited is unsaid": a row is a node's own only when it differs
    /// from the nearest node above it, and is not a platform colour.
    func testOwnColourIsWhatDiffersFromTheNodeAbove() throws {
        let canvasText: [String: Any] = ["sys": "labelColor", "c": [[0, 0, 0, 255], [255, 255, 255, 255]]]
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "props": ["id": "n1"], "style": ["text_color": [17, 17, 17, 255]]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "n2"], "style": ["text_color": [17, 17, 17, 255]]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "n3"], "style": ["text_color": [181, 86, 43, 255]]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "n4"], "style": ["text_color": canvasText]],
            ["op": "create", "id": 5, "kind": "view", "props": ["id": "n5"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5]],
            ["op": "roots", "ids": [1]],
        ])
        XCTAssertNotNil(try XCTUnwrap(p.views[1]).ownColor("text_color"), "the page's colour is the page's own")
        XCTAssertNil(try XCTUnwrap(p.views[2]).ownColor("text_color"), "inherited")
        XCTAssertEqual(try XCTUnwrap(p.views[3]).ownColor("text_color")?.numbers, [181, 86, 43, 255])
        XCTAssertNotNil(try XCTUnwrap(p.views[3]).ownUIColor("text_color"))
        XCTAssertNil(try XCTUnwrap(p.views[4]).ownColor("text_color"), "a platform colour is the platform's")
        XCTAssertNil(try XCTUnwrap(p.views[5]).ownColor("text_color"), "no row")
    }

    /// D2: a page with no background shows the platform's, not white.
    func testAnUnsetBackgroundIsTheSystemBackground() throws {
        let p = presenter([["op": "create", "id": 1, "kind": "view"], ["op": "roots", "ids": [1]]])
        let dark = try XCTUnwrap(p.viewport.backgroundColor).resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark))
        XCTAssertEqual(dark, UIColor.systemBackground.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)))
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": [255, 0, 0, 255]]]]))
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        try XCTUnwrap(p.viewport.backgroundColor).getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertEqual([r, g, b, a], [1, 0, 0, 1], "an authored one wins")
    }

    /// Done puts the keyboard away; Next moves to the next field, and from
    /// the last field puts it away; an unlabelled Return leaves it up.
    func testReturnDoesWhatItsLabelSays() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        let fields = (1...3).map { i -> NodeView in
            let node = NodeView(id: UInt32(i), kind: "input", presenter: p)
            node.handlers = ["change"]
            node.frame = CGRect(x: 0, y: CGFloat(i) * 50, width: 200, height: 40)
            p.root.addSubview(node); p.views[node.id] = node
            return node
        }
        window.makeKeyAndVisible()
        fields[0].applyProps(set: ["enterKeyHint": "next"], clear: [])
        fields[1].applyProps(set: ["enterKeyHint": "next"], clear: [])
        fields[2].applyProps(set: ["enterKeyHint": "done"], clear: [])
        let editors = try fields.map { try XCTUnwrap($0.field) }
        XCTAssertTrue(editors[0].becomeFirstResponder())
        XCTAssertFalse(fields[0].textFieldShouldReturn(editors[0]))
        XCTAssertTrue(editors[1].isFirstResponder, "Next moves to the next field")
        XCTAssertFalse(fields[1].textFieldShouldReturn(editors[1]))
        XCTAssertTrue(editors[2].isFirstResponder)
        XCTAssertFalse(fields[2].textFieldShouldReturn(editors[2]))
        XCTAssertFalse(editors[2].isFirstResponder, "Done puts the keyboard away")
        fields[2].applyProps(set: [:], clear: ["enterKeyHint"])
        XCTAssertTrue(editors[2].becomeFirstResponder())
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertTrue(editors[2].isFirstResponder, "a plain Return commits and keeps editing, as on the web")
        fields[2].applyProps(set: ["enterKeyHint": "next"], clear: [])
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertFalse(editors[2].isFirstResponder, "Next from the last field puts the keyboard away")
    }

    /// Only one scroll view answers the status bar, or UIKit scrolls none:
    /// with no routes and a page that does not scroll, the first vertical
    /// scroller; a horizontal one and the viewport are off.
    func testOneScrollerAnswersTheStatusBar() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "n2"], "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "n3"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "n4"], "style": ["overflow_y": "scroll"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 400.0, "h": 350.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
            ["op": "content", "id": 3, "w": 400.0, "h": 2000.0],
        ])
        let page = try XCTUnwrap(p.views[3]?.scroll)
        XCTAssertTrue(page.scrollsToTop, "the page's content scroller")
        XCTAssertFalse(try XCTUnwrap(p.views[2]?.scroll).scrollsToTop, "a horizontal strip")
        XCTAssertFalse(try XCTUnwrap(p.views[4]?.scroll).scrollsToTop, "a nested scroller")
        XCTAssertFalse(p.viewport.scrollsToTop, "the viewport, which does not scroll")
    }
}
#endif

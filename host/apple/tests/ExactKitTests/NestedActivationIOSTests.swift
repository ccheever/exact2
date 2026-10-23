#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class NestedActivationIOSTests: XCTestCase {
    private var window: UIWindow?
    private var presenter: Presenter?
    override func tearDown() {
        presenter?.menus.reset(); presenter = nil
        window?.isHidden = true; window = nil
        super.tearDown()
    }
    private final class Touch: UITouch {
        let target: UIView
        init(_ target: UIView) { self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint {
            target.convert(CGPoint(x: 20, y: 20), to: view)
        }
    }
    private func page() throws -> Presenter {
        let p = Presenter(); presenter = p
        p.buttonFace = { _ in ButtonFace() }
        let w = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = w.bounds; w.addSubview(p.viewport); w.makeKeyAndVisible(); window = w
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "outer"], "handlers": ["press"]],
            ["op": "create", "id": 3, "kind": "button"],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Inner"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["id": "outer", "popover": "manual"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["id": "inner", "popover": "manual"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2, 5, 6]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "frame", "id": 2, "x": 20, "y": 40, "w": 240, "h": 100],
            ["op": "frame", "id": 3, "x": 10, "y": 10, "w": 180, "h": 60],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 160, "h": 44],
            ["op": "frame", "id": 5, "x": 20, "y": 200, "w": 150, "h": 100],
            ["op": "frame", "id": 6, "x": 200, "y": 200, "w": 150, "h": 100],
        ]))
        p.viewport.layoutIfNeeded()
        return p
    }
    func testAnInnerButtonStopsActivationButKeepsTheAncestorPress() throws {
        let p = try page(), inner = try XCTUnwrap(p.views[3]), outer = try XCTUnwrap(p.views[5])
        var presses: [UInt32] = []; p.onPress = { presses.append($0) }
        let touch = Touch(inner)
        inner.touchesBegan([touch], with: nil); inner.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [2]); XCTAssertFalse(p.menus.isOpen(outer))
        inner.handlers = ["press"]
        inner.touchesBegan([touch], with: nil); inner.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [2, 3]); XCTAssertFalse(p.menus.isOpen(outer))
    }
    func testAHandlerlessInnerInvokerActivatesOnlyItsOwnTarget() throws {
        let p = try page(), inner = try XCTUnwrap(p.views[3])
        inner.props["popovertarget"] = "inner"
        var presses: [UInt32] = []; p.onPress = { presses.append($0) }
        // The pull-down calls this before UIKit presents its own target.
        p.menus.pressAsTouched(inner)
        XCTAssertEqual(presses, [2]); XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[5])))
        let touch = Touch(inner)
        inner.touchesBegan([touch], with: nil); inner.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [2, 2])
        XCTAssertTrue(p.menus.isOpen(try XCTUnwrap(p.views[6])))
        XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[5])))
    }
    func testInlinePressInvokesItsButtonUnlessItsAncestryContainsALink() throws {
        let p = try page(), paragraph = try XCTUnwrap(p.views[4]), inner = try XCTUnwrap(p.views[3])
        inner.props["popovertarget"] = "inner"
        var presses: [UInt32] = []; p.onPress = { presses.append($0) }
        for link in [false, true] {
            p.applyParagraph(4, [
                InlineText(id: 10, parent: 4, props: link ? ["href": "/elsewhere"] : [:], style: InlineStyle(), handlers: [], paints: false),
                InlineText(id: 11, parent: 10, props: ["text": "Inline"], style: InlineStyle(), handlers: ["press"], paints: true),
            ])
            XCTAssertTrue(paragraph.activateInline(11))
            XCTAssertEqual(p.menus.isOpen(try XCTUnwrap(p.views[6])), !link)
            XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[5])))
            p.menus.action("hidePopover", ["inner"])
        }
        XCTAssertEqual(presses, [11, 11])
    }
}
#endif

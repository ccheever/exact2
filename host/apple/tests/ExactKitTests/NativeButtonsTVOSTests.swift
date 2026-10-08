// @ref LLP 1104 D6: the UIButton owns tvOS focus, including restoration and guides.
#if os(tvOS)
import UIKit
import XCTest
@testable import ExactKit

final class NativeButtonsTVOSTests: XCTestCase {
    func testNativeFocusOwnerRestoresReplacementAndDoesNotMakeAScrollerAStop() throws {
        let p = Presenter()
        var face = ButtonFace(); face.title = "Focus"
        p.buttonFace = { _ in face }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 800, height: 600))
        window.addSubview(p.viewport); window.makeKeyAndVisible()
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["focusGuide": "auto"], "style": ["overflow_y": "scroll"]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 150],
            ["op": "content", "id": 1, "w": 300, "h": 600],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "button", "testId": "same"], "handlers": ["press"], "style": [:]],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 120, "h": 40],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]]))
        let node = try XCTUnwrap(p.views[2]), control = try XCTUnwrap(p.controls.controls[2])
        XCTAssertFalse(node.canBecomeFocused); XCTAssertTrue(control.canBecomeFocused)
        p.focusKey = "same"
        XCTAssertTrue(p.focusReturn === control)
        XCTAssertTrue(try XCTUnwrap(p.views[1]?.scroll).holdsFocusableNode)
        XCTAssertFalse(try XCTUnwrap(p.views[1]).focusableScroller)
        p.focusGuides.sync(); p.focusGuides.focused(node); p.focusGuides.sync()
        let guide = try XCTUnwrap(p.views[1]?.layoutGuides.compactMap { $0 as? UIFocusGuide }.first)
        XCTAssertTrue(guide.preferredFocusEnvironments.first === control)
        p.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "control", "props": ["type": "button", "testId": "same"], "handlers": ["press"], "style": [:]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 120, "h": 40],
            ["op": "children", "id": 1, "ids": [3]], ["op": "destroy", "id": 2]]))
        XCTAssertTrue(p.focusReturn === p.controls.controls[3])
        XCTAssertTrue(guide.preferredFocusEnvironments.first === p.controls.controls[3], "the batch immediately retargets the guide to the replacement native owner")
    }
}
#endif

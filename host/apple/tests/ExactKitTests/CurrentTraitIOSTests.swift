#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// `aria-current` on a pressable (a bar's current item, BarItem's) is the
/// selected trait on iOS, as `aria-selected` is: UIKit has no current
/// (theme2 2026-10-22 06:40); `false` or nothing is not selected.
final class CurrentTraitIOSTests: XCTestCase {
    private var window: UIWindow?
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    func testACurrentPressableIsSelectedToVoiceOver() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 200))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        defer { p.reset() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["accessibilityLabel": "Home", "accessibilityCurrent": "page"]],
            ["op": "create", "id": 3, "kind": "button", "props": ["accessibilityLabel": "Search", "accessibilityCurrent": "false"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 200],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 100, "h": 44],
            ["op": "frame", "id": 3, "x": 100, "y": 0, "w": 100, "h": 44],
        ]))
        let home = try XCTUnwrap(p.views[2]), search = try XCTUnwrap(p.views[3])
        XCTAssertTrue(home.accessibilityTraits.contains(.button))
        XCTAssertTrue(home.accessibilityTraits.contains(.selected), "aria-current=page: selected")
        XCTAssertFalse(search.accessibilityTraits.contains(.selected), "aria-current=false: not")
        p.apply(wireBatch([["op": "props", "id": 2, "set": [String: String](), "clear": ["accessibilityCurrent"]],
                           ["op": "props", "id": 3, "set": ["accessibilityCurrent": "page"]]]))
        XCTAssertFalse(home.accessibilityTraits.contains(.selected), "no longer current")
        XCTAssertTrue(search.accessibilityTraits.contains(.selected), "now current")
    }
}
#endif

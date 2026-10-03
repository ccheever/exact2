#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// `tree --ax` on UIKit (LLP 1080.002 §3 stage 2): the walk reports what
/// UIKit exposes after the host's own assignments, joins each element to
/// its view, excludes only by documented properties, and flags a modal
/// view's leak by UIKit's rule. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class AccessibilityTreeIOSTests: XCTestCase {
    private var window: UIWindow!

    /// Two buttons (one named, one an image with no name) and a column.
    private func fixture() throws -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "props": ["testId": "named", "accessibilityLabel": "Play"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["testId": "bare"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "box"]],
            ["op": "create", "id": 5, "kind": "button", "handlers": ["press"], "props": ["testId": "inside", "accessibilityLabel": "Inside"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 40.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 100.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
        ]))
        p.syncAccessibility()
        window.layoutIfNeeded()
        return p
    }
    private func elements(_ ax: [String: Any]) -> [[String: Any]] { ax["elements"] as? [[String: Any]] ?? [] }
    private func element(_ ax: [String: Any], _ testId: String) -> [String: Any]? { elements(ax).first { $0["testId"] as? String == testId } }

    func testElementsAreUIKitsAnswersJoinedToTheirViews() throws {
        let p = try fixture()
        let ax = p.axElements(roots: [p.viewport])
        XCTAssertEqual(ax["source"] as? String, "uikit")
        XCTAssertEqual(ax["order"] as? String, "containment")
        let named = try XCTUnwrap(element(ax, "named"))
        XCTAssertEqual(named["role"] as? String, "button")
        XCTAssertEqual(named["name"] as? String, "Play")
        XCTAssertEqual(named["id"] as? UInt32, 2)
        XCTAssertEqual(named["via"] as? String, "self")
        XCTAssertEqual(named["interactive"] as? Bool, true)
        // UIKit was given no name for the bare button: the reply says so.
        XCTAssertEqual(element(ax, "bare")?["name"] as? String, "")
        // Frames are UIKit's to compute, and it does so only with its
        // accessibility runtime loaded; without it the reply says coverage
        // is incomplete, and why.
        let coverage = try XCTUnwrap(ax["coverage"] as? [String: Any])
        if Presenter.axRuntimeLoaded {
            let frame = try XCTUnwrap(named["frame"] as? [String: Any])
            XCTAssertEqual(frame["w"] as? Double, 100)
            XCTAssertEqual(frame["source"] as? String, "element")
            XCTAssertEqual(coverage["complete"] as? Bool, true)
        } else {
            XCTAssertEqual(coverage["complete"] as? Bool, false)
            XCTAssertEqual((coverage["excluded"] as? [[String: Any]])?.first?["root"] as? String, "process")
        }
    }

    func testOnlyDocumentedPropertiesExcludeAndClearingOneExposesTheSubtree() throws {
        let p = try fixture()
        let box = try XCTUnwrap(p.views[4])
        box.accessibilityElementsHidden = true
        XCTAssertNil(element(p.axElements(roots: [p.viewport]), "inside"), "hidden from UIKit, so not exposed")
        let listed = try XCTUnwrap(element(p.axElements(roots: [p.viewport], excluded: true), "inside"))
        XCTAssertEqual(listed["excluded"] as? [String], ["elementsHidden"])
        // A negative through the walk: cleared after mount, the subtree is exposed again.
        box.accessibilityElementsHidden = false
        XCTAssertNotNil(element(p.axElements(roots: [p.viewport]), "inside"))
        // Alpha is not a documented exclusion: an alpha-0 view is still reported, and coverage says so.
        box.alpha = 0
        let ax = p.axElements(roots: [p.viewport])
        XCTAssertNotNil(element(ax, "inside"))
        XCTAssertEqual((ax["coverage"] as? [String: Any])?["limits"] as? [String], ["alpha", "reading-order"])
    }

    func testAModalViewHidesItsSiblingsOnlyAndALeakIsMarked() throws {
        let p = try fixture()
        // The box is modal: its siblings (the two buttons) are hidden by
        // UIKit's rule; a button beside the viewport's parent leaks.
        let box = try XCTUnwrap(p.views[4])
        box.accessibilityViewIsModal = true
        // A plain element: a UIButton is one only with UIKit's accessibility runtime loaded.
        let outside = UIView()
        outside.isAccessibilityElement = true
        outside.accessibilityLabel = "Outside"
        outside.accessibilityTraits = .button
        outside.frame = CGRect(x: 0, y: 300, width: 100, height: 40)
        window.addSubview(outside)
        let ax = p.axElements(roots: [p.viewport, outside])
        let modal = try XCTUnwrap(ax["modal"] as? [String: Any])
        XCTAssertEqual(modal["present"] as? Bool, true)
        XCTAssertEqual(modal["id"] as? UInt32, 4)
        XCTAssertEqual(element(ax, "named")?["outsideModal"] as? Bool, false, "a sibling: hidden by UIKit's rule")
        let leak = try XCTUnwrap(elements(ax).first { $0["name"] as? String == "Outside" })
        XCTAssertEqual(leak["outsideModal"] as? Bool, true)
        XCTAssertEqual(leak["via"] as? String, "none")
        XCTAssertNil(element(ax, "inside")?["outsideModal"], "inside the modal view")
    }

    func testAnOwnedElementJoinsItsDeclaredViewAndASecureValueIsOmitted() throws {
        let p = try fixture()
        let box = try XCTUnwrap(p.views[4])
        final class Owned: UIAccessibilityElement, AgentOwned { var agentViewId: UInt32? { 5 } }
        let owned = Owned(accessibilityContainer: box)
        owned.accessibilityLabel = "Declared"
        owned.isAccessibilityElement = true
        let paragraph = UIAccessibilityElement(accessibilityContainer: box)
        paragraph.accessibilityLabel = "Text"
        paragraph.isAccessibilityElement = true
        box.accessibilityElements = [paragraph, owned]
        let field = UITextField(frame: CGRect(x: 0, y: 50, width: 100, height: 30))
        field.isSecureTextEntry = true
        field.text = "hunter2"
        p.views[2]?.addSubview(field)
        let ax = p.axElements(roots: [p.viewport])
        let declared = try XCTUnwrap(elements(ax).first { $0["name"] as? String == "Declared" })
        XCTAssertEqual(declared["id"] as? UInt32, 5)
        XCTAssertEqual(declared["via"] as? String, "owner")
        let text = try XCTUnwrap(elements(ax).first { $0["name"] as? String == "Text" })
        XCTAssertEqual([text["id"] as? UInt32, text["via"] as? String] as [AnyHashable?], [4, "owner"])
        // The named button is an element, so UIKit never descends into it: the field is not walked there.
        p.views[2]?.isAccessibilityElement = false
        guard Presenter.axRuntimeLoaded else { return } // a field is an element only with UIKit's accessibility runtime
        let secure = try XCTUnwrap(elements(p.axElements(roots: [p.viewport])).first { $0["role"] as? String == "textbox" })
        XCTAssertNil(secure["value"])
        XCTAssertEqual((secure["states"] as? [String: Any])?["protected"] as? Bool, true)
    }

    func testACycleIsCutAndBudgetsBoundTheWalk() throws {
        let p = try fixture()
        let box = try XCTUnwrap(p.views[4])
        box.accessibilityElements = [box, p.views[5] as Any]
        let ax = p.axElements(roots: [p.viewport])
        XCTAssertEqual(ax["cycles"] as? Int, 1)
        let small = p.axElements(roots: [p.viewport], limit: 2)
        XCTAssertEqual(elements(small).count, 2)
        XCTAssertEqual((small["coverage"] as? [String: Any])?["complete"] as? Bool, false)
        XCTAssertNotNil(small["truncated"])
    }
}
#endif

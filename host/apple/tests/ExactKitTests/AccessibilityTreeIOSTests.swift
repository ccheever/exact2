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

    func testAModalViewHidesItsSiblingsAndALeakIsMarked() throws {
        let p = try fixture()
        // The box is modal: UIKit's rule hides its siblings (the two buttons),
        // so they are not exposed; an element beside the viewport leaks.
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
        XCTAssertNil(element(ax, "named"), "a sibling of the modal view is hidden by UIKit's rule")
        XCTAssertNil(element(ax, "bare"))
        let listed = p.axElements(roots: [p.viewport, outside], excluded: true)
        XCTAssertEqual(element(listed, "named")?["excluded"] as? [String], ["modalSibling"])
        let leak = try XCTUnwrap(elements(ax).first { $0["name"] as? String == "Outside" })
        XCTAssertEqual(leak["outsideModal"] as? Bool, true)
        XCTAssertEqual(leak["via"] as? String, "none")
        XCTAssertEqual(element(ax, "inside")?["outsideModal"] as? Bool, false, "inside the modal view")
        // A modal view UIKit owns is reported, and its sibling rule is not guessed.
        box.accessibilityViewIsModal = false
        let dimming = UIView()
        dimming.accessibilityViewIsModal = true
        p.viewport.addSubview(dimming)
        let kit = p.axElements(roots: [p.viewport])
        XCTAssertNotNil(element(kit, "named"))
        XCTAssertEqual((kit["coverage"] as? [String: Any])?["complete"] as? Bool, false)
    }

    /// `aria-modal` (LLP 1080.003) is the property UIKit's rule reads, only
    /// while the view is exposed: the walk then hides the box's siblings, and
    /// clearing it, `display: none` or an exit exposes them again.
    func testAriaModalIsUIKitsModalViewWhileExposed() throws {
        let p = try fixture()
        let box = try XCTUnwrap(p.views[4])
        let named = { self.element(p.axElements(roots: [p.viewport]), "named") }
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["accessibilityModal": "true"]]]))
        XCTAssertTrue(box.accessibilityViewIsModal)
        XCTAssertEqual((p.axElements(roots: [p.viewport])["modal"] as? [String: Any])?["id"] as? UInt32, 4)
        XCTAssertNil(named(), "a sibling of the aria-modal view is hidden")
        XCTAssertNotNil(element(p.axElements(roots: [p.viewport]), "inside"))
        XCTAssertEqual(p.announcedModal?.id, 4)
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["accessibilityModal": "false"]]]))
        XCTAssertFalse(box.accessibilityViewIsModal)
        XCTAssertNotNil(named())
        XCTAssertNil(p.announcedModal?.id, "VoiceOver goes back to the screen")
        // Shown and hidden by `display`, with the prop left true.
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["accessibilityModal": "true"]]]))
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["display": "none"]]]))
        XCTAssertFalse(box.accessibilityViewIsModal, "a modal that is not displayed hides nothing")
        XCTAssertNotNil(named())
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["display": "flex"]]]))
        XCTAssertTrue(box.accessibilityViewIsModal)
        // Of two modal siblings only the front one is modal; UIKit would hide each from the other.
        p.apply(wireBatch([
            ["op": "create", "id": 6, "kind": "view", "props": ["testId": "front", "accessibilityModal": "true"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 6]],
            ["op": "frame", "id": 6, "x": 0.0, "y": 50.0, "w": 400.0, "h": 200.0],
        ]))
        let front = try XCTUnwrap(p.views[6])
        XCTAssertTrue(front.accessibilityViewIsModal)
        XCTAssertFalse(box.accessibilityViewIsModal)
        XCTAssertEqual(p.announcedModal?.id, 6)
        // A leaving modal hides nothing while it exits.
        p.apply(wireBatch([["op": "exit", "id": 6]]))
        XCTAssertFalse(front.accessibilityViewIsModal)
        XCTAssertTrue(box.accessibilityViewIsModal, "the one left is modal again, though the leaver still covers it")
        p.apply(wireBatch([["op": "props", "id": 4, "clear": ["accessibilityModal"]]]))
        XCTAssertFalse(box.accessibilityViewIsModal, "a cleared prop is not modal")
        XCTAssertNotNil(named())
        XCTAssertNil(p.announcedModal?.id)
    }

    /// A modal inside a branch another modal hides is not modal; an
    /// accessibility-hidden ancestor or a destroy ends modality; a sibling
    /// painted over the modal (a sheet's stack) does too (LLP 1080.003 D2).
    func testAriaModalFollowsTheHierarchyAndDestroy() throws {
        let p = try fixture()
        // A (the box, 4) and B (7) are sibling modals, B in front; C (8) is a newer modal inside A.
        p.apply(wireBatch([
            ["op": "props", "id": 4, "set": ["accessibilityModal": "true"]],
            ["op": "create", "id": 7, "kind": "view", "props": ["testId": "b", "accessibilityModal": "true"]],
            ["op": "create", "id": 8, "kind": "view", "props": ["testId": "c", "accessibilityModal": "true"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 7]],
            ["op": "children", "id": 4, "ids": [5, 8]],
            ["op": "frame", "id": 7, "x": 0.0, "y": 300.0, "w": 400.0, "h": 50.0],
            ["op": "frame", "id": 8, "x": 0.0, "y": 50.0, "w": 100.0, "h": 40.0],
        ]))
        let a = try XCTUnwrap(p.views[4]), b = try XCTUnwrap(p.views[7]), c = try XCTUnwrap(p.views[8])
        XCTAssertEqual([a.accessibilityViewIsModal, b.accessibilityViewIsModal, c.accessibilityViewIsModal], [false, true, false])
        XCTAssertEqual(p.announcedModal?.id, 7, "C is inside the branch B hides")
        // Destroying B makes A modal, and C inside it the innermost.
        p.apply(wireBatch([["op": "destroy", "id": 7], ["op": "children", "id": 1, "ids": [2, 3, 4]]]))
        XCTAssertEqual([a.accessibilityViewIsModal, c.accessibilityViewIsModal], [true, true])
        XCTAssertEqual(p.announcedModal?.id, 8)
        // An accessibility-hidden ancestor hides both.
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["accessibilityElementsHidden": "true"]]]))
        XCTAssertEqual([a.accessibilityViewIsModal, c.accessibilityViewIsModal], [false, false])
        XCTAssertNil(p.announcedModal?.id)
        p.apply(wireBatch([["op": "props", "id": 1, "clear": ["accessibilityElementsHidden"]]]))
        XCTAssertEqual(p.announcedModal?.id, 8)
        // A visible sibling painted over A (as a sheet's stack is) ends its modality.
        let sheetController = UIViewController()
        let sheet: UIView = sheetController.view
        sheet.frame = a.frame
        a.superview?.addSubview(sheet)
        p.syncModal()
        XCTAssertEqual([a.accessibilityViewIsModal, c.accessibilityViewIsModal], [false, false], "C is under the covered A")
        XCTAssertNil(p.announcedModal?.id)
        sheet.removeFromSuperview()
        p.syncModal()
        XCTAssertEqual([a.accessibilityViewIsModal, c.accessibilityViewIsModal], [true, true])
        XCTAssertEqual(p.announcedModal?.id, 8)
    }

    /// The post itself: once per change of the innermost modal, never for an
    /// unchanged batch, and once when the only modal is destroyed with no
    /// reference left to it (LLP 1080.003 D2).
    func testAriaModalPostsOncePerChangeAndOnDestroy() throws {
        var posts = 0
        let post = Presenter.postScreenChanged
        Presenter.postScreenChanged = { _ in posts += 1 }
        defer { Presenter.postScreenChanged = post }
        let p = try fixture()
        posts = 0
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "view", "props": ["testId": "dialog", "accessibilityModal": "true"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 9]],
            ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
        ]))
        XCTAssertEqual(posts, 1, "VoiceOver moves into the dialog")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityLabel": "Pause"]]]))
        p.syncModal()
        XCTAssertEqual(posts, 1, "an unchanged modal is not announced again")
        weak var dialog = p.views[9]
        p.apply(wireBatch([["op": "destroy", "id": 9], ["op": "children", "id": 1, "ids": [2, 3, 4]]]))
        XCTAssertEqual(posts, 2, "and back out when it is destroyed")
        XCTAssertNil(p.announcedModal)
        _ = dialog
    }

    func testATargetScopesTheReplyAndTheWireRefusesWhatD1Refuses() throws {
        let p = try fixture()
        let box = try XCTUnwrap(p.views[4])
        _ = box
        // A projected element (a swipe cell, here a declared owner) lives
        // outside its view's subtree; the scope is the joined ids, so it stays.
        final class Projected: UIAccessibilityElement, AgentOwned { var agentViewId: UInt32? { 5 } }
        let projected = Projected(accessibilityContainer: try XCTUnwrap(p.views[2]))
        projected.isAccessibilityElement = true
        projected.accessibilityLabel = "Projected"
        p.views[2]?.isAccessibilityElement = false
        p.views[2]?.accessibilityElements = [projected]
        let ax = p.axElements(roots: [p.viewport], scope: [4, 5])
        XCTAssertEqual(Set(elements(ax).compactMap { $0["name"] as? String }), ["Inside", "Projected"])
        XCTAssertNotNil(ax["ancestors"])
        let session = ExactApp.shared.makeSession(label: "ax-wire")
        defer { session.destroy() }
        let agent = Agent(session: session)
        XCTAssertNotNil(agent.accessibilityElementsTree(["op": "tree", "ax": true, "shallow": true])["error"])
        XCTAssertNotNil(agent.accessibilityElementsTree(["op": "tree", "ax": true, "limit": "5"])["error"])
        XCTAssertNotNil(agent.accessibilityElementsTree(["op": "tree", "ax": true, "limit": 2.5])["error"])
        XCTAssertNotNil(agent.accessibilityElementsTree(["op": "tree", "ax": true, "limit": 0])["error"])
        XCTAssertNotNil(agent.accessibilityElementsTree(["op": "tree", "ax": true, "target": "no-such-view"])["error"])
    }

    func testAWalkStopsEnumeratingAtItsBudget() throws {
        let p = try fixture()
        final class Many: UIView {
            var asked = 0
            override func accessibilityElementCount() -> Int { 100_000 }
            override func accessibilityElement(at index: Int) -> Any? {
                asked += 1
                let e = UIAccessibilityElement(accessibilityContainer: self); e.isAccessibilityElement = true; e.accessibilityLabel = "\(index)"; return e
            }
        }
        let many = Many()
        p.views[1]?.addSubview(many)
        let ax = p.axElements(roots: [p.viewport], limit: 1)
        XCTAssertLessThan(many.asked, 200, "enumeration is bounded by the visit budget, not the reported count")
        XCTAssertEqual((ax["truncated"] as? [String: Any])?["elements"] as? String, "unknown")
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

    /// The native boundary (round 3): a 300 KB testId is cut before the
    /// element is budgeted, and a whole reply over 256 KB is cut, counted.
    func testTheNativeReplyIsBoundedAtTheBoundary() throws {
        let p = try fixture()
        p.views[2]?.props["testId"] = String(repeating: "t", count: 300_000)
        let ax = p.axElements(roots: [p.viewport])
        let named = try XCTUnwrap(elements(ax).first { $0["id"] as? UInt32 == 2 })
        XCTAssertLessThanOrEqual((named["testId"] as? String)?.count ?? 0, 200)
        let big: [[String: Any]] = (0..<3000).map { ["i": $0, "name": String(repeating: "é", count: 200)] }
        let bounded = Presenter.axBounded(["epoch": 1, "ax": ["elements": big, "coverage": ["complete": true], "truncated": ["elements": 0, "fields": 0]]])
        let data = try JSONSerialization.data(withJSONObject: bounded)
        XCTAssertLessThanOrEqual(data.count, 256 * 1024)
        let out = try XCTUnwrap(bounded["ax"] as? [String: Any])
        let kept = (out["elements"] as? [Any])?.count ?? 0
        XCTAssertEqual((out["truncated"] as? [String: Any])?["elements"] as? Int, 3000 - kept)
        XCTAssertEqual((out["coverage"] as? [String: Any])?["complete"] as? Bool, false)
        if #available(iOS 18, *) { XCTAssertTrue((ax["observes"] as? [String])?.contains("expanded") == true) }
        else { XCTAssertFalse((ax["observes"] as? [String])?.contains("expanded") == true) }
    }
}
#endif

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

    func testLabelledImagesExposeTheirTraitAndDecorativeImagesStayHidden() throws {
        let p = try fixture()
        let image = NodeView(id: 20, kind: "image", presenter: p)
        p.views[image.id] = image
        p.root.addSubview(image)
        image.frame = CGRect(x: 200, y: 0, width: 60, height: 60)
        image.applyProps(set: ["accessibilityLabel": "Station map"], clear: [])
        XCTAssertTrue(image.isAccessibilityElement)
        XCTAssertTrue(image.accessibilityTraits.contains(.image))
        XCTAssertEqual(image.accessibilityLabel, "Station map")
        image.applyProps(set: ["imageSource": "symbol:train.side.front.car", "symbolName": "train.side.front.car"], clear: [])
        image.updateSymbol()
        XCTAssertTrue(image.isAccessibilityElement)
        XCTAssertTrue(image.accessibilityTraits.contains(.image))
        image.applyProps(set: [:], clear: ["accessibilityLabel"])
        XCTAssertFalse(image.isAccessibilityElement)
        XCTAssertFalse(image.accessibilityTraits.contains(.image))
        image.applyProps(set: ["accessibilityLabel": ""], clear: [])
        XCTAssertFalse(image.isAccessibilityElement)
        image.applyProps(set: ["accessibilityLabel": "Station map"], clear: [])
        p.apply(wireBatch([["op": "style", "id": 20, "style": ["visibility": "hidden"]]]))
        image.applyProps(set: [:], clear: ["accessibilityLabel"])
        p.apply(wireBatch([["op": "style", "id": 20, "style": ["visibility": "visible"]]]))
        XCTAssertFalse(image.isAccessibilityElement, "clearing a label while hidden also clears its stored exposure")
    }

    func testAnImagesExplicitRangeRoleKeepsItsSpokenValue() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "image", "props": [
                "accessibilityLabel": "Download", "accessibilityRole": "progressbar", "accessibilityValueNow": "50"]],
            ["op": "children", "id": 4, "ids": [5, 20]],
            ["op": "frame", "id": 20, "x": 110.0, "y": 0.0, "w": 60.0, "h": 60.0],
        ]))
        let image = try XCTUnwrap(p.views[20])
        XCTAssertTrue(image.isAccessibilityElement)
        XCTAssertFalse(image.accessibilityTraits.contains(.image))
        XCTAssertEqual(image.accessibilityValue, image.rangeValueText)
        XCTAssertNotNil(image.accessibilityValue)
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["accessibilityValueNow": "75"]]]))
        XCTAssertEqual(image.accessibilityValue, image.rangeValueText)
        p.apply(wireBatch([["op": "props", "id": 20, "clear": ["accessibilityRole"]]]))
        XCTAssertTrue(image.accessibilityTraits.contains(.image))
        XCTAssertNil(image.accessibilityValue)
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["accessibilityRole": "progressbar"]]]))
        XCTAssertFalse(image.accessibilityTraits.contains(.image))
        XCTAssertEqual(image.accessibilityValue, image.rangeValueText)
        p.apply(wireBatch([
            ["op": "create", "id": 21, "kind": "text", "props": ["id": "caption", "text": "Download progress"]],
            ["op": "children", "id": 4, "ids": [5, 20, 21]],
            ["op": "props", "id": 20, "set": ["accessibilityLabelledBy": "caption"]],
        ]))
        XCTAssertEqual(image.accessibilityLabel, "Download progress")
        XCTAssertFalse(image.accessibilityTraits.contains(.image))
        XCTAssertEqual(image.accessibilityValue, image.rangeValueText)
    }

    /// A native control is named by `aria-labelledby` as by `aria-label`
    /// (accname), and follows the text it names.
    func testANativeCheckboxIsNamedByTheTextItsLabelledbyNames() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "control", "props": ["type": "checkbox", "accessibilityLabelledBy": "notify"], "handlers": ["change"]],
            ["op": "create", "id": 21, "kind": "text", "props": ["id": "notify", "text": "Notify me"]],
            ["op": "children", "id": 4, "ids": [5, 20, 21]],
            ["op": "frame", "id": 20, "x": 110.0, "y": 0.0, "w": 22.0, "h": 22.0],
            ["op": "frame", "id": 21, "x": 140.0, "y": 0.0, "w": 100.0, "h": 22.0],
        ]))
        let control = try XCTUnwrap(p.controls.controls[20])
        XCTAssertEqual(control.accessibilityLabel, "Notify me")
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["text": "Email me"]]]))
        XCTAssertEqual(control.accessibilityLabel, "Email me")
    }

    func testImageExposureFollowsItsReferencedCaptionsArrivalAndRemoval() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "image", "props": ["accessibilityLabelledBy": "caption"]],
            ["op": "children", "id": 4, "ids": [5, 20]],
            ["op": "frame", "id": 20, "x": 110.0, "y": 0.0, "w": 60.0, "h": 60.0],
        ]))
        let image = try XCTUnwrap(p.views[20])
        XCTAssertFalse(image.isAccessibilityElement)
        XCTAssertFalse(image.accessibilityTraits.contains(.image))
        p.apply(wireBatch([
            ["op": "create", "id": 21, "kind": "text", "props": ["id": "caption", "text": "Station map"]],
            ["op": "children", "id": 4, "ids": [5, 20, 21]],
            ["op": "frame", "id": 21, "x": 180.0, "y": 0.0, "w": 100.0, "h": 30.0],
        ]))
        XCTAssertEqual(image.accessibilityLabel, "Station map")
        XCTAssertTrue(image.isAccessibilityElement)
        XCTAssertTrue(image.accessibilityTraits.contains(.image))
        p.apply(wireBatch([
            ["op": "create", "id": 22, "kind": "image", "props": ["accessibilityLabelledBy": "caption"]],
            ["op": "children", "id": 4, "ids": [5, 20, 21, 22]],
            ["op": "frame", "id": 22, "x": 110.0, "y": 65.0, "w": 60.0, "h": 60.0],
        ]))
        let alreadyNamed = try XCTUnwrap(p.views[22])
        XCTAssertTrue(alreadyNamed.isAccessibilityElement)
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["text": ""]]]))
        XCTAssertFalse(image.isAccessibilityElement)
        XCTAssertFalse(image.accessibilityTraits.contains(.image))
        XCTAssertFalse(alreadyNamed.isAccessibilityElement)
        XCTAssertFalse(alreadyNamed.accessibilityTraits.contains(.image))
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["text": "Updated map"]]]))
        XCTAssertEqual(image.accessibilityLabel, "Updated map")
        XCTAssertTrue(image.isAccessibilityElement)
        XCTAssertTrue(image.accessibilityTraits.contains(.image))
        p.apply(wireBatch([
            ["op": "children", "id": 4, "ids": [5, 20, 22]],
            ["op": "destroy", "id": 21],
        ]))
        XCTAssertFalse(image.isAccessibilityElement)
        XCTAssertFalse(image.accessibilityTraits.contains(.image))
        XCTAssertFalse(alreadyNamed.isAccessibilityElement)
        XCTAssertFalse(alreadyNamed.accessibilityTraits.contains(.image))
    }

    func testToggleButtonsReportFalseTrueAndMixedFromThePlatform() throws {
        let p = try fixture()
        for value in ["false", "true", "mixed"] {
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityPressed": value]]]))
            p.syncAccessibility()
            let ax = p.axElements(roots: [p.viewport])
            let button = try XCTUnwrap(element(ax, "named"))
            XCTAssertEqual(button["role"] as? String, "button")
            let states = try XCTUnwrap(button["states"] as? [String: Any])
            if value == "mixed" { XCTAssertEqual(states["pressed"] as? String, "mixed") }
            else { XCTAssertEqual(states["pressed"] as? Bool, value == "true") }
            XCTAssertNil(states["checked"])
            XCTAssertTrue((ax["observes"] as? [String] ?? []).contains("pressed"))
        }
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["accessibilityPressed"]]]))
        let ax = p.axElements(roots: [p.viewport])
        let button = try XCTUnwrap(element(ax, "named"))
        XCTAssertNil((button["states"] as? [String: Any])?["pressed"])
    }

    func testUnjoinedTogglesUseTheirControlRoleAndDoNotInventPressedState() throws {
        let p = try fixture()
        let toggle = UISwitch(frame: CGRect(x: 200, y: 250, width: 60, height: 40))
        toggle.isOn = true
        toggle.isAccessibilityElement = true
        toggle.accessibilityLabel = "Native switch"
        toggle.accessibilityValue = "1"
        p.viewport.addSubview(toggle)
        let unknown = UIView(frame: CGRect(x: 200, y: 300, width: 60, height: 40))
        unknown.isAccessibilityElement = true
        unknown.accessibilityTraits = .toggleButton
        unknown.accessibilityLabel = "Unknown toggle"
        unknown.accessibilityValue = "2"
        p.viewport.addSubview(unknown)
        let ax = p.axElements(roots: [p.viewport])
        let native = try XCTUnwrap(elements(ax).first { $0["name"] as? String == "Native switch" })
        XCTAssertTrue(native["id"] is NSNull)
        XCTAssertEqual(native["role"] as? String, "switch")
        let state = native["states"] as? [String: Any] ?? [:]
        XCTAssertEqual(state["checked"] as? Bool, true)
        XCTAssertNil(state["pressed"])
        let other = try XCTUnwrap(elements(ax).first { $0["name"] as? String == "Unknown toggle" })
        XCTAssertTrue(other["id"] is NSNull)
        let unknownState = other["states"] as? [String: Any] ?? [:]
        XCTAssertNil(unknownState["checked"])
        XCTAssertNil(unknownState["pressed"])
    }

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

    /// LLP 1115.000 D11.6: a `link` is a link to VoiceOver. The tag lowers
    /// to a pressable whose role is `link`, and iOS called every pressable a
    /// button: only a plain box with `role="link"` got the link trait.
    func testALinkIsALinkToVoiceOver() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 6, "kind": "button", "props": ["testId": "link", "accessibilityRole": "link", "href": "https://example.com/", "accessibilityLabel": "Example"]],
            ["op": "create", "id": 7, "kind": "button", "handlers": ["press"], "props": ["testId": "pressed-link", "accessibilityRole": "link", "accessibilityLabel": "Open"]],
            ["op": "children", "id": 4, "ids": [5, 6, 7]],
            ["op": "frame", "id": 6, "x": 110.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 7, "x": 220.0, "y": 0.0, "w": 100.0, "h": 40.0],
        ]))
        p.syncAccessibility()
        for id: UInt32 in [6, 7] {
            let link = try XCTUnwrap(p.views[id])
            XCTAssertTrue(link.accessibilityTraits.contains(.link), "a link element is a link")
            XCTAssertFalse(link.accessibilityTraits.contains(.button), "and not also a button")
        }
        XCTAssertTrue(try XCTUnwrap(p.views[2]).accessibilityTraits.contains(.button), "a button is a button")
        XCTAssertFalse(try XCTUnwrap(p.views[2]).accessibilityTraits.contains(.link))
        XCTAssertEqual(element(p.axElements(roots: [p.viewport]), "link")?["role"] as? String, "link", "`tree --ax` reads it as UIKit says it")
        // The role taken away, it is a button again.
        p.apply(wireBatch([["op": "props", "id": 7, "set": [:], "clear": ["accessibilityRole"]]]))
        XCTAssertTrue(try XCTUnwrap(p.views[7]).accessibilityTraits.contains(.button))
        XCTAssertFalse(try XCTUnwrap(p.views[7]).accessibilityTraits.contains(.link))
    }

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

    func testHiddenSubtreesReturnWhenTheirExclusionsAreCleared() throws {
        let p = try fixture()
        let box = try XCTUnwrap(p.views[4])
        box.accessibilityElementsHidden = true
        XCTAssertNil(element(p.axElements(roots: [p.viewport]), "inside"), "hidden from UIKit, so not exposed")
        let listed = try XCTUnwrap(element(p.axElements(roots: [p.viewport], excluded: true), "inside"))
        XCTAssertEqual(listed["excluded"] as? [String], ["elementsHidden"])
        // A negative through the walk: cleared after mount, the subtree is exposed again.
        box.accessibilityElementsHidden = false
        XCTAssertNotNil(element(p.axElements(roots: [p.viewport]), "inside"))
        // UIKit's settled snapshot omits a fully transparent subtree.
        box.alpha = 0
        let ax = p.axElements(roots: [p.viewport])
        XCTAssertNil(element(ax, "inside"))
        XCTAssertEqual((ax["coverage"] as? [String: Any])?["limits"] as? [String], ["reading-order"])
    }

    func testAnEmptyNativeContainerExcludesItsSubtreeUntilBothAxesHaveArea() throws {
        let p = try fixture()
        let holder = UIView(frame: CGRect(x: 0, y: 100, width: 400, height: 100))
        holder.addSubview(try XCTUnwrap(p.views[5]))
        p.viewport.addSubview(holder)
        for size in [CGSize(width: 400, height: 0), CGSize(width: 0, height: 100)] {
            holder.bounds.size = size
            XCTAssertNil(element(p.axElements(roots: [p.viewport]), "inside"))
            let hidden = try XCTUnwrap(element(p.axElements(roots: [p.viewport], excluded: true), "inside"))
            XCTAssertEqual(hidden["excluded"] as? [String], ["emptyBounds"])
        }
        holder.bounds.size = CGSize(width: 1, height: 1)
        XCTAssertNotNil(element(p.axElements(roots: [p.viewport]), "inside"), "a point on both axes is enough")
    }

    func testAZeroScaleOrFullyTransparentContainerIsAbsentFromTheSettledTree() throws {
        let p = try fixture(), box = try XCTUnwrap(p.views[4])
        box.transform = CGAffineTransform(scaleX: 0, y: 0)
        XCTAssertNil(element(p.axElements(roots: [p.viewport]), "inside"))
        XCTAssertEqual(element(p.axElements(roots: [p.viewport], excluded: true), "inside")?["excluded"] as? [String], ["emptyFrame"])
        box.transform = .identity
        box.alpha = 0
        XCTAssertNil(element(p.axElements(roots: [p.viewport]), "inside"))
        XCTAssertEqual(element(p.axElements(roots: [p.viewport], excluded: true), "inside")?["excluded"] as? [String], ["transparent"])
        box.alpha = 0.001
        XCTAssertNotNil(element(p.axElements(roots: [p.viewport]), "inside"), "UIKit exposes even this faint subtree")
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
        let many = Many(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
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
        // A field is an element only with UIKit's accessibility runtime.
        guard Presenter.axRuntimeLoaded else { throw missing("accessibility", "UIKit's accessibility runtime is not loaded") }
        let secure = try XCTUnwrap(elements(p.axElements(roots: [p.viewport])).first { $0["role"] as? String == "textbox" })
        XCTAssertNil(secure["value"])
        XCTAssertEqual((secure["states"] as? [String: Any])?["protected"] as? Bool, true)
    }

    /// A button with no name that shows only a symbol is named by it, as a
    /// `UIButton` with an SF Symbol image is ("Share"); one with words, or
    /// whose symbol is `aria-hidden`, is not.
    func testASymbolOnlyButtonIsNamedByItsSymbol() throws {
        guard Presenter.axRuntimeLoaded else { throw missing("accessibility", "UIKit names a symbol only with its accessibility runtime") }
        let p = try fixture()
        let symbol: [String: Any] = ["imageSource": "symbol:sf/square.and.arrow.up", "symbolName": "square.and.arrow.up"]
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "image", "props": symbol, "style": ["font_size": 17]],
            ["op": "create", "id": 31, "kind": "button", "handlers": ["press"]],
            ["op": "create", "id": 32, "kind": "image", "props": symbol, "style": ["font_size": 17]],
            ["op": "create", "id": 33, "kind": "text", "props": ["text": "Send"]],
            ["op": "create", "id": 34, "kind": "button", "handlers": ["press"]],
            ["op": "create", "id": 35, "kind": "image", "props": symbol.merging(["accessibilityElementsHidden": "true"]) { a, _ in a }, "style": ["font_size": 17]],
            ["op": "children", "id": 3, "ids": [30]],
            ["op": "children", "id": 31, "ids": [32, 33]],
            ["op": "children", "id": 34, "ids": [35]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 31, 34]],
            ["op": "frame", "id": 31, "x": 0.0, "y": 210.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 34, "x": 0.0, "y": 260.0, "w": 40.0, "h": 40.0],
        ]))
        p.syncAccessibility()
        let share = try XCTUnwrap(UIImage(systemName: "square.and.arrow.up")?.accessibilityLabel)
        XCTAssertEqual(p.views[3]?.accessibilityLabel, share)
        XCTAssertEqual(p.views[31]?.accessibilityLabel, "Send")
        XCTAssertEqual(p.views[34]?.accessibilityLabel ?? "", "")
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

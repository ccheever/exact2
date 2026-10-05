#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1069.011 on UIKit: a `Control` of type `button` is UIKit's own
/// `UIButton` with the configuration its `buttonStyles` row names, its title
/// and symbol the node's face (D2, D5); its primary action is a custom
/// button's touch-up, once (D4); the node keeps keys and focus and the
/// control is the one accessibility element (D4); a glass style's control is
/// isolated in a glass group (D9); a select stays a select (D3).
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class NativeButtonsIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter(_ ops: [[String: Any]], faces: [UInt32: ButtonFace] = [:]) -> Presenter {
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        p.selectOptions = { _ in SelectMenu(options: [.init(value: "a", label: "A", disabled: false)], chosen: 0) }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }
    private func face(_ title: String?, symbol: String? = nil, style: String = "bordered", ios: String = "bordered") -> ButtonFace {
        var f = ButtonFace()
        f.title = title; f.symbol = symbol; f.style = style; f.ios = ios; f.iosBefore26 = "bordered"
        return f
    }
    private func native(_ id: Int, _ props: [String: String] = [:], handlers: [String] = ["press"], x: Double = 0) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control", "props": ["type": "button", "accessibilityRole": "button"].merging(props) { $1 },
          "handlers": handlers, "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": x, "y": 0.0, "w": 120.0, "h": 34.0]]
    }
    private func box(_ id: Int, _ props: [String: String] = [:], handlers: [String] = [], w: Double = 300) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "view", "props": props, "handlers": handlers, "style": ["text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": w, "h": 40.0]]
    }
    private func button(_ p: Presenter, _ id: UInt32) throws -> NativeButtonIOS {
        try XCTUnwrap(p.controls.controls[id] as? NativeButtonIOS)
    }

    /// b6 review B7: a cleared date reads as no date until a value is
    /// applied or chosen, as the Mac's `DateField.empty`; a `UIDatePicker`
    /// always holds some date, so the picker records it.
    func testAClearedDateReadsEmptyUntilAValueComes() throws {
        let p = presenter([["op": "create", "id": 10, "kind": "control", "props": ["type": "date", "value": "2026-06-01"], "handlers": ["change"], "style": [:]],
                           ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 140.0, "h": 34.0], ["op": "roots", "ids": [10]]])
        let node = try XCTUnwrap(p.views[10]), picker = try XCTUnwrap(p.controls.controls[10] as? UIDatePicker)
        XCTAssertEqual(p.controls.valueObservation(picker)?["value"] as? String, "2026-06-01")
        let cleared = try XCTUnwrap(p.controls.type(node, ""))
        XCTAssertNil(cleared["error"], "\(cleared)")
        XCTAssertEqual(p.controls.valueObservation(picker)?["value"] as? String, "", "cleared reads empty")
        XCTAssertNotNil(p.controls.type(node, "2026-07-04"))
        XCTAssertEqual(p.controls.valueObservation(picker)?["value"] as? String, "2026-07-04")
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["value": ""]]]))
        XCTAssertEqual(p.controls.valueObservation(picker)?["value"] as? String, "", "an empty bound value")
    }

    func testItIsUIKitsButtonWithItsFaceAndStyle() throws {
        let p = presenter(box(1) + native(2, ["testId": "send"]) + native(3) + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Send", symbol: "paperplane", style: "filled", ios: "filled"), 3: face("Next", style: "plain", ios: "plain")])
        let send = try button(p, 2), next = try button(p, 3)
        XCTAssertEqual(send.configuration?.title, "Send")
        XCTAssertNotNil(send.configuration?.image)
        XCTAssertEqual(send.configuration?.imagePlacement, .leading)
        XCTAssertEqual(send.drawn, "filled")
        XCTAssertEqual(next.drawn, "plain")
        XCTAssertTrue(send.superview === p.views[2], "mounted on its node")
        XCTAssertEqual(send.frame, p.views[2]?.bounds, "it fills the node's box")
        let seen = try XCTUnwrap(p.controls.observation(try XCTUnwrap(p.views[2])))
        XCTAssertEqual(seen["view"] as? String, "UIButton")
        XCTAssertEqual(seen["style"] as? String, "filled")
        XCTAssertEqual(send.accessibilityLabel, "Send", "the title names it")
        XCTAssertEqual(send.accessibilityIdentifier, "send")
        XCTAssertEqual(p.views[2]?.accessibleName, "Send", "the agent's name for it is its title")
        XCTAssertFalse(try XCTUnwrap(p.views[2]).isAccessibilityElement, "the control is the element, not the node")
    }

    func testItsActionPressesOnceItsOwnOrAnAncestorsHandler() throws {
        let p = presenter(box(1, handlers: ["press"]) + native(2) + native(3, handlers: []) + native(4, ["disabled": "true"])
                          + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Own"), 3: face("Ancestor's"), 4: face("Off")])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        try button(p, 2).sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(pressed, [2])
        try button(p, 3).sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(pressed, [2, 1], "no handler of its own: the nearest ancestor's")
        XCTAssertFalse(try button(p, 4).isEnabled)
        try XCTUnwrap(p.views[4]).activateNative()
        XCTAssertEqual(pressed, [2, 1], "disabled: no press")
    }

    func testItsActionMovesFocusAsACustomButtonsTouchDoes() throws {
        let p = presenter(box(1) + box(5, ["retainFocus": "true"]) + native(2) + native(3)
                          + [["op": "create", "id": 4, "kind": "input", "props": [:], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
                             ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
                             ["op": "children", "id": 5, "ids": [3]], ["op": "children", "id": 1, "ids": [2, 4, 5]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Go"), 3: face("Keep")])
        p.onPress = { _ in }
        let field = try XCTUnwrap(p.views[4]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        try button(p, 3).sendActions(for: .primaryActionTriggered)
        XCTAssertTrue(field.isFirstResponder, "a retainFocus ancestor keeps the editor")
        try button(p, 2).sendActions(for: .primaryActionTriggered)
        XCTAssertFalse(field.isFirstResponder, "otherwise the press takes the focus")
        XCTAssertTrue(try XCTUnwrap(p.views[2]).isFirstResponder, "to the button's node, the focus owner")
    }

    func testItTakesTheFocusWithNoPressAnywhere() throws {
        let p = presenter(box(1) + native(2, handlers: ["focus"])
                          + [["op": "create", "id": 3, "kind": "input", "props": [:], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
                             ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
                             ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Mark")])
        var focused: [UInt32] = []
        p.onFocus = { focused.append($0) }
        let field = try XCTUnwrap(p.views[3]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        try button(p, 2).sendActions(for: .primaryActionTriggered)
        XCTAssertTrue(try XCTUnwrap(p.views[2]).isFirstResponder, "a custom button's touch focuses it too")
        XCTAssertEqual(focused, [2])
    }

    func testAPanCancelsItsTouchAsACustomButtons() {
        let scroll = ScrollView()
        XCTAssertTrue(scroll.touchesShouldCancel(in: NativeButtonIOS(configuration: .bordered())))
        XCTAssertFalse(scroll.touchesShouldCancel(in: UISwitch()), "other controls keep UIKit's rule")
    }

    func testASelectStaysASelect() throws {
        let p = presenter(box(1) + native(2)
                          + [["op": "create", "id": 3, "kind": "control", "props": ["type": "select"], "handlers": ["change"], "style": ["text_color": [0, 0, 0, 255]]],
                             ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
                             ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face("Go")])
        let select = try XCTUnwrap(p.controls.controls[3] as? UIButton)
        XCTAssertFalse(select is NativeButtonIOS)
        XCTAssertEqual(p.controls.observation(try XCTUnwrap(p.views[3]))?["view"] as? String, "UIButton(pop-up)")
        XCTAssertNil(p.controls.activate(try XCTUnwrap(p.views[2])), "a native button takes the ordinary tap path")
        XCTAssertNil(p.controls.unopened(try XCTUnwrap(p.views[2])))
        XCTAssertNil(p.controls.type(try XCTUnwrap(p.views[2]), "x"))
    }

    func testAGlassButtonIsIsolatedInItsGroupAndGivenBack() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("Liquid Glass is iOS 26") }
        var faces: [UInt32: ButtonFace] = [2: face("Lock", style: "glass", ios: "glass"), 3: face("Fade", style: "glass", ios: "glass")]
        let p = presenter(box(1, ["glassGroup": "24"]) + native(2) + native(3, x: 130)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]], faces: faces)
        let fade = try button(p, 3), node = try XCTUnwrap(p.views[3])
        XCTAssertTrue(fade.isGlass)
        let slot = try XCTUnwrap(node.glassSlot, "a glass button in a group gets a slot")
        XCTAssertTrue(fade.superview === slot.contentView)
        XCTAssertNil(slot.effect, "joined")
        p.apply(wireBatch([["op": "present", "id": 3, "property": "opacity", "x": 0.3, "y": 0.0, "w": 0.0, "h": 0.0]]))
        XCTAssertNotNil(slot.effect, "fading: isolated")
        p.apply(wireBatch([["op": "present", "id": 3, "property": "opacity", "x": 1.0, "y": 0.0, "w": 0.0, "h": 0.0]]))
        XCTAssertNil(slot.effect)
        // Two more batches: the control host and the pass leave it where it is.
        p.apply(wireBatch([])); p.apply(wireBatch([]))
        XCTAssertTrue(fade.superview === slot.contentView)
        // Glass to plain: the button back on its node, the slot gone.
        faces[3] = face("Fade", style: "plain", ios: "plain")
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["testId": "plain"], "clear": []]]))
        XCTAssertFalse(fade.isGlass)
        XCTAssertNil(node.glassSlot)
        XCTAssertTrue(fade.superview === node)
    }

    func testALeavingButtonKeepsItsControlUntilItsExitEnds() throws {
        let p = presenter(box(1) + native(2) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]], faces: [2: face("Bye")])
        let b = try button(p, 2)
        p.beginExit(2)
        p.apply(wireBatch([]))
        XCTAssertTrue(p.controls.controls[2] === b, "kept while it draws")
        XCTAssertNotNil(b.superview)
        _ = p.endExit(2)
        p.apply(wireBatch([]))
        XCTAssertNil(p.controls.controls[2])
    }

    /// A face is asked for once, then again only when a batch says the
    /// control's contents changed (`controls`) or touches the node: the
    /// batches of a fling ask nothing.
    func testAFaceIsAskedForOnlyWhenItCanHaveChanged() throws {
        var faces: [UInt32: ButtonFace] = [2: face("Send")]
        var asked = 0
        let p = presenter(box(1) + native(2) + box(3) + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        p.buttonFace = { asked += 1; return faces[$0] ?? ButtonFace() }
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["testId": "send"]]]))
        XCTAssertEqual(asked, 1, "a touched button is asked again")
        XCTAssertEqual(try button(p, 2).configuration?.title, "Send")
        for _ in 0..<5 { p.apply(wireBatch([["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 300.0, "h": 40.0]])) }
        XCTAssertEqual(asked, 1, "batches that leave it alone ask nothing")
        faces[2] = face("Sent")
        var contents = wireBatch([["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0]])
        contents.controls = true
        p.apply(contents)
        XCTAssertEqual(asked, 2)
        XCTAssertEqual(try button(p, 2).configuration?.title, "Sent", "a batch that changed its contents shows the new face")
        // Its natural size is measured again for a new face, not kept.
        let short = try button(p, 2).naturalSize
        faces[2] = face("Sent to everyone in the group")
        p.apply(contents)
        XCTAssertGreaterThan(try button(p, 2).naturalSize.width, short.width)
        let measured = try button(p, 2).intrinsicContentSize
        XCTAssertEqual(try button(p, 2).naturalSize, CGSize(width: ceil(measured.width), height: ceil(measured.height)), "what UIKit measures, rounded up")
        // And for a larger text size.
        let regular = try button(p, 2).naturalSize
        try button(p, 2).traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraLarge
        try button(p, 2).layoutIfNeeded()
        XCTAssertGreaterThan(try button(p, 2).naturalSize.height, regular.height)
    }
}
#endif

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
        XCTAssertTrue(send.isAccessibilityElement, "the native button exposes its explicit name even before UIKit loads its accessibility runtime")
        XCTAssertTrue(send.accessibilityTraits.contains(.button))
        XCTAssertEqual(p.views[2]?.accessibleName, "Send", "the agent's name for it is its title")
        XCTAssertFalse(try XCTUnwrap(p.views[2]).isAccessibilityElement, "the control is the element, not the node")
        let ax = p.axElements(roots: [p.viewport])
        let named = (ax["elements"] as? [[String: Any]])?.filter { $0["testId"] as? String == "send" } ?? []
        XCTAssertEqual(named.count, 1, "the platform tree exposes the native button once")
        XCTAssertEqual(named.first?["name"] as? String, "Send")
        XCTAssertEqual(named.first?["role"] as? String, "button")
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

    func testAuthoredColoursFollowALightToDarkSwitch() throws {
        var f = face("Scheme", symbol: "lock.fill", style: "filled", ios: "filled")
        let pair: BatchValue = .array([.array([.number(0), .number(0), .number(0), .number(255)]),
            .array([.number(255), .number(255), .number(255), .number(255)])])
        f.rows.title["text_color"] = pair; f.rows.symbol["tint_color"] = pair
        let p = presenter(native(2) + [["op": "roots", "ids": [2]]], faces: [2: f])
        window.overrideUserInterfaceStyle = .light; window.updateTraitsIfNeeded()
        let b = try button(p, 2); b.updateTraitsIfNeeded(); p.controls.sync()
        let old = b.written
        XCTAssertEqual(b.configuration?.baseForegroundColor?.cgColor.components, [0, 0, 0, 1])
        window.overrideUserInterfaceStyle = .dark; window.updateTraitsIfNeeded(); b.updateTraitsIfNeeded(); p.controls.sync()
        XCTAssertNotEqual(b.written, old, "appearance invalidates the title and baked symbol tint")
        XCTAssertEqual(b.configuration?.baseForegroundColor?.cgColor.components, [1, 1, 1, 1])
        XCTAssertEqual(b.configuration?.image?.renderingMode, .alwaysOriginal)
    }
    private final class LocatedTouch: UITouch {
        let target: UIView
        init(_ target: UIView) { self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { target.convert(CGPoint(x: target.bounds.midX, y: target.bounds.midY), to: view) }
    }
    func testProductionTouchesLightDismissAndHideANativeInvokersPopover() throws {
        let p = presenter(box(1) + native(2, ["popovertarget": "choices"]) + native(3)
            + box(4, ["popover": "auto", "id": "choices"]) + native(5, ["popovertarget": "choices", "popovertargetaction": "hide"], handlers: [])
            + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "children", "id": 4, "ids": [5]], ["op": "roots", "ids": [1]]],
            faces: [2: face("Open"), 3: face("Outside"), 5: face("Hide")])
        let pop = try XCTUnwrap(p.views[4]), opener = try button(p, 2)
        opener.sendActions(for: .primaryActionTriggered)
        XCTAssertTrue(p.menus.isOpen(pop))
        let watcher = try XCTUnwrap(p.viewport.gestureRecognizers?.first { String(describing: type(of: $0)) == "PopoverTouch" }, "production touch observer")
        XCTAssertFalse(watcher.cancelsTouchesInView); XCTAssertFalse(watcher.delaysTouchesBegan)
        watcher.touchesBegan([LocatedTouch(opener)], with: UIEvent())
        XCTAssertTrue(p.menus.isOpen(pop), "its invoker is excluded from light dismiss")
        watcher.reset()
        watcher.touchesBegan([LocatedTouch(try button(p, 5))], with: UIEvent())
        XCTAssertTrue(p.menus.isOpen(pop), "a touch inside keeps it until activation")
        try button(p, 5).sendActions(for: .primaryActionTriggered)
        XCTAssertFalse(p.menus.isOpen(pop), "hide-only native button closes it")
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["commandfor": "choices", "command": "hide-popover"]]]))
        opener.sendActions(for: .primaryActionTriggered)
        try button(p, 5).sendActions(for: .primaryActionTriggered)
        XCTAssertFalse(p.menus.isOpen(pop), "the command invoker closes it too")
        opener.sendActions(for: .primaryActionTriggered)
        watcher.reset(); watcher.touchesBegan([LocatedTouch(try button(p, 3))], with: UIEvent())
        XCTAssertFalse(p.menus.isOpen(pop), "outside touch reaches production dismissal without agentTap")
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

    private func selectMenu() -> SelectMenu {
        SelectMenu(options: [.init(value: "plain", label: "Plain", disabled: false),
                             .init(value: "wide", label: "Chocolate Strawberry", disabled: false),
                             .init(value: "off", label: "Unavailable", disabled: true)], chosen: 0)
    }

    private func selectPresenter(_ menu: SelectMenu) -> Presenter {
        let p = presenter([["op": "create", "id": 3, "kind": "control", "props": ["type": "select"],
                            "handlers": ["input", "change"], "style": [:]],
                           ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 320.0, "h": 52.0],
                           ["op": "roots", "ids": [3]]])
        p.selectOptions = { _ in menu }
        var batch = wireBatch([])
        batch.controls = true
        p.apply(batch)
        return p
    }

    private func popupReference(_ menu: SelectMenu, alignment: UIControl.ContentHorizontalAlignment) -> UIButton {
        var configuration = UIButton.Configuration.plain()
        configuration.indicator = .popup
        configuration.contentInsets = .zero
        let button = UIButton(configuration: configuration)
        button.showsMenuAsPrimaryAction = true
        button.changesSelectionAsPrimaryAction = true
        button.contentHorizontalAlignment = alignment
        button.menu = UIMenu(children: menu.options.enumerated().map { i, option in
            UIAction(title: option.label, attributes: option.disabled ? .disabled : [],
                     state: i == menu.chosen ? .on : .off) { _ in }
        })
        window.addSubview(button)
        return button
    }

    private func popupTitleRect(_ button: UIButton) throws -> CGRect {
        button.setNeedsLayout()
        button.layoutIfNeeded()
        let title = try XCTUnwrap(button.titleLabel)
        XCTAssertFalse(title.bounds.isEmpty, "the native selected title must be laid out")
        return button.convert(title.bounds, from: title)
    }

    func testSelectAlignmentMovesItsNativeTitleWithoutRebuildingTheMenu() throws {
        let menu = selectMenu(), p = selectPresenter(selectMenu())
        let node = try XCTUnwrap(p.views[3]), select = try XCTUnwrap(p.controls.controls[3] as? UIButton)
        let natural = p.controls.naturalSize(select, node)
        var widestMenu = menu
        widestMenu.chosen = 1
        let widest = popupReference(widestMenu, alignment: .center)
        XCTAssertEqual(natural.width, ceil(widest.intrinsicContentSize.width), "the slot measures the widest native option")
        widest.removeFromSuperview()
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 0.0, "y": 0.0,
                            "w": Double(natural.width), "h": 52.0]]))
        let originalMenu = try XCTUnwrap(select.menu)
        let reference = popupReference(menu, alignment: .center)
        reference.frame = CGRect(origin: .zero, size: select.bounds.size)
        defer { reference.removeFromSuperview() }
        var titleRects: [String: CGRect] = [:]
        let cases: [(String?, UIControl.ContentHorizontalAlignment)] = [
            (nil, .center), ("right", .right), ("left", .left), ("center", .center), (nil, .center)
        ]
        for (alignment, expected) in cases {
            let style: [String: Any] = alignment.map { ["text_align": $0] } ?? [:]
            p.apply(wireBatch([["op": "style", "id": 3, "style": style]]))
            if reference.contentHorizontalAlignment != expected {
                reference.contentHorizontalAlignment = expected
                let configuration = reference.configuration
                reference.configuration = nil
                reference.configuration = configuration
            }
            let actual = try popupTitleRect(select), native = try popupTitleRect(reference)
            XCTAssertEqual(select.currentTitle, "Plain")
            XCTAssertEqual(actual.minX, native.minX, accuracy: 0.5, "restyled \(alignment ?? "default") popup matches UIKit")
            XCTAssertEqual(actual.width, native.width, accuracy: 0.5)
            XCTAssertTrue(select.menu === originalMenu, "a style-only change keeps the native menu")
            XCTAssertEqual(p.controls.naturalSize(select, node), natural, "alignment does not change widest-option sizing")
            XCTAssertEqual(select.frame.width, natural.width)
            titleRects[alignment ?? "default"] = actual
        }
        XCTAssertGreaterThan(try XCTUnwrap(titleRects["right"]).minX, try XCTUnwrap(titleRects["center"]).minX)
        XCTAssertLessThan(try XCTUnwrap(titleRects["left"]).minX, try XCTUnwrap(titleRects["center"]).minX)
        XCTAssertEqual(titleRects["default"], titleRects["center"], "clearing alignment restores UIKit's centering")
    }

    func testSelectLogicalEdgesRespectDirectionInsideAWidePaddedBox() throws {
        let p = selectPresenter(selectMenu())
        let node = try XCTUnwrap(p.views[3]), select = try XCTUnwrap(p.controls.controls[3] as? UIButton)
        let natural = p.controls.naturalSize(select, node), menu = try XCTUnwrap(select.menu)
        let cases: [(String, String, UIControl.ContentHorizontalAlignment)] = [
            ("start", "ltr", .left), ("end", "ltr", .right),
            ("start", "rtl", .right), ("end", "rtl", .left),
            ("left", "rtl", .left), ("right", "rtl", .right),
            ("justify", "ltr", .left), ("justify", "rtl", .right), ("center", "rtl", .center)
        ]
        for (alignment, direction, expected) in cases {
            p.apply(wireBatch([["op": "style", "id": 3, "style": ["text_align": alignment, "direction": direction,
                               "padding_left": 17.0, "padding_right": 29.0, "padding_top": 3.0, "padding_bottom": 7.0]]]))
            let content = node.contentBox()
            XCTAssertEqual(content.minX, 17)
            XCTAssertEqual(content.maxX, 291)
            let x = expected == .left ? content.minX : expected == .right ? content.maxX - natural.width : content.midX - natural.width / 2
            XCTAssertEqual(select.frame.minX, x, accuracy: 0.001)
            XCTAssertEqual(select.frame.midY, content.midY, accuracy: 0.001)
            XCTAssertEqual(select.frame.size, natural)
            XCTAssertEqual(select.contentHorizontalAlignment, expected)
            XCTAssertTrue(select.menu === menu)
            XCTAssertEqual(node.bounds.width, 320, "alignment preserves the authored slot")
        }
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["direction": "rtl"]]]))
        XCTAssertEqual(select.contentHorizontalAlignment, .center, "clearing alignment restores UIKit's centering even in RTL")
        XCTAssertEqual(select.frame.midX, node.contentBox().midX, accuracy: 0.001)
    }

    func testAlignedSelectKeepsItsChoiceEventsAndWidestOptionMeasurement() throws {
        var menu = selectMenu()
        let p = selectPresenter(menu)
        p.selectOptions = { _ in menu }
        let node = try XCTUnwrap(p.views[3]), select = try XCTUnwrap(p.controls.controls[3] as? UIButton)
        let natural = p.controls.naturalSize(select, node)
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["text_align": "right"]]]))
        var heard: [String] = []
        p.onControlValue = { id, value, input, change in
            XCTAssertEqual(id, 3)
            if input { heard.append("input:\(value)") }
            if change { heard.append("change:\(value)") }
            menu.chosen = menu.options.firstIndex { $0.value == value }
        }
        let choice = try XCTUnwrap(p.controls.type(node, "wide"))
        XCTAssertNil(choice["error"])
        XCTAssertEqual(heard, ["input:wide", "change:wide"])
        XCTAssertEqual(select.currentTitle, "Chocolate Strawberry")
        XCTAssertEqual(select.contentHorizontalAlignment, .right)
        XCTAssertTrue(select.showsMenuAsPrimaryAction)
        XCTAssertTrue(select.changesSelectionAsPrimaryAction)
        XCTAssertEqual(p.controls.naturalSize(select, node), natural)
        XCTAssertEqual(p.controls.valueObservation(select)?["value"] as? String, "wide")
        let reference = popupReference(menu, alignment: .right)
        reference.frame = CGRect(origin: .zero, size: select.bounds.size)
        XCTAssertEqual(try popupTitleRect(select).minX, try popupTitleRect(reference).minX, accuracy: 0.5)
        reference.removeFromSuperview()
        XCTAssertNotNil(p.controls.type(node, "off")?["error"], "disabled menu options stay unavailable")
        XCTAssertEqual(heard, ["input:wide", "change:wide"], "a refused choice emits nothing")
        XCTAssertEqual(select.currentTitle, "Chocolate Strawberry")
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

    func testNativeWorldLayoutFollowsAnEqualBoundsChildsAncestor() {
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        let ancestor = UIView(frame: CGRect(x: 20, y: 100, width: 200, height: 100))
        let child = NativeWorldLayoutProbe(frame: CGRect(x: 10, y: 20, width: 14, height: 14))
        window.addSubview(ancestor); ancestor.addSubview(child)
        child.layoutIfNeeded()
        var geometry = NativeWorldLayout()
        geometry.refresh(child); child.layoutIfNeeded()
        let before = child.layouts.count, bounds = child.bounds
        geometry.refresh(child); child.layoutIfNeeded()
        XCTAssertEqual(child.layouts.count, before, "unchanged geometry does no layout work")
        ancestor.frame.origin.y += 1.0 / 6.0
        child.layoutIfNeeded()
        XCTAssertEqual(child.bounds, bounds)
        XCTAssertEqual(child.layouts.count, before, "UIKit does not lay out the fixed child for an ancestor move")
        geometry.refresh(child); child.layoutIfNeeded()
        XCTAssertEqual(child.layouts.count, before + 1)
        XCTAssertEqual(child.layouts.last?.minY ?? 0, 120 + 1.0 / 6.0, accuracy: 0.0001)
    }

    func testNativeWorldLayoutRearmsAfterDetachAndDoesNotRetainAWindow() {
        var geometry = NativeWorldLayout()
        let child = NativeWorldLayoutProbe(frame: CGRect(x: 10, y: 20, width: 14, height: 14))
        weak var released: UIWindow?
        autoreleasepool {
            let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
            released = window
            window.addSubview(child)
            geometry.refresh(child); child.layoutIfNeeded()
            child.removeFromSuperview()
            geometry.refresh(child)
            window.addSubview(child); child.layoutIfNeeded()
            let before = child.layouts.count
            geometry.refresh(child); child.layoutIfNeeded()
            XCTAssertEqual(child.layouts.count, before + 1, "same coordinates after a detach are a fresh layout")
            child.removeFromSuperview()
        }
        XCTAssertNil(released, "the cached window identity must not retain the window")
    }
}

private final class NativeWorldLayoutProbe: UIView {
    var layouts: [CGRect] = []
    override func layoutSubviews() {
        super.layoutSubviews()
        layouts.append(convert(bounds, to: window))
    }
}
#endif

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1069.011.000 on UIKit: a native button goes where a button goes, read
/// through its face (D1) — a swipe action drawn from its symbol and accent
/// (D6), a tab of a tab bar or a segment (D4), a menu row whose symbol is its
/// item's image, as a custom row's now is, and a confirmation's rows (D5).
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class NativeContextsIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter(_ ops: [[String: Any]], faces: [UInt32: ButtonFace]) -> Presenter {
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }
    private func face(_ title: String?, symbol: String? = nil, label: String? = nil) -> ButtonFace {
        var f = ButtonFace()
        f.title = title; f.symbol = symbol; f.label = label
        return f
    }
    private func native(_ id: Int, _ props: [String: String] = [:], style: [String: Any] = [:], x: Double = 0, w: Double = 80, h: Double = 40) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control", "handlers": ["press"],
          "props": ["type": "button", "accessibilityRole": "button"].merging(props) { $1 },
          "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]].merging(style) { $1 }],
         ["op": "frame", "id": id, "x": x, "y": 0.0, "w": w, "h": h]]
    }
    private func view(_ id: Int, _ props: [String: String] = [:], style: [String: Any] = [:], x: Double = 0, w: Double = 300, h: Double = 40) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "view", "props": props, "handlers": [], "style": ["text_color": [0, 0, 0, 255]].merging(style) { $1 }],
         ["op": "frame", "id": id, "x": x, "y": 0.0, "w": w, "h": h]]
    }

    func testNativeMenuItemKeepsItsSubtitle() throws {
        var f = face("Last Parked"); f.subtitle = "Updated just now"
        let p = presenter(view(1, ["popover": "auto", "id": "menu"])
            + native(2) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]], faces: [2: f])
        defer { window.isHidden = true }
        let item = try XCTUnwrap(p.menus.items(of: try XCTUnwrap(p.views[1])).first as? UIAction)
        XCTAssertEqual(item.title, "Last Parked")
        XCTAssertEqual(item.subtitle, "Updated just now")
    }
    /// A bare row (its second text on an `if` arm keeps it bare in place)
    /// whose second text is a line under its first is still title and
    /// subtitle in UIKit's menu, which draws no box: euxy's sort menu showed
    /// "Date Added Newest first" as a two-line title. Two texts side by side
    /// (`Inbox` and its count, `Save` and `⌘S`) stay one title.
    func testABareMenuRowsStackedSecondTextIsItsSubtitle() throws {
        var ops: [[String: Any]] = view(1, ["popover": "auto", "id": "menu"])
        /// A row of `texts`, the second styled `second`.
        func row(_ id: Int, _ texts: [String], display: String? = nil, second: [String: Any] = [:]) {
            let kids = texts.indices.map { 100 * id + $0 }
            var style: [String: Any] = ["text_color": [0, 0, 0, 255]]
            if let display { style["display"] = display }
            ops.append(["op": "create", "id": id, "kind": "button", "handlers": ["press"], "props": [:], "style": style])
            for (i, (kid, text)) in zip(kids, texts).enumerated() {
                ops.append(["op": "create", "id": kid, "kind": "text", "props": ["text": text], "style": i == 1 ? second : [:]])
            }
            ops.append(["op": "children", "id": id, "ids": kids])
        }
        row(2, ["Date Added", "Newest first"])
        row(3, ["BPM"])
        row(4, ["Tempo", "Fastest first"], display: "flex")
        row(5, ["Inbox", "3"], display: "flex")
        row(6, ["Save", "⌘S"], second: ["display": "inline"])
        // A count positioned at the row's top right, in a block row and in a
        // flex column: out of the flow, no line under the title.
        row(7, ["Drafts", "12"], second: ["position_type": "absolute"])
        row(8, ["Drafts", "12"], display: "flex", second: ["position_type": "absolute"])
        ops += [["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7, 8]], ["op": "roots", "ids": [1]]]
        var column = ButtonFace(); column.placement = "top"
        var across = ButtonFace(); across.placement = "leading"
        let p = presenter(ops, faces: [4: column, 5: across, 8: column])
        defer { window.isHidden = true }
        let items = p.menus.items(of: try XCTUnwrap(p.views[1])).compactMap { $0 as? UIAction }
        XCTAssertEqual(items.map(\.title), ["Date Added", "BPM", "Tempo", "Inbox 3", "Save ⌘S", "Drafts 12", "Drafts 12"])
        XCTAssertEqual(items.map(\.subtitle), ["Newest first", nil, "Fastest first", nil, nil, nil, nil])
    }
    func testCustomHideOnlyButtonClosesNativeContentPopoverInProduction() throws {
        XCTAssertFalse(ExactEnv.agentMode, "this test exercises production handling")
        let p = presenter(native(1, ["popovertarget": "content"])
            + view(2, ["popover": "auto", "id": "content"], h: 120)
            + [["op": "create", "id": 3, "kind": "button", "props": ["popovertarget": "content", "popovertargetaction": "hide"], "handlers": []],
               ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 100, "h": 40],
               ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1, 2]]], faces: [1: face("Open")])
        defer { p.menus.reset(); window.isHidden = true }
        let opener = try XCTUnwrap(p.controls.controls[1] as? UIButton)
        opener.sendActions(for: .touchUpInside)
        let pop = try XCTUnwrap(p.views[2]), closer = try XCTUnwrap(p.views[3])
        XCTAssertTrue(p.menus.lifted(pop))
        p.menus.lightDismiss(closer)
        XCTAssertTrue(p.menus.lifted(pop), "an inside touch does not light dismiss")
        XCTAssertTrue(closer.accessibilityActivate(), "the production touch activation rule selects a hide-only button")
        XCTAssertFalse(p.menus.lifted(pop)); XCTAssertTrue(pop.isHidden)
    }

    func testLiftedContentKeepsItsAuthoredARIAUnavailableAncestor() throws {
        let p = presenter(view(9) + native(1, ["popovertarget": "content"])
            + view(2, ["popover": "auto", "id": "content"], h: 120)
            + native(3) + view(4)
            + [["op": "children", "id": 2, "ids": [3, 4]],
               ["op": "children", "id": 9, "ids": [1, 2]], ["op": "roots", "ids": [9]]],
            faces: [1: face("Open"), 3: face("Inside")])
        defer { p.menus.reset(); window.isHidden = true }
        p.press(1)
        let pop = try XCTUnwrap(p.views[2]), node = try XCTUnwrap(p.views[3])
        XCTAssertTrue(p.menus.lifted(pop))
        p.apply(wireBatch([["op": "props", "id": 9, "set": ["accessibilityDisabled": "true"]]]))
        let button = try XCTUnwrap(p.controls.controls[3] as? UIButton)
        XCTAssertTrue(node.ariaUnavailable)
        XCTAssertTrue(button.accessibilityTraits.contains(.notEnabled))
        XCTAssertTrue(button.isEnabled)
        p.apply(wireBatch([["op": "props", "id": 9, "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertFalse(node.ariaUnavailable)
        XCTAssertFalse(button.accessibilityTraits.contains(.notEnabled))
    }
    func testANativeSwipeActionIsDrawnFromItsFace() throws {
        let p = presenter(
            view(1, ["swipeContent": "body", "swipeTrailing": "delete mute pin flag archive"], style: ["overflow_x": "scroll", "overflow_y": "hidden"])
                + view(2, w: 540, h: 80) + view(3, ["id": "body"], h: 80)
                // An accent it only inherits (LLP 1115.000 D2): the value, and no word that its author wrote it.
                + native(11, ["id": "archive", "accessibilityLabel": "Archive"], style: ["accent_color": [88, 86, 214, 255]], x: 490, w: 50, h: 50)
                + native(4, ["id": "delete", "destructive": "true"], style: ["accent_color": [0, 255, 0, 255]], x: 300, w: 50, h: 50)
                + native(5, ["id": "mute", "accessibilityLabel": "Hide alerts"], style: ["accent_color": [88, 86, 214, 255], "written": ["accent_color"]], x: 360, w: 50, h: 50)
                + [["op": "create", "id": 6, "kind": "button", "handlers": ["press"], "props": ["id": "pin", "accessibilityLabel": "Pin"], "style": ["text_color": [0, 0, 0, 255]]],
                   ["op": "frame", "id": 6, "x": 410.0, "y": 0.0, "w": 40.0, "h": 50.0],
                   ["op": "create", "id": 9, "kind": "button", "handlers": ["press"], "props": ["id": "flag", "accessibilityLabel": "Flag"], "style": ["text_color": [0, 0, 0, 255]]],
                   ["op": "frame", "id": 9, "x": 450.0, "y": 0.0, "w": 40.0, "h": 50.0]]
                // Each custom action's text child: Pin's shown, Flag's authored hidden.
                + [["op": "create", "id": 7, "kind": "text", "props": [:], "handlers": [], "style": [:]],
                   ["op": "create", "id": 10, "kind": "text", "props": [:], "handlers": [], "style": ["display": "none"]],
                   ["op": "children", "id": 6, "ids": [7]], ["op": "children", "id": 9, "ids": [10]]]
                + [["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3, 4, 5, 6, 9, 11]], ["op": "roots", "ids": [1]],
                   ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0], ["op": "content", "id": 1, "x": 0.0, "y": 0.0, "w": 540.0, "h": 80.0]],
            faces: [4: face("Delete", symbol: "trash"), 5: face(nil, symbol: "bell", label: "Hide alerts"), 6: face("Pin", symbol: "pin", label: "Pin"), 9: face("Flag", symbol: "flag", label: "Flag"),
                    11: face(nil, symbol: "archivebox", label: "Archive")])
        let owner = try XCTUnwrap(p.views[1])
        p.swipeActions.touch(owner)
        let table = try XCTUnwrap(owner.subviews.compactMap { $0 as? UITableView }.first)
        XCTAssertTrue(table.touchesShouldCancel(in: NativeButtonIOS(configuration: .bordered())), "a pan takes a native body button's touch")
        let configuration = try XCTUnwrap(table.delegate?.tableView?(table, trailingSwipeActionsConfigurationForRowAt: IndexPath(row: 0, section: 0)))
        XCTAssertEqual(configuration.actions.count, 5)
        let delete = configuration.actions[0], mute = configuration.actions[1], pin = configuration.actions[2], flag = configuration.actions[3]
        XCTAssertEqual(delete.style, .destructive)
        XCTAssertNotNil(delete.image, "its symbol")
        XCTAssertEqual(delete.title, "Delete", "and its visible title under it")
        XCTAssertNil(mute.title, "a symbol alone shows no title")
        XCTAssertEqual(pin.title, "Pin", "a custom action's too")
        XCTAssertNil(flag.title, "but not a title authored hidden")
        let red = UIContextualAction(style: .destructive, title: nil) { _, _, _ in }.backgroundColor
        XCTAssertEqual(delete.backgroundColor, red, "UIKit's red: a destructive action's accent sets nothing")
        XCTAssertEqual(pin.image?.isSymbolImage, true, "a custom action's symbol is a system image, not a snapshot")
        XCTAssertEqual(delete.accessibilityLabel, "Delete", "its title names it")
        XCTAssertEqual(mute.accessibilityLabel, "Hide alerts")
        XCTAssertEqual(mute.backgroundColor, TextEngine.color([88, 86, 214, 255]), "the accent its author wrote on it")
        XCTAssertEqual(configuration.actions[4].backgroundColor, UIContextualAction(style: .normal, title: nil) { _, _, _ in }.backgroundColor,
                       "UIKit's grey: an accent the action only inherits is not its own")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        mute.handler(mute, UIView()) { _ in }
        XCTAssertEqual(pressed, [5])
    }

    private func tablist(_ faces: [UInt32: ButtonFace], labels: [Int: String] = [:], role: String = "tablist") -> Presenter {
        var ops = view(1, ["accessibilityRole": role])
        for (i, id) in [2, 3, 4].enumerated() {
            var props = ["accessibilityRole": role == "radiogroup" ? "radio" : "tab"]
            if i == 0 { props[role == "radiogroup" ? "accessibilityChecked" : "accessibilitySelected"] = "true" }
            if let label = labels[id] { props["accessibilityLabel"] = label }
            ops += native(id, props, x: Double(i) * 100, w: 100)
        }
        return presenter(ops + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]], faces: faces)
    }

    /// A written `appearance="none"` keeps a radio group's authored keys, as it
    /// keeps a tablist's: the author's design, not `UISegmentedControl`, with
    /// WAI-ARIA's radio group keys: one Tab stop (the checked radio), and the
    /// arrows focus and press the next or previous radio, wrapping.
    func testAWrittenAppearanceNoneKeepsAnAuthoredRadioGroup() throws {
        let p = tablist([2: face("Day"), 3: face("Week"), 4: face("Month")], role: "radiogroup")
        XCTAssertNotNil(p.segments.control(of: 1), "a radio group is a segmented control")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["appearance": "none", "written": ["appearance"]]]]))
        XCTAssertNil(p.segments.control(of: 1))
        XCTAssertFalse(try XCTUnwrap(p.views[2]).isHidden, "its authored radios show")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.press(3)
        XCTAssertEqual(pressed, [3])
        let (day, week, month) = (try XCTUnwrap(p.views[2]), try XCTUnwrap(p.views[3]), try XCTUnwrap(p.views[4]))
        XCTAssertEqual([day, week, month].map(\.ariaRadioTabStop), [true, false, false], "the checked radio is the one stop")
        pressed = []
        XCTAssertTrue(p.controls.radioKey(day, "ArrowRight", held: ""))
        XCTAssertTrue(p.controls.radioKey(day, "ArrowLeft", held: ""))
        XCTAssertFalse(p.controls.radioKey(day, "a", held: ""))
        XCTAssertEqual(pressed, [3, 4], "the next radio, then the previous one wrapping")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityChecked": "false"]], ["op": "props", "id": 3, "set": ["accessibilityChecked": "true"]]]))
        XCTAssertEqual([day, week, month].map(\.ariaRadioTabStop), [false, true, false])
        // Right-to-left swaps Left and Right; a hidden radio is no target and no stop.
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["appearance": "none", "written": ["appearance"], "direction": "rtl"]]]))
        pressed = []
        XCTAssertTrue(p.controls.radioKey(week, "ArrowRight", held: ""))
        XCTAssertTrue(p.controls.radioKey(week, "ArrowLeft", held: ""))
        XCTAssertTrue(p.controls.radioKey(week, "ArrowDown", held: ""))
        XCTAssertEqual(pressed, [2, 4, 4])
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["appearance": "none", "written": ["appearance"]]],
                           ["op": "style", "id": 4, "style": ["visibility": "hidden"]]]))
        pressed = []
        XCTAssertTrue(p.controls.radioKey(week, "ArrowDown", held: ""))
        XCTAssertEqual(pressed, [2], "the hidden radio is passed over")
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["accessibilityChecked": "false"]], ["op": "props", "id": 4, "set": ["accessibilityChecked": "true"]]]))
        XCTAssertEqual([day, week, month].map(\.ariaRadioTabStop), [true, false, false], "a hidden checked radio is not the stop")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["appearance": "auto", "written": ["appearance"]]]]))
        XCTAssertNil(day.ariaRadioGroup, "a projected group's keys are the segmented control's")
        XCTAssertNotNil(p.segments.control(of: 1))
    }

    /// Opacity only paints (CSS); `visibility: hidden` and `inert` take input away.
    func testSegmentsFollowTheirStateNotTheirOpacity() throws {
        let p = tablist([2: face("Day"), 3: face("Week"), 4: face("Month")])
        let owner = try XCTUnwrap(p.views[1])
        let segments = try XCTUnwrap(owner.subviews.compactMap { $0 as? UISegmentedControl }.first)
        owner.alpha = 0
        p.segments.sync()
        XCTAssertTrue(segments.isEnabled, "a transparent tablist still takes input")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertEqual(p.segments.activate(try XCTUnwrap(p.views[3])), true)
        XCTAssertEqual(pressed, [3])
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["visibility": "hidden"]]]))
        p.segments.sync()
        XCTAssertFalse(segments.isEnabled, "a hidden tablist takes no input")
        XCTAssertEqual(p.segments.activate(try XCTUnwrap(p.views[4])), false)
        XCTAssertEqual(pressed, [3], "nor a pick")
        p.apply(wireBatch([["op": "style", "id": 1, "style": [:]]]))
        owner.props["inert"] = "true"
        p.segments.sync()
        XCTAssertFalse(segments.isEnabled, "inert disables it too")
    }

    func testNativeTabsAreATabBarOrSegmentsFromTheirFaces() throws {
        // A symbol and a title on every tab: the tab bar.
        var p = tablist([2: face("Inbox", symbol: "tray"), 3: face("Sent", symbol: "paperplane"), 4: face("Trash", symbol: "trash")],
                        labels: [2: "Inbox, three unread"])
        let bar = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UITabBar }.first)
        XCTAssertEqual(bar.items?.map(\.title), ["Inbox", "Sent", "Trash"])
        XCTAssertEqual(bar.items?.map(\.accessibilityLabel), ["Inbox, three unread", "Sent", "Trash"], "a tab's label names its item")
        XCTAssertTrue(bar.selectedItem === bar.items?.first)
        // Titles alone: segments with those titles.
        p = tablist([2: face("Day"), 3: face("Week"), 4: face("Month")])
        var segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        XCTAssertEqual((0..<3).map { segments.titleForSegment(at: $0) }, ["Day", "Week", "Month"])
        // A face that becomes a symbol drops its title.
        p.buttonFace = { [unowned self] id in id == 2 ? self.face(nil, symbol: "sun.max", label: "Day") : self.face(id == 3 ? "Week" : "Month") }
        p.views[2]?.props["accessibilityLabel"] = "Day"
        p.segments.sync()
        XCTAssertNotNil(segments.imageForSegment(at: 0))
        XCTAssertEqual(segments.titleForSegment(at: 0) ?? "", "", "an image segment shows no title")
        // Symbols alone, named by their labels: image segments.
        p = tablist([2: face(nil, symbol: "list.bullet", label: "List"), 3: face(nil, symbol: "square.grid.2x2", label: "Grid"), 4: face(nil, symbol: "map", label: "Map")],
                    labels: [2: "List", 3: "Grid", 4: "Map"])
        segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        XCTAssertNotNil(segments.imageForSegment(at: 0))
        XCTAssertEqual(segments.imageForSegment(at: 1)?.accessibilityLabel, "Grid")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        // A pick presses its tab, as the agent's tap of the authored tab does.
        XCTAssertEqual(p.segments.activate(try XCTUnwrap(p.views[4])), true)
        XCTAssertEqual(pressed, [4])
    }

    /// What UIKit does for a control's change, without the application a test bundle has none of.
    private func report(_ control: UIControl) {
        for target in control.allTargets {
            for name in control.actions(forTarget: target, forControlEvent: .valueChanged) ?? [] { _ = (target as? NSObject)?.perform(Selector(name), with: control) }
        }
    }

    /// UIKit reports a segment's tap after the finger lifts, as the selection
    /// settles (iOS 26); a sync in between keeps the finger's choice.
    func testASegmentKeepsAFingersChoiceUntilItIsReported() throws {
        let p = tablist([2: face("Day"), 3: face("Week"), 4: face("Month")])
        let segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        XCTAssertEqual(segments.selectedSegmentIndex, 0)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        segments.selectedSegmentIndex = 2
        p.segments.sync()
        XCTAssertEqual(segments.selectedSegmentIndex, 2, "a sync before the report keeps the finger's choice")
        report(segments)
        XCTAssertEqual(pressed, [4], "the report names the segment the finger chose")
        // The app kept Day selected: once reported, the next sync shows it.
        p.segments.sync()
        XCTAssertEqual(segments.selectedSegmentIndex, 0)
        // A tablist that gains a tab gets a new control, which shows the selection.
        p.buttonFace = { [unowned self] id in self.face(["Day", "Week", "Month", "Year"][Int(id) - 2]) }
        p.apply(wireBatch(native(5, ["accessibilityRole": "tab"], x: 300, w: 100) + [["op": "children", "id": 1, "ids": [2, 3, 4, 5]]]))
        p.segments.sync()
        let rebuilt = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        XCTAssertEqual(rebuilt.numberOfSegments, 4)
        XCTAssertEqual(rebuilt.selectedSegmentIndex, 0, "a rebuilt control shows the selection")
    }

    /// A radio group of three native radios, the one `checked` checked.
    private func radiogroup(_ faces: [UInt32: ButtonFace], checked: UInt32 = 3) -> Presenter {
        var ops = view(1, ["accessibilityRole": "radiogroup", "accessibilityLabel": "Tip"])
        for (i, id) in [2, 3, 4].enumerated() {
            ops += native(id, ["accessibilityRole": "radio", "accessibilityChecked": id == checked ? "true" : "false", "testId": "tip-\(id)"],
                          x: Double(i) * 100, w: 100)
        }
        return presenter(ops + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]], faces: faces)
    }

    /// LLP 1116.004 D5.2: a value chosen from a few is a radio group, and a
    /// radio group of radios is UIKit's segmented control, as a tablist of
    /// tabs is: its selection the checked radio, a pick pressing that radio,
    /// the next sync showing what the app checked. A value is never a tab:
    /// symbol-over-label radios are no tab bar.
    func testARadioGroupIsASegmentedControlWhosePickPressesItsRadio() throws {
        var p = radiogroup([2: face("15%"), 3: face("18%"), 4: face("20%")])
        defer { window.isHidden = true }
        let segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        XCTAssertEqual((0..<3).map { segments.titleForSegment(at: $0) }, ["15%", "18%", "20%"])
        XCTAssertEqual(segments.selectedSegmentIndex, 1, "the checked radio is the selected segment")
        XCTAssertEqual(segments.accessibilityLabel, "Tip", "the group's label names the control")
        XCTAssertEqual(p.views[2]?.isHidden, true, "the authored radios are the control's")
        XCTAssertEqual(p.segments.observation(try XCTUnwrap(p.views[4]))?["view"] as? String, "UISegmentedControl")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        segments.selectedSegmentIndex = 2
        report(segments)
        XCTAssertEqual(pressed, [4], "a pick presses its radio")
        p.segments.sync()
        XCTAssertEqual(segments.selectedSegmentIndex, 1, "unchecked by the app, the pick does not stay")
        p.views[3]?.props["accessibilityChecked"] = "false"
        p.views[4]?.props["accessibilityChecked"] = "true"
        p.segments.sync()
        XCTAssertEqual(segments.selectedSegmentIndex, 2, "the radio the app checked")
        // The agent's tap of an authored radio presses it through the control.
        XCTAssertEqual(p.segments.activate(try XCTUnwrap(p.views[2])), true)
        XCTAssertEqual(pressed, [4, 2])
        // Symbols over labels: no tab bar and no segments; the radios stay as authored, the checked one selected.
        p = radiogroup([2: face("Low", symbol: "drop"), 3: face("Mid", symbol: "drop.halffull"), 4: face("High", symbol: "drop.fill")])
        let group = try XCTUnwrap(p.views[1])
        XCTAssertTrue(group.subviews.compactMap { $0 as? UITabBar }.isEmpty, "a value is never a tab bar")
        XCTAssertTrue(group.subviews.compactMap { $0 as? UISegmentedControl }.isEmpty)
        XCTAssertEqual(p.views[3]?.isHidden, false)
        XCTAssertFalse(group.accessibilityTraits.contains(.tabBar))
        let checked = try XCTUnwrap(p.controls.controls[3] as? UIButton), other = try XCTUnwrap(p.controls.controls[2] as? UIButton)
        XCTAssertTrue(checked.accessibilityTraits.contains(.selected), "a checked radio is selected, as Safari's VoiceOver reads one")
        XCTAssertFalse(other.accessibilityTraits.contains(.selected))
    }

    /// The same from Contract (scripts/fixtures/native-buttons.contract, its
    /// `period`): a `row role="radiogroup"` of default `button role="radio"`s
    /// compiles to native radios, which UIKit draws as one segmented control;
    /// a pick runs that radio's `press`, and the runner's `aria-checked` keeps it.
    func testAContractRadioGroupIsOneSegmentedControlAndAPickRunsItsPress() throws {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_BUTTONS_PLAN"])
        let session = ExactApp.shared.makeSession(label: "radio-group")
        defer { session.destroy() }
        let controller = UIViewController()
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: .zero)
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 850)
        window.rootViewController = controller
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        session.presenter.viewport.frame = window.bounds
        controller.view.addSubview(session.presenter.viewport)
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: window.bounds.size).error)
        window.layoutIfNeeded()
        let p = session.presenter
        let node = { (id: String) in p.views.values.first { $0.props["testId"] == id } }
        let group = try XCTUnwrap(node("period"))
        let segments = try XCTUnwrap(group.subviews.compactMap { $0 as? UISegmentedControl }.first)
        XCTAssertEqual((0..<segments.numberOfSegments).map { segments.titleForSegment(at: $0) }, ["Day", "Week", "Month"])
        XCTAssertEqual(segments.selectedSegmentIndex, 1, "the checked radio, Week")
        XCTAssertEqual(segments.accessibilityLabel, "Period")
        XCTAssertEqual(node("period-Day")?.isHidden, true)
        segments.selectedSegmentIndex = 2
        report(segments)
        XCTAssertEqual(node("period-Month")?.props["accessibilityChecked"], "true", "the pick ran Month's press")
        XCTAssertEqual(node("period-Week")?.props["accessibilityChecked"], "false")
        p.segments.sync()
        XCTAssertEqual(segments.selectedSegmentIndex, 2)
    }

    /// LLP 1116.003 (r5 weather: "ExactSegmentedControl covers its middle"):
    /// a real touch on a tab a segmented control draws aims at its segment,
    /// the control taking the touch, never at the hidden authored tab.
    func testARealTouchAimsAtTheSegmentItsControlDraws() throws {
        let session = ExactApp.shared.makeSession(label: "segment-aim")
        defer { session.destroy() }
        let p = session.presenter
        let faces: [UInt32: ButtonFace] = [2: face("Day"), 3: face("Week"), 4: face("Month")]
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        var ops = view(1, ["accessibilityRole": "tablist"])
        for (i, id) in [2, 3, 4].enumerated() { ops += native(id, ["accessibilityRole": "tab"], x: Double(i) * 100, w: 100) }
        p.apply(wireBatch(ops + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]]))
        let control = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        control.layoutIfNeeded()
        let reply = Agent(session: session).aim(["id": 3, "aim": ["press": true]])
        if (reply["error"] as? String)?.contains("foreground") == true { throw missing("foreground", "the test host is in the background: a real touch's aim refuses it") }
        let aim = try XCTUnwrap(reply["aim"] as? [String: Any], "\(reply)")
        let w = try XCTUnwrap(aim["window"] as? [Double])
        let local = control.convert(CGPoint(x: w[0], y: w[1]), from: nil)
        XCTAssertTrue(control.bounds.contains(local), "on the control: \(local)")
        XCTAssertEqual(Int(local.x / (control.bounds.width / 3)), 1, "on Week, the middle segment: \(local)")
        XCTAssertEqual(aim["hit"] as? Int, 1, "the touch lands in the tablist's node: \(aim)")
    }

    func testAMenuRowsSymbolIsItsItemsImageNativeOrCustom() throws {
        let p = presenter(
            view(1, ["popover": "auto", "id": "menu", "accessibilityRole": "menu"])
                + native(2, ["accessibilityRole": "menuitem"])
                + [["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": [:], "style": ["text_color": [0, 0, 0, 255]]],
                   ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 200.0, "h": 40.0]]
                + native(4, ["accessibilityLabel": "Share"])
                + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]],
            faces: [2: face("Send", symbol: "paperplane"), 3: face(nil, symbol: "trash", label: "Delete"), 4: face(nil, symbol: "square.and.arrow.up", label: "Share")])
        let items = p.menus.items(of: try XCTUnwrap(p.views[1])).compactMap { $0 as? UIAction }
        XCTAssertEqual(items.count, 3)
        XCTAssertEqual(items[0].title, "Send")
        XCTAssertNotNil(items[0].image, "a native row's symbol")
        XCTAssertNotNil(items[1].image, "a custom row's symbol, from its face")
        XCTAssertEqual(items[1].title, "Delete", "a symbol-only custom row shows its label, as a native one does")
        XCTAssertEqual(items[2].title, "Share", "a symbol-only native row shows its label")
    }

    /// A confirmation whose action and cancel are native rows that only close
    /// it (astra's code review): its alert's actions, the press the action's.
    func testANativeConfirmationsRowsAreItsAlertsActions() throws {
        let p = Presenter()
        p.buttonFace = { [unowned self] id in self.face(id == 3 ? "Delete" : "Cancel") }
        // A presentation completes only in a scene's window; without one (a
        // hostless test bundle) the alert is checked as presented, not used.
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        defer { p.menus.reset(); window.isHidden = true }
        let controller = UIViewController()
        window.rootViewController = controller
        window.makeKeyAndVisible()
        p.viewport.frame = controller.view.bounds
        controller.view.addSubview(p.viewport)
        let closes = ["popovertarget": "confirm", "popovertargetaction": "hide"]
        p.apply(wireBatch(
            [["op": "create", "id": 1, "kind": "button", "handlers": [], "props": ["popovertarget": "confirm", "accessibilityLabel": "Remove"], "style": ["text_color": [0, 0, 0, 255]]],
             ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0]]
                + view(2, ["id": "confirm", "popover": "auto", "accessibilityRole": "alertdialog"], h: 120)
                + native(3, closes.merging(["destructive": "true"]) { $1 })
                + [["op": "create", "id": 4, "kind": "control", "handlers": [],
                    "props": ["type": "button", "accessibilityRole": "button"].merging(closes) { $1 },
                    "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                   ["op": "frame", "id": 4, "x": 0.0, "y": 40.0, "w": 80.0, "h": 40.0],
                   ["op": "children", "id": 2, "ids": [3, 4]], ["op": "roots", "ids": [1, 2]]]))
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true, "it opens")
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.actions.map(\.title), ["Delete", "Cancel"])
        XCTAssertNil(alert.title, "its popover has no label; the invoker's is not one")
        XCTAssertEqual(alert.actions.map(\.style), [.destructive, .cancel])
        guard scene != nil else { throw missing("scene", "a presentation completes only in a scene's window") }
        let settled = expectation(description: "presented")
        func poll() { if p.menus.inTransition { DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: poll) } else { settled.fulfill() } }
        poll()
        wait(for: [settled], timeout: 5)
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[3])), true, "its action")
        let done = expectation(description: "pressed")
        func pressedYet() { if pressed.isEmpty { DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: pressedYet) } else { done.fulfill() } }
        pressedYet()
        wait(for: [done], timeout: 5)
        XCTAssertEqual(pressed, [3], "the action's press, once")
    }

    /// A presenter in a scene's window (a presentation completes only
    /// there), its viewport under a controller that can present.
    private func presenting(_ faces: @escaping (UInt32) -> ButtonFace) -> (Presenter, UIViewController, Bool) {
        let p = Presenter()
        p.buttonFace = faces
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        let controller = UIViewController()
        window.rootViewController = controller
        window.makeKeyAndVisible()
        p.viewport.frame = controller.view.bounds
        controller.view.addSubview(p.viewport)
        return (p, controller, scene != nil)
    }
    private func settle(_ p: Presenter, until done: @escaping () -> Bool) {
        let settled = expectation(description: "settled")
        func poll() { if done() { settled.fulfill() } else { DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: poll) } }
        poll()
        wait(for: [settled], timeout: 5)
    }

    /// An "Open in…" chooser: an alertdialog popover (2) opened by 1 with a
    /// press row per provider (3–5; 4 remembered, `disabled` lists ids
    /// authored disabled) and a hide-only Cancel (6).
    private func chooser(_ names: @escaping () -> [UInt32: String], disabled: Set<Int> = []) -> (Presenter, UIViewController, Bool) {
        let (p, controller, scene) = presenting { [unowned self] id in self.face(names()[id]) }
        let closes = ["popovertarget": "open-in", "popovertargetaction": "hide"]
        func row(_ id: Int, _ extra: [String: String] = [:]) -> [[String: Any]] {
            native(id, closes.merging(extra) { $1 }.merging(disabled.contains(id) ? ["disabled": "true"] : [:]) { $1 })
        }
        p.apply(wireBatch(
            [["op": "create", "id": 1, "kind": "button", "handlers": [], "props": ["popovertarget": "open-in", "accessibilityLabel": "Open in Maps"], "style": ["text_color": [0, 0, 0, 255]]],
             ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0]]
                + view(2, ["id": "open-in", "popover": "auto", "accessibilityRole": "alertdialog", "accessibilityLabel": "Open location in"], h: 200)
                + row(3) + row(4, ["accessibilityChecked": "true"]) + row(5)
                + [["op": "create", "id": 6, "kind": "control", "handlers": [],
                    "props": ["type": "button", "accessibilityRole": "button"].merging(closes) { $1 },
                    "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                   ["op": "frame", "id": 6, "x": 0.0, "y": 160.0, "w": 80.0, "h": 40.0],
                   ["op": "children", "id": 2, "ids": [3, 4, 5, 6]], ["op": "roots", "ids": [1, 2]]]))
        return (p, controller, scene)
    }
    private static let providers: [UInt32: String] = [3: "Apple Maps", 4: "Google Maps", 5: "Waze", 6: "Cancel"]
    /// What UIKit runs when an action is chosen: its own handler.
    private func choose(_ action: UIAlertAction) throws {
        typealias Handler = @convention(block) (UIAlertAction) -> Void
        let block = try XCTUnwrap(action.value(forKey: "handler") as AnyObject?, "UIAlertAction's handler")
        unsafeBitCast(block, to: Handler.self)(action)
    }

    /// An alertdialog with an action per choice and one cancel is a sheet
    /// with all of them, titled by its `aria-label`; a disabled choice is
    /// dimmed, not a reason to open nothing.
    func testAChooserIsASheetWithAnActionPerChoice() throws {
        let (p, controller, _) = chooser({ Self.providers }, disabled: [5])
        defer { p.menus.reset(); window.isHidden = true }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true, "it opens")
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        XCTAssertEqual(alert.title, "Open location in", "its aria-label titles it")
        XCTAssertNil(alert.message, "no text rows, no message")
        XCTAssertEqual(alert.actions.map(\.title), ["Apple Maps", "Google Maps", "Waze", "Cancel"])
        XCTAssertEqual(alert.actions.map(\.style), [.default, .default, .default, .cancel])
        XCTAssertEqual(alert.actions.map(\.isEnabled), [true, true, false, true], "Waze authored disabled")
        XCTAssertEqual(p.menus.observation()?["actions"] as? Int, 3)
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[5])), false, "a disabled choice is not chosen")
    }

    /// Each sheet action's own UIKit handler presses its own row, once; the
    /// cancel's presses nothing. Presentation completes only in a scene.
    func testAChoosersEachActionPressesItsOwnRow() throws {
        for (index, expected) in [(0, [UInt32(3)]), (2, [5]), (3, [])] {
            let (p, controller, scene) = chooser({ Self.providers })
            defer { p.menus.reset(); window.isHidden = true }
            guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
            settle(p) { !p.menus.inTransition }
            let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
            try choose(alert.actions[index])
            settle(p) { p.menus.observation() == nil }
            // One more turn: a dispatch would have landed by now.
            settle(p) { true }
            XCTAssertEqual(pressed, expected, "action \(index)")
        }
    }

    /// A row that now says something else ends the sheet: it never
    /// dispatches under the title it was presented with.
    func testAChooserWhoseRowChangesItsTitleCloses() throws {
        var names = Self.providers
        let (p, controller, _) = chooser({ names })
        defer { p.menus.reset(); window.isHidden = true }
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), true)
        XCTAssertNotNil(controller.presentedViewController as? UIAlertController)
        names[3] = "Citymapper"
        p.menus.sync()
        XCTAssertNil(p.menus.observation(), "the sheet is gone")
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[3])), false)
        XCTAssertEqual(pressed, [])
    }

    /// `position-area: center` on the sheet (LLP 1021 "Placement"): anchored
    /// at the whole invoker, no arrow, allowed over it; UIKit centres the
    /// sheet across the invoker and picks its vertical position itself.
    /// Without a cancel, as a chooser with one is an unanchored sheet on a
    /// compact screen (LLP 1115 D6, ConfirmationAlertIOSTests).
    func testACentredSheetSitsOverItsInvoker() throws {
        let (p, controller, scene) = chooser({ Self.providers })
        defer { p.menus.reset(); window.isHidden = true }
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["position_area": "center"]],
                           ["op": "children", "id": 2, "ids": [3, 4, 5]],
                           ["op": "frame", "id": 1, "x": 100.0, "y": 150.0, "w": 200.0, "h": 40.0]]))
        let source = try XCTUnwrap(p.views[1])
        XCTAssertEqual(p.menus.activate(source), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        let presentation = try XCTUnwrap(alert.popoverPresentationController)
        XCTAssertEqual(presentation.sourceRect, source.bounds, "the whole invoker, not its label")
        XCTAssertEqual(presentation.permittedArrowDirections, [])
        XCTAssertTrue(presentation.canOverlapSourceViewRect)
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        settle(p) { !p.menus.inTransition }
        let sheet = alert.view.convert(alert.view.bounds, to: nil), invoker = source.convert(source.bounds, to: nil)
        print("position-area center: sheet \(sheet), invoker \(invoker)")
        XCTAssertEqual(sheet.midX, invoker.midX, accuracy: 2, "centred across it")
    }

    /// With no `position-area`, a sheet points at its whole invoker with
    /// UIKit's arrow on the side UIKit picks and never covers it, as a
    /// UIAlertController presented from a control does by default. Without a
    /// cancel, as `testACentredSheetSitsOverItsInvoker`.
    func testAnUnplacedSheetPointsAtItsInvokerWithoutCoveringIt() throws {
        let (p, controller, scene) = chooser({ Self.providers })
        defer { p.menus.reset(); window.isHidden = true }
        // Low in the window, so UIKit has room to put the sheet above it
        p.apply(wireBatch([["op": "children", "id": 2, "ids": [3, 4, 5]],
                           ["op": "frame", "id": 1, "x": 100.0, "y": 340.0, "w": 200.0, "h": 40.0]]))
        let source = try XCTUnwrap(p.views[1])
        XCTAssertEqual(p.menus.activate(source), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        let presentation = try XCTUnwrap(alert.popoverPresentationController)
        XCTAssertEqual(presentation.sourceRect, source.bounds, "the whole invoker")
        XCTAssertFalse(presentation.canOverlapSourceViewRect)
        guard scene else { throw missing("scene", "a presentation completes only in a scene's window") }
        settle(p) { !p.menus.inTransition }
        XCTAssertEqual(presentation.permittedArrowDirections, .any, "UIKit's arrow, on the side it picks")
        XCTAssertNotEqual(presentation.arrowDirection, .unknown, "an arrow points at the invoker")
        let window = try XCTUnwrap(source.window).bounds
        let sheet = alert.view.convert(alert.view.bounds, to: nil), invoker = source.convert(source.bounds, to: nil)
        XCTAssertTrue(window.contains(invoker) && !sheet.isEmpty, "both shown: sheet \(sheet), invoker \(invoker)")
        XCTAssertFalse(sheet.intersects(invoker.insetBy(dx: 1, dy: 1)), "sheet \(sheet) covers invoker \(invoker)")
    }

    /// A shape the sheet cannot present, two cancels, opens nothing. (Its
    /// logged reason needs a session; this presenter has none.)
    func testAChooserTheSheetCannotPresentIsRefused() throws {
        let (p, controller, _) = presenting { [unowned self] _ in self.face("Row") }
        defer { p.menus.reset(); window.isHidden = true }
        let closes = ["popovertarget": "c", "popovertargetaction": "hide"]
        func cancel(_ id: Int) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "control", "handlers": [],
              "props": ["type": "button", "accessibilityRole": "button"].merging(closes) { $1 },
              "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
             ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 80.0, "h": 40.0]]
        }
        p.apply(wireBatch(
            [["op": "create", "id": 1, "kind": "button", "handlers": [], "props": ["popovertarget": "c"], "style": ["text_color": [0, 0, 0, 255]]],
             ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0]]
                + view(2, ["id": "c", "popover": "auto", "accessibilityRole": "alertdialog"], h: 120)
                + native(3, closes) + cancel(4) + cancel(5)
                + [["op": "children", "id": 2, "ids": [3, 4, 5]], ["op": "roots", "ids": [1, 2]]]))
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[1])), false)
        XCTAssertNil(controller.presentedViewController)
    }

    /// HTML's `hr` (LLP 1021 D1) arrives as a row like any other and, having
    /// no press, breaks the menu into inline sections; the cancel after it,
    /// a hide-only row, is no item (the menu closes itself).
    func testAnHrRowIsASectionBreak() throws {
        let row = { (id: Int) -> [[String: Any]] in
            [["op": "create", "id": id, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "open-in", "popovertargetaction": "hide"], "style": [:]],
             ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0]]
        }
        let p = presenter(
            view(1, ["popover": "auto", "id": "open-in", "accessibilityRole": "menu"])
                + row(2) + row(3)
                + view(4, ["semanticTag": "hr"], style: ["border_width_top": 1, "border_width_bottom": 1], h: 2)
                + row(5)
                + [["op": "create", "id": 6, "kind": "button", "handlers": [], "props": ["popovertarget": "open-in", "popovertargetaction": "hide"], "style": [:]],
                   ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
                   ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6]], ["op": "roots", "ids": [1]]],
            faces: [2: face("Apple Maps"), 3: face("Google Maps"), 5: face("Waze"), 6: face("Cancel")])
        let pop = try XCTUnwrap(p.views[1])
        XCTAssertTrue(pop.container.subviews.contains { $0 === p.views[4] }, "the hr is a NodeView row of the popover")
        let sections = p.menus.items(of: pop).compactMap { $0 as? UIMenu }
        XCTAssertEqual(sections.map { $0.children.compactMap { ($0 as? UIAction)?.title } }, [["Apple Maps", "Google Maps"], ["Waze"]])
        XCTAssertTrue(sections.allSatisfy { $0.options.contains(.displayInline) })
    }

    /// A menu is titled by its popover's `aria-label`, as an inline section's
    /// heading; a row whose image is
    /// an `img` shows it as its item's image, a symbol's as itself and a
    /// bitmap fitted to the row's icon box.
    func testAMenuIsTitledByItsLabelAndShowsARowsImg() throws {
        let p = presenter(
            [["op": "create", "id": 1, "kind": "button", "handlers": [], "props": ["popovertarget": "open-in"], "style": [:]],
             ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0]]
                + view(2, ["popover": "auto", "id": "open-in", "accessibilityRole": "menu", "accessibilityLabel": "Open location in"])
                + [["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "open-in", "popovertargetaction": "hide"], "style": [:]],
                   ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 200.0, "h": 40.0],
                   ["op": "create", "id": 4, "kind": "image", "handlers": [], "props": [:], "style": [:]],
                   ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0],
                   ["op": "children", "id": 3, "ids": [4]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1, 2]]],
            faces: [3: face("Waze")])
        let invoker = try XCTUnwrap(p.views[1])
        let button = try XCTUnwrap(invoker.subviews.compactMap { $0 as? UIButton }.first { $0.showsMenuAsPrimaryAction })
        let section = try XCTUnwrap(button.menu?.children.first as? UIMenu)
        XCTAssertEqual(button.menu?.title, "", "no top-level title: its header is taller than a section's")
        XCTAssertEqual(section.title, "Open location in")
        XCTAssertTrue(section.options.contains(.displayInline), "an inline section's heading, as a hand-built pull-down has it")
        func row() throws -> UIAction { try XCTUnwrap(p.menus.items(of: try XCTUnwrap(p.views[2])).compactMap { $0 as? UIAction }.first) }
        XCTAssertEqual(try row().title, "Waze")
        XCTAssertNil(try row().image, "no image until it has loaded")
        let img = try XCTUnwrap(p.views[4])
        img.imageSource = "symbol:sf/car"
        img.image = UIImage(systemName: "car")
        XCTAssertTrue(try row().image === img.image, "a symbol img, as itself")
        let context = try XCTUnwrap(CGContext(data: nil, width: 64, height: 32, bitsPerComponent: 8, bytesPerRow: 256,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        XCTAssertEqual(MenuHost.rowImage(try XCTUnwrap(context.makeImage())).size, CGSize(width: 24, height: 12),
                       "a bitmap, fit in the row's icon box with its ratio kept")
    }
}
#endif

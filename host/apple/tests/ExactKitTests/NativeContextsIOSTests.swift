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

    func testANativeSwipeActionIsDrawnFromItsFace() throws {
        let p = presenter(
            view(1, ["swipeContent": "body", "swipeTrailing": "delete mute pin flag"], style: ["overflow_x": "scroll", "overflow_y": "hidden"])
                + view(2, w: 490, h: 80) + view(3, ["id": "body"], h: 80)
                + native(4, ["id": "delete", "destructive": "true"], style: ["accent_color": [0, 255, 0, 255]], x: 300, w: 50, h: 50)
                + native(5, ["id": "mute", "accessibilityLabel": "Hide alerts"], style: ["accent_color": [88, 86, 214, 255]], x: 360, w: 50, h: 50)
                + [["op": "create", "id": 6, "kind": "button", "handlers": ["press"], "props": ["id": "pin", "accessibilityLabel": "Pin"], "style": ["text_color": [0, 0, 0, 255]]],
                   ["op": "frame", "id": 6, "x": 410.0, "y": 0.0, "w": 40.0, "h": 50.0],
                   ["op": "create", "id": 9, "kind": "button", "handlers": ["press"], "props": ["id": "flag", "accessibilityLabel": "Flag"], "style": ["text_color": [0, 0, 0, 255]]],
                   ["op": "frame", "id": 9, "x": 450.0, "y": 0.0, "w": 40.0, "h": 50.0]]
                // Each custom action's text child: Pin's shown, Flag's authored hidden.
                + [["op": "create", "id": 7, "kind": "text", "props": [:], "handlers": [], "style": [:]],
                   ["op": "create", "id": 10, "kind": "text", "props": [:], "handlers": [], "style": ["display": "none"]],
                   ["op": "children", "id": 6, "ids": [7]], ["op": "children", "id": 9, "ids": [10]]]
                + [["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3, 4, 5, 6, 9]], ["op": "roots", "ids": [1]],
                   ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0], ["op": "content", "id": 1, "x": 0.0, "y": 0.0, "w": 490.0, "h": 80.0]],
            faces: [4: face("Delete", symbol: "trash"), 5: face(nil, symbol: "bell", label: "Hide alerts"), 6: face("Pin", symbol: "pin", label: "Pin"), 9: face("Flag", symbol: "flag", label: "Flag")])
        let owner = try XCTUnwrap(p.views[1])
        p.swipeActions.touch(owner)
        let table = try XCTUnwrap(owner.subviews.compactMap { $0 as? UITableView }.first)
        XCTAssertTrue(table.touchesShouldCancel(in: NativeButtonIOS(configuration: .bordered())), "a pan takes a native body button's touch")
        let configuration = try XCTUnwrap(table.delegate?.tableView?(table, trailingSwipeActionsConfigurationForRowAt: IndexPath(row: 0, section: 0)))
        XCTAssertEqual(configuration.actions.count, 4)
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
        XCTAssertEqual(mute.backgroundColor, TextEngine.color([88, 86, 214, 255]), "its accent")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        mute.handler(mute, UIView()) { _ in }
        XCTAssertEqual(pressed, [5])
    }

    private func tablist(_ faces: [UInt32: ButtonFace], labels: [Int: String] = [:]) -> Presenter {
        var ops = view(1, ["accessibilityRole": "tablist"])
        for (i, id) in [2, 3, 4].enumerated() {
            var props = ["accessibilityRole": "tab"]
            if i == 0 { props["accessibilitySelected"] = "true" }
            if let label = labels[id] { props["accessibilityLabel"] = label }
            ops += native(id, props, x: Double(i) * 100, w: 100)
        }
        return presenter(ops + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]], faces: faces)
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
        XCTAssertNil(alert.title, "a confirmation has no title row; its label is not one")
        XCTAssertEqual(alert.actions.map(\.style), [.destructive, .cancel])
        guard scene != nil else { return }
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
            try XCTSkipUnless(scene, "a presentation completes only in a scene's window")
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
    func testACentredSheetSitsOverItsInvoker() throws {
        let (p, controller, scene) = chooser({ Self.providers })
        defer { p.menus.reset(); window.isHidden = true }
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["position_area": "center"]],
                           ["op": "frame", "id": 1, "x": 100.0, "y": 150.0, "w": 200.0, "h": 40.0]]))
        let source = try XCTUnwrap(p.views[1])
        XCTAssertEqual(p.menus.activate(source), true)
        let alert = try XCTUnwrap(controller.presentedViewController as? UIAlertController)
        let presentation = try XCTUnwrap(alert.popoverPresentationController)
        XCTAssertEqual(presentation.sourceRect, source.bounds, "the whole invoker, not its label")
        XCTAssertEqual(presentation.permittedArrowDirections, [])
        XCTAssertTrue(presentation.canOverlapSourceViewRect)
        try XCTSkipUnless(scene, "a presentation completes only in a scene's window")
        settle(p) { !p.menus.inTransition }
        let sheet = alert.view.convert(alert.view.bounds, to: nil), invoker = source.convert(source.bounds, to: nil)
        print("position-area center: sheet \(sheet), invoker \(invoker)")
        XCTAssertEqual(sheet.midX, invoker.midX, accuracy: 2, "centred across it")
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

    /// A menu is titled by its popover's `aria-label`; a row whose image is
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
        XCTAssertEqual(button.menu?.title, "Open location in")
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

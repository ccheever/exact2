#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1069.011.000 on UIKit: a native button goes where a button goes, read
/// through its face (D1) — a swipe action drawn from its symbol and accent
/// (D6), a tab of a tab bar or a segment (D4), a menu row whose symbol is its
/// item's image, as a custom row's now is (D5).
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
            view(1, ["swipeContent": "body", "swipeTrailing": "delete mute"], style: ["overflow_x": "scroll", "overflow_y": "hidden"])
                + view(2, w: 450, h: 80) + view(3, ["id": "body"], h: 80)
                + native(4, ["id": "delete", "destructive": "true"], x: 300, w: 50, h: 50)
                + native(5, ["id": "mute", "accessibilityLabel": "Hide alerts"], style: ["accent_color": [88, 86, 214, 255]], x: 360, w: 50, h: 50)
                + [["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3, 4, 5]], ["op": "roots", "ids": [1]],
                   ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0], ["op": "content", "id": 1, "x": 0.0, "y": 0.0, "w": 450.0, "h": 80.0]],
            faces: [4: face("Delete", symbol: "trash"), 5: face(nil, symbol: "bell", label: "Hide alerts")])
        let owner = try XCTUnwrap(p.views[1])
        p.swipeActions.touch(owner)
        let table = try XCTUnwrap(owner.subviews.compactMap { $0 as? UITableView }.first)
        XCTAssertTrue(table.touchesShouldCancel(in: NativeButtonIOS(configuration: .bordered())), "a pan takes a native body button's touch")
        let configuration = try XCTUnwrap(table.delegate?.tableView?(table, trailingSwipeActionsConfigurationForRowAt: IndexPath(row: 0, section: 0)))
        XCTAssertEqual(configuration.actions.count, 2, "a title-only native action keeps the row native")
        let delete = configuration.actions[0], mute = configuration.actions[1]
        XCTAssertEqual(delete.style, .destructive)
        XCTAssertNotNil(delete.image, "its symbol")
        XCTAssertNotEqual(delete.backgroundColor, .systemBlue, "UIKit's red, not overwritten")
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
        var p = tablist([2: face("Inbox", symbol: "tray"), 3: face("Sent", symbol: "paperplane"), 4: face("Trash", symbol: "trash")])
        let bar = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UITabBar }.first)
        XCTAssertEqual(bar.items?.map(\.title), ["Inbox", "Sent", "Trash"])
        XCTAssertTrue(bar.selectedItem === bar.items?.first)
        // Titles alone: segments with those titles.
        p = tablist([2: face("Day"), 3: face("Week"), 4: face("Month")])
        var segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        XCTAssertEqual((0..<3).map { segments.titleForSegment(at: $0) }, ["Day", "Week", "Month"])
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

    func testAMenuRowsSymbolIsItsItemsImageNativeOrCustom() throws {
        let p = presenter(
            view(1, ["popover": "auto", "id": "menu", "accessibilityRole": "menu"])
                + native(2, ["accessibilityRole": "menuitem"])
                + [["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": [:], "style": ["text_color": [0, 0, 0, 255]]],
                   ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 200.0, "h": 40.0]]
                + native(4, ["accessibilityLabel": "Share"])
                + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]],
            faces: [2: face("Send", symbol: "paperplane"), 3: face(nil, symbol: "trash"), 4: face(nil, symbol: "square.and.arrow.up", label: "Share")])
        let items = p.menus.items(of: try XCTUnwrap(p.views[1])).compactMap { $0 as? UIAction }
        XCTAssertEqual(items.count, 3)
        XCTAssertEqual(items[0].title, "Send")
        XCTAssertNotNil(items[0].image, "a native row's symbol")
        XCTAssertNotNil(items[1].image, "a custom row's symbol, from its face")
        XCTAssertEqual(items[2].title, "Share", "a symbol-only native row shows its label")
    }
}
#endif

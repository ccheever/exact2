#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1021 "The chooser" on AppKit: an alertdialog popover is an NSMenu
/// below its invoker, an item per action, and a chosen item presses its own
/// row once, only while the menu still shows what is there.
final class ChooserMacTests: XCTestCase {
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows.removeAll() }

    private static let providers = ["Apple Maps", "Google Maps", "Waze"]
    private func text(_ id: Int, _ value: String) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "text", "props": ["text": value]],
         ["op": "frame", "id": id, "x": 0, "y": 0, "w": 180, "h": 20]]
    }
    private func row(_ id: Int, _ label: Int, press: Bool, props: [String: String] = [:]) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "button", "handlers": press ? ["press"] : [],
          "props": ["popovertarget": "c", "popovertargetaction": "hide"].merging(props) { $1 }],
         ["op": "frame", "id": id, "x": 0, "y": 0, "w": 200, "h": 40],
         ["op": "children", "id": id, "ids": [label]]]
    }
    /// An invoker (1) and its popover (2): three providers (3–5, labels
    /// 13–15) and a cancel (6, label 16), plus `message` text rows from 20.
    private func chooser(_ names: [String] = providers, label: String = "Open location in", message: [String] = [],
                         props: [Int: [String: String]] = [:], cancels: Int = 1) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "props": ["popovertarget": "c"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "c", "accessibilityRole": "alertdialog", "accessibilityLabel": label]],
            ["op": "create", "id": 9, "kind": "view"],
        ]
        var rows: [Int] = []
        for (i, line) in message.enumerated() { ops += text(20 + i, line); rows.append(20 + i) }
        for (i, name) in names.enumerated() {
            ops += text(13 + i, name) + row(3 + i, 13 + i, press: true, props: props[3 + i] ?? [:])
            rows.append(3 + i)
        }
        for c in 0..<cancels { ops += text(16 + c * 10, "Cancel") + row(6 + c * 10, 16 + c * 10, press: false); rows.append(6 + c * 10) }
        ops += [["op": "children", "id": 2, "ids": rows], ["op": "children", "id": 9, "ids": [1, 2]], ["op": "roots", "ids": [9]],
                ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 500, "h": 400],
                ["op": "frame", "id": 1, "x": 20, "y": 20, "w": 120, "h": 30],
                ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 220, "h": 200]]
        p.apply(wireBatch(ops))
        return p
    }
    private func owner(_ p: Presenter) throws -> (MenuHost.Confirmation, NSMenu) {
        let owner = try XCTUnwrap(p.menus.confirmation(of: try XCTUnwrap(p.views[2]), from: try XCTUnwrap(p.views[1])))
        return (owner, p.menus.menu(of: owner))
    }
    private func send(_ item: NSMenuItem) throws {
        NSApp.sendAction(try XCTUnwrap(item.action), to: item.target, from: item)
    }

    func testAChooserIsAMenuWithAnItemPerChoice() throws {
        let p = chooser(props: [4: ["accessibilityChecked": "true"], 5: ["disabled": "true"], 3: ["destructive": "true"]])
        defer { p.menus.reset() }
        let (_, menu) = try owner(p)
        XCTAssertTrue(menu.items[0].isSectionHeader, "its aria-label heads a chooser")
        XCTAssertEqual(menu.items.map(\.title), ["Open location in"] + Self.providers, "no Cancel item: Escape is the cancel")
        XCTAssertEqual(menu.items.dropFirst().map(\.state), [.off, .on, .off], "aria-checked")
        XCTAssertEqual(menu.items.dropFirst().map(\.isEnabled), [true, true, false], "Waze authored disabled")
        XCTAssertEqual(menu.items[1].attributedTitle?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor, .systemRed)
        p.press(1)
        let pop = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.menus.isOpen(pop))
        if !ExactEnv.agentMode {
            XCTAssertFalse(p.menus.owns(pop), "presented as a menu, not painted")
            XCTAssertEqual(p.menus.observation?["kind"] as? String, "confirmation")
            XCTAssertEqual(p.menus.observation?["actions"] as? Int, 3)
        }
    }

    func testEachItemPressesItsOwnRowOnce() throws {
        for (index, expected) in [(1, [UInt32(3)]), (2, [4]), (3, [])] {
            let p = chooser(props: [5: ["disabled": "true"]])
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            let (_, menu) = try owner(p)
            try send(menu.items[index])
            try send(menu.items[index])
            XCTAssertEqual(pressed, expected, "item \(index), once; a disabled one never")
        }
    }

    func testARowThatChangesItsTitleIsNeverChosenUnderTheOldOne() throws {
        let p = chooser()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let (owner, menu) = try owner(p)
        XCTAssertTrue(p.menus.valid(owner))
        p.apply(wireBatch([["op": "props", "id": 14, "set": ["text": "Citymapper"]]]))
        XCTAssertFalse(p.menus.valid(owner), "a row now says something else")
        try send(menu.items[2])
        XCTAssertEqual(pressed, [])
        // Open, the same change ends the menu.
        let q = chooser()
        q.press(1)
        XCTAssertTrue(q.menus.isOpen(try XCTUnwrap(q.views[2])))
        q.apply(wireBatch([["op": "props", "id": 13, "set": ["text": "Citymapper"]]]))
        XCTAssertFalse(q.menus.isOpen(try XCTUnwrap(q.views[2])) && !ExactEnv.agentMode)
    }

    func testResetEndsTheMenuAndNothingDispatches() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent keeps every popover painted (LLP 1021 D4)")
        let p = chooser()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.press(1)
        let owner = try XCTUnwrap(p.menus.presentedConfirmation)
        let menu = p.menus.menu(of: owner)
        p.menus.reset()
        XCTAssertNil(p.menus.presentedConfirmation)
        try send(menu.items[1])
        XCTAssertEqual(pressed, [])
    }

    func testAConfirmationShowsItsTextAndNoTitle() throws {
        let p = chooser(["Block"], label: "Block Contact", message: ["Blocked people can't call you."])
        let (owner, menu) = try owner(p)
        XCTAssertNil(owner.heading, "a confirmation keeps its text as its only heading")
        XCTAssertEqual(menu.items.map(\.title), ["Blocked people can't call you.", "", "Block"])
        XCTAssertFalse(menu.items[0].isEnabled)
        XCTAssertNotNil(menu.items[0].view, "wrapped to a menu's width")
        XCTAssertTrue(menu.items[1].isSeparatorItem)
    }

    func testAShapeTheMenuCannotPresentIsRefusedAndStaysPainted() throws {
        let p = chooser(cancels: 2)
        defer { p.menus.reset() }
        XCTAssertNil(p.menus.confirmation(of: try XCTUnwrap(p.views[2]), from: try XCTUnwrap(p.views[1])))
        p.press(1)
        XCTAssertTrue(p.menus.owns(try XCTUnwrap(p.views[2])), "the painted top layer, as before")
        let none = chooser(props: [3: ["disabled": "true"], 4: ["disabled": "true"], 5: ["disabled": "true"]])
        XCTAssertNil(none.menus.confirmation(of: try XCTUnwrap(none.views[2]), from: try XCTUnwrap(none.views[1])), "every action disabled")
    }

    /// The real popUp: AppKit's own tracking, an item chosen in it.
    func testThePoppedUpMenuDispatchesTheChosenItem() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent keeps every popover painted (LLP 1021 D4)")
        let p = chooser()
        let window = try XCTUnwrap(windows.last)
        window.orderFrontRegardless()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        var seen: NSMenu?
        let token = NotificationCenter.default.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu, menu.items.contains(where: { $0.title == "Waze" }) else { return }
            seen = menu
            // Choose Google Maps from inside AppKit's tracking loop.
            RunLoop.current.perform(inModes: [.eventTracking, .default]) {
                menu.performActionForItem(at: 2)
                menu.cancelTracking()
            }
        }
        defer { NotificationCenter.default.removeObserver(token) }
        p.press(1)
        let done = Date(timeIntervalSinceNow: 5)
        while (seen == nil || p.menus.isOpen(p.views[2]!)) && Date() < done { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        XCTAssertNotNil(seen, "the menu tracked")
        XCTAssertEqual(seen?.items.first?.title, "Open location in")
        XCTAssertEqual(pressed, [4], "Google Maps, once")
        XCTAssertFalse(p.menus.isOpen(p.views[2]!))
    }

    func testAMenuIsTitledByItsLabelAndFitsARowsBitmap() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "button", "props": ["popovertarget": "m"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "m", "accessibilityRole": "menu", "accessibilityLabel": "Open location in"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "m", "popovertargetaction": "hide"]],
            ["op": "create", "id": 4, "kind": "image"],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Waze"]],
            ["op": "create", "id": 6, "kind": "button", "props": ["popovertarget": "m", "popovertargetaction": "hide"]],
            ["op": "create", "id": 7, "kind": "text", "props": ["text": "Cancel"]],
            ["op": "children", "id": 3, "ids": [4, 5]], ["op": "children", "id": 6, "ids": [7]],
            ["op": "children", "id": 2, "ids": [3, 6]], ["op": "roots", "ids": [1, 2]],
        ]))
        let pop = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.menus.isMenuShaped(pop), "a row of an img and text is a menu row")
        let menu = p.menus.menu(of: pop)
        XCTAssertTrue(menu.items[0].isSectionHeader)
        XCTAssertEqual(menu.items.map(\.title), ["Open location in", "Waze", "Cancel"])
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 64, height: 32, bitsPerComponent: 8, bytesPerRow: 0,
                                          space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        let image = MenuHost.rowImage(try XCTUnwrap(ctx.makeImage()))
        XCTAssertEqual(image.size, NSSize(width: 16, height: 8), "fitted to a menu image, its ratio kept")
    }
}
#endif

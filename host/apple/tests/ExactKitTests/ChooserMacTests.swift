#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1021 "The chooser" on AppKit: an alertdialog popover is an NSMenu
/// against its invoker, an item per action, and a chosen item presses its
/// own row once, on the next turn, only while the menu still shows what is there.
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
    /// A chosen item presses on the next main-queue turn: let every turn
    /// queued so far run (the main queue is FIFO).
    private func turn() {
        let turned = expectation(description: "the next turn")
        DispatchQueue.main.async { turned.fulfill() }
        wait(for: [turned], timeout: 5)
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
            XCTAssertEqual(pressed, [], "not inside the menu's action")
            turn()
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
        turn()
        XCTAssertEqual(pressed, [])
    }

    func testAnOpenMenuWhoseRowChangesItsTitleEnds() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent keeps every popover painted (LLP 1021 D4)")
        let q = chooser()
        q.press(1)
        XCTAssertTrue(q.menus.isOpen(try XCTUnwrap(q.views[2])))
        q.apply(wireBatch([["op": "props", "id": 13, "set": ["text": "Citymapper"]]]))
        XCTAssertFalse(q.menus.isOpen(try XCTUnwrap(q.views[2])), "a row now says something else")
    }

    /// A batch between the choice and its turn that hides the chosen row —
    /// its own `display: none`, inert, or a hidden ancestor of the popover —
    /// leaves nothing to press. The popover itself is hidden in place while
    /// its menu presents it, which does not count.
    func testAChoiceHiddenBeforeItsTurnIsNeverPressed() throws {
        let hides: [[[String: Any]]] = [
            [["op": "style", "id": 4, "style": ["display": "none"]]],
            [["op": "props", "id": 4, "set": ["inert": "true"]]],
            [["op": "style", "id": 10, "style": ["display": "none"]]],
        ]
        for hide in hides {
            let p = chooser()
            // The popover authored in its own wrapper (10), beside its invoker.
            p.apply(wireBatch([["op": "create", "id": 10, "kind": "view"], ["op": "children", "id": 10, "ids": [2]],
                               ["op": "frame", "id": 10, "x": 0, "y": 0, "w": 220, "h": 200],
                               ["op": "children", "id": 9, "ids": [1, 10]]]))
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            let (owner, menu) = try owner(p)
            XCTAssertTrue(p.menus.valid(owner), "a hidden popover's rows are choosable")
            XCTAssertTrue(menu.items[2].isEnabled)
            try send(menu.items[2])
            p.apply(wireBatch(hide))
            XCTAssertFalse(p.menus.valid(owner), "\(hide)")
            turn()
            XCTAssertEqual(pressed, [], "\(hide)")
        }
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
        turn()
        XCTAssertEqual(pressed, [])
        // Chosen, then reset before its turn: nothing either.
        let q = chooser()
        q.onPress = { pressed.append($0) }
        let (_, chosen) = try self.owner(q)
        try send(chosen.items[1])
        q.menus.reset()
        turn()
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

    /// The real popUp: AppKit's own tracking of the menu in its invoker, an
    /// item chosen inside it (`performActionForItem`, AppKit's own send of
    /// the item's action; synthesized clicks need accessibility trust), and
    /// the press doing what Messages' Discard Changes does: its batch
    /// unmounts the invoker the menu was popped up in. The press lands once,
    /// after the tracking has ended, and nothing touches the dead invoker.
    func testAChoiceThatUnmountsTheInvokerPressesOnceAfterTheMenuEnds() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent keeps every popover painted (LLP 1021 D4)")
        let p = chooser()
        let window = try XCTUnwrap(windows.last)
        window.orderFrontRegardless()
        var pressed: [UInt32] = []
        var tracking = false, trackingAtPress: Bool?
        weak var invoker = p.views[1]
        p.onPress = { id in
            pressed.append(id)
            trackingAtPress = tracking
            // The session's apply of the app's batch: navigate back, the
            // whole screen — invoker, popover, rows — gone.
            p.apply(wireBatch([["op": "children", "id": 9, "ids": []]]
                              + [1, 2, 3, 4, 5, 6, 13, 14, 15, 16].map { ["op": "destroy", "id": $0] }))
        }
        var seen: NSMenu?
        let center = NotificationCenter.default
        let begin = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu, menu.items.contains(where: { $0.title == "Waze" }) else { return }
            seen = menu
            tracking = true
            // Choose Google Maps from inside AppKit's tracking loop.
            RunLoop.current.perform(inModes: [.eventTracking, .default]) {
                menu.performActionForItem(at: 2)
                XCTAssertEqual(pressed, [], "nothing presses inside the tracking")
                menu.cancelTracking()
            }
        }
        let end = center.addObserver(forName: NSMenu.didEndTrackingNotification, object: nil, queue: nil) { note in
            if note.object as? NSMenu === seen { tracking = false }
        }
        defer { center.removeObserver(begin); center.removeObserver(end) }
        p.press(1)
        let done = Date(timeIntervalSinceNow: 5)
        while pressed.isEmpty && Date() < done { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        turn()
        XCTAssertNotNil(seen, "the menu tracked")
        XCTAssertEqual(seen?.items.first?.title, "Open location in")
        XCTAssertEqual(pressed, [4], "Google Maps, once")
        XCTAssertEqual(trackingAtPress, false, "pressed after the menu's tracking ended")
        XCTAssertNil(p.views[1], "the invoker is unmounted")
        XCTAssertNil(invoker?.superview)
        XCTAssertNil(p.menus.presentedConfirmation)
    }

    /// A batch during the tracking that hides the row the user then picks
    /// (`display: none`, its title and handler unchanged) ends the menu, and
    /// the pick presses nothing.
    func testARowHiddenDuringTheTrackingIsNeverPressed() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent keeps every popover painted (LLP 1021 D4)")
        let p = chooser()
        try XCTUnwrap(windows.last).orderFrontRegardless()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        var seen: NSMenu?, ended = false
        let center = NotificationCenter.default
        let begin = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu, menu.items.contains(where: { $0.title == "Waze" }) else { return }
            seen = menu
            RunLoop.current.perform(inModes: [.eventTracking, .default]) {
                p.apply(wireBatch([["op": "style", "id": 4, "style": ["display": "none"]]]))
                menu.performActionForItem(at: 2)
                menu.cancelTracking()
            }
        }
        let end = center.addObserver(forName: NSMenu.didEndTrackingNotification, object: nil, queue: nil) { note in
            if note.object as? NSMenu === seen { ended = true }
        }
        defer { center.removeObserver(begin); center.removeObserver(end) }
        p.press(1)
        let done = Date(timeIntervalSinceNow: 5)
        while !ended && Date() < done { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        turn()
        XCTAssertNotNil(seen, "the menu tracked")
        XCTAssertEqual(pressed, [], "a hidden row is not chosen")
        XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[2])))
    }

    /// The menu pops up where the popover's box would sit (§5): its
    /// `position-area` against the invoker, its margins around the menu.
    func testTheMenuPopsUpByItsPositionArea() throws {
        let p = chooser()
        // Mid-window, so no side clamps a menu of a few rows.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 20, "y": 200, "w": 120, "h": 30]]))
        let (pop, source) = (try XCTUnwrap(p.views[2]), try XCTUnwrap(p.views[1]))
        let (_, menu) = try owner(p)
        let size = menu.size
        XCTAssertGreaterThan(size.height, 0)
        XCTAssertEqual(p.menus.popUpPoint(menu, pop, in: source), NSPoint(x: 0, y: 30), "none: below, at its left edge")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["position_area": "top", "margin_bottom": 12.0]]]))
        let top = p.menus.popUpPoint(menu, pop, in: source)
        XCTAssertEqual(top.y, -size.height - 12, "above, a 12-point gap")
        XCTAssertEqual(top.x, max(-20, 60 - size.width / 2), "centred on the invoker, clamped to the viewport")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["position_area": "center", "margin_bottom": 0.0]]]))
        XCTAssertEqual(p.menus.popUpPoint(menu, pop, in: source).y, 15 - size.height / 2, "centred over it")
    }

    /// The real popUp at a `top` area: the menu's window sits above the
    /// invoker rather than below it.
    func testATopAreaMenuOpensAboveItsInvoker() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent keeps every popover painted (LLP 1021 D4)")
        let p = chooser()
        let window = try XCTUnwrap(windows.last)
        window.orderFrontRegardless()
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 20, "y": 340, "w": 120, "h": 30],
                           ["op": "style", "id": 2, "style": ["position_area": "top"]]]))
        let source = try XCTUnwrap(p.views[1])
        let invoker = window.convertToScreen(source.convert(source.bounds, to: nil))
        var frame: NSRect?, seen = false
        let token = NotificationCenter.default.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu, menu.items.contains(where: { $0.title == "Waze" }) else { return }
            seen = true
            RunLoop.current.perform(inModes: [.eventTracking, .default]) {
                frame = NSApp.windows.filter { $0.isVisible && $0.level.rawValue >= NSWindow.Level.popUpMenu.rawValue }
                    .map(\.frame).max { $0.height < $1.height }
                menu.cancelTracking()
            }
        }
        defer { NotificationCenter.default.removeObserver(token) }
        p.press(1)
        let done = Date(timeIntervalSinceNow: 5)
        while (!seen || p.menus.isOpen(p.views[2]!)) && Date() < done { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        let menu = try XCTUnwrap(frame, "the menu's window")
        XCTAssertGreaterThanOrEqual(menu.minY, invoker.maxY - 8, "above the invoker (screen y grows up): \(menu) vs \(invoker)")
        XCTAssertEqual(menu.midX, invoker.midX, accuracy: 12, "centred on it")
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

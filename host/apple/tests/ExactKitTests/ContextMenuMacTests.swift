#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1021 §5.1 on AppKit: a context menu is its popover's menu rows as an
/// NSMenu (a Mac's has no preview, so the `contextPreview` row is no item),
/// and a picked item presses its row on the next turn, as a button menu's.
final class ContextMenuMacTests: XCTestCase {
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows.removeAll() }

    /// A source (1) naming popover 2: a preview row (3) and two items (4, 5)
    /// with an `hr` (6) between them.
    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        func text(_ id: Int, _ value: String) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "text", "props": ["text": value]], ["op": "frame", "id": id, "x": 0, "y": 0, "w": 180, "h": 20]]
        }
        func row(_ id: Int, _ label: Int, _ props: [String: String] = [:]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "m", "popovertargetaction": "hide"].merging(props) { $1 }],
             ["op": "frame", "id": id, "x": 0, "y": 0, "w": 200, "h": 40], ["op": "children", "id": id, "ids": [label]]]
        }
        let ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press", "contextmenu"], "props": ["contextPopover": "m"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "m", "accessibilityRole": "menu"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 9, "kind": "view"],
        ] + text(13, "Open the chat") + row(3, 13, ["contextPreview": "true"]) + text(14, "Pin") + row(4, 14)
            + text(15, "Delete") + row(5, 15, ["destructive": "true"])
            + [["op": "children", "id": 2, "ids": [3, 4, 6, 5]], ["op": "children", "id": 9, "ids": [1, 2]], ["op": "roots", "ids": [9]],
               ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 500, "h": 400], ["op": "frame", "id": 1, "x": 20, "y": 20, "w": 120, "h": 30],
               ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 220, "h": 200]]
        p.apply(wireBatch(ops))
        return p
    }

    func testTheContextMenuIsItsPopoversItemsWithoutThePreview() throws {
        let p = presenter()
        let pop = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.menus.isMenuShaped(pop), "a preview row does not keep it from being a menu")
        let menu = p.menus.menu(of: pop)
        XCTAssertEqual(menu.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Pin", "—", "Delete"])
        var pressed: [UInt32] = []
        let picked = expectation(description: "the picked row is pressed")
        p.onPress = { pressed.append($0); picked.fulfill() }
        NSApp.sendAction(try XCTUnwrap(menu.items[2].action), to: menu.items[2].target, from: menu.items[2])
        XCTAssertEqual(pressed, [], "on the next turn")
        // That turn, however late a loaded machine runs it: a fixed 50 ms
        // spin of the run loop could return before the main queue's turn.
        wait(for: [picked], timeout: 10)
        XCTAssertEqual(pressed, [5])
    }

    private func turn() {
        let turned = expectation(description: "the next turn")
        DispatchQueue.main.async { turned.fulfill() }
        wait(for: [turned], timeout: 10)
    }

    private func contextKey(_ window: NSWindow, type: NSEvent.EventType = .keyDown,
                            code: UInt16 = 0x6E, flags: NSEvent.ModifierFlags = []) -> NSEvent {
        NSEvent.keyEvent(with: type, location: NSPoint(x: 450, y: 350), modifierFlags: flags, timestamp: 0,
                         windowNumber: window.windowNumber, context: nil, characters: "\u{F735}",
                         charactersIgnoringModifiers: "\u{F735}", isARepeat: false, keyCode: code)!
    }

    /// #235: a real menu key runs the action first, then tracks the updated
    /// native menu. Its point is the focus's centre, not the mouse position.
    func testContextMenuKeyRunsTheActionThenTracksTheNativeMenu() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent presents painted popovers")
        let p = presenter(), window = try XCTUnwrap(windows.last)
        let source = try XCTUnwrap(p.views[1])
        window.orderFrontRegardless()
        XCTAssertTrue(window.makeFirstResponder(source))
        source.handlers.insert("key")
        var heard: [String] = [], sample: [String] = []
        p.onKey = { _, key in heard.append("key \(key.chord)") }
        p.onClipboard = { id, kind, line in
            XCTAssertEqual(id, 1); XCTAssertEqual(kind, 10)
            heard.append("context")
            sample = line.components(separatedBy: ",")
            p.apply(wireBatch([["op": "props", "id": 14, "set": ["text": "Pin after context"]]]))
        }
        p.onPress = { heard.append("pick \($0)") }
        var tracked = false
        let center = NotificationCenter.default
        let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu, menu.items.first?.title == "Pin after context" else { return }
            tracked = true
            RunLoop.current.perform(inModes: [.eventTracking, .default]) {
                menu.performActionForItem(at: 0)
                menu.cancelTracking()
            }
        }
        defer { center.removeObserver(observer) }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        let deadline = Date(timeIntervalSinceNow: 5)
        while !heard.contains("pick 4") && Date() < deadline { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertTrue(tracked)
        XCTAssertEqual(heard, ["key ContextMenu", "context", "pick 4"])
        XCTAssertEqual(sample.prefix(4).compactMap(Double.init), [60, 15, 0, 0])
        XCTAssertEqual(sample.dropFirst(6).prefix(2).compactMap(Double.init), [80, 35])
    }

    func testContextMenuHonorsKeyCancellationAndOnlyTheUnmodifiedKeyDown() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        source.handlers.insert("key")
        XCTAssertTrue(window.makeFirstResponder(source))
        var heard = 0, prevent = true
        p.onKey = { _, _ in p.defaultPrevented = prevent }
        p.onClipboard = { _, _, _ in heard += 1; p.defaultPrevented = true }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, 0, "a key handler cancelled the default")
        prevent = false
        XCTAssertFalse(p.routeKey(contextKey(window, type: .keyUp), focused: true))
        XCTAssertFalse(p.routeKey(contextKey(window, code: 109, flags: .shift), focused: true), "Shift+F10")
        XCTAssertFalse(p.routeKey(contextKey(window, flags: .control), focused: true))
        XCTAssertFalse(p.routeKey(contextKey(window), focused: false), "another session has the focus")
        XCTAssertEqual(heard, 0)
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, 1)
        XCTAssertFalse(p.defaultPrevented, "the context action's flag cannot leak into another event")
    }

    func testContextMenuUsesTheFocusAfterKeyHandlersAndKeepsThatContextTarget() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        source.handlers.insert("key")
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "button", "handlers": ["contextmenu"]],
            ["op": "frame", "id": 7, "x": 180, "y": 20, "w": 100, "h": 30],
            ["op": "children", "id": 9, "ids": [1, 2, 7]],
        ]))
        let second = try XCTUnwrap(p.views[7])
        XCTAssertTrue(window.makeFirstResponder(source))
        p.onKey = { _, _ in window.makeFirstResponder(second) }
        var heard: [UInt32] = []
        p.onClipboard = { id, _, _ in heard.append(id); window.makeFirstResponder(source) }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, [7], "the key action moved focus; the context action does not retarget itself")
    }

    func testContextMenuCarriesDrawnCoordinatesToTheNearestHandler() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "button"],
            ["op": "frame", "id": 7, "x": 10, "y": 4, "w": 20, "h": 10],
            ["op": "children", "id": 1, "ids": [7]],
            ["op": "present", "id": 1, "property": "translate", "x": 100.0, "y": 0.0],
        ]))
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[7])))
        var heard: [UInt32] = [], sample: [String] = []
        p.onClipboard = { id, _, line in heard.append(id); sample = line.components(separatedBy: ",") }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, [1])
        XCTAssertEqual(sample.prefix(2).compactMap(Double.init), [20, 9])
        XCTAssertEqual(sample.dropFirst(6).prefix(2).compactMap(Double.init), [140, 29])
    }

    func testContextMenuKeepsFieldEditingAndCompositionWithTheirOwner() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last)
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "input", "props": ["value": ""]],
            ["op": "frame", "id": 7, "x": 0, "y": 0, "w": 100, "h": 30],
            ["op": "children", "id": 1, "ids": [7]],
        ]))
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[7]?.field)))
        var heard = 0
        p.onClipboard = { _, _, _ in heard += 1 }
        XCTAssertFalse(p.routeKey(contextKey(window), focused: true))
        let editor = try XCTUnwrap(window.firstResponder as? NSTextView)
        editor.setMarkedText("ㅎ", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertFalse(p.routeKey(contextKey(window), focused: true))
        XCTAssertTrue(editor.hasMarkedText())
        XCTAssertEqual(heard, 0, "an ancestor's context menu cannot take the editor's key")
    }

    func testContextMenuFindsAFrameTranslatedIntoView() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        p.apply(wireBatch([
            ["op": "frame", "id": 1, "x": -150, "y": 20, "w": 120, "h": 30],
            ["op": "present", "id": 1, "property": "translate", "x": 200.0, "y": 0.0],
        ]))
        XCTAssertTrue(window.makeFirstResponder(source))
        var sample: [String] = []
        p.onClipboard = { _, _, line in sample = line.components(separatedBy: ",") }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(sample.prefix(2).compactMap(Double.init), [60, 15])
        XCTAssertEqual(sample.dropFirst(6).prefix(2).compactMap(Double.init), [110, 35])
    }

    func testContextMenuFallsBackInsideItsSessionWhenTheKeyHandlerRemovesFocus() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.handlers.insert("key")
        p.views[9]?.handlers.insert("contextmenu")
        XCTAssertTrue(window.makeFirstResponder(source))
        p.onKey = { _, _ in
            p.apply(wireBatch([["op": "children", "id": 9, "ids": [2]], ["op": "destroy", "id": 1]]))
            window.makeFirstResponder(p.viewport)
        }
        var heard: [UInt32] = [], sample: [String] = []
        p.onClipboard = { id, _, line in heard.append(id); sample = line.components(separatedBy: ",") }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, [9])
        XCTAssertEqual(sample.prefix(2).compactMap(Double.init), [1, 1])
    }

    private final class NativeKeySink: NSView { override var acceptsFirstResponder: Bool { true } }
    func testContextMenuDoesNotTakeAnEmbeddedViewsKeys() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        let embedded = NativeKeySink(frame: source.bounds)
        source.addSubview(embedded)
        XCTAssertTrue(window.makeFirstResponder(embedded))
        var heard = 0
        p.onClipboard = { _, _, _ in heard += 1 }
        XCTAssertFalse(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, 0)
    }

    func testContextMenuDoesNotOpenForDisabledInertHiddenOrRemovedSources() throws {
        let changes: [(String, [String: Any])] = [
            ("disabled", ["op": "props", "id": 1, "set": ["disabled": "true"]]),
            ("inert", ["op": "props", "id": 1, "set": ["inert": "true"]]),
            ("hidden", ["op": "style", "id": 1, "style": ["display": "none"]]),
            ("removed", ["op": "destroy", "id": 1]),
        ]
        for (name, change) in changes {
            let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
            XCTAssertTrue(window.makeFirstResponder(source))
            var heard = 0
            p.onClipboard = { _, _, _ in heard += 1 }
            p.apply(wireBatch([change]))
            XCTAssertFalse(p.routeKey(contextKey(window), focused: true), name)
            XCTAssertEqual(heard, 0, name)
        }
    }
    private func choose(_ item: NSMenuItem) throws {
        NSApp.sendAction(try XCTUnwrap(item.action), to: item.target, from: item)
    }
    private func shape(_ menu: NSMenu) -> [String] {
        menu.items.map { item in
            if item.isSeparatorItem { return "—" }
            var label = item.title + (item.state == .on ? " ✓" : "")
            if let sub = item.submenu { label += " ▸ [" + shape(sub).joined(separator: " | ") + "]" }
            return label
        }
    }

    /// LLP 1021 §5, "Submenus" (#141): a source (1) naming popover `row-menu`
    /// (2): Pinned (4, checked), Copy (7, opening `copy-menu`), an `hr` (6),
    /// Delete (5). `copy-menu` (8): Copy path (10), Copy link (11), an `hr`
    /// (12), Absolute paths (13, checked). `inside` authors `copy-menu` as a
    /// child of `row-menu` rather than beside it.
    private func submenuPresenter(inside: Bool = false, copyProps: [String: String] = [:]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        func row(_ id: Int, _ label: String, hides: String = "row-menu", _ props: [String: String] = [:], press: Bool = true) -> [[String: Any]] {
            let target = props["popovertarget"] == nil ? ["popovertarget": hides, "popovertargetaction": "hide"] : [:]
            return [["op": "create", "id": 100 + id, "kind": "text", "props": ["text": label]],
                    ["op": "frame", "id": 100 + id, "x": 0, "y": 0, "w": 180, "h": 20],
                    ["op": "create", "id": id, "kind": "button", "handlers": press ? ["press"] : [], "props": target.merging(props) { $1 }],
                    ["op": "frame", "id": id, "x": 0, "y": 0, "w": 200, "h": 30], ["op": "children", "id": id, "ids": [100 + id]]]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": ["contextmenu"], "props": ["contextPopover": "row-menu"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "row-menu", "accessibilityRole": "menu", "accessibilityLabel": "Row actions"]],
            ["op": "create", "id": 8, "kind": "view", "props": ["popover": "auto", "id": "copy-menu", "accessibilityRole": "menu", "accessibilityLabel": "Copy"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 12, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 9, "kind": "view"],
        ]
        ops += row(4, "Pinned", ["accessibilityChecked": "true", "accessibilityRole": "menuitemcheckbox"])
        ops += row(7, "Copy", ["popovertarget": "copy-menu", "accessibilityHasPopup": "menu"].merging(copyProps) { $1 }, press: false)
        ops += row(5, "Delete")
        ops += row(10, "Copy path") + row(11, "Copy link") + row(13, "Absolute paths", ["accessibilityChecked": "true"])
        ops += [["op": "children", "id": 2, "ids": inside ? [4, 7, 8, 6, 5] : [4, 7, 6, 5]],
                ["op": "children", "id": 8, "ids": [10, 11, 12, 13]],
                ["op": "children", "id": 9, "ids": inside ? [1, 2] : [1, 2, 8]], ["op": "roots", "ids": [9]],
                ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 500, "h": 400], ["op": "frame", "id": 1, "x": 20, "y": 20, "w": 120, "h": 30],
                ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 220, "h": 200], ["op": "frame", "id": 8, "x": 0, "y": 0, "w": 220, "h": 200]]
        p.apply(wireBatch(ops))
        return p
    }

    /// A row whose `popovertarget` names another menu is a submenu item,
    /// beside its menu or authored inside it; check marks keep working in
    /// it; and choosing a nested item presses its row on the next turn, once
    /// for the whole menu.
    func testARowThatOpensAMenuIsASubmenu() throws {
        for inside in [false, true] {
            let p = submenuPresenter(inside: inside)
            let pop = try XCTUnwrap(p.views[2])
            XCTAssertTrue(p.menus.isMenuShaped(pop), "inside \(inside): a nested popover is no row")
            let menu = p.menus.menu(of: pop)
            XCTAssertEqual(shape(menu), ["Row actions", "Pinned ✓", "Copy ▸ [Copy path | Copy link | — | Absolute paths ✓]", "—", "Delete"], "inside \(inside)")
            let copy = try XCTUnwrap(menu.items.first { $0.title == "Copy" })
            XCTAssertEqual(copy.action, #selector(NSMenu.submenuAction(_:)), "AppKit's own: it opens the submenu and picks nothing")
            XCTAssertTrue(copy.isEnabled)
            let sub = try XCTUnwrap(copy.submenu)
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            try choose(sub.items[0])
            try choose(menu.items[1])
            XCTAssertEqual(pressed, [], "on the next turn")
            turn()
            XCTAssertEqual(pressed, [10], "inside \(inside): Copy path, and only it")
        }
    }

    /// A batch before the nested item's turn that points its opener at
    /// another popover, disables or hides the opener, or presents the menu
    /// again, cancels the choice; untouched, it presses.
    func testANestedChoiceDiesWithItsOpener() throws {
        let changes: [(String, (Presenter) -> Void)] = [
            ("untouched", { _ in }),
            ("opener retargeted", { p in p.apply(wireBatch([["op": "props", "id": 7, "set": ["popovertarget": "row-menu"]]])) }),
            ("opener disabled", { p in p.apply(wireBatch([["op": "props", "id": 7, "set": ["disabled": "true"]]])) }),
            ("opener hidden", { p in p.apply(wireBatch([["op": "style", "id": 7, "style": ["display": "none"]]])) }),
        ]
        for (name, change) in changes {
            let p = submenuPresenter()
            let menu = p.menus.menu(of: try XCTUnwrap(p.views[2]))
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            let copy = try XCTUnwrap(menu.items.first { $0.title == "Copy" }?.submenu)
            try choose(copy.items[1])
            change(p)
            turn()
            XCTAssertEqual(pressed, name == "untouched" ? [11] : [], name)
        }
    }

    /// A row naming a popover already on its path (a cycle), or one that
    /// hides its target, is an item, not a submenu; a disabled opener is a
    /// disabled submenu item.
    func testACycleOrAHideIsNoSubmenu() throws {
        let cycle = submenuPresenter(copyProps: ["popovertarget": "row-menu"])
        let items = cycle.menus.menu(of: try XCTUnwrap(cycle.views[2])).items
        XCTAssertNil(items.first { $0.title == "Copy" }?.submenu, "a row naming its own menu")
        let hides = submenuPresenter(copyProps: ["popovertargetaction": "hide"])
        XCTAssertNil(hides.menus.menu(of: try XCTUnwrap(hides.views[2])).items.first { $0.title == "Copy" }?.submenu)
        let disabled = submenuPresenter(copyProps: ["disabled": "true"])
        let copy = try XCTUnwrap(disabled.menus.menu(of: try XCTUnwrap(disabled.views[2])).items.first { $0.title == "Copy" })
        XCTAssertNotNil(copy.submenu)
        XCTAssertFalse(copy.isEnabled)
    }

    /// An item's title is its row's accessible name: an `aria-hidden` child
    /// (the web's own `›` on a submenu row) is left out, as the menu draws
    /// its own arrow.
    func testAnAriaHiddenChildIsNotInTheTitle() throws {
        let p = submenuPresenter()
        p.apply(wireBatch([["op": "create", "id": 50, "kind": "text", "props": ["text": "›", "accessibilityElementsHidden": "true"]],
                           ["op": "frame", "id": 50, "x": 0, "y": 0, "w": 10, "h": 20], ["op": "children", "id": 7, "ids": [107, 50]]]))
        let menu = p.menus.menu(of: try XCTUnwrap(p.views[2]))
        XCTAssertNotNil(menu.items.first { $0.title == "Copy" }?.submenu)
    }
}
#endif

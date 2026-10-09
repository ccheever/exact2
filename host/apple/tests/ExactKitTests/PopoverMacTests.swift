#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PopoverMacTests: XCTestCase {
    func testPointerOnlyPopoverContentDoesNotPressThePageBehindIt() throws {
        let p = fixture(), pop = try XCTUnwrap(p.views[3]), window = try XCTUnwrap(p.viewport.window)
        let behind = try XCTUnwrap(p.views[6])
        p.views[1]?.handlers = ["press"]; behind.handlers = ["press"]
        p.press(2)
        // A plain child, neither a control nor selectable text, forwards
        // through the popover. The page's responder chain must not hear it.
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "view", "handlers": ["pointerdown", "pointerup"]],
            ["op": "children", "id": 3, "ids": [4, 5, 7]],
            ["op": "frame", "id": 7, "x": 120, "y": 80, "w": 100, "h": 40]]))
        let child = try XCTUnwrap(p.views[7])
        let at = child.convert(NSPoint(x: 20, y: 20), to: nil)
        var presses: [UInt32] = [], pointers: [UInt32] = []
        p.onPress = { presses.append($0) }; p.onPointer = { id, _, _ in pointers.append(id) }
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(behind)
        for type: NSEvent.EventType in [.leftMouseDown, .leftMouseUp] {
            let event = try XCTUnwrap(NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: 0,
                windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1))
            window.sendEvent(event)
        }
        XCTAssertEqual(pointers, [7, 7])
        XCTAssertEqual(presses, [], "no page ancestor or focused page control is activated")
        XCTAssertTrue(p.menus.isOpen(pop))
    }
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows.removeAll() }

    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "form"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["popover": "auto", "id": "form"]],
            ["op": "create", "id": 4, "kind": "input", "props": ["value": "draft", "autofocus": "true"]],
            ["op": "create", "id": 5, "kind": "button", "props": ["popovertarget": "form", "popovertargetaction": "hide"]],
            ["op": "create", "id": 6, "kind": "button"],
            ["op": "children", "id": 1, "ids": [2, 6, 3]],
            ["op": "children", "id": 3, "ids": [4, 5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 500, "h": 400],
            ["op": "frame", "id": 2, "x": 400, "y": 340, "w": 80, "h": 30],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 240, "h": 150],
            ["op": "frame", "id": 4, "x": 10, "y": 10, "w": 200, "h": 30],
            ["op": "frame", "id": 5, "x": 10, "y": 80, "w": 80, "h": 30],
            ["op": "frame", "id": 6, "x": 10, "y": 10, "w": 80, "h": 30],
        ]))
        window.makeFirstResponder(p.views[2])
        return p
    }
    private func key(_ window: NSWindow, up: Bool = false, repeatKey: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: up ? .keyUp : .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: repeatKey, keyCode: 53)!
    }
    private func pointer(_ p: Presenter, _ type: NSEvent.EventType, _ node: NodeView) {
        let point = node.convert(NSPoint(x: node.bounds.midX, y: node.bounds.midY), to: nil)
        let event = NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: 0,
            windowNumber: p.viewport.window!.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        p.menus.pointer(event)
    }

    func testFormKeepsItsFieldAndFocusAndDoesNotTrapTab() throws {
        let p = fixture(), pop = p.views[3]!, window = p.viewport.window!
        let field = try XCTUnwrap(p.views[4]?.field)
        XCTAssertTrue(pop.isHidden)
        XCTAssertFalse(p.menus.isMenuShaped(pop))
        XCTAssertTrue(p.views[2]!.pressable, "a target-only button is an invoker")
        p.press(2)
        XCTAssertFalse(pop.isHidden)
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(field.currentEditor() === window.firstResponder)
        XCTAssertTrue(p.views[4]?.field === field)
        XCTAssertFalse(p.views[6]!.inert, "the page stays interactive")
        XCTAssertTrue(p.views[2]!.nextKeyView === field, "focus enters after the invoker, not at the authored popover position")
        XCTAssertTrue(field.nextKeyView === p.views[5])
        XCTAssertTrue(p.views[5]!.nextKeyView === p.views[6], "Tab exits to the next page control")
        p.press(5)
        XCTAssertTrue(pop.isHidden)
        XCTAssertTrue(pop.superview === p.views[1])
        XCTAssertTrue(window.firstResponder === p.views[2])
        XCTAssertEqual(field.stringValue, "draft")
        p.press(2)
        XCTAssertTrue(p.views[4]?.field === field)
        XCTAssertTrue(field.currentEditor() === window.firstResponder)
        p.menus.reset()
    }
    func testToggleShowHideAndAppActionRefreshAreOrdered() {
        let p = fixture(), pop = p.views[3]!
        p.views[2]!.handlers = ["press"]
        var presses = 0
        p.onPress = { _ in
            presses += 1
            p.apply(wireBatch([["op": "props", "id": 4, "set": ["value": "fresh"], "clear": []]]))
        }
        p.press(2)
        XCTAssertEqual(presses, 1)
        XCTAssertEqual(p.views[4]!.field?.stringValue, "fresh")
        p.views[2]!.props["popovertargetaction"] = "show"
        p.press(2)
        XCTAssertTrue(p.menus.isOpen(pop))
        p.views[2]!.props["popovertargetaction"] = "toggle"
        p.press(2)
        XCTAssertFalse(p.menus.isOpen(pop))
        p.press(5)
        XCTAssertEqual(presses, 3, "a closed popover cannot activate its row")
        p.onPress = { _ in p.reset() }
        p.press(2)
        XCTAssertTrue(p.menus.presented.isEmpty, "a command captured before replacement must be dropped")
    }
    func testLayoutClampsAndRetainedBatchesKeepTheEditor() {
        let p = fixture(), pop = p.views[3]!, window = p.viewport.window!
        p.press(2)
        let editor = window.firstResponder
        XCTAssertEqual(pop.frame.origin, NSPoint(x: 260, y: 250))
        p.apply(wireBatch([
            ["op": "children", "id": 1, "ids": [2, 3, 6]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 260, "h": 160],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 80, "h": 30],
        ]))
        XCTAssertTrue(window.firstResponder === editor)
        XCTAssertEqual(pop.frame, NSRect(x: 10, y: 40, width: 260, height: 160))
        window.setContentSize(NSSize(width: 200, height: 120))
        p.menus.layout()
        XCTAssertEqual(pop.frame.origin, .zero)
        p.menus.close(pop)
        XCTAssertEqual(pop.frame, NSRect(x: 0, y: 0, width: 260, height: 160))
        XCTAssertEqual(p.views[1]!.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3, 6])
    }
    func testLightDismissRequiresMatchingDownAndUpAndDoesNotStealOutsideFocus() {
        let p = fixture(), pop = p.views[3]!, window = p.viewport.window!
        p.press(2)
        pointer(p, .leftMouseDown, p.views[4]!)
        pointer(p, .leftMouseUp, p.views[6]!)
        XCTAssertTrue(p.menus.isOpen(pop), "a drag out of the popover keeps it open")
        pointer(p, .leftMouseDown, p.views[2]!)
        pointer(p, .leftMouseUp, p.views[2]!)
        XCTAssertTrue(p.menus.isOpen(pop), "the invoker's default toggle owns its click")
        pointer(p, .leftMouseDown, p.views[6]!)
        window.makeFirstResponder(p.views[6])
        pointer(p, .leftMouseUp, p.views[6]!)
        XCTAssertFalse(p.menus.isOpen(pop))
        XCTAssertTrue(window.firstResponder === p.views[6])
    }
    func testRemovalInertAndResetRetireTheTopLayer() {
        let p = fixture(), pop = p.views[3]!
        p.press(2)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["inert": "true"], "clear": []]]))
        XCTAssertFalse(p.menus.isOpen(pop))
        XCTAssertTrue(p.views[4]!.inert)
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["inert"]]]))
        p.press(2)
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 6]]]))
        XCTAssertFalse(p.menus.isOpen(pop))
        XCTAssertNil(pop.superview)
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 6, 3]]]))
        p.press(2)
        p.reset()
        XCTAssertTrue(p.menus.presented.isEmpty)
        XCTAssertFalse(p.viewport.subviews.contains { String(describing: type(of: $0)) == "PopoverLayer" })
    }
    func testEscapeDoesNotCrossSessionsAndHeldEscapeDoesNotCloseTheDialog() {
        let p = fixture(), other = fixture(), window = p.viewport.window!
        p.views[1]!.props["semanticTag"] = "dialog"
        p.dialogs.sync()
        p.dialogs.show(p.views[1]!)
        window.makeFirstResponder(p.views[2])
        p.press(2)
        other.press(2)
        XCTAssertFalse(other.menus.key(key(window)))
        XCTAssertTrue(other.menus.isOpen(other.views[3]!))
        XCTAssertTrue(p.menus.key(key(window)))
        XCTAssertFalse(p.menus.isOpen(p.views[3]!))
        XCTAssertNotNil(p.dialogs.active)
        XCTAssertTrue(p.menus.key(key(window, repeatKey: true)))
        XCTAssertTrue(p.menus.key(key(window, up: true)))
        XCTAssertNotNil(p.dialogs.active)
        XCTAssertFalse(p.menus.key(key(window)))
        XCTAssertTrue(p.dialogs.key(key(window)))
        XCTAssertNil(p.dialogs.active)
        other.menus.reset()
    }
    func testCompositionOwnsEscapeAndNoAutofocusLeavesFocusAtTheInvoker() throws {
        let p = fixture(), window = p.viewport.window!
        p.views[4]!.props.removeValue(forKey: "autofocus")
        p.press(2)
        XCTAssertTrue(window.firstResponder === p.views[2])
        window.makeFirstResponder(p.views[4]!.field)
        let editor = try XCTUnwrap(window.firstResponder as? NSTextView)
        editor.setMarkedText("draft", selectedRange: NSRange(location: 0, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertFalse(p.menus.key(key(window)))
        XCTAssertTrue(p.menus.isOpen(p.views[3]!))
        editor.unmarkText()
        XCTAssertTrue(p.menus.key(key(window)))
    }
    func testNativeProjectionRequiresOnlyTextButtonsAndSeparators() {
        let p = fixture(), pop = p.views[3]!
        XCTAssertFalse(p.menus.isMenuShaped(pop), "a form must never become a menu")
        p.apply(wireBatch([["op": "children", "id": 3, "ids": [5]]]))
        XCTAssertTrue(p.menus.isMenuShaped(pop), "a hide-only button is still a menu row")
        p.views[5]!.props["accessibilityChecked"] = "true"
        p.views[5]!.props["disabled"] = "true"
        let menu = p.menus.menu(of: pop)
        XCTAssertEqual(menu.items.count, 1)
        XCTAssertEqual(menu.items[0].state, .on)
        XCTAssertFalse(menu.items[0].isEnabled)
        p.press(2)
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertEqual(pop.isHidden, !ExactEnv.agentMode)
        XCTAssertEqual(p.menus.owns(pop), ExactEnv.agentMode)
        p.menus.reset() // Retire a deferred NSMenu before its tracking loop.
        p.apply(wireBatch([["op": "create", "id": 8, "kind": "native"], ["op": "children", "id": 5, "ids": [8]]]))
        XCTAssertFalse(p.menus.isMenuShaped(pop), "custom content inside a button must keep its pixels")
    }
    /// HTML's `hr` (LLP 1021 D1) is a menu row: the separator between items.
    func testAnHrRowIsTheMenusSeparator() {
        let p = fixture(), pop = p.views[3]!
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 10, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "form", "popovertargetaction": "hide"]],
            ["op": "children", "id": 3, "ids": [5, 9, 10]],
        ]))
        XCTAssertTrue(p.menus.isMenuShaped(pop), "an hr keeps a menu menu-shaped")
        let menu = p.menus.menu(of: pop)
        XCTAssertEqual(menu.items.map(\.isSeparatorItem), [false, true, false])
    }
    func testNativeHideButtonClosesTheFormThroughAppKit() throws {
        let p = fixture(), pop = p.views[3]!
        p.buttonFace = { _ in var face = ButtonFace(); face.title = "Close"; return face }
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "control", "props": ["type": "button", "popovertarget": "form", "popovertargetaction": "hide"], "style": ["appearance": "auto"]],
            ["op": "children", "id": 3, "ids": [4, 7]],
            ["op": "frame", "id": 7, "x": 10, "y": 80, "w": 80, "h": 30],
        ]))
        p.press(2)
        XCTAssertTrue(p.menus.owns(pop), "the input keeps this a form")
        let button = try XCTUnwrap(p.controls.controls[7] as? NSButton)
        button.performClick(nil)
        XCTAssertFalse(p.menus.isOpen(pop), "a native hide-only row takes its own command")
        XCTAssertTrue(pop.isHidden)
        XCTAssertTrue(p.viewport.window?.firstResponder === p.views[2])
        XCTAssertEqual(p.views[4]?.field?.stringValue, "draft")
    }
    func testNativeAndCustomSymbolRowsKeepTheirMenuFacesAndActions() throws {
        let p = fixture(), pop = p.views[3]!
        p.buttonFace = { id in
            var face = ButtonFace()
            if id == 5 { face.label = "Close"; face.symbol = "xmark" }
            if id == 7 { face.title = "Send"; face.symbol = "paperplane" }
            return face
        }
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "control", "handlers": ["press"], "props": ["type": "button", "accessibilityChecked": "true"], "style": ["appearance": "auto"]],
            ["op": "create", "id": 8, "kind": "image", "props": ["symbolName": "xmark"]],
            ["op": "children", "id": 5, "ids": [8]],
            ["op": "children", "id": 3, "ids": [5, 7]],
        ]))
        XCTAssertTrue(p.menus.isMenuShaped(pop), "native and custom symbol rows remain menu items")
        let menu = p.menus.menu(of: pop)
        XCTAssertEqual(menu.items.map(\.title), ["Close", "Send"])
        XCTAssertTrue(menu.items.allSatisfy { $0.image != nil })
        XCTAssertEqual(menu.items.last?.state, .on)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.press(2)
        defer { p.menus.reset() }
        // The presentation's own menu: one built before it opened is stale.
        let item = try XCTUnwrap(p.menus.menu(of: pop, from: p.views[2]!).items.last)
        NSApp.sendAction(try XCTUnwrap(item.action), to: item.target, from: item)
        NSApp.sendAction(try XCTUnwrap(item.action), to: item.target, from: item)
        XCTAssertEqual(pressed, [], "not inside the menu's action")
        // Retire the deferred popUp (it would track during the turn); the pick stands.
        p.menus.close(pop)
        turn()
        XCTAssertEqual(pressed, [7], "a native menu row still invokes the app action, once")
    }
    /// A picked item presses on the next main-queue turn: let every turn
    /// queued so far run (the main queue is FIFO).
    private func turn() {
        let turned = expectation(description: "the next turn")
        DispatchQueue.main.async { turned.fulfill() }
        wait(for: [turned], timeout: 5)
    }
    /// A menu of two press rows (10 "Copy", 11 "Share") opened by 2.
    private func menuFixture() -> (Presenter, NSMenu) {
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "form", "popovertargetaction": "hide"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "form", "popovertargetaction": "hide"]],
            ["op": "create", "id": 12, "kind": "text", "props": ["text": "Copy"]],
            ["op": "create", "id": 13, "kind": "text", "props": ["text": "Share"]],
            ["op": "children", "id": 10, "ids": [12]], ["op": "children", "id": 11, "ids": [13]],
            ["op": "children", "id": 3, "ids": [10, 11]],
        ]))
        return (p, p.menus.menu(of: p.views[3]!))
    }
    /// A picked row presses once, a turn later, only if it is still the row
    /// the menu showed; a reset before the turn, a hidden or retitled row,
    /// or an id reused by another node presses nothing.
    func testAPickedMenuRowPressesOnceOnTheNextTurnWhileItIsStillThatRow() throws {
        let changes: [[[String: Any]]?] = [
            nil,
            [["op": "style", "id": 11, "style": ["display": "none"]]],
            [["op": "props", "id": 13, "set": ["text": "Print"]]],
            [["op": "destroy", "id": 11], ["op": "destroy", "id": 13],
             ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "form", "popovertargetaction": "hide"]],
             ["op": "children", "id": 3, "ids": [10, 11]]],
        ]
        for change in changes {
            let (p, menu) = menuFixture()
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            let share = menu.items[1]
            XCTAssertEqual(share.title, "Share")
            NSApp.sendAction(try XCTUnwrap(share.action), to: share.target, from: share)
            NSApp.sendAction(try XCTUnwrap(menu.items[0].action), to: menu.items[0].target, from: menu.items[0])
            if let change { p.apply(wireBatch(change)) }
            turn()
            XCTAssertEqual(pressed, change == nil ? [11] : [], "\(change ?? [])")
        }
        let (p, menu) = menuFixture()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        NSApp.sendAction(try XCTUnwrap(menu.items[0].action), to: menu.items[0].target, from: menu.items[0])
        p.menus.reset()
        turn()
        XCTAssertEqual(pressed, [], "reset before its turn")
    }
    /// A pick belongs to its invoker and its presentation: a batch before its
    /// turn that unmounts, hides or disables the invoker — the shared
    /// popover still mounted — cancels it, as does a retitle undone by a
    /// later batch and the popover presented again. Untouched, it presses.
    func testAPickDiesWithItsInvokerOrItsPresentation() throws {
        let changes: [(String, (Presenter) -> Void)] = [
            ("untouched", { _ in }),
            ("invoker unmounted", { p in
                p.apply(wireBatch([["op": "children", "id": 1, "ids": [6, 3]], ["op": "destroy", "id": 2]])) }),
            ("invoker hidden", { p in p.apply(wireBatch([["op": "style", "id": 2, "style": ["display": "none"]]])) }),
            ("invoker disabled", { p in p.apply(wireBatch([["op": "props", "id": 2, "set": ["disabled": "true"]]])) }),
            ("retitled, then restored", { p in
                p.apply(wireBatch([["op": "props", "id": 13, "set": ["text": "Print"]]]))
                p.apply(wireBatch([["op": "props", "id": 13, "set": ["text": "Share"]]])) }),
            ("presented again", { p in p.menus.show(p.views[3]!, from: p.views[2]!); p.menus.close(p.views[3]!) }),
        ]
        for (name, change) in changes {
            let (p, _) = menuFixture()
            // A second invoker keeps the popover shared and mounted.
            p.apply(wireBatch([["op": "create", "id": 7, "kind": "button", "props": ["popovertarget": "form"]],
                               ["op": "children", "id": 1, "ids": [2, 6, 7, 3]]]))
            let menu = p.menus.menu(of: p.views[3]!, from: p.views[2]!)
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            let share = menu.items[1]
            NSApp.sendAction(try XCTUnwrap(share.action), to: share.target, from: share)
            change(p)
            turn()
            XCTAssertEqual(pressed, name == "untouched" ? [11] : [], name)
            p.menus.reset()
        }
    }
    /// A hidden or inert row is an item that cannot be chosen, as a chooser's.
    func testAHiddenOrInertRowIsADisabledItem() throws {
        for change: [String: Any] in [["op": "style", "id": 11, "style": ["display": "none"]],
                                      ["op": "props", "id": 11, "set": ["inert": "true"]]] {
            let (p, _) = menuFixture()
            p.apply(wireBatch([change]))
            let menu = p.menus.menu(of: p.views[3]!, from: p.views[2]!)
            XCTAssertEqual(menu.items.map(\.title), ["Copy", "Share"])
            XCTAssertEqual(menu.items.map(\.isEnabled), [true, false], "\(change)")
        }
    }
    /// The real popUp: a row picked inside AppKit's tracking whose press's
    /// batch unmounts the invoker the menu was popped up in. It presses
    /// once, after the tracking has ended.
    func testAPickThatUnmountsTheInvokerPressesOnceAfterTheMenuEnds() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent keeps every popover painted (LLP 1021 D4)")
        let (p, _) = menuFixture()
        try XCTUnwrap(windows.last).orderFrontRegardless()
        var pressed: [UInt32] = []
        var tracking = false, trackingAtPress: Bool?
        p.onPress = { id in
            pressed.append(id)
            trackingAtPress = tracking
            p.apply(wireBatch([["op": "children", "id": 1, "ids": []]]
                              + [2, 3, 4, 5, 6, 10, 11, 12, 13].map { ["op": "destroy", "id": $0] }))
        }
        var seen: NSMenu?
        let center = NotificationCenter.default
        let begin = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu, menu.items.contains(where: { $0.title == "Share" }) else { return }
            seen = menu
            tracking = true
            RunLoop.current.perform(inModes: [.eventTracking, .default]) {
                menu.performActionForItem(at: 1)
                XCTAssertEqual(pressed, [], "nothing presses inside the tracking")
                menu.cancelTracking()
            }
        }
        let end = center.addObserver(forName: NSMenu.didEndTrackingNotification, object: nil, queue: nil) { note in
            if note.object as? NSMenu === seen { tracking = false }
        }
        defer { center.removeObserver(begin); center.removeObserver(end) }
        p.press(2)
        let done = Date(timeIntervalSinceNow: 5)
        while pressed.isEmpty && Date() < done { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        turn()
        XCTAssertNotNil(seen, "the menu tracked")
        XCTAssertEqual(pressed, [11], "Share, once")
        XCTAssertEqual(trackingAtPress, false, "pressed after the menu's tracking ended")
        XCTAssertNil(p.views[2], "the invoker is unmounted")
    }
    func testNativeContextAndButtonMenusKeepTheMainQueueRunningWhileTracking() throws {
        try XCTSkipIf(ExactEnv.agentMode, "agent menus are painted")
        for context in [false, true] {
            let (p, _) = menuFixture()
            try XCTUnwrap(windows.last).orderFrontRegardless()
            var tracking = false, tracked = false, ranDuring = false
            var fallback: Timer?
            let center = NotificationCenter.default
            let begin = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
                guard let menu = note.object as? NSMenu, menu.items.contains(where: { $0.title == "Share" }) else { return }
                tracking = true; tracked = true
                // Native module calls use this same queue. A menu opened
                // inside a dispatch callout used to starve it until close.
                DispatchQueue.main.async {
                    if tracking { ranDuring = true }
                    menu.cancelTracking()
                }
                let timer = Timer(timeInterval: 0.25, repeats: false) { _ in menu.cancelTracking() }
                fallback = timer; RunLoop.main.add(timer, forMode: .common)
            }
            let end = center.addObserver(forName: NSMenu.didEndTrackingNotification, object: nil, queue: nil) { _ in tracking = false }
            defer { center.removeObserver(begin); center.removeObserver(end); fallback?.invalidate(); p.menus.reset() }
            if context {
                p.views[2]?.props["contextPopover"] = "form"
                p.menus.context(try XCTUnwrap(p.views[2]), at: .zero)
            } else { p.press(2) }
            let deadline = Date(timeIntervalSinceNow: 5)
            while (!tracked || tracking) && Date() < deadline { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
            XCTAssertTrue(tracked, "a real NSMenu tracked")
            XCTAssertTrue(ranDuring, context ? "context menu" : "button menu")
        }
    }
    func testNestedAutoPopoversKeepOnlyTheirAncestorBranch() {
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "view", "props": ["popover": "auto", "id": "nested"]],
            ["op": "create", "id": 8, "kind": "input"],
            ["op": "create", "id": 9, "kind": "button", "props": ["popovertarget": "nested"]],
            ["op": "children", "id": 7, "ids": [8]],
            ["op": "children", "id": 3, "ids": [4, 5, 9]],
            ["op": "children", "id": 1, "ids": [2, 6, 3, 7]],
            ["op": "frame", "id": 7, "x": 0, "y": 0, "w": 100, "h": 70],
        ]))
        p.press(2); p.press(9)
        XCTAssertEqual(p.menus.presented.map(\.id), [3, 7])
        p.menus.close(p.views[3]!)
        XCTAssertTrue(p.menus.presented.isEmpty)
        p.press(2)
        p.views[6]!.props["popovertarget"] = "nested"
        p.press(6)
        XCTAssertEqual(p.menus.presented.map(\.id), [7], "an unrelated invoker closes the old branch")
        p.apply(wireBatch([["op": "destroy", "id": 6]]))
        XCTAssertTrue(p.menus.presented.isEmpty, "destroying the invoker retires its presentation")
    }
    func testAuthorVisibilityAndRemovingThePopoverAttributeRestoreOrdinaryContent() {
        let p = fixture(), pop = p.views[3]!
        p.press(2)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["display": "none"]]]))
        XCTAssertFalse(p.menus.isOpen(pop))
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["display": "block"]]]))
        p.press(2)
        p.apply(wireBatch([["op": "props", "id": 3, "set": [:], "clear": ["popover"]]]))
        XCTAssertFalse(p.menus.isOpen(pop))
        XCTAssertFalse(pop.isHidden)
        XCTAssertTrue(pop.superview === p.views[1])
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["popover": "auto"], "clear": []]]))
        XCTAssertTrue(pop.isHidden)
        p.apply(wireBatch([["op": "props", "id": 3, "set": [:], "clear": ["popover"]]]))
        XCTAssertFalse(pop.isHidden, "removing the attribute also restores a closed popover")
    }
}
#endif

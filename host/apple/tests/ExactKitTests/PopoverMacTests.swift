#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PopoverMacTests: XCTestCase {
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

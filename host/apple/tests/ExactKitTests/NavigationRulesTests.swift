// The navigation, sheet, keyboard and focus rules (LLP 1035.001 slice 1),
// held as assertions about decisions: no window, no run loop, no clock,
// nothing that can flake on timing. `NavigationRules` is what UIKit's
// projection applies; this file is what says the projection's rules are
// the RFC's.
//
// @ref LLP 1035.001 D1–D5, D8; `rules/RULES.md` §Loop shape
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class NavigationRulesTests: XCTestCase {
    func testRawSymbolMissKeepsImageOnlySegmentsBlank() throws {
        _ = NSApplication.shared
        let p = Presenter()
        defer { p.reset() }
        func apply(_ ops: [[String: Any]]) {
            p.apply(batchFixture(ops: ops, timers: false, motion: false, clock: nil, error: nil))
        }
        apply([
            ["op": "create", "id": 1, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            ["op": "create", "id": 2, "kind": "button", "props": ["accessibilityRole": "tab", "accessibilityLabel": "Home", "accessibilitySelected": "true"], "handlers": ["press"]],
            ["op": "create", "id": 3, "kind": "button", "props": ["accessibilityRole": "tab", "accessibilityLabel": "Saved"], "handlers": ["press"]],
            ["op": "create", "id": 4, "kind": "image", "props": ["imageSource": "symbol:sf/airpodsmax", "symbolName": "airpodsmax"]],
            ["op": "create", "id": 5, "kind": "image", "props": ["imageSource": "symbol:sf/star", "symbolName": "star"]],
            ["op": "children", "id": 2, "ids": [4]],
            ["op": "children", "id": 3, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
        ])
        let owner = try XCTUnwrap(p.views[1]), icon = try XCTUnwrap(p.views[4])
        let segments = try XCTUnwrap(owner.subviews.first { $0 is NSSegmentedControl } as? NSSegmentedControl)
        for name in ["airpodsmax", "exact.nonexistent.symbol", "", "airpodsmax"] {
            apply([["op": "props", "id": 4, "set": ["imageSource": "symbol:sf/\(name)", "symbolName": name]]])
            XCTAssertEqual(icon.image != nil, name == "airpodsmax")
            XCTAssertEqual(segments.image(forSegment: 0)?.accessibilityDescription, "Home")
            XCTAssertEqual(segments.label(forSegment: 0), "")
            XCTAssertEqual(segments.toolTip(forSegment: 0), "Home")
            XCTAssertEqual(segments.selectedSegment, 0)
            XCTAssertNil(icon.symbolRefusal)
        }
    }

    func testListSiblingUpdatesAndReparentingPreserveInheritedAvailability() throws {
        _ = NSApplication.shared
        let p = Presenter()
        defer { p.reset() }
        func apply(_ ops: [[String: Any]]) {
            p.apply(batchFixture(ops: ops, timers: false, motion: false, clock: nil, error: nil))
        }
        apply([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["inert": "true"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "input"],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 1, "ids": [3]],
            ["op": "roots", "ids": [1, 2]]
        ])
        let input = try XCTUnwrap(p.views[4])
        XCTAssertFalse(input.isAccessibilityHidden())
        apply([
            ["op": "create", "id": 5, "kind": "view"],
            ["op": "children", "id": 1, "ids": [3, 5]]
        ])
        XCTAssertFalse(input.isAccessibilityHidden(), "retaining a sibling preserves its inherited gate")
        apply([
            ["op": "children", "id": 1, "ids": [5]],
            ["op": "children", "id": 2, "ids": [3]]
        ])
        XCTAssertTrue(input.isAccessibilityHidden(), "moving a retained subtree under inert changes descendants")
        apply([["op": "props", "id": 2, "clear": ["inert"]]])
        XCTAssertFalse(input.isAccessibilityHidden(), "an ancestor gate change still visits unchanged descendants")
        apply([["op": "props", "id": 2, "set": ["inert": "true"]]])
        XCTAssertTrue(input.isAccessibilityHidden())
        apply([
            ["op": "children", "id": 2, "ids": []],
            ["op": "children", "id": 1, "ids": [3, 5]]
        ])
        XCTAssertFalse(input.isAccessibilityHidden(), "moving out of inert restores descendants")
    }

    func testNativeAvailabilityFollowsCurrentStateAndAncestorChanges() {
        _ = NSApplication.shared
        let p = Presenter()
        let owner = NodeView(id: 1, kind: "view", presenter: p)
        let input = NodeView(id: 2, kind: "input", presenter: p)
        let area = NodeView(id: 3, kind: "textarea", presenter: p)
        input.field = NSTextField(); area.textArea = NSTextView()
        p.views = [1: owner, 2: input, 3: area]
        p.root.addSubview(owner); owner.addSubview(input); owner.addSubview(area)
        p.navigation.sync(batchFixture(ops: [["op": "roots", "ids": [1]]], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertFalse(input.isAccessibilityHidden())
        XCTAssertTrue(input.field!.isEnabled); XCTAssertTrue(area.textArea!.isEditable)
        owner.props["inert"] = "true"; p.navigation.sync(batchFixture(ops: [["op": "props", "id": 1], ["op": "props", "id": 2], ["op": "props", "id": 3]], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertTrue(input.isAccessibilityHidden()); XCTAssertTrue(area.isAccessibilityHidden())
        XCTAssertFalse(input.field!.isEnabled); XCTAssertFalse(area.textArea!.isEditable)
        owner.props.removeValue(forKey: "inert"); p.navigation.sync(batchFixture(ops: [["op": "props", "id": 1], ["op": "props", "id": 2], ["op": "props", "id": 3]], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertFalse(input.isAccessibilityHidden()); XCTAssertTrue(input.field!.isEnabled)
        XCTAssertTrue(area.textArea!.isEditable)
        input.props["disabled"] = "true"; area.props["editable"] = "false"; p.navigation.sync(batchFixture(ops: [["op": "props", "id": 1], ["op": "props", "id": 2], ["op": "props", "id": 3]], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertFalse(input.field!.isEnabled); XCTAssertFalse(area.textArea!.isEditable)
        owner.isHidden = true; p.navigation.sync(batchFixture(ops: [["op": "props", "id": 1], ["op": "props", "id": 2], ["op": "props", "id": 3]], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertTrue(input.isAccessibilityHidden())
        owner.isHidden = false; p.navigation.sync(batchFixture(ops: [["op": "props", "id": 1], ["op": "props", "id": 2], ["op": "props", "id": 3]], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertFalse(input.isAccessibilityHidden())
        // Another native projection can change state between identical batches.
        input.setAccessibilityHidden(true); input.field!.isEnabled = true
        area.textArea!.isEditable = true; p.navigation.sync(batchFixture(ops: [["op": "props", "id": 1], ["op": "props", "id": 2], ["op": "props", "id": 3]], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertFalse(input.isAccessibilityHidden())
        XCTAssertFalse(input.field!.isEnabled); XCTAssertFalse(area.textArea!.isEditable)
    }

    // @ref LLP 1038 D6 — shared first controller is insufficient for a swap.
    func testPushOrPopRequiresACompletePrefix() {
        XCTAssertTrue(NavigationRules.isPushOrPop(from: [1], to: [1, 2, 3]))
        XCTAssertTrue(NavigationRules.isPushOrPop(from: [1, 2, 3], to: [1]))
        XCTAssertTrue(NavigationRules.isPushOrPop(from: [1, 2], to: [1, 2]))
        XCTAssertFalse(NavigationRules.isPushOrPop(from: [1, 2], to: [1, 3]))
        XCTAssertFalse(NavigationRules.isPushOrPop(from: [1, 2], to: [4, 5]))
    }

    /// D1: the stack is the prefix through the selected route; a key that
    /// matches no route leaves the stack alone.
    func testTheStackIsThePrefixThroughTheSelectedRoute() {
        let keys = ["", "thread", "details:thread", "compose"]
        XCTAssertEqual(NavigationRules.stack(routeKeys: keys, selected: ""), 0..<1)
        XCTAssertEqual(NavigationRules.stack(routeKeys: keys, selected: "details:thread"), 0..<3)
        XCTAssertNil(NavigationRules.stack(routeKeys: keys, selected: "elsewhere"))
    }

    func testPresentationBoundariesRetainTheirOwnerThroughLaterRoutes() {
        XCTAssertEqual(NavigationRules.segments(presentations: [nil, nil, "fullscreen", "modal"]), [0..<2, 2..<3, 3..<4])
        XCTAssertEqual(NavigationRules.segments(presentations: [nil, "fullscreen", nil]), [0..<1, 1..<3])
        XCTAssertEqual(NavigationRules.segments(presentations: [nil, nil]), [0..<2])
        XCTAssertEqual(NavigationRules.segments(presentations: ["modal"]), [0..<0, 0..<1])
    }

    /// D2: only a completed gesture from the still-selected route may
    /// dispatch. Finishing an old programmatic Back cannot cancel Compose.
    func testACompletedPopBelongsToItsInteractiveSource() {
        XCTAssertTrue(NavigationRules.dispatchesBack(shownKey: "", rootKey: "thread", sourceKey: "thread", modalActive: false))
        // Cancelled: UIKit shows the same route the root still names.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "thread", rootKey: "thread", sourceKey: "thread", modalActive: false))
        // Programmatic: the key already moved when UIKit finished.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "", rootKey: "", sourceKey: nil, modalActive: false))
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "", rootKey: "compose", sourceKey: nil, modalActive: false))
        // An interactive completion cannot dismiss a newly selected route.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "", rootKey: "compose", sourceKey: "thread", modalActive: false))
        // A sheet's dismissal has its own path.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "", rootKey: "compose", sourceKey: "compose", modalActive: true))
    }

    /// D2's exception: the app replaced the swiped screen in place while the
    /// finger was down (capture giving way to what it wrote). The gesture
    /// applies to the replacement, so it is not pushed back in.
    func testASourceReplacedInPlaceIsStillWhatTheSwipeDismissed() {
        XCTAssertTrue(NavigationRules.dispatchesBack(shownKey: "journal", rootKey: "day", sourceKey: nil, sourceReplaced: true, modalActive: false))
        // A cancelled swipe still dispatches nothing, replaced or not.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "day", rootKey: "day", sourceKey: nil, sourceReplaced: true, modalActive: false))
        // A newly selected route that did not replace the source keeps D2's rule.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "journal", rootKey: "compose", sourceKey: nil, sourceReplaced: false, modalActive: false))
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "journal", rootKey: "day", sourceKey: nil, sourceReplaced: true, modalActive: true))
    }

    /// D1: the Back control is resolved by id among enabled, pressable, live
    /// controls, lowest view id first; a disabled one blocks nothing but
    /// resolves to nothing.
    func testTheBackControlIsResolvedAtUseByHTMLId() {
        struct C { let id: UInt32; let html: String?; let press: Bool; let disabled: Bool; var active = true }
        let controls = [
            C(id: 1, html: "back", press: true, disabled: false, active: false),
            C(id: 9, html: "back", press: true, disabled: true),
            C(id: 12, html: "back", press: true, disabled: false),
            C(id: 4, html: "back", press: false, disabled: false),
            C(id: 3, html: "close", press: true, disabled: false),
        ]
        let resolve = { (target: String?) -> UInt32? in
            NavigationRules.backControl(named: target, among: controls, id: \.id, htmlID: \.html, pressable: \.press, disabled: \.disabled, inActiveRoute: \.active)?.id
        }
        XCTAssertEqual(resolve("back"), 12)
        XCTAssertEqual(resolve("close"), 3)
        XCTAssertNil(resolve("missing"))
        XCTAssertNil(resolve(nil))
        // All disabled: nothing may begin.
        let disabled = [C(id: 1, html: "back", press: true, disabled: true)]
        XCTAssertNil(NavigationRules.backControl(named: "back", among: disabled, id: \.id, htmlID: \.html, pressable: \.press, disabled: \.disabled, inActiveRoute: \.active))
        let inactive = controls.filter { !$0.active }
        XCTAssertNil(NavigationRules.backControl(named: "back", among: inactive, id: \.id, htmlID: \.html, pressable: \.press, disabled: \.disabled, inActiveRoute: \.active))
    }

    /// D1: an interactive pop needs a stack to pop, no transition, no sheet,
    /// a Back control, and no context preview.
    func testWhenAPopMayBegin() {
        XCTAssertTrue(NavigationRules.popMayBegin(depth: 2, changing: false, modalActive: false, hasBackControl: true, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 1, changing: false, modalActive: false, hasBackControl: true, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 2, changing: true, modalActive: false, hasBackControl: true, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 2, changing: false, modalActive: true, hasBackControl: true, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 2, changing: false, modalActive: false, hasBackControl: false, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 2, changing: false, modalActive: false, hasBackControl: true, contextPreviewActive: true))
    }

    /// D1's arbitration: the edge is navigation's; past it a `swiperight`
    /// node under the start wins; then horizontal beats vertical.
    func testAPanYieldsToASwipeRightNodePastTheEdge() {
        XCTAssertTrue(NavigationRules.panMayBegin(startX: 10, overSwipeRight: true, velocity: CGPoint(x: 300, y: 20)))
        XCTAssertFalse(NavigationRules.panMayBegin(startX: 40, overSwipeRight: true, velocity: CGPoint(x: 300, y: 20)))
        XCTAssertTrue(NavigationRules.panMayBegin(startX: 40, overSwipeRight: false, velocity: CGPoint(x: 300, y: 20)))
        XCTAssertFalse(NavigationRules.panMayBegin(startX: 40, overSwipeRight: false, velocity: CGPoint(x: 20, y: 300)))
        // Asked before any motion: allowed, unless over a swipe past the edge.
        XCTAssertTrue(NavigationRules.panMayBegin(startX: 12, overSwipeRight: true, velocity: .zero))
        XCTAssertTrue(NavigationRules.panMayBegin(startX: 40, overSwipeRight: false, velocity: .zero))
        XCTAssertFalse(NavigationRules.panMayBegin(startX: 40, overSwipeRight: true, velocity: .zero))
        // Zero velocity reads the travel: horizontal pops, vertical is refused.
        let horizontal = NavigationRules.popDirection(velocity: .zero, travel: CGPoint(x: 12, y: 2))
        XCTAssertTrue(NavigationRules.panMayBegin(startX: 40, overSwipeRight: false, velocity: horizontal))
        let vertical = NavigationRules.popDirection(velocity: .zero, travel: CGPoint(x: 1, y: 14))
        XCTAssertFalse(NavigationRules.panMayBegin(startX: 10, overSwipeRight: false, velocity: vertical))
        XCTAssertEqual(NavigationRules.popDirection(velocity: CGPoint(x: 0, y: 300), travel: CGPoint(x: 12, y: 0)), CGPoint(x: 0, y: 300), "a velocity wins")
    }

    /// D1: `closedby="none"` refuses the sheet gesture; anything else permits it.
    func testClosedByNoneRefusesDismissal() {
        XCTAssertTrue(NavigationRules.modalRefusesDismissal(closedby: "none"))
        XCTAssertFalse(NavigationRules.modalRefusesDismissal(closedby: "closerequest"))
        XCTAssertFalse(NavigationRules.modalRefusesDismissal(closedby: nil))
    }

    /// D4: deferred geometry replays frames before contents, ids ascending.
    func testDeferredGeometryReplaysFramesBeforeContentsInIdOrder() {
        let order = NavigationRules.replayOrder(deferred: [7: ["content", "frame"], 3: ["frame"], 12: ["content"]])
        XCTAssertEqual(order.map { "\($0.id):\($0.kind)" }, ["3:frame", "7:frame", "7:content", "12:content"])
    }

    /// D5: a session answers a keyboard only for its own editor, or while it
    /// still holds an inset it applied.
    func testAKeyboardConcernsOnlyTheSessionThatOwnsIt() {
        XCTAssertTrue(NavigationRules.keyboardConcerns(editing: true, holdsInset: false))
        XCTAssertTrue(NavigationRules.keyboardConcerns(editing: false, holdsInset: true))
        XCTAssertFalse(NavigationRules.keyboardConcerns(editing: false, holdsInset: false))
    }

    /// D5: the viewport freeze is for an initially interactive pop, never a
    /// sheet — the Messages defect of 2026-09-09.
    func testTheViewportFreezeIsForAnInteractivePopOnly() {
        XCTAssertTrue(NavigationRules.freezesViewport(modalActive: false, changing: true, initiallyInteractive: true))
        XCTAssertFalse(NavigationRules.freezesViewport(modalActive: true, changing: true, initiallyInteractive: true))
        XCTAssertFalse(NavigationRules.freezesViewport(modalActive: false, changing: false, initiallyInteractive: true))
        XCTAssertFalse(NavigationRules.freezesViewport(modalActive: false, changing: true, initiallyInteractive: false))
    }

    /// D3: a focus that cannot be delivered has a named reason, in a fixed
    /// order, and a deliverable one has none.
    func testAFocusRefusalNamesItsReason() {
        XCTAssertNil(NavigationRules.focusRefusal(mounted: true, disabled: false, zeroSize: false, hiddenAncestor: false, inertAncestor: false))
        XCTAssertEqual(NavigationRules.focusRefusal(mounted: false, disabled: true, zeroSize: true, hiddenAncestor: true, inertAncestor: true), "not mounted")
        XCTAssertEqual(NavigationRules.focusRefusal(mounted: true, disabled: false, zeroSize: false, hiddenAncestor: false, inertAncestor: true), "inert ancestor")
    }
}

final class MacShortcutTests: XCTestCase {
    private final class MenuAction: NSObject {
        let body: () -> Void
        init(_ body: @escaping () -> Void) { self.body = body }
        @objc func invoke(_ sender: Any?) { body() }
    }

    private func window(_ presenter: Presenter) -> NSWindow {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                              styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.root
        return window
    }

    private func button(_ id: UInt32, _ title: String, _ shortcut: String, in presenter: Presenter, parent: NSView? = nil) -> NodeView {
        let node = NodeView(id: id, kind: "button", presenter: presenter)
        node.props = ["accessibilityLabel": title, "accessibilityKeyShortcuts": shortcut]
        node.handlers = ["press"]
        presenter.views[id] = node
        (parent ?? presenter.root).addSubview(node)
        return node
    }

    private func event(_ key: String, window: NSWindow, modifiers: NSEvent.ModifierFlags = .command,
                       code: UInt16 = 0, ignoring: String? = nil, repeat repeating: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers,
                         timestamp: 0, windowNumber: window.windowNumber, context: nil,
                         characters: key, charactersIgnoringModifiers: ignoring ?? key, isARepeat: repeating, keyCode: code)!
    }

    func testNamedKeysPrintablePlusAndContextKeysReachTheirDeclaredAction() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let node = button(1, "Command", "", in: presenter)
        var presses = 0
        presenter.onPress = { _ in presses += 1 }
        let cases: [(String, String, NSEvent.ModifierFlags, UInt16)] = [
            ("Meta+Shift+Enter", "\r", [.command, .shift], 36),
            ("Meta+Shift+Enter", "\u{3}", [.command, .shift], 76),
            ("Meta+Shift+ArrowUp", "\u{f700}", [.command, .shift], 126),
            ("Meta+Shift+ArrowDown", "\u{f701}", [.command, .shift], 125),
            ("Control+ArrowLeft", "\u{f702}", .control, 123),
            ("Control+ArrowRight", "\u{f703}", .control, 124),
            ("Meta+Shift+Plus", "+", [.command, .shift], 24),
            ("Meta+Shift++", "+", [.command, .shift], 24),
            ("Plus", "+", [], 69),
            ("c", "c", [], 8),
            ("Shift+C", "C", .shift, 8),
            ("Space", " ", [], 49),
            ("Escape", "\u{1b}", [], 53),
            ("Tab", "\t", [], 48),
            ("Backspace", "\u{7f}", [], 51),
            ("Delete", "\u{f728}", [], 117),
            ("Meta+Home", "\u{f729}", .command, 115),
            ("Meta+End", "\u{f72b}", .command, 119),
            ("PageUp", "\u{f72c}", [], 116),
            ("PageDown", "\u{f72d}", [], 121),
            ("F1", "\u{f704}", .function, 122),
            ("Meta+F12", "\u{f70f}", [.command, .function], 111),
        ]
        for (index, row) in cases.enumerated() {
            node.props["accessibilityKeyShortcuts"] = row.0
            XCTAssertTrue(presenter.shortcuts.perform(event(row.1, window: window, modifiers: row.2, code: row.3)), row.0)
            XCTAssertEqual(presses, index + 1, row.0)
            XCTAssertFalse(presenter.shortcuts.perform(event(row.1, window: window, modifiers: row.2.symmetricDifference(.option), code: row.3)), row.0)
        }
        for invalid in ["Meta+", "Meta+++", "Meta+Unknown", "Cmd+c", "F0", "F36", "Meta+Meta", "Enter+c"] {
            node.props["accessibilityKeyShortcuts"] = invalid
            XCTAssertFalse(presenter.shortcuts.perform(event("c", window: window)), invalid)
        }
    }

    func testNamedMenuEquivalentsAreAppKitCharacters() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        _ = button(1, "Send", "Meta+Shift+Enter", in: presenter)
        _ = button(2, "Previous", "Meta+ArrowUp", in: presenter)
        _ = button(3, "Zoom In", "Meta+Shift+Plus", in: presenter)
        let menu = ShortcutMenu(title: "File")
        presenter.shortcuts.attach(menu)
        XCTAssertEqual(Array(menu.items.prefix(3)).map(\.keyEquivalent), ["\r", "\u{f700}", "+"])
        XCTAssertEqual(menu.items[0].keyEquivalentModifierMask, [.command, .shift])
    }

    func testMenuFallbackRejectsExtraModifiersAndRestoresEquivalents() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let node = button(1, "Command", "Meta+Enter", in: presenter)
        let menu = ShortcutMenu(title: "File")
        menu.autoenablesItems = false
        presenter.shortcuts.attach(menu)
        for (name, key, code) in [("Enter", "\r", UInt16(36)), ("ArrowUp", "\u{f700}", UInt16(126))] {
            node.props["accessibilityKeyShortcuts"] = "Meta+" + name
            presenter.shortcuts.sync()
            let item = menu.items[0]
            let shifted = event(key, window: window, modifiers: [.command, .shift], code: code)
            XCTAssertFalse(presenter.shortcuts.perform(shifted))
            XCTAssertFalse(menu.performKeyEquivalent(with: shifted), name)
            XCTAssertEqual(item.keyEquivalent, key, "The visible menu equivalent must survive filtering")
            XCTAssertTrue(menu.performKeyEquivalent(with: event(key, window: window, code: code)), name)
            node.props["accessibilityKeyShortcuts"] = "Meta+Shift+" + name
            presenter.shortcuts.sync()
            XCTAssertTrue(menu.performKeyEquivalent(with: shifted), name)
        }
        // Unowned commands still use AppKit, and a top-level menu must recurse
        // through the filtered submenu rather than bypassing it.
        let bar = NSMenu(title: "Main")
        bar.addItem(withTitle: "File", action: nil, keyEquivalent: "").submenu = menu
        node.props["accessibilityKeyShortcuts"] = "Meta+Enter"
        presenter.shortcuts.sync()
        XCTAssertFalse(bar.performKeyEquivalent(with: event("\r", window: window, modifiers: [.command, .shift], code: 36)))
        let ordinary = menu.addItem(withTitle: "Ordinary", action: nil, keyEquivalent: "x")
        ordinary.keyEquivalentModifierMask = .command
        let update = MenuAction {
            node.props["accessibilityKeyShortcuts"] = "Meta+ArrowUp"
            presenter.shortcuts.sync()
        }
        ordinary.target = update
        ordinary.action = #selector(MenuAction.invoke(_:))
        XCTAssertTrue(menu.performKeyEquivalent(with: event("x", window: window)))
        XCTAssertEqual(menu.items[0].keyEquivalent, "\u{f700}", "Dispatch must not restore a stale equivalent over a plan update")
    }

    /// Issue #110: a key that types no Latin character (Korean 2-Set's ㅠ on
    /// B, key code 11) is its physical key's chord, as the web falls back to
    /// `code`; a Latin character is the key whatever the physical key is
    /// (AZERTY, Dvorak, German's ö), so one chord fires, never two.
    func testNonLatinCharactersMatchThePhysicalKeyAndLatinOnesTheirOwn() throws {
        // The physical key's character is the Mac's ASCII-capable layout's.
        guard KeyCodes.asciiCharacters(11, shift: false, option: false) == "b",
              KeyCodes.asciiCharacters(45, shift: false, option: false) == "n" else {
            throw XCTSkip("the ASCII-capable layout has no B on key 11 or N on key 45")
        }
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        _ = button(1, "Toggle Sidebar", "Meta+B", in: presenter)
        _ = button(2, "New Thread", "Meta+N", in: presenter)
        _ = button(3, "Inspector", "Meta+Shift+B Meta+;", in: presenter)
        var presses: [UInt32] = []
        presenter.onPress = { presses.append($0) }
        XCTAssertTrue(presenter.shortcuts.perform(event("ㅠ", window: window, code: 11)))
        XCTAssertTrue(presenter.shortcuts.perform(event("ㅠ", window: window, modifiers: [.command, .shift], code: 11)))
        XCTAssertTrue(presenter.shortcuts.perform(event("n", window: window, code: 11)))
        XCTAssertTrue(presenter.shortcuts.perform(event("b", window: window, code: 45)))
        XCTAssertFalse(presenter.shortcuts.perform(event("q", window: window, code: 11)))
        XCTAssertFalse(presenter.shortcuts.perform(event("ö", window: window, code: 41)), "German's ö is a Latin letter, not ;")
        XCTAssertFalse(presenter.shortcuts.perform(event("ㅠ", window: window, modifiers: [.command, .control], code: 11)))
        XCTAssertEqual(presses, [1, 3, 2, 1])
        // XCTest has no key window, so the item's action (`activate`) does not
        // press; that it is the item AppKit sends is what the menu decides.
        let menu = ShortcutMenu(title: "File")
        presenter.shortcuts.attach(menu)
        var sent: [String] = []
        let observer = NotificationCenter.default.addObserver(forName: NSMenu.didSendActionNotification, object: menu, queue: nil) {
            sent.append(($0.userInfo?["MenuItem"] as? NSMenuItem)?.title ?? "")
        }
        defer { NotificationCenter.default.removeObserver(observer) }
        XCTAssertTrue(menu.performKeyEquivalent(with: event("ㅠ", window: window, code: 11)))
        XCTAssertEqual(sent, ["Toggle Sidebar"])
        XCTAssertEqual(menu.items[0].keyEquivalent, "b", "The visible menu equivalent stays the declared letter")
        XCTAssertFalse(menu.performKeyEquivalent(with: event("ㅠ", window: window, modifiers: [.command, .control], code: 11)))
        XCTAssertFalse(menu.performKeyEquivalent(with: event("ㅂ", window: window, code: 15)), "R's key has no command")
        XCTAssertEqual(sent, ["Toggle Sidebar"])
    }

    func testOptionMatchesProducedCharactersAndDoesNotMatchDeadKeys() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let node = button(1, "Context", "Alt+c Alt+e", in: presenter)
        var presses = 0
        presenter.onPress = { _ in presses += 1 }
        let cedilla = event("ç", window: window, modifiers: .option, code: 8, ignoring: "c")
        XCTAssertFalse(presenter.shortcuts.perform(cedilla))
        XCTAssertFalse(presenter.shortcuts.perform(event("", window: window, modifiers: .option, code: 14, ignoring: "e")))
        node.props["accessibilityKeyShortcuts"] = "Alt+ç"
        XCTAssertTrue(presenter.shortcuts.perform(cedilla))
        XCTAssertEqual(presses, 1)
        let editor = NSTextView(frame: .zero)
        presenter.root.addSubview(editor)
        XCTAssertTrue(window.makeFirstResponder(editor))
        XCTAssertFalse(presenter.shortcuts.perform(cedilla), "Option text still belongs to the editor")
        XCTAssertEqual(presses, 1)
    }

    func testEditorKeepsTypingAndCompositionButPermitsDeclaredCommands() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        _ = button(1, "Context", "c ArrowUp Space Alt+c", in: presenter)
        _ = button(2, "Send", "Meta+Shift+Enter", in: presenter)
        _ = button(3, "Cancel", "Escape", in: presenter)
        let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 300, height: 100))
        presenter.root.addSubview(editor)
        XCTAssertTrue(window.makeFirstResponder(editor))
        var presses: [UInt32] = []
        presenter.onPress = { presses.append($0) }
        XCTAssertFalse(presenter.shortcuts.perform(event("c", window: window, modifiers: [], code: 8)))
        XCTAssertFalse(presenter.shortcuts.perform(event("c", window: window, modifiers: .option, code: 8)))
        XCTAssertFalse(presenter.shortcuts.perform(event("\u{f700}", window: window, modifiers: [], code: 126)))
        XCTAssertFalse(presenter.shortcuts.perform(event(" ", window: window, modifiers: [], code: 49)))
        let send = event("\r", window: window, modifiers: [.command, .shift], code: 36)
        let escape = event("\u{1b}", window: window, modifiers: [], code: 53)
        XCTAssertTrue(presenter.shortcuts.perform(send))
        XCTAssertTrue(presenter.shortcuts.perform(escape))
        editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(editor.hasMarkedText())
        XCTAssertFalse(presenter.shortcuts.perform(send))
        XCTAssertFalse(presenter.shortcuts.perform(escape))
        editor.unmarkText()
        XCTAssertTrue(presenter.shortcuts.perform(send))
        XCTAssertEqual(presses, [2, 3, 2])
    }

    func testFieldEditorKeepsContextKeysAndMenuRequiresTheKeyWindow() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let node = button(1, "Send", "Meta+Shift+Enter c ArrowDown", in: presenter)
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 30))
        presenter.root.addSubview(field)
        XCTAssertTrue(window.makeFirstResponder(field))
        XCTAssertTrue(window.firstResponder is NSTextView)
        var presses: [UInt32] = []
        presenter.onPress = { presses.append($0) }
        XCTAssertFalse(presenter.shortcuts.perform(event("c", window: window, modifiers: [], code: 8)))
        XCTAssertFalse(presenter.shortcuts.perform(event("\u{f701}", window: window, modifiers: [], code: 125)))
        let menu = ShortcutMenu(title: "File")
        presenter.shortcuts.attach(menu)
        let send = event("\r", window: window, modifiers: [.command, .shift], code: 36)
        // XCTest has no application event loop/key window. The menu must not
        // dispatch into an inactive window even when its equivalent matches.
        XCTAssertFalse(presenter.shortcuts.validateMenuItem(menu.items[0]))
        _ = menu.performKeyEquivalent(with: send)
        XCTAssertTrue(presses.isEmpty)
        XCTAssertTrue(presenter.shortcuts.perform(send))
        XCTAssertEqual(presses, [1])
        node.props["disabled"] = "true"
        presenter.shortcuts.sync()
        _ = menu.performKeyEquivalent(with: send)
        XCTAssertEqual(presses, [1])
        node.props["disabled"] = nil
        node.props["accessibilityKeyShortcuts"] = "Meta+F01 Meta+F+1"
        presenter.shortcuts.sync()
        XCTAssertTrue(menu.items.isEmpty, "Malformed function names must not become menu equivalents")
        XCTAssertFalse(presenter.shortcuts.perform(event("\u{f704}", window: window)))
    }

    /// The chrome passes visit what this index names instead of every view, so
    /// it has to follow every write to a view's props, whoever makes it.
    func testChromeIndexFollowsEveryWriteToPropsAndForgetsDestroyedViews() {
        let presenter = Presenter()
        func apply(_ ops: [[String: Any]]) {
            presenter.apply(batchFixture(ops: ops, timers: false, motion: false, clock: nil, error: nil))
        }
        apply([
            ["op": "create", "id": 1, "kind": "view", "props": ["semanticTag": "main"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["accessibilityRole": "listitem"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
        ])
        // A page's own landmarks and a list's rows are not chrome.
        XCTAssertFalse(presenter.chrome.hidesOrInerts)
        XCTAssertTrue(presenter.carrying("tag:dialog").isEmpty)
        XCTAssertTrue(presenter.carrying("role:tablist").isEmpty)

        apply([["op": "props", "id": 2, "set": ["popover": "auto", "accessibilityRole": "tablist"], "clear": []]])
        XCTAssertEqual(presenter.carrying("popover").map(\.id), [2])
        XCTAssertEqual(presenter.carrying("role:tablist").map(\.id), [2])
        XCTAssertTrue(presenter.chrome.hidesOrInerts)

        // A write that does not come through a batch is indexed too.
        presenter.views[1]!.props["semanticTag"] = "dialog"
        XCTAssertEqual(presenter.carrying("tag:dialog").map(\.id), [1])
        presenter.views[1]!.props["semanticTag"] = "main"
        XCTAssertTrue(presenter.carrying("tag:dialog").isEmpty)

        apply([["op": "props", "id": 2, "set": [:], "clear": ["popover", "accessibilityRole"]]])
        XCTAssertFalse(presenter.chrome.hidesOrInerts)
        apply([
            ["op": "props", "id": 2, "set": ["navigationBack": "true"], "clear": []],
            ["op": "children", "id": 1, "ids": []],
            ["op": "destroy", "id": 2],
        ])
        XCTAssertTrue(presenter.carrying("navigationBack").isEmpty)
        XCTAssertFalse(presenter.chrome.hidesOrInerts)
    }

    func testMenuSyncPreservesStaticCommandsAndUsesNativePlacement() {
        let presenter = Presenter()
        let window = window(presenter)
        defer { window.close() }
        let previousServices = NSApp.servicesMenu
        let previousWindows = NSApp.windowsMenu
        defer { NSApp.servicesMenu = previousServices; NSApp.windowsMenu = previousWindows }
        let compose = button(1, "New Post", "Meta+n Control+n", in: presenter)
        _ = button(2, "Preferences", "Meta+,", in: presenter)
        _ = button(6, "Back", "Meta+[", in: presenter)
        let tabs = NodeView(id: 4, kind: "column", presenter: presenter)
        tabs.props["accessibilityRole"] = "tablist"
        presenter.root.addSubview(tabs)
        let home = button(5, "Home", "Meta+1", in: presenter, parent: tabs)
        home.props["accessibilityRole"] = "tab"
        home.props["accessibilitySelected"] = "true"
        let bar = DevMenu.makeMenu(shortcuts: presenter.shortcuts, documents: true)
        let app = bar.items[0].submenu!
        let file = bar.items.first { $0.submenu?.title == "File" }!.submenu!
        let go = bar.items.first { $0.submenu?.title == "Go" }!.submenu!
        let open = file.items.first { $0.keyEquivalent == "o" }!
        let close = file.items.first { $0.keyEquivalent == "w" }!
        let new = file.items.first { $0.keyEquivalent == "n" }!
        XCTAssertEqual(app.items[2].title, "Settings…")
        XCTAssertEqual(app.items[2].keyEquivalent, ",")
        XCTAssertEqual(go.items.map(\.title), ["Back", "Home"])
        XCTAssertEqual(go.items[1].state, .on)
        XCTAssertFalse(file.items.contains { $0.keyEquivalent == "," || $0.keyEquivalent == "[" })
        XCTAssertNotNil(app.items.first { $0.action == #selector(NSApplication.hideOtherApplications(_:)) })
        XCTAssertNotNil(bar.items.first { $0.submenu?.title == "Window" })
        for _ in 0..<3 { presenter.shortcuts.sync() }
        XCTAssertTrue(file.items.first { $0.keyEquivalent == "o" } === open)
        XCTAssertTrue(file.items.first { $0.keyEquivalent == "w" } === close)
        XCTAssertTrue(file.items.first { $0.keyEquivalent == "n" } === new)
        XCTAssertEqual(file.items.filter { $0.keyEquivalent == "n" }.count, 1)
        compose.props["disabled"] = "true"
        home.props["inert"] = "true"
        presenter.shortcuts.sync()
        XCTAssertFalse(new.isEnabled)
        XCTAssertFalse(go.items[1].isEnabled)
        compose.removeFromSuperview()
        presenter.views.removeValue(forKey: compose.id)
        presenter.shortcuts.sync()
        XCTAssertFalse(file.items.contains { $0 === new })
        XCTAssertTrue(file.items.first === open)
    }

    /// Studio diary R16: a command's chord places it — Edit's own stand in
    /// for the host's items, View's join View — and no chord is in the bar
    /// twice; a host item gets its chord back when the command goes.
    func testEditAndViewCommandsTakeTheirPlacesAndTheHostsStepAside() {
        let presenter = Presenter()
        let window = window(presenter)
        defer { window.close() }
        let previousServices = NSApp.servicesMenu
        let previousWindows = NSApp.windowsMenu
        defer { NSApp.servicesMenu = previousServices; NSApp.windowsMenu = previousWindows }
        let undo = button(1, "Undo Move", "Meta+z", in: presenter)
        _ = button(2, "Duplicate", "Meta+d", in: presenter)
        _ = button(3, "Zoom In", "Meta+=", in: presenter)
        _ = button(4, "Close Board", "Meta+w", in: presenter)
        _ = button(5, "Export", "Meta+e", in: presenter)
        let bar = DevMenu.makeMenu(shortcuts: presenter.shortcuts, documents: false)
        let menu = { (title: String) in bar.items.first { $0.submenu?.title == title }!.submenu! }
        let shown = { (title: String) in menu(title).items.filter { !$0.isHidden && !$0.isSeparatorItem } }
        for _ in 0..<3 { presenter.shortcuts.sync() }
        XCTAssertEqual(shown("Edit").first?.title, "Undo Move")
        XCTAssertEqual(shown("Edit").first?.keyEquivalent, "z")
        XCTAssertEqual(menu("Edit").items.filter { $0.title == "Undo" }.map(\.isHidden), [true])
        XCTAssertEqual(shown("Edit").last?.title, "Duplicate")
        XCTAssertEqual(shown("View").first?.title, "Zoom In")
        XCTAssertEqual(shown("File").map(\.title).prefix(2), ["Close Board", "Export"])
        let close = menu("File").items.first { $0.title == "Close Window" }!
        XCTAssertEqual(close.keyEquivalent, "", "⌘W is the app's Close Board, once")
        let all: [NSMenuItem] = bar.items.flatMap { (item: NSMenuItem) -> [NSMenuItem] in item.submenu?.items ?? [] }
        let closers = all.filter { (item: NSMenuItem) -> Bool in item.keyEquivalent == "w" && item.keyEquivalentModifierMask == .command }
        XCTAssertEqual(closers.map(\.title), ["Close Board"])
        // A command that goes (a field being edited drops its chord) gives the host's back.
        undo.removeFromSuperview()
        presenter.views.removeValue(forKey: undo.id)
        presenter.shortcuts.sync()
        XCTAssertEqual(shown("Edit").first?.title, "Undo")
        XCTAssertEqual(shown("Edit").first?.keyEquivalent, "z")
        XCTAssertEqual(shown("Edit").last?.title, "Duplicate")
        XCTAssertEqual(shown("Edit").last?.keyEquivalent, "d")
    }

    func testShortcutsRespectDisabledInertHiddenRepeatedAndWindowOwnership() {
        let presenter = Presenter()
        let window = window(presenter)
        defer { window.close() }
        let owner = NodeView(id: 1, kind: "column", presenter: presenter)
        presenter.root.addSubview(owner)
        let node = button(2, "New Post", "Meta+n", in: presenter, parent: owner)
        var presses: [UInt32] = []
        presenter.onPress = { presses.append($0) }
        let key = event("n", window: window)
        XCTAssertTrue(presenter.shortcuts.perform(key))
        XCTAssertEqual(presses, [2])
        node.props["disabled"] = "true"
        XCTAssertTrue(presenter.shortcuts.perform(key))
        node.props["disabled"] = nil
        XCTAssertTrue(presenter.shortcuts.perform(event("n", window: window, repeat: true)))
        owner.props["inert"] = "true"
        XCTAssertFalse(presenter.shortcuts.perform(key))
        owner.props["inert"] = nil
        owner.routeInert = true
        XCTAssertFalse(presenter.shortcuts.perform(key))
        owner.routeInert = false
        owner.isHidden = true
        XCTAssertFalse(presenter.shortcuts.perform(key))
        owner.isHidden = false
        let other = self.window(Presenter())
        defer { other.close() }
        XCTAssertFalse(presenter.shortcuts.perform(event("n", window: other)))
        XCTAssertEqual(presses, [2])
        node.removeFromSuperview()
        XCTAssertFalse(presenter.shortcuts.perform(key))
    }

    func testShortcutsCannotActivateBehindANativeSheet() {
        let presenter = Presenter()
        let window = window(presenter)
        defer { window.close() }
        _ = button(1, "New Post", "Meta+n", in: presenter)
        var presses = 0
        presenter.onPress = { _ in presses += 1 }
        let sheet = NSWindow(contentRect: .zero, styleMask: .titled, backing: .buffered, defer: false)
        sheet.isReleasedWhenClosed = false
        window.beginSheet(sheet)
        defer { window.endSheet(sheet); sheet.close() }
        XCTAssertTrue(window.attachedSheet === sheet)
        XCTAssertFalse(presenter.shortcuts.perform(event("n", window: window)))
        XCTAssertEqual(presses, 0)
    }
}
final class MacToolbarTests: XCTestCase {
    private func fixture() -> (Presenter, NSWindow, NodeView, NodeView, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 480),
                         styleMask: [.titled, .closable], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false; w.title = "Original"; w.contentView = p.root
        let bar = NodeView(id: 1, kind: "view", presenter: p)
        bar.props = ["accessibilityRole": "toolbar", "toolbarPlacement": "window"]
        let heading = NodeView(id: 2, kind: "text", presenter: p)
        heading.props = ["text": "Home", "accessibilityRole": "heading"]
        let action = NodeView(id: 3, kind: "button", presenter: p)
        action.props = ["accessibilityLabel": "Compose", "accessibilityKeyShortcuts": "Meta+n"]
        action.handlers = ["press"]
        p.root.addSubview(bar); bar.addSubview(heading); bar.addSubview(action)
        for n in [bar, heading, action] { p.views[n.id] = n }
        return (p, w, bar, heading, action)
    }

    /// The head's title (LLP 1048.003 D1) is the window's own title: held
    /// until the app attaches its window, shown without a toolbar, under a
    /// projected toolbar heading, and the app's again when no head sets one.
    func testTheHeadTitleIsTheWindowsOwnTitle() {
        let (p, w, bar, _, _) = fixture()
        defer { p.toolbar.detach(); w.close() }
        bar.props.removeValue(forKey: "toolbarPlacement")
        p.headTitle("Before attach")
        XCTAssertEqual(w.title, "Original", "an embedded view never claims the window")
        XCTAssertTrue(p.toolbar.attach(to: w))
        XCTAssertEqual(w.title, "Before attach")
        p.headTitle("A question")
        XCTAssertEqual(w.title, "A question")
        bar.props["toolbarPlacement"] = "window"; p.toolbar.sync()
        XCTAssertEqual(w.title, "Home", "the toolbar's heading shows over the head's title")
        p.headTitle("Another question")
        XCTAssertEqual(w.title, "Home")
        bar.props.removeValue(forKey: "toolbarPlacement"); p.toolbar.sync()
        XCTAssertEqual(w.title, "Another question")
        p.headTitle(nil)
        XCTAssertEqual(w.title, "Original")
        // `head edited` (LLP 1069.010 D6): the window's edited mark.
        p.headEdited(true)
        XCTAssertTrue(w.isDocumentEdited)
        p.headEdited(false)
        XCTAssertFalse(w.isDocumentEdited)
    }

    func testRequiresBothExplicitDeclarationAndWindowOwnerAttachment() {
        let (p, w, bar, _, _) = fixture()
        defer { p.toolbar.detach(); w.close() }
        p.toolbar.sync()
        XCTAssertNil(w.toolbar); XCTAssertFalse(bar.isHidden)
        bar.props.removeValue(forKey: "toolbarPlacement")
        XCTAssertTrue(p.toolbar.attach(to: w))
        XCTAssertNil(w.toolbar); XCTAssertFalse(bar.isHidden)
        bar.props["toolbarPlacement"] = "window"
        p.toolbar.sync()
        XCTAssertTrue(w.toolbar === p.toolbar.toolbar); XCTAssertTrue(bar.isHidden)
        XCTAssertTrue(bar.isAccessibilityHidden())
        p.toolbar.detach()
        XCTAssertFalse(bar.isHidden); XCTAssertFalse(bar.isAccessibilityHidden())
    }

    func testStableNativeObjectsAndOneActionForToolbarAndShortcut() {
        let (p, w, _, heading, action) = fixture()
        defer { p.toolbar.detach(); w.close() }
        let icon = NodeView(id: 5, kind: "image", presenter: p)
        icon.props["symbolName"] = "square.and.pencil"
        action.props["toolbarPlacement"] = "navigation"
        action.addSubview(icon); p.views[icon.id] = icon
        XCTAssertTrue(p.toolbar.attach(to: w))
        let toolbar = w.toolbar, item = p.toolbar.items[action.id]
        let image = item?.image
        XCTAssertEqual(item?.isNavigational, true)
        XCTAssertEqual(w.title, "Home"); XCTAssertNotNil(item); XCTAssertNil(item?.view)
        var presses: [UInt32] = []; p.onPress = { presses.append($0) }
        XCTAssertEqual(p.toolbar.activate(action), true)
        let key = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command,
                                  timestamp: 0, windowNumber: w.windowNumber, context: nil,
                                  characters: "n", charactersIgnoringModifiers: "n", isARepeat: false, keyCode: 45)!
        XCTAssertTrue(p.shortcuts.perform(key))
        XCTAssertEqual(presses, [action.id, action.id])
        p.touched(heading.id, textChanged: true)
        heading.applyProps(set: ["text": "Profile"], clear: [])
        action.props["accessibilityLabel"] = "New Prompt"
        p.toolbar.sync()
        XCTAssertTrue(w.toolbar === toolbar); XCTAssertTrue(p.toolbar.items[action.id] === item)
        XCTAssertTrue(item?.image === image)
        XCTAssertEqual(w.title, "Profile"); XCTAssertEqual(item?.label, "New Prompt")
        XCTAssertEqual(p.toolbar.observation(action)?["geometry"] as? String, "system-owned")
    }

    func testDisabledInertHiddenRemovedAndStaleItemsCannotActivate() {
        let (p, w, bar, _, action) = fixture()
        defer { p.toolbar.detach(); w.close() }
        p.toolbar.attach(to: w)
        let stale = p.toolbar.items[action.id]!
        var presses = 0; p.onPress = { _ in presses += 1 }
        action.props["disabled"] = "true"; p.toolbar.sync()
        XCTAssertFalse(stale.isEnabled); XCTAssertEqual(p.toolbar.activate(action), false)
        action.props["disabled"] = "false"; bar.props["inert"] = "true"; p.toolbar.sync()
        XCTAssertFalse(p.toolbar.validateToolbarItem(stale)); XCTAssertEqual(p.toolbar.activate(action), false)
        bar.props["inert"] = "false"; action.isHidden = true; p.toolbar.sync()
        XCTAssertNil(w.toolbar); XCTAssertFalse(bar.isHidden)
        XCTAssertFalse(p.toolbar.validateToolbarItem(stale))
        action.isHidden = false; p.toolbar.sync()
        p.toolbar.reset()
        XCTAssertNil(w.toolbar); XCTAssertEqual(w.title, "Original")
        XCTAssertFalse(p.toolbar.validateToolbarItem(stale)); XCTAssertEqual(presses, 0)
    }

    func testCannotReplaceExistingOrSubsequentlyInstalledHostToolbar() {
        let (p, w, bar, _, _) = fixture()
        defer { p.toolbar.detach(); w.close() }
        let host = NSToolbar(identifier: "host")
        w.toolbar = host
        XCTAssertFalse(p.toolbar.attach(to: w)); XCTAssertFalse(bar.isHidden)
        XCTAssertTrue(w.toolbar === host)
        w.toolbar = nil
        XCTAssertTrue(p.toolbar.attach(to: w))
        w.toolbar = host
        p.toolbar.sync(); p.toolbar.detach()
        XCTAssertTrue(w.toolbar === host); XCTAssertFalse(bar.isHidden)
    }

    func testAmbiguousDeclarationsAndUnmountRestoreAuthoredRendering() {
        let (p, w, bar, _, _) = fixture()
        defer { p.toolbar.detach(); w.close() }
        p.toolbar.attach(to: w)
        let other = NodeView(id: 4, kind: "view", presenter: p)
        other.props = bar.props; p.views[other.id] = other; p.root.addSubview(other)
        p.toolbar.sync()
        XCTAssertNil(w.toolbar); XCTAssertFalse(bar.isHidden)
        other.removeFromSuperview(); p.views.removeValue(forKey: other.id)
        p.toolbar.sync(); XCTAssertNotNil(w.toolbar)
        p.root.removeFromSuperview(); p.toolbar.sync()
        XCTAssertNil(w.toolbar); XCTAssertFalse(bar.isHidden)
    }

    func testEveryToolbarActionHasAMenuItemEvenWithoutShortcut() {
        let (p, w, _, _, action) = fixture()
        defer { p.toolbar.detach(); w.close() }
        action.props.removeValue(forKey: "accessibilityKeyShortcuts")
        p.toolbar.attach(to: w)
        let menu = ShortcutMenu(title: "File")
        p.shortcuts.attach(menu)
        XCTAssertEqual(menu.items.first?.title, "Compose")
        XCTAssertEqual(menu.items.first?.keyEquivalent, "")
        let item = menu.items.first
        p.toolbar.sync(); p.shortcuts.sync()
        XCTAssertTrue(menu.items.first === item)
    }

    func testCSSDisplayNoneAndProjectedDescendants() {
        let (p, w, bar, _, action) = fixture()
        defer { p.toolbar.detach(); w.close() }
        let icon = NodeView(id: 5, kind: "image", presenter: p)
        action.addSubview(icon); p.views[icon.id] = icon
        p.toolbar.attach(to: w)
        XCTAssertTrue(p.toolbar.suppresses(action)); XCTAssertTrue(p.toolbar.suppresses(icon))
        action.applyStyle(["display": "none"]); p.toolbar.sync()
        XCTAssertNil(w.toolbar); XCTAssertFalse(p.toolbar.visible(action))
        action.applyStyle([:]); p.toolbar.sync(); XCTAssertNotNil(w.toolbar)
        bar.applyStyle(["display": "none"]); p.toolbar.sync()
        XCTAssertNil(w.toolbar); XCTAssertFalse(p.toolbar.visible(action))
        bar.applyStyle([:]); p.toolbar.sync(); XCTAssertNotNil(w.toolbar)
        p.toolbar.detach()
        XCTAssertFalse(p.toolbar.suppresses(icon)); XCTAssertFalse(bar.isAccessibilityHidden())
    }

    func testNativeSheetBlocksToolbarActivation() {
        let (p, w, _, _, action) = fixture()
        defer { p.toolbar.detach(); w.close() }
        p.toolbar.attach(to: w)
        let sheet = NSWindow(contentRect: .zero, styleMask: .titled, backing: .buffered, defer: false)
        sheet.isReleasedWhenClosed = false
        w.beginSheet(sheet)
        defer { w.endSheet(sheet); sheet.close() }
        XCTAssertEqual(p.toolbar.activate(action), false)
        XCTAssertFalse(p.toolbar.validateToolbarItem(p.toolbar.items[action.id]!))
    }

    func testWindowGeometryWaitsForTheOutermostBatchAndCoalesces() {
        let p = Presenter()
        var events: [String] = []
        p.onKey = { _, _ in events.append("input") }
        p.onViewportFit = {
            events.append("chrome")
            XCTAssertTrue(p.deferGeometry { events.append("obsolete") })
            XCTAssertTrue(p.deferGeometry {
                events.append("geometry")
                XCTAssertFalse(p.deferGeometry { events.append("unexpected") })
            })
            p.key(1, "x")
            XCTAssertEqual(events, ["chrome"])
        }
        p.apply(batchFixture(ops: [
            ["op": "create", "id": 1, "kind": "view", "props": ["viewportFit": "cover"]],
            ["op": "roots", "ids": [1]]
        ], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertEqual(events, ["chrome", "geometry", "input"])
    }

    func testResetDoesNotApplyGeometryAheadOfTheIncomingBootBatch() {
        let (p, w, _, _, _) = fixture()
        defer { p.toolbar.detach(); w.close() }
        p.toolbar.attach(to: w)
        var geometryApplied = false
        p.toolbar.onChange = {
            XCTAssertTrue(p.deferGeometry { geometryApplied = true })
        }
        p.reset()
        XCTAssertFalse(geometryApplied)
        p.apply(batchFixture(ops: [], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertTrue(geometryApplied)
    }
}
#endif

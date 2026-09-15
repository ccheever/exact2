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

    private func event(_ key: String, window: NSWindow, repeat repeating: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command,
                         timestamp: 0, windowNumber: window.windowNumber, context: nil,
                         characters: key, charactersIgnoringModifiers: key, isARepeat: repeating, keyCode: 0)!
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
#endif

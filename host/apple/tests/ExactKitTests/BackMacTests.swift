// @ref LLP 1115.000 D5.1 — Back on the Mac: Go ▸ Back ⌘[, the toolbar's
// chevron (kept, disabled, at a stack's root) and the swipe, each going the
// way iOS's Back goes.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class BackMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    /// A navigation root (1) whose stack is home (2), then `routes` in
    /// order from id 3, each `(key, presentation)`; the root names
    /// `selected`. Every route holds a text; `control` puts an authored
    /// Back control (id 20) in the last route.
    private func fixture(routes: [(String, String?)] = [("b", nil)], selected: String, control: Bool = false, toolbar: Bool = false) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.title = "Original"
        window.contentView = p.viewport
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationBack": "back", "navigationKey": selected]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "create", "id": 2, "kind": "view", "props": ["navigationKey": "a"]],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 400, "h": 300],
        ]
        var stack = [2]
        for (index, route) in routes.enumerated() {
            let id = 3 + index
            var props = ["navigationKey": route.0]
            if let presentation = route.1 { props["navigationPresentation"] = presentation }
            ops += [["op": "create", "id": id, "kind": "view", "props": props], ["op": "frame", "id": id, "x": 0, "y": 0, "w": 400, "h": 300]]
            stack.append(id)
        }
        if control {
            ops += [["op": "create", "id": 20, "kind": "button", "props": ["id": "back"], "handlers": ["press"]],
                    ["op": "frame", "id": 20, "x": 0, "y": 0, "w": 60, "h": 30],
                    ["op": "children", "id": stack.last!, "ids": [20]]]
        }
        var children = stack
        if toolbar {
            ops += [["op": "create", "id": 30, "kind": "view", "props": ["accessibilityRole": "toolbar", "toolbarPlacement": "window"]],
                    ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 400, "h": 30],
                    ["op": "create", "id": 31, "kind": "button", "props": ["accessibilityLabel": "Compose"], "handlers": ["press"]],
                    ["op": "frame", "id": 31, "x": 0, "y": 0, "w": 60, "h": 30],
                    ["op": "children", "id": 30, "ids": [31]]]
            children.append(30)
        }
        ops += [["op": "children", "id": 1, "ids": children], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        return p
    }
    private func select(_ key: String, in p: Presenter) {
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["navigationKey": key]]]))
    }

    func testAPushedRouteGoesBackByItsAuthoredControlAndARootOrAModalRouteDoesNot() {
        let p = fixture(selected: "b", control: true)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertTrue(p.navigation.pushedRoute?.route === p.views[3])
        XCTAssertTrue(p.navigation.backOffered)
        XCTAssertEqual(p.navigation.back, .press(20), "an authored Back control is what the platform's Back presses")
        XCTAssertTrue(p.navigation.canGoBack)
        XCTAssertTrue(p.navigation.goBack())
        XCTAssertEqual(pressed, [20])
        // The author's say: a disabled control keeps the way offered and refuses it.
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["disabled": "true"]]]))
        XCTAssertTrue(p.navigation.backOffered)
        XCTAssertNil(p.navigation.back)
        XCTAssertFalse(p.navigation.goBack())
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["disabled": "false"]]]))
        // The stack's root has nowhere to go back to, whatever it holds.
        select("a", in: p)
        XCTAssertNil(p.navigation.pushedRoute)
        XCTAssertFalse(p.navigation.backOffered)
        XCTAssertFalse(p.navigation.goBack())
        XCTAssertEqual(pressed, [20])
        window.close()
        // A sheet or modal route is not pushed: its close is its own. A
        // route pushed inside it is.
        let modal = fixture(routes: [("b", "modal")], selected: "b", control: true)
        XCTAssertNil(modal.navigation.pushedRoute)
        XCTAssertFalse(modal.navigation.backOffered)
        XCTAssertFalse(modal.navigation.canGoBack)
        window.close()
        let inside = fixture(routes: [("b", "modal"), ("c", nil)], selected: "c", control: true)
        XCTAssertTrue(inside.navigation.pushedRoute?.route === inside.views[4])
        XCTAssertEqual(inside.navigation.back, .press(20))
    }

    func testARouteThatIsNoVisitAndDeclaresNoControlOffersNoBackAndADialogHoldsIt() {
        // No router beneath it (a key that is no visit's id) and no control: nothing to offer.
        let p = fixture(selected: "b")
        XCTAssertNotNil(p.navigation.pushedRoute)
        XCTAssertFalse(p.navigation.backOffered)
        XCTAssertNil(p.navigation.back)
        window.close()
        // A modal dialog over the page holds Back, as it holds the page.
        let held = fixture(selected: "b", control: true)
        held.apply(wireBatch([
            ["op": "create", "id": 40, "kind": "view", "props": ["semanticTag": "dialog", "id": "ask"]],
            ["op": "frame", "id": 40, "x": 0, "y": 0, "w": 200, "h": 100],
            ["op": "children", "id": 1, "ids": [2, 3, 40]],
        ]))
        XCTAssertTrue(held.navigation.canGoBack)
        held.dialogs.show(held.views[40]!)
        XCTAssertFalse(held.navigation.canGoBack)
        XCTAssertFalse(held.navigation.goBack())
        held.dialogs.close(held.views[40]!)
        XCTAssertTrue(held.navigation.canGoBack)
    }

    /// The title bar's height: the window's frame less its content's.
    private var bar: CGFloat { window.frame.height - window.contentLayoutRect.height }

    /// With no toolbar of the author's, the window of an app with a stack has
    /// one that holds only Back, disabled at the stack's root as Finder's and
    /// Safari's is, so the title bar is the same height on every route (it
    /// came and went with each push and pop, 8 points each time). The
    /// route's header and the window's title stay as they are. An app with
    /// no stack has no toolbar.
    func testTheWindowOfAnAppWithAStackKeepsItsBackItemDisabledAtTheRoot() throws {
        let p = fixture(selected: "a", control: true)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let bare = bar, content = try XCTUnwrap(window.contentView).frame.size
        XCTAssertTrue(p.toolbar.attach(to: window))
        let toolbar = try XCTUnwrap(window.toolbar, "the toolbar is there at the root")
        XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier])
        let back = try XCTUnwrap(toolbar.items.first)
        XCTAssertTrue(back === p.toolbar.backItem)
        XCTAssertTrue(back.isNavigational)
        XCTAssertNotNil(back.image)
        XCTAssertFalse(back.isEnabled, "nowhere to go back to: disabled, not gone")
        XCTAssertFalse(p.toolbar.validateToolbarItem(back))
        _ = NSApp.sendAction(try XCTUnwrap(back.action), to: back.target, from: back)
        XCTAssertTrue(pressed.isEmpty, "a disabled Back presses nothing")
        let atRoot = bar
        XCTAssertGreaterThan(atRoot, bare, "a toolbar makes the title bar taller: the height that must not come and go")
        XCTAssertEqual(window.contentView?.frame.size, content, "the content keeps its size: AppKit alone made it 32 points taller")
        select("b", in: p)
        XCTAssertTrue(window.toolbar === toolbar, "the same toolbar on the pushed route")
        XCTAssertTrue(toolbar.items.first === back)
        XCTAssertEqual(bar, atRoot, "the title bar is the same height on a pushed route")
        XCTAssertTrue(back.isEnabled)
        XCTAssertEqual(window.title, "Original", "the route's title is not projected")
        XCTAssertFalse(try XCTUnwrap(p.views[3]).isHidden)
        XCTAssertTrue(p.toolbar.validateToolbarItem(back))
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(back.action), to: back.target, from: back))
        XCTAssertEqual(pressed, [20])
        // A batch that changes nothing of the stack keeps the same toolbar and item.
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["accessibilityLabel": "Back"]]]))
        XCTAssertTrue(window.toolbar === toolbar)
        XCTAssertTrue(window.toolbar?.items.first === back)
        // A sheet on the window holds it.
        let sheet = NSWindow(contentRect: .zero, styleMask: .titled, backing: .buffered, defer: false)
        sheet.isReleasedWhenClosed = false
        window.beginSheet(sheet)
        XCTAssertFalse(p.toolbar.validateToolbarItem(back))
        window.endSheet(sheet); sheet.close()
        select("a", in: p)
        XCTAssertTrue(window.toolbar === toolbar, "back at the root the toolbar stays")
        XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier])
        XCTAssertEqual(bar, atRoot)
        XCTAssertFalse(back.isEnabled)
        XCTAssertEqual(window.title, "Original")
        XCTAssertFalse(p.toolbar.validateToolbarItem(back))
        // No navigation root: no stack, and no toolbar.
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["navigationBack"]]]))
        XCTAssertNil(window.toolbar, "an app with no stack has no toolbar")
        XCTAssertFalse(p.toolbar.validateToolbarItem(back), "AppKit can still validate the retired Back item")
        back.tag = Int.max
        XCTAssertFalse(p.toolbar.validateToolbarItem(back), "an unrepresentable tag is not a node ID")
        XCTAssertEqual(bar, bare)
        XCTAssertEqual(window.contentView?.frame.size, content, "and keeps it as the toolbar goes")
        p.toolbar.detach()
    }

    /// Where the author declared a window toolbar, Back leads it, enabled
    /// while a route is pushed and disabled at the root, so the author's
    /// items do not move with each push and pop.
    func testBackLeadsTheAuthorsToolbarAndStaysAtTheRoot() throws {
        let p = fixture(selected: "b", control: true, toolbar: true)
        XCTAssertTrue(p.toolbar.attach(to: window))
        let toolbar = try XCTUnwrap(window.toolbar)
        let compose = try XCTUnwrap(p.toolbar.items[31])
        XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier, compose.itemIdentifier])
        XCTAssertTrue(p.toolbar.backItem.isEnabled)
        XCTAssertTrue(try XCTUnwrap(p.views[30]).isHidden, "the declared toolbar is still projected")
        select("a", in: p)
        XCTAssertTrue(window.toolbar === toolbar, "the author's toolbar stays")
        XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier, compose.itemIdentifier], "Back keeps its place at the root")
        XCTAssertFalse(p.toolbar.backItem.isEnabled)
        XCTAssertTrue(toolbar.items.last === compose)
        select("b", in: p)
        XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier, compose.itemIdentifier])
        XCTAssertTrue(p.toolbar.backItem.isEnabled)
        // With no stack the author's toolbar is theirs alone.
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["navigationBack"]]]))
        XCTAssertTrue(window.toolbar === toolbar)
        XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [compose.itemIdentifier])
        p.toolbar.detach()
    }

    /// An app whose every mounted route asks for no bar has no toolbar. One
    /// comes with the first route that shows a bar and then stays, through
    /// routes that ask for none: the title bar's height changes that once.
    func testAnAppWhoseRoutesAllAskForNoBarHasNoToolbarUntilOneShowsABar() throws {
        let p = fixture(selected: "a", control: true)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["navigationBar": "none"]],
                          ["op": "props", "id": 3, "set": ["navigationBar": "none"]]]))
        XCTAssertFalse(p.navigation.barRoute)
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertNil(window.toolbar, "no route shows a bar: nothing to hold")
        select("b", in: p)
        XCTAssertNil(window.toolbar)
        XCTAssertTrue(p.navigation.canGoBack, "the way back is not the bar's")
        // A route that shows a bar, mounted but not selected: the toolbar
        // comes now, before the route does, and the selected one has no item.
        p.apply(wireBatch([["op": "props", "id": 2, "set": [:], "clear": ["navigationBar"]]]))
        let toolbar = try XCTUnwrap(window.toolbar)
        XCTAssertTrue(toolbar.items.isEmpty, "the selected route asked for no bar")
        let held = bar
        select("a", in: p)
        XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier])
        XCTAssertEqual(bar, held)
        // Every route asks for none again: the toolbar it has stays.
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["navigationBar": "none"]]]))
        XCTAssertFalse(p.navigation.barRoute)
        XCTAssertTrue(window.toolbar === toolbar, "once there it stays")
        XCTAssertTrue(toolbar.items.isEmpty)
        XCTAssertEqual(bar, held)
        p.toolbar.detach()
    }

    /// A route that asks for a screen with no bar (`navigationBar="none"`,
    /// LLP 1115.000 D5.2; the prop set by hand here) gets no chevron, and
    /// the toolbar the app's other routes have stays, empty, so the title bar
    /// keeps its height. The author hid the bar, not the way back: the
    /// menu's Back and the swipe still go.
    func testARouteThatAsksForNoBarHasNoBackItemButStillGoesBack() throws {
        let p = fixture(selected: "b", control: true)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertTrue(p.toolbar.attach(to: window))
        let toolbar = try XCTUnwrap(window.toolbar)
        XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier])
        let withBack = bar
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["navigationBar": "none"]]]))
        XCTAssertTrue(window.toolbar === toolbar, "the toolbar stays for the routes that have a bar")
        XCTAssertTrue(toolbar.items.isEmpty, "no Back item on a route that asked for no bar")
        XCTAssertEqual(bar, withBack, "and the title bar keeps its height")
        XCTAssertFalse(p.navigation.backInToolbar)
        XCTAssertTrue(p.navigation.backOffered)
        XCTAssertTrue(p.navigation.canGoBack, "Go ▸ Back and ⌘[ are enabled by this")
        XCTAssertTrue(p.navigation.goBack())
        XCTAssertTrue(p.backSwipe.swiped(deltaX: 1))
        var handler: ((CGFloat, NSEvent.Phase, Bool) -> Void)?
        p.backSwipe.track = { _, tracked in handler = tracked }
        let cg = try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: 0, wheel2: 14, wheel3: 0))
        cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
        cg.setIntegerValueField(.scrollWheelEventScrollPhase, value: 1)
        XCTAssertTrue(p.backSwipe.scroll(try XCTUnwrap(NSEvent(cgEvent: cg)), enabled: true))
        try XCTUnwrap(handler)(1, .ended, true)
        XCTAssertEqual(pressed, [20, 20, 20])
        // `auto`, or the prop gone: the chevron is back.
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["navigationBar": "auto"]]]))
        XCTAssertEqual(window.toolbar?.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier])
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["navigationBar": "none"]]]))
        XCTAssertEqual(window.toolbar?.items.count, 0)
        p.apply(wireBatch([["op": "props", "id": 3, "set": [:], "clear": ["navigationBar"]]]))
        XCTAssertEqual(window.toolbar?.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier])
        XCTAssertTrue(window.toolbar === toolbar)
        XCTAssertEqual(bar, withBack)
        p.toolbar.detach()
        window.close()
        // In the author's own toolbar: it stays, without Back.
        let q = fixture(selected: "b", control: true, toolbar: true)
        XCTAssertTrue(q.toolbar.attach(to: window))
        let compose = try XCTUnwrap(q.toolbar.items[31])
        XCTAssertEqual(window.toolbar?.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier, compose.itemIdentifier])
        q.apply(wireBatch([["op": "props", "id": 3, "set": ["navigationBar": "none"]]]))
        XCTAssertEqual(window.toolbar?.items.map(\.itemIdentifier), [compose.itemIdentifier])
        XCTAssertTrue(q.navigation.canGoBack)
        q.toolbar.detach()
    }

    /// Go ▸ Back ⌘[ leads the Go menu of an app with a navigation root; an
    /// app with none has no Go menu, and the app's own ⌘[ stands in for it.
    func testGoBackIsInTheMenuBarOfAnAppThatNavigates() throws {
        let previous = (NSApp.servicesMenu, NSApp.windowsMenu, NSApp.helpMenu)
        defer { NSApp.servicesMenu = previous.0; NSApp.windowsMenu = previous.1; NSApp.helpMenu = previous.2 }
        let p = fixture(selected: "b", control: true)
        let bar = DevMenu.makeMenu(shortcuts: p.shortcuts, documents: false, develop: false)
        let item = try XCTUnwrap(bar.items.first { $0.submenu?.title == "Go" })
        let go = try XCTUnwrap(item.submenu)
        let back = try XCTUnwrap(go.items.first)
        XCTAssertEqual(back.title, "Back")
        XCTAssertEqual(back.keyEquivalent, "[")
        XCTAssertEqual(back.keyEquivalentModifierMask, .command)
        XCTAssertTrue(back.target is GoMenuTarget)
        XCTAssertFalse(item.isHidden)
        XCTAssertFalse(back.isHidden)
        // The app's own Back command with ⌘[ stands in for the host's, and gives it back.
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["accessibilityKeyShortcuts": "Meta+[", "accessibilityLabel": "Back to list"]]]))
        XCTAssertTrue(back.isHidden)
        XCTAssertEqual(go.items.filter { !$0.isHidden }.map(\.title), ["Back to list"])
        XCTAssertEqual(go.items.filter { $0.keyEquivalent == "[" }.count, 1, "⌘[ is in the bar once")
        p.apply(wireBatch([["op": "props", "id": 20, "set": [:], "clear": ["accessibilityKeyShortcuts"]]]))
        XCTAssertFalse(back.isHidden)
        XCTAssertEqual(back.keyEquivalent, "[")
        // `-exact-menu-remove="back"` takes Back and its ⌘[ (LLP 1021 §5.3.1), and Go with it.
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["menuRemove": "back"]]]))
        XCTAssertTrue(p.navigation.canGoBack)
        XCTAssertTrue(back.isHidden)
        XCTAssertTrue(item.isHidden)
        XCTAssertFalse(bar.performKeyEquivalent(with: NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: "[", charactersIgnoringModifiers: "[", isARepeat: false, keyCode: 33)!))
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["menuRemove"]]]))
        XCTAssertFalse(back.isHidden)
        XCTAssertFalse(item.isHidden)
        // No navigation root: no Back, and so no Go menu.
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["navigationBack"]]]))
        XCTAssertTrue(back.isHidden)
        XCTAssertTrue(item.isHidden)
    }

    /// The two-finger swipe begins only for a trackpad's sideways gesture
    /// toward the page before, with swipe tracking on and a way back; Back
    /// lands once, when AppKit says the gesture ended where it set out for.
    func testTheSwipeGoesBackWhenAppKitsTrackingEndsOnTheSideItSetOutFor() throws {
        XCTAssertTrue(BackSwipe.begins(gesture: true, dx: 12, dy: 3, enabled: true, available: true))
        XCTAssertFalse(BackSwipe.begins(gesture: true, dx: -12, dy: 3, enabled: true, available: true), "toward the page after: there is none")
        XCTAssertFalse(BackSwipe.begins(gesture: true, dx: 5, dy: 9, enabled: true, available: true), "a vertical scroll")
        XCTAssertFalse(BackSwipe.begins(gesture: false, dx: 12, dy: 0, enabled: true, available: true), "a mouse wheel, or momentum")
        XCTAssertFalse(BackSwipe.begins(gesture: true, dx: 12, dy: 0, enabled: false, available: true), "Swipe between pages is off")
        XCTAssertFalse(BackSwipe.begins(gesture: true, dx: 12, dy: 0, enabled: true, available: false), "nowhere to go back to")

        let p = fixture(selected: "b", control: true)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        var handler: ((CGFloat, NSEvent.Phase, Bool) -> Void)?
        p.backSwipe.track = { _, tracked in handler = tracked }
        func gesture(dx: Int32, dy: Int32, phase: Int64) throws -> NSEvent {
            let cg = try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0))
            cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
            cg.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
            return try XCTUnwrap(NSEvent(cgEvent: cg))
        }
        let began = try gesture(dx: 14, dy: 2, phase: 1)
        XCTAssertEqual(began.phase, .began)
        XCTAssertGreaterThan(began.scrollingDeltaX, 0)
        XCTAssertFalse(p.backSwipe.scroll(try gesture(dx: 2, dy: 14, phase: 1), enabled: true), "a vertical gesture is the scroller's")
        XCTAssertFalse(p.backSwipe.scroll(began, enabled: false))
        XCTAssertNil(handler)
        XCTAssertTrue(p.backSwipe.scroll(began, enabled: true))
        XCTAssertTrue(p.backSwipe.tracking)
        XCTAssertFalse(p.backSwipe.scroll(began, enabled: true), "one gesture at a time")
        let tracked = try XCTUnwrap(handler)
        tracked(0.2, .changed, false)
        tracked(0.6, .changed, false)
        XCTAssertTrue(pressed.isEmpty, "nothing until the fingers lift")
        tracked(0.7, .ended, false)
        XCTAssertEqual(pressed, [20])
        tracked(1, [], true)
        XCTAssertEqual(pressed, [20], "once")
        XCTAssertFalse(p.backSwipe.tracking)
        // Cancelled, or ended back past where it started: nowhere.
        for end in [(CGFloat(0.1), NSEvent.Phase.cancelled), (CGFloat(-0.8), NSEvent.Phase.ended)] {
            handler = nil
            XCTAssertTrue(p.backSwipe.scroll(began, enabled: true))
            let again = try XCTUnwrap(handler)
            again(0.3, .changed, false)
            again(end.0, end.1, false)
            again(0, [], true)
            XCTAssertEqual(pressed, [20])
        }
        // The three-finger swipe, where that is the setting.
        XCTAssertFalse(p.backSwipe.swiped(deltaX: -1))
        XCTAssertTrue(p.backSwipe.swiped(deltaX: 1))
        XCTAssertEqual(pressed, [20, 20])
        select("a", in: p)
        XCTAssertFalse(p.backSwipe.swiped(deltaX: 1), "nowhere to go back to")
        XCTAssertFalse(p.backSwipe.scroll(began, enabled: true))
    }

    /// While the bar tracks, a batch's removal of Back is written on the
    /// run loop's next turn (ShortcutsMac.swift), its ⌘[ out of the bar at
    /// once. Each sync sets Back's state through the removal, so nothing
    /// shows it again: Back is written once, not on every turn.
    func testARemovalWhileTheBarTracksWritesBackOnceOnTheNextTurn() throws {
        let previous = (NSApp.servicesMenu, NSApp.windowsMenu, NSApp.helpMenu)
        defer { NSApp.servicesMenu = previous.0; NSApp.windowsMenu = previous.1; NSApp.helpMenu = previous.2 }
        let p = fixture(selected: "b", control: true)
        let bar = DevMenu.makeMenu(shortcuts: p.shortcuts, documents: false, develop: false)
        let back = try XCTUnwrap(bar.items.first { $0.submenu?.title == "Go" }?.submenu?.items.first)
        p.shortcuts.sync()
        XCTAssertFalse(back.isHidden)
        var writes = 0
        let observation = back.observe(\.isHidden) { _, _ in writes += 1 }
        defer { observation.invalidate() }
        let center = NotificationCenter.default
        center.post(name: NSMenu.didBeginTrackingNotification, object: bar)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["menuRemove": "back"]]]))
        XCTAssertFalse(back.isHidden, "not written inside the batch")
        let command = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: window.windowNumber,
                                       context: nil, characters: "[", charactersIgnoringModifiers: "[", isARepeat: false, keyCode: 33)!
        XCTAssertFalse(bar.performKeyEquivalent(with: command), "⌘[ leaves with the removal")
        let turns = { (seconds: TimeInterval) in
            let end = Date(timeIntervalSinceNow: seconds)
            while Date() < end { RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        }
        turns(0.2)
        XCTAssertTrue(back.isHidden)
        // Batches go on arriving while the bar tracks.
        for _ in 0..<3 { p.shortcuts.sync(); turns(0.1) }
        turns(0.5)
        XCTAssertTrue(back.isHidden)
        XCTAssertEqual(writes, 1, "Back written once")
        // Withdrawn while the bar tracks, Back shows on the next turn.
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["menuRemove"]]]))
        turns(0.2)
        XCTAssertFalse(back.isHidden)
        XCTAssertEqual(writes, 2)
        center.post(name: NSMenu.didEndTrackingNotification, object: bar)
    }
}

/// The same ways back over a real session: a route of the router that
/// declares no Back control goes back by the root's `navigate`, or by the
/// runner's own `back` where the root has no handler (LLP 1115 D5).
final class BackSessionMacTests: XCTestCase {
    private func plan(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    private func source(navigate: Bool) -> String {
        """
        routes nav
          home "/"
            detail "/detail/:id"

        component App
          action open
            nav = push(nav, "/detail/7")
          action follow(location: string)
            nav = go(nav, location)
          view
            main navigationKey=`${top(nav).id}` navigationBack="back"\(navigate ? " navigate=follow" : "") width="100%" height="100%"
              for e in stack(nav) key=e.id
                column navigationKey=`${e.id}` position="absolute" inset=0
                  text `${e.name} ${e.url}`
                  if e.name == "home"
                    button press=open testId="open"
                      text "Open"

        """
    }

    func testAVisitWithNoAuthoredControlGoesBackByNavigateOrTheRunnersBack() throws {
        _ = NSApplication.shared
        let previous = (NSApp.servicesMenu, NSApp.windowsMenu, NSApp.helpMenu, DevMenu.session)
        defer { NSApp.servicesMenu = previous.0; NSApp.windowsMenu = previous.1; NSApp.helpMenu = previous.2; DevMenu.session = previous.3 }
        for navigate in [true, false] {
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            XCTAssertNil(session.boot(plan: try plan(source(navigate: navigate)), size: CGSize(width: 400, height: 300)).error)
            let view = ExactView(session: session)
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled, .closable], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            defer { window.close() }
            window.contentView = view
            XCTAssertTrue(view.attachWindowToolbar(to: window))
            let p = session.presenter
            DevMenu.session = session
            let bar = DevMenu.makeMenu(shortcuts: p.shortcuts, documents: false, develop: false)
            let back = try XCTUnwrap(bar.items.first { $0.submenu?.title == "Go" }?.submenu?.items.first)
            let target = try XCTUnwrap(back.target as? GoMenuTarget)
            let shown = { p.views.values.filter { $0.props["navigationKey"] != nil && $0.props["navigationBack"] == nil && !$0.isHidden }.count }
            // At the root: Go ▸ Back and the toolbar's Back are there and disabled, no swipe.
            XCTAssertFalse(back.isHidden)
            XCTAssertFalse(target.validateMenuItem(back))
            let toolbar = try XCTUnwrap(window.toolbar)
            XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier])
            XCTAssertFalse(p.toolbar.backItem.isEnabled)
            XCTAssertFalse(view.wantsScrollEventsForSwipeTracking(on: .horizontal))
            let open = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "open" })
            p.press(open.id)
            let pushed = try XCTUnwrap(p.navigation.pushedRoute, "the detail route is pushed (navigate: \(navigate))")
            let key = try XCTUnwrap(pushed.route.props["navigationKey"])
            XCTAssertEqual(p.navigation.back, navigate ? .navigate("/") : .pop)
            XCTAssertTrue(target.validateMenuItem(back))
            XCTAssertTrue(window.toolbar === toolbar, "the same toolbar, pushed or not")
            XCTAssertEqual(toolbar.items.map(\.itemIdentifier), [WindowToolbarHost.backIdentifier])
            XCTAssertTrue(p.toolbar.backItem.isEnabled)
            XCTAssertTrue(view.wantsScrollEventsForSwipeTracking(on: .horizontal))
            XCTAssertFalse(view.wantsScrollEventsForSwipeTracking(on: .vertical))
            // By the menu's item (what ⌘[ sends).
            target.back(back)
            XCTAssertNil(p.navigation.pushedRoute, "back at the root (navigate: \(navigate))")
            XCTAssertNotEqual(pushed.root.props["navigationKey"], key)
            XCTAssertFalse(target.validateMenuItem(back))
            XCTAssertTrue(window.toolbar === toolbar)
            XCTAssertFalse(p.toolbar.backItem.isEnabled)
            XCTAssertEqual(shown(), 1)
            // By the toolbar's item.
            p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == "open" }).id)
            let item = try XCTUnwrap(window.toolbar?.items.first)
            XCTAssertTrue(item.isEnabled)
            XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(item.action), to: item.target, from: item))
            XCTAssertNil(p.navigation.pushedRoute)
            XCTAssertFalse(item.isEnabled)
            // By the three-finger swipe.
            p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == "open" }).id)
            XCTAssertNotNil(p.navigation.pushedRoute)
            XCTAssertTrue(p.backSwipe.swiped(deltaX: 1))
            XCTAssertNil(p.navigation.pushedRoute)
            // By the two-finger swipe: a trackpad's sideways gesture goes up
            // from the content through the page's scroll view to the session's
            // view, which hands it to AppKit's tracking (a stand-in here); a
            // vertical one stays the scroller's. AppKit passes nothing up
            // with "Swipe between pages" off, so there is nothing to see then.
            if NSEvent.isSwipeTrackingFromScrollEventsEnabled {
                p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == "open" }).id)
                XCTAssertNotNil(p.navigation.pushedRoute)
                var tracked = 0
                p.backSwipe.track = { _, handler in
                    tracked += 1
                    handler(0.4, .changed, false); handler(0.8, .ended, false); handler(1, [], true)
                }
                // The window server routes a scroll to a window that is on screen.
                window.orderFrontRegardless()
                let began: Int64 = 1, changed: Int64 = 2, ended: Int64 = 4, at = NSPoint(x: 200, y: 40)
                // The web's sign, as the agent's wheel takes it: a negative dx moves the content right.
                XCTAssertTrue(Agent.wheel([(began, 0, 20), (changed, 0, 40), (ended, 0, 0)], gesture: true, at: at, in: window, flags: []))
                XCTAssertEqual(tracked, 0, "a vertical gesture is no swipe")
                XCTAssertNotNil(p.navigation.pushedRoute)
                XCTAssertTrue(Agent.wheel([(began, -20, 0), (changed, -40, 0), (ended, 0, 0)], gesture: true, at: at, in: window, flags: []))
                XCTAssertEqual(tracked, 1)
                XCTAssertNil(p.navigation.pushedRoute, "the swipe went back (navigate: \(navigate))")
            }
            view.detachWindowToolbar()
        }
    }
}
#endif

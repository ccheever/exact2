// The Mac menu bar's own items (LLP 1021 §5.3): app-wide commands, the
// group under About, concealed commands, the host items `-exact-menu-remove`
// names, and View's zoom.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class MenuBarMacTests: XCTestCase {
    private final class MenuAction: NSObject {
        var calls = 0
        @objc func invoke(_ sender: Any?) { calls += 1 }
    }
    private var saved: (NSMenu?, NSMenu?, NSMenu?, NSMenu?)?
    /// Counts the writes to its `isHidden`, changed or not.
    private final class CountingItem: NSMenuItem {
        var writes = 0
        override var isHidden: Bool { didSet { writes += 1 } }
    }
    /// A menu item's own view, which runs `moved` as it enters the open
    /// menu's window: inside AppKit's layout of the menu's rows.
    private final class MenuLayoutProbe: NSView {
        var moved: () -> Void = {}
        var calls = 0
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard window != nil else { return }
            calls += 1
            moved()
        }
    }

    override func setUp() {
        _ = NSApplication.shared
        saved = (NSApp.mainMenu, NSApp.windowsMenu, NSApp.helpMenu, NSApp.servicesMenu)
    }
    override func tearDown() {
        guard let saved else { return }
        NSApp.mainMenu = saved.0; NSApp.windowsMenu = saved.1; NSApp.helpMenu = saved.2; NSApp.servicesMenu = saved.3
    }

    private func window(_ presenter: Presenter) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                              styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.root
        return window
    }

    @discardableResult
    private func button(_ id: UInt32, _ title: String, _ shortcut: String? = nil, menu: String? = nil, in presenter: Presenter, parent: NSView? = nil) -> NodeView {
        let node = NodeView(id: id, kind: "button", presenter: presenter)
        node.props = ["accessibilityLabel": title]
        if let shortcut { node.props["accessibilityKeyShortcuts"] = shortcut }
        if let menu { node.props["menuPlacement"] = menu }
        node.handlers = ["press"]
        presenter.views[id] = node
        (parent ?? presenter.root).addSubview(node)
        return node
    }

    private func key(_ characters: String, ignoring: String? = nil, _ modifiers: NSEvent.ModifierFlags = .command, code: UInt16 = 0, window: NSWindow) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: 0, windowNumber: window.windowNumber,
                         context: nil, characters: characters, charactersIgnoringModifiers: ignoring ?? characters, isARepeat: false, keyCode: code)!
    }

    private func item(_ menu: NSMenu, _ id: UInt32) -> NSMenuItem? {
        menu.items.first { ($0.representedObject as? NSNumber)?.uint32Value == id }
    }

    /// A command in the app menu or Help is the app's: enabled with no key
    /// window, under a sheet and under a modal dialog, as Settings… and
    /// Check for Updates… are in a hand-built Mac app. A File command stays
    /// the window's.
    func testAppWideCommandsStayEnabledOutsideTheKeyWindowAndUnderASheet() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let updates = button(1, "Check for Updates…", menu: "app", in: presenter)
        button(2, "Settings", "Meta+,", in: presenter)
        button(3, "Documentation", menu: "help", in: presenter)
        button(4, "Export", menu: "file", in: presenter)
        var presses: [UInt32] = []
        presenter.onPress = { presses.append($0) }
        let bar = DevMenu.makeMenu(shortcuts: presenter.shortcuts, documents: false, develop: false)
        let app = bar.items[0].submenu!, file = bar.items[1].submenu!, help = bar.items.last!.submenu!
        let items = [item(app, 1)!, item(app, 2)!, item(help, 3)!]
        let export = item(file, 4)!
        // XCTest has no key window: what a closed or ordered-out window leaves.
        XCTAssertNil(NSApp.keyWindow)
        for item in items { XCTAssertTrue(presenter.shortcuts.validateMenuItem(item), item.title) }
        XCTAssertFalse(presenter.shortcuts.validateMenuItem(export))
        let sheet = NSWindow(contentRect: .zero, styleMask: .titled, backing: .buffered, defer: false)
        sheet.isReleasedWhenClosed = false
        window.beginSheet(sheet)
        defer { window.endSheet(sheet); sheet.close() }
        presenter.shortcuts.sync()
        for item in items {
            XCTAssertTrue(item.isEnabled, item.title)
            XCTAssertTrue(presenter.shortcuts.validateMenuItem(item), item.title)
        }
        XCTAssertFalse(export.isEnabled)
        // Chosen, it presses its button.
        app.performActionForItem(at: app.index(of: items[0]))
        XCTAssertEqual(presses, [1])
        // `disabled` and an authored `inert` still disable it.
        updates.props["disabled"] = "true"
        XCTAssertFalse(presenter.shortcuts.validateMenuItem(items[0]))
        updates.props["disabled"] = nil
        updates.props["inert"] = "true"
        XCTAssertFalse(presenter.shortcuts.validateMenuItem(items[0]))
        app.performActionForItem(at: app.index(of: items[0]))
        XCTAssertEqual(presses, [1])
    }

    /// `-exact-menu="about"` sits directly under About, before its
    /// separator, as Check for Updates… does in apps that update themselves;
    /// Settings… and the `app` items follow the separator.
    func testAboutPlacementIsDirectlyUnderAbout() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let updates = button(1, "Check for Updates…", menu: "about", in: presenter)
        button(2, "Settings", "Meta+,", in: presenter)
        button(3, "Licenses", menu: "app", in: presenter)
        let bar = DevMenu.makeMenu(shortcuts: presenter.shortcuts, documents: false, develop: false)
        let app = bar.items[0].submenu!
        let listed = { app.items.prefix(8).map { $0.isSeparatorItem ? "—" : $0.title } }
        let about = "About \(ExactEnv.appName)"
        for _ in 0..<3 { presenter.shortcuts.sync() }
        XCTAssertEqual(listed(), [about, "Check for Updates…", "—", "Settings…", "Licenses", "—", "Services", "—"])
        // Moving between the groups keeps one of each separator.
        updates.props["menuPlacement"] = "app"
        presenter.shortcuts.sync()
        XCTAssertEqual(listed(), [about, "—", "Check for Updates…", "Settings…", "Licenses", "—", "Services", "—"])
        updates.props["menuPlacement"] = "about"
        presenter.shortcuts.sync()
        XCTAssertEqual(listed(), [about, "Check for Updates…", "—", "Settings…", "Licenses", "—", "Services", "—"])
    }

    /// `-exact-menu="none"`: the chord is a shortcut with no item to see. It
    /// takes the chord from a host item, which keeps its place, and leaves
    /// File and Edit as they look.
    func testConcealedCommandsKeepTheirChordsWithoutAnItem() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        button(1, "Toggle Diff", "Meta+d", menu: "none", in: presenter)
        button(2, "Undo Thread", "Meta+z", menu: "none", in: presenter)
        let bar = DevMenu.makeMenu(shortcuts: presenter.shortcuts, documents: false, develop: true)
        let menu = { (title: String) in bar.items.first { $0.submenu?.title == title }!.submenu! }
        for _ in 0..<2 { presenter.shortcuts.sync() }
        let concealed = menu("File").items.filter { $0.target is ShortcutHost }
        XCTAssertEqual(concealed.map(\.keyEquivalent), ["d", "z"])
        XCTAssertTrue(concealed.allSatisfy { $0.isHidden && $0.allowsKeyEquivalentWhenHidden })
        XCTAssertEqual(menu("File").items.filter { !$0.isHidden }.map(\.title), ["Close Window"])
        // The host's Undo keeps its place without ⌘Z; App Info… without ⌘D.
        let undo = menu("Edit").items.first { $0.action == NSSelectorFromString("undo:") }!
        XCTAssertFalse(undo.isHidden)
        XCTAssertEqual(undo.keyEquivalent, "")
        XCTAssertEqual(menu("Develop").items.first { $0.title == "App Info…" }?.keyEquivalent, "")
        // Shown, the same command stands in for Undo; concealed again, Undo is back.
        presenter.views[2]?.props["menuPlacement"] = nil
        presenter.shortcuts.sync()
        XCTAssertTrue(undo.isHidden)
        presenter.views[2]?.props["menuPlacement"] = "none"
        presenter.shortcuts.sync()
        XCTAssertFalse(undo.isHidden)
        // The bar takes its chord (XCTest has no key window, so the item,
        // a window's command, does not press: the app drive checks that).
        XCTAssertTrue(bar.performKeyEquivalent(with: key("d", code: 2, window: window)))
    }

    /// The host items `-exact-menu-remove` names beyond the window's
    /// (LLP 1021 §5.3), found by action or identifier, never by title.
    /// Develop leaves the bar and keeps its chords: Save Trace's ⌥⌘T.
    func testRemovalNamesTheHostsOwnItemsAndDevelopKeepsItsChords() throws {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let root = NodeView(id: 900, kind: "view", presenter: presenter)
        let ids = "develop back paste-and-match-style find spelling-and-grammar substitutions transformations app-help actual-size zoom-in zoom-out"
        root.props = ["menuRemove": ids]
        presenter.views[root.id] = root; presenter.root.addSubview(root)
        let bar = DevMenu.makeMenu(shortcuts: presenter.shortcuts, documents: false, develop: true)
        NSApp.mainMenu = bar
        let menu = { (title: String) in bar.items.first { $0.submenu?.title == title }!.submenu! }
        for title in ["Edit", "View", "Help", "Go"] { menu(title).title = "Traduit \(title)" }
        presenter.shortcuts.sync()
        let shown = { (menu: NSMenu) in menu.items.filter { !$0.isHidden && !$0.isSeparatorItem }.map(\.title) }
        let edit = bar.items[2].submenu!, view = bar.items[3].submenu!, help = NSApp.helpMenu!
        XCTAssertEqual(shown(edit), ["Undo", "Redo", "Cut", "Copy", "Paste", "Delete", "Select All", "Speech"])
        XCTAssertEqual(shown(view), ["Enter Full Screen"])
        XCTAssertFalse(view.items.first { !$0.isHidden }?.isSeparatorItem ?? true, "no separator leads View")
        XCTAssertEqual(shown(help), [])
        // A removed submenu's chords go with it: ⌘F finds nothing.
        XCTAssertFalse(bar.performKeyEquivalent(with: key("f", code: 3, window: window)))
        let develop = try XCTUnwrap(bar.items.first { $0.submenu?.title == "Develop" })
        XCTAssertTrue(develop.isHidden)
        let trace = try XCTUnwrap(develop.submenu?.items.first { $0.title == "Save Trace" })
        let target = MenuAction()
        trace.target = target; trace.action = #selector(MenuAction.invoke(_:))
        XCTAssertTrue(bar.performKeyEquivalent(with: key("†", ignoring: "t", [.command, .option], code: 17, window: window)))
        XCTAssertEqual(target.calls, 1, "Save Trace's chord outlives the menu")
        // Removing the declaration restores every item.
        root.props["menuRemove"] = ""
        presenter.shortcuts.sync()
        XCTAssertFalse(develop.isHidden)
        XCTAssertEqual(shown(edit), ["Undo", "Redo", "Cut", "Copy", "Paste", "Paste and Match Style", "Delete", "Select All",
                                     "Find", "Spelling and Grammar", "Substitutions", "Transformations", "Speech"])
        XCTAssertEqual(shown(view), ["Actual Size", "Zoom In", "Zoom Out", "Enter Full Screen"])
        XCTAssertEqual(shown(help), ["\(ExactEnv.appName) Help"])
    }

    /// View ▸ Actual Size ⌘0, Zoom In ⌘+ and Zoom Out ⌘-, which step
    /// through Safari's zoom levels; an app's own zoom commands stand in.
    func testZoomItemsStepSafarisLevelsAndAnAppsCommandsStandIn() {
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let bar = DevMenu.makeMenu(shortcuts: presenter.shortcuts, documents: false, develop: false)
        let view = bar.items.first { $0.submenu?.title == "View" }!.submenu!
        let zoom = view.items.filter { $0.target is ZoomMenuTarget && !$0.isHidden }
        XCTAssertEqual(zoom.map(\.title), ["Actual Size", "Zoom In", "Zoom Out"])
        XCTAssertEqual(zoom.map(\.keyEquivalent), ["0", "+", "-"])
        // A hidden ⌘= answers an event that carries no layout (the agent's).
        let equals = view.items.first { $0.identifier == DevMenu.zoomAlternate }!
        XCTAssertEqual(equals.keyEquivalent, "=")
        XCTAssertTrue(equals.isHidden && equals.allowsKeyEquivalentWhenHidden)
        XCTAssertTrue(view.performKeyEquivalent(with: key("=", code: 24, window: window)))
        let listed = { view.items.filter { !$0.isHidden }.map { $0.isSeparatorItem ? "—" : $0.title } }
        XCTAssertEqual(listed(), ["Actual Size", "Zoom In", "Zoom Out", "—", "Enter Full Screen"])
        XCTAssertEqual(PageZoom.next(after: 1), 1.15)
        XCTAssertEqual(PageZoom.previous(before: 1), 0.85)
        XCTAssertNil(PageZoom.next(after: 3))
        XCTAssertNil(PageZoom.previous(before: 0.5))
        // With no key window's session there is nothing to zoom.
        XCTAssertFalse(DevMenu.zoomTarget.validateMenuItem(zoom[1]))
        // An app's ⌘= stands in for Zoom In, its ⌘0 for Actual Size.
        button(1, "Bigger", "Meta+=", in: presenter)
        button(2, "Fit", "Meta+0", in: presenter)
        presenter.shortcuts.sync()
        XCTAssertEqual(listed(), ["Fit", "Bigger", "Zoom Out", "—", "Enter Full Screen"], "each in its host item's place")
        presenter.views[1]?.removeFromSuperview(); presenter.views.removeValue(forKey: 1)
        presenter.shortcuts.sync()
        XCTAssertFalse(zoom[1].isHidden)
        XCTAssertEqual(zoom[1].keyEquivalent, "+")
        XCTAssertTrue(equals.isHidden, "its alternate stays hidden")
        XCTAssertEqual(equals.keyEquivalent, "=")
    }

    /// View with Zoom In and Zoom Out removed and a probe as its last item,
    /// attached to the presenter's shortcut host.
    private func removedZoom(_ presenter: Presenter, items extra: [NSMenuItem] = []) -> (root: NodeView, view: ShortcutMenu, zoom: [CountingItem], probe: MenuLayoutProbe, file: ShortcutMenu) {
        let root = NodeView(id: 900, kind: "view", presenter: presenter)
        root.props = ["menuRemove": "zoom-in zoom-out"]
        presenter.views[root.id] = root; presenter.root.addSubview(root)
        let view = ShortcutMenu(title: "View")
        let zoom = [CountingItem(title: "Zoom In", action: NSSelectorFromString("zoomIn:"), keyEquivalent: "+"),
                    CountingItem(title: "Zoom Out", action: NSSelectorFromString("zoomOut:"), keyEquivalent: "-")]
        let probe = MenuLayoutProbe(frame: NSRect(x: 0, y: 0, width: 160, height: 22))
        let probeItem = NSMenuItem()
        probeItem.view = probe
        for item in zoom + extra + [probeItem] { view.addItem(item) }
        let file = ShortcutMenu(title: "File") // the host holds its menus weakly
        presenter.shortcuts.attach(file, view: view)
        return (root, view, zoom, probe, file)
    }

    /// Pops `menu` up and runs `whileOpen` once its rows are laid out and
    /// shown, then closes it. Whether AppKit tracked it.
    private func popUp(_ menu: NSMenu, in window: NSWindow, whileOpen: @escaping () -> Void = {}) -> Bool {
        window.orderFrontRegardless()
        var tracked = false
        let observer = NotificationCenter.default.addObserver(forName: NSMenu.didBeginTrackingNotification, object: menu, queue: nil) { _ in
            tracked = true
            let close = Timer(timeInterval: 0.3, repeats: false) { _ in whileOpen(); menu.cancelTrackingWithoutAnimation() }
            RunLoop.current.add(close, forMode: .common)
        }
        defer { NotificationCenter.default.removeObserver(observer) }
        menu.popUp(positioning: nil, at: .zero, in: window.contentView)
        return tracked
    }

    /// AppKit lays an open menu out as a table, and a view of the menu's
    /// own entering the menu's window can start a plan batch inside that
    /// layout (Help's search field becomes key, which posts the window
    /// facts). A sync there leaves a standing removal untouched: an item
    /// shown and hidden again inside the layout inserts a row, and AppKit
    /// throws from `-[NSTableRowData insertRowsAtIndexes:withRowAnimation:]`.
    func testASyncInsideAnOpenMenusLayoutLeavesItsRemovedItemsAlone() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent tracks no menu")
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        // The sync writes Show Sidebar's hidden state, unchanged, each time.
        let sidebar = NSMenuItem(title: "Show Sidebar", action: #selector(NSSplitViewController.toggleSidebar(_:)), keyEquivalent: "")
        let menu = removedZoom(presenter, items: [sidebar])
        defer { withExtendedLifetime(menu.file) {} }
        XCTAssertTrue(menu.zoom.allSatisfy(\.isHidden))
        let writes = menu.zoom.map(\.writes)
        menu.probe.moved = { presenter.shortcuts.sync() }
        XCTAssertTrue(popUp(menu.view, in: window), "the menu tracked")
        XCTAssertGreaterThan(menu.probe.calls, 0, "the sync ran inside the menu's layout")
        XCTAssertEqual(menu.zoom.map(\.writes), writes, "an unchanged removal writes nothing")
        XCTAssertTrue(menu.zoom.allSatisfy(\.isHidden))
        // Withdrawn with the menu closed, the removal shows the items at once.
        menu.probe.moved = {}
        menu.root.props["menuRemove"] = ""
        presenter.shortcuts.sync()
        XCTAssertFalse(menu.zoom.contains(where: \.isHidden))
    }

    /// A removal withdrawn inside the open menu's layout shows its item on
    /// the run loop's next turn, with the menu still open.
    func testARemovalWithdrawnInsideAnOpenMenusLayoutShowsItsItemOnTheNextTurn() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent tracks no menu")
        let presenter = Presenter(), window = window(presenter)
        defer { window.close() }
        let menu = removedZoom(presenter)
        defer { withExtendedLifetime(menu.file) {} }
        var hiddenInLayout: [Bool] = []
        menu.probe.moved = {
            menu.root.props["menuRemove"] = "zoom-in"
            presenter.shortcuts.sync()
            hiddenInLayout = menu.zoom.map(\.isHidden)
        }
        var hiddenWhileOpen: [Bool] = []
        XCTAssertTrue(popUp(menu.view, in: window) { hiddenWhileOpen = menu.zoom.map(\.isHidden) }, "the menu tracked")
        XCTAssertGreaterThan(menu.probe.calls, 0, "the sync ran inside the menu's layout")
        XCTAssertEqual(hiddenInLayout, [true, true], "nothing written inside the layout")
        XCTAssertEqual(hiddenWhileOpen, [true, false], "Zoom Out shows while the menu is open")
    }
}
#endif

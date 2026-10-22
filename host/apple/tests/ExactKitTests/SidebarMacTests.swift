// @ref LLP 1115.002 D3 — the root tab bar as the window's sidebar on the
// Mac: which tablist (the kernel's rule and the web's placement rule), what
// its rows show, that a row presses its authored tab and follows the app's
// selection, that the tablist's box leaves layout while the sidebar stands
// for it, and what the split view does on a narrow window.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class SidebarMacTests: XCTestCase {
    private var window: NSWindow?
    private var session: ExactSession?
    override func tearDown() {
        window?.close(); window = nil
        session?.destroy(); session = nil
    }

    /// The tabs recipe (docs/recipes/tabs.md) with what the tests turn on
    /// by `mode`: a disabled tab, a badge, the tablist hidden as the recipe
    /// hides it, and the tablist placed by the app (a row beside the panels).
    private let app = """
    routes nav
      tab today "/"
      tab history "/history"
        day "/day/:key"

    component App
      state mode = ""
      action pick(name: string)
        nav = select(nav, name)
      action follow(url: string)
        nav = go(nav, url)
      action open
        nav = go(nav, "/day/2026-10-09")
      action set(to: string)
        mode = to
      view
        main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow testId="app"
          width="100%" height="100%" display="flex" flex-direction=(mode == "placed" ? "row" : "column")
          column flex=1 min-height=0 position="relative" testId="panels"
            for t in nav.tabs key=t.name
              column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
                for e in t.stack key=e.id
                  column navigationKey=`${e.id}` position="absolute" inset=0 testId=`route-${e.name}`
                    button press=pick("today") testId=`go-today-${e.name}`
                      text "Today"
                    button press=open testId=`open-${e.name}`
                      text "Open"
                    button press=set("renamed") testId=`rename-${e.name}`
                      text "Rename"
                    button press=set("disabled") testId=`disabled-${e.name}`
                      text "Disable"
                    button press=set("badge") testId=`badge-${e.name}`
                      text "Badge"
                    button press=set("hidden") testId=`hidden-${e.name}`
                      text "Hide"
                    button press=set("authored") testId=`authored-${e.name}`
                      text "Authored"
                    button press=set("placed") testId=`placed-${e.name}`
                      text "Place"
                    button press=set("") testId=`plain-${e.name}`
                      text "Plain"
          row role="tablist" testId="tabs" aria-label="Places" display=(mode == "hidden" ? "none" : "flex")
            appearance=(mode == "authored" ? "none" : "auto")
            button role="tab" aria-controls="panel-today" aria-selected=(nav.tab == "today") press=pick("today") testId="tab-today" flex=1
              image (mode == "renamed" ? "symbol:book" : "symbol:calendar")
              text (mode == "renamed" ? "Renamed" : "Today")
            button appearance="none" role="tab" aria-controls="panel-history" aria-selected=(nav.tab == "history") press=pick("history") testId="tab-history" flex=1
              disabled=(mode == "disabled")
              image "symbol:chart"
              text "History"
              if mode == "badge"
                column background-color="#ff3b30" testId="count"
                  text "3"

    """

    private func boot(size: CGSize) throws -> ExactSession {
        _ = NSApplication.shared
        let dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../target/sidebar-test-" + UUID().uuidString).standardizedFileURL
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let contract = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try app.write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process(), said = Pipe()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        compiler.standardError = said
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0, String(decoding: said.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self))
        let session = ExactApp.shared.makeSession(label: "sidebar-" + UUID().uuidString)
        self.session = session
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: size).error)
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }
    private func press(_ testId: String) throws {
        let session = try XCTUnwrap(self.session)
        session.apply(session.runtime.press(try node(testId).id, now: session.now()))
    }

    /// The window's owner, as ExactMac composes it: a full-size-content
    /// window whose content is the split view, and the window's toolbar.
    /// `shown`: ordered in, off every screen, never key and never active.
    /// AppKit shows a hidden sidebar again, and keeps a split view's panes
    /// by name, only for a window that is shown.
    private func compose(_ view: ExactView, width: CGFloat = 700, autosave: String? = nil, shown: Bool = false) -> ExactSidebarController {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: width, height: 500),
                              styleMask: [.titled, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        self.window = window
        let split = ExactSidebarController(view: view, autosaveName: autosave, minimumPageWidth: 320)
        split.view.frame = NSRect(x: 0, y: 0, width: width, height: 500)
        window.contentViewController = split
        window.setContentSize(NSSize(width: width, height: 500))
        XCTAssertTrue(view.attachWindowToolbar(to: window))
        if shown {
            window.setFrameOrigin(NSPoint(x: -20000, y: -20000))
            window.orderFrontRegardless()
        }
        window.layoutIfNeeded()
        return split
    }
    private func cell(_ split: ExactSidebarController, _ row: Int) throws -> SidebarCell {
        try XCTUnwrap(split.table.view(atColumn: 0, row: row, makeIfNecessary: true) as? SidebarCell)
    }
    private func choose(_ split: ExactSidebarController, _ row: Int) {
        // A person's choice of a row, as the table asks its delegate on the way down.
        XCTAssertEqual(split.table.delegate?.tableView?(split.table, shouldSelectRow: row), false, "the table never selects by itself")
    }

    func testANativeTabsChangedFaceReachesItsSidebarRow() throws {
        let session = try boot(size: CGSize(width: 700, height: 500))
        let view = ExactView(session: session)
        let split = compose(view)
        let tab = try node("tab-today")
        XCTAssertTrue(tab.isNativeButton)
        XCTAssertEqual(try cell(split, 0).textField?.stringValue, "Today")
        let batch = session.runtime.press(try node("rename-today").id, now: session.now())
        XCTAssertTrue(batch.controls, "the native face travels in controls")
        session.apply(batch)
        let row = try XCTUnwrap(session.presenter.sidebar.shown?.rows.first)
        XCTAssertEqual(row.face.title, "Renamed")
        XCTAssertEqual(row.face.base, "book")
        XCTAssertEqual(try cell(split, 0).textField?.stringValue, "Renamed")
        XCTAssertEqual(split.table.selectedRowIndexes, [0])
    }

    func testTheRootTabBarIsASourceListWhoseRowsPressTheAuthoredTabsAndFollowTheApp() throws {
        let session = try boot(size: CGSize(width: 700, height: 500))
        let p = session.presenter
        let view = ExactView(session: session)
        // Wanted, by the kernel's root tabs and the web's placement rule; nothing claimed by mounting.
        XCTAssertTrue(view.wantsWindowSidebar)
        XCTAssertFalse(p.sidebar.installed)
        let tablist = try node("tabs"), panels = try node("panels")
        XCTAssertFalse(tablist.isHidden)
        XCTAssertGreaterThan(tablist.frame.height, 0, "as authored: the tab bar takes its row under the panels")
        XCTAssertEqual(panels.frame.height + tablist.frame.height, 500)

        let split = compose(view)
        let window = try XCTUnwrap(self.window)
        XCTAssertTrue(p.sidebar.installed)
        // AppKit's source list with the system's row size style: nothing set by hand.
        let table = split.table
        XCTAssertEqual(table.style, .sourceList)
        XCTAssertEqual(table.rowSizeStyle, .default)
        XCTAssertNotEqual(table.effectiveRowSizeStyle, .custom)
        XCTAssertEqual(table.numberOfRows, 2)
        XCTAssertEqual(table.accessibilityLabel(), "Places")
        let today = try cell(split, 0), history = try cell(split, 1)
        XCTAssertEqual([today.textField?.stringValue, history.textField?.stringValue], ["Today", "History"])
        XCTAssertNotNil(today.imageView?.image)
        XCTAssertNotNil(history.imageView?.image)
        XCTAssertNil(today.imageView?.contentTintColor, "the symbol's colour is AppKit's")
        XCTAssertEqual(today.accessibilityIdentifier(), "tab-today")
        XCTAssertEqual(table.selectedRowIndexes, [0])
        // The tablist's box is the platform's: hidden, and out of layout, so the page has the pane's height.
        XCTAssertTrue(tablist.isHidden)
        XCTAssertEqual(p.covers.delivered, [tablist.id: .whole])
        XCTAssertEqual(view.frame.width, 700 - split.sidebarWidth - split.splitView.dividerThickness, accuracy: 1, "the page narrows by the sidebar")
        XCTAssertEqual(panels.frame.size, view.bounds.size, "and the tab bar takes no row of it")
        XCTAssertEqual(view.frame.height, window.contentLayoutRect.height, accuracy: 0.5, "the page starts under the title bar")
        XCTAssertLessThan(view.frame.height, try XCTUnwrap(view.superview).bounds.height, "which is over its pane's top")
        // The window's toolbar leads with the sidebar's button and its separator.
        XCTAssertEqual(window.toolbar?.items.prefix(2).map(\.itemIdentifier), [.toggleSidebar, .sidebarTrackingSeparator])

        // A row chosen presses its authored tab; the app's answer selects the row.
        choose(split, 1)
        XCTAssertEqual(table.selectedRowIndexes, [1])
        XCTAssertFalse(try node("route-history").isHiddenOrHasHiddenAncestor)
        XCTAssertTrue(try node("route-today").isHiddenOrHasHiddenAncestor)
        // The app changes the tab itself: the row follows.
        try press("go-today-history")
        XCTAssertEqual(table.selectedRowIndexes, [0])
        // The driver's tap on a tab is its row's press.
        XCTAssertEqual(p.sidebar.activate(try node("tab-history")), true)
        XCTAssertEqual(table.selectedRowIndexes, [1])
        XCTAssertNil(p.sidebar.activate(panels), "a node that is no tab of the sidebar's")
        // A press on the selected row is the tab's own re-press: its stack pops to its root.
        try press("open-history")
        XCTAssertFalse(try node("route-day").isHiddenOrHasHiddenAncestor)
        choose(split, 1)
        XCTAssertTrue(session.presenter.views.values.first { $0.props["testId"] == "route-day" }?.isHiddenOrHasHiddenAncestor ?? true,
                      "the pushed route is gone or hidden")
        XCTAssertFalse(try node("route-history").isHiddenOrHasHiddenAncestor)
        XCTAssertEqual(table.selectedRowIndexes, [1])

        // A disabled tab's row is disabled, and choosing it does nothing.
        try press("go-today-history")
        try press("disabled-today")
        XCTAssertEqual(try cell(split, 1).textField?.isEnabled, false)
        choose(split, 1)
        XCTAssertEqual(table.selectedRowIndexes, [0])
        XCTAssertEqual(p.sidebar.activate(try node("tab-history")), false)
        // A badge box on the tab is the row's badge.
        try press("badge-today")
        let badge = try XCTUnwrap(try cell(split, 1).subviews.compactMap { $0 as? NSButton }.first)
        XCTAssertEqual(badge.title, "3")
        XCTAssertEqual(badge.bezelStyle, .badge)
        XCTAssertNil(try cell(split, 0).subviews.compactMap { $0 as? NSButton }.first)
        try press("plain-today")
        XCTAssertNil(try cell(split, 1).subviews.compactMap { $0 as? NSButton }.first)
        XCTAssertEqual(try cell(split, 1).textField?.isEnabled, true)
    }

    func testAppearanceNoneRestoresTheAuthoredBarAndKeepsEachTabsStack() throws {
        let session = try boot(size: CGSize(width: 700, height: 500))
        let p = session.presenter, view = ExactView(session: session)
        let split = compose(view)
        let tabs = try node("tabs")
        XCTAssertTrue(p.sidebar.installed)
        choose(split, 1)
        try press("open-history")
        try press("authored-day")
        XCTAssertFalse(view.wantsWindowSidebar)
        XCTAssertFalse(p.sidebar.installed)
        XCTAssertFalse(tabs.isHidden)
        XCTAssertNil(p.segments.control(of: tabs.id))
        XCTAssertNil(p.covers.delivered[tabs.id])
        XCTAssertGreaterThan(tabs.frame.height, 0)
        try press("tab-today")
        XCTAssertFalse(try node("route-today").isHiddenOrHasHiddenAncestor)
        try press("tab-history")
        XCTAssertFalse(try node("route-day").isHiddenOrHasHiddenAncestor)
        try press("plain-day")
        XCTAssertTrue(view.wantsWindowSidebar)
        XCTAssertTrue(p.sidebar.installed)
        XCTAssertTrue(tabs.isHidden)
        XCTAssertEqual(split.table.selectedRowIndexes, [1])
        XCTAssertFalse(try node("route-day").isHiddenOrHasHiddenAncestor)
    }

    func testAppearanceToggleRecomposesTheWindowAndSettles() throws {
        let session = try boot(size: CGSize(width: 700, height: 500))
        let view = ExactView(session: session)
        var split: ExactSidebarController? = compose(view, shown: true)
        let window = try XCTUnwrap(self.window)
        var compositions = 0
        view.onViewportFit = { split?.viewportFitChanged() }
        view.onWindowSidebar = {
            compositions += 1
            XCTAssertLessThanOrEqual(compositions, 3, "one composition per appearance change")
            if view.wantsWindowSidebar {
                window.contentView = NSView()
                split = ExactSidebarController(view: view, autosaveName: nil, minimumPageWidth: 320)
                split?.install(in: window)
            } else {
                split?.release()
                split = nil
                window.contentViewController = nil
                window.contentView = view
                window.styleMask.remove(.fullSizeContentView)
            }
            view.attachWindowToolbar(to: window)
        }
        defer { view.onWindowSidebar = nil; view.onViewportFit = nil }
        // From a pushed route, as the fixture's drive does: the toolbar holds
        // Back, and the view leaving the split view takes the toolbar away
        // and brings a new one. Its Back is a new item: AppKit asserted
        // (NSToolbarItemViewer minSize) laying out one item in two toolbars.
        try press("tab-history")
        try press("open-history")
        XCTAssertEqual(window.toolbar?.items.map(\.itemIdentifier).contains(WindowToolbarHost.backIdentifier), true)
        var backs: [NSToolbarItem] = []
        for (target, authored) in [("authored-day", true), ("plain-day", false), ("authored-day", true)] {
            let reply = Agent(session: session).tap(["id": Int(try node(target).id)])
            XCTAssertNil(reply["error"], "\(reply)")
            for _ in 0..<12 {
                window.layoutIfNeeded()
                RunLoop.main.run(mode: .default, before: Date())
            }
            XCTAssertEqual(view.wantsWindowSidebar, !authored)
            XCTAssertEqual(session.presenter.sidebar.installed, !authored)
            XCTAssertTrue(view.window === window)
            XCTAssertFalse(try node("route-day").isHiddenOrHasHiddenAncestor)
            let toolbar = try XCTUnwrap(window.toolbar)
            let back = try XCTUnwrap(toolbar.items.first { $0.itemIdentifier == WindowToolbarHost.backIdentifier })
            XCTAssertTrue(back.isEnabled)
            XCTAssertFalse(backs.contains { $0 === back }, "each toolbar has its own Back")
            backs.append(back)
            XCTAssertEqual(toolbar.items.first?.itemIdentifier == .toggleSidebar, !authored)
        }
        XCTAssertEqual(compositions, 3)
    }

    func testTheAuthorsHideAndPlacementAndANarrowWindow() throws {
        let session = try boot(size: CGSize(width: 700, height: 500))
        let p = session.presenter
        let view = ExactView(session: session)
        let split = compose(view, shown: true)
        let window = try XCTUnwrap(self.window)
        let tablist = try node("tabs")
        var changes = 0
        view.onWindowSidebar = { changes += 1 }
        let items = { window.toolbar?.items.map(\.itemIdentifier) ?? [] }

        // On a narrow window the page keeps its minimum and the window stops at the two panes' (LLP 1115.002 §1).
        window.setContentSize(NSSize(width: 300, height: 500))
        window.layoutIfNeeded()
        XCTAssertEqual(view.frame.width, 320, accuracy: 1)
        XCTAssertEqual(window.contentView?.frame.width ?? 0, 320 + split.sidebarWidth, accuracy: 2)
        XCTAssertFalse(split.collapsed, "AppKit hides no sidebar by itself in a window")
        window.setContentSize(NSSize(width: 700, height: 500))
        window.layoutIfNeeded()

        // View ▸ Show Sidebar is there for a window with a sidebar, with AppKit's chord.
        let menu = DevMenu.makeMenu(shortcuts: p.shortcuts, documents: false, develop: false)
        let item = try XCTUnwrap(menu.items.compactMap { $0.submenu }.flatMap(\.items).first { $0.action == #selector(NSSplitViewController.toggleSidebar(_:)) })
        XCTAssertFalse(item.isHidden)
        XCTAssertEqual(item.keyEquivalent, "s")
        XCTAssertEqual(item.keyEquivalentModifierMask, [.command, .control])
        XCTAssertTrue(split.validateUserInterfaceItem(item))
        XCTAssertEqual(item.title, "Hide Sidebar", "AppKit names the item for what it will do, when it validates it")
        // Its action reaches the split view controller from inside the page, up the responder chain; the
        // sidebar goes and the page takes the window, then comes back. AppKit animates both.
        func toggled(_ hidden: Bool, _ why: String) {
            XCTAssertEqual(view.tryToPerform(#selector(NSSplitViewController.toggleSidebar(_:)), with: item), true, why)
            let limit = Date().addingTimeInterval(10)
            // The animation has ended when the sidebar's pane is hidden, or shown at its width.
            let settled = {
                split.collapsed == hidden && split.splitView.arrangedSubviews.first?.isHidden == hidden
                    && abs(view.frame.width - (hidden ? 700 : 700 - split.sidebarWidth - split.splitView.dividerThickness)) < 1.5
            }
            while !settled(), Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
            XCTAssertTrue(settled(), "\(why): collapsed \(split.collapsed), page \(view.frame.width)")
        }
        toggled(true, "hidden by the person")
        XCTAssertTrue(tablist.isHidden, "hidden is not gone: the tablist stays the sidebar's")
        XCTAssertEqual(p.covers.delivered, [tablist.id: .whole])
        XCTAssertTrue(split.validateUserInterfaceItem(item))
        XCTAssertEqual(item.title, "Show Sidebar")
        toggled(false, "shown again")

        // The author hides the tab bar on a screen (`display: none`): the sidebar goes with it, and its button.
        try press("hidden-today")
        XCTAssertTrue(split.collapsed)
        XCTAssertFalse(items().contains(.toggleSidebar))
        XCTAssertFalse(split.validateUserInterfaceItem(item), "Exact's hide is not the person's to undo")
        XCTAssertTrue(view.wantsWindowSidebar, "hidden, not gone: the window's content stays composed")
        try press("plain-today")
        XCTAssertFalse(split.collapsed)
        XCTAssertEqual(items().prefix(2), [.toggleSidebar, .sidebarTrackingSeparator])
        XCTAssertEqual(changes, 0)

        // A tablist the app places itself (a row beside the panels) is the app's: no sidebar, and its box comes back.
        // The placement is the kernel's to read, asked again when the root's level moves, as a window's resize
        // moves it (what turns a responsive app's bar into its own rail).
        try press("placed-today")
        window.setContentSize(NSSize(width: 760, height: 500))
        window.layoutIfNeeded()
        XCTAssertFalse(view.wantsWindowSidebar)
        XCTAssertEqual(changes, 1, "the owner hears, and takes the view back")
        XCTAssertFalse(p.sidebar.installed)
        XCTAssertFalse(tablist.isHidden)
        XCTAssertEqual(p.covers.delivered, [:])
        XCTAssertGreaterThan(tablist.frame.width, 0)
        p.shortcuts.sync()
        XCTAssertTrue(item.isHidden, "no sidebar, no menu item")
        split.release()
        XCTAssertNil(view.superview)
        XCTAssertTrue(view.translatesAutoresizingMaskIntoConstraints)
        // Unsaid again (the bar moves back under the panels): wanted again.
        try press("plain-today")
        XCTAssertTrue(view.wantsWindowSidebar)
        XCTAssertEqual(changes, 2)
    }

    /// The owner's window is a plain titled one showing something else when
    /// the app's first batch names its tabs, as ExactMac's is. Installed
    /// there, the list's first row is where AppKit puts one in a window that
    /// was full-size-content from the start: under the bar, with none of the
    /// padding a list keeps from a window whose content stopped under it.
    func testInstalledInAPlainWindowTheFirstRowIsUnderTheBar() throws {
        let session = try boot(size: CGSize(width: 700, height: 500))
        let view = ExactView(session: session)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 500), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        self.window = window
        window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: 700, height: 500))
        // Ordered in, off every screen, never key and never active.
        window.setFrameOrigin(NSPoint(x: -20000, y: -20000))
        window.orderFrontRegardless()
        window.layoutIfNeeded()
        XCTAssertFalse(window.styleMask.contains(.fullSizeContentView))

        let split = ExactSidebarController(view: view, autosaveName: nil, minimumPageWidth: 320)
        split.install(in: window)
        XCTAssertTrue(window.contentViewController === split)
        XCTAssertTrue(window.styleMask.contains(.fullSizeContentView), "the sidebar runs under the title bar")
        XCTAssertEqual(window.frame.width, 700, "the window keeps its width")
        XCTAssertTrue(view.attachWindowToolbar(to: window))
        window.layoutIfNeeded()
        let table = split.table
        XCTAssertEqual(table.numberOfRows, 2)
        XCTAssertEqual(table.rect(ofRow: 0).minY, 0, "no padding above the first row: the bar is over the list")
        let row = try XCTUnwrap(table.rowView(atRow: 0, makeIfNecessary: true))
        let bar = window.frame.height - window.contentLayoutRect.height
        XCTAssertGreaterThan(bar, 0)
        XCTAssertEqual(window.frame.height - row.convert(row.bounds, to: nil).maxY, bar, accuracy: 0.5, "the first row starts where the bar ends")
    }

    func testTheSidebarsWidthIsRememberedByName() throws {
        let session = try boot(size: CGSize(width: 700, height: 500))
        let view = ExactView(session: session)
        let name = "exact.test.sidebar." + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "NSSplitView Subview Frames " + name) }
        var split = compose(view, autosave: name, shown: true)
        let first = split.sidebarWidth
        split.splitView.setPosition(first + 76, ofDividerAt: 0)
        window?.layoutIfNeeded()
        XCTAssertEqual(split.sidebarWidth, first + 76, accuracy: 1)
        // AppKit writes it on a later turn of the run loop: waited for, bounded.
        let saved = { UserDefaults.standard.object(forKey: "NSSplitView Subview Frames " + name) != nil }
        let limit = Date().addingTimeInterval(10)
        while !saved(), Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(saved(), "AppKit's own autosave holds it")
        split.release()
        window?.close()
        split = compose(view, autosave: name, shown: true)
        XCTAssertEqual(split.sidebarWidth, first + 76, accuracy: 1, "the next window's sidebar opens at the width the person left")
    }

    /// The kernel names the root's tabs: a presenter with no kernel behind
    /// it has no sidebar, and where the kernel and the views disagree the
    /// tablist stays as authored.
    func testTheKernelsRootTabsDecide() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationBack": "back", "navigationKey": "a"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["accessibilityRole": "tabpanel", "id": "one"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["navigationKey": "a"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            ["op": "create", "id": 5, "kind": "button", "props": ["accessibilityRole": "tab", "accessibilityControls": "one", "accessibilityLabel": "One"], "handlers": ["press"]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "roots", "ids": [1]],
        ]))
        XCTAssertNotNil(p.navigation.root?.tabs, "the views name a root tab")
        XCTAssertFalse(p.sidebar.wanted, "no kernel to ask")
        let touch = { p.apply(wireBatch([["op": "props", "id": 4, "set": ["accessibilityLabel": "Tabs"]]])) }
        p.rootTabs = { _ in RootTabs(tablist: 4, tabs: [5], panels: [2], unsaid: true) }
        touch()
        XCTAssertTrue(p.sidebar.wanted)
        XCTAssertEqual(p.sidebar.shown?.rows.map(\.face.title), ["One"])
        p.rootTabs = { _ in RootTabs(tablist: 4, tabs: [5], panels: [2], unsaid: false) }
        touch()
        XCTAssertFalse(p.sidebar.wanted, "placed by the app")
        p.rootTabs = { _ in RootTabs(tablist: 9, tabs: [5], panels: [2], unsaid: true) }
        touch()
        XCTAssertFalse(p.sidebar.wanted, "the kernel's tablist is not the views'")
        p.rootTabs = { _ in nil }
        touch()
        XCTAssertFalse(p.sidebar.wanted, "the views name tabs the kernel does not")
    }
}
#endif

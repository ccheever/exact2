// @ref LLP 1115.002 D4 — a list and its detail are a split view on the Mac:
// which stack splits (the runner's rule), what each pane holds (the page as
// the list's column, the detail's routes in a pane of their own through the
// kernel's containing block), the widths and minimums AppKit gives three
// columns and two, the selected row, Back, the two headers in one bar (§5
// Q10), a sheet over the split, and what the driver sees.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class SplitMacTests: XCTestCase {
    private var window: NSWindow?
    private var session: ExactSession?
    override func tearDown() {
        window?.close(); window = nil
        session?.destroy(); session = nil
    }

    /// A library of books under a root tab bar: its rows link to a book,
    /// a book pushes its notes, and + opens a sheet. `tabs: false` is the
    /// same stack with no tab bar.
    private func app(tabs: Bool) -> String {
        let routes = """
                      when e.name == "library"
                        column navigationKey=`${e.id}` position="absolute" inset=0 display="flex" flex-direction="column" testId="route-library"
                          header display="flex" align-items="center"
                            text "Library" role="heading" aria-level=1 flex=1
                            button press=openAdd aria-label="Add Book" aria-keyshortcuts="Meta+N" testId="add-book"
                              image "symbol:add"
                            input type="search" value=query input=search placeholder="Search" testId="search"
                          list testId="books" flex=1 min-height=0
                            each id in ["a", "b", "c"] key=id
                              link href=path("book", id) testId=`row-${id}`
                                text `Book ${id}`
                      when e.name == "book"
                        column navigationKey=`${e.id}` position="absolute" inset=0 display="flex" flex-direction="column" testId=`route-book-${e.params.id}`
                          header display="flex" align-items="center"
                            text `About ${e.params.id}` role="heading" aria-level=2 flex=1 testId=`title-${e.params.id}`
                            button press=openNotes(e.params.id) aria-label="Notes" testId=`notes-${e.params.id}`
                              image "symbol:pencil"
                            button press=count aria-label="Count" aria-keyshortcuts="Meta+N" testId=`count-${e.params.id}`
                              image "symbol:delete"
                          box testId=`half-${e.params.id}` width="50%" height=40 background-color="#f9ab00"
                          input value=query input=search testId=`field-${e.params.id}`
                      when e.name == "notes"
                        column navigationKey=`${e.id}` position="absolute" inset=0 testId="route-notes"
                          text "Notes on the book"
                      when e.name == "add"
                        column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0 testId="route-add"
                          header display="flex" align-items="center"
                            button id="back" press=back testId="add-cancel"
                              text "Cancel"
                            text "New Book" role="heading"
                          text "A form"
                      when e.name == "stats"
                        column navigationKey=`${e.id}` position="absolute" inset=0 testId="route-stats"
                          text "Stats"
        """
        let head = """
        routes nav
        \(tabs ? "  tab library \"/\"\n    book \"/book/:id\"\n    notes \"/notes/:id\"\n    add \"/add\"\n  tab stats \"/stats\"" : "  library \"/\"\n    book \"/book/:id\"\n    notes \"/notes/:id\"\n    add \"/add\"")

        component App
          state query = ""
          state counted = 0
          action pick(name: string)
            nav = select(nav, name)
          action follow(url: string)
            nav = go(nav, url)
          action back
            nav = back(nav)
          action openAdd
            nav = push(nav, "/add")
          action openNotes(id: string)
            nav = push(nav, path("notes", id))
          action search(v: string)
            query = v
          action count
            counted = counted + 1
          view
            main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow testId="app"
              width="100%" height="100%" display="flex" flex-direction="column"

        """
        if !tabs {
            let dedented = routes.split(separator: "\n", omittingEmptySubsequences: false).map { String($0.dropFirst(6)) }.joined(separator: "\n")
            return head + "      each e in stack(nav) key=e.id\n" + dedented + "\n"
        }
        return head + """
              column flex=1 min-height=0 position="relative" testId="panels"
                each t in nav.tabs key=t.name
                  column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
                    each e in t.stack key=e.id

        """ + routes + """

              row role="tablist" testId="tabs" aria-label="Places"
                button role="tab" aria-controls="panel-library" aria-selected=(nav.tab == "library") press=pick("library") testId="tab-library" flex=1
                  image "symbol:calendar"
                  text "Library"
                button role="tab" aria-controls="panel-stats" aria-selected=(nav.tab == "stats") press=pick("stats") testId="tab-stats" flex=1
                  image "symbol:chart"
                  text "Stats"

        """
    }

    private func boot(tabs: Bool, size: CGSize) throws -> ExactSession {
        _ = NSApplication.shared
        let dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../target/split-test-" + UUID().uuidString).standardizedFileURL
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let contract = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try app(tabs: tabs).write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process(), said = Pipe()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        compiler.standardError = said
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0, String(decoding: said.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self))
        let session = ExactApp.shared.makeSession(label: "split-" + UUID().uuidString)
        self.session = session
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: size).error)
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }
    private func has(_ testId: String) -> Bool { session?.presenter.views.values.contains { $0.props["testId"] == testId } == true }
    private func press(_ testId: String) throws {
        let session = try XCTUnwrap(self.session)
        session.presenter.press(try node(testId).id)
    }

    /// The window's owner, as ExactMac composes it: a full-size-content
    /// window whose content is the split view, and the window's toolbar.
    private func compose(_ view: ExactView, width: CGFloat, sidebar: Bool, minimum: CGFloat = 320, autosave: String? = nil) -> ExactSidebarController {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: width, height: 500),
                              styleMask: [.titled, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        self.window = window
        window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: width, height: 500))
        let split = ExactSidebarController(view: view, autosaveName: autosave, minimumPageWidth: minimum, sidebar: sidebar)
        split.install(in: window)
        window.setContentSize(NSSize(width: width, height: 500))
        XCTAssertTrue(view.attachWindowToolbar(to: window))
        window.layoutIfNeeded()
        return split
    }
    private func width(_ window: NSWindow, _ width: CGFloat) {
        window.setContentSize(NSSize(width: width, height: 500))
        window.layoutIfNeeded()
    }
    private func items(_ window: NSWindow) -> [String] { window.toolbar?.items.map(\.itemIdentifier.rawValue) ?? [] }

    func testAListAndItsDetailAreTwoPanesBesideTheSidebar() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500))
        let p = session.presenter, split = p.navigation.split
        let view = ExactView(session: session)
        // Wanted by the runner's rule; nothing is claimed by mounting, and the stack pushes.
        XCTAssertTrue(view.wantsWindowSplit)
        XCTAssertFalse(split.active)
        let columns = compose(view, width: 1100, sidebar: true)
        let window = try XCTUnwrap(self.window)
        XCTAssertTrue(split.active)
        XCTAssertTrue(columns.splits)
        let divider = columns.splitView.dividerThickness
        let library = try node("route-library"), books = try node("books")
        // Three columns: AppKit's sidebar, the list at the rule's width, and the detail with the rest.
        XCTAssertEqual(columns.sidebarWidth, 144, accuracy: 1)
        XCTAssertEqual(columns.listColumnWidth, ExactSidebarController.listWidth, accuracy: 1)
        XCTAssertEqual(columns.detailWidth, 1100 - columns.sidebarWidth - columns.listColumnWidth - divider, accuracy: 1)
        XCTAssertEqual(library.frame.width, columns.listColumnWidth, accuracy: 1, "the page is the list's column")
        XCTAssertNotNil(p.tables.feeds[books.id], "whose list is a table")
        // Nothing selected: the pane says so, in AppKit's words.
        XCTAssertEqual(columns.detail.saysNothing, SplitDetail.noSelection)
        XCTAssertEqual(SplitDetail.noSelection, "No Selection")
        XCTAssertEqual(window.title, "Library")
        XCTAssertFalse(p.navigation.canGoBack)

        // A row is pressed: its route is in the detail's pane, laid out at the pane's size, beside a list that stays live.
        let feed = try XCTUnwrap(p.tables.feeds[books.id])
        feed.table.selectRowIndexes([1], byExtendingSelection: false)
        XCTAssertTrue(feed.pressRow(1))
        let book = try node("route-book-b"), pane = columns.detailPane
        XCTAssertTrue(book.superview === pane)
        XCTAssertEqual(book.frame, NSRect(origin: .zero, size: pane.bounds.size))
        XCTAssertEqual(pane.bounds.width, columns.detailWidth, accuracy: 1)
        XCTAssertEqual(try node("half-b").frame.width, pane.bounds.width / 2, accuracy: 0.5, "a percentage is the pane's")
        XCTAssertEqual(library.frame.width, columns.listColumnWidth, accuracy: 1)
        XCTAssertFalse(library.isHidden)
        XCTAssertFalse(library.inert, "the list is live beside its detail")
        XCTAssertFalse(book.isHidden)
        XCTAssertNil(columns.detail.saysNothing)
        XCTAssertEqual(feed.table.selectedRow, 1, "the row whose detail shows stays selected")
        XCTAssertEqual(split.shown?.row, try node("row-b").id)
        // The list owns the window (§5 Q10): its heading is the title, and the detail's stays the first line of its pane.
        XCTAssertEqual(window.title, "Library")
        let title = try node("title-b")
        XCTAssertFalse(title.isHiddenOrHasHiddenAncestor)
        XCTAssertTrue(try node("notes-b").isHidden, "its buttons are the bar's")
        let add = try node("add-book"), notes = try node("notes-b"), count = try node("count-b"), search = try node("search")
        XCTAssertEqual(items(window), ["NSToolbarToggleSidebarItem", "NSToolbarSidebarTrackingSeparatorItemIdentifier", "exact.command.\(add.id)",
                                       "exact.split.separator", "exact.command.\(notes.id)", "exact.command.\(count.id)", "NSToolbarFlexibleSpaceItem", "exact.search.\(search.id)"])
        XCTAssertEqual(p.covers.delivered[notes.id], .whole)
        XCTAssertNil(p.covers.delivered[title.id])
        // The detail has no Back: nothing was pushed from it.
        XCTAssertFalse(p.navigation.canGoBack)

        // Another row, which this app pushes over the first: it is that row's detail, and still nothing leads Back.
        feed.table.selectRowIndexes([2], byExtendingSelection: false)
        XCTAssertTrue(feed.pressRow(2))
        let other = try node("route-book-c")
        XCTAssertTrue(other.superview === pane)
        XCTAssertTrue(book.isHidden)
        XCTAssertEqual(split.shown?.base, 2)
        XCTAssertFalse(p.navigation.canGoBack)
        XCTAssertEqual(feed.table.selectedRow, 2)

        // A push inside the detail: the window's Back leads from it, and the row stays selected.
        p.press(try node("notes-c").id)
        let pushed = try node("route-notes")
        XCTAssertTrue(pushed.superview === pane)
        XCTAssertEqual(pushed.frame.size, pane.bounds.size)
        XCTAssertTrue(p.navigation.canGoBack)
        XCTAssertEqual(feed.table.selectedRow, 2)
        XCTAssertEqual(window.title, "Library")
        // Back is at the bar's leading end, after the sidebar's own items, and came with the push.
        XCTAssertEqual(Array(items(window).prefix(3)), ["NSToolbarToggleSidebarItem", "NSToolbarSidebarTrackingSeparatorItemIdentifier", "exact.navigation.back"])
        XCTAssertTrue(p.navigation.goBack())
        XCTAssertFalse(has("route-notes"))
        XCTAssertFalse(other.isHidden)
        XCTAssertFalse(p.navigation.canGoBack)
        XCTAssertFalse(items(window).contains("exact.navigation.back"), "and goes with it")

        // The divider moves: the detail is laid out at its pane's new size.
        columns.splitView.setPosition(columns.sidebarWidth + 400, ofDividerAt: columns.listDivider)
        window.layoutIfNeeded()
        XCTAssertEqual(columns.listColumnWidth, 400, accuracy: 1)
        XCTAssertEqual(other.frame.size, pane.bounds.size)
        XCTAssertEqual(try node("half-c").frame.width, pane.bounds.width / 2, accuracy: 0.5)

        // Another tab, whose stack pushes: the pane goes, the page has the window beside the sidebar, and the book is not shown.
        try press("tab-stats")
        XCTAssertFalse(columns.splits)
        XCTAssertEqual(view.frame.width, 1100 - columns.sidebarWidth, accuracy: 1)
        XCTAssertTrue(other.isHidden)
        XCTAssertTrue(other.inert)
        XCTAssertFalse(items(window).contains("exact.split.separator"))
        // And back: the split as the person left it, with the same route in the same views.
        try press("tab-library")
        XCTAssertTrue(columns.splits)
        XCTAssertEqual(columns.listColumnWidth, 400, accuracy: 1)
        XCTAssertTrue(other.superview === pane)
        XCTAssertFalse(other.isHidden)
        XCTAssertEqual(other.frame.size, pane.bounds.size)

        // The person takes the selection off the row: the pane shows nothing, as the stack goes back to its list.
        feed.table.deselectAll(nil)
        XCTAssertFalse(has("route-book-c"))
        XCTAssertFalse(has("route-book-b"))
        XCTAssertEqual(columns.detail.saysNothing, "No Selection")
        XCTAssertEqual(feed.table.selectedRow, -1)
    }

    func testTheColumnsNarrowAsAppKitsDoAndTheWindowStopsAtTheirMinimums() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1300, height: 500))
        let view = ExactView(session: session)
        let columns = compose(view, width: 1300, sidebar: true)
        let window = try XCTUnwrap(self.window)
        let divider = columns.splitView.dividerThickness
        func widths() -> [CGFloat] { [columns.sidebarWidth, columns.listColumnWidth, columns.detailWidth].map { $0.rounded() } }
        XCTAssertEqual(widths(), [144, 320, 1300 - 144 - 320 - divider])
        // The detail gives first, down to the page's minimum.
        width(window, 900)
        XCTAssertEqual(widths(), [144, 320, 900 - 144 - 320 - divider])
        width(window, 144 + 320 + 320 + divider)
        XCTAssertEqual(widths(), [144, 320, 320])
        // Then the list, down to AppKit's 200 for a content list.
        width(window, 144 + 260 + 320 + divider)
        XCTAssertEqual(widths(), [144, 260, 320])
        // And then the window stops: the sidebar's 144, the list's 200, the detail's minimum and the dividers.
        width(window, 300)
        let least = 144 + ExactSidebarController.listMinimum + 320 + divider
        XCTAssertEqual(window.contentView?.frame.width ?? 0, least, accuracy: 1)
        XCTAssertEqual(widths(), [144, 200, 320])
        // Wider again: the list has its width back, as AppKit's own content list does (measured), and the detail the rest.
        width(window, 1000)
        XCTAssertEqual(widths(), [144, 320, 1000 - 144 - 320 - divider])
    }

    func testWithNoTabBarTheWindowIsTheListAndItsDetail() throws {
        let session = try boot(tabs: false, size: CGSize(width: 900, height: 500))
        let p = session.presenter
        let view = ExactView(session: session)
        XCTAssertFalse(view.wantsWindowSidebar)
        XCTAssertTrue(view.wantsWindowSplit)
        let columns = compose(view, width: 900, sidebar: false)
        let window = try XCTUnwrap(self.window)
        XCTAssertTrue(columns.splits)
        XCTAssertFalse(columns.hasSidebar)
        XCTAssertEqual(columns.listColumnWidth, 320, accuracy: 1)
        XCTAssertEqual(columns.detailWidth, 900 - 320 - columns.splitView.dividerThickness, accuracy: 1)
        try press("row-a")
        let book = try node("route-book-a"), add = try node("add-book"), notes = try node("notes-a"), count = try node("count-a"), search = try node("search")
        XCTAssertTrue(book.superview === columns.detailPane)
        // No sidebar's button; the list's items, the separator on the one divider, the detail's.
        XCTAssertEqual(items(window), ["exact.command.\(add.id)", "exact.split.separator",
                                       "exact.command.\(notes.id)", "exact.command.\(count.id)", "NSToolbarFlexibleSpaceItem", "exact.search.\(search.id)"])
        XCTAssertEqual(window.title, "Library")
        // The window stops at the list's 200, the divider and the detail's minimum.
        width(window, 200)
        XCTAssertEqual(window.contentView?.frame.width ?? 0, 200 + 320 + columns.splitView.dividerThickness, accuracy: 1)
        XCTAssertFalse(p.navigation.canGoBack)
    }

    func testASheetOverTheSplitLeavesBothPanesUnderItAndTheListsKeyWinsATie() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500))
        let p = session.presenter
        let view = ExactView(session: session)
        let columns = compose(view, width: 1100, sidebar: true)
        let window = try XCTUnwrap(self.window)
        window.setFrameOrigin(NSPoint(x: -20000, y: -20000))
        window.orderFrontRegardless()
        try press("row-b")
        let book = try node("route-book-b"), library = try node("route-library")
        XCTAssertTrue(book.superview === columns.detailPane)
        // ⌘N is declared in both panes: the list's is the window's, whichever pane has the focus.
        let field = try XCTUnwrap(try node("field-b").field)
        XCTAssertTrue(window.makeFirstResponder(field))
        XCTAssertTrue(view.ownsShortcutFocus(), "the detail's pane holds this session's focus")
        // The page's Tab order goes through the pane, where the route was written: its field is a stop of the page's loop.
        p.keyViewLoopStale = true
        p.flushKeyViewLoop()
        var stop: NSView? = p.viewport.nextKeyView, reached = false
        for _ in 0..<64 {
            guard let at = stop, !reached else { break }
            reached = at === field
            stop = at.nextKeyView
        }
        XCTAssertTrue(reached, "Tab from the page reaches the detail's field")
        let key = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: window.windowNumber,
                                                 context: nil, characters: "n", charactersIgnoringModifiers: "n", isARepeat: false, keyCode: 45))
        XCTAssertTrue(p.shortcuts.perform(key))
        // The list's + opened its sheet: a modal route over the split, on the window, with both panes shown under it.
        let form = try node("route-add")
        let opened = expectation(description: "the sheet arrives")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { opened.fulfill() }
        wait(for: [opened], timeout: 5)
        XCTAssertNotNil(window.attachedSheet)
        XCTAssertTrue(form.window === window.attachedSheet)
        XCTAssertFalse(library.isHidden)
        XCTAssertFalse(book.isHidden)
        XCTAssertTrue(library.inert)
        XCTAssertTrue(book.inert)
        XCTAssertTrue(book.superview === columns.detailPane)
        XCTAssertEqual(window.title, "Library")
        // Its Cancel: the stack goes back, and the split is as it was.
        p.press(try node("add-cancel").id, fromNativeMenu: true)
        let closed = expectation(description: "the sheet leaves")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { closed.fulfill() }
        wait(for: [closed], timeout: 5)
        XCTAssertFalse(has("route-add"))
        XCTAssertFalse(library.inert)
        XCTAssertFalse(book.inert)
        XCTAssertTrue(book.superview === columns.detailPane)
    }

    func testUnderTheDriversOwnChromeTheStackPushesAndTheDriverSeesTheSplit() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500))
        let p = session.presenter, split = p.navigation.split
        let view = ExactView(session: session)
        let columns = compose(view, width: 1100, sidebar: true)
        try press("row-a")
        let book = try node("route-book-a"), library = try node("route-library")
        var summary = split.summary
        XCTAssertEqual(summary["installed"] as? Bool, true)
        XCTAssertEqual(summary["detail"] as? Int, Int(book.id))
        XCTAssertEqual(summary["row"] as? Int, Int(try node("row-a").id))
        XCTAssertEqual(summary["listWidth"] as? Double, 320)
        // The driver's `back` from a split's detail: as a person leaves it, the selection off its row.
        let went = try XCTUnwrap(p.navigation.agentBack())
        XCTAssertEqual(went.way, "route")
        XCTAssertEqual(went.refused, "")
        XCTAssertFalse(has("route-book-a"))
        XCTAssertNil(p.navigation.agentBack(), "and from the list there is nowhere to go back to")

        // The driver's own chrome: the stack pushes in the page, as authored.
        try press("row-a")
        let again = try node("route-book-a")
        XCTAssertTrue(again.superview === columns.detailPane)
        split.painted = true
        XCTAssertFalse(split.active)
        XCTAssertFalse(columns.splits)
        XCTAssertFalse(again.superview === columns.detailPane)
        XCTAssertTrue(library.isHidden, "a pushed route covers its list")
        XCTAssertTrue(p.navigation.canGoBack)
        summary = split.summary
        XCTAssertEqual(summary["installed"] as? Bool, false)
        split.painted = false
        XCTAssertTrue(columns.splits)
        XCTAssertTrue(again.superview === columns.detailPane)
        XCTAssertFalse(library.isHidden)
    }
}
#endif

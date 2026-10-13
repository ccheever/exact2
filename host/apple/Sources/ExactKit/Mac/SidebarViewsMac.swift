// @ref LLP 1115.002 D3, D1 — the sidebar's views: a split view controller
// whose first item is a real sidebar holding AppKit's source list, one row a
// root tab, and whose other item holds the session's view. What is AppKit's
// is used and left alone: the list has the system's row size style, so its
// rows follow the person's Sidebar icon size, and a row is a plain
// `NSTableCellView` whose image and title AppKit places, sizes and colours
// (the symbols in the accent, as it draws a sidebar's). Nothing here sets a
// row height, an inset, a font or a selection colour.
//
// On a narrow window the split view does what AppKit does (measured,
// LLP 1115.002 §1): the page narrows to its minimum, then the sidebar to
// its own, and then the window stops. The one number given is the page's
// minimum width, which the window had.
#if os(macOS)
import AppKit

/// The window's content while the app's root tab bar is its sidebar. The
/// window's owner makes one when `ExactView.wantsWindowSidebar` and installs
/// it (`install(in:)`); `release()` gives the view back.
public final class ExactSidebarController: NSSplitViewController {
    public let exactView: ExactView
    private let list: SidebarList
    private let page: NSViewController
    private let sidebarItem: NSSplitViewItem
    private let pageItem: NSSplitViewItem
    private let autosave: String?
    private var pins: [NSLayoutConstraint] = []
    private var pinnedCover: Bool?
    /// The sidebar is hidden because the author hid the tab bar on this
    /// screen: Exact's own hide, which the toolbar's button does not undo.
    private var hiddenForApp = false

    /// `autosaveName` names where the sidebar's width, and whether the
    /// person hid it, are kept (AppKit's own autosave); `minimumPageWidth`
    /// is the narrowest the page goes, which with the sidebar's own minimum
    /// is the narrowest the window goes.
    public init(view exact: ExactView, autosaveName: String?, minimumPageWidth: CGFloat) {
        let list = SidebarList(host: exact.session.presenter.sidebar), page = NSViewController()
        page.view = NSView()
        exactView = exact
        self.list = list
        self.page = page
        sidebarItem = NSSplitViewItem(sidebarWithViewController: list)
        pageItem = NSSplitViewItem(viewController: page)
        autosave = autosaveName
        super.init(nibName: nil, bundle: nil)
        if minimumPageWidth > 0 { pageItem.minimumThickness = minimumPageWidth }
        addSplitViewItem(sidebarItem)
        addSplitViewItem(pageItem)
        splitView.autosaveName = autosaveName
        exact.translatesAutoresizingMaskIntoConstraints = false
        page.view.addSubview(exact)
        pin()
        exact.session.presenter.sidebar.attach(self)
    }
    public required init?(coder: NSCoder) { nil }

    /// The view fills its pane: under the title bar and toolbar, which are
    /// over the pane's top in a full-size content window, or the whole pane
    /// under `viewport-fit=cover`, where the page reads the bar as its top
    /// inset itself.
    private func pin() {
        // Removing the page can synchronously reset its toolbar and call
        // viewportFitChanged while release is still on the stack.
        guard exactView.superview === page.view else { return }
        let cover = exactView.viewportFit == "cover"
        guard cover != pinnedCover else { return }
        pinnedCover = cover
        NSLayoutConstraint.deactivate(pins)
        let pane = page.view, safe = pane.safeAreaLayoutGuide
        pins = cover
            ? [exactView.leadingAnchor.constraint(equalTo: pane.leadingAnchor), exactView.trailingAnchor.constraint(equalTo: pane.trailingAnchor),
               exactView.topAnchor.constraint(equalTo: pane.topAnchor), exactView.bottomAnchor.constraint(equalTo: pane.bottomAnchor)]
            : [exactView.leadingAnchor.constraint(equalTo: safe.leadingAnchor), exactView.trailingAnchor.constraint(equalTo: safe.trailingAnchor),
               exactView.topAnchor.constraint(equalTo: safe.topAnchor), exactView.bottomAnchor.constraint(equalTo: safe.bottomAnchor)]
        NSLayoutConstraint.activate(pins)
    }
    /// The first root's `viewport-fit` changed (the owner's hook).
    public func viewportFitChanged() { pin() }

    /// Make this the window's content, at the size its content has. The
    /// window is made full-size-content first, and that order is AppKit's:
    /// it decides a source list's top padding when the list enters its
    /// window. One that enters a window whose content stops under the title
    /// bar is given 10 points above its first row and keeps them when the
    /// bar comes over it afterwards; one that enters a full-size-content
    /// window starts under the bar, as a hand-built app's does (measured,
    /// LLP 1115.002 §1).
    public func install(in window: NSWindow) {
        if !window.styleMask.contains(.fullSizeContentView) { window.styleMask.insert(.fullSizeContentView) }
        // The window takes its content's size: the one it has.
        view.frame = NSRect(origin: .zero, size: window.contentView?.frame.size ?? window.frame.size)
        window.contentViewController = self
    }

    /// The view leaves the pane, placed by frames again: the owner makes it
    /// the window's content when the root has no tab bar for a sidebar.
    public func release() {
        exactView.session.presenter.sidebar.detach(self)
        NSLayoutConstraint.deactivate(pins)
        pins = []
        pinnedCover = nil
        guard exactView.superview === page.view else { return }
        exactView.removeFromSuperview()
        exactView.translatesAutoresizingMaskIntoConstraints = true
    }

    /// Whether this is the sidebar of a window that shows `presenter`'s view.
    func holds(_ presenter: Presenter) -> Bool {
        exactView.session.presenter === presenter && exactView.superview === page.view && view.window != nil
    }
    var collapsed: Bool { sidebarItem.isCollapsed }
    var sidebarWidth: CGFloat { list.view.frame.width }
    /// Whether a person could reach a row: the sidebar is in a window and showing.
    var rowsReachable: Bool { view.window != nil && !sidebarItem.isCollapsed }
    /// The source list (the tests').
    var table: NSTableView { list.table }

    func show(_ shown: SidebarHost.Shown) {
        list.show(shown)
        if shown.hidden, !hiddenForApp {
            // The person's own hide stands, and there is nothing of Exact's
            // to undo later. Else Exact hides it, unsaved: what is kept for
            // the next launch stays the person's.
            guard !sidebarItem.isCollapsed else { return }
            splitView.autosaveName = nil
            sidebarItem.isCollapsed = true
            hiddenForApp = true
        } else if !shown.hidden, hiddenForApp {
            sidebarItem.isCollapsed = false
            splitView.autosaveName = autosave
            hiddenForApp = false
        }
    }

    public override func validateUserInterfaceItem(_ item: NSValidatedUserInterfaceItem) -> Bool {
        if item.action == #selector(toggleSidebar(_:)), hiddenForApp { return false }
        return super.validateUserInterfaceItem(item)
    }
    public override func toggleSidebar(_ sender: Any?) {
        if !hiddenForApp { super.toggleSidebar(sender) }
    }
}

/// The source list: one row a root tab.
final class SidebarList: NSViewController, NSTableViewDataSource, NSTableViewDelegate {
    private unowned let host: SidebarHost
    let table = NSTableView()
    private var rows: [SidebarRow] = []
    /// The row a mouse-down pressed, and that click's number: its mouse-up
    /// is the same click, not a second press.
    private var pressed: (row: Int, click: Int)?

    init(host: SidebarHost) {
        self.host = host
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { nil }

    override func loadView() {
        let scroll = NSScrollView()
        let column = NSTableColumn(identifier: .init("tab"))
        column.resizingMask = .autoresizingMask
        table.addTableColumn(column)
        table.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle
        table.headerView = nil
        table.style = .sourceList
        // The person's Sidebar icon size (System Settings ▸ Appearance).
        table.rowSizeStyle = .default
        table.allowsMultipleSelection = false
        table.allowsEmptySelection = true
        table.dataSource = self
        table.delegate = self
        table.target = self
        table.action = #selector(clicked(_:))
        scroll.documentView = table
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        view = scroll
    }

    func show(_ shown: SidebarHost.Shown) {
        loadViewIfNeeded()
        if rows != shown.rows {
            rows = shown.rows
            table.reloadData()
        }
        let selection = shown.selected.map { IndexSet(integer: $0) } ?? IndexSet()
        if table.selectedRowIndexes != selection { table.selectRowIndexes(selection, byExtendingSelection: false) }
        if table.accessibilityLabel() != shown.label { table.setAccessibilityLabel(shown.label) }
    }

    func numberOfRows(in tableView: NSTableView) -> Int { rows.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        SidebarCell(rows[row])
    }

    /// The table never selects a row by itself. A person's choice (a click,
    /// an arrow key, a typed letter) presses the row's authored tab, here on
    /// the way down as a sidebar acts, and the app's answer selects the row
    /// in the batch that press makes.
    func tableView(_ tableView: NSTableView, shouldSelectRow row: Int) -> Bool {
        if let event = NSApp.currentEvent, event.type == .leftMouseDown { pressed = (row, event.eventNumber) }
        host.press(row: row)
        return false
    }

    func tableView(_ tableView: NSTableView, typeSelectStringFor tableColumn: NSTableColumn?, row: Int) -> String? {
        rows.indices.contains(row) ? rows[row].face.title : nil
    }

    /// A click on a row: one on the selected row presses its tab again, as
    /// a tab bar's item does (the app pops that tab to its root).
    @objc private func clicked(_ sender: NSTableView) {
        let row = sender.clickedRow, down = pressed
        pressed = nil
        guard row >= 0 else { return }
        // Only a mouse event has a number: the click that already pressed on its way down.
        if let down, down.row == row, let event = NSApp.currentEvent, event.type == .leftMouseUp, event.eventNumber == down.click { return }
        host.press(row: row)
    }
}

/// A row's badge: AppKit's own badge button, which draws a count in a
/// source list (secondary at rest, the selection's colour in a selected
/// row). It shows; it takes no click.
private final class SidebarBadge: NSButton {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

/// A row: the cell's own image view and text field, with no frame and no
/// constraint, so AppKit lays them out for the row size style, and a badge
/// at the trailing edge that the title stops before.
final class SidebarCell: NSTableCellView {
    private var badge: NSButton?

    init(_ row: SidebarRow) {
        super.init(frame: .zero)
        let title = NSTextField(labelWithString: row.face.title)
        title.lineBreakMode = .byTruncatingTail
        title.isEnabled = !row.face.disabled
        addSubview(title)
        textField = title
        if let name = row.face.base, let symbol = NSImage(systemSymbolName: name, accessibilityDescription: nil) {
            let image = NSImageView(image: symbol)
            image.isEnabled = !row.face.disabled
            addSubview(image)
            imageView = image
        }
        if let count = row.face.badge {
            let badge = SidebarBadge(title: count, target: nil, action: nil)
            badge.bezelStyle = .badge
            badge.refusesFirstResponder = true
            badge.setAccessibilityRole(.staticText)
            badge.translatesAutoresizingMaskIntoConstraints = false
            addSubview(badge)
            NSLayoutConstraint.activate([badge.trailingAnchor.constraint(equalTo: trailingAnchor), badge.centerYAnchor.constraint(equalTo: centerYAnchor)])
            self.badge = badge
        }
        setAccessibilityIdentifier(row.testId)
        // `aria-disabled` reads as disabled and still presses; `disabled` does neither.
        if row.face.disabled || row.face.accessibilityDisabled { setAccessibilityEnabled(false) }
    }
    required init?(coder: NSCoder) { nil }

    override func layout() {
        super.layout()
        // AppKit gives the title the cell's width: it ends where the badge begins.
        if let title = textField, let badge, title.frame.maxX > badge.frame.minX {
            title.frame.size.width = max(0, badge.frame.minX - title.frame.minX)
        }
    }
}
#endif

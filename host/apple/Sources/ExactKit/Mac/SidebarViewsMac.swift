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

/// The window's content while the app's root tab bar is its sidebar, or a
/// stack of it shows a list beside its detail (LLP 1115.002 D4,
/// SplitMac.swift): the sidebar, the page, and the detail's pane. The
/// window's owner makes one when `ExactView.wantsWindowSidebar` or
/// `wantsWindowSplit` and installs it (`install(in:)`); `release()` gives
/// the view back. The page's pane is the list's column while a stack
/// splits, and the detail's pane is collapsed while none does.
public final class ExactSidebarController: NSSplitViewController {
    public let exactView: ExactView
    /// The source list; nil in a window whose app has no root tab bar for one.
    private let list: SidebarList?
    private let page: NSViewController
    let detail: SplitDetail
    private let sidebarItem: NSSplitViewItem?
    private let pageItem: NSSplitViewItem
    private let detailItem: NSSplitViewItem
    private let autosave: String?
    private let minimumPageWidth: CGFloat
    private var pins: [NSLayoutConstraint] = []
    private var pinnedCover: Bool?
    /// The sidebar is hidden because the author hid the tab bar on this
    /// screen: Exact's own hide, which the toolbar's button does not undo.
    private var hiddenForApp = false
    /// Whether this window has a sidebar's column: the owner makes another
    /// controller when the app gains or loses its root tab bar.
    public var hasSidebar: Bool { sidebarItem != nil }

    /// `autosaveName` names where the sidebar's width, and whether the
    /// person hid it, are kept (AppKit's own autosave), and the list
    /// column's width beside it; `minimumPageWidth` is the narrowest the
    /// page goes, which with the columns' own minimums is the narrowest the
    /// window goes. `sidebar`: whether the app's root tab bar is the
    /// window's sidebar.
    public init(view exact: ExactView, autosaveName: String?, minimumPageWidth: CGFloat, sidebar: Bool = true) {
        let list = sidebar ? SidebarList(host: exact.session.presenter.sidebar) : nil, page = NSViewController()
        page.view = NSView()
        exactView = exact
        self.list = list
        self.page = page
        detail = SplitDetail(presenter: exact.session.presenter)
        sidebarItem = list.map { NSSplitViewItem(sidebarWithViewController: $0) }
        pageItem = NSSplitViewItem(viewController: page)
        detailItem = NSSplitViewItem(viewController: detail)
        autosave = autosaveName
        self.minimumPageWidth = minimumPageWidth
        super.init(nibName: nil, bundle: nil)
        if minimumPageWidth > 0 { pageItem.minimumThickness = minimumPageWidth }
        if let sidebarItem { addSplitViewItem(sidebarItem) }
        addSplitViewItem(pageItem)
        addSplitViewItem(detailItem)
        splitView.autosaveName = autosaveName
        // Whatever the autosave kept of the detail's pane, a stack that splits is what shows it.
        detailItem.isCollapsed = true
        exact.translatesAutoresizingMaskIntoConstraints = false
        page.view.addSubview(exact)
        pin()
        exact.session.presenter.sidebar.attach(self)
        exact.session.presenter.navigation.split.attach(self)
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
        detail.pin(cover: cover)
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
        // In its window now: a stack that splits shows its pane, with no batch to say so.
        exactView.session.presenter.navigation.split.sync()
    }

    /// The view leaves the pane, placed by frames again: the owner makes it
    /// the window's content when the root has no tab bar for a sidebar and
    /// no stack that splits.
    public func release() {
        exactView.session.presenter.navigation.split.detach(self)
        exactView.session.presenter.sidebar.detach(self)
        NSLayoutConstraint.deactivate(pins)
        pins = []
        pinnedCover = nil
        guard exactView.superview === page.view else { return }
        exactView.removeFromSuperview()
        exactView.translatesAutoresizingMaskIntoConstraints = true
    }

    /// Whether this is the content of a window that shows `presenter`'s view.
    func holds(_ presenter: Presenter) -> Bool {
        exactView.session.presenter === presenter && exactView.superview === page.view && view.window != nil
    }
    var collapsed: Bool { sidebarItem?.isCollapsed ?? true }
    var sidebarWidth: CGFloat { list?.view.frame.width ?? 0 }
    /// Whether a person could reach a row: the sidebar is in a window and showing.
    var rowsReachable: Bool { view.window != nil && sidebarItem?.isCollapsed == false }
    /// The source list (the tests').
    var table: NSTableView { list?.table ?? NSTableView() }

    func show(_ shown: SidebarHost.Shown) {
        guard let list, let sidebarItem else { return }
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
        if item.action == #selector(toggleSidebar(_:)), hiddenForApp || sidebarItem == nil { return false }
        return super.validateUserInterfaceItem(item)
    }
    public override func toggleSidebar(_ sender: Any?) {
        if !hiddenForApp, sidebarItem != nil { super.toggleSidebar(sender) }
    }

    // MARK: A list and its detail (LLP 1115.002 D4)

    /// AppKit's minimum for a content list's column (measured, LLP 1115.002
    /// §1: `NSSplitViewItem(contentListWithViewController:)`, 200 points at
    /// holding priority 255), given to the page's pane while it is a list's
    /// column. The item itself stays the page's: a split view item's kind
    /// is said when it is made, and a content list has a maximum, which the
    /// page alone in its window must not have.
    static let listMinimum: CGFloat = 200
    /// The list column's width where neither the author (`navigationSplit`)
    /// nor the person (a drag of the divider) has said one: the shared
    /// rule's own (LLP 1116.004 D1.2), as an AppKit list opens at the width
    /// its view names.
    static let listWidth: CGFloat = 320
    /// Whether the detail's pane shows beside the list's column.
    private(set) var splits = false
    /// The width the list's column was last given, by the rule or the person.
    private var placed: CGFloat?
    private var placing = false
    /// The divider between the list's column and the detail's pane.
    var listDivider: Int { sidebarItem == nil ? 0 : 1 }
    /// Where the detail's routes are shown.
    var detailPane: BlockView { detail.loadViewIfNeeded(); return detail.pane }
    var listColumnWidth: CGFloat { page.view.frame.width }
    var detailWidth: CGFloat { splits ? detail.view.frame.width : 0 }
    private var widthKey: String? { autosave.map { "Exact Split List Width " + $0 } }

    /// Show the detail's pane beside the page, which is then the list's
    /// column, `width` wide (nil: the person's, else the rule's); or with
    /// `on` false take the pane away, and the page has the window. `empty`:
    /// the pane shows no route, and says so.
    func split(_ on: Bool, width: CGFloat?, empty: Bool) {
        loadViewIfNeeded()
        detail.show(empty: on && empty)
        if on != splits {
            splits = on
            // Exact's own move of the divider, not the person's.
            placing = true
            defer { placing = false }
            let none = NSSplitViewItem.unspecifiedDimension
            pageItem.minimumThickness = on ? Self.listMinimum : minimumPageWidth > 0 ? minimumPageWidth : none
            pageItem.holdingPriority = on ? .init(255) : .defaultLow
            detailItem.minimumThickness = on && minimumPageWidth > 0 ? minimumPageWidth : none
            detailItem.isCollapsed = !on
            placed = nil
            // Now, not at the next display: the page is laid out at its column's width before it is seen.
            if view.window != nil { splitView.layoutSubtreeIfNeeded() }
        }
        guard on else { return }
        // The person's width, kept from a drag of the divider, over the author's and the rule's.
        let kept = chosen ?? widthKey.flatMap { UserDefaults.standard.object(forKey: $0) as? Double }.map { CGFloat($0) }
        let want = max(Self.listMinimum, kept ?? width ?? Self.listWidth)
        guard placed != want, view.window != nil, splitView.bounds.width > 0 else { return }
        placed = want
        placing = true
        defer { placing = false }
        // The column begins where the page's pane does: AppKit puts no divider's width after a sidebar.
        let start = splitView.convert(page.view.bounds, from: page.view).minX
        splitView.setPosition(start + want, ofDividerAt: listDivider)
        splitView.layoutSubtreeIfNeeded()
    }
    /// The width the person gave the list's column in this window.
    private var chosen: CGFloat?

    /// The person moved the divider between the list and its detail: the
    /// list's column keeps that width, here and at the next launch.
    public override func splitViewDidResizeSubviews(_ notification: Notification) {
        super.splitViewDidResizeSubviews(notification)
        guard splits, !placing, notification.userInfo?["NSSplitViewDividerIndex"] as? Int == listDivider else { return }
        let width = page.view.frame.width
        guard width >= Self.listMinimum else { return }
        placed = width
        chosen = width
        if let widthKey { UserDefaults.standard.set(Double(width), forKey: widthKey) }
    }
}

/// The detail's pane: a view that holds the routes shown beside the list,
/// each laid out in a containing block of its size (BlockHostMac.swift), on
/// the page's ground; and what a split view shows with nothing selected.
final class SplitDetail: NSViewController {
    let pane = BlockView(frame: .zero)
    private let ground: ModalGround
    private let nothing = NSTextField(labelWithString: "")
    private var pins: [NSLayoutConstraint] = []

    init(presenter: Presenter) {
        ground = ModalGround(presenter: presenter)
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { nil }

    override func loadView() {
        let view = NSView()
        ground.frame = view.bounds
        view.addSubview(ground)
        pane.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(pane)
        // AppKit's own words for nothing selected (its bindings' marker,
        // which it localizes), as a hand-built window's empty pane says
        // them: one quiet line in the middle, in the secondary label's colour.
        nothing.stringValue = Self.noSelection
        nothing.font = .preferredFont(forTextStyle: .title2)
        nothing.textColor = .secondaryLabelColor
        nothing.alignment = .center
        nothing.lineBreakMode = .byTruncatingTail
        nothing.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        nothing.translatesAutoresizingMaskIntoConstraints = false
        nothing.isHidden = true
        nothing.setAccessibilityIdentifier("exact.split.empty")
        view.addSubview(nothing)
        NSLayoutConstraint.activate([nothing.centerXAnchor.constraint(equalTo: pane.centerXAnchor), nothing.centerYAnchor.constraint(equalTo: pane.centerYAnchor),
                                     nothing.widthAnchor.constraint(lessThanOrEqualTo: pane.widthAnchor, constant: -32)])
        self.view = view
        pin(cover: false)
    }

    /// "No Selection", as AppKit says it.
    static let noSelection: String = {
        let said = Bundle(for: NSApplication.self).localizedString(forKey: "No Selection", value: "No Selection", table: nil)
        return said.isEmpty ? "No Selection" : said
    }()

    /// The pane under the title bar and toolbar, or the whole of it under
    /// `viewport-fit=cover`, as the page's view is in its own.
    func pin(cover: Bool) {
        guard isViewLoaded else { return }
        NSLayoutConstraint.deactivate(pins)
        let safe = view.safeAreaLayoutGuide
        pins = cover
            ? [pane.leadingAnchor.constraint(equalTo: view.leadingAnchor), pane.trailingAnchor.constraint(equalTo: view.trailingAnchor),
               pane.topAnchor.constraint(equalTo: view.topAnchor), pane.bottomAnchor.constraint(equalTo: view.bottomAnchor)]
            : [pane.leadingAnchor.constraint(equalTo: safe.leadingAnchor), pane.trailingAnchor.constraint(equalTo: safe.trailingAnchor),
               pane.topAnchor.constraint(equalTo: safe.topAnchor), pane.bottomAnchor.constraint(equalTo: safe.bottomAnchor)]
        NSLayoutConstraint.activate(pins)
    }

    /// Whether the pane says that nothing is selected: a split with no detail.
    func show(empty: Bool) {
        loadViewIfNeeded()
        if nothing.isHidden == empty { nothing.isHidden = !empty }
    }
    /// What the pane says while it shows no route.
    var saysNothing: String? { isViewLoaded && !nothing.isHidden ? nothing.stringValue : nil }
    func groundChanged() { if isViewLoaded { ground.needsDisplay = true } }
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

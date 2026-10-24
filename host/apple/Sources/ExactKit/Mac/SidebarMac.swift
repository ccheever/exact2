// @ref LLP 1115.002 D3 — the root tab bar as the window's sidebar. Where an
// app's root `tablist` is placed as the tabs recipe writes it (the web's
// rule for its own sidebar, LLP 1116.004 D1.1: a horizontal bar, a direct
// child of the root's flex column, in flow), a Mac window shows the tabs as
// AppKit's source list in a real sidebar beside the page. Which tablist is
// the root's is the kernel's rule (`Kernel::root_tabs`, LLP 1115.001 D11);
// what a tab shows is `TabFace`'s, as UIKit's tab bar item's is.
//
// This host says whether the sidebar is wanted and what it shows. The
// window's owner composes it (`ExactSidebarController`, SidebarViewsMac.swift):
// a view mounted in someone else's window claims no chrome. While a
// controller shows the rows, the authored tablist is hidden and its box is
// the platform's (`HostCovers`, D8.1), so the page simply narrows. A row a
// person chooses presses its authored tab: selection, history and a
// re-press at a tab's root stay the app's, and the selected row is the
// tab whose stack holds the route the root shows.
//
// Under the agent the authored tablist paints, as on iOS, and
// `--chrome platform` shows the sidebar, whose rows `tree` and `tap`
// address as the authored tabs.
#if os(macOS)
import AppKit

/// One row of the sidebar: an authored tab and what it shows.
struct SidebarRow: Equatable {
    let tab: UInt32
    let face: TabFace
    /// The tab's test id: its row's accessibility identifier.
    let testId: String?
}

final class SidebarHost {
    private unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// The root tab bar as a sidebar shows it.
    struct Shown {
        let tablist: NodeView
        /// The tablist's `aria-label`: the list's.
        let label: String?
        var rows: [SidebarRow]
        /// The tab whose stack holds the route the root shows.
        var selected: Int?
        /// The author hid the tab bar here (`display: none` on the tablist,
        /// the tabs recipe's way): the sidebar is hidden with it, as iOS
        /// hides its tab bar.
        var hidden: Bool
    }
    /// What a sidebar would show now; nil for an app with no root tab bar,
    /// one whose placement is the author's, or the agent's own chrome.
    private(set) var shown: Shown?
    var wanted: Bool { shown != nil }
    /// The window's owner hears when `wanted` changes, after the batch.
    var onWanted: (() -> Void)?
    /// The owner's split view, while it holds this session's view.
    private(set) weak var controller: ExactSidebarController?
    /// The authored tablist the sidebar stands for, hidden while it does.
    private weak var projected: NodeView?
    private var savedAccessibilityHidden = false
    private var syncing = false
    /// What the kernel was last asked of, and its answer: asked again only
    /// when the root's tabs as the views name them, or the tablists
    /// mounted, change — never on a batch that only scrolls.
    private var asked: [UInt32]?
    private var kernel: RootTabs?
    /// The tablist's subtree: a batch that touches none of it leaves the rows.
    private var watched: Set<UInt32> = []
    private var decision: String?

    private func decide(_ said: String?) {
        guard decision != said else { return }
        decision = said
        if let said { presenter.session?.log("sidebar: \(said)") }
    }

    /// LLP 1080.001 D3: the authored tablist hidden while the sidebar shows it.
    func hides(_ node: NodeView) -> Bool { projected === node }
    /// Whether the sidebar shows this tablist's tabs: no segmented control
    /// is made of it (SegmentsMac.swift).
    func adopts(_ tablist: NodeView) -> Bool { projected === tablist }
    /// The tablist and what it holds, while the sidebar stands for them:
    /// their boxes are no place to click.
    func suppresses(_ node: NodeView) -> Bool {
        guard let projected else { return false }
        return node === projected || node.isDescendant(of: projected)
    }
    /// Whether the window's toolbar holds the sidebar's button and its
    /// tracking separator: a sidebar is installed, and the author has not
    /// hidden the tab bar on this screen.
    func inToolbar(of window: NSWindow) -> Bool { installed(in: window) && shown?.hidden == false }
    /// Whether this window's content is a sidebar standing for the tablist.
    func installed(in window: NSWindow) -> Bool { projected != nil && controller?.view.window === window }
    /// Whether a sidebar is installed for this session (View ▸ Show Sidebar).
    var installed: Bool { controller != nil && projected != nil }
    /// The installed sidebar's split view, whose divider the toolbar's separator follows.
    var splitView: NSSplitView? { installed ? controller?.splitView : nil }

    func attach(_ controller: ExactSidebarController) {
        self.controller = controller
        sync()
    }
    func detach(_ controller: ExactSidebarController) {
        guard self.controller === controller else { return }
        self.controller = nil
        sync()
    }

    /// A restart: every view goes and the covers are forgotten with them
    /// (`HostCovers.reset`), so nothing is told to the runner that is leaving.
    func reset() {
        projected = nil
        shown = nil; asked = nil; kernel = nil; watched = []; decision = nil
    }

    /// Each batch, after the navigation's pass; and when a controller comes
    /// or goes, with no batch.
    func sync(_ batch: Batch? = nil) {
        guard !syncing else { return }
        syncing = true
        defer { syncing = false }
        let was = wanted, stood = projected, inBar = controller?.view.window.map(inToolbar(of:))
        read(batch)
        present()
        if wanted != was { presenter.afterBatch { [weak self] in self?.onWanted?() } }
        // With no batch to run them, the passes that read the projection.
        if batch == nil, stood !== projected || inBar != controller?.view.window.map(inToolbar(of:)) {
            presenter.afterBatch { [weak presenter] in
                presenter?.segments.sync(); presenter?.toolbar.sync(); presenter?.shortcuts.sync()
            }
        }
    }

    private func read(_ batch: Batch?) {
        guard !ExactEnv.authoredChrome, let root = presenter.navigation.root else { shown = nil; return }
        let host = root.tabs
        // What the kernel is asked by: the root, its tabs as the views name
        // them, else the tablists mounted (the kernel may find one of them).
        let signature = [root.node.id] + (host.map { [$0.tablist.id] + $0.tabs.map(\.id) + [0] + $0.panels.map(\.id) }
            ?? [0] + presenter.carrying("role:tablist").map(\.id))
        // The placement is read from the root's and the tablist's layout
        // rows, which do not cross: a batch that writes either one's props
        // or style, or that moves the root or a child of it (what a change
        // of the root's direction does, and a resize), asks again. A list
        // scrolling inside a tab does neither.
        let level = Set(root.node.container.subviews.compactMap { ($0 as? NodeView)?.id } + [root.node.id])
        let placed = batch?.ops.contains { op in
            guard let id = op.nodeID else { return false }
            switch op.op {
            case .style, .props: return id == root.node.id || id == host?.tablist.id
            case .frame: return level.contains(id)
            default: return false
            }
        } ?? false
        if signature != asked || placed {
            // Other tabs: their rows are read again.
            if signature != asked { watched = [] }
            asked = signature
            kernel = presenter.rootTabs?(root.node.id)
        }
        let names = { (ids: [UInt32]) in ids.map { "#\($0)" }.joined(separator: " ") }
        guard let host, let kernel = self.kernel else {
            shown = nil
            // One names root tabs and the other none: said, never guessed
            // past. A presenter fed by hand has no kernel to ask.
            if let kernel = self.kernel { decide("kept as authored: the kernel names root tabs (tablist #\(kernel.tablist): \(names(kernel.tabs))) that this host's views do not") }
            else if host != nil, presenter.rootTabs != nil { decide("kept as authored: this host's views name root tabs the kernel does not") }
            else { decide(nil) }
            return
        }
        guard kernel.tablist == host.tablist.id, kernel.tabs == host.tabs.map(\.id), kernel.panels == host.panels.map(\.id) else {
            shown = nil
            decide("kept as authored: the kernel's root tabs (tablist #\(kernel.tablist): \(names(kernel.tabs)); panels \(names(kernel.panels))) are not this host's (tablist #\(host.tablist.id): \(names(host.tabs.map(\.id))); panels \(names(host.panels.map(\.id))))")
            return
        }
        // An explicit opt-out keeps the bar authored, including when the
        // sidebar already stands for it (LLP 1075.003 §9.10).
        if host.tablist.wrote("appearance"), host.tablist.style["appearance"]?.string == "none" {
            shown = nil
            decide("kept as authored: the root tablist #\(host.tablist.id) writes appearance none")
            return
        }
        // The web's rule for a bar whose placement the app left unsaid: a
        // direct child of the root, not vertical, in flow in the root's flex
        // column, itself a row. A tablist the app arranged stays the app's.
        guard kernel.unsaid else {
            shown = nil
            decide("kept as authored: the root tablist #\(host.tablist.id) is placed by the app (not a row in flow in the root's flex column)")
            return
        }
        decide("the root tablist #\(host.tablist.id) is the window's sidebar (\(host.tabs.count) tabs)")
        let hidden = host.tablist.style["display"]?.string == "none"
        // Native button faces are viewless. Rebuild rows for every controls batch,
        // including unrelated buttons, and for changes in the tablist subtree.
        let touched = batch.map { $0.controls || $0.ops.contains { op in op.op != .frame && op.op != .present && op.nodeID.map(watched.contains) == true } } ?? true
        if let now = shown, now.tablist === host.tablist, now.rows.map(\.tab) == host.tabs.map(\.id), !watched.isEmpty, !touched {
            shown?.selected = root.at
            shown?.hidden = hidden
            return
        }
        watched = [host.tablist.id]
        func watch(_ view: NSView) {
            if let node = view as? NodeView { watched.insert(node.id) }
            view.subviews.forEach(watch)
        }
        host.tabs.forEach(watch)
        shown = Shown(tablist: host.tablist, label: host.tablist.authoredLabel,
                      rows: host.tabs.map { SidebarRow(tab: $0.id, face: TabFace($0), testId: $0.props["testId"]) },
                      selected: root.at, hidden: hidden)
    }

    /// Show what was read, where a controller holds this session's view,
    /// and say in the same step that the tablist's box is the platform's.
    private func present() {
        guard let shown, let controller, controller.hasSidebar, controller.holds(presenter) else { return unproject() }
        controller.show(shown)
        if projected !== shown.tablist {
            unprojectView()
            savedAccessibilityHidden = shown.tablist.isAccessibilityHidden()
            projected = shown.tablist
        }
        shown.tablist.isHidden = true
        shown.tablist.setAccessibilityHidden(true)
        presenter.covers.report("sidebar", [shown.tablist.id: .whole])
    }

    private func unprojectView() {
        guard let node = projected else { return }
        projected = nil
        guard presenter.views[node.id] === node else { return }
        node.isHidden = false
        node.setAccessibilityHidden(savedAccessibilityHidden)
    }

    private func unproject() {
        unprojectView()
        presenter.covers.report("sidebar", [:])
    }

    // MARK: Rows

    /// The row that stands for an authored tab, while the sidebar shows it.
    func row(of node: NodeView) -> Int? {
        guard projected != nil else { return nil }
        return shown?.rows.firstIndex { $0.tab == node.id }
    }
    /// Whether a tab this projection hides is shown through its row, as a
    /// tab a segment shows is: its press and its shortcut still stand. Nil
    /// for a view that is none of its tabs.
    func shows(_ node: NodeView) -> Bool? { row(of: node).map { _ in true } }

    /// A row chosen, by a person or by name: its authored tab is pressed,
    /// so the app's own handler runs and the next batch selects the row.
    @discardableResult func press(row: Int) -> Bool {
        guard let shown, shown.rows.indices.contains(row), !shown.rows[row].face.disabled,
              let tab = presenter.views[shown.rows[row].tab], !tab.inert else { return false }
        presenter.press(tab.id)
        return true
    }

    // MARK: The agent

    /// The driver's tap on an authored tab the sidebar stands for: pressed
    /// as its row presses it (host activation: the driver names the node,
    /// AppKit owns its pixels). Nil for a node that is no such tab; false
    /// where a person could not reach the row.
    func activate(_ node: NodeView) -> Bool? {
        guard let row = row(of: node) else { return nil }
        guard let controller, controller.rowsReachable else { return false }
        return press(row: row)
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        guard let controller, let shown, projected != nil else { return nil }
        if node === shown.tablist { return ["view": "NSTableView", "style": "sourceList", "rows": shown.rows.count, "collapsed": controller.collapsed] }
        guard let row = row(of: node) else { return nil }
        return ["view": "NSTableView", "row": row, "selected": shown.selected ?? NSNull(), "rows": shown.rows.count,
                "enabled": !shown.rows[row].face.disabled, "collapsed": controller.collapsed]
    }

    /// `state.window.sidebar` (the agent).
    var summary: [String: Any] {
        guard let controller, let shown, projected != nil else { return ["installed": false, "wanted": wanted] }
        return ["installed": true, "tablist": Int(shown.tablist.id), "collapsed": controller.collapsed, "hiddenByApp": shown.hidden,
                "width": Agent.r2(controller.sidebarWidth), "selected": shown.selected ?? NSNull(),
                "rows": shown.rows.map { row -> [String: Any] in
                    ["tab": Int(row.tab), "testId": row.testId ?? NSNull(), "title": row.face.title, "symbol": FaceSymbol.observed(row.face.base, icon: row.face.icon) ?? NSNull(),
                     "badge": row.face.badge ?? NSNull(), "enabled": !row.face.disabled]
                }]
    }
}
#endif

// @ref LLP 1115.002 D4 — a list and its detail are a split view. Where a
// stack's root route holds a list whose rows push (the runner's rule,
// `Runner::stack_split`, shared by every host that splits), a hand-built
// Mac window shows the list and the detail side by side: an
// `NSSplitViewController` whose divider, minimums and narrowing are
// AppKit's. On the Mac it is so at every window width (measured, D4): the
// window stops narrowing instead.
//
// **Which view is which pane.** The page's view is the list's column: the
// root route fills it as it filled the window, and the page narrows to the
// column as it narrows beside a sidebar. The stack's routes above the list,
// up to its first sheet, live in the detail's pane: each is a subtree of
// the one session laid out in a containing block the pane's size
// (`BlockHost`, D8.2), so nothing about a route changes but the box it is
// laid out in. The pane shows the top one. With none it shows the box the
// root route marks `navigationSplit="placeholder"` (LLP 1116.006 D2), as
// the web's shell does, or else says so in AppKit's words.
//
// **A row of the list draws no disclosure chevron** (D5): a table's row
// draws none in its cell (TableInkMac.swift), and a painted list's row (the
// grouped form, D6) draws none while its list is the list's column.
//
// **What a row's detail is.** A route some row of the list links to is
// that row's detail (the rule names the pairs), and so is one that came
// while a table's row was being pressed. The topmost such route at or under
// the one shown is where the pane begins: nothing leads Back from it, as a
// hand-built split's detail has no Back, and its row is the list's selected
// row. A route pushed from inside the detail is above it, and the window's
// Back leads from that (BackMac.swift, at the bar's leading end).
//
// This host says whether a split is wanted and what it shows. The window's
// owner composes it (`ExactSidebarController`, SidebarViewsMac.swift), as
// it composes the sidebar: a view mounted in someone else's window claims
// no chrome, and its stack pushes as before. Under the agent's own chrome
// the stack pushes too, and `--chrome platform` shows the split.
#if os(macOS)
import AppKit
import CExact

/// A stack that shows its root route as a list beside its detail, as the
/// runner finds it (`exact_stack_split`; `Runner::stack_split`): the rule
/// every host with a split reads, over the kernel's own tree. Only the Mac
/// reads it yet, so it is here, as a block's wire is the Mac's
/// (BlockHostMac.swift); the iPad's half moves it to shared Swift.
struct StackSplit: Equatable {
    /// The list whose row pushes; nil where only `navigationSplit` asked.
    let list: UInt32?
    /// The list column's width as `navigationSplit` names it.
    let width: Double?
    /// Each route above the root that is a row's detail, by the route's id,
    /// with that row's press.
    let rows: [UInt32: UInt32]

    init(list: UInt32?, width: Double?, rows: [UInt32: UInt32]) { (self.list, self.width, self.rows) = (list, width, rows) }

    init?(json: Data) {
        guard let o = try? JSONSerialization.jsonObject(with: json) as? [String: Any] else { return nil }
        let pairs = (o["rows"] as? [[Int]] ?? []).filter { $0.count == 2 }.map { (UInt32($0[0]), UInt32($0[1])) }
        self.init(list: (o["list"] as? Int).map { UInt32($0) }, width: (o["width"] as? NSNumber)?.doubleValue,
                  rows: Dictionary(pairs, uniquingKeysWith: { first, _ in first }))
    }
}

extension Runtime {
    /// Whether the stack whose root route is `view` splits, by the runner's
    /// rule; nil for one that pushes, and while the runner is busy.
    func stackSplit(_ view: UInt32, retaining list: UInt32?) -> StackSplit? {
        return on(busy: nil) {
            let len = exact_stack_split(rt, view, list ?? 0)
            return StackSplit(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
    }
}

final class SplitHost {
    private unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Whose the window is where two headers are in view (LLP 1115.002 §5
    /// Q10, ruled as a trial on 2026-10-12): the list's. Its heading is the
    /// window's title and the detail's stays the first line of its pane,
    /// which is what SwiftUI's three columns and AppKit's show. False is
    /// the other reading, SwiftUI's two columns: the detail's heading is
    /// the title and the list's stays over its rows. Either way each
    /// header's buttons are over its own column (`splitHeaderRoutes`,
    /// HeaderMac.swift; `WindowToolbarHost.present`).
    var listOwnsWindow = true { didSet { if listOwnsWindow != oldValue { sync() } } }
    /// A stack that pushes, not a split: the driver's own chrome (`--chrome
    /// agent`), as it paints the header, the tablist and a list's rows. A
    /// test of the pushed stack says so.
    var painted = ExactEnv.authoredChrome { didSet { if painted != oldValue { sync() } } }

    /// What the selected stack of the first navigation root shows as a
    /// list beside its detail.
    struct Shown {
        let root: NodeView
        /// The stack's root route: the list's column.
        let list: NodeView
        /// The list whose row pushes, by the rule; nil where only
        /// `navigationSplit` asked for the split.
        let table: UInt32?
        /// The list column's width as `navigationSplit` names it.
        let width: CGFloat?
        /// The pane's routes: the stack's above the list, up to its first sheet.
        let routes: [NodeView]
        /// The one the pane shows; nil with none.
        let detail: NodeView?
        /// Where in the stack the pane begins: the topmost route at or
        /// under the one shown that is a row's detail, else the first.
        let base: Int
        /// The press of the row whose detail that is, where it is known.
        let row: UInt32?
    }
    private(set) var shown: Shown?
    /// Whether a window shows the split: the stack splits, and the window's
    /// owner composed a controller that holds this session's view.
    var active: Bool { shown != nil && capable }
    /// The session's document spans both panes while its window is split.
    var agentRoot: NSView { active ? controller?.view ?? presenter.viewport : presenter.viewport }
    /// Captures keep the layout/pointer origin at the page's clip, and
    /// extend across the detail rather than including the window's chrome.
    var agentCaptureArea: CGRect {
        let root = agentRoot
        guard active else { return CGRect(origin: .zero, size: root.bounds.size) }
        let clip = presenter.viewport.contentView, rect = root.convert(clip.bounds, from: clip)
        let x = rect.minX - root.bounds.minX
        let y = root.isFlipped ? rect.minY - root.bounds.minY : root.bounds.maxY - rect.maxY
        return CGRect(x: x, y: y, width: root.bounds.width - x, height: clip.bounds.height)
    }
    private var capable: Bool { controller?.holds(presenter) == true }
    /// Whether the window's owner should compose a controller: a stack of
    /// this session has split. It stays wanted while the session runs, so a
    /// tab whose stack pushes only collapses the detail's pane.
    private(set) var wanted = false
    /// The window's owner hears when `wanted` changes, after the batch.
    var onWanted: (() -> Void)?
    private(set) weak var controller: ExactSidebarController?

    /// Each root route's prior list: the runner decides whether it may keep
    /// its column with no actionable rows, including static empty content.
    private var kept: [UInt32: UInt32] = [:]
    /// Each route that is a row's detail, with that row's press.
    private var rowOf: [UInt32: UInt32] = [:]
    /// The routes read as some pane's so far: one that comes while a row is
    /// being pressed is that row's.
    private var seen = Set<UInt32>()
    /// The row a table is pressing now (`TableFeed.pressRow`).
    var pressing: UInt32?
    /// The routes in the detail's pane, each with its stack's root route.
    private var held: [UInt32: (route: NodeView, list: UInt32)] = [:]
    /// The stacks that split no longer: their routes go back to the page.
    private var ended = Set<UInt32>()
    private var lastDetail: UInt32?
    private weak var groundList: NodeView?
    private var queued = false
    /// The root route's own placeholder while the pane shows it.
    private weak var placeholder: NodeView?
    /// The painted list whose chevrons are not drawn, and whether its rows
    /// may have changed since they were looked at (`NavigationHost.sync`).
    private weak var veiled: NodeView?
    var rowsChanged = true
    /// A commit restyled or moved the placeholder the pane shows (a margin
    /// or an inset changes its frame and sends no style): its shown style
    /// is asked for again (`Runner::show_split_placeholder`).
    private var placeholderRestyled = false
    func restyled(_ id: UInt32) { if id == placeholder?.id { placeholderRestyled = true } }
    /// The placeholder the pane shows, whose gates the navigation's pass
    /// reads by its authored ancestors (`NavigationHost.sync`).
    var shownPlaceholder: NodeView? { placeholder }

    /// Whether `route` is in the detail's pane: out of its tabpanel's
    /// views, so what hides and inerts the panel must reach it by name.
    func holds(_ route: NodeView) -> Bool { held[route.id]?.route === route }
    /// The ancestor's exit ended; its pane route can now leave the window.
    func retire(_ route: NodeView) {
        guard holds(route) else { return }
        held[route.id] = nil
        presenter.blocks.release(route)
        schedule()
    }
    /// Where in its stack the selected route's Back stops: the pane's first
    /// route, which has none (BackMac.swift). Zero for a stack that pushes.
    var base: Int { active ? shown?.base ?? 0 : 0 }

    func attach(_ controller: ExactSidebarController) {
        self.controller = controller
        sync()
    }
    func detach(_ controller: ExactSidebarController) {
        guard self.controller === controller else { return }
        self.controller = nil
        sync()
    }
    /// Read and present again with no batch: a controller came or went, or
    /// entered its window.
    func sync() {
        presenter.navigation.resync()
        // With no batch to run them, the passes that read what the window shows.
        presenter.afterBatch { [weak presenter] in presenter?.toolbar.sync(); presenter?.shortcuts.sync() }
    }
    /// The node children `home` holds with the pane's routes back in their
    /// authored places among them: the page's Tab order goes through the
    /// detail where its route was written, after the list. A route in a
    /// sheet's window is a Tab loop of its own, and stays out.
    func authored(_ mounted: [NodeView], in home: NSView) -> [NodeView] {
        guard !held.isEmpty || placeholder != nil else { return mounted }
        return presenter.blocks.authored(mounted, in: home).filter { node in mounted.contains { $0 === node } || holds(node) || node === placeholder }
    }
    /// The detail's pane, when `view` is in it.
    func pane(holding view: NSView?) -> NSView? {
        guard active, let view, let pane = controller?.detailPane, view.isDescendant(of: pane) else { return nil }
        return pane
    }
    /// Whether the keyboard is in the detail's pane: `view` is in it.
    func holdsFocus(_ view: NSView?) -> Bool { pane(holding: view) != nil }

    /// A restart: every view goes, the pane's routes with them, and the new
    /// runner knows no block.
    func reset() {
        for entry in held.values { entry.route.removeFromSuperview() }
        placeholder?.removeFromSuperview()
        held.removeAll(); rowOf.removeAll(); seen.removeAll(); kept.removeAll(); ended.removeAll()
        shown = nil; pressing = nil; lastDetail = nil; selected = nil; placeholder = nil; veiled = nil
        controller?.split(false, width: nil, empty: false)
    }

    private static func sheet(_ route: NodeView) -> Bool { ["modal", "fullscreen"].contains(route.props["navigationPresentation"] ?? "") }

    /// Read the runner's rule every batch: layout-only style changes can
    /// leave the wire's frames and paint unchanged. Only the prior list's
    /// identity is kept; its eligibility and any explicit width are fresh.
    private func ask(_ list: NodeView) -> StackSplit? {
        let answer = presenter.session?.runtime.stackSplit(list.id, retaining: kept[list.id])
        kept[list.id] = answer?.list
        return answer
    }

    /// Each batch, inside the navigation's pass, which gates the routes by
    /// the answer: what the selected stack of the first root shows.
    /// `routes` is that stack and `selected` the route its root shows.
    func read(root: NodeView, routes: [NodeView], selected: Int) {
        defer { schedule() }
        guard !painted, let list = routes.first, routes.indices.contains(selected), !Self.sheet(list),
              let rule = ask(list) else {
            if let list = routes.first, held.values.contains(where: { $0.list == list.id }) { ended.insert(list.id) }
            shown = nil
            return
        }
        ended.remove(list.id)
        let end = routes.indices.dropFirst().first { Self.sheet(routes[$0]) } ?? routes.count
        let pane = Array(routes[1..<end]), top = min(selected, end - 1)
        let detail = top >= 1 ? routes[top] : nil
        for (route, row) in rule.rows { rowOf[route] = row }
        // A route that comes, or stands, as a table presses a row is that row's.
        if let pressing, let detail, top == selected, !seen.contains(detail.id) || rule.rows[detail.id] == nil { rowOf[detail.id] = rule.rows[detail.id] ?? pressing }
        seen.formUnion(pane.map(\.id))
        let base = top >= 1 ? stride(from: top, through: 1, by: -1).first { rowOf[routes[$0].id] != nil } ?? 1 : 1
        shown = Shown(root: root, list: list, table: rule.list, width: rule.width.map { CGFloat($0) }, routes: pane, detail: detail,
                      base: base, row: top >= 1 ? rowOf[routes[base].id] : nil)
        if !wanted {
            wanted = true
            presenter.afterBatch { [weak self] in self?.onWanted?() }
        }
    }

    /// No navigation root, or no stack its key names: nothing splits.
    func readNone() {
        shown = nil
        schedule()
    }

    private func schedule() {
        guard !queued else { return }
        queued = true
        // After the batch: a route moves between views, and the kernel is told its block.
        presenter.afterBatch { [weak self] in self?.present() }
    }

    /// Show what was read, where a controller holds this session's view:
    /// the pane beside the list's column, each of the pane's routes in it
    /// and laid out at its size, and the list's selected row.
    private func present() {
        queued = false
        // A painted popover goes where its invoker now is: the pane or the page.
        defer { presenter.menus.layout() }
        for (id, entry) in held where presenter.views[id] !== entry.route && !presenter.isLeaving(entry.route) {
            held[id] = nil
            presenter.blocks.release(entry.route)
        }
        rowOf = rowOf.filter { presenter.views[$0.key] != nil }
        seen = seen.filter { presenter.views[$0] != nil }
        guard let controller, capable else {
            release { _ in true }
            showPlaceholder(nil); veil(nil)
            groundList?.sheetGroundChanged(); groundList = nil
            lastDetail = nil
            return
        }
        release { ended.contains($0) }
        ended.removeAll()
        guard let shown else {
            showPlaceholder(nil); veil(nil)
            if !held.values.contains(where: { presenter.isLeaving($0.route) }) { controller.split(false, width: nil, empty: false) }
            groundList?.sheetGroundChanged(); groundList = nil
            lastDetail = nil
            return
        }
        let own = shown.detail == nil ? ownPlaceholder(in: shown.list) : nil
        controller.split(true, width: shown.width, empty: shown.detail == nil && own == nil)
        if groundList !== shown.list {
            groundList = shown.list
            shown.list.sheetGroundChanged()
        }
        let pane = controller.detailPane
        pane.sizeChanged = { [weak self] size in self?.resized(size) }
        pane.positionChanged = { [weak self, weak pane] in
            if let pane { self?.resized(pane.bounds.size) }
        }
        // The pointer over the pane hovers what is under it, as over the page: in the key window (LLP 1115.000 D7.9).
        if pane.pointerTracking == nil {
            pane.pointerTracking = { [weak presenter] event in
                if event.window?.isKeyWindow == true || event.type == .mouseExited { presenter?.trackPointer(event) }
            }
        }
        for route in shown.routes where held[route.id]?.route !== route && presenter.views[route.id] === route {
            held[route.id] = (route, shown.list.id)
            presenter.blocks.show(route, in: pane, block: .size(pane.bounds.size), placement: placement(of: route, in: pane))
            route.sheetGroundChanged()
            presenter.keyViewLoopStale = true
        }
        showPlaceholder(own, in: pane)
        select(shown)
        veil(shown.table.flatMap { presenter.tables.feeds[$0] == nil ? presenter.views[$0] : nil })
        lastDetail = shown.detail?.id
    }

    /// The box `list`, the stack's root route, marks as the split's own
    /// placeholder: the first so marked whose route, by authored parents
    /// (the pane holds it apart), is `list`.
    private func ownPlaceholder(in list: NodeView) -> NodeView? {
        presenter.carrying("split:placeholder").first { node in
            presenter.logicalAncestry(of: node).dropFirst().first { ($0 as? NodeView)?.props["navigationKey"] != nil } === list
        }
    }

    /// Show `own` in the pane, laid out at its size, or with nil give the
    /// one shown back to its route, where it is `display: none` again.
    private func showPlaceholder(_ own: NodeView?, in pane: BlockView? = nil) {
        if let shown = placeholder, shown !== own || !presenter.blocks.owns(shown) {
            placeholder = nil
            presenter.blocks.release(shown)
            presenter.keyViewLoopStale = true
        }
        guard let own, let pane else { return }
        if placeholder === own {
            // Cleared first: the answer can be applied before this returns.
            if placeholderRestyled { placeholderRestyled = false; presenter.blocks.showAgain(own) }
            return
        }
        placeholder = own
        placeholderRestyled = false
        presenter.blocks.show(own, in: pane, block: .size(pane.bounds.size), placement: placement(of: own, in: pane), placeholder: true)
        presenter.keyViewLoopStale = true
    }

    /// A painted list in the list's column draws its rows' chevrons no
    /// more than a table does (TableInkMac.swift `rowDisclosure`); one that
    /// leaves the column draws them again. A table's rows are its feed's.
    private func veil(_ list: NodeView?) {
        if veiled !== list {
            veiled?.rowDisclosure(hidden: false)
            veiled = list
            rowsChanged = true
        }
        if rowsChanged { list?.rowDisclosure(hidden: true) }
        rowsChanged = false
    }

    private func release(where gone: (UInt32) -> Bool) {
        for (id, entry) in held where gone(entry.list) && !presenter.isLeaving(entry.route) {
            held[id] = nil
            presenter.blocks.release(entry.route)
            entry.route.sheetGroundChanged()
            presenter.keyViewLoopStale = true
        }
    }

    private func placement(of route: NodeView, in pane: NSView) -> BlockPlacement? {
        var parent = presenter.blocks.parent(of: route)
        while let view = parent {
            if let node = view as? NodeView, node.window === pane.window {
                return BlockPlacement(rect: node.convertNativePresentation(pane.bounds, from: pane), within: node.id)
            }
            parent = view.superview
        }
        return nil
    }

    private var samplingPlacement = false
    /// Runtime reads sample native movement even when the pane's model frame is unchanged.
    func samplePlacement() {
        guard !samplingPlacement, active, !held.isEmpty || placeholder != nil, let pane = controller?.detailPane else { return }
        samplingPlacement = true
        defer { samplingPlacement = false }
        resized(pane.bounds.size)
    }

    private func resized(_ size: CGSize) {
        let shown = held.values.map(\.route) + [placeholder].compactMap { $0 }.filter(presenter.blocks.owns)
        for route in shown { presenter.blocks.resize(route, .size(size), placement: controller.map { placement(of: route, in: $0.detailPane) } ?? nil) }
    }

    /// The list's selected row is the row whose detail the pane shows; when
    /// the detail goes, no row is.
    private func select(_ shown: Shown) {
        guard let table = shown.table, let feed = presenter.tables.feeds[table] else { return }
        if shown.detail == nil {
            if lastDetail != nil { feed.show(selected: nil) }
            selected = nil
        } else if let row = shown.row, selected?.detail != shown.detail?.id || selected?.row != row {
            // Told once for a detail and its row, and again until the row is mounted: not on every batch of a scroll.
            if feed.show(selected: row) { selected = (shown.detail?.id, row) }
        }
    }
    /// The detail and the row the table was last told of.
    private var selected: (detail: UInt32?, row: UInt32)?

    /// The person took the selection off the list's row (a Command-click on
    /// it): the pane shows nothing, so its routes go, as the stack goes back.
    func deselected(_ list: UInt32) {
        guard active, shown?.table == list else { return }
        closeDetail()
    }

    /// Leave the pane's routes, top first, each the way its stack goes
    /// back (`NavigationHost.dismiss`). True when the pane shows none after.
    @discardableResult func closeDetail() -> Bool {
        for _ in 0..<32 {
            guard active, let top = shown?.detail else { return true }
            if presenter.navigation.dismiss(top) != nil || shown?.detail === top { return false }
        }
        return shown?.detail == nil
    }

    /// The page's ground changed colour.
    func groundChanged() { controller?.detail.groundChanged() }

    // MARK: The agent

    /// `state.window.split` (the agent).
    var summary: [String: Any] {
        guard let controller, active, let shown else { return ["installed": false, "wanted": wanted] }
        return ["installed": true, "list": Int(shown.list.id), "table": shown.table.map { Int($0) } ?? NSNull(),
                "listWidth": Agent.r2(controller.listColumnWidth), "detailWidth": Agent.r2(controller.detailWidth),
                "authoredWidth": shown.width.map { Agent.r2($0) } ?? NSNull(),
                "routes": shown.routes.map { Int($0.id) }, "detail": shown.detail.map { Int($0.id) } ?? NSNull(),
                "base": shown.base, "row": shown.row.map { Int($0) } ?? NSNull(), "empty": controller.detail.saysNothing ?? NSNull(),
                "placeholder": placeholder.map { Int($0.id) } ?? NSNull()]
    }
}

extension Presenter {
    /// The view over `view` in the authored tree: an open popover's authored
    /// parent, and for a subtree the split's pane holds, its home in the
    /// page; else its superview. What inerts or hides a node from
    /// assistive technology is read up it.
    func authoredSuperview(of view: NSView) -> NSView? {
        if let pane = navigation.split.controller?.detailPane, view.superview === pane, navigation.split.active, let node = view as? NodeView {
            return blocks.parent(of: node)
        }
        return menus.parent(of: view)
    }
}

extension NodeView {
    /// The implicit form ground uses AppKit's desktop tint in both panes.
    var standsOnSplitGround: Bool {
        guard !wrote("background_color"), case .object(let role)? = style["background_color"],
              role["sys"]?.string == "windowBackgroundColor", let presenter,
              presenter.navigation.split.active else { return false }
        return isDescendant(of: presenter.viewport) || presenter.navigation.split.pane(holding: self) != nil
    }
}
#endif

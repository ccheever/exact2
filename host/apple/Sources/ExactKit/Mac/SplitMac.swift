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
// laid out in. The pane shows the top one. With none it says so, in
// AppKit's words.
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
    func stackSplit(_ view: UInt32) -> StackSplit? {
        return on(busy: nil) {
            let len = exact_stack_split(rt, view)
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
    static let listOwnsWindow = true
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
    private var capable: Bool { controller?.holds(presenter) == true }
    /// Whether the window's owner should compose a controller: a stack of
    /// this session has split. It stays wanted while the session runs, so a
    /// tab whose stack pushes only collapses the detail's pane.
    private(set) var wanted = false
    /// The window's owner hears when `wanted` changes, after the batch.
    var onWanted: (() -> Void)?
    private(set) weak var controller: ExactSidebarController?

    /// What the rule was last asked of, and its answer.
    private var asked: (route: UInt32, signature: [String], answer: StackSplit?)?
    /// Each root route's last split: a list that loses its last row keeps its column.
    private var kept: [UInt32: StackSplit] = [:]
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
    private var queued = false

    /// Whether `route` is in the detail's pane: out of its tabpanel's
    /// views, so what hides and inerts the panel must reach it by name.
    func holds(_ route: NodeView) -> Bool { held[route.id]?.route === route }
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
        guard !held.isEmpty else { return mounted }
        return presenter.blocks.authored(mounted, in: home).filter { node in mounted.contains { $0 === node } || holds(node) }
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
        held.removeAll(); rowOf.removeAll(); seen.removeAll(); kept.removeAll(); ended.removeAll()
        asked = nil; shown = nil; pressing = nil; lastDetail = nil; selected = nil
        controller?.split(false, width: nil, empty: false)
    }

    private static func sheet(_ route: NodeView) -> Bool { ["modal", "fullscreen"].contains(route.props["navigationPresentation"] ?? "") }

    /// The rule's answer for the stack whose root route is `list`, asked
    /// again when the stack's routes or the route's `navigationSplit`
    /// change, when a row is being pressed, or when the tree, properties or
    /// styles that the rule reads change. Frame-only scroll batches reuse it.
    private func ask(_ list: NodeView, _ routes: [NodeView], _ batch: Batch?) -> StackSplit? {
        let said = list.props["navigationSplit"] ?? ""
        guard said != "none" else { kept[list.id] = nil; asked = nil; return nil }
        let signature = [said] + routes.map { "\($0.id)" }
        let changed = batch?.ops.contains { [.create, .destroy, .children, .props, .style].contains($0.op) } ?? true
        if asked?.route != list.id || asked?.signature != signature || pressing != nil || changed {
            let answer = presenter.session?.runtime.stackSplit(list.id)
            asked = (list.id, signature, answer)
            if let answer { kept[list.id] = answer }
        }
        if let answer = asked?.answer { return answer }
        // Its list still stands, with no row that pushes now: the column stays.
        guard let old = kept[list.id], old.width != nil || old.list.map({ remains($0, under: list) }) == true else { kept[list.id] = nil; return nil }
        return StackSplit(list: old.list, width: old.width, rows: [:])
    }

    /// A retained list may lose its last row, but it must still belong to
    /// this route and be shown as authored. Navigation's own hiding is not
    /// an authored hide: the list stays beside the detail and under a sheet.
    private func remains(_ id: UInt32, under route: NodeView) -> Bool {
        guard let list = presenter.views[id] else { return false }
        var ancestor: NSView? = list
        while let view = ancestor {
            if let node = view as? NodeView, node.style["display"]?.string == "none" { return false }
            if view === route { return true }
            ancestor = view.superview
        }
        return false
    }

    /// Each batch, inside the navigation's pass, which gates the routes by
    /// the answer: what the selected stack of the first root shows.
    /// `routes` is that stack and `selected` the route its root shows.
    func read(root: NodeView, routes: [NodeView], selected: Int, batch: Batch?) {
        defer { schedule() }
        guard !painted, let list = routes.first, routes.indices.contains(selected), !Self.sheet(list),
              let rule = ask(list, routes, batch) else {
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
        held = held.filter { presenter.views[$0.key] === $0.value.route }
        rowOf = rowOf.filter { presenter.views[$0.key] != nil }
        seen = seen.filter { presenter.views[$0] != nil }
        guard let controller, capable else {
            release { _ in true }
            lastDetail = nil
            return
        }
        release { ended.contains($0) }
        ended.removeAll()
        guard let shown else {
            controller.split(false, width: nil, empty: false)
            lastDetail = nil
            return
        }
        controller.split(true, width: shown.width, empty: shown.detail == nil)
        let pane = controller.detailPane
        pane.sizeChanged = { [weak self] size in self?.resized(size) }
        for route in shown.routes where held[route.id]?.route !== route {
            held[route.id] = (route, shown.list.id)
            presenter.blocks.show(route, in: pane, block: .size(pane.bounds.size))
            presenter.keyViewLoopStale = true
        }
        select(shown)
        lastDetail = shown.detail?.id
    }

    private func release(where gone: (UInt32) -> Bool) {
        for (id, entry) in held where gone(entry.list) {
            held[id] = nil
            presenter.blocks.release(entry.route)
            presenter.keyViewLoopStale = true
        }
    }

    private func resized(_ size: CGSize) {
        for entry in held.values { presenter.blocks.resize(entry.route, .size(size)) }
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
                "base": shown.base, "row": shown.row.map { Int($0) } ?? NSNull(), "empty": controller.detail.saysNothing ?? NSNull()]
    }
}
#endif

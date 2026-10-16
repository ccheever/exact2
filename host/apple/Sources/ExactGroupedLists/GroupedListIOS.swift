// @ref LLP 1084 D5 — a grouped list (`list appearance="auto"`) is UIKit's
// own list: a UICollectionView with UICollectionLayoutListConfiguration in
// the list's box, its cells UIListContentConfiguration and cell accessories
// read from the kernel (`exact_grouped_list`), so separators, highlight,
// dynamic type and dark mode are the platform's. The collection is the list's
// only scroller. The authored sections stay hidden under it, a custom row's
// views are carried into their cell, and every batch sees that hierarchy.
import ExactKit

/// The grouped lists capability (LLP 1047.001 D4): an app's composition
/// links this module only when its plan has a grouped list, and installs it
/// before any session is made. On macOS a grouped list is its authored
/// nodes, and installing it does nothing.
public enum ExactGroupedLists {
    public static func install() {
        #if os(iOS) || os(tvOS)
        GroupedListsLink.make = { GroupedListHost($0) }
        GroupedListsLink.part = GroupedListHost.part
        #endif
    }
}

#if os(iOS) || os(tvOS)
import CExact
import UIKit

extension Runtime {
    /// A grouped list's sections and rows (LLP 1084 D4). The model is read
    /// on iOS; tvOS shows a grouped list's authored nodes.
    func groupedList(_ view: UInt32) -> GroupedListModel? {
        #if os(iOS)
        return on(busy: nil) {
            let len = exact_grouped_list(rt, view)
            return GroupedListModel(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
        #else
        return nil
        #endif
    }
}

/// A grouped list as the kernel reads it (`Kernel::grouped_list`).
struct GroupedListModel: Equatable {
    /// How a text is set where its author said so (`TextLook`, LLP 1116.003
    /// D7: author > platform); the defaults leave it the platform's.
    struct Look: Equatable {
        /// A font as the text's computed rows have it (`TextFont`).
        struct Font: Equatable {
            var size: Double, weight: Int, italic = false, family = 0, numeric = 0
        }
        /// The most lines it shows, 0 for any number.
        var lines = 0
        /// A line cut short ends in "…", not at the edge.
        var ellipsis = false
        /// Its colour, as a style row crosses; nil: the platform's.
        var color: BatchValue?
        /// Its font; nil: the platform's.
        var font: Font?
        init(lines: Int = 0, ellipsis: Bool = false, color: BatchValue? = nil, font: Font? = nil) {
            self.lines = lines; self.ellipsis = ellipsis; self.color = color; self.font = font
        }
        init(json: Any?) {
            let o = json as? [String: Any] ?? [:]
            lines = (o["lines"] as? NSNumber)?.intValue ?? 0
            ellipsis = o["ellipsis"] as? Bool ?? false
            color = o["color"].flatMap(Self.value)
            font = (o["font"] as? [String: Any]).flatMap { f in
                (f["size"] as? NSNumber).map { Font(size: $0.doubleValue, weight: (f["weight"] as? NSNumber)?.intValue ?? 400,
                                                    italic: f["italic"] as? Bool ?? false, family: (f["family"] as? NSNumber)?.intValue ?? 0,
                                                    numeric: (f["numeric"] as? NSNumber)?.intValue ?? 0) }
            }
        }
        /// A JSON value as the batch decodes one.
        private static func value(_ json: Any) -> BatchValue? {
            switch json {
            case let n as NSNumber: return CFGetTypeID(n) == CFBooleanGetTypeID() ? .bool(n.boolValue) : .number(n.doubleValue)
            case let s as String: return .string(s)
            case let a as [Any]: return .array(a.compactMap(value))
            case let o as [String: Any]: return .object(o.compactMapValues(value))
            default: return nil
            }
        }
        /// Onto a cell's or a header's text: only what the author said
        /// changes UIKit's. `engine` makes the author's font.
        func apply(to text: inout UIListContentConfiguration.TextProperties, engine: TextEngine?) {
            if lines > 0 {
                text.numberOfLines = lines
                text.lineBreakMode = ellipsis ? .byTruncatingTail : .byClipping
            }
            if let color {
                // Each appearance's side, a platform colour resolved by UIKit.
                text.color = UIColor { traits in
                    color.channels(dark: traits.userInterfaceStyle == .dark, contrast: traits.accessibilityContrast == .high)
                        .map(TextEngine.color) ?? .label
                }
            }
            if let font, let made = engine?.font(size: CGFloat(font.size), weight: font.weight, family: font.family, italic: font.italic, numeric: font.numeric) {
                text.font = made
            }
        }
    }
    struct Row: Equatable {
        var view: UInt32
        var custom = false
        var symbol: String?
        var title: String?
        var secondary: String?
        var subtitle = false
        var accessory = "none"
        var target: UInt32?
        /// A `radio` row's radio is checked (LLP 1115.000 D13.2).
        var checked = false
        var pressable = false
        var destructive = false
        var disabled = false
        var titleLook = Look()
        var secondaryLook = Look()
    }
    struct Section: Equatable {
        var view: UInt32
        var header: String?
        var footer: String?
        var headerLook = Look()
        var footerLook = Look()
        var rows: [Row]
        /// Whether the rows sit on a card; false for a transparent group
        /// (`section background-color="transparent"`): clear cells, no separators.
        var card = true
        /// The space above it when the author changed a margin there (the
        /// web's, collapsed); nil keeps UIKit's gap (`Kernel::grouped_list`).
        var spaceAbove: CGFloat?
    }
    var style = "inset-grouped"
    var sections: [Section] = []
    /// The space under the last section when its author set it.
    var spaceBelow: CGFloat?

    init(style: String = "inset-grouped", sections: [Section] = []) { self.style = style; self.sections = sections }
    init?(json: Data) {
        guard let o = try? JSONSerialization.jsonObject(with: json) as? [String: Any] else { return nil }
        style = o["style"] as? String ?? "inset-grouped"
        func id(_ v: Any?) -> UInt32? { (v as? NSNumber).map { $0.uint32Value } }
        sections = (o["sections"] as? [[String: Any]] ?? []).compactMap { s in
            guard let view = id(s["view"]) else { return nil }
            let rows: [Row] = (s["rows"] as? [[String: Any]] ?? []).compactMap { r in
                guard let view = id(r["view"]) else { return nil }
                return Row(view: view, custom: r["custom"] as? Bool ?? false, symbol: r["symbol"] as? String,
                           title: r["title"] as? String, secondary: r["secondary"] as? String,
                           subtitle: r["subtitle"] as? Bool ?? false, accessory: r["accessory"] as? String ?? "none",
                           target: id(r["target"]), checked: r["checked"] as? Bool ?? false, pressable: r["pressable"] as? Bool ?? false,
                           destructive: r["destructive"] as? Bool ?? false, disabled: r["disabled"] as? Bool ?? false,
                           titleLook: Look(json: r["titleLook"]), secondaryLook: Look(json: r["secondaryLook"]))
            }
            return Section(view: view, header: s["header"] as? String, footer: s["footer"] as? String,
                           headerLook: Look(json: s["headerLook"]), footerLook: Look(json: s["footerLook"]), rows: rows,
                           card: s["card"] as? Bool ?? true,
                           spaceAbove: (s["spaceAbove"] as? NSNumber).map { CGFloat($0.doubleValue) })
        }
        spaceBelow = (o["spaceBelow"] as? NSNumber).map { CGFloat($0.doubleValue) }
    }
    var appearance: UICollectionLayoutListConfiguration.Appearance {
        // tvOS has no inset grouped list.
        #if os(tvOS)
        switch style { case "plain": .plain; default: .grouped }
        #else
        switch style { case "plain": .plain; case "grouped": .grouped; default: .insetGrouped }
        #endif
    }
}

final class GroupedListHost: GroupedLists {
    unowned let presenter: Presenter
    private(set) var lists: [UInt32: GroupedListView] = [:]
    /// A list's model: the kernel's, through the runtime; a test's own.
    lazy var model: (UInt32) -> GroupedListModel? = { [unowned self] id in presenter.session?.runtime.groupedList(id) }
    init(_ presenter: Presenter) { self.presenter = presenter }

    func rows(for id: UInt32) -> [[UInt32]]? {
        #if os(iOS)
        return model(id)?.sections.map { $0.rows.map(\.view) }
        #else
        return nil
        #endif
    }

    /// Before a batch: every carried row back where the presenter put it.
    func prepare() { for list in lists.values { list.restore() } }

    /// After a batch. `changed` is the touched views and their ancestors; a
    /// list outside it keeps its model and only carries its rows again.
    func sync(changed: Set<UInt32>? = nil) {
        var live = Set<UInt32>()
        for owner in presenter.carrying("listStyle") where owner.kind == "list" {
            live.insert(owner.id)
            if let list = lists[owner.id], list.owner === owner, let changed, !changed.contains(owner.id) {
                list.mount()
                continue
            }
            guard let model = model(owner.id) else { continue }
            if let old = lists[owner.id], old.owner !== owner { old.remove(); lists[owner.id] = nil }
            let list = lists[owner.id] ?? GroupedListView(owner: owner, host: self)
            lists[owner.id] = list
            list.update(model)
        }
        for id in Array(lists.keys) where !live.contains(id) { lists.removeValue(forKey: id)?.remove() }
    }

    func reset() {
        for list in lists.values { list.remove() }
        lists.removeAll()
    }

    /// The list that draws `id`, a row or a row's toggle or detail button.
    private func list(drawing id: UInt32) -> (GroupedListView, GroupedListModel.Row)? {
        for list in lists.values {
            for section in list.model.sections {
                // A custom row is its own views, carried: the ordinary paths
                // find them, nested controls included.
                if let row = section.rows.first(where: { !$0.custom && ($0.view == id || $0.target == id) }) { return (list, row) }
            }
        }
        return nil
    }

    /// The collection view a wheel on `id` scrolls: the list's own, or the
    /// one drawing that row or any carried view in it.
    func scroller(for id: UInt32) -> UIScrollView? {
        if let list = lists[id] { return list.collection }
        if let (list, _) = list(drawing: id) { return list.collection }
        // A custom row or a view inside one, wherever its cell's reuse left
        // it: up its own views to a row a list's model names.
        var at: UIView? = presenter.views[id]
        while let view = at {
            if let node = view as? NodeView {
                if let list = lists[node.id] { return list.collection }
                if let list = lists.values.first(where: { $0.model.sections.contains { $0.rows.contains { $0.view == node.id } } }) {
                    return list.collection
                }
            }
            at = view.superview
        }
        return nil
    }

    /// Whether a list draws `id` (a row, or a row's toggle or detail
    /// button): the agent finds it in UIKit's cell, not the hidden row.
    func draws(_ id: UInt32) -> Bool { list(drawing: id) != nil }

    /// A route's screen is coming back into view: the rows its lists kept
    /// selected under the screen they pushed let go (LLP 1115.000 D11.3).
    func appearing(_ route: NodeView, alongside coordinator: UIViewControllerTransitionCoordinator?) {
        for list in lists.values where list.owner.isDescendant(of: route) { list.deselect(alongside: coordinator) }
    }

    /// The agent's reveal (r5 password generator): a row a list draws, or a
    /// view in a custom row, scrolled to by its cell in the collection view,
    /// as a finger scrolls the list. A row with no cell yet has its views in
    /// the hidden authored scroll, whose offset moves nothing on screen.
    func reveal(_ id: UInt32, port: (UIScrollView) -> CGRect) -> Bool? {
        if let (list, row) = list(drawing: id) { return list.reveal(row.view, inner: nil, port: port(list.collection)) }
        guard let node = presenter.views[id] else { return nil }
        var at: UIView? = node
        while let view = at {
            if let row = view as? NodeView, let list = lists.values.first(where: { $0.model.sections.contains { $0.rows.contains { $0.view == row.id && $0.custom } } }) {
                return list.reveal(row.id, inner: node.convert(node.bounds, to: row), port: port(list.collection))
            }
            at = view.superview
        }
        return nil
    }

    /// Where a real finger aimed at `node` lands, for a row this host draws
    /// (LLP 1080.000 D4): the row's cell; on its toggle's control, the cell's
    /// switch; on its detail button's, that accessory, with the collection
    /// view whose port the point must be in. A refusal when the cell is off
    /// that port or the accessory is not shown; nil for any node this host
    /// does not draw, which the ordinary aim takes.
    func shown(_ node: NodeView) -> GroupedAim? {
        guard let (list, row) = list(drawing: node.id) else { return nil }
        guard let cell = list.cell(row.view), list.collection.bounds.intersects(cell.frame) else {
            return .refused("its cell is outside the list's port; scroll it into view first")
        }
        // A radio's row is the radio: a finger on the cell checks it.
        guard row.view != node.id, row.accessory != "radio" else { return .view(cell, port: list.collection) }
        // Never the row in its place: its press is not the control's.
        guard let control = list.accessory(row.view) else {
            return .refused("its \(row.accessory == "toggle" ? "switch" : "detail button") is not shown")
        }
        return .view(control, port: list.collection)
    }

    /// The row and part of a grouped list's cell that `view` is in: the
    /// cell, its switch, or its detail button; nil outside every list cell.
    static func part(_ view: UIView?) -> [String: Any]? {
        var at = view, control: UIView?
        while let v = at, !(v is GroupedCell) {
            if v is UIControl { control = v }
            at = v.superview
        }
        guard let cell = at as? GroupedCell, let row = cell.row else { return nil }
        #if os(tvOS)
        let part = control == nil ? "cell" : "detail"
        #else
        let part = control == nil ? "cell" : control is UISwitch ? "switch" : "detail"
        #endif
        return ["row": Int(row), "part": part]
    }

    /// The agent's `tap` on a row UIKit draws: the cell's own selection, as a
    /// finger's; on a toggle's control, its switch's flip; on a detail
    /// button, its accessory's action. Refused, having done nothing, when
    /// the cell is off the list's port or something covers its middle. Nil
    /// for any node this host does not draw.
    func activate(_ node: NodeView) -> [String: Any]? {
        guard let (list, row) = list(drawing: node.id) else { return nil }
        let id = Int(node.id)
        guard let cell = list.cell(row.view), let window = cell.window,
              list.collection.bounds.intersects(cell.frame) else {
            return ["error": "tap #\(id): its cell is outside the list's port; a finger would scroll it into view first"]
        }
        let middle = cell.convert(CGPoint(x: cell.bounds.midX, y: cell.bounds.midY), to: window)
        guard list.collection.convert(list.collection.bounds, to: window).contains(middle),
              let hit = window.hitTest(middle, with: nil), hit === cell || hit.isDescendant(of: cell) else {
            return ["error": "tap #\(id): something covers its cell's middle"]
        }
        // The software keyboard is a window of its own, which the app's hit
        // test never sees (`Agent.obscured`).
        if let container = presenter.keyboardContainer,
           let top = presenter.keyboardGuideTop(in: container), middle.y >= container.convert(CGPoint(x: 0, y: top), to: nil).y {
            return ["error": "tap #\(id): its cell's middle is under the software keyboard; dismiss it or scroll the row above it first"]
        }
        let at = presenter.viewportScroll.convert(middle, from: nil)
        let reply: [String: Any] = ["tapped": id, "at": [Agent.r2(at.x), Agent.r2(at.y - presenter.viewportScroll.contentOffset.y)],
                                    "delivery": "host-activation", "native": "grouped-list"]
        if row.view != node.id, row.accessory == "toggle" {
            guard list.toggle(row.view) else { return ["error": "tap #\(id): the switch is disabled"] }
            return reply
        }
        if row.view != node.id, row.accessory == "radio" {
            return list.select(row.view) ? reply : ["error": "tap #\(id): the radio or its row is disabled"]
        }
        if row.view != node.id {
            guard list.detail(row.view) else { return ["error": "tap #\(id): the detail button is disabled"] }
            return reply
        }
        return list.select(row.view) ? reply : ["error": "tap #\(id): the row is disabled or not a button"]
    }

    /// `layout <id>`'s native fields for a list or a row it draws.
    func observation(_ node: NodeView) -> [String: Any]? {
        if let list = lists[node.id] {
            let c = list.collection, i = c.adjustedContentInset
            return ["view": "UICollectionView", "listStyle": list.model.style, "sections": list.model.sections.count,
                    "rows": list.model.sections.reduce(0) { $0 + $1.rows.count },
                    "offset": Agent.r2(c.contentOffset.y + i.top), "content": Agent.r2(c.contentSize.height),
                    "insets": [Agent.r2(i.top), Agent.r2(i.bottom)]]
        }
        guard let (list, row) = list(drawing: node.id), row.view == node.id else { return nil }
        var seen: [String: Any] = ["view": "UICollectionViewListCell", "accessory": row.accessory, "custom": row.custom]
        if let cell = list.cell(row.view) {
            let r = cell.convert(cell.bounds, to: presenter.viewportScroll)
            seen["frame"] = [Agent.r2(r.minX), Agent.r2(r.minY - presenter.viewportScroll.contentOffset.y), Agent.r2(r.width), Agent.r2(r.height)]
        }
        return seen
    }

    // LLP 1080.001 D3: what the inspection walk accounts for.
    func inspectionOwns(_ view: UIView) -> Bool { lists.values.contains { $0.collection === view || $0.owns(skirt: view) } }
    func hides(_ node: NodeView) -> Bool { lists.values.contains { $0.hidesAuthored(node) } }
    func projects(_ view: UIView) -> Bool { lists.values.contains { $0.carried.keys.contains((view as? NodeView)?.id ?? 0) } }

    func projectedRect(for node: NodeView, in scroll: UIScrollView) -> CGRect? {
        lists.values.first { $0.collection === scroll }?.projectedRect(for: node)
    }

    func scrollAnchors(for id: UInt32) -> [(node: NodeView, y: CGFloat)] {
        lists[id]?.scrollAnchors() ?? []
    }

    func logicalParent(of view: UIView) -> UIView? {
        guard let node = view as? NodeView, presenter.views[node.id] === node else { return nil }
        return lists.values.lazy.compactMap { $0.carried[node.id]?.parent }.first
    }
    var carriesRows: Bool { lists.values.contains { !$0.carried.isEmpty } }
}

/// One projected list.
final class GroupedListView: NSObject, UICollectionViewDelegate {
    let owner: NodeView
    unowned let host: GroupedListHost
    let collection: UICollectionView
    private(set) var model = GroupedListModel()
    private var source: UICollectionViewDiffableDataSource<UInt32, UInt32>!
    private var rows: [UInt32: GroupedListModel.Row] = [:]
    /// Each row's symbol tint as last configured.
    private var looks: [UInt32: BatchValue?] = [:]
    /// The theme's roles this list last drew with (LLP 1095 D4): a new table
    /// (an update, a dev reload) configures every row and the layout again.
    private var theme = ThemeRoles.generation
    #if !os(tvOS)
    private var switches: [UInt32: UISwitch] = [:]
    #endif
    /// Custom rows: where the presenter put each, to give it back.
    private(set) var carried: [UInt32: (parent: UIView, index: Int, frame: CGRect, inert: Bool)] = [:]
    /// The order they were carried in: given back last first, each index
    /// is where it was before the ones carried after it left.
    private var carriedOrder: [UInt32] = []
    /// The row whose switch's action is running.
    private var flipping: UInt32?
    /// A custom cell's height changed since the list was last laid out.
    private var resized = false
    /// Only the host-hidden bit: CSS display remains authoritative.
    private var hiddenRoots: [UInt32: (node: NodeView, hidden: Bool)] = [:]
    /// The list's padding and final footer space, independent of other insets.
    private var addedInsets = UIEdgeInsets.zero
    /// Custom rows whose cell was last sized to nothing: a row with no box
    /// yet, as one a commit inserts under a sheet has until the sheet gives
    /// back its geometry (LLP 1116.002 D2). UIKit never shows a cell with no
    /// height, so never configures it again to learn the row's later size:
    /// once the row has one, `remeasure` has UIKit measure it anew.
    private var unsized: Set<UInt32> = []
    #if os(iOS)
    /// The list's background past the viewport's edges (`hem`), and the
    /// list's colours it was painted from.
    private var skirt: UIView?
    private var skirtKey: [[Double]?] = []
    #endif
    /// Whether `view` is this list's background past the viewport's edges.
    func owns(skirt view: UIView) -> Bool {
        #if os(iOS)
        return skirt === view
        #else
        return false
        #endif
    }

    init(owner: NodeView, host: GroupedListHost) {
        self.owner = owner; self.host = host
        collection = GroupedCollectionView(frame: owner.bounds, collectionViewLayout: UICollectionViewFlowLayout())
        super.init()
        collection.setCollectionViewLayout(layout(), animated: false)
        collection.delegate = self
        collection.accessibilityIdentifier = owner.props["testId"]
        let cell = UICollectionView.CellRegistration<GroupedCell, UInt32> { [unowned self] cell, _, id in configure(cell, id) }
        let header = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(elementKind: UICollectionView.elementKindSectionHeader) { [unowned self] view, _, path in
            label(view, header: true, section: path.section)
        }
        let footer = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(elementKind: UICollectionView.elementKindSectionFooter) { [unowned self] view, _, path in
            label(view, header: false, section: path.section)
        }
        source = UICollectionViewDiffableDataSource(collectionView: collection) { view, path, id in
            view.dequeueConfiguredReusableCell(using: cell, for: path, item: id)
        }
        source.supplementaryViewProvider = { view, kind, path in
            kind == UICollectionView.elementKindSectionHeader
                ? view.dequeueConfiguredReusableSupplementary(using: header, for: path)
                : view.dequeueConfiguredReusableSupplementary(using: footer, for: path)
        }
    }

    private func section(at index: Int) -> GroupedListModel.Section? {
        model.sections.indices.contains(index) ? model.sections[index] : nil
    }

    /// A section's header or footer: UIKit's, in the author's colour and
    /// font where the author gave its text one (LLP 1116.003 D7).
    private func label(_ view: UICollectionViewListCell, header: Bool, section index: Int) {
        let s = section(at: index)
        var c = view.defaultContentConfiguration()
        c.text = header ? s?.header : s?.footer
        // UIKit's header and footer ink is the secondary label: the theme's where it replaces it.
        if let ink = ThemeRoles.color("secondary-label") { c.textProperties.color = ink }
        (header ? s?.headerLook : s?.footerLook)?.apply(to: &c.textProperties, engine: owner.text)
        view.contentConfiguration = c
        view.marginsIgnoreSafeArea()
    }

    /// Each section's header and footer exist only where it has the text.
    private func layout() -> UICollectionViewLayout {
        UICollectionViewCompositionalLayout { [weak self] index, environment in
            guard let self else { return nil }
            var c = UICollectionLayoutListConfiguration(appearance: model.appearance)
            // The theme's grouped ground where it replaces the role (LLP 1095 D4).
            if let ground = ThemeRoles.color("grouped-background") { c.backgroundColor = ground }
            let s = section(at: index)
            c.headerMode = s?.header == nil ? .none : .supplementary
            c.footerMode = s?.footer == nil ? .none : .supplementary
            #if !os(tvOS)
            c.showsSeparators = s?.card ?? true
            #endif
            #if os(iOS)
            // A row that names swipe actions swipes as a hand-built list's
            // cell does (LLP 1116.001 D1): UIKit's actions, a full swipe the first.
            c.leadingSwipeActionsConfigurationProvider = { [weak self] path in self?.swipeActions(at: path, leading: true) }
            c.trailingSwipeActionsConfigurationProvider = { [weak self] path in self?.swipeActions(at: path, leading: false) }
            #endif
            // An authored space above a header is above it: UIKit's top inset
            // there is the header-to-rows gap (§6.4).
            let space = s?.spaceAbove
            if let space, s?.header != nil { c.headerTopPadding = space }
            // The list's own background stays behind a card-less section's
            // clear cells: the inset card is the cells' background, not the
            // section's (a clear section background showed the route's white).
            let section = NSCollectionLayoutSection.list(using: c, layoutEnvironment: environment)
            // A boundary the author changed (Signal's 20-point sections) is
            // the web's space, all of it above the later section; a footer's
            // bottom inset is its own gap under the rows, and stays.
            var insets = section.contentInsets
            if let space, s?.header == nil { insets.top = space }
            if s?.footer == nil {
                if index + 1 < model.sections.count { if self.section(at: index + 1)?.spaceAbove != nil { insets.bottom = 0 } }
                else if let below = model.spaceBelow { insets.bottom = below }
            }
            section.contentInsets = insets
            // A plain list's footer stays under its rows, as its header
            // stays at their top: UIKit pins both by default.
            for item in section.boundarySupplementaryItems where item.elementKind == UICollectionView.elementKindSectionFooter {
                item.pinToVisibleBounds = false
            }
            return section
        }
    }

    #if os(iOS)
    /// A custom row's swipe actions on one side, as the swipe host offers
    /// them (`swipeContent`, LLP 1008 §9); nil for any other row.
    func swipeActions(at path: IndexPath, leading: Bool) -> UISwipeActionsConfiguration? {
        guard let id = source.itemIdentifier(for: path), rows[id]?.custom == true else { return nil }
        return host.presenter.swipeActions.configuration(row: id, leading: leading)
    }
    #endif

    func update(_ next: GroupedListModel) {
        let previous = model, old = rows
        let rethemed = theme != ThemeRoles.generation
        theme = ThemeRoles.generation
        let restyled = next.style != previous.style || rethemed
        model = next
        rows = Dictionary(next.sections.flatMap { $0.rows }.map { ($0.view, $0) }, uniquingKeysWith: { a, _ in a })
        #if !os(tvOS)
        switches = switches.filter { rows[$0.key]?.accessory == "toggle" }
        #endif
        var snapshot = NSDiffableDataSourceSnapshot<UInt32, UInt32>()
        var seen = Set<UInt32>()
        for s in next.sections where seen.insert(s.view).inserted {
            snapshot.appendSections([s.view])
            snapshot.appendItems(s.rows.map(\.view).filter { seen.insert($0).inserted }, toSection: s.view)
        }
        // Changed native parts configure the cell again. A custom row keeps
        // its own live views; mount carries their current box into the cell.
        // A standard row whose symbol's authored tint changed is too (D7).
        let tints: [UInt32: BatchValue?] = Dictionary(uniqueKeysWithValues: snapshot.itemIdentifiers.map { ($0, tint(of: $0)) })
        // A row whose card changed (its section gained or lost one, or the
        // row moved between sections) is configured again.
        let wasCarded = Dictionary(previous.sections.flatMap { s in s.rows.map { ($0.view, s.card) } }, uniquingKeysWith: { a, _ in a })
        let recarded = Set(next.sections.flatMap { s in s.rows.compactMap { r in wasCarded[r.view].flatMap { $0 != s.card ? r.view : nil } } })
        let changed = snapshot.itemIdentifiers.filter { id in rethemed || old[id].map { $0 != rows[id] || tints[id] != looks[id] || recarded.contains(id) } ?? false }
        looks = tints
        // A row whose switch is firing is reconfigured once its action has
        // returned: rebuilding its accessories would take the switch out of
        // its superview inside its own action.
        if let id = flipping, changed.contains(id) {
            DispatchQueue.main.async { [weak self] in
                guard let self, source.indexPath(for: id) != nil else { return }
                var later = source.snapshot()
                later.reconfigureItems([id])
                source.apply(later, animatingDifferences: false)
                mount()
            }
        }
        snapshot.reconfigureItems(changed.filter { $0 != flipping })
        // Headers and footers live in the sections' layout: one that came,
        // went or changed lays the list out again.
        let texts = previous.sections.map { [$0.header, $0.footer] } != next.sections.map { [$0.header, $0.footer] }
            || previous.sections.map { [$0.headerLook, $0.footerLook] } != next.sections.map { [$0.headerLook, $0.footerLook] }
            || !recarded.isEmpty // separators are the section layout's
        let regapped = previous.sections.map(\.spaceAbove) != next.sections.map(\.spaceAbove) || previous.spaceBelow != next.spaceBelow
        if restyled { collection.setCollectionViewLayout(layout(), animated: false) }
        // An unchanged snapshot can reset UIKit's estimated geometry and
        // adjust its offset after the core's pending scroll write. Remounting
        // still refreshes switches and custom boxes; changed parts reconfigure.
        if previous != next || !changed.isEmpty { source.apply(snapshot, animatingDifferences: false) }
        if texts {
            for kind in [UICollectionView.elementKindSectionHeader, UICollectionView.elementKindSectionFooter] {
                for path in collection.indexPathsForVisibleSupplementaryElements(ofKind: kind) {
                    guard let view = collection.supplementaryView(forElementKind: kind, at: path) as? UICollectionViewListCell else { continue }
                    label(view, header: kind == UICollectionView.elementKindSectionHeader, section: path.section)
                }
            }
            collection.collectionViewLayout.invalidateLayout()
        } else if regapped { collection.collectionViewLayout.invalidateLayout() }
        mount()
    }

    /// The collection owns scrolling; the original sections retain their
    /// authored layout and custom rows are carried into their cells.
    func mount() {
        if collection.superview !== owner { owner.addSubview(collection) }
        // Padding is room around the sections; final footer space is below
        // the footer. Apply only this module's delta, keeping refresh and
        // keyboard insets on the collection that owns them.
        let added = UIEdgeInsets(
            top: owner.padding("top"), left: 0,
            bottom: owner.padding("bottom")
                + (model.sections.last?.footer != nil ? model.spaceBelow ?? 0 : 0), right: 0)
        let roomChanged = added != addedInsets
        owner.mountNativeScrollView(collection, contentInsets: added)
        if roomChanged {
            let delta = UIEdgeInsets(top: added.top - addedInsets.top, left: 0,
                                     bottom: added.bottom - addedInsets.bottom, right: 0)
            addedInsets = added
            let atTop = abs(collection.contentOffset.y + collection.adjustedContentInset.top) < 0.5
            var inset = collection.contentInset
            inset.top += delta.top
            inset.bottom += delta.bottom
            assign(collection, \.contentInset, inset)
            if delta.top != 0, atTop, !(collection.isTracking || collection.isDragging || collection.isDecelerating) {
                collection.contentOffset.y = -collection.adjustedContentInset.top
            }
            var indicator = collection.verticalScrollIndicatorInsets
            indicator.top += delta.top
            indicator.bottom += delta.bottom
            assign(collection, \.verticalScrollIndicatorInsets, indicator)
        }
        // A short settings list bounces, as UIKit's collection default would not.
        assign(collection, \.alwaysBounceVertical, owner.scrollsVertically)
        assign(collection, \.bounces, owner.style["overscroll_behavior_y"]?.string != "none")
        assign(collection, \.frame, owner.bounds)
        for cell in collection.visibleCells {
            guard let cell = cell as? GroupedCell, let id = cell.row else { continue }
            carry(id, into: cell)
            interact(cell, id)
        }
        // A switch shows its control as it now stands, which a batch may
        // change without changing the row.
        #if !os(tvOS)
        for (id, toggle) in switches { refresh(id, toggle) }
        #endif
        for case let section as NodeView in owner.container.subviews {
            if hiddenRoots[section.id] == nil { hiddenRoots[section.id] = (section, section.hiddenByHost) }
            section.isHidden = true
        }
        remeasure()
        #if os(iOS)
        hem()
        #endif
        if resized { resized = false; collection.collectionViewLayout.invalidateLayout() }
        // Settle UIKit's dirty layout before the core applies pending offsets.
        // This is a no-op when unchanged, including a simple remount.
        collection.layoutIfNeeded()
        // The CSS origin can change without moving UIKit's physical offset.
        if roomChanged { owner.scrollViewDidScroll(collection) }
    }

    func hidesAuthored(_ node: NodeView) -> Bool {
        hiddenRoots.values.contains { node === $0.node || node.isDescendant(of: $0.node) }
    }

    /// A standard row has a native frame even when its original view is hidden
    /// or its cell is offscreen. A carried descendant keeps its own exact box.
    func projectedRect(for node: NodeView) -> CGRect? {
        if node.isDescendant(of: collection) { return node.convert(node.bounds, to: collection) }
        var at: UIView? = node
        while let view = at {
            if let n = view as? NodeView, let row = rows[n.id] {
                if row.target == node.id, let control = accessory(row.view) {
                    return control.convert(control.bounds, to: collection)
                }
                guard let frame = source.indexPath(for: row.view).flatMap({ collection.layoutAttributesForItem(at: $0)?.frame }) else { return nil }
                if row.custom {
                    // The same row x used by carry; nested scroll offsets are
                    // already represented by this descendant-in-row conversion.
                    return node.convert(node.bounds, to: n).offsetBy(dx: frame.minX + n.frame.minX, dy: frame.minY)
                }
                return frame
            }
            at = view.superview
        }
        return nil
    }

    func scrollAnchors() -> [(node: NodeView, y: CGFloat)] {
        let port = collection.bounds.inset(by: owner.scrollPortInsets(collection))
        var whole: [(node: NodeView, y: CGFloat)] = [], partial: [(node: NodeView, y: CGFloat)] = []
        for path in collection.indexPathsForVisibleItems.sorted() {
            guard let id = source.itemIdentifier(for: path), let node = host.presenter.views[id],
                  let frame = collection.layoutAttributesForItem(at: path)?.frame,
                  frame.width > 0, frame.height > 0, frame.intersects(port) else { continue }
            // A sliver of a row above the reader is only a fallback: its own
            // height can grow while its top stays fixed. Prefer a whole row.
            if port.contains(frame) { whole.append((node, frame.minY)) }
            else { partial.append((node, frame.minY)) }
        }
        whole.append(contentsOf: partial)
        return whole
    }

    #if os(iOS)
    /// LLP 1116.004 D2 (chat-rooms, round 6): where the list's box meets an
    /// edge of a viewport the safe area keeps short of the screen's — a page
    /// with no navigation container keeps its content inside the safe area —
    /// the list's background goes on to the screen's edge, under the home
    /// indicator or the status bar, as a collection view pinned to its view's
    /// edges draws its grouped background there. The canvas there was the
    /// root's white under a grey list. Behind the viewport, so only what lies
    /// past its edges shows. Not while the keyboard ends the viewport (the
    /// keys cover that edge), nor on a page that scrolls, where the list does
    /// not stay at the edge.
    private func hem() {
        let presenter = host.presenter, viewport = presenter.viewport
        guard let container = viewport.superview, owner.window != nil, owner.isDescendant(of: viewport),
              presenter.keyboardInset == 0, viewport.contentSize.height <= viewport.bounds.height + 0.5,
              viewport.contentSize.width <= viewport.bounds.width + 0.5 else { skirt?.isHidden = true; return }
        let port = viewport.frame, whole = container.bounds, box = owner.convert(owner.bounds, to: container)
        let meets = { (a: CGFloat, b: CGFloat) in abs(a - b) < 0.5 }
        let top = meets(box.minY, port.minY) && port.minY > whole.minY ? whole.minY : box.minY
        let bottom = meets(box.maxY, port.maxY) && port.maxY < whole.maxY ? whole.maxY : box.maxY
        let left = meets(box.minX, port.minX) && port.minX > whole.minX ? whole.minX : box.minX
        let right = meets(box.maxX, port.maxX) && port.maxX < whole.maxX ? whole.maxX : box.maxX
        let reach = CGRect(x: left, y: top, width: right - left, height: bottom - top)
        guard reach != box, !owner.isHidden else { skirt?.isHidden = true; return }
        let view = skirt ?? {
            let made = UIView()
            made.isUserInteractionEnabled = false
            skirt = made
            return made
        }()
        // The list's own background in each appearance: the sheet's grouped grey, or the author's.
        let sides = [owner.channels("background_color", dark: false), owner.channels("background_color", dark: true)]
        if sides != skirtKey || view.backgroundColor == nil {
            skirtKey = sides
            view.backgroundColor = UIColor { traits in sides[traits.userInterfaceStyle == .dark ? 1 : 0].map(TextEngine.color) ?? ThemeRoles.color("grouped-background", or: .systemGroupedBackground) }
        }
        if view.superview !== container || container.subviews.firstIndex(of: view).map({ $0 > (container.subviews.firstIndex(of: viewport) ?? 0) }) == true {
            container.insertSubview(view, belowSubview: viewport)
        }
        assign(view, \.frame, reach)
        assign(view, \.isHidden, false)
    }
    #endif

    func cell(_ id: UInt32) -> UICollectionViewCell? {
        source.indexPath(for: id).flatMap { collection.cellForItem(at: $0) }
    }

    /// Row `id`'s middle, or the middle of `inner` (a box in the row's own
    /// space), scrolled to the middle of `port` when outside it, unanimated
    /// and inside the scrollable range; its cell made and its views carried.
    /// A row never shown has an estimated place until its cell sizes it, so
    /// it is measured again after each move, at most three times.
    func reveal(_ id: UInt32, inner: CGRect?, port: CGRect) -> Bool {
        guard let path = source.indexPath(for: id) else { return false }
        // The port as it sits in the bounds, which move as the list scrolls.
        let seen = port.offsetBy(dx: 0, dy: -collection.bounds.minY)
        var scrolled = false
        for _ in 0..<3 {
            collection.layoutIfNeeded()
            guard let frame = collection.layoutAttributesForItem(at: path)?.frame else { break }
            let mid = CGPoint(x: frame.midX, y: frame.minY + (inner?.midY ?? frame.height / 2))
            let now = seen.offsetBy(dx: 0, dy: collection.bounds.minY)
            if now.minY <= mid.y && mid.y < now.maxY { break }
            let i = collection.adjustedContentInset
            let y = min(max(collection.contentOffset.y + mid.y - now.midY, -i.top), max(-i.top, collection.contentSize.height + i.bottom - collection.bounds.height))
            guard y != collection.contentOffset.y else { break }
            collection.contentOffset.y = y
            scrolled = true
        }
        collection.layoutIfNeeded()
        mount()
        return scrolled
    }

    /// The control a row's accessory shows: its switch, or UIKit's detail
    /// button (a control in the cell outside its content). Nil for any other.
    func accessory(_ id: UInt32) -> UIView? {
        switch rows[id]?.accessory {
        #if !os(tvOS)
        case "toggle": return switches[id].flatMap { $0.window != nil ? $0 : nil }
        #endif
        case "detail":
            guard let cell = cell(id) as? UICollectionViewListCell else { return nil }
            func control(_ v: UIView) -> UIControl? {
                if v === cell.contentView { return nil }
                return (v as? UIControl) ?? v.subviews.lazy.compactMap(control).first
            }
            return control(cell)
        default: return nil
        }
    }

    /// Whether the section holding `id` draws its card.
    private func card(of id: UInt32) -> Bool {
        model.sections.first { $0.rows.contains { $0.view == id } }?.card ?? true
    }

    private func configure(_ cell: GroupedCell, _ id: UInt32) {
        cell.row = id
        guard let row = rows[id] else { return }
        interact(cell, id)
        // A card-less section's rows sit on the list's background; a
        // pressable standard row still shows UIKit's highlight while pressed.
        if card(of: id) {
            cell.configurationUpdateHandler = nil
            var card = cell.defaultBackgroundConfiguration()
            if let fill = ThemeRoles.color("secondary-grouped-background") { card.backgroundColor = fill }
            cell.backgroundConfiguration = card
        } else {
            let highlights = row.pressable && !row.custom && !row.disabled
            cell.configurationUpdateHandler = { cell, state in
                cell.backgroundConfiguration = highlights && (state.isHighlighted || state.isSelected)
                    ? cell.defaultBackgroundConfiguration().updated(for: state) : .clear()
            }
            cell.backgroundConfiguration = .clear()
        }
        if row.custom {
            cell.contentConfiguration = nil
            cell.marginsIgnoreSafeArea()
            cell.accessories = []
            carry(id, into: cell)
            return
        }
        for case let carried as NodeView in cell.contentView.subviews { carried.removeFromSuperview() }
        cell.height = nil
        var c: UIListContentConfiguration = row.subtitle ? .subtitleCell() : row.secondary != nil ? .valueCell() : .cell()
        c.image = row.symbol.flatMap { UIImage(systemName: $0) }
        // The symbol's tint as the sheet draws it (D7): the author's over the
        // sheet's own, each side of a light-dark() pair for its appearance.
        // The accent (`AccentColor`, the sheet's own) is UIKit's: the cell's
        // tint, which an authored accent-color sets (LLP 1116.004 D2).
        if let symbol = symbolView(of: id), let value = symbol.style["tint_color"], !symbol.followsTint("tint_color") {
            c.imageProperties.tintColor = UIColor { traits in
                value.channels(dark: traits.userInterfaceStyle == .dark).map(TextEngine.color) ?? .tintColor
            }
        }
        c.text = row.title
        c.secondaryText = row.secondary
        // A `line-clamp` or a line kept whole, and the author's colour and
        // font, as outside a list (D7); a destructive row's own colour is
        // red where its author gave none.
        // Where the author gave none, the theme's label colours (LLP 1095 D4).
        if row.titleLook.color == nil, let ink = ThemeRoles.color("label") { c.textProperties.color = ink }
        if row.secondaryLook.color == nil, let ink = ThemeRoles.color("secondary-label") { c.secondaryTextProperties.color = ink }
        row.titleLook.apply(to: &c.textProperties, engine: owner.text)
        row.secondaryLook.apply(to: &c.secondaryTextProperties, engine: owner.text)
        if row.destructive {
            let red = ThemeRoles.color("system-red", or: .systemRed)
            if row.titleLook.color == nil { c.textProperties.color = red }
            c.imageProperties.tintColor = red
        }
        if row.disabled {
            let gray = ThemeRoles.color("tertiary-label", or: .tertiaryLabel)
            c.textProperties.color = gray
            c.secondaryTextProperties.color = gray
            c.imageProperties.tintColor = gray
        }
        cell.contentConfiguration = c
        cell.marginsIgnoreSafeArea()
        cell.accessories = accessories(row)
    }

    private func accessories(_ row: GroupedListModel.Row) -> [UICellAccessory] {
        switch row.accessory {
        case "disclosure": return [.disclosureIndicator()]
        case "checkmark": return [.checkmark()]
        // A radio is the chosen row's checkmark, as a list of choices in
        // Settings has it (LLP 1115.000 D13.2); UIKit has no radio.
        case "radio": return row.checked ? [.checkmark()] : []
        case "detail":
            let id = row.view
            return [.detail(displayed: .always) { [weak self] in _ = self?.detail(id) }]
        #if os(tvOS)
        // tvOS has no switch: a toggle row shows its state as a checkmark.
        case "toggle":
            let on = row.target.flatMap { host.presenter.views[$0] }?.props["checked"] == "true"
            return on ? [.checkmark()] : []
        #else
        case "toggle":
            let id = row.view
            let toggle = switches[id] ?? {
                let s = UISwitch()
                s.addAction(UIAction { [weak self, weak s] _ in
                    guard let self, let s else { return }
                    flip(id, s)
                }, for: .valueChanged)
                switches[id] = s
                return s
            }()
            refresh(id, toggle)
            // A custom-view accessory takes a view with no superview, and a
            // reconfigured cell builds its accessories again.
            toggle.removeFromSuperview()
            return [.customView(configuration: .init(customView: toggle, placement: .trailing()))]
        #endif
        default: return []
        }
    }

    #if os(tvOS)
    /// The agent's tap on a toggle row: flips its current control's `checked`.
    func toggle(_ id: UInt32) -> Bool {
        guard let target = rows[id]?.target, let node = host.presenter.views[target],
              rows[id]?.disabled == false, !node.disabled, !node.inert else { return false }
        host.presenter.checked(target, node.props["checked"] != "true")
        return true
    }
    #else
    /// A row's switch as its control now stands: the row's current target
    /// (an `if` may have replaced it), its committed `checked`, whether it
    /// or the row is disabled, its accent and name.
    private func refresh(_ id: UInt32, _ toggle: UISwitch) {
        let node = rows[id]?.target.flatMap { host.presenter.views[$0] }
        let on = node?.props["checked"] == "true"
        if toggle.isOn != on { toggle.setOn(on, animated: toggle.window != nil && !ExactEnv.agentFreezes) }
        assign(toggle, \.isEnabled, node != nil && rows[id]?.disabled == false && node?.disabled == false && node?.inert == false)
        if !toggle.isEnabled || node?.ariaUnavailable == true { toggle.accessibilityTraits.insert(.notEnabled) }
        else { toggle.accessibilityTraits.remove(.notEnabled) }
        assign(toggle, \.onTintColor, node?.channels("accent_color").map { TextEngine.color($0) })
        assign(toggle, \.accessibilityIdentifier, node?.props["testId"])
    }

    /// The switch flips the row's current control's `checked`, as that
    /// control does (LLP 1069.001 D4): the committed state is authoritative.
    private func flip(_ id: UInt32, _ toggle: UISwitch) {
        guard let target = rows[id]?.target, let node = host.presenter.views[target],
              rows[id]?.disabled == false, !node.disabled, !node.inert else { refresh(id, toggle); return }
        flipping = id
        host.presenter.checked(target, toggle.isOn)
        flipping = nil
        refresh(id, toggle)
    }

    /// The agent's tap on a row's switch: what a finger's flip does.
    func toggle(_ id: UInt32) -> Bool {
        guard let s = switches[id], s.isEnabled else { return false }
        s.setOn(!s.isOn, animated: false)
        s.sendActions(for: .valueChanged)
        return true
    }
    #endif

    /// The detail button's press, the row's current button, unless it or
    /// the row is disabled.
    func detail(_ id: UInt32) -> Bool {
        guard let row = rows[id], row.accessory == "detail", !row.disabled, let target = row.target,
              let node = host.presenter.views[target], !node.disabled, !node.inert else { return false }
        host.presenter.press(target)
        return true
    }

    /// The view of the symbol the model named: the row's first shown child,
    /// when it is an image of that symbol (`kernel/src/grouped.rs`).
    private func symbolView(of id: UInt32) -> NodeView? {
        guard let symbol = rows[id]?.symbol, !(rows[id]?.custom ?? true), let row = host.presenter.views[id] else { return nil }
        let first = row.container.subviews.lazy.compactMap { $0 as? NodeView }.first { $0.style["display"]?.string != "none" }
        guard let image = first, image.kind == "image", let source = image.props["imageSource"], source.hasPrefix("symbol:") else { return nil }
        let name = source.dropFirst("symbol:".count)
        return name.hasPrefix("sf/") && name.dropFirst(3) != symbol ? nil : image
    }
    private func tint(of id: UInt32) -> BatchValue? {
        symbolView(of: id)?.style["tint_color"]
    }

    /// A custom row's own views in its cell, at the row's place in its
    /// group, as tall as the row less the separator the cell draws instead.
    private func carry(_ id: UInt32, into cell: GroupedCell) {
        guard rows[id]?.custom == true, let row = host.presenter.views[id] else { return }
        for case let other as NodeView in cell.contentView.subviews where other !== row { other.removeFromSuperview() }
        if carried[id] == nil, let parent = row.superview {
            // Its inertness is its authored ancestors', which a cell is not.
            carried[id] = (parent, parent.subviews.firstIndex(of: row) ?? 0, row.frame, row.inert)
            carriedOrder.append(id)
        }
        guard let place = carried[id] else { return }
        let height = cellHeight(id, row: place.frame.height)
        // Never invalidated here: a cell is configured inside the data
        // source's update; `mount` lays the list out after it.
        if cell.height != height { cell.height = height; resized = true }
        if height > 0 { unsized.remove(id) } else { unsized.insert(id) }
        cell.contentView.clipsToBounds = true
        if row.superview !== cell.contentView {
            cell.contentView.addSubview(row)
            // A paragraph drawn while its row was home, under the hidden
            // scroll, painted nothing and may have found no room for a
            // worker; it shows now, outside any batch's refresh when a
            // cell comes back, so the pump paints it (`refreshVisibleText`).
            host.presenter.requestTextPublication()
        }
        row.frame = CGRect(origin: CGPoint(x: place.frame.minX, y: 0), size: place.frame.size)
    }

    /// A custom row's cell height for a row box `height` tall: less the
    /// separator the cell draws instead of the row's own border. A card-less
    /// section has no UIKit separator to stand in for it, so the row keeps
    /// its full height (§6.2).
    private func cellHeight(_ id: UInt32, row height: CGFloat) -> CGFloat {
        let separator = card(of: id) ? CGFloat(host.presenter.views[id]?.style["border_width_bottom"]?.number ?? 0) : 0
        return max(0, height - separator)
    }

    /// A row whose cell was sized to nothing and that has a box now (the
    /// sheet over it closed and gave back its geometry) comes into the list
    /// again: UIKit configures and measures its cell anew, which carries the
    /// row in at its height.
    private func remeasure() {
        unsized = unsized.filter { rows[$0]?.custom == true }
        let grown = unsized.filter { id in
            host.presenter.views[id].map { cellHeight(id, row: carried[id]?.frame.height ?? $0.frame.height) > 0 } ?? false
        }
        guard !grown.isEmpty else { return }
        unsized.subtract(grown)
        // Out and in again: a reload keeps the size the layout learned; an
        // inserted item starts from the estimate, which shows its cell.
        let snapshot = source.snapshot()
        var without = snapshot
        without.deleteItems(grown.filter { snapshot.indexOfItem($0) != nil })
        source.apply(without, animatingDifferences: false)
        source.apply(snapshot, animatingDifferences: false)
    }

    /// Every carried row back in its authored place.
    func restore() {
        for id in carriedOrder.reversed() {
            guard let place = carried[id], let row = host.presenter.views[id] else { continue }
            if row.superview !== place.parent { place.parent.insertSubview(row, at: min(place.index, place.parent.subviews.count)) }
            row.frame = place.frame
        }
        carried.removeAll()
        carriedOrder.removeAll()
        for root in hiddenRoots.values { root.node.isHidden = root.hidden }
        hiddenRoots.removeAll(keepingCapacity: true)
    }

    func remove() {
        restore()
        owner.mountNativeScrollView(nil)
        #if os(iOS)
        skirt?.removeFromSuperview()
        #endif
        collection.removeFromSuperview()
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) { owner.scrollViewDidScroll(scrollView) }
    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) { owner.scrollViewWillBeginDragging(scrollView) }
    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        owner.scrollViewDidEndDragging(scrollView, willDecelerate: decelerate)
    }
    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) { owner.scrollViewDidEndDecelerating(scrollView) }
    func scrollViewDidEndScrollingAnimation(_ scrollView: UIScrollView) { owner.scrollViewDidEndScrollingAnimation(scrollView) }
    func scrollViewWillEndDragging(_ scrollView: UIScrollView, withVelocity velocity: CGPoint, targetContentOffset: UnsafeMutablePointer<CGPoint>) {
        owner.scrollViewWillEndDragging(scrollView, withVelocity: velocity, targetContentOffset: targetContentOffset)
    }

    /// A tap: UIKit's highlight, then the row's press.
    func select(_ id: UInt32) -> Bool {
        guard pressable(id), let path = source.indexPath(for: id) else { return false }
        collection.selectItem(at: path, animated: false, scrollPosition: [])
        collectionView(collection, didSelectItemAt: path)
        return true
    }

    func collectionView(_ view: UICollectionView, willDisplay cell: UICollectionViewCell, forItemAt path: IndexPath) {
        guard let cell = cell as? GroupedCell, let id = source.itemIdentifier(for: path) else { return }
        // UIKit may prepare a cell before a batch gives its carried row back.
        // Displaying that prepared cell does not configure it again.
        let height = cell.height
        carry(id, into: cell)
        interact(cell, id)
        if cell.height != height { host.presenter.requestProjectionSync() }
    }

    func collectionView(_ view: UICollectionView, shouldHighlightItemAt path: IndexPath) -> Bool {
        source.itemIdentifier(for: path).map(pressable) ?? false
    }

    /// Whether a tap presses the row: a button, or a radio's row, not
    /// disabled, not inert (its node or an ancestor, the section included).
    private func pressable(_ id: UInt32) -> Bool {
        guard let row = rows[id], row.pressable || radio(row) != nil, !row.disabled, let node = host.presenter.views[id] else { return false }
        return !(carried[id]?.inert ?? node.inert)
    }
    /// The radio a row's tap checks, unless it is disabled or inert.
    private func radio(_ row: GroupedListModel.Row) -> UInt32? {
        guard row.accessory == "radio", let target = row.target, let node = host.presenter.views[target], !node.disabled, !node.inert else { return nil }
        return target
    }

    /// An inert row's cell takes no touch and is no element, as its node.
    private func interact(_ cell: UICollectionViewCell, _ id: UInt32) {
        assign(cell, \.accessibilityIdentifier, host.presenter.views[id]?.props["testId"])
        let inert = carried[id]?.inert ?? host.presenter.views[id]?.inert ?? false
        assign(cell, \.isUserInteractionEnabled, !inert)
        assign(cell, \.accessibilityElementsHidden, inert)
        let node = host.presenter.views[id]
        if node?.disabled == true || node?.ariaUnavailable == true { cell.accessibilityTraits.insert(.notEnabled) }
        else { cell.accessibilityTraits.remove(.notEnabled) }
    }
    func collectionView(_ view: UICollectionView, shouldSelectItemAt path: IndexPath) -> Bool {
        collectionView(view, shouldHighlightItemAt: path)
    }
    /// A tapped row stays selected while its press runs and, when the press
    /// pushed a screen over the list's, under that screen: it fades as the
    /// reader comes back (`deselect`), as a table view's row does
    /// (`clearsSelectionOnViewWillAppear`; LLP 1115.000 D11.3). A row that
    /// pushed nothing, or opened a sheet, lets go once its press has run.
    func collectionView(_ view: UICollectionView, didSelectItemAt path: IndexPath) {
        let letGo = { for selected in view.indexPathsForSelectedItems ?? [] { view.deselectItem(at: selected, animated: !ExactEnv.agentFreezes) } }
        guard let id = source.itemIdentifier(for: path), pressable(id) else { return letGo() }
        host.presenter.endEditing()
        // The screen the list is on, read before the press can change it.
        let screen = sequence(first: owner as UIResponder, next: { $0.next }).lazy.compactMap { $0 as? UIViewController }.first
        // A radio's row checks its radio, as a click on the radio does:
        // `input` and `change` carry its value, and the checkmark follows
        // the bound `checked` (LLP 1115.000 D13.2).
        if let row = rows[id], let target = radio(row) { host.presenter.checkRadio(target) }
        guard rows[id]?.pressable == true else { return letGo() }
        host.presenter.press(id)
        if let screen, let stack = screen.navigationController, stack.viewControllers.contains(screen), stack.topViewController !== screen { return }
        letGo()
    }

    /// The list's screen is coming back into view: its selected rows fade
    /// with the transition, so the fade follows a back swipe's finger, and a
    /// swipe let go of early leaves the row selected under the screen that
    /// stayed. With no transition, at once.
    func deselect(alongside coordinator: UIViewControllerTransitionCoordinator?) {
        let paths = collection.indexPathsForSelectedItems ?? []
        guard !paths.isEmpty else { return }
        guard let coordinator, !ExactEnv.agentFreezes else {
            for path in paths { collection.deselectItem(at: path, animated: false) }
            return
        }
        let list = collection
        coordinator.animate(alongsideTransition: { _ in
            for path in paths { list.deselectItem(at: path, animated: true) }
        }, completion: { context in
            guard context.isCancelled else { return }
            for path in paths { list.selectItem(at: path, animated: false, scrollPosition: []) }
        })
    }
}

/// A grouped list's collection view, by type, for the agent's wheel.
final class GroupedCollectionView: UICollectionView, GroupedScroller {}

extension UICollectionViewListCell {
    /// A row's or header's margins are the list layout's, never the safe
    /// area's: a cell under the home indicator (a fling past the end)
    /// otherwise grew by the inset, moved, and shrank a pixel a layout pass
    /// until UIKit's feedback-loop check stopped the app. The layout's
    /// sections already keep clear of the sides' safe area (its
    /// `contentInsetsReference`). Again after each configuration, which may
    /// replace the content view.
    func marginsIgnoreSafeArea() {
        insetsLayoutMarginsFromSafeArea = false
        contentView.insetsLayoutMarginsFromSafeArea = false
    }
}

/// A list cell; a custom row's is as tall as its carried views.
final class GroupedCell: UICollectionViewListCell {
    var row: UInt32?
    var height: CGFloat?
    #if os(iOS)
    /// A touch its row leaves to no control goes to the row's one field: the
    /// cell is the field (LLP 1116.005 D2).
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        NodeView.cellField(in: self, hit: super.hitTest(point, with: event))
    }
    #endif
    override func preferredLayoutAttributesFitting(_ attributes: UICollectionViewLayoutAttributes) -> UICollectionViewLayoutAttributes {
        guard let height else { return super.preferredLayoutAttributesFitting(attributes) }
        let fitted = attributes.copy() as! UICollectionViewLayoutAttributes
        fitted.size.height = height
        return fitted
    }
}
#endif

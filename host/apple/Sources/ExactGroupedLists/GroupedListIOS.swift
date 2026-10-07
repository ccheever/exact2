// @ref LLP 1084 D5 — a grouped list (`list appearance="auto"`) is UIKit's
// own list: a UICollectionView with UICollectionLayoutListConfiguration in
// the list's box, its cells UIListContentConfiguration and cell accessories
// read from the kernel (`exact_grouped_list`), so separators, highlight,
// dynamic type and dark mode are the platform's. The authored scroll stays
// beneath, hidden: the kernel still lays it out, a custom row's views are
// carried into their cell, and every batch sees the authored hierarchy.
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
    struct Row: Equatable {
        var view: UInt32
        var custom = false
        var symbol: String?
        var title: String?
        var secondary: String?
        var subtitle = false
        var accessory = "none"
        var target: UInt32?
        var pressable = false
        var destructive = false
        var disabled = false
    }
    struct Section: Equatable {
        var view: UInt32
        var header: String?
        var footer: String?
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
                           target: id(r["target"]), pressable: r["pressable"] as? Bool ?? false,
                           destructive: r["destructive"] as? Bool ?? false, disabled: r["disabled"] as? Bool ?? false)
            }
            return Section(view: view, header: s["header"] as? String, footer: s["footer"] as? String, rows: rows,
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
        guard row.view != node.id else { return .view(cell, port: list.collection) }
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
    func inspectionOwns(_ view: UIView) -> Bool { lists.values.contains { $0.collection === view } }
    func hides(_ node: NodeView) -> Bool { lists.values.contains { $0.owner.scrollView.map { node.isDescendant(of: $0) } ?? false } }
    func projects(_ view: UIView) -> Bool { lists.values.contains { $0.carried.keys.contains((view as? NodeView)?.id ?? 0) } }
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
    private var scrollWasHidden = false

    init(owner: NodeView, host: GroupedListHost) {
        self.owner = owner; self.host = host
        collection = GroupedCollectionView(frame: owner.bounds, collectionViewLayout: UICollectionViewFlowLayout())
        super.init()
        collection.setCollectionViewLayout(layout(), animated: false)
        collection.delegate = self
        collection.accessibilityIdentifier = owner.props["testId"]
        let cell = UICollectionView.CellRegistration<GroupedCell, UInt32> { [unowned self] cell, _, id in configure(cell, id) }
        let header = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(elementKind: UICollectionView.elementKindSectionHeader) { [unowned self] view, _, path in
            var c = view.defaultContentConfiguration()
            c.text = section(at: path.section)?.header
            view.contentConfiguration = c
        }
        let footer = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(elementKind: UICollectionView.elementKindSectionFooter) { [unowned self] view, _, path in
            var c = view.defaultContentConfiguration()
            c.text = section(at: path.section)?.footer
            view.contentConfiguration = c
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

    /// Each section's header and footer exist only where it has the text.
    private func layout() -> UICollectionViewLayout {
        UICollectionViewCompositionalLayout { [weak self] index, environment in
            guard let self else { return nil }
            var c = UICollectionLayoutListConfiguration(appearance: model.appearance)
            let s = section(at: index)
            c.headerMode = s?.header == nil ? .none : .supplementary
            c.footerMode = s?.footer == nil ? .none : .supplementary
            #if !os(tvOS)
            c.showsSeparators = s?.card ?? true
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

    func update(_ next: GroupedListModel) {
        let previous = model, old = rows
        let restyled = next.style != previous.style
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
        // A row whose parts changed is configured again; a custom row always
        // is, as its views may have changed size.
        // A standard row whose symbol's authored tint changed is too (D7).
        let tints: [UInt32: BatchValue?] = Dictionary(uniqueKeysWithValues: snapshot.itemIdentifiers.map { ($0, tint(of: $0)) })
        // A row whose card changed (its section gained or lost one, or the
        // row moved between sections) is configured again.
        let wasCarded = Dictionary(previous.sections.flatMap { s in s.rows.map { ($0.view, s.card) } }, uniquingKeysWith: { a, _ in a })
        let recarded = Set(next.sections.flatMap { s in s.rows.compactMap { r in wasCarded[r.view].flatMap { $0 != s.card ? r.view : nil } } })
        let changed = snapshot.itemIdentifiers.filter { id in old[id].map { $0 != rows[id] || $0.custom || tints[id] != looks[id] || recarded.contains(id) } ?? false }
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
            || !recarded.isEmpty // separators are the section layout's
        let regapped = previous.sections.map(\.spaceAbove) != next.sections.map(\.spaceAbove) || previous.spaceBelow != next.spaceBelow
        if restyled { collection.setCollectionViewLayout(layout(), animated: false) }
        source.apply(snapshot, animatingDifferences: false)
        if texts {
            for kind in [UICollectionView.elementKindSectionHeader, UICollectionView.elementKindSectionFooter] {
                for path in collection.indexPathsForVisibleSupplementaryElements(ofKind: kind) {
                    guard let view = collection.supplementaryView(forElementKind: kind, at: path) as? UICollectionViewListCell else { continue }
                    var c = view.defaultContentConfiguration()
                    c.text = kind == UICollectionView.elementKindSectionHeader ? section(at: path.section)?.header : section(at: path.section)?.footer
                    view.contentConfiguration = c
                }
            }
            collection.collectionViewLayout.invalidateLayout()
        } else if regapped { collection.collectionViewLayout.invalidateLayout() }
        mount()
    }

    /// Shown in the list's box over the hidden authored scroll, insets as
    /// that scroll's; custom rows carried into their cells.
    func mount() {
        if collection.superview !== owner { owner.addSubview(collection) }
        if let scroll = owner.scrollView {
            if !scroll.isHidden { scrollWasHidden = false; scroll.isHidden = true }
            assign(collection, \.contentInsetAdjustmentBehavior, scroll.contentInsetAdjustmentBehavior)
            // An authored space under a last section with a footer is under
            // the footer: its section's bottom inset is the rows-to-footer gap.
            var inset = scroll.contentInset
            if model.sections.last?.footer != nil, let below = model.spaceBelow { inset.bottom += below }
            assign(collection, \.contentInset, inset)
            assign(collection, \.verticalScrollIndicatorInsets, scroll.verticalScrollIndicatorInsets)
            // A short list bounces, as Settings does; UICollectionView's own
            // default would not.
            assign(collection, \.alwaysBounceVertical, owner.scrollsVertically)
            assign(collection, \.bounces, owner.style["overscroll_behavior_y"]?.string != "none")
        }
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
        if resized { resized = false; collection.collectionViewLayout.invalidateLayout() }
    }

    func cell(_ id: UInt32) -> UICollectionViewCell? {
        source.indexPath(for: id).flatMap { collection.cellForItem(at: $0) }
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
        cell.accessibilityIdentifier = host.presenter.views[id]?.props["testId"]
        // A card-less section's rows sit on the list's background; a
        // pressable standard row still shows UIKit's highlight while pressed.
        if card(of: id) {
            cell.configurationUpdateHandler = nil
            cell.backgroundConfiguration = cell.defaultBackgroundConfiguration()
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
        if let value = symbolView(of: id)?.style["tint_color"] {
            c.imageProperties.tintColor = UIColor { traits in
                value.channels(dark: traits.userInterfaceStyle == .dark).map(TextEngine.color) ?? .tintColor
            }
        }
        c.text = row.title
        c.secondaryText = row.secondary
        if row.destructive {
            c.textProperties.color = .systemRed
            c.imageProperties.tintColor = .systemRed
        }
        if row.disabled {
            c.textProperties.color = .tertiaryLabel
            c.secondaryTextProperties.color = .tertiaryLabel
            c.imageProperties.tintColor = .tertiaryLabel
        }
        cell.contentConfiguration = c
        cell.accessories = accessories(row)
    }

    private func accessories(_ row: GroupedListModel.Row) -> [UICellAccessory] {
        switch row.accessory {
        case "disclosure": return [.disclosureIndicator()]
        case "checkmark": return [.checkmark()]
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
    /// (a `when` may have replaced it), its committed `checked`, whether it
    /// or the row is disabled, its accent and name.
    private func refresh(_ id: UInt32, _ toggle: UISwitch) {
        let node = rows[id]?.target.flatMap { host.presenter.views[$0] }
        let on = node?.props["checked"] == "true"
        if toggle.isOn != on { toggle.setOn(on, animated: toggle.window != nil && !ExactEnv.agentFreezes) }
        assign(toggle, \.isEnabled, node != nil && rows[id]?.disabled == false && node?.disabled == false && node?.inert == false)
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
    /// group. UIKit draws the separator inside the cell, independently of any
    /// CSS border the author gave the row.
    private func carry(_ id: UInt32, into cell: GroupedCell) {
        guard rows[id]?.custom == true, let row = host.presenter.views[id] else { return }
        for case let other as NodeView in cell.contentView.subviews where other !== row { other.removeFromSuperview() }
        if carried[id] == nil, let parent = row.superview {
            // Its inertness is its authored ancestors', which a cell is not.
            carried[id] = (parent, parent.subviews.firstIndex(of: row) ?? 0, row.frame, row.inert)
            carriedOrder.append(id)
        }
        guard let place = carried[id] else { return }
        let height = max(0, place.frame.height)
        // Never invalidated here: a cell is configured inside the data
        // source's update; `mount` lays the list out after it.
        if cell.height != height { cell.height = height; resized = true }
        cell.contentView.clipsToBounds = true
        if row.superview !== cell.contentView { cell.contentView.addSubview(row) }
        row.frame = CGRect(origin: CGPoint(x: place.frame.minX, y: 0), size: place.frame.size)
        row.setGroupedNativeButtonContent(cell.contentView)
        row.setNeedsDisplay()
    }

    /// Every carried row back in its authored place.
    func restore() {
        for id in carriedOrder.reversed() {
            guard let place = carried[id], let row = host.presenter.views[id] else { continue }
            if row.superview !== place.parent { place.parent.insertSubview(row, at: min(place.index, place.parent.subviews.count)) }
            row.frame = place.frame
            row.setGroupedNativeButtonContent(nil)
        }
        carried.removeAll()
        carriedOrder.removeAll()
    }

    func remove() {
        restore()
        collection.removeFromSuperview()
        owner.scrollView?.isHidden = scrollWasHidden
    }

    /// A tap: UIKit's highlight, then the row's press.
    func select(_ id: UInt32) -> Bool {
        guard pressable(id), let path = source.indexPath(for: id) else { return false }
        collection.selectItem(at: path, animated: false, scrollPosition: [])
        collectionView(collection, didSelectItemAt: path)
        return true
    }

    func collectionView(_ view: UICollectionView, shouldHighlightItemAt path: IndexPath) -> Bool {
        source.itemIdentifier(for: path).map(pressable) ?? false
    }

    /// Whether a tap presses the row: a button, not disabled, not inert
    /// (its node or an ancestor, the section included).
    private func pressable(_ id: UInt32) -> Bool {
        guard let row = rows[id], row.pressable, !row.disabled, let node = host.presenter.views[id] else { return false }
        return !(carried[id]?.inert ?? node.inert)
    }

    /// An inert row's cell takes no touch and is no element, as its node.
    private func interact(_ cell: UICollectionViewCell, _ id: UInt32) {
        let inert = carried[id]?.inert ?? host.presenter.views[id]?.inert ?? false
        assign(cell, \.isUserInteractionEnabled, !inert)
        assign(cell, \.accessibilityElementsHidden, inert)
    }
    func collectionView(_ view: UICollectionView, shouldSelectItemAt path: IndexPath) -> Bool {
        collectionView(view, shouldHighlightItemAt: path)
    }
    func collectionView(_ view: UICollectionView, didSelectItemAt path: IndexPath) {
        view.deselectItem(at: path, animated: !ExactEnv.agentFreezes)
        guard let id = source.itemIdentifier(for: path), pressable(id) else { return }
        host.presenter.viewportScroll.endEditing(true)
        host.presenter.press(id)
    }
}

/// A grouped list's collection view, by type, for the agent's wheel.
final class GroupedCollectionView: UICollectionView, GroupedScroller {}

/// A list cell; a custom row's is as tall as its carried views.
final class GroupedCell: UICollectionViewListCell {
    var row: UInt32?
    var height: CGFloat?
    override func layoutSubviews() {
        super.layoutSubviews()
        // Standard cells have no carried row or native button to lay out.
        guard height != nil else { return }
        // UIKit can update content margins after carry, during cell layout.
        for case let row as NodeView in contentView.subviews {
            row.setGroupedNativeButtonContent(contentView)
        }
    }
    override func preferredLayoutAttributesFitting(_ attributes: UICollectionViewLayoutAttributes) -> UICollectionViewLayoutAttributes {
        guard let height else { return super.preferredLayoutAttributesFitting(attributes) }
        let fitted = attributes.copy() as! UICollectionViewLayoutAttributes
        fitted.size.height = height
        return fitted
    }
}
#endif

// @ref LLP 1082 D5 — a grouped list (`list appearance="auto"`) is UIKit's
// own list: a UICollectionView with UICollectionLayoutListConfiguration in
// the list's box, its cells UIListContentConfiguration and cell accessories
// read from the kernel (`exact_grouped_list`), so separators, highlight,
// dynamic type and dark mode are the platform's. The authored scroll stays
// beneath, hidden: the kernel still lays it out, a custom row's views are
// carried into their cell, and every batch sees the authored hierarchy.
#if os(iOS)
import UIKit

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
    }
    var style = "inset-grouped"
    var sections: [Section] = []

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
            return Section(view: view, header: s["header"] as? String, footer: s["footer"] as? String, rows: rows)
        }
    }
    var appearance: UICollectionLayoutListConfiguration.Appearance {
        switch style { case "plain": .plain; case "grouped": .grouped; default: .insetGrouped }
    }
}

final class GroupedListHost {
    unowned let presenter: Presenter
    private(set) var lists: [UInt32: GroupedListView] = [:]
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
            guard let model = presenter.groupedList?(owner.id) else { continue }
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
                if let row = section.rows.first(where: { $0.view == id || (!$0.custom && $0.target == id) }) { return (list, row) }
            }
        }
        return nil
    }

    /// The agent's `tap` on a row UIKit draws: the cell's own selection, as a
    /// finger's; on a detail button, its accessory's action. Nil for any node
    /// this host does not draw (a toggle's control is the control host's).
    func activate(_ node: NodeView) -> Bool? {
        guard let (list, row) = list(drawing: node.id), list.collection.window != nil else { return nil }
        if row.view != node.id {
            guard row.accessory == "detail", !row.disabled else { return nil }
            presenter.press(node.id)
            return true
        }
        return list.select(row.view)
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
            let r = cell.convert(cell.bounds, to: presenter.viewport)
            seen["frame"] = [Agent.r2(r.minX), Agent.r2(r.minY - presenter.viewport.contentOffset.y), Agent.r2(r.width), Agent.r2(r.height)]
        }
        return seen
    }

    // LLP 1080.001 D3: what the inspection walk accounts for.
    func inspectionOwns(_ view: UIView) -> Bool { lists.values.contains { $0.collection === view } }
    func hides(_ node: NodeView) -> Bool { lists.values.contains { $0.owner.scroll.map { node.isDescendant(of: $0) } ?? false } }
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
    private var switches: [UInt32: UISwitch] = [:]
    /// Custom rows: where the presenter put each, to give it back.
    private(set) var carried: [UInt32: (parent: UIView, index: Int, frame: CGRect)] = [:]
    private var scrollWasHidden = false

    init(owner: NodeView, host: GroupedListHost) {
        self.owner = owner; self.host = host
        collection = UICollectionView(frame: owner.bounds, collectionViewLayout: UICollectionViewFlowLayout())
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
            let section = NSCollectionLayoutSection.list(using: c, layoutEnvironment: environment)
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
        switches = switches.filter { rows[$0.key]?.accessory == "toggle" }
        var snapshot = NSDiffableDataSourceSnapshot<UInt32, UInt32>()
        var seen = Set<UInt32>()
        for s in next.sections where seen.insert(s.view).inserted {
            snapshot.appendSections([s.view])
            snapshot.appendItems(s.rows.map(\.view).filter { seen.insert($0).inserted }, toSection: s.view)
        }
        // A row whose parts changed is configured again; a custom row always
        // is, as its views may have changed size.
        let changed = snapshot.itemIdentifiers.filter { id in old[id].map { $0 != rows[id] || $0.custom } ?? false }
        snapshot.reconfigureItems(changed)
        // Headers and footers live in the sections' layout: one that came,
        // went or changed lays the list out again.
        let texts = previous.sections.map { [$0.header, $0.footer] } != next.sections.map { [$0.header, $0.footer] }
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
        }
        mount()
    }

    /// Shown in the list's box over the hidden authored scroll, insets as
    /// that scroll's; custom rows carried into their cells.
    func mount() {
        if collection.superview !== owner { owner.addSubview(collection) }
        if let scroll = owner.scroll {
            if !scroll.isHidden { scrollWasHidden = false; scroll.isHidden = true }
            assign(collection, \.contentInsetAdjustmentBehavior, scroll.contentInsetAdjustmentBehavior)
            assign(collection, \.contentInset, scroll.contentInset)
            assign(collection, \.verticalScrollIndicatorInsets, scroll.verticalScrollIndicatorInsets)
        }
        assign(collection, \.frame, owner.bounds)
        for cell in collection.visibleCells { if let cell = cell as? GroupedCell, let id = cell.row { carry(id, into: cell) } }
        // A switch shows its control's committed `checked`, which a batch
        // may change without changing the row.
        for (id, toggle) in switches {
            guard let target = rows[id]?.target, let node = host.presenter.views[target] else { continue }
            let on = node.props["checked"] == "true"
            if toggle.isOn != on { toggle.setOn(on, animated: toggle.window != nil && !ExactEnv.agentFreezes) }
        }
    }

    func cell(_ id: UInt32) -> UICollectionViewCell? {
        source.indexPath(for: id).flatMap { collection.cellForItem(at: $0) }
    }

    private func configure(_ cell: GroupedCell, _ id: UInt32) {
        cell.row = id
        guard let row = rows[id] else { return }
        cell.accessibilityIdentifier = host.presenter.views[id]?.props["testId"]
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
            guard let target = row.target else { return [] }
            return [.detail(displayed: .always) { [weak self] in
                guard let self, !row.disabled, host.presenter.views[target] != nil else { return }
                host.presenter.press(target)
            }]
        case "toggle":
            guard let target = row.target else { return [] }
            let toggle = switches[row.view] ?? {
                let s = UISwitch()
                s.addAction(UIAction { [weak self, weak s] _ in
                    guard let self, let s else { return }
                    flip(target, s)
                }, for: .valueChanged)
                switches[row.view] = s
                return s
            }()
            let node = host.presenter.views[target]
            let on = node?.props["checked"] == "true"
            if toggle.isOn != on { toggle.setOn(on, animated: toggle.window != nil) }
            assign(toggle, \.isEnabled, !(row.disabled || node?.disabled == true))
            assign(toggle, \.onTintColor, node?.channels("accent_color").map { TextEngine.color($0) })
            toggle.accessibilityIdentifier = node?.props["testId"]
            return [.customView(configuration: .init(customView: toggle, placement: .trailing()))]
        default: return []
        }
    }

    /// The switch flips the authored control's `checked`, as that control
    /// does (LLP 1069.001 D4): the committed state is authoritative.
    private func flip(_ target: UInt32, _ toggle: UISwitch) {
        guard host.presenter.views[target] != nil else { return }
        host.presenter.checked(target, toggle.isOn)
        if let committed = host.presenter.views[target]?.props["checked"].map({ $0 == "true" }), toggle.isOn != committed {
            toggle.setOn(committed, animated: true)
        }
    }

    /// A custom row's own views in its cell, at the row's place in its
    /// group, as tall as the row less the separator the cell draws instead.
    private func carry(_ id: UInt32, into cell: GroupedCell) {
        guard rows[id]?.custom == true, let row = host.presenter.views[id] else { return }
        for case let other as NodeView in cell.contentView.subviews where other !== row { other.removeFromSuperview() }
        if carried[id] == nil, let parent = row.superview {
            carried[id] = (parent, parent.subviews.firstIndex(of: row) ?? 0, row.frame)
        }
        guard let place = carried[id] else { return }
        let separator = CGFloat(row.style["border_width_bottom"]?.number ?? 0)
        let height = max(0, place.frame.height - separator)
        if cell.height != height { cell.height = height; collection.collectionViewLayout.invalidateLayout() }
        cell.contentView.clipsToBounds = true
        if row.superview !== cell.contentView { cell.contentView.addSubview(row) }
        row.frame = CGRect(origin: CGPoint(x: place.frame.minX, y: 0), size: place.frame.size)
    }

    /// Every carried row back in its authored place.
    func restore() {
        for (id, place) in carried {
            guard let row = host.presenter.views[id] else { continue }
            if row.superview !== place.parent { place.parent.insertSubview(row, at: min(place.index, place.parent.subviews.count)) }
            row.frame = place.frame
        }
        carried.removeAll()
    }

    func remove() {
        restore()
        collection.removeFromSuperview()
        owner.scroll?.isHidden = scrollWasHidden
    }

    /// A tap: UIKit's highlight, then the row's press.
    func select(_ id: UInt32) -> Bool {
        guard let row = rows[id], row.pressable, !row.disabled, let path = source.indexPath(for: id) else { return false }
        collection.selectItem(at: path, animated: false, scrollPosition: [])
        collectionView(collection, didSelectItemAt: path)
        return true
    }

    func collectionView(_ view: UICollectionView, shouldHighlightItemAt path: IndexPath) -> Bool {
        source.itemIdentifier(for: path).flatMap { rows[$0] }.map { $0.pressable && !$0.disabled } ?? false
    }
    func collectionView(_ view: UICollectionView, shouldSelectItemAt path: IndexPath) -> Bool {
        collectionView(view, shouldHighlightItemAt: path)
    }
    func collectionView(_ view: UICollectionView, didSelectItemAt path: IndexPath) {
        view.deselectItem(at: path, animated: !ExactEnv.agentFreezes)
        guard let id = source.itemIdentifier(for: path), host.presenter.views[id] != nil else { return }
        host.presenter.viewport.endEditing(true)
        host.presenter.press(id)
    }
}

/// A list cell; a custom row's is as tall as its carried views.
final class GroupedCell: UICollectionViewListCell {
    var row: UInt32?
    var height: CGFloat?
    override func preferredLayoutAttributesFitting(_ attributes: UICollectionViewLayoutAttributes) -> UICollectionViewLayoutAttributes {
        guard let height else { return super.preferredLayoutAttributesFitting(attributes) }
        let fitted = attributes.copy() as! UICollectionViewLayoutAttributes
        fitted.size.height = height
        return fitted
    }
}
#endif

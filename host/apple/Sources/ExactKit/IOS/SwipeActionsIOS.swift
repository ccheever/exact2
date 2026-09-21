// @ref LLP 1008 §9 — authored row content and action controls projected into
// UIKit swipe cells. The kernel owns dimensions; UIKit owns the gesture.
#if os(iOS)
import UIKit

final class SwipeActionsHost {
    unowned let presenter: Presenter
    private var rows: [UInt32: Row] = [:]
    private var refusals: [UInt32: String] = [:]
    init(_ presenter: Presenter) { self.presenter = presenter }

    // Ordinary batches always see the original hierarchy and local frames.
    func prepare() { for row in rows.values { row.restore() } }
    func reset() {
        for row in rows.values { row.remove() }
        rows.removeAll(); refusals.removeAll()
    }

    func sync() {
        let named = Dictionary(grouping: presenter.carrying("id").compactMap { node in
            node.props["id"].map { ($0, node) }
        }, by: { $0.0 })
        var wanted = Set<UInt32>()
        var claimed = Set<UInt32>()
        for owner in presenter.carrying("swipeContent") {
            guard let content = owner.props["swipeContent"] else { continue }
            func resolve(_ name: String) -> NodeView? {
                guard let matches = named[name], matches.count == 1 else { return nil }
                let node = matches[0].1
                return node !== owner && node.isDescendant(of: owner) ? node : nil
            }
            let leadingNames = (owner.props["swipeLeading"] ?? "").split(whereSeparator: \.isWhitespace).map(String.init)
            let trailingNames = (owner.props["swipeTrailing"] ?? "").split(whereSeparator: \.isWhitespace).map(String.init)
            let names = leadingNames + trailingNames
            let controls = names.compactMap(resolve)
            guard owner.scroll != nil, let body = resolve(content),
                  !names.isEmpty, Set(names).count == names.count, controls.count == names.count,
                  controls.allSatisfy({ $0.handlers.contains("press") && !$0.isDescendant(of: body) && $0 !== body && !label($0).isEmpty }),
                  abs(body.bounds.width - owner.bounds.width) < 0.5,
                  abs(body.bounds.height - owner.bounds.height) < 0.5,
                  claimed.insert(body.id).inserted else {
                let message = "swipeContent on #\(owner.id) requires one full-size descendant and uniquely named descendant press controls with accessible names"
                if refusals[owner.id] != message { fputs("exact: \(message)\n", stderr); refusals[owner.id] = message }
                continue
            }
            refusals.removeValue(forKey: owner.id)
            if let old = rows[owner.id], old.body !== body { old.remove(); rows.removeValue(forKey: owner.id) }
            let row = rows[owner.id] ?? Row(owner: owner, body: body, host: self)
            rows[owner.id] = row
            row.leading = Array(controls.prefix(leadingNames.count))
            row.trailing = Array(controls.dropFirst(leadingNames.count))
            row.mount()
            wanted.insert(owner.id)
        }
        for id in Array(rows.keys) where !wanted.contains(id) { rows.removeValue(forKey: id)?.remove() }
        refusals = refusals.filter { presenter.views[$0.key] != nil }
    }

    private func label(_ node: NodeView) -> String { node.accessibilityLabel ?? node.props["accessibilityLabel"] ?? "" }
    func ownsAction(_ id: UInt32) -> Bool { rows.values.contains { ($0.leading + $0.trailing).contains { $0.id == id } } }
    func actionView(_ id: UInt32) -> UIButton? {
        for row in rows.values {
            if let button = row.actionView(id) { return button }
        }
        return nil
    }

    private final class Ancestor {
        weak var view: UIView?
        init(_ view: UIView) { self.view = view }
    }

    private final class Cell: UITableViewCell {
        weak var control: NodeView?
        override func accessibilityActivate() -> Bool {
            control?.accessibilityActivate() ?? super.accessibilityActivate()
        }
    }

    private final class Row: NSObject, UITableViewDataSource, UITableViewDelegate {
        unowned let host: SwipeActionsHost
        let owner: NodeView
        let body: NodeView
        var leading: [NodeView] = []
        var trailing: [NodeView] = []
        private weak var logicalParent: UIView?
        private var carrier: UIView?
        private var hiddenControls: [(NodeView, Bool)] = []
        // Captured before projection: UIKit disables its cell while an action
        // completes. That temporary state is not an authored input restriction.
        private var actionAncestors: [UInt32: [Ancestor]] = [:]
        private var scrollWasHidden = false
        private var logicalFrame = CGRect.zero
        private var priorSize = CGSize.zero
        private let table = UITableView(frame: .zero, style: .plain)
        private let cell = Cell(style: .default, reuseIdentifier: nil)
        private var images: [UInt32: UIImage] = [:]

        init(owner: NodeView, body: NodeView, host: SwipeActionsHost) {
            self.owner = owner; self.body = body; self.host = host
            super.init()
            table.dataSource = self; table.delegate = self
            // Only the outer authored scroll container scrolls vertically.
            table.isScrollEnabled = false
            table.contentInsetAdjustmentBehavior = .never
            table.separatorStyle = .none; table.backgroundColor = .clear
            table.estimatedRowHeight = 0; table.sectionHeaderTopPadding = 0
            table.allowsSelection = false
            cell.backgroundConfiguration = .listPlainCell()
        }
        func restore() {
            if let parent = logicalParent, let carrier { parent.addSubview(carrier); carrier.frame = logicalFrame }
            for (control, hidden) in hiddenControls { control.isHidden = hidden }
            hiddenControls.removeAll()
            owner.scroll?.isHidden = scrollWasHidden
            body.nativeSwipeBody = false
        }
        func remove() {
            restore(); table.setEditing(false, animated: false); table.removeFromSuperview()
        }
        func mount() {
            guard let scroll = owner.scroll else { return }
            var content: UIView = body
            while let parent = content.superview, parent !== scroll { content = parent }
            guard content.superview === scroll else { return }
            actionAncestors.removeAll()
            for control in leading + trailing {
                var ancestors: [Ancestor] = [], current: UIView? = control
                while let view = current { ancestors.append(Ancestor(view)); current = view.superview }
                actionAncestors[control.id] = ancestors
            }
            scrollWasHidden = scroll.isHidden
            carrier = content; logicalParent = scroll; logicalFrame = content.frame
            let origin = body.convert(CGPoint.zero, to: content)
            if table.superview !== owner { owner.addSubview(table) }
            owner.scroll?.isHidden = true
            if priorSize != owner.bounds.size || host.presenter.navigation.isInactiveRoute(containing: owner) {
                table.setEditing(false, animated: false)
            }
            priorSize = owner.bounds.size
            table.frame = owner.bounds; table.rowHeight = body.bounds.height
            if content.superview !== cell.contentView { cell.contentView.addSubview(content) }
            // Keep the original ancestors between content and the row. Their
            // opacity, clips, inherited semantics and input restrictions apply.
            content.frame = CGRect(origin: CGPoint(x: -origin.x, y: -origin.y), size: logicalFrame.size)
            for control in leading + trailing { hiddenControls.append((control, control.isHidden)); control.isHidden = true }
            body.nativeSwipeBody = true; body.setNeedsDisplay()
            // UIKit derives its cell label from native text controls; an
            // authored button paints its own text. Preserve that button's
            // explicit name and activation at the native presentation boundary.
            if body.kind == "button", let label = body.accessibilityLabel, !label.isEmpty {
                cell.control = body
                cell.isAccessibilityElement = true
                cell.accessibilityLabel = label
                cell.accessibilityIdentifier = body.accessibilityIdentifier
                cell.accessibilityTraits = body.accessibilityTraits.union(.button)
            } else {
                cell.control = nil
                cell.isAccessibilityElement = false
                cell.accessibilityLabel = nil
                cell.accessibilityIdentifier = nil
                cell.accessibilityTraits = []
            }
            table.layoutIfNeeded()
        }
        func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { 1 }
        func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell { cell }
        func tableView(_ tableView: UITableView, heightForRowAt indexPath: IndexPath) -> CGFloat { body.bounds.height }
        func tableView(_ tableView: UITableView, willBeginEditingRowAt indexPath: IndexPath) {
            for other in host.rows.values where other !== self { other.table.setEditing(false, animated: true) }
            DispatchQueue.main.async { [weak self] in self?.nameActions() }
        }
        func tableView(_ tableView: UITableView, leadingSwipeActionsConfigurationForRowAt indexPath: IndexPath) -> UISwipeActionsConfiguration? { configuration(leading) }
        func tableView(_ tableView: UITableView, trailingSwipeActionsConfigurationForRowAt indexPath: IndexPath) -> UISwipeActionsConfiguration? { configuration(trailing) }

        private func enabled(_ target: NodeView) -> Bool {
            guard let ancestors = actionAncestors[target.id], !ancestors.isEmpty else { return false }
            for ancestor in ancestors {
                guard let view = ancestor.view else { return false }
                let hidden = view === owner.scroll ? scrollWasHidden :
                    hiddenControls.first(where: { $0.0 === view })?.1 ?? view.isHidden
                if hidden || !view.isUserInteractionEnabled || (view as? NodeView)?.disabled == true ||
                    (view as? NodeView)?.props["inert"] == "true" { return false }
            }
            return true
        }
        private func configuration(_ controls: [NodeView]) -> UISwipeActionsConfiguration? {
            let actions = controls.filter(enabled).map { target in
                let destructive = target.props["destructive"] == "true"
                let action = UIContextualAction(style: destructive ? .destructive : .normal, title: nil) { [weak self, weak target] _, _, complete in
                    guard let self, let target, self.host.presenter.views[target.id] === target, self.enabled(target) else { complete(false); return }
                    complete(true)
                    self.host.presenter.press(target.id)
                }
                action.backgroundColor = target.color("background_color", .systemBlue)
                action.accessibilityLabel = host.label(target)
                if let glyph = target.container.subviews.first as? NodeView, !glyph.bounds.isEmpty {
                    func display(_ view: UIView) { view.layer.displayIfNeeded(); for child in view.subviews { display(child) } }
                    display(glyph)
                    // Layer rendering omits the root view's transform. Capture
                    // its transformed box, so an authored icon scale/rotation
                    // survives projection into UIKit's centered image slot.
                    let bounds = glyph.bounds.applying(glyph.transform)
                    if !bounds.isEmpty {
                        let image = UIGraphicsImageRenderer(size: bounds.size).image { context in
                            context.cgContext.translateBy(x: -bounds.minX, y: -bounds.minY)
                            context.cgContext.concatenate(glyph.transform)
                            glyph.layer.render(in: context.cgContext)
                        }.withRenderingMode(.alwaysOriginal)
                        image.accessibilityLabel = host.label(target)
                        images[target.id] = image; action.image = image
                    }
                } else { action.title = host.label(target) }
                return action
            }
            guard !actions.isEmpty else { return nil }
            DispatchQueue.main.async { [weak self] in self?.nameActions() }
            let configuration = UISwipeActionsConfiguration(actions: actions)
            configuration.performsFirstActionWithFullSwipe = controls.first.map(enabled) ?? false
            return configuration
        }

        // Observe public UIKit controls by their label/image. Never infer a
        // target from a private class name, an action's position, or testId.
        private func nameActions() {
            for target in leading + trailing {
                if let button = actionView(target.id, visibleOnly: false) {
                    button.accessibilityLabel = host.label(target)
                    button.accessibilityIdentifier = target.props["id"]
                }
            }
        }
        func actionView(_ id: UInt32, visibleOnly: Bool = true) -> UIButton? {
            guard let target = (leading + trailing).first(where: { $0.id == id }) else { return nil }
            var matches: [UIButton] = []
            func hasImage(_ view: UIView) -> Bool {
                if let actual = (view as? UIImageView)?.image, let expected = images[id], actual === expected || actual.isEqual(expected) { return true }
                return view.subviews.contains(where: hasImage)
            }
            func visit(_ view: UIView) {
                if visibleOnly && (view.isHidden || view.alpha <= 0.01) { return }
                if let button = view as? UIButton,
                   (!visibleOnly || table.bounds.intersects(button.convert(button.bounds, to: table))),
                   button.accessibilityLabel == host.label(target) || hasImage(button) { matches.append(button) }
                for child in view.subviews { visit(child) }
            }
            visit(table)
            guard matches.count == 1 else { return nil }
            return matches[0]
        }
    }
}
#endif

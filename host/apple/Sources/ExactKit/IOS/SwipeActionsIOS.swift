// @ref LLP 1008 §9 — authored row content and action controls projected into
// UIKit swipe cells. The kernel owns dimensions; UIKit owns the gesture.
#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass

/// A row pays for its UIKit cell only while a swipe can start. At rest the
/// authored scroll holds the content, as on the web, and a batch walks no
/// hierarchy. A touch landing on the row projects it into a one-row table
/// (`touch`, from the row's hit test: before UIKit gathers the touch's
/// recognizers, so the table's own swipe sees the first pan). Once the touch
/// has ended, the row is closed and UIKit's animation has settled, the
/// content goes back and the table goes. VoiceOver and Switch Control read a
/// row's actions from its cell, so while either runs every row stays projected.
final class SwipeActionsHost {
    unowned let presenter: Presenter
    private var rows: [UInt32: Row] = [:]
    private var refusals: [UInt32: String] = [:]
    private var observers: [NSObjectProtocol] = []
    init(_ presenter: Presenter) {
        self.presenter = presenter
        for name in [UIAccessibility.voiceOverStatusDidChangeNotification, UIAccessibility.switchControlStatusDidChangeNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                guard let self else { return }
                for row in rows.values { if assistive { row.mount() } else { row.settle() } }
            })
        }
    }
    deinit { observers.forEach { NotificationCenter.default.removeObserver($0) } }
    var assistive: Bool { UIAccessibility.isVoiceOverRunning || UIAccessibility.isSwitchControlRunning }

    // Ordinary batches always see the original hierarchy and local frames.
    func prepare() { for row in rows.values where row.projected { row.restore() } }
    /// A touch is landing on `owner`: project its row now, and release it
    /// again if the touch never reaches the table.
    func touch(_ owner: NodeView) {
        guard let row = rows[owner.id], row.owner === owner, !row.projected else { return }
        row.mount()
        DispatchQueue.main.async { [weak row] in row?.settle() }
    }
    /// The row projected for `owner`, if any (tests and diagnostics).
    func cell(of owner: NodeView) -> UITableViewCell? { rows[owner.id]?.projection }
    func reset() {
        for row in rows.values { row.remove() }
        rows.removeAll(); refusals.removeAll()
    }

    /// `changed`: the views the batch touched and their ancestors. An owner
    /// with a row, outside it and none of whose names changed carriers,
    /// keeps its row as it was; nil revisits every owner.
    func sync(changed: Set<UInt32>? = nil) {
        let named = presenter.chrome.named
        let renamed = presenter.takeChangedNames()
        var wanted = Set<UInt32>()
        var claimed = Set<UInt32>()
        for owner in presenter.carrying("swipeContent") {
            guard let content = owner.props["swipeContent"] else { continue }
            // A refused owner is retried every batch, as the names its
            // controls read from their subtrees can arrive a batch later.
            // An owner outside `changed` has the props its row was built
            // from, so the row's names are still the owner's.
            if let changed, !changed.contains(owner.id), !renamed.contains(content),
               let row = rows[owner.id], row.owner === owner, !row.names.contains(where: renamed.contains) {
                claimed.insert(row.body.id)
                if row.projected || assistive { row.mount() }
                wanted.insert(owner.id)
                continue
            }
            let leadingNames = (owner.props["swipeLeading"] ?? "").split(whereSeparator: \.isWhitespace).map(String.init)
            let trailingNames = (owner.props["swipeTrailing"] ?? "").split(whereSeparator: \.isWhitespace).map(String.init)
            let names = leadingNames + trailingNames
            func resolve(_ name: String) -> NodeView? {
                guard let matches = named[name], matches.count == 1, let node = presenter.views[matches.first!] else { return nil }
                return node !== owner && node.isDescendant(of: owner) ? node : nil
            }
            let controls = names.compactMap(resolve)
            guard owner.scroll != nil || owner.scrollDormant, let body = resolve(content),
                  !names.isEmpty, Set(names).count == names.count, controls.count == names.count,
                  controls.allSatisfy({ $0.handlers.contains("press") && !$0.isDescendant(of: body) && $0 !== body && !label($0).isEmpty }),
                  abs(body.bounds.width - owner.bounds.width) < 0.5,
                  abs(body.bounds.height - owner.bounds.height) < 0.5,
                  claimed.insert(body.id).inserted else {
                let message = "swipeContent on #\(owner.id) requires one full-size descendant and uniquely named descendant press controls with accessible names"
                if refusals[owner.id] != message { fputs("exact: \(message)\n", stderr); refusals[owner.id] = message }
                // A refused row swipes as the web does: by its scroll.
                owner.needScroll()
                continue
            }
            refusals.removeValue(forKey: owner.id)
            if let old = rows[owner.id], old.body !== body { old.remove(); rows.removeValue(forKey: owner.id) }
            let row = rows[owner.id] ?? Row(owner: owner, body: body, host: self)
            rows[owner.id] = row
            row.names = names
            row.leading = Array(controls.prefix(leadingNames.count))
            row.trailing = Array(controls.dropFirst(leadingNames.count))
            if row.projected || assistive { row.mount() }
            wanted.insert(owner.id)
        }
        for id in Array(rows.keys) where !wanted.contains(id) { rows.removeValue(forKey: id)?.remove() }
        refusals = refusals.filter { presenter.views[$0.key] != nil }
    }

    /// An action's name; a native button's is its label, else its title
    /// (LLP 1069.011.000 D6).
    private func label(_ node: NodeView) -> String {
        node.accessibilityLabel ?? node.props["accessibilityLabel"] ?? (node.isNativeButton ? node.face?.title : nil) ?? ""
    }
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

    /// Recognizes nothing and prevents nothing: sees a projected row's
    /// touches begin, and the sequence end (UIKit resets it then).
    private final class TouchWatch: UIGestureRecognizer, UIGestureRecognizerDelegate {
        var began: () -> Void = {}
        var ended: () -> Void = {}
        init() {
            super.init(target: nil, action: nil)
            cancelsTouchesInView = false; delaysTouchesEnded = false; delegate = self
        }
        override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) { began() }
        override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) { finish(event) }
        override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) { finish(event) }
        private func finish(_ event: UIEvent) {
            if event.allTouches?.allSatisfy({ $0.phase == .ended || $0.phase == .cancelled }) ?? true { state = .failed }
        }
        override func reset() { super.reset(); ended() }
        override func canPrevent(_ other: UIGestureRecognizer) -> Bool { false }
        override func canBePrevented(by other: UIGestureRecognizer) -> Bool { false }
        func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
    }

    private final class Row: NSObject, UITableViewDataSource, UITableViewDelegate {
        unowned let host: SwipeActionsHost
        let owner: NodeView
        let body: NodeView
        var leading: [NodeView] = []
        var trailing: [NodeView] = []
        /// The owner's leading and trailing names, as its props said when
        /// this row was last built.
        var names: [String] = []
        private weak var logicalParent: UIView?
        private var carrier: UIView?
        private var hiddenControls: [(NodeView, Bool)] = []
        // Captured before projection: UIKit disables its cell while an action
        // completes. That temporary state is not an authored input restriction.
        private var actionAncestors: [UInt32: [Ancestor]] = [:]
        private var scrollWasHidden = false
        private var logicalFrame = CGRect.zero
        private var priorSize = CGSize.zero
        /// The projection, only while a swipe can start or is shown.
        private var table: UITableView?
        private var cell: Cell?
        private var touching = false
        /// UIKit is presenting this row's actions (between its begin and end editing).
        private var open = false
        private var images: [UInt32: UIImage] = [:]
        var projected: Bool { table != nil }
        var projection: UITableViewCell? { cell }

        init(owner: NodeView, body: NodeView, host: SwipeActionsHost) {
            self.owner = owner; self.body = body; self.host = host
            super.init()
        }
        private func project() -> (UITableView, Cell) {
            if let table, let cell { return (table, cell) }
            let table = SwipeTable(frame: owner.bounds, style: .plain)
            let cell = Cell(style: .default, reuseIdentifier: nil)
            table.dataSource = self; table.delegate = self
            // Only the outer authored scroll container scrolls vertically.
            table.isScrollEnabled = false
            table.contentInsetAdjustmentBehavior = .never
            table.separatorStyle = .none; table.backgroundColor = .clear
            table.estimatedRowHeight = 0; table.sectionHeaderTopPadding = 0
            table.allowsSelection = false
            // The row paints itself (its own background, its corners); the
            // cell is only where UIKit's swipe happens.
            cell.backgroundConfiguration = .clear()
            let watch = TouchWatch()
            watch.began = { [weak self] in self?.touching = true }
            watch.ended = { [weak self] in
                guard let self, touching else { return }
                touching = false
                DispatchQueue.main.async { [weak self] in self?.settle() }
            }
            table.addGestureRecognizer(watch)
            self.table = table; self.cell = cell
            return (table, cell)
        }
        func restore() {
            if let parent = logicalParent, let carrier { parent.addSubview(carrier); carrier.frame = logicalFrame }
            for (control, hidden) in hiddenControls { control.isHidden = hidden }
            hiddenControls.removeAll()
            owner.scroll?.isHidden = scrollWasHidden
        }
        func remove() {
            guard let table else { return }
            restore(); carrier = nil; logicalParent = nil
            table.setEditing(false, animated: false); table.removeFromSuperview()
            self.table = nil; cell = nil; images.removeAll(); touching = false; open = false
        }
        /// Release the projection once nothing can be swiping: no touch, the
        /// row closed, and UIKit's close animation finished.
        func settle() {
            guard let table, !touching, !open, !host.assistive else { return }
            func moving(_ layer: CALayer) -> Bool {
                // Authored content may animate for ever; only UIKit's own layers count.
                if layer === carrier?.layer { return false }
                return !(layer.animationKeys() ?? []).isEmpty || (layer.sublayers ?? []).contains(where: moving)
            }
            if moving(table.layer) {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in self?.settle() }
                return
            }
            remove()
        }
        func mount() {
            // A waiting scroll's children are the owner's own (`scrollDormant`).
            guard owner.scroll != nil || owner.scrollDormant else { return }
            let scroll: UIView = owner.scroll ?? owner
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
            let (table, cell) = project()
            if table.superview !== owner { owner.addSubview(table) }
            owner.scroll?.isHidden = true
            // What the hidden scroll would hide: the content's siblings.
            if owner.scroll == nil {
                for case let sibling as NodeView in owner.subviews where sibling !== content { hide(sibling) }
            }
            if priorSize != owner.bounds.size || host.presenter.navigation.isInactiveRoute(containing: owner) {
                table.setEditing(false, animated: false)
            }
            priorSize = owner.bounds.size
            table.frame = owner.bounds
            // The cell follows the row: UIKit keeps a row's height until it reloads.
            if table.rowHeight != body.bounds.height { table.rowHeight = body.bounds.height; table.reloadData() }
            if content.superview !== cell.contentView { cell.contentView.addSubview(content) }
            // Keep the original ancestors between content and the row. Their
            // opacity, clips, inherited semantics and input restrictions apply.
            content.frame = CGRect(origin: CGPoint(x: -origin.x, y: -origin.y), size: logicalFrame.size)
            for control in leading + trailing { hide(control) }
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
        /// Hidden while projected; `restore` gives back what it was, once.
        private func hide(_ view: NodeView) {
            if !hiddenControls.contains(where: { $0.0 === view }) { hiddenControls.append((view, view.isHidden)) }
            view.isHidden = true
        }
        func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { 1 }
        func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell { cell ?? UITableViewCell() }
        func tableView(_ tableView: UITableView, heightForRowAt indexPath: IndexPath) -> CGFloat { body.bounds.height }
        func tableView(_ tableView: UITableView, willBeginEditingRowAt indexPath: IndexPath) {
            open = true
            for other in host.rows.values where other !== self { other.table?.setEditing(false, animated: true) }
            DispatchQueue.main.async { [weak self] in self?.nameActions() }
        }
        func tableView(_ tableView: UITableView, didEndEditingRowAt indexPath: IndexPath?) {
            open = false
            DispatchQueue.main.async { [weak self] in self?.settle() }
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
                action.accessibilityLabel = host.label(target)
                if target.isNativeButton {
                    // A native action (LLP 1069.011.000 D6): its accent, or the
                    // platform's colour (UIKit's red for destructive); its
                    // symbol, recorded for discovery as a snapshot is.
                    if let accent = target.channels("accent_color").map({ TextEngine.color($0) }) { action.backgroundColor = accent }
                    else if !destructive { action.backgroundColor = .systemBlue }
                    if let symbol = target.face?.symbol, let image = UIImage(systemName: symbol) {
                        image.accessibilityLabel = host.label(target)
                        images[target.id] = image; action.image = image
                    } else { action.title = host.label(target) }
                    return action
                }
                action.backgroundColor = target.color("background_color", .systemBlue)
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
            guard let table, let target = (leading + trailing).first(where: { $0.id == id }) else { return nil }
            var matches: [UIButton] = []
            func hasImage(_ view: UIView) -> Bool {
                if let actual = (view as? UIImageView)?.image, let expected = images[id], actual === expected || actual.isEqual(expected) { return true }
                return view.subviews.contains(where: hasImage)
            }
            func visit(_ view: UIView) {
                // UIKit's rendered actions only: never the authored row, its
                // body or its hidden actions, so a native body button labelled
                // like an action is not taken for it (LLP 1069.011.000 D6).
                if view is NodeView { return }
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
/// The swipe row's table: a pan cancels a touch in a native button in the
/// row's body, as it does a custom button's (LLP 1069.011.000 D6).
private final class SwipeTable: UITableView {
    override func touchesShouldCancel(in view: UIView) -> Bool {
        view is NativeButtonIOS || super.touchesShouldCancel(in: view)
    }
}
#endif

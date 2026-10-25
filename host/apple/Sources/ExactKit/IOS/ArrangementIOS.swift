#if os(iOS)
import UIKit

/// Authored siblings laid out by UIKit without introducing navigation or another renderer.
@available(iOS 27.1, *)
final class ArrangementHost {
    private unowned let presenter: Presenter
    private var requests: [UInt32: UIArrangementViewController] = [:]
    private var children: [UInt32: [UInt32]] = [:]
    private(set) var entries: [UInt32: Entry] = [:]

    final class Entry {
        let node: NodeView
        let controller: UIArrangementViewController
        let panes: [NodeView]
        let holders: [PaneController]
        init(node: NodeView, controller: UIArrangementViewController, panes: [NodeView], changed: @escaping () -> Void) {
            self.node = node; self.controller = controller; self.panes = panes
            holders = panes.map { PaneController(node: $0, changed: changed) }
            for holder in holders { holder.loadViewIfNeeded() }
            controller.setViewController(holders[0], for: .primary)
            controller.setViewController(holders[1], for: .secondary)
        }
        func retire() {
            for pane in panes { node.container.addSubview(pane) }
            controller.willMove(toParent: nil)
            controller.view.removeFromSuperview()
            controller.removeFromParent()
        }
    }

    final class PaneController: UIViewController {
        let node: NodeView
        let changed: () -> Void
        init(node: NodeView, changed: @escaping () -> Void) {
            self.node = node; self.changed = changed
            super.init(nibName: nil, bundle: nil)
        }
        required init?(coder: NSCoder) { nil }
        override func loadView() {
            view = UIView()
            view.clipsToBounds = true
            view.addSubview(node)
        }
        override func viewDidLayoutSubviews() {
            super.viewDidLayoutSubviews()
            changed()
        }
    }

    init(_ presenter: Presenter) { self.presenter = presenter }

    func set(_ node: UInt32, controller: UIArrangementViewController?) -> Bool {
        guard let root = presenter.views[node] else { return false }
        if controller != nil {
            let ids = children[node] ?? root.container.subviews.compactMap { ($0 as? NodeView)?.id }
            guard ids.count == 2 else { return false }
            for id in ids where presenter.views[id] == nil { presenter.flats.promote(id) }
            guard ids.allSatisfy({ presenter.views[$0] != nil }) else { return false }
        }
        if let controller, controller.parent != nil, entries[node]?.controller !== controller { return false }
        requests[node] = controller
        sync()
        return true
    }

    func record(_ node: NodeView, children ids: [UInt32]) {
        if node.props["hatch"] != nil || requests[node.id] != nil || entries[node.id] != nil { children[node.id] = ids }
    }

    func owns(_ node: NodeView, under parent: NodeView) -> Bool {
        entries[parent.id]?.panes.contains { $0 === node } == true
    }

    func sync() {
        for (id, entry) in entries where requests[id] !== entry.controller || presenter.views[id] !== entry.node || entry.panes.contains(where: { presenter.views[$0.id] !== $0 }) || (children[id] != nil && children[id] != entry.panes.map(\.id)) {
            entry.retire(); entries[id] = nil; children[id] = nil
        }
        for (id, controller) in requests where entries[id] == nil {
            guard let node = presenter.views[id], let owner = ViewPresentation.controllers(for: node).first else { continue }
            let ids = children[id] ?? node.container.subviews.compactMap { ($0 as? NodeView)?.id }
            let panes = ids.compactMap { presenter.views[$0] }
            guard panes.count == 2 else { continue }
            let entry = Entry(node: node, controller: controller, panes: panes) { [weak presenter] in presenter?.owners.publish() }
            entries[id] = entry
            owner.addChild(controller)
            controller.view.frame = node.container.bounds
            controller.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            node.container.addSubview(controller.view)
            controller.didMove(toParent: owner)
        }
        presenter.owners.publish()
    }

    var blocks: [UInt32: (block: HostBlock, placement: BlockPlacement?)] {
        var out: [UInt32: (block: HostBlock, placement: BlockPlacement?)] = [:]
        for entry in entries.values {
            for holder in entry.holders {
                let bounds = holder.view.superview == nil ? CGRect.zero : holder.view.bounds
                let rect = bounds.isEmpty ? CGRect.zero : holder.view.convert(bounds, to: entry.node.container)
                out[holder.node.id] = (.size(bounds.size), BlockPlacement(rect: rect, within: entry.node.id, contentOffset: .zero))
            }
        }
        return out
    }

    func remove(_ id: UInt32) {
        requests[id] = nil
        if let entry = entries.removeValue(forKey: id) { entry.retire() }
        children[id] = nil
    }

    func reset() {
        for entry in entries.values { entry.retire() }
        entries = [:]; requests = [:]; children = [:]
    }
}
#endif

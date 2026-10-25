#if os(iOS)
import UIKit

/// Authored siblings laid out by UIKit without introducing navigation or another renderer.
@available(iOS 27.1, *)
final class ArrangementHost {
    private unowned let presenter: Presenter
    private var requests: [UInt32: UIArrangementViewController] = [:]
    private var insetScrollers: [UInt32: UIScrollView] = [:]
    private var pendingTops: [UInt32: CGFloat] = [:]
    private var syncingInsets = false
    private var layoutPending = false
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
        func isHidden(_ holder: PaneController) -> Bool {
            let placement: UIArrangementViewController.ViewPlacement = holder === holders[0] ? .primary : .secondary
            return controller.state(for: placement)?.isHidden == true || holder.view.superview == nil
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
        }
        override func viewDidLayoutSubviews() {
            super.viewDidLayoutSubviews()
            changed()
        }
        override func viewSafeAreaInsetsDidChange() {
            super.viewSafeAreaInsetsDidChange()
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
        guard !requests.isEmpty || !entries.isEmpty || !insetScrollers.isEmpty else { return }
        for (id, entry) in entries where requests[id] !== entry.controller || presenter.views[id] !== entry.node || entry.panes.contains(where: { presenter.views[$0.id] !== $0 }) || (children[id] != nil && children[id] != entry.panes.map(\.id)) {
            retire(entry); entries[id] = nil
        }
        for (id, controller) in requests where entries[id] == nil {
            guard let node = presenter.views[id], let owner = ViewPresentation.controllers(for: node).first else { continue }
            let ids = children[id] ?? node.container.subviews.compactMap { ($0 as? NodeView)?.id }
            guard ids.count == 2 else { continue }
            for id in ids where presenter.views[id] == nil { presenter.flats.promote(id) }
            let panes = ids.compactMap { presenter.views[$0] }
            guard panes.count == 2 else { continue }
            let entry = Entry(node: node, controller: controller, panes: panes) { [weak self] in self?.layoutChanged() }
            entries[id] = entry
            owner.addChild(controller)
            controller.view.frame = node.container.bounds
            controller.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            node.container.addSubview(controller.view)
            controller.didMove(toParent: owner)
            controller.view.layoutIfNeeded()
            // Keep visible content in its window while changing containment
            for holder in entry.holders { holder.view.addSubview(holder.node) }
        }
        syncInsets()
        presenter.owners.publish()
    }

    private func retire(_ entry: Entry) {
        let ids = children[entry.node.id] ?? entry.panes.map(\.id)
        let held = entry.holders.filter { $0.node.superview === $0.view }.map(\.node)
        for pane in held where presenter.views[pane.id] !== pane || !ids.contains(pane.id) { pane.removeFromSuperview() }
        for id in ids {
            if let pane = held.first(where: { $0.id == id }), presenter.views[id] === pane { entry.node.container.addSubview(pane) }
        }
        entry.controller.willMove(toParent: nil)
        entry.controller.view.removeFromSuperview()
        entry.controller.removeFromParent()
    }

    /// Follow authored pane order across the UIKit containment boundary.
    func panes(in node: NodeView) -> [NodeView]? {
        guard let entry = entries[node.id] else { return nil }
        return entry.holders.filter { !entry.isHidden($0) && presenter.views[$0.node.id] === $0.node }.map(\.node)
    }

    private func layoutChanged() {
        guard !layoutPending else { return }
        layoutPending = true
        // Read UIKit's safe area after its current layout has finished
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            layoutPending = false
            guard !entries.isEmpty || !insetScrollers.isEmpty else { return }
            syncInsets()
            presenter.owners.publish()
            presenter.navigation.coversChanged()
        }
    }

    /// Peer scrollers share a route edge only when every visible pane can own its inset.
    func scrollers(in route: NodeView, top: Bool) -> [NodeView] {
        entries.values.flatMap { entry -> [NodeView] in
            let node = entry.node, room = node.contentBox()
            guard node.isDescendant(of: route), node.transform.isIdentity, node.clipPath == nil,
                  top ? room.minY <= 0.5 : node.bounds.maxY - room.maxY <= 0.5,
                  reachesEdge(node, in: route, top: top) else { return [] }
            let holders = entry.holders.filter { !entry.isHidden($0) && !$0.view.isHidden && !$0.node.isHidden && !$0.view.bounds.isEmpty }
            guard !holders.isEmpty, holders.allSatisfy({ holder in
                let pane = holder.node, frame = pane.convert(pane.bounds, to: holder.view)
                let allocation = holder.view.convert(holder.view.bounds, to: node)
                let reaches = abs(top ? allocation.minY : node.bounds.maxY - allocation.maxY) < 1
                return reaches && pane.scrollView != nil && pane.scrollsVertically && pane.transform.isIdentity && pane.clipPath == nil && NavigationHost.inFlow(pane)
                    && abs(top ? frame.minY : holder.view.bounds.maxY - frame.maxY) < 1
            }) else { return [] }
            return holders.map(\.node)
        }
    }

    private func reachesEdge(_ node: NodeView, in route: NodeView, top: Bool) -> Bool {
        var child = node
        while child !== route {
            guard !child.isHidden, NavigationHost.inFlow(child) else { return false }
            var up = child.superview
            while let view = up, !(view is NodeView) {
                guard !(view is UIScrollView), view.transform.isIdentity, view.layer.mask == nil else { return false }
                up = view.superview
            }
            guard let parent = up as? NodeView, parent.transform.isIdentity, parent.clipPath == nil, !parent.isHidden else { return false }
            let room = parent.contentBox()
            let rect = child.convert(child.bounds, to: parent)
            let distance = top ? rect.minY - room.minY : room.maxY - rect.maxY
            if parent === route {
                var cover: CGFloat = 0
                if case .edges(let edges)? = presenter.navigation.covers[parent.id] { cover = top ? edges.top : edges.bottom }
                // The route may still reserve this edge, or its scrollers may already own it
                guard abs(distance) < 1 || abs(distance - cover) < 1 else { return false }
            } else {
                guard abs(distance) < 1, top ? room.minY <= 0.5 : parent.bounds.maxY - room.maxY <= 0.5 else { return false }
            }
            child = parent
        }
        return true
    }

    private func syncInsets() {
        guard !syncingInsets else { return }
        syncingInsets = true
        defer { syncingInsets = false }
        syncSafeAreas()
        var now: [UInt32: UIScrollView] = [:]
        for route in presenter.navigation.controllers.values where !NavigationHost.transparentBar(route.node) {
            for node in scrollers(in: route.node, top: true) + scrollers(in: route.node, top: false) {
                now[node.id] = node.scrollView
            }
        }
        let previous = insetScrollers
        insetScrollers = now
        for (id, scroll) in previous where now[id] !== scroll {
            let node = presenter.views[id]
            let saved = pendingTops.removeValue(forKey: id)
            if hiddenPanes.contains(id), let node {
                let top = node.pendingScrollTop.map { CGFloat($0) } ?? saved ?? (scroll.contentOffset.y + node.scrollTopInset(scroll))
                node.hiddenScroll = CGPoint(x: scroll.contentOffset.x, y: top)
            }
            if let node, presenter.navigation.controllers.values.contains(where: { $0.collapseScroll === node || $0.edgeScroll === node }) { continue }
            let top = scroll.contentOffset.y + (node?.scrollTopInset(scroll) ?? scroll.adjustedContentInset.top)
            scroll.contentInsetAdjustmentBehavior = .never
            node?.scrollOrigin = 0; node?.scrollCollapsed = 0
            scroll.contentOffset.y = top - (node?.scrollTopInset(scroll) ?? 0)
        }
        for (id, scroll) in now {
            guard let node = presenter.views[id] else { continue }
            let top = scroll.contentOffset.y + node.scrollTopInset(scroll)
            if previous[id] !== scroll {
                pendingTops[id] = node.pendingScrollTop.map { CGFloat($0) } ?? node.hiddenScroll?.y ?? top
            }
            if scroll.contentInsetAdjustmentBehavior != .automatic {
                scroll.contentInsetAdjustmentBehavior = .automatic
                scroll.contentOffset.y = top - node.scrollTopInset(scroll)
            }
        }
        for entry in entries.values {
            for holder in entry.holders {
                let scroll = now[holder.node.id]
                if holder.contentScrollView(for: .top) !== scroll || holder.contentScrollView(for: .bottom) !== scroll {
                    holder.setContentScrollView(scroll, for: .all)
                }
            }
        }
        finishScrollHandoffs()
    }

    /// Forward the enclosing bars where nested UIKit containment omits their safe area.
    private func syncSafeAreas() {
        for entry in entries.values {
            guard let route = ViewPresentation.controllers(for: entry.node).compactMap({ $0 as? RouteController }).first,
                  let window = route.view.window else { continue }
            for holder in entry.holders where holder.view.window === window {
                let frame = holder.view.convert(holder.view.bounds, to: route.view)
                let room = route.view.bounds.inset(by: route.view.safeAreaInsets)
                let expected = UIEdgeInsets(top: max(0, min(frame.height, room.minY - frame.minY)), left: 0,
                                           bottom: max(0, min(frame.height, frame.maxY - room.maxY)), right: 0)
                let actual = holder.view.safeAreaInsets, extra = holder.additionalSafeAreaInsets
                let needed = UIEdgeInsets(top: max(0, expected.top - max(0, actual.top - extra.top)), left: 0,
                                         bottom: max(0, expected.bottom - max(0, actual.bottom - extra.bottom)), right: 0)
                if needed != extra { holder.additionalSafeAreaInsets = needed }
            }
        }
    }

    private func finishScrollHandoffs() {
        // Finish adoption after UIKit has laid out the pane under its bars
        for (id, top) in pendingTops {
            guard let node = presenter.views[id], let scroll = insetScrollers[id] else { continue }
            if scroll.isTracking || scroll.isDragging || scroll.isDecelerating || (node.pendingScrollTop != nil && node.extras?.pendingScrollTransfer != true) {
                pendingTops[id] = nil
                continue
            }
            let point = PixelGrid.offset(CGPoint(x: scroll.contentOffset.x, y: top - node.scrollTopInset(scroll)), in: scroll)
            if scroll.contentOffset != point { scroll.contentOffset = point }
            guard let route = ViewPresentation.controllers(for: node).compactMap({ $0 as? RouteController }).first else { continue }
            let frame = scroll.convert(scroll.bounds, to: route.view)
            let expected = max(0, route.view.safeAreaInsets.top - frame.minY)
            if abs(scroll.safeAreaInsets.top - expected) < 1 {
                scroll.contentOffset = PixelGrid.offset(scroll.contentOffset, in: scroll)
                pendingTops[id] = nil
            }
        }
    }

    func cancelScrollHandoff(_ id: UInt32) { pendingTops[id] = nil }

    /// Native-hidden panes take no room in the authored edge chain.
    var hiddenPanes: [UInt32] {
        entries.values.flatMap { entry in
            entry.holders.filter { entry.isHidden($0) }.map { $0.node.id }
        }
    }

    var blocks: [UInt32: (block: HostBlock, placement: BlockPlacement?)] {
        var out: [UInt32: (block: HostBlock, placement: BlockPlacement?)] = [:]
        for entry in entries.values {
            for holder in entry.holders {
                let bounds = entry.isHidden(holder) ? CGRect.zero : holder.view.bounds
                let rect = bounds.isEmpty ? CGRect.zero : holder.view.convert(bounds, to: entry.node.container)
                out[holder.node.id] = (.size(bounds.size), BlockPlacement(rect: rect, within: entry.node.id, contentOffset: nil))
            }
        }
        return out
    }

    func remove(_ id: UInt32) {
        let request = requests.removeValue(forKey: id)
        let entry = entries.removeValue(forKey: id)
        if let entry { retire(entry) }
        children[id] = nil
        guard request != nil || entry != nil else { return }
        presenter.afterBatch { [weak self] in self?.sync() }
    }

    func reset() {
        for entry in entries.values { retire(entry) }
        entries = [:]; requests = [:]; children = [:]
        syncInsets()
    }
}
#endif

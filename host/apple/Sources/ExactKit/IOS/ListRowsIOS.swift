// Which batches touch only a virtualized list's rows (iOS, tvOS): a fling's
// fills build, move and drop rows many times a second, and the route and
// bar projection reads nothing in them.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// Whether every op in `batch` is a list snapshot or lands in a
    /// virtualized list's rows: on a node under one, on a node the batch
    /// places under one, or on the list's own children, content size, frame
    /// or rank. A node on the way up to the list that the projection reads
    /// (a header, a tab, tablist or tabpanel, a route, the stack's Back
    /// control), a list that is a route or a routes' owner, or one inside a
    /// header, a tab or the Back control makes the op count. Read before the batch applies,
    /// while its destroys still have views.
    func onlyListRows(_ batch: Batch) -> Bool {
        guard !batch.controls, !batch.ops.isEmpty else { return false }
        let back = navigation.container?.props["navigationBack"]
        func projected(_ props: [String: String], kind: String) -> Bool {
            props["semanticTag"] == "header" || ["tab", "tablist", "tabpanel"].contains(props["accessibilityRole"] ?? "")
                || props["navigationKey"] != nil || props["navigationBack"] != nil || (back != nil && props["id"] == back)
        }
        func projected(_ node: NodeView) -> Bool { projected(node.props, kind: node.kind) }
        /// Read with everything under it: a header's items and title, a
        /// tab's face, a tablist's segments, the Back control's title.
        func readsInside(_ node: NodeView) -> Bool {
            node.props["semanticTag"] == "header" || ["tab", "tablist"].contains(node.props["accessibilityRole"] ?? "")
                || (back != nil && node.props["id"] == back)
        }
        var placed: [UInt32: UInt32] = [:], created: [UInt32: BatchOp] = [:]
        for op in batch.ops {
            if op.op == .children { for child in op.ids { placed[child] = op.id } }
            if op.op == .create { created[op.id] = op }
        }
        // What the projection reads by name or role, wherever it is: tab
        // lists (their tint comes from their ancestors), the Back control,
        // each route's named content scroller, the routes themselves.
        var read = chrome.ids("role:tablist").union(chrome.ids("navigationBack"))
        let names = [back].compactMap { $0 } + navigation.controllers.values.compactMap { $0.node.props["navigationScroll"] }
        for name in names { read.formUnion(chrome.named[name] ?? []) }
        read.formUnion(navigation.controllers.keys)
        var lists: [ObjectIdentifier: Bool] = [:]
        /// A virtualized list that is not a route or a routes' owner, that no
        /// header, tab or Back control holds, and that holds nothing the
        /// projection reads.
        func rowsList(_ list: NodeView) -> Bool {
            if let known = lists[ObjectIdentifier(list)] { return known }
            var result = list.kind == "list" && collections.owns(list.id) && !projected(list) && navigation.container !== list
            var up = list.superview
            while result, let u = up { if let n = u as? NodeView, readsInside(n) { result = false }; up = u.superview }
            for id in read where result {
                var up = views[id]?.superview
                while let u = up { if u === list { result = false; break }; up = u.superview }
            }
            lists[ObjectIdentifier(list)] = result
            return result
        }
        var rows: [UInt32: Bool] = [:]
        /// Strictly under a rows list, nothing projected on the way.
        func inRows(_ id: UInt32, _ depth: Int = 0) -> Bool {
            if let known = rows[id] { return known }
            var result = false
            if let view = views[id] {
                var up: UIView? = view
                while let u = up {
                    if let n = u as? NodeView {
                        if projected(n) { break }
                        if n !== view, n.kind == "list" { result = rowsList(n); break }
                    }
                    up = u.superview
                }
            } else if depth < 64, let parent = placed[id] ?? flats.parent(of: id) {
                let own = created[id].map { projected($0.props, kind: $0.kind) } ?? false
                if !own { result = views[parent].map { $0.kind == "list" ? rowsList($0) : inRows(parent, depth + 1) } ?? inRows(parent, depth + 1) }
            } else if created[id] == nil, !flats.isFlat(id), leaving[id] == nil {
                // A node with no view (a layout box the presenter never
                // makes): an op on it does nothing here.
                result = true
            }
            rows[id] = result
            return result
        }
        return batch.ops.allSatisfy { op in
            switch op.op {
            case .collections:
                return true
            case .children, .content, .frame, .rank:
                if let list = views[op.id], list.kind == "list" { return rowsList(list) }
                return inRows(op.id)
            case .create, .props:
                // What it sets may make it projected: a route, a tab, the Back control.
                return !projected(op.props, kind: op.kind) && inRows(op.id)
            case .present:
                // A presented value (a fade, a slide) is read only by a tab's
                // face (a badge's opacity) and what a header or Back shows.
                guard let view = views[op.id] else { return inRows(op.id) }
                var up: UIView? = view
                while let u = up { if let n = u as? NodeView, readsInside(n) { return false }; up = u.superview }
                return true
            case .destroy, .style, .paragraph, .flow:
                return inRows(op.id)
            default:
                return false
            }
        }
    }
}
#endif

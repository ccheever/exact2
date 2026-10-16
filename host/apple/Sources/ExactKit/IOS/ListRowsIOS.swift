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
        let scrolls = Set(navigation.controllers.values.compactMap { $0.node.props["navigationScroll"] })
        // The names a route's header buttons invoke (a route's header is a
        // `header` child of a node with a `navigationKey`): whether each
        // opens a popover is the header rule's to read, wherever that is.
        var invoked = Set<String>()
        func gather(_ node: NodeView) {
            for key in ["commandfor", "popovertarget"] { if let name = node.props[key], !name.isEmpty { invoked.insert(name) } }
            for case let child as NodeView in node.container.subviews { gather(child) }
        }
        for header in chrome.ids("tag:header").compactMap({ views[$0] }) {
            let route = sequence(first: header.superview, next: { $0?.superview }).lazy.compactMap { $0 as? NodeView }.first
            if route?.props["navigationKey"] != nil { gather(header) }
        }
        func projected(_ props: [String: String], kind: String) -> Bool {
            props["semanticTag"] == "header" || ["tab", "tablist", "tabpanel", "radiogroup"].contains(props["accessibilityRole"] ?? "")
                || (props["accessibilityRole"] == "radio" && props["type"] != "radio")
                || props["navigationKey"] != nil || props["navigationBack"] != nil || (back != nil && props["id"] == back)
                || props["id"].map(scrolls.contains) == true || props["id"].map(invoked.contains) == true
        }
        func projected(_ node: NodeView) -> Bool { projected(node.props, kind: node.kind) }
        /// Read with everything under it: a header's items and title, a
        /// tab's face, a tablist's segments, the Back control's title.
        func readsInside(_ node: NodeView) -> Bool {
            node.props["semanticTag"] == "header" || ["tab", "tablist", "radiogroup"].contains(node.props["accessibilityRole"] ?? "")
                || (node.props["accessibilityRole"] == "radio" && node.props["type"] != "radio")
                || (back != nil && node.props["id"] == back)
        }
        // The nodes the batch creates, and those of them the projection
        // reads, decided once: a fill's children ops name every row they
        // place, each asked again below.
        var placed: [UInt32: UInt32] = [:], created = Set<UInt32>(), createdProjected = Set<UInt32>()
        for op in batch.ops {
            if op.op == .children { for child in op.ids { placed[child] = op.id } }
            if op.op == .create {
                created.insert(op.id)
                // The last create of an id is the one read, as before.
                if projected(op.props, kind: op.kind) { createdProjected.insert(op.id) } else { createdProjected.remove(op.id) }
            }
        }
        // What the projection reads by name or role, wherever it is: tab
        // lists (their tint comes from their ancestors), the Back control,
        // each route's named content scroller, the routes themselves.
        var read = chrome.ids("role:tablist").union(chrome.ids("role:radiogroup")).union(chrome.ids("navigationBack"))
        for name in [back].compactMap({ $0 }) + Array(scrolls) { read.formUnion(chrome.named[name] ?? []) }
        let targets = invoked.reduce(into: Set<UInt32>()) { $0.formUnion(chrome.named[$1] ?? []) }
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
                if !createdProjected.contains(id) { result = views[parent].map { $0.kind == "list" ? rowsList($0) : inRows(parent, depth + 1) } ?? inRows(parent, depth + 1) }
            } else if !created.contains(id), !flats.isFlat(id), leaving[id] == nil {
                // A node with no view (a layout box the presenter never
                // makes): an op on it does nothing here.
                result = true
            }
            rows[id] = result
            return result
        }
        return batch.ops.allSatisfy { op in
            // It changes or removes what a header button invokes, a view or a paragraph's run.
            if targets.contains(op.id) { return false }
            if !invoked.isEmpty, op.op == .paragraph || op.op == .destroy {
                // A paragraph's runs, as they were and as the op sets them,
                // or the run it destroys (each node of a subtree has its own).
                let runs = op.runs + (views[op.id]?.inlineText ?? []) + [inlineText(op.id)].compactMap { $0 }
                if runs.contains(where: { $0.props["id"].map(invoked.contains) == true }) { return false }
            }
            switch op.op {
            case .collections:
                return true
            case .children where op.ids.contains { read.contains($0) || createdProjected.contains($0) }:
                // It places something the projection reads (a named scroller, a tablist).
                return false
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

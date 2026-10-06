// Which batches touch only a list's rows (iOS, tvOS): a fling's fills build,
// move and drop rows many times a second, and the passes that project the
// app's routes and bars read nothing inside a list.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// Whether every op in `batch` is a list snapshot, a presented value, or
    /// lands inside a list: on a node under one, on a node the batch places
    /// under one, or on the list's own children, content size, frame or rank.
    /// Read before the batch applies, while its destroys still have views.
    func onlyListRows(_ batch: Batch) -> Bool {
        guard !batch.controls, !batch.ops.isEmpty else { return false }
        var placed: [UInt32: UInt32] = [:]
        for op in batch.ops where op.op == .children { for child in op.ids { placed[child] = op.id } }
        var known: [UInt32: Bool] = [:]
        func underList(_ id: UInt32, _ depth: Int = 0) -> Bool {
            if let k = known[id] { return k }
            var result = false
            if let view = views[id] {
                var up = view.superview
                while let u = up { if let n = u as? NodeView, n.kind == "list" { result = true; break }; up = u.superview }
            } else if depth < 64, let parent = placed[id] ?? flats.parent(of: id) {
                result = views[parent]?.kind == "list" || underList(parent, depth + 1)
            }
            known[id] = result
            return result
        }
        return batch.ops.allSatisfy { op in
            switch op.op {
            case .collections, .present: return true
            case .children, .content, .frame, .rank where views[op.id]?.kind == "list": return true
            case .create, .destroy, .children, .props, .style, .paragraph, .flow, .frame, .content, .rank: return underList(op.id)
            default: return false
            }
        }
    }
}
#endif

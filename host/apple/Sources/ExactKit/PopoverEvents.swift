// HTML queues one toggle task per element, retaining the first oldState and
// the latest newState. Deliver after presentation and the command batch finish.
import Foundation

final class PopoverEvents {
    private weak var presenter: Presenter?
    private struct Key: Hashable {
        let object: ObjectIdentifier
        let id: UInt32
        let incarnation: UInt64
        init(_ node: NodeView) {
            object = ObjectIdentifier(node); id = node.id
            #if os(iOS) || os(tvOS)
            incarnation = node.incarnation
            #else
            incarnation = 0
            #endif
        }
    }
    private struct Pending {
        weak var node: NodeView?
        let old: Bool
        var new: Bool
    }
    private var pending: [Key: Pending] = [:]
    init(_ presenter: Presenter) { self.presenter = presenter }
    func changed(_ node: NodeView, open: Bool) {
        guard node.props["popover"] != nil, node.handlers.contains("toggle") else { return }
        let key = Key(node)
        if var event = pending[key] { event.new = open; pending[key] = event; return }
        pending[key] = Pending(node: node, old: !open, new: open)
        DispatchQueue.main.async { [weak self] in
            guard let self, let event = self.pending.removeValue(forKey: key), let node = event.node,
                  Key(node) == key, let presenter = self.presenter, presenter.views[key.id] === node else { return }
            presenter.clipboard(key.id, 50, "\(event.old ? "open" : "closed"),\(event.new ? "open" : "closed")")
        }
    }
}

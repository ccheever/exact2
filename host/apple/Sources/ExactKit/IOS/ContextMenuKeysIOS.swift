#if os(iOS)
import UIKit

extension Presenter {
    /// Dispatch an authored context menu at the focus left by key handlers.
    /// Text editors keep their own menu, including when an ancestor has one.
    func contextMenuKey() -> Bool {
        guard let focus = focusedNode, focus.window != nil, !focus.inert,
              menus.eligible(focus, contextMenu: true), focus.field == nil, focus.textArea == nil else { return false }
        for view in logicalAncestry(of: focus) {
            guard let node = view as? NodeView else { continue }
            if node.field != nil || node.textArea != nil { return false }
            guard views[node.id] === node, !node.inert, menus.eligible(node, contextMenu: true),
                  (node.handlers.contains("contextmenu") || node.props["contextPopover"] != nil) else { continue }
            let point = focus.convert(CGPoint(x: focus.bounds.midX, y: focus.bounds.midY), to: node)
            let box = node.contentBox(), client = self.client(node.convert(point, to: nil))
            let sample = PointerSample(x: Double(point.x - box.minX), y: Double(point.y - box.minY),
                                       buttons: 0, pressure: 0, type: "mouse", id: 1,
                                       clientX: Double(client.x), clientY: Double(client.y))
            defaultPrevented = false
            let handled = node.handlers.contains("contextmenu")
            if handled { contextmenu(node.id, line: sample.line) }
            let prevented = defaultPrevented
            defaultPrevented = false
            if prevented { return true }
            let opened = ExactEnv.agentPaints ? menus.agentContext(node) : menus.context.showKeyboard(node, at: point)
            return opened || handled
        }
        return false
    }
}
#endif

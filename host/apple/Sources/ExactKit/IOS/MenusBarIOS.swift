// @ref LLP 1021 D3 — a bar item's popover on iOS: a menu (or a
// confirmation) is the item's pull-down; any other popover opens as itself,
// lifted and anchored on the item.
#if os(iOS) || os(tvOS)
import UIKit

extension MenuHost {
    /// Whether a bar item opening `target` is its pull-down (LLP 1021 D3):
    /// a menu-shaped popover, or a confirmation (which its rows present).
    func pullsDown(_ target: NodeView) -> Bool { target.menuShaped || isConfirmation(target) }
    /// A bar item's popover that is no menu, opened (or closed, a second
    /// tap) from the item as a native button in the page opens one: lifted,
    /// anchored on the item (`lift`). False when the item is ineligible or
    /// names nothing it opens now.
    func openFromBar(_ node: NodeView) -> Bool {
        guard let presenter, eligible(node, inBar: true), let invocation = presenter.invocation(of: node), invocation.opens,
              !isConfirmation(invocation.target), let name = invocation.target.props["id"] else { return false }
        // Open before the tap's light dismiss, which would close it first.
        let open = isOpen(invocation.target)
        if open, invocation.action == .show { return true }
        lightDismiss(node)
        if open { drop(name) } else { agentShow(invocation.target, named: name, from: node.id) }
        return true
    }
    /// Under the agent's painted popovers and UIKit's bars (`--chrome
    /// platform`), the tap of a bar item whose button opens a menu: what a
    /// finger opens is UIKit's pull-down, which no public API opens, so its
    /// popover opens painted under the bar, as a content invoker's does, and
    /// `tap <row>` presses a row as the pull-down's action does (LLP
    /// 1116.020 D6). A confirmation it names presents as the pull-down's
    /// provider presents it. Nil when the agent shows UIKit's menus instead
    /// (`--touch platform`: a real touch opens them); false when it opens
    /// nothing now.
    func agentOpen(barItem node: NodeView) -> Bool? {
        guard ExactEnv.agentPaints, let presenter else { return nil }
        guard eligible(node, inBar: true), let invocation = presenter.invocation(of: node), invocation.opens else { return false }
        let target = invocation.target
        if isConfirmation(target) {
            return openConfirmation(from: nil, popover: target, modal: invocation.action == .showModal, dispatchPress: false)
        }
        guard let name = target.props["id"] else { return false }
        lightDismiss(node)
        if !isOpen(target) { agentShow(target, named: name, from: node.id) } else { drop(name) }
        return true
    }
}
#endif

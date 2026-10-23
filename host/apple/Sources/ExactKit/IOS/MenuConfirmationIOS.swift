#if os(iOS) || os(tvOS)
import UIKit

extension MenuHost {
    /// A visible native cell replaces its host-hidden authored section; CSS hiding still withdraws the invoker.
    func groupedProjectionShows(_ source: NodeView, behind view: UIView) -> Bool {
        guard let node = view as? NodeView, node.hiddenByHost, node.style["display"]?.string != "none",
              let lists = presenter?.groupedLists, lists.hides(node),
              case .view(let drawn, let port)? = lists.shown(source) else { return false }
        return drawn.window != nil && !drawn.isHidden && !port.isHidden
    }

    /// An alertdialog-shaped popover has one or more actions and at most one
    /// hide-only cancel: a chooser ("Open in…") is one action per choice.
    /// Retain this owner until native dismissal completes; ids alone are not
    /// enough because a restart can reuse them for unrelated controls.
    #if os(tvOS)
    // tvOS has no popovers; the sheet keeps the adaptive delegate only.
    typealias ConfirmationDelegate = UIAdaptivePresentationControllerDelegate
    #else
    typealias ConfirmationDelegate = UIPopoverPresentationControllerDelegate
    #endif
    final class Confirmation: NSObject, ConfirmationDelegate {
        /// How it shows (LLP 1115 D6): `alert`, the platform's centred
        /// alert, for a confirmation (a message and a cancel, at most three
        /// buttons); `sheet`, UIKit's unanchored action sheet on a compact
        /// screen (its Cancel drawn; iOS 27 places it mid-screen), for a chooser with a cancel; `popover`, the sheet as a
        /// popover at its invoker, where a tap outside is its only way out
        /// (no cancel) or the screen is regular width.
        enum Style: String { case alert, sheet, popover }
        weak var host: MenuHost?
        weak var source: NodeView?
        /// A source element, whether an invoker or an explicit `source=`.
        let sourced: Bool
        let invoked: Bool
        /// Opened by `show-modal`, not as a popover: HTML's popover commands
        /// leave it open, and its dialog `close` closes only this mode
        let modal: Bool
        weak var popover: NodeView?
        let route: String?
        let style: Style
        let alert: UIAlertController
        var finishing = false
        var dismissing = false
        var chosen: NodeView?
        /// An action as presented: the node, the title the sheet shows for
        /// it, and whether it could be chosen.
        final class Presented {
            weak var node: NodeView?
            let title: String
            /// A prompt's follows its action as the fields are typed in
            /// (`followPrompt`), as a hand-built rename's Save does.
            var enabled: Bool
            init(_ node: NodeView, title: String, enabled: Bool) { self.node = node; self.title = title; self.enabled = enabled }
        }
        let actions: [Presented]
        /// The alert's buttons for `actions`, in order.
        var buttons: [UIAlertAction] = []
        /// The hide-only cancel as presented, and the alert's button for it.
        let cancel: Presented?
        var cancelButton: UIAlertAction?
        /// A prompt's text fields (`PromptFieldIOS.swift`).
        var fields: [PromptField] = []
        init(host: MenuHost, source: NodeView?, popover: NodeView, modal: Bool, actions: [Presented], cancel: Presented?, style: Style, title: String?, message: String?) {
            self.host = host; self.source = source; self.popover = popover; self.modal = modal; self.actions = actions; self.cancel = cancel; self.style = style
            sourced = source != nil; invoked = source.map { host.opens($0, popover) } ?? false
            route = host.presenter?.navigation.routeKey(containing: source ?? popover)
            alert = UIAlertController(title: title, message: message, preferredStyle: style == .alert ? .alert : .actionSheet)
        }
        func observeDisappearance() {
            let lifetime = ConfirmationLifetime(frame: .zero)
            lifetime.owner = self
            alert.view.addSubview(lifetime)
        }
        func owns(_ node: NodeView) -> Bool { actions.contains { $0.node === node } }
        func adaptivePresentationStyle(for controller: UIPresentationController) -> UIModalPresentationStyle { .none }
        #if !os(tvOS)
        func popoverPresentationControllerDidDismissPopover(_ controller: UIPopoverPresentationController) {
            host?.cancelled(self)
        }
        #endif
        func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
            host?.cancelled(self)
        }
    }

}
private final class ConfirmationLifetime: UIView {
    weak var owner: MenuHost.Confirmation?
    private var mounted = false
    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil { mounted = true; return }
        guard mounted else { return }
        DispatchQueue.main.async { [weak self] in
            guard let owner = self?.owner, owner.alert.presentingViewController == nil, !owner.finishing else { return }
            owner.host?.cancelled(owner)
        }
    }
}

#endif

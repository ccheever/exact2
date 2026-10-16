// @ref LLP 1123 D2 — a presented route's logical ancestors hear its touches.
#if os(iOS)
import UIKit

/// What a presented route's or a modal dialog's authored ancestors listen
/// for, carried by the view its sheet mounts it in. UIKit gives a touch to
/// the recognizers of the views it falls in, and a sheet's content is
/// mounted out of the document it is declared in, so an ancestor's own
/// recognizers no longer see it. As a DOM event bubbles to an ancestor,
/// the mount's recognizers give a touch to the nearest ancestor that listens
/// — a pointer observer, a long press (`contextmenu`), a double tap
/// (`dblclick`) — unless a nearer listener in the sheet takes it with its
/// own recognizer. Recognition and arbitration stay UIKit's.
final class MountRelay: NSObject, UIGestureRecognizerDelegate {
    private weak var presenter: Presenter?
    unowned let mount: UIView
    private let pointer = PointerRecognizer(target: nil, action: nil)
    private let press = UILongPressGestureRecognizer()
    private let double = UITapGestureRecognizer()
    private weak var pressed, doubled: NodeView?

    init(_ presenter: Presenter, mount: UIView) {
        self.presenter = presenter
        self.mount = mount
        super.init()
        pointer.relay = self
        press.addTarget(self, action: #selector(longPressed(_:)))
        double.addTarget(self, action: #selector(doubleTapped(_:)))
        double.numberOfTapsRequired = 2
        for g in [pointer, press, double] as [UIGestureRecognizer] {
            // A recognized long press takes its touch, as a node's own does:
            // no press follows a context menu
            g.cancelsTouchesInView = g === press; g.delaysTouchesBegan = false; g.delaysTouchesEnded = false
            if g !== pointer { g.delegate = self }
            mount.addGestureRecognizer(g)
        }
    }

    /// The nearest node above `touched`, through logical parents, that
    /// `listens`, when it is outside the mount: nil when none does or when
    /// one in the sheet is nearer, whose own recognizer takes the touch.
    func listener(_ touched: UIView?, _ listens: (NodeView) -> Bool) -> NodeView? {
        var view = touched
        while let v = view {
            if let n = v as? NodeView, listens(n), !n.disabled { return n.isDescendant(of: mount) ? nil : n }
            view = presenter?.pointerParent(of: v) ?? v.superview
        }
        return nil
    }

    func gestureRecognizer(_ g: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // What a node's own long press and double tap leave to native input
        // (`NodeView.gestureRecognizer(_:shouldReceive:)`)
        if g !== pointer, NodeView.nativeInputOwns(touch.view, below: mount) { return false }
        if g === press { pressed = listener(touch.view) { $0.handlers.contains("contextmenu") }; return pressed != nil }
        if g === double { doubled = listener(touch.view) { $0.handlers.contains("dblclick") }; return doubled != nil }
        return true
    }
    @objc private func longPressed(_ g: UILongPressGestureRecognizer) { pressed?.openContext(g) }
    @objc private func doubleTapped(_ g: UITapGestureRecognizer) { doubled?.doubleClicked(g) }
}
#endif

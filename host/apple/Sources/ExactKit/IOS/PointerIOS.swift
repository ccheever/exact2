// `pointerdown` and `pointerup` on UIKit (LLP 1005 §3; Charlie,
// 2026-10-03): DOM's names for a touch going down on a node and coming up
// or being cancelled. An observer, not a gesture: it never recognizes, so it
// takes nothing from a press, a pan, a long press or the scroll view, and
// it fires before any of them has decided.
#if os(iOS)
import UIKit

final class PointerRecognizer: UIGestureRecognizer {
    weak var node: NodeView?
    private var touch: UITouch?
    /// No touch held: a pooled row may keep it (it is synced with the
    /// handlers it is reused with).
    var idle: Bool { touch == nil }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesBegan(touches, with: event)
        guard touch == nil, let first = touches.first, let node, !node.disabled, !nearer(first.view) else { return }
        touch = first
        if node.handlers.contains("pointerdown") { node.presenter?.pointer(node.id, down: true) }
    }
    /// Whether an enabled pointer node between the touched view and this
    /// one takes the touch: the innermost does, as on the web and macOS.
    func nearer(_ touched: UIView?) -> Bool {
        var view = touched
        while let v = view, v !== node {
            if let n = v as? NodeView, n.wantsPointer, !n.disabled { return true }
            view = v.superview
        }
        return false
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesEnded(touches, with: event)
        lift(touches)
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesCancelled(touches, with: event)
        lift(touches)
    }
    private func lift(_ touches: Set<UITouch>) {
        guard let held = touch, touches.contains(held) else { return }
        touch = nil
        if let node, node.handlers.contains("pointerup") { node.presenter?.pointer(node.id, down: false) }
        state = .failed
    }
    override func reset() {
        super.reset()
        touch = nil
    }
    // Never in anyone's way.
    override func canPrevent(_ preventedGestureRecognizer: UIGestureRecognizer) -> Bool { false }
    override func canBePrevented(by preventingGestureRecognizer: UIGestureRecognizer) -> Bool { false }
}

extension NodeView {
    /// Install or remove the node's pointer observer with its handlers.
    var wantsPointer: Bool { handlers.contains("pointerdown") || handlers.contains("pointerup") }
    func syncPointerRecognizer() {
        let wants = wantsPointer
        let current = gestureRecognizers?.first { $0 is PointerRecognizer }
        if wants, current == nil {
            let g = PointerRecognizer(target: nil, action: nil)
            g.node = self
            g.cancelsTouchesInView = false; g.delaysTouchesBegan = false; g.delaysTouchesEnded = false
            addGestureRecognizer(g)
        } else if !wants, let current {
            removeGestureRecognizer(current)
        }
    }
}
#elseif os(tvOS)
import UIKit

// tvOS touches no node: the Siri Remote moves focus (`RemoteTVOS.swift`), so
// nothing goes down on a node and no observer is installed.
final class PointerRecognizer: UIGestureRecognizer {
    var idle: Bool { true }
}

extension NodeView {
    func syncPointerRecognizer() {}
}
#endif

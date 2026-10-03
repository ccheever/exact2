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

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesBegan(touches, with: event)
        guard touch == nil, let first = touches.first, let node, !node.disabled else { return }
        touch = first
        if node.handlers.contains("pointerdown") { node.presenter?.pointer(node.id, down: true) }
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
    func syncPointerRecognizer() {
        let wants = handlers.contains("pointerdown") || handlers.contains("pointerup")
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
#endif

// `pointerdown` and `pointerup` on UIKit (LLP 1005 §3; Charlie,
// 2026-10-03): DOM's names for a touch going down on a node and coming up
// or being cancelled, and `pointermove` while it is down (LLP 1056 §3 stage
// 3: UIKit delivers moves once a frame; a touch has no hover, and an iPad's
// pointer hovering is not delivered). An observer, not a gesture: it never
// recognizes, so it takes nothing from a press, a pan, a long press or the
// scroll view, and it fires before any of them has decided.
#if os(iOS)
import UIKit

final class PointerRecognizer: UIGestureRecognizer {
    weak var node: NodeView?
    private var touch: UITouch?
    /// The held touch's id, DOM's `pointerId`: the mouse is 1, so a touch
    /// counts on from 2, a new one for each contact.
    private var touchId = 1
    /// No touch held: a pooled row may keep it (it is synced with the
    /// handlers it is reused with).
    var idle: Bool { touch == nil }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesBegan(touches, with: event)
        guard touch == nil, let first = touches.first, let node, !node.disabled, !nearer(first.view) else { return }
        touch = first
        touchId += 1
        if node.handlers.contains("pointerdown") { node.presenter?.pointer(node.id, .down, sample(first)) }
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesMoved(touches, with: event)
        guard let held = touch, touches.contains(held), let node, node.handlers.contains("pointermove") else { return }
        node.presenter?.pointer(node.id, .move, sample(held))
    }
    /// The record of `t` as the node sees it, from its content box: a
    /// pencil's or a 3D Touch's force over its maximum where UIKit measures
    /// one, else DOM's 0.5 while down.
    private func sample(_ t: UITouch, lifted: Bool = false) -> PointerSample {
        guard let node else { return PointerSample(x: 0, y: 0, buttons: 0, pressure: 0, type: "touch", id: touchId) }
        let point = t.location(in: node), box = node.contentBox()
        let type = t.type == .pencil ? "pen" : t.type == .indirectPointer ? "mouse" : "touch"
        let pressure = lifted ? 0 : t.maximumPossibleForce > 0 ? Double(t.force / t.maximumPossibleForce) : 0.5
        // The keys a hardware keyboard holds, an iPad's ⇧ or ⌘ (gallery F20).
        return PointerSample(x: Double(point.x - box.minX), y: Double(point.y - box.minY), buttons: lifted ? 0 : 1,
                             pressure: pressure, type: type, id: type == "mouse" ? 1 : touchId, held: KeyCodes.held(modifierFlags))
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
        if let node, node.handlers.contains("pointerup") { node.presenter?.pointer(node.id, .up, sample(held, lifted: true)) }
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
    var wantsPointer: Bool { handlers.contains("pointerdown") || handlers.contains("pointerup") || handlers.contains("pointermove") }
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

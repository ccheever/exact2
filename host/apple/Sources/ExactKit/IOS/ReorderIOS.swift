// Arrange on UIKit: UIKit recognizes. A handle whose authored touch-action
// keeps the List's pan off a vertical contact starts on the drag itself;
// any handle starts on a long press, which the List's pan beats by moving
// first (scroll always wins).
#if os(iOS) || os(tvOS)
import UIKit
import QuartzCore

extension NodeView {
    func updateReorderGesture() {
        let handle = !(props["reorderFor"] ?? "").isEmpty
        if handle, reorderPan == nil {
            let pan = UIPanGestureRecognizer(target: self, action: #selector(reorderDragged(_:)))
            #if !os(tvOS)
            pan.maximumNumberOfTouches = 1
            #endif
            pan.delegate = self
            let press = UILongPressGestureRecognizer(target: self, action: #selector(reorderDragged(_:)))
            press.delegate = self
            addGestureRecognizer(pan); addGestureRecognizer(press)
            reorderPan = pan; reorderPress = press
        } else if !handle, let pan = reorderPan {
            let previous = reorderHold; reorderHold = nil
            DispatchQueue.main.async { previous?.cancel() }
            removeGestureRecognizer(pan); reorderPan = nil
            if let press = reorderPress { removeGestureRecognizer(press); reorderPress = nil }
        }
    }
    /// Nil for a recognizer that is not Arrange's.
    func reorderShouldBegin(_ gesture: UIGestureRecognizer) -> Bool? {
        if gesture === reorderPress { return SwipeInput.allows(self) }
        guard gesture === reorderPan, let pan = gesture as? UIPanGestureRecognizer else { return nil }
        let v = pan.velocity(in: window), t = pan.translation(in: window)
        let vertical = HeightDragDirection.accepts(velocityX: Double(v.x), velocityY: Double(v.y),
            translationX: Double(t.x), translationY: Double(t.y))
        return SwipeInput.allows(self) && vertical && !allowsTouchPan(CGPoint(x: 0, y: v.y == 0 ? t.y : v.y))
    }
    @objc func reorderDragged(_ gesture: UIGestureRecognizer) {
        let point = gesture.location(in: window), time = CACurrentMediaTime()
        switch gesture.state {
        case .began:
            reorderHold?.cancel()
            reorderOrigin = point
            reorderHold = ReorderHold(self, point: point, time: time)
        case .changed:
            guard let hold = reorderHold else { return }
            if !hold.move(dy: Double(point.y - reorderOrigin.y), point: point, time: time) { reorderHold = nil }
        case .ended, .cancelled, .failed:
            let previous = reorderHold; reorderHold = nil
            previous?.finish(dy: Double(point.y - reorderOrigin.y), point: point, time: time, cancel: gesture.state != .ended)
        default: break
        }
    }
}

extension ReorderHold {
    /// The List's actual scrollTop (what its collection feedback reports), and
    /// where the pointer is against its port.
    func portFacts() -> (top: Double, inside: Bool, offset: Double, height: Double)? {
        guard let presenter, let scroll = presenter.views[state.list]?.scroll,
              let top = presenter.collections.geometry(state.list)?.offset else { return nil }
        let port = scroll.bounds.inset(by: scroll.adjustedContentInset)
        let p = scroll.convert(point, from: nil)
        return (top, port.contains(p), Double(p.y - port.minY), Double(port.height))
    }
    /// Move the List's offset within its content; UIKit's delegate reports it.
    func scrollList(by delta: Double) -> Bool {
        guard let scroll = presenter?.views[state.list]?.scroll, delta.isFinite else { return false }
        let insets = scroll.adjustedContentInset
        let top = -insets.top, bottom = max(top, scroll.contentSize.height + insets.bottom - scroll.bounds.height)
        let y = min(bottom, max(top, scroll.contentOffset.y + CGFloat(delta)))
        guard y != scroll.contentOffset.y else { return false }
        scroll.setContentOffset(CGPoint(x: scroll.contentOffset.x, y: y), animated: false)
        return true
    }
}
#endif

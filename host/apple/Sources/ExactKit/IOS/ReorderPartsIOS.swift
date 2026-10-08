// Dropping across lists on UIKit (LLP 1094 D6): the ghost is a snapshot in
// the window, its port facts and scrollers in window coordinates (y down).
#if os(iOS) || os(tvOS)
import UIKit

/// The lifted row's stand-in, above everything in the window.
final class ReorderGhost {
    let view: UIView
    private weak var window: UIWindow?
    /// Where it stands in the window, unscaled.
    private(set) var frame: CGRect
    init?(of wrapper: NodeView) {
        guard let window = wrapper.window, let snap = wrapper.snapshotView(afterScreenUpdates: false) else { return nil }
        frame = wrapper.convert(wrapper.bounds, to: nil)
        snap.bounds = CGRect(origin: .zero, size: frame.size)
        snap.center = CGPoint(x: frame.midX, y: frame.midY)
        snap.isUserInteractionEnabled = false
        // The host's look (D6): 0 8px 24px at 25% black, and 1.03 unless
        // motion is reduced.
        snap.layer.shadowColor = UIColor.black.cgColor
        snap.layer.shadowOpacity = 0.25
        snap.layer.shadowRadius = 12
        snap.layer.shadowOffset = CGSize(width: 0, height: 8)
        view = snap; self.window = window
    }
    func show() {
        window?.addSubview(view)
        if !DisplayPreferences.reducedMotion { view.transform = CGAffineTransform(scaleX: 1.03, y: 1.03) }
    }
    func place(_ origin: CGPoint) {
        frame.origin = origin
        view.center = CGPoint(x: frame.midX, y: frame.midY)
    }
    /// Spring onto `row` (where the dragged row is now), or fade without one.
    func land(on row: NodeView?, done: @escaping () -> Void) {
        guard let row, row.window != nil else {
            UIView.animate(withDuration: 0.2, animations: { self.view.alpha = 0 }, completion: { _ in done() })
            return
        }
        let target = row.convert(row.bounds, to: nil)
        frame = target
        UIView.animate(withDuration: 0.35, delay: 0, usingSpringWithDamping: 0.85, initialSpringVelocity: 0, options: [],
            animations: {
                self.view.center = CGPoint(x: target.midX, y: target.midY)
                self.view.transform = .identity
                self.view.layer.shadowOpacity = 0
            }, completion: { _ in done() })
    }
    func remove() { view.removeFromSuperview() }
}

/// A grouped list's port in the window, and its scrollTop as its collection
/// feedback reports it (D5).
struct ReorderPort {
    let rect: CGRect
    let scrollTop: Double
    init?(list node: NodeView, presenter: Presenter) {
        guard let scroll = node.scroll, scroll.window != nil,
              let top = presenter.collections.geometry(node.id)?.offset else { return nil }
        rect = scroll.convert(scroll.bounds.inset(by: scroll.adjustedContentInset), to: nil)
        scrollTop = top
    }
    /// The centre's viewport y, less the port's viewport top, plus its scrollTop.
    func contentY(_ p: CGPoint) -> Double { scrollTop + Double(p.y - rect.minY) }
}

/// One scroller autoscroll may move, on its own axis (D7).
struct ReorderScroller {
    let scroll: UIScrollView
    let vertical: Bool
    let rect: CGRect
    /// D7, amended 2026-10-07: an ancestor joins while it holds the centre,
    /// or the contact once a ghost held off its middle has carried its centre
    /// past the edge (that edge's band then measures the centre beyond it).
    static func chain(from list: NodeView, holding centre: CGPoint, contact: CGPoint? = nil) -> [ReorderScroller] {
        var out: [ReorderScroller] = []
        if let scroll = list.scroll, scroll.window != nil {
            let rect = scroll.convert(scroll.bounds.inset(by: scroll.adjustedContentInset), to: nil)
            if centre.x >= rect.minX, centre.x <= rect.maxX { out.append(ReorderScroller(scroll: scroll, vertical: true, rect: rect)) }
        }
        var view = list.superview
        while let current = view {
            if let node = current as? NodeView, let scroll = node.scroll, scroll.window != nil {
                let rect = scroll.convert(scroll.bounds.inset(by: scroll.adjustedContentInset), to: nil)
                if rect.contains(centre) || contact.map({ rect.contains($0) }) == true {
                    let wide = scroll.contentSize.width > scroll.bounds.width + 0.5
                    out.append(ReorderScroller(scroll: scroll, vertical: !wide, rect: rect))
                }
            }
            view = current.superview
        }
        return out
    }
    func direction(_ centre: CGPoint) -> Double {
        vertical ? ReorderEdge.direction(offset: Double(centre.y - rect.minY), height: Double(rect.height))
            : ReorderEdge.direction(offset: Double(centre.x - rect.minX), height: Double(rect.width))
    }
    private var range: (now: CGFloat, low: CGFloat, high: CGFloat) {
        let insets = scroll.adjustedContentInset
        if vertical {
            let low = -insets.top
            return (scroll.contentOffset.y, low, max(low, scroll.contentSize.height + insets.bottom - scroll.bounds.height))
        }
        let low = -insets.left
        return (scroll.contentOffset.x, low, max(low, scroll.contentSize.width + insets.right - scroll.bounds.width))
    }
    func canScroll(toward direction: Double) -> Bool {
        let r = range
        return direction < 0 ? r.now > r.low : direction > 0 ? r.now < r.high : false
    }
    func scroll(by delta: Double) -> Bool {
        let r = range
        let next = min(r.high, max(r.low, r.now + CGFloat(delta)))
        guard next != r.now, delta.isFinite else { return false }
        var offset = scroll.contentOffset
        if vertical { offset.y = next } else { offset.x = next }
        scroll.setContentOffset(offset, animated: false)
        return true
    }
}

extension NodeView {
    /// Where focus goes after a keyboard or custom-action move (D9).
    func focusAfterReorder() { UIAccessibility.post(notification: .layoutChanged, argument: self) }

    /// A grouped grip's drag (D6): the ghost's session, not Arrange's.
    package func reorderGroupDragged(_ gesture: UIGestureRecognizer, point: CGPoint) -> Bool {
        let mine = presenter?.reorderGroup.map { $0.handle === self } ?? false
        guard mine || (gesture.state == .began && reorderGroupList != nil) else { return false }
        switch gesture.state {
        case .began:
            if !mine { _ = ReorderGroupHold(self, point: point, ghost: true) }
        case .changed:
            presenter?.reorderGroup?.move(point)
        case .ended:
            presenter?.reorderGroup?.move(point)
            presenter?.reorderGroup?.finish(cancel: false)
        case .cancelled, .failed:
            presenter?.reorderGroup?.cancel()
        default: break
        }
        return true
    }

    /// "Move earlier", "Move later", "Move to previous list", "Move to next
    /// list" on a grouped grip (D9).
    package override var accessibilityCustomActions: [UIAccessibilityCustomAction]? {
        get {
            guard reorderGroupList != nil else { return super.accessibilityCustomActions }
            return ReorderGroupStep.actions.map { name, step in
                UIAccessibilityCustomAction(name: name) { [weak self] _ in self?.reorderAction(step) ?? false }
            }
        }
        set { super.accessibilityCustomActions = newValue }
    }
}
#endif

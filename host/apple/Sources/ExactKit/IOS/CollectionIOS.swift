// UIKit keeps gesture recognition; this recognizer only observes a bounded pin.
#if os(iOS)
import UIKit

private final class CollectionContact: UIGestureRecognizer {
    weak var collections: CollectionHost?
    private weak var contact: UITouch?
    override func canPrevent(_ other: UIGestureRecognizer) -> Bool { false }
    override func canBePrevented(by other: UIGestureRecognizer) -> Bool { false }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        guard contact == nil, let touch = touches.first else { return }
        contact = touch
        var view = touch.view
        while let current = view {
            if let node = current as? NodeView {
                collections?.pointer(node.id); return
            }
            view = current.superview
        }
        collections?.pointer(nil)
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        if let contact, touches.contains(contact) { collections?.releaseInteractionLater(); self.contact = nil; state = .failed }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        collections?.releaseInteractionLater(); contact = nil; state = .cancelled
    }
    override func reset() { super.reset(); contact = nil }
}

extension CollectionHost {
    private func hidden(_ node: UIView) -> Bool {
        var view: UIView? = node
        while let current = view {
            if current.isHidden { return true }
            view = current.superview
        }
        return false
    }
    func geometry(_ id: UInt32) -> CollectionFacts? {
        guard let node = presenter?.views[id], let scroll = node.scroll, !hidden(node) else { return nil }
        let bounds = scroll.bounds, insets = scroll.adjustedContentInset
        let content = node.contentBox()
        let portWidth = max(0, bounds.width - insets.left - insets.right)
        let portHeight = max(0, bounds.height - insets.top - insets.bottom)
        let available = max(0, portWidth - content.minX - (node.bounds.width - content.maxX))
        let width = entries[id]?.snapshot.rows.first.flatMap { rowWidth($0.view) }.map { CGFloat($0) } ?? available
        guard portWidth.isFinite, portHeight.isFinite, width.isFinite else { return nil }
        return CollectionFacts(top: Double(max(0, scroll.contentOffset.y + insets.top - content.minY)),
            portWidth: Double(portWidth), portHeight: Double(portHeight), rowWidth: Double(width),
            measurements: [], focus: nil, interaction: nil)
    }
    func rowWidth(_ id: UInt32) -> Double? {
        presenter?.views[id].map { Double($0.bounds.width) }
    }
    func height(_ id: UInt32) -> Double? {
        guard let node = presenter?.views[id], !hidden(node) else { return nil }
        return Double(node.bounds.height)
    }
    func correct(_ id: UInt32, top: Double, extent: Double) {
        guard let node = presenter?.views[id], let scroll = node.scroll else { return }
        let content = node.contentBox(), insets = scroll.adjustedContentInset
        let bottom = node.bounds.height - content.maxY
        let height = max(scroll.bounds.height, max(CGFloat(extent) + content.minY + bottom, node.content.height))
        if scroll.contentSize.height != height { scroll.contentSize.height = height }
        // UIKit owns dragging/deceleration. A matching historical anchor is
        // not permission to interrupt that animation with setContentOffset.
        // The next native offset notification supplies the continuing intent.
        guard !scroll.isTracking, !scroll.isDragging, !scroll.isDecelerating else { return }
        let maximum = max(-insets.top, height + insets.bottom - scroll.bounds.height)
        let target = CGPoint(x: scroll.contentOffset.x,
            y: min(maximum, max(-insets.top, CGFloat(top) + content.minY - insets.top)))
        if scroll.contentOffset != target { scroll.setContentOffset(target, animated: false) }
    }
    func focusedView() -> UInt32? {
        presenter?.views.values.first(where: {
            $0.isFirstResponder || $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true
        })?.id
    }
    func startTracking() {
        guard let viewport = presenter?.viewport else { return }
        let recognizer = CollectionContact(target: nil, action: nil)
        recognizer.collections = self
        recognizer.cancelsTouchesInView = false
        recognizer.delaysTouchesBegan = false
        recognizer.delaysTouchesEnded = false
        viewport.addGestureRecognizer(recognizer)
        let observer = NotificationCenter.default.addObserver(forName: UIApplication.willResignActiveNotification,
            object: nil, queue: .main) { [weak self] _ in self?.releaseInteractionLater() }
        stopTracking = { [weak viewport] in
            viewport?.removeGestureRecognizer(recognizer)
            NotificationCenter.default.removeObserver(observer)
        }
    }
}
#endif

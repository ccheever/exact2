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
    /// A list's facts on its own axes (LLP 1070 H1): a row list's offset,
    /// port and measured sizes run along x, its cross along y.
    func geometry(_ id: UInt32) -> CollectionFacts? {
        guard let node = presenter?.views[id], let scroll = node.scroll, !hidden(node) else { return nil }
        let bounds = scroll.bounds, insets = scroll.adjustedContentInset
        let content = node.contentBox()
        let portWidth = max(0, bounds.width - insets.left - insets.right)
        let portHeight = max(0, bounds.height - insets.top - insets.bottom)
        guard portWidth.isFinite, portHeight.isFinite else { return nil }
        let horizontal = entries[id]?.snapshot.horizontal ?? false
        // Animating to a smooth correction: where it is headed is the port
        // the runner plans from (`CollectionHost.animating`).
        let at = (animating.contains(id) ? owedTargets[id] ?? animationTargets[id] : nil) ?? scroll.contentOffset
        let measured = entries[id]?.snapshot.rows.first.flatMap { crossSize($0.view, horizontal: horizontal) }.map { CGFloat($0) }
        if horizontal {
            let available = max(0, portHeight - content.minY - (node.bounds.height - content.maxY))
            let cross = measured ?? available
            guard cross.isFinite else { return nil }
            return CollectionFacts(offset: Double(max(0, at.x + insets.left - content.minX)),
                portMain: Double(portWidth), portCross: Double(portHeight), cross: Double(cross),
                measurements: [], focus: nil, interaction: nil)
        }
        let available = max(0, portWidth - content.minX - (node.bounds.width - content.maxX))
        let width = measured ?? available
        guard width.isFinite else { return nil }
        return CollectionFacts(offset: Double(max(0, at.y + insets.top - content.minY)),
            portMain: Double(portHeight), portCross: Double(portWidth), cross: Double(width),
            measurements: [], focus: nil, interaction: nil)
    }
    /// A row wrapper's size across the list: a vertical list's row width,
    /// a row list's item height.
    func crossSize(_ id: UInt32, horizontal: Bool) -> Double? {
        presenter?.views[id].map { Double(horizontal ? $0.bounds.height : $0.bounds.width) }
    }
    /// A row wrapper's size along the list, what the index measures.
    func size(_ id: UInt32, horizontal: Bool) -> Double? {
        guard let node = presenter?.views[id], !hidden(node) else { return nil }
        return Double(horizontal ? node.bounds.width : node.bounds.height)
    }
    /// An anchor's correction (`CollectionCursor.takeShift`): the offset
    /// moves by `delta` in the layout pass that moved the rows. Assigning
    /// `contentOffset` keeps a pan or a deceleration going from the new
    /// offset at its velocity, as `UICollectionView`'s self-sizing
    /// invalidation does with its content offset adjustment.
    func shift(_ id: UInt32, by delta: Double, extent: Double) {
        guard let node = presenter?.views[id], let scroll = node.scroll else { return }
        fit(node, scroll, extent: extent)
        guard delta != 0, delta.isFinite else { return }
        let horizontal = entries[id]?.snapshot.horizontal ?? false
        if animating.contains(id), let headed = owedTargets[id] ?? animationTargets[id] {
            // Setting the offset would stop the animation where it is: the
            // rows moved, so where it lands moves with them.
            owedTargets[id] = horizontal ? CGPoint(x: headed.x + CGFloat(delta), y: headed.y) : CGPoint(x: headed.x, y: headed.y + CGFloat(delta))
            return
        }
        let insets = scroll.adjustedContentInset
        var target = scroll.contentOffset
        if horizontal { target.x += CGFloat(delta) } else { target.y += CGFloat(delta) }
        if !(scroll.isTracking || scroll.isDragging || scroll.isDecelerating) {
            // At rest the port stays inside the content (under a finger or
            // a fling UIKit's own rubber band owns the overshoot).
            if horizontal {
                let maximum = max(-insets.left, scroll.contentSize.width + insets.right - scroll.bounds.width)
                target.x = min(maximum, max(-insets.left, target.x))
            } else {
                let maximum = max(-insets.top, scroll.contentSize.height + insets.bottom - scroll.bounds.height)
                target.y = min(maximum, max(-insets.top, target.y))
            }
        }
        guard scroll.contentOffset != target else { return }
        let moved = horizontal ? target.x - scroll.contentOffset.x : target.y - scroll.contentOffset.y
        scroll.contentOffset = target
        // Not the reader's travel: the fill's velocity reads on from here.
        presenter?.scrollPump.shifted(id, by: moved)
    }
    /// The content size the list's extent needs.
    private func fit(_ node: NodeView, _ scroll: UIScrollView, extent: Double) {
        let content = node.contentBox()
        if entries[node.id]?.snapshot.horizontal ?? false {
            let right = node.bounds.width - content.maxX
            let width = max(scroll.bounds.width, max(CGFloat(extent) + content.minX + right, node.content.width))
            if scroll.contentSize.width != width { scroll.contentSize.width = width }
        } else {
            let bottom = node.bounds.height - content.maxY
            let height = max(scroll.bounds.height, max(CGFloat(extent) + content.minY + bottom, node.content.height))
            if scroll.contentSize.height != height { scroll.contentSize.height = height }
        }
    }
    /// A smooth correction's target that arrived while one was animating.
    func landAnimation(_ id: UInt32) {
        guard let target = owedTargets.removeValue(forKey: id), let scroll = presenter?.views[id]?.scroll,
              !scroll.isTracking, !scroll.isDragging, !scroll.isDecelerating else { return }
        let gap = abs(target.y - scroll.contentOffset.y) + abs(target.x - scroll.contentOffset.x)
        guard gap > 0.5 else { return }
        let animate = gap > 24 && !ExactEnv.agentFreezes
        if animate {
            animating.insert(id); animationTargets[id] = target
        } else if animating.remove(id) != nil {
            // An ordinary correction stops it where it is.
            animationTargets[id] = nil; owedTargets[id] = nil
        }
        scroll.setContentOffset(target, animated: animate)
    }
    func correct(_ id: UInt32, top: Double, extent: Double, smooth: Bool = false) {
        guard let node = presenter?.views[id], let scroll = node.scroll else { return }
        let content = node.contentBox(), insets = scroll.adjustedContentInset
        let horizontal = entries[id]?.snapshot.horizontal ?? false
        fit(node, scroll, extent: extent)
        // UIKit owns dragging/deceleration. An authored position is not
        // permission to interrupt that animation with setContentOffset (an
        // anchor's correction moves with it instead: `shift`). The next
        // native offset notification supplies the continuing intent.
        guard !scroll.isTracking, !scroll.isDragging, !scroll.isDecelerating else { return }
        let target: CGPoint
        if horizontal {
            let maximum = max(-insets.left, scroll.contentSize.width + insets.right - scroll.bounds.width)
            target = CGPoint(x: min(maximum, max(-insets.left, CGFloat(top) + content.minX - insets.left)),
                y: scroll.contentOffset.y)
        } else {
            let maximum = max(-insets.top, scroll.contentSize.height + insets.bottom - scroll.bounds.height)
            target = CGPoint(x: scroll.contentOffset.x,
                y: min(maximum, max(-insets.top, CGFloat(top) + content.minY - insets.top)))
        }
        guard scroll.contentOffset != target else { return }
        // A smooth correction is UIKit's scroll animation (LLP 1070.000
        // §6.2): the window is already built at the destination; rows in
        // between are not, as a fling's are owed. Under the agent's frozen
        // clock it lands at once.
        let animate = smooth && !ExactEnv.agentFreezes && node.window != nil
        if animate, animating.contains(id) {
            // Already on its way: a new animated set would restart UIKit's
            // ease from rest, and corrections arriving as rows are measured
            // made the scroll creep. It goes on; this target is taken when
            // it lands (`animationEnded`).
            owedTargets[id] = target
            return
        }
        if animate { animating.insert(id); animationTargets[id] = target }
        scroll.setContentOffset(target, animated: animate)
    }
    /// A row's frame in the scroll view, as a range along the list's axis.
    private func span(_ view: UIView, in scroll: UIScrollView, horizontal: Bool) -> ClosedRange<CGFloat> {
        let frame = view.superview === scroll ? view.frame : view.convert(view.bounds, to: scroll)
        return horizontal ? frame.minX...frame.maxX : frame.minY...frame.maxY
    }
    /// Whether the mounted rows cover the scrollport: no spacer (unbuilt
    /// rows) shows. Padding before the first item or after the last counts.
    func covers(_ id: UInt32) -> Bool {
        guard let entry = entries[id], let node = presenter?.views[id], let scroll = node.scroll,
              let first = entry.snapshot.rows.first, let last = entry.snapshot.rows.last else { return false }
        let horizontal = entry.snapshot.horizontal
        let port = horizontal ? scroll.bounds.minX...scroll.bounds.maxX : scroll.bounds.minY...scroll.bounds.maxY
        func box(_ row: CollectionSnapshot.Row) -> ClosedRange<CGFloat>? {
            presenter?.views[row.view].map { span($0, in: scroll, horizontal: horizontal) }
        }
        guard let head = box(first), let tail = box(last) else { return false }
        var reached = first.index == 0 ? max(port.lowerBound, head.lowerBound) : port.lowerBound
        let end = last.index == entry.snapshot.count - 1 ? min(port.upperBound, tail.upperBound) : port.upperBound
        if reached >= end { return true }
        for row in entry.snapshot.rows {
            guard let frame = box(row) else { return false }
            if frame.upperBound <= reached { continue }
            if frame.lowerBound > reached + 0.5 { return false }
            reached = frame.upperBound
            if reached >= end { return true }
        }
        return false
    }
    /// Rows past the mounted ones the port will need once it has travelled
    /// `ahead` points (negative: toward the start), at their mean size.
    func rowsToCover(_ id: UInt32, ahead: CGFloat) -> UInt32 {
        guard ahead != 0, let entry = entries[id], let node = presenter?.views[id], let scroll = node.scroll else { return 0 }
        let horizontal = entry.snapshot.horizontal
        let frames = entry.snapshot.rows.compactMap { row in presenter?.views[row.view].map { span($0, in: scroll, horizontal: horizontal) } }
        guard !frames.isEmpty else { return 0 }
        let mean = frames.map { $0.upperBound - $0.lowerBound }.reduce(0, +) / CGFloat(frames.count)
        guard mean > 0 else { return 0 }
        let port = horizontal ? scroll.bounds.minX...scroll.bounds.maxX : scroll.bounds.minY...scroll.bounds.maxY
        var shortfall: CGFloat = 0
        if ahead > 0 {
            if entry.snapshot.rows.last?.index == entry.snapshot.count - 1 { return 0 }
            var reached = port.lowerBound
            for frame in frames where frame.lowerBound <= reached + 0.5 { reached = max(reached, frame.upperBound) }
            shortfall = port.upperBound + ahead - reached
        } else {
            if entry.snapshot.rows.first?.index == 0 { return 0 }
            var reached = port.upperBound
            for frame in frames.reversed() where frame.upperBound >= reached - 0.5 { reached = min(reached, frame.lowerBound) }
            shortfall = reached - (port.lowerBound + ahead)
        }
        return shortfall > 0 ? UInt32(min(64, (shortfall / mean).rounded(.up))) : 0
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

// UIKit keeps gesture recognition; this recognizer only observes a bounded pin.
#if os(iOS) || os(tvOS)
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
            // From the first item: negative in the padding before it, down
            // to that padding (LLP 1010 §6.9).
            return CollectionFacts(offset: Double(max(-node.number("padding_left"), at.x + insets.left - content.minX)),
                portMain: Double(portWidth), portCross: Double(portHeight), cross: Double(cross),
                measurements: [], focus: nil, interaction: nil)
        }
        let available = max(0, portWidth - content.minX - (node.bounds.width - content.maxX))
        let width = measured ?? available
        guard width.isFinite else { return nil }
        return CollectionFacts(offset: Double(max(-node.number("padding_top"), at.y + insets.top - content.minY)),
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
    /// moves by `delta` from `start` (where the batch began) in the layout
    /// pass that moved the rows. Assigning
    /// `contentOffset` keeps a pan or a deceleration going from the new
    /// offset at its velocity, as `UICollectionView`'s self-sizing
    /// invalidation does with its content offset adjustment.
    func shift(_ id: UInt32, by delta: Double, extent: Double, from start: Double?) {
        guard let node = presenter?.views[id], let scroll = node.scroll else { return }
        fit(node, scroll, extent: extent)
        guard delta != 0, delta.isFinite else { return }
        let horizontal = entries[id]?.snapshot.horizontal ?? false
        if animating.contains(id), let headed = owedTargets[id] ?? animationTargets[id] {
            // The rows moved under a running smooth correction: where it
            // lands moves with them, retargeted from what shows.
            let moved = horizontal ? CGPoint(x: headed.x + CGFloat(delta), y: headed.y) : CGPoint(x: headed.x, y: headed.y + CGFloat(delta))
            owedTargets[id] = nil
            animateOffset(id, scroll, to: moved)
            return
        }
        let insets = scroll.adjustedContentInset
        var target = scroll.contentOffset
        if let start {
            // From where the batch began (`geometry`'s offset): UIKit has
            // already pulled the offset in to content that shrank under it.
            let content = node.contentBox()
            if horizontal { target.x = CGFloat(start) - insets.left + content.minX } else { target.y = CGFloat(start) - insets.top + content.minY }
        }
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
        if ExactEnv.agentFreezes {
            if animating.contains(id) { stopAnimation(id) }
            scroll.setContentOffset(target, animated: false)
        } else {
            animateOffset(id, scroll, to: target)
        }
    }
    /// A smooth correction: one ease-in-out motion of `smoothDuration` from
    /// where the port is now, driven a display frame at a time on the
    /// scroll view's own `contentOffset`, as UIKit's scroll animation is: the
    /// offset every reader sees (hit-testing, a drag's start, stickies, the
    /// scroll event) is the one on screen. A later target while it runs
    /// retargets it from where it is, never a jump (LLP 1010 §6.8): a list
    /// following its end first heads for an estimated row, then the measured
    /// one. UIKit's `setContentOffset(_:animated:)` could only restart from
    /// rest, so a second target waited and landed at once.
    ///
    /// It begins on the next turn with the last target that turn gives: two
    /// motions begun milliseconds apart showed a step back first.
    func animateOffset(_ id: UInt32, _ scroll: UIScrollView, to target: CGPoint) {
        let target = Self.reachable(target, in: scroll)
        if let driver = offsetDrivers[id], driver.scroll === scroll {
            beginAnimation(id, to: target)
            driver.retarget(target, serial: animationSerial[id] ?? 0)
            return
        }
        let pending = startOwed.contains(id) && animating.contains(id)
        let serial = beginAnimation(id, to: target)
        if pending, let token = pendingToken[id] { pendingSerial[id] = serial; _ = token; return }
        // Its own token: a stale start (a list reset and recreated under the
        // same id) neither starts nor stops the one that replaced it.
        nextPendingToken += 1
        let token = nextPendingToken
        startOwed.insert(id)
        pendingToken[id] = token
        pendingSerial[id] = serial
        DispatchQueue.main.async { [weak self, weak scroll] in
            guard let self, self.pendingToken[id] == token else { return }
            self.startOwed.remove(id); self.pendingToken[id] = nil
            let serial = self.pendingSerial.removeValue(forKey: id)
            guard self.animationSerial[id] == serial, let latest = self.animationTargets[id] else { return }
            guard let scroll, scroll.window != nil, self.presenter?.views[id]?.scroll === scroll else {
                // Its view went away before it began: nothing is in flight,
                // and the list reports where it is.
                self.stopAnimation(id)
                self.reportAgain(id)
                return
            }
            // Its target folded to where the port already is (an estimate,
            // then the measured row): no motion.
            if abs(latest.y - scroll.contentOffset.y) + abs(latest.x - scroll.contentOffset.x) < 0.5 {
                if scroll.contentOffset != latest { scroll.setContentOffset(latest, animated: false) }
                self.animationEnded(id, atTarget: true)
                return
            }
            self.startOffsetDriver(id, scroll, to: latest)
        }
    }
    /// `target` within the offsets the port can reach now.
    static func reachable(_ target: CGPoint, in scroll: UIScrollView) -> CGPoint {
        let i = scroll.adjustedContentInset
        let maxX = max(-i.left, scroll.contentSize.width + i.right - scroll.bounds.width)
        let maxY = max(-i.top, scroll.contentSize.height + i.bottom - scroll.bounds.height)
        return CGPoint(x: min(maxX, max(-i.left, target.x)), y: min(maxY, max(-i.top, target.y)))
    }
    private func startOffsetDriver(_ id: UInt32, _ scroll: UIScrollView, to target: CGPoint) {
        let serial = animationSerial[id] ?? beginAnimation(id, to: target)
        let driver = OffsetDriver(scroll: scroll, to: target, duration: Self.smoothDuration, serial: serial) { [weak self] serial, finished in
            guard let self, self.animationSerial[id] == serial else { return }
            self.offsetDrivers[id] = nil
            if finished {
                self.animationEnded(id, atTarget: true)
            } else {
                // Stopped short (its view left the window): it reports where it is.
                self.stopAnimation(id)
                self.reportAgain(id)
            }
        } reclamp: { [weak self] reached in
            self?.animationTargets[id] = reached
        }
        offsetDrivers[id]?.cancel()
        offsetDrivers[id] = driver
        driver.start()
    }
    /// UIKit's batch-update and scroll timing, which Signal's transcript rides.
    static let smoothDuration: TimeInterval = 0.3
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
        // A smooth correction is UIKit's scroll animation (LLP 1070.000
        // §6.2): the window is already built at the destination; rows in
        // between are not (§11 declares it). Under the agent's frozen clock
        // it lands at once.
        let animate = smooth && !ExactEnv.agentFreezes && node.window != nil
        if !animate, animating.contains(id) {
            // An ordinary correction stops it, even where it already is.
            stopAnimation(id)
            scroll.setContentOffset(target, animated: false)
            return
        }
        if animate {
            // On its way or at rest: one motion from what shows, retargeted
            // if one runs (`animateOffset`).
            // A gap under half a point is no motion: 0.3 s of frames for it
            // moved nothing anyone could see.
            if !animating.contains(id), abs(scroll.contentOffset.y - target.y) + abs(scroll.contentOffset.x - target.x) < 0.5 {
                if scroll.contentOffset != target { scroll.setContentOffset(target, animated: false) }
                return
            }
            animateOffset(id, scroll, to: target)
            return
        }
        guard scroll.contentOffset != target else { return }
        scroll.setContentOffset(target, animated: false)
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

#if os(iOS) || os(tvOS)
/// One smooth correction's frames: ease-in-out from where the port was to
/// where it is headed, the offset set each display frame. A retarget once a
/// frame has shown begins a fresh ease from where the port is; one before
/// is still the motion's one target (`retarget`).
final class OffsetDriver: NSObject {
    weak var scroll: UIScrollView?
    private var from: CGPoint
    private(set) var to: CGPoint, began: CFTimeInterval = 0
    /// Whether a frame has moved the port yet.
    private(set) var drawn = false
    private let duration: TimeInterval
    private var serial: Int
    private let done: (Int, Bool) -> Void
    private let reclamp: (CGPoint) -> Void
    init(scroll: UIScrollView, to: CGPoint, duration: TimeInterval, serial: Int, done: @escaping (Int, Bool) -> Void,
         reclamp: @escaping (CGPoint) -> Void) {
        self.scroll = scroll; self.from = scroll.contentOffset; self.to = to
        self.duration = duration; self.serial = serial; self.done = done; self.reclamp = reclamp
    }
    func start() {
        began = CACurrentMediaTime()
        FrameClock.shared.want(self, .offsetDriver, rate: FrameClock.full(on: scroll?.window?.screen)) { [weak self] in self?.frame($0) }
    }
    func retarget(_ target: CGPoint, serial: Int) {
        guard let scroll else { return }
        self.serial = serial
        // Before its first frame the motion has not shown: a followed end's
        // measured row, a report after its estimate, is still its one target
        // (LLP 1010 §6.8). After it, or once the port moved under it (content
        // that shrank clamped it), a fresh ease from what shows.
        if !drawn, scroll.contentOffset == from {
            // Folded back onto the port: no motion at all.
            if abs(target.y - from.y) + abs(target.x - from.x) < 0.5 {
                scroll.contentOffset = target
                cancel(); done(serial, true)
                return
            }
            to = target
            return
        }
        from = scroll.contentOffset; to = target
        began = CACurrentMediaTime()
    }
    func cancel() { FrameClock.shared.drop(self) }
    /// CSS `ease-in-out`, cubic-bezier(0.42, 0, 0.58, 1), as UIKit's.
    static func ease(_ x: Double) -> Double {
        let bez = { (a: Double, b: Double, s: Double) in 3 * a * s * (1 - s) * (1 - s) + 3 * b * s * s * (1 - s) + s * s * s }
        var lo = 0.0, hi = 1.0
        for _ in 0..<32 { let m = (lo + hi) / 2; if bez(0.42, 0.58, m) < x { lo = m } else { hi = m } }
        return bez(0, 1, (lo + hi) / 2)
    }
    private func frame(_ link: CADisplayLink) {
        guard let scroll, scroll.window != nil else { cancel(); done(serial, false); return }
        // Content that shrank under it: from where the port can be now,
        // toward the edge it can reach; every frame stays inside the range.
        let reached = CollectionHost.reachable(to, in: scroll)
        let here = CollectionHost.reachable(scroll.contentOffset, in: scroll)
        if reached != to || here != scroll.contentOffset {
            from = here; to = reached; began = link.targetTimestamp; reclamp(reached)
            if here == reached { scroll.contentOffset = here; cancel(); done(serial, true); return }
        }
        let x = min(1, (link.targetTimestamp - began) / duration)
        let e = CGFloat(Self.ease(max(0, x)))
        let point = CGPoint(x: from.x + (to.x - from.x) * e, y: from.y + (to.y - from.y) * e)
        scroll.contentOffset = CollectionHost.reachable(point, in: scroll)
        drawn = true
        if x >= 1 { cancel(); done(serial, true) }
    }
}
#endif

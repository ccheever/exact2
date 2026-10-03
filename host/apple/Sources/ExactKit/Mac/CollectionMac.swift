// AppKit collection observations are independent of authored event handlers.
#if os(macOS)
import AppKit

/// A scroll container's document view. A collection's is also where
/// AppKit's responsive scrolling asks for overdraw (`prepareContent(in:)`):
/// it may carry only the rows the collection has built, so a concurrent
/// scroll pauses at their edge until the main thread builds more, rather
/// than carrying the background into view (LLP 1050.000 D5).
final class FlippedView: NSView {
    override var isFlipped: Bool { true }
    /// What the mounted rows cover, in this view's coordinates; nil: anything.
    var preparedLimit: (() -> NSRect?)?
    /// AppKit's last request, kept so rows built later can widen the answer.
    private(set) var requestedPrepared: NSRect?
    override func prepareContent(in rect: NSRect) {
        requestedPrepared = rect
        super.prepareContent(in: clampPrepared(rect))
    }
    /// `rect` within the rows' cover. What shows stays prepared, gap or not:
    /// the scroll callback's rescue builds it.
    func clampPrepared(_ rect: NSRect) -> NSRect {
        guard let limit = preparedLimit?() else { return rect }
        let clamped = rect.intersection(limit)
        return clamped.isNull || clamped.isEmpty ? visibleRect : clamped.union(visibleRect)
    }
    /// The rows changed: offer AppKit what they now cover of its request.
    func refreshPrepared() {
        guard preparedLimit != nil else { return }
        let target = clampPrepared(requestedPrepared ?? preparedContentRect)
        if preparedContentRect != target { preparedContentRect = target }
    }
}

/// A knob drag on a collection's scroller. AppKit derives a knob drag's
/// offsets from the pointer, and derives one more at the mouse-up. Rows
/// measured during the drag change the document's height, so that last
/// offset lands elsewhere and the text jumps after the release, which a
/// browser's scrollbar never does. This keeps the offset the reader saw.
/// AppKit's own scroller stays: one set later loses its overlay behavior.
final class KnobDrag {
    private static var key = 0
    static func of(_ scroll: NSScrollView) -> KnobDrag? { objc_getAssociatedObject(scroll, &key) as? KnobDrag }

    private(set) var tracking = false
    private var shown: NSPoint?
    /// The reader saw the document's end: rows measured since can lengthen
    /// the document, and the end is still what they were reading.
    private var shownAtEnd = false
    private weak var scroll: NSScrollView?
    /// A row list's knob is the horizontal scroller's (LLP 1070 §3.3).
    var horizontal = false
    private var observers: [NSObjectProtocol] = []
    var currentEventType: () -> NSEvent.EventType? = { NSApp.currentEvent?.type }

    init(_ scroll: NSScrollView) {
        self.scroll = scroll
        let center = NotificationCenter.default
        // Delivered synchronously: the mouse-up's offset is replaced in the
        // same event, before a frame shows it.
        observers = [
            center.addObserver(forName: NSScrollView.willStartLiveScrollNotification, object: scroll, queue: nil) { [weak self] _ in self?.began() },
            center.addObserver(forName: NSScrollView.didEndLiveScrollNotification, object: scroll, queue: nil) { [weak self] _ in self?.ended() }
        ]
        if let document = scroll.documentView {
            document.postsFrameChangedNotifications = true
            observers.append(center.addObserver(forName: NSView.frameDidChangeNotification, object: document, queue: nil) { [weak self] _ in self?.followEnd() })
        }
        objc_setAssociatedObject(scroll, &Self.key, self, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }
    deinit { observers.forEach { NotificationCenter.default.removeObserver($0) } }

    /// The knob is held where the reader sees the document's end.
    var holdsEnd: Bool { tracking && shownAtEnd }

    /// A live scroll that starts with a press is the knob's; a gesture's
    /// starts with a wheel event and keeps every offset. A press where the
    /// document ends already shows the end.
    func began() {
        tracking = currentEventType() == .leftMouseDown
        shown = nil
        shownAtEnd = tracking && (scroll.map { atEnd($0.contentView) } ?? false)
    }
    /// The clip's offset and its maximum along the list's axis.
    private func travel(_ clip: NSClipView) -> (at: CGFloat, maximum: CGFloat) {
        let document = clip.documentView?.frame.size ?? .zero
        return horizontal
            ? (clip.bounds.minX, max(0, document.width - clip.bounds.width))
            : (clip.bounds.minY, max(0, document.height - clip.bounds.height))
    }
    private func atEnd(_ clip: NSClipView) -> Bool {
        let (at, maximum) = travel(clip)
        return maximum > 0 && at >= maximum - 0.5
    }
    private func point(_ clip: NSClipView, _ along: CGFloat) -> NSPoint {
        horizontal ? NSPoint(x: along, y: clip.bounds.minY) : NSPoint(x: clip.bounds.minX, y: along)
    }
    /// Whether the clip view's new offset is one the reader sees. During a
    /// knob drag, the mouse-up's own derivation is not, unless it reaches an
    /// end of the document: a knob held at the end of its track means the end.
    /// A collection's anchor correction moves what is shown but not where the
    /// reader is: after one, a reader at the end is still at the end.
    func admits(_ clip: NSClipView, correcting: Bool = false) -> Bool {
        guard tracking else { return true }
        let (at, maximum) = travel(clip)
        let edge = at <= 0.5 || at >= maximum - 0.5
        if currentEventType() == .leftMouseUp, !edge { return false }
        shown = clip.bounds.origin
        if !correcting { shownAtEnd = atEnd(clip) }
        return true
    }
    func ended() {
        guard tracking else { return }
        tracking = false
        guard let seen = shown, let scroll else { return }
        shown = nil
        let clip = scroll.contentView
        let maximum = travel(clip).maximum
        let target = point(clip, shownAtEnd ? maximum : min(maximum, max(0, horizontal ? seen.x : seen.y)))
        guard clip.bounds.origin != target else { return }
        clip.scroll(to: target)
        scroll.reflectScrolledClipView(clip)
    }
    /// Rows measured while the knob is held at the end lengthen the document.
    /// The knob under the pointer still means the end, so the reader sees the
    /// new end while holding it, and the release has nothing left to move.
    func followEnd() {
        guard holdsEnd, let scroll else { return }
        let clip = scroll.contentView
        let (at, maximum) = travel(clip)
        guard at != maximum else { return }
        clip.scroll(to: point(clip, maximum))
        scroll.reflectScrolledClipView(clip)
    }
}

extension CollectionHost {
    /// The contiguous run of mounted rows that reaches into the port, in the
    /// document's coordinates and across its full cross size. Padding
    /// before the first item or after the last counts as covered. Nil when
    /// no run reaches the port's start: a gap shows there. A row list's run
    /// is along x (LLP 1070 §5.5).
    func preparedCover(_ id: UInt32) -> NSRect? {
        guard let entry = entries[id], let node = presenter?.views[id], let scroll = node.scroll,
              let document = scroll.documentView else { return nil }
        let horizontal = entry.snapshot.horizontal
        let lo = { (r: NSRect) in horizontal ? r.minX : r.minY }, hi = { (r: NSRect) in horizontal ? r.maxX : r.maxY }
        let port = scroll.contentView.bounds, whole = document.bounds
        let frames = entry.snapshot.rows.compactMap { row -> (index: Int, frame: NSRect)? in
            guard let view = presenter?.views[row.view], view.isDescendant(of: document) else { return nil }
            return (row.index, view.superview === document ? view.frame : view.convert(view.bounds, to: document))
        }.sorted { lo($0.frame) < lo($1.frame) }
        var start = 0
        while start < frames.count {
            var end = start
            var reached = hi(frames[start].frame)
            while end + 1 < frames.count, lo(frames[end + 1].frame) <= reached + 0.5 {
                end += 1
                reached = max(reached, hi(frames[end].frame))
            }
            let first = frames[start].index == 0 ? lo(whole) : lo(frames[start].frame)
            let last = frames[end].index == entry.snapshot.count - 1 ? hi(whole) : reached
            if first <= lo(port) + 0.5 && last > lo(port) {
                return horizontal
                    ? NSRect(x: first, y: whole.minY, width: last - first, height: whole.height)
                    : NSRect(x: whole.minX, y: first, width: whole.width, height: last - first)
            }
            start = end + 1
        }
        return nil
    }
    /// Whether the mounted rows cover the scrollport: no spacer shows.
    func covers(_ id: UInt32) -> Bool {
        guard let cover = preparedCover(id), let scroll = presenter?.views[id]?.scroll else { return false }
        let port = scroll.contentView.bounds, whole = scroll.documentView?.bounds ?? port
        if entries[id]?.snapshot.horizontal == true {
            return cover.maxX >= min(port.maxX, whole.maxX) - 0.5
        }
        return cover.maxY >= min(port.maxY, whole.maxY) - 0.5
    }
    /// Rows past the cover the port will need once it has travelled `ahead`
    /// points (negative: toward the start), at the mounted rows' mean size.
    func rowsToCover(_ id: UInt32, ahead: CGFloat) -> UInt32 {
        guard ahead != 0, let entry = entries[id], let scroll = presenter?.views[id]?.scroll,
              let document = scroll.documentView, let cover = preparedCover(id) else { return 0 }
        let horizontal = entry.snapshot.horizontal
        let sizes = entry.snapshot.rows.compactMap { presenter?.views[$0.view].map { horizontal ? $0.frame.width : $0.frame.height } }
        guard !sizes.isEmpty else { return 0 }
        let mean = sizes.reduce(0, +) / CGFloat(sizes.count)
        guard mean > 0 else { return 0 }
        // Travel ends at the document's edges; the cover reaches them at the ends.
        let port = scroll.contentView.bounds, extent = document.bounds
        let shortfall = horizontal
            ? (ahead > 0 ? min(extent.maxX, port.maxX + ahead) - cover.maxX : cover.minX - max(extent.minX, port.minX + ahead))
            : (ahead > 0 ? min(extent.maxY, port.maxY + ahead) - cover.maxY : cover.minY - max(extent.minY, port.minY + ahead))
        return shortfall > 0 ? UInt32(min(64, (shortfall / mean).rounded(.up))) : 0
    }
    /// After a batch: each collection's document offers responsive scrolling
    /// only what its rows cover (D5).
    func limitPrepared() {
        for id in entries.keys {
            guard let document = presenter?.views[id]?.scroll?.documentView as? FlippedView else { continue }
            if document.preparedLimit == nil {
                // No run reaching the port's top: nothing past what shows.
                document.preparedLimit = { [weak self] in
                    guard let self, entries[id] != nil else { return nil }
                    return preparedCover(id) ?? .zero
                }
            }
            document.refreshPrepared()
        }
    }
    /// A collection's list keeps the offset a knob drag showed (`KnobDrag`);
    /// other lists keep AppKit's.
    func observeKnobDrags() {
        for id in entries.keys {
            guard let scroll = presenter?.views[id]?.scroll else { continue }
            (KnobDrag.of(scroll) ?? KnobDrag(scroll)).horizontal = entries[id]?.snapshot.horizontal == true
        }
    }

    func orderChildren(_ children: [NodeView], in container: NSView) {
        // Spacer reuse can move a pinned row within the same parent. Detaching
        // it to reorder clears AppKit's first responder, even when its logical
        // selection and the row itself survive the collection commit.
        let ordered = container.subviews.filter { !($0 is NodeView) } + children
        guard !ordered.elementsEqual(container.subviews, by: { $0 === $1 }) else { return }
        var ranks = Dictionary(uniqueKeysWithValues: ordered.enumerated().map { (ObjectIdentifier($0.element), $0.offset) })
        withUnsafeMutablePointer(to: &ranks) { context in
            container.sortSubviews({ left, right, raw in
                let order = raw!.assumingMemoryBound(to: [ObjectIdentifier: Int].self).pointee
                let a = order[ObjectIdentifier(left)]!, b = order[ObjectIdentifier(right)]!
                return a < b ? .orderedAscending : a > b ? .orderedDescending : .orderedSame
            }, context: context)
        }
    }

    func pointerDown(_ view: UInt32?, event: NSEvent) {
        guard contactEvent !== event else { return }
        contactEvent = event
        pointer(view)
    }

    /// A list's facts on its own axes (LLP 1070 H1).
    func geometry(_ id: UInt32) -> CollectionFacts? {
        guard let node = presenter?.views[id], let scroll = node.scroll,
              !node.isHiddenOrHasHiddenAncestor else { return nil }
        let bounds = scroll.contentView.bounds
        let content = node.contentBox()
        guard bounds.width.isFinite, bounds.height.isFinite else { return nil }
        let horizontal = entries[id]?.snapshot.horizontal ?? false
        // Animating to a smooth correction: where it is headed is the port
        // the runner plans from (`CollectionHost.animating`).
        let at = (animating.contains(id) ? owedTargets[id] ?? animationTargets[id] : nil) ?? bounds.origin
        let measured = entries[id]?.snapshot.rows.first.flatMap { crossSize($0.view, horizontal: horizontal) }.map { CGFloat($0) }
        if horizontal {
            let available = max(0, bounds.height - content.minY - (node.bounds.height - content.maxY))
            let cross = measured ?? available
            guard cross.isFinite else { return nil }
            return CollectionFacts(offset: Double(max(0, at.x - content.minX)),
                portMain: Double(max(0, bounds.width)), portCross: Double(max(0, bounds.height)),
                cross: Double(cross), measurements: [], focus: nil, interaction: nil)
        }
        let available = max(0, bounds.width - content.minX - (node.bounds.width - content.maxX))
        let width = measured ?? available
        guard width.isFinite else { return nil }
        return CollectionFacts(offset: Double(max(0, at.y - content.minY)),
            portMain: Double(max(0, bounds.height)), portCross: Double(max(0, bounds.width)),
            cross: Double(width), measurements: [], focus: nil, interaction: nil)
    }
    /// A row wrapper's size across the list.
    func crossSize(_ id: UInt32, horizontal: Bool) -> Double? {
        presenter?.views[id].map { Double(horizontal ? $0.bounds.height : $0.bounds.width) }
    }
    /// A row wrapper's size along the list, what the index measures.
    func size(_ id: UInt32, horizontal: Bool) -> Double? {
        guard let node = presenter?.views[id], !node.isHiddenOrHasHiddenAncestor else { return nil }
        return Double(horizontal ? node.bounds.width : node.bounds.height)
    }
    /// An anchor's correction (`CollectionCursor.takeShift`): the offset
    /// moves by `delta` with the rows that moved; a momentum scroll's next
    /// delta goes on from there.
    func shift(_ id: UInt32, by delta: Double, extent: Double) {
        guard let node = presenter?.views[id], let scroll = node.scroll else { return }
        let clip = scroll.contentView, horizontal = entries[id]?.snapshot.horizontal ?? false
        let content = node.contentBox()
        if animating.contains(id), delta.isFinite, let headed = owedTargets[id] ?? animationTargets[id] {
            // The rows moved under a running animation: the document takes
            // the new extent, where it lands moves with them, and the
            // animation goes on.
            fit(node, scroll, extent: extent, horizontal: horizontal)
            guard delta != 0 else { return }
            owedTargets[id] = horizontal ? NSPoint(x: headed.x + CGFloat(delta), y: headed.y) : NSPoint(x: headed.x, y: headed.y + CGFloat(delta))
            return
        }
        let now = Double(horizontal ? clip.bounds.minX - content.minX : clip.bounds.minY - content.minY)
        correct(id, top: now + (delta.isFinite ? delta : 0), extent: extent)
    }
    /// A smooth correction's target that arrived while one was animating.
    func landAnimation(_ id: UInt32) {
        guard let target = owedTargets.removeValue(forKey: id), let scroll = presenter?.views[id]?.scroll else { return }
        let clip = scroll.contentView
        let gap = abs(target.y - clip.bounds.minY) + abs(target.x - clip.bounds.minX)
        guard gap > 0.5 else { return }
        if gap > 24 && !ExactEnv.agentFreezes {
            animate(id, scroll, to: target)
        } else {
            clip.scroll(to: target)
            scroll.reflectScrolledClipView(clip)
        }
    }
    /// AppKit's own scroll animation, the clip view's animator; its end is
    /// the animation's (`animationEnded`).
    private func animate(_ id: UInt32, _ scroll: NSScrollView, to target: NSPoint) {
        let serial = beginAnimation(id, to: target)
        let clip = scroll.contentView
        NSAnimationContext.runAnimationGroup({ _ in clip.animator().setBoundsOrigin(target) }, completionHandler: { [weak self, weak scroll] in
            if let scroll { scroll.reflectScrolledClipView(scroll.contentView) }
            // A later animation or an ordinary correction took over (AppKit
            // runs a stopped group's completion too).
            guard let self, self.animationSerial[id] == serial, self.animating.contains(id) else { return }
            self.animationEnded(id)
        })
    }
    /// The document the list's extent needs. Preserve the measured kernel
    /// extent when it already contains more than the provisional index (a
    /// newly measured row can be larger).
    private func fit(_ node: NodeView, _ scroll: NSScrollView, extent: Double, horizontal: Bool) {
        guard let document = scroll.documentView else { return }
        let content = node.contentBox(), clip = scroll.contentView
        let size: NSSize
        if horizontal {
            let right = node.bounds.width - content.maxX
            let width = max(clip.bounds.width, CGFloat(extent) + content.minX + right)
            size = NSSize(width: max(width, node.content.width), height: document.frame.height)
        } else {
            let bottom = node.bounds.height - content.maxY
            let height = max(clip.bounds.height, CGFloat(extent) + content.minY + bottom)
            size = NSSize(width: document.frame.width, height: max(height, node.content.height))
        }
        if document.frame.size != size { document.setFrameSize(size) }
    }
    func correct(_ id: UInt32, top: Double, extent: Double, smooth: Bool = false) {
        guard let node = presenter?.views[id], let scroll = node.scroll,
              let document = scroll.documentView else { return }
        let content = node.contentBox()
        let clip = scroll.contentView
        let horizontal = entries[id]?.snapshot.horizontal ?? false
        fit(node, scroll, extent: extent, horizontal: horizontal)
        // The anchor stays put, except under a knob held at the end (`KnobDrag`).
        let holdsEnd = KnobDrag.of(scroll)?.holdsEnd == true
        let target: NSPoint
        if horizontal {
            let maximum = max(0, document.frame.width - clip.bounds.width)
            target = NSPoint(x: holdsEnd ? maximum : min(maximum, max(0, CGFloat(top) + content.minX)), y: clip.bounds.minY)
        } else {
            let maximum = max(0, document.frame.height - clip.bounds.height)
            target = NSPoint(x: clip.bounds.minX, y: holdsEnd ? maximum : min(maximum, max(0, CGFloat(top) + content.minY)))
        }
        // A smooth correction is AppKit's scroll animation (LLP 1070.000
        // §6.2), as UIKit's is on iOS; one already running goes on and
        // takes this target when it lands.
        if smooth && !ExactEnv.agentFreezes && !holdsEnd && node.window != nil {
            if animating.contains(id) { owedTargets[id] = target; return }
            guard clip.bounds.origin != target else { return }
            animate(id, scroll, to: target)
            return
        }
        if animating.remove(id) != nil {
            // An ordinary correction stops it, even where it already is: a
            // zero-length animation replaces the running one.
            animationTargets[id] = nil; owedTargets[id] = nil
            NSAnimationContext.runAnimationGroup({ c in c.duration = 0; clip.animator().setBoundsOrigin(target) })
            scroll.reflectScrolledClipView(clip)
            return
        }
        guard clip.bounds.origin != target else { return }
        clip.scroll(to: target)
        scroll.reflectScrolledClipView(clip)
    }
    func focusedView() -> UInt32? {
        guard let presenter, let responder = presenter.viewport.window?.firstResponder else { return nil }
        for node in presenter.views.values {
            if responder === node || responder === node.textArea ||
                (node.field.flatMap { $0.currentEditor() }.map { responder === $0 } ?? false) { return node.id }
        }
        return nil
    }
    private func nodeID(_ hit: NSView?) -> UInt32? {
        var view = hit
        while let current = view {
            if let node = current as? NodeView, presenter?.views[node.id] === node { return node.id }
            view = current.superview
        }
        return nil
    }
    func startTracking() {
        let monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp,
            .rightMouseDown, .rightMouseUp, .otherMouseDown, .otherMouseUp, .keyDown]) { [weak self] event in
            guard let self, let presenter, event.window === presenter.viewport.window else { return event }
            switch event.type {
            case .leftMouseDown, .rightMouseDown, .otherMouseDown:
                let point = presenter.viewport.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow
                pointerDown(nodeID(presenter.viewport.hitTest(point)), event: event)
            case .leftMouseUp, .rightMouseUp, .otherMouseUp:
                // Release after AppKit delivers this event and any authored
                // handler. Captured generation is checked by the weak helper.
                releaseInteractionLater()
            case .keyDown:
                if event.keyCode == 53 { releaseInteractionLater() }
            default: break
            }
            return event
        }
        let observer = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification,
            object: nil, queue: .main) { [weak self] notification in
                guard let self, let window = notification.object as? NSWindow,
                      window === presenter?.viewport.window else { return }
                releaseInteractionLater()
            }
        stopTracking = {
            if let monitor { NSEvent.removeMonitor(monitor) }
            NotificationCenter.default.removeObserver(observer)
        }
    }
}
#endif

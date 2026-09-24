// AppKit collection observations are independent of authored event handlers.
#if os(macOS)
import AppKit

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
    private weak var scroll: NSScrollView?
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
        objc_setAssociatedObject(scroll, &Self.key, self, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }
    deinit { observers.forEach { NotificationCenter.default.removeObserver($0) } }

    /// A live scroll that starts with a press is the knob's; a gesture's
    /// starts with a wheel event and keeps every offset.
    func began() {
        tracking = currentEventType() == .leftMouseDown
        shown = nil
    }
    /// Whether the clip view's new offset is one the reader sees. During a
    /// knob drag, the mouse-up's own derivation is not, unless it reaches an
    /// end of the document: a knob held at the end of its track means the end.
    func admits(_ clip: NSClipView) -> Bool {
        guard tracking else { return true }
        let maximum = max(0, (clip.documentView?.frame.height ?? 0) - clip.bounds.height)
        let edge = clip.bounds.minY <= 0.5 || clip.bounds.minY >= maximum - 0.5
        if currentEventType() == .leftMouseUp, !edge { return false }
        shown = clip.bounds.origin
        return true
    }
    func ended() {
        guard tracking else { return }
        tracking = false
        guard let seen = shown, let scroll, let document = scroll.documentView else { return }
        shown = nil
        let clip = scroll.contentView
        let maximum = max(0, document.frame.height - clip.bounds.height)
        let target = NSPoint(x: clip.bounds.minX, y: min(maximum, max(0, seen.y)))
        guard clip.bounds.origin != target else { return }
        clip.scroll(to: target)
        scroll.reflectScrolledClipView(clip)
    }
}

extension CollectionHost {
    /// A collection's list keeps the offset a knob drag showed (`KnobDrag`);
    /// other lists keep AppKit's.
    func observeKnobDrags() {
        for id in entries.keys {
            guard let scroll = presenter?.views[id]?.scroll, KnobDrag.of(scroll) == nil else { continue }
            _ = KnobDrag(scroll)
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

    func geometry(_ id: UInt32) -> CollectionFacts? {
        guard let node = presenter?.views[id], let scroll = node.scroll,
              !node.isHiddenOrHasHiddenAncestor else { return nil }
        let bounds = scroll.contentView.bounds
        let content = node.contentBox()
        let left = content.minX, right = node.bounds.width - content.maxX
        let available = max(0, bounds.width - left - right)
        let width = entries[id]?.snapshot.rows.first.flatMap { rowWidth($0.view) }.map { CGFloat($0) } ?? available
        guard bounds.width.isFinite, bounds.height.isFinite, width.isFinite else { return nil }
        return CollectionFacts(top: Double(max(0, bounds.minY - content.minY)),
            portWidth: Double(max(0, bounds.width)), portHeight: Double(max(0, bounds.height)),
            rowWidth: Double(width), measurements: [], focus: nil, interaction: nil)
    }
    func rowWidth(_ id: UInt32) -> Double? {
        presenter?.views[id].map { Double($0.bounds.width) }
    }
    func height(_ id: UInt32) -> Double? {
        guard let node = presenter?.views[id], !node.isHiddenOrHasHiddenAncestor else { return nil }
        return Double(node.bounds.height)
    }
    func correct(_ id: UInt32, top: Double, extent: Double) {
        guard let node = presenter?.views[id], let scroll = node.scroll,
              let document = scroll.documentView else { return }
        let content = node.contentBox()
        let bottom = node.bounds.height - content.maxY
        let height = max(scroll.contentView.bounds.height, CGFloat(extent) + content.minY + bottom)
        // Preserve the measured kernel extent when it already contains more
        // than the provisional index (a newly measured row can be taller).
        let size = NSSize(width: document.frame.width, height: max(height, node.content.height))
        if document.frame.size != size { document.setFrameSize(size) }
        let maximum = max(0, document.frame.height - scroll.contentView.bounds.height)
        let target = NSPoint(x: scroll.contentView.bounds.minX,
            y: min(maximum, max(0, CGFloat(top) + content.minY)))
        if scroll.contentView.bounds.origin != target {
            scroll.contentView.scroll(to: target)
            scroll.reflectScrolledClipView(scroll.contentView)
        }
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

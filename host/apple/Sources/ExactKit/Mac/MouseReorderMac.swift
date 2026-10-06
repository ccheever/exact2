// Arrange on AppKit: a vertical mouse drag that starts inside a `reorderFor`
// handle. A press, field or text area nearer the pointer keeps the click.
#if os(macOS)
import AppKit

final class MouseReorder {
    weak var presenter: Presenter?
    private(set) weak var candidate: NodeView?
    private var downEvent: NSEvent?
    private var origin = CGPoint.zero
    private(set) var hold: ReorderHold?
    /// This contact lifted a grouped grip's ghost (LLP 1094 D6).
    private(set) var grouped = false
    private var escape: Any?
    private var inactive: NSObjectProtocol?
    /// The grouped contact's own events, while its grip is hidden.
    private var contact: Any? { didSet { if let oldValue { NSEvent.removeMonitor(oldValue) } } }

    init(_ presenter: Presenter) {
        self.presenter = presenter
        escape = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.keyCode == 53, self?.hold != nil { self?.cancel(); return nil }
            // A grouped drag cancels before its drop; a hold ignores it (LLP 1094 D8).
            if event.keyCode == 53, self?.grouped == true, self?.presenter?.reorderGroup?.active == true { self?.cancel(); return nil }
            return event
        }
        inactive = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification,
            object: nil, queue: .main) { [weak self] note in
                guard let self, let window = note.object as? NSWindow,
                      window === self.presenter?.viewport.window else { return }
                cancel()
            }
    }
    deinit {
        if let escape { NSEvent.removeMonitor(escape) }
        if let contact { NSEvent.removeMonitor(contact) }
        if let inactive { NotificationCenter.default.removeObserver(inactive) }
    }
    func down(_ node: NodeView, event: NSEvent) {
        guard downEvent !== event else { return }
        cancel(); downEvent = event
        var ancestor: NSView? = node
        while let current = ancestor {
            if let view = current as? NodeView {
                guard SwipeInput.allows(view) else { return }
                if !(view.props["reorderFor"] ?? "").isEmpty {
                    candidate = view; origin = event.locationInWindow; return
                }
                if view.field != nil || view.textArea != nil || view.handlers.contains("press") { return }
            }
            ancestor = current.superview
        }
    }
    func drag(_ event: NSEvent) -> Bool {
        guard let candidate, let presenter, presenter.views[candidate.id] === candidate else { return false }
        let point = event.locationInWindow
        if grouped { presenter.reorderGroup?.move(point); return true }
        if hold == nil, candidate.reorderGroupList != nil {
            // A grouped grip lifts past the slop in any direction (LLP 1094 D6).
            guard hypot(point.x - origin.x, point.y - origin.y) > Gesture.slop else { return false }
            guard ReorderGroupHold(candidate, point: origin, ghost: true) != nil else { self.candidate = nil; return false }
            grouped = true
            // The row's visibility is hidden, so it is not a hit target. The
            // window's own events still carry the drag until the button
            // lifts: returning nil swallows them before a view sees them.
            contact = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDragged, .leftMouseUp]) { [weak self] event in
                guard let self, grouped else { return event }
                if event.type == .leftMouseUp { _ = up(event) } else { presenter.reorderGroup?.move(event.locationInWindow) }
                return nil
            }
            presenter.selection.clear()
            for node in presenter.views.values where node.pressed { node.pressed = false }
            presenter.reorderGroup?.move(point)
            return true
        }
        if hold == nil {
            let dx = point.x - origin.x, dy = point.y - origin.y
            if abs(dx) > Gesture.slop && abs(dx) >= abs(dy) { self.candidate = nil; return false }
            guard abs(dy) > Gesture.slop, abs(dy) > abs(dx) else { return false }
            guard let started = ReorderHold(candidate, point: point, time: event.timestamp) else {
                self.candidate = nil; return false
            }
            hold = started; origin = point
            presenter.selection.clear()
            for node in presenter.views.values where node.pressed { node.pressed = false }
        }
        // AppKit window coordinates grow upward; Arrange's travel grows down.
        if hold?.move(dy: Double(origin.y - point.y), point: point, time: event.timestamp) != true { hold = nil }
        return true
    }
    func up(_ event: NSEvent) -> Bool {
        defer { candidate = nil; downEvent = nil }
        if grouped {
            grouped = false; contact = nil
            presenter?.reorderGroup?.move(event.locationInWindow)
            presenter?.reorderGroup?.finish(cancel: false)
            return true
        }
        guard let held = hold else { return false }
        hold = nil
        let point = event.locationInWindow
        held.finish(dy: Double(origin.y - point.y), point: point, time: event.timestamp, cancel: false)
        return true
    }
    func cancel() {
        let prior = hold
        hold = nil; candidate = nil; downEvent = nil
        prior?.cancel()
        // A grouped session, a key's too, cancels before its drop (D8).
        grouped = false; contact = nil
        presenter?.reorderGroup?.cancel()
    }
    func retire(_ id: UInt32) {
        guard candidate?.id == id else { return }
        let prior = hold
        hold = nil; candidate = nil; downEvent = nil
        DispatchQueue.main.async { prior?.cancel() }
    }
}

extension ReorderHold {
    /// The List's actual scrollTop (what its collection feedback reports), and
    /// where the pointer is against its port. The document is flipped.
    func portFacts() -> (top: Double, inside: Bool, offset: Double, height: Double)? {
        guard let presenter, let scroll = presenter.views[state.list]?.scroll,
              let top = presenter.collections.geometry(state.list)?.offset else { return nil }
        let clip = scroll.contentView, port = clip.bounds
        let p = clip.convert(point, from: nil)
        let offset = clip.isFlipped ? p.y - port.minY : port.maxY - p.y
        return (top, port.contains(p), Double(offset), Double(port.height))
    }
    /// Move the List's offset within its document; the clip view reports it.
    func scrollList(by delta: Double) -> Bool {
        guard let scroll = presenter?.views[state.list]?.scroll, let document = scroll.documentView,
              delta.isFinite else { return false }
        let clip = scroll.contentView
        let maximum = max(0, document.frame.height - clip.bounds.height)
        let y = min(maximum, max(0, clip.bounds.minY + CGFloat(clip.isFlipped ? delta : -delta)))
        guard y != clip.bounds.minY else { return false }
        clip.scroll(to: NSPoint(x: clip.bounds.minX, y: y))
        scroll.reflectScrolledClipView(clip)
        return true
    }
}

// Dropping across lists on AppKit (LLP 1094 D6): the ghost is the row's
// cached display in a topmost image view; facts are in window coordinates,
// which grow upward.
/// The lifted row's stand-in, above everything in the window.
final class ReorderGhost {
    let view: NSImageView
    private weak var window: NSWindow?
    /// Where it stands in the window, unscaled.
    private(set) var frame: CGRect
    private let scale: CGFloat
    init?(of wrapper: NodeView) {
        guard let window = wrapper.window, let rep = wrapper.bitmapImageRepForCachingDisplay(in: wrapper.bounds) else { return nil }
        wrapper.cacheDisplay(in: wrapper.bounds, to: rep)
        let image = NSImage(size: wrapper.bounds.size)
        image.addRepresentation(rep)
        frame = wrapper.convert(wrapper.bounds, to: nil)
        view = NSImageView(image: image)
        view.imageScaling = .scaleAxesIndependently
        view.wantsLayer = true
        // The host's look (D6): 0 8px 24px at 25% black (AppKit's y grows
        // up), and 1.03 unless motion is reduced.
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(0.25)
        shadow.shadowBlurRadius = 24
        shadow.shadowOffset = NSSize(width: 0, height: -8)
        view.shadow = shadow
        scale = DisplayPreferences.reducedMotion ? 1 : 1.03
        self.window = window
    }
    private func shown(_ rect: CGRect, scale: CGFloat) -> CGRect {
        let rect = rect.insetBy(dx: -rect.width * (scale - 1) / 2, dy: -rect.height * (scale - 1) / 2)
        return window?.contentView?.convert(rect, from: nil) ?? rect
    }
    func show() {
        guard let content = window?.contentView else { return }
        view.frame = shown(frame, scale: scale)
        content.addSubview(view, positioned: .above, relativeTo: nil)
    }
    func place(_ origin: CGPoint) {
        frame.origin = origin
        view.frame = shown(frame, scale: scale)
    }
    /// Spring onto `row` (where the dragged row is now), or fade without one.
    func land(on row: NodeView?, done: @escaping () -> Void) {
        guard let row, row.window != nil else {
            NSAnimationContext.runAnimationGroup({ $0.duration = 0.2; view.animator().alphaValue = 0 }, completionHandler: done)
            return
        }
        frame = row.convert(row.bounds, to: nil)
        let target = shown(frame, scale: 1)
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.3
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            view.animator().frame = target
        }, completionHandler: done)
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
        rect = scroll.contentView.convert(scroll.contentView.bounds, to: nil)
        scrollTop = top
    }
    /// The centre's viewport y, less the port's viewport top, plus its scrollTop.
    func contentY(_ p: CGPoint) -> Double { scrollTop + Double(rect.maxY - p.y) }
}

/// One scroller autoscroll may move, on its own axis (D7).
struct ReorderScroller {
    let scroll: NSScrollView
    let vertical: Bool
    let rect: CGRect
    static func chain(from list: NodeView, holding centre: CGPoint) -> [ReorderScroller] {
        var out: [ReorderScroller] = []
        if let scroll = list.scroll, scroll.window != nil {
            let rect = scroll.contentView.convert(scroll.contentView.bounds, to: nil)
            if centre.x >= rect.minX, centre.x <= rect.maxX { out.append(ReorderScroller(scroll: scroll, vertical: true, rect: rect)) }
        }
        var view = list.superview
        while let current = view {
            if let node = current as? NodeView, let scroll = node.scroll, scroll.window != nil {
                let rect = scroll.contentView.convert(scroll.contentView.bounds, to: nil)
                if rect.contains(centre) {
                    let wide = (scroll.documentView?.frame.width ?? 0) > scroll.contentView.bounds.width + 0.5
                    out.append(ReorderScroller(scroll: scroll, vertical: !wide, rect: rect))
                }
            }
            view = current.superview
        }
        return out
    }
    func direction(_ centre: CGPoint) -> Double {
        vertical ? ReorderEdge.direction(offset: Double(rect.maxY - centre.y), height: Double(rect.height))
            : ReorderEdge.direction(offset: Double(centre.x - rect.minX), height: Double(rect.width))
    }
    /// The clip's offset from its document's start along the axis, and its end.
    private var range: (now: CGFloat, high: CGFloat) {
        let clip = scroll.contentView, document = scroll.documentView?.frame.size ?? .zero
        if vertical {
            let high = max(0, document.height - clip.bounds.height)
            return (clip.isFlipped ? clip.bounds.minY : high - clip.bounds.minY, high)
        }
        return (clip.bounds.minX, max(0, document.width - clip.bounds.width))
    }
    func canScroll(toward direction: Double) -> Bool {
        let r = range
        return direction < 0 ? r.now > 0 : direction > 0 ? r.now < r.high : false
    }
    func scroll(by delta: Double) -> Bool {
        let r = range, clip = scroll.contentView
        let next = min(r.high, max(0, r.now + CGFloat(delta)))
        guard next != r.now, delta.isFinite else { return false }
        var origin = clip.bounds.origin
        if vertical { origin.y = clip.isFlipped ? next : r.high - next } else { origin.x = next }
        clip.scroll(to: origin)
        scroll.reflectScrolledClipView(clip)
        return true
    }
}

extension NodeView {
    /// Where focus goes after a keyboard or custom-action move (D9).
    func focusAfterReorder() {
        if acceptsFirstResponder { window?.makeFirstResponder(self) }
    }
    /// "Move earlier", "Move later", "Move to previous list", "Move to next
    /// list" on a grouped grip (D9).
    override func accessibilityCustomActions() -> [NSAccessibilityCustomAction]? {
        guard reorderGroupList != nil else { return super.accessibilityCustomActions() }
        return ReorderGroupStep.actions.map { name, step in
            NSAccessibilityCustomAction(name: name) { [weak self] in self?.reorderAction(step) ?? false }
        }
    }
}
#endif

// Arrange on AppKit: a vertical mouse drag that starts inside a `reorderFor`
// handle. A press, field or text area nearer the pointer keeps the click.
import ExactKit
import CExact
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
#endif

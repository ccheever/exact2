#if os(macOS)
import AppKit

/// Primary-button pan only on a published photo handle subtree. Scroll wheels,
/// keyboard zoom actions and controls keep their existing input paths.
final class MouseTransformDrag {
    weak var presenter: Presenter?
    private(set) weak var candidate: NodeView?
    private var downEvent: NSEvent?
    private var origin = CGPoint.zero
    private(set) var hold: TransformDragHold?
    private var escape: Any?
    private var inactive: NSObjectProtocol?
    init(_ presenter: Presenter) {
        self.presenter = presenter
        escape = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.keyCode == 53, self?.hold != nil { self?.cancel(); return nil }
            return event
        }
        inactive = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification,
            object: nil, queue: .main) { [weak self] note in
                guard let self, let window = note.object as? NSWindow, window === self.presenter?.viewport.window else { return }
                cancel()
            }
    }
    deinit {
        if let escape { NSEvent.removeMonitor(escape) }
        if let inactive { NotificationCenter.default.removeObserver(inactive) }
    }
    func down(_ node: NodeView, event: NSEvent) {
        guard downEvent !== event, event.buttonNumber == 0 else { return }
        cancel(); downEvent = event
        presenter?.transformGeometry.changed()
        var ancestor: NSView? = node
        while let view = ancestor {
            if let node = view as? NodeView {
                guard SwipeInput.allows(node) else { return }
                if presenter?.transformBindings[node.id]?.target != nil {
                    candidate = node; origin = event.locationInWindow; return
                }
                if node.field != nil || node.textArea != nil || node.handlers.contains("press") { return }
            }
            ancestor = view.superview
        }
    }
    func drag(_ event: NSEvent) -> Bool {
        guard let candidate, let presenter, presenter.views[candidate.id] === candidate else { return false }
        let point = event.locationInWindow
        if hold == nil {
            guard hypot(point.x - origin.x, point.y - origin.y) > 4 else { return false }
            guard let started = TransformDragHold(candidate, time: event.timestamp) else { self.candidate = nil; return false }
            hold = started; origin = point
            presenter.selection.clear()
            for view in presenter.views.values where view.pressed { view.pressed = false }
        }
        if hold?.move(dx: Double(point.x - origin.x), dy: Double(origin.y - point.y), time: event.timestamp) != true { cancel() }
        return true
    }
    func up(_ event: NSEvent) -> Bool {
        defer { candidate = nil; downEvent = nil }
        guard let held = hold else { return false }
        hold = nil
        held.finish(dx: Double(event.locationInWindow.x - origin.x), dy: Double(origin.y - event.locationInWindow.y),
            time: event.timestamp, cancel: false)
        return true
    }
    func cancel() {
        let previous = hold; hold = nil; candidate = nil; downEvent = nil
        previous?.cancel()
        let pad = trackpad?.hold; trackpad = nil; pad?.cancel()
    }
    func retire(_ id: UInt32) {
        guard candidate?.id == id || trackpad?.hold.handle?.id == id else { return }
        let previous = hold ?? trackpad?.hold; hold = nil; candidate = nil; downEvent = nil; trackpad = nil
        DispatchQueue.main.async { previous?.cancel() }
    }

    // The trackpad (LLP 1057.001 §4): a magnify gesture pinches the pair about
    // the pointer, and while the photo is zoomed a two-finger scroll pans it,
    // as Preview does; unzoomed, the scroll stays the scroll view's.
    private var trackpad: (hold: TransformDragHold, origin: TransformDragPosition, focal: CGPoint, factor: Double, pan: CGPoint)?
    private var momentum = false
    /// The photo handle at or above `node`, and the pointer from its clip's centre.
    private func handle(_ node: NodeView, _ event: NSEvent) -> (NodeView, CGPoint)? {
        var ancestor: NSView? = node
        while let view = ancestor {
            if let node = view as? NodeView {
                guard SwipeInput.allows(node) else { return nil }
                if let binding = presenter?.transformBindings[node.id], binding.target != nil,
                   let clipID = binding.clip, let clip = presenter?.views[clipID] {
                    // A synthesized gesture has no window: its location is the screen's.
                    let inWindow = event.window == nil ? (clip.window?.convertPoint(fromScreen: event.locationInWindow) ?? event.locationInWindow) : event.locationInWindow
                    let p = clip.convert(inWindow, from: nil)
                    return (node, CGPoint(x: p.x - clip.bounds.midX, y: p.y - clip.bounds.midY))
                }
                if node.field != nil || node.textArea != nil || node.handlers.contains("press") { return nil }
            }
            ancestor = view.superview
        }
        return nil
    }
    private func start(_ node: NodeView, _ event: NSEvent) -> Bool {
        guard hold == nil, trackpad == nil, let (handle, focal) = handle(node, event),
              let started = TransformDragHold(handle, time: event.timestamp),
              let origin = TransformDragPosition(started.current) else { return false }
        trackpad = (started, origin, focal, 1, .zero)
        return true
    }
    private func follow(_ event: NSEvent) {
        guard let t = trackpad else { return }
        let pinched = t.origin.focused(from: t.focal, to: CGPoint(x: t.focal.x + t.pan.x, y: t.focal.y + t.pan.y), factor: t.factor)
        if let values = pinched?.values, t.hold.move(to: values, time: event.timestamp) { return }
        trackpad = nil; t.hold.cancel()
    }
    private func end(_ event: NSEvent, cancel: Bool) {
        guard let t = trackpad else { return }
        trackpad = nil
        t.hold.finish(to: t.hold.current, time: event.timestamp, cancel: cancel)
    }
    func magnify(_ node: NodeView, event: NSEvent) -> Bool {
        switch event.phase {
        case .began: guard start(node, event) else { return false }
            trackpad?.factor = max(0.01, 1 + event.magnification); follow(event)
        case .changed: guard trackpad != nil else { return false }
            trackpad?.factor *= max(0.01, 1 + event.magnification); follow(event)
        case .ended, .cancelled: guard trackpad != nil else { return false }
            end(event, cancel: event.phase == .cancelled)
        default: return trackpad != nil
        }
        return true
    }
    func scroll(_ node: NodeView, event: NSEvent) -> Bool {
        if !event.momentumPhase.isEmpty {
            // The fling after a zoomed pan belongs to the pan: it neither
            // scrolls the page nor moves the photo (no decay driver).
            if event.momentumPhase.contains(.ended) || event.momentumPhase.contains(.cancelled) { defer { momentum = false } }
            return momentum
        }
        switch event.phase {
        case .began:
            momentum = false
            guard let (handle, _) = handle(node, event), let target = presenter?.transformBindings[handle.id]?.target,
                  let scale = presenter?.views[target]?.transformDragModel()?.scale, scale > 1.001, start(node, event) else { return false }
            fallthrough
        case .changed:
            guard trackpad != nil else { return false }
            trackpad?.pan.x += event.scrollingDeltaX; trackpad?.pan.y += event.scrollingDeltaY; follow(event)
        case .ended, .cancelled:
            guard trackpad != nil else { return false }
            momentum = true; end(event, cancel: event.phase == .cancelled)
        default: return trackpad != nil
        }
        return true
    }
}
#endif

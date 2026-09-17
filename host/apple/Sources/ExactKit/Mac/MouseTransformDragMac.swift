#if os(macOS)
import AppKit

/// Primary-button pan only on a published photo handle subtree. Scroll wheels,
/// keyboard zoom actions and controls keep their existing input paths.
final class MouseTransformDrag {
    weak var presenter: Presenter?
    private weak var candidate: NodeView?
    private var downEvent: NSEvent?
    private var origin = CGPoint.zero
    private var hold: TransformDragHold?
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
    }
    func retire(_ id: UInt32) {
        guard candidate?.id == id else { return }
        let previous = hold; hold = nil; candidate = nil; downEvent = nil
        DispatchQueue.main.async { previous?.cancel() }
    }
}
#endif

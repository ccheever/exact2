#if os(macOS)
import AppKit

/// AppKit owns event dispatch. A dominant rightward mouse drag recognizes only
/// on an authored swiperight node; ordinary clicks and vertical drags still pass.
final class MouseSwipe {
    weak var presenter: Presenter?
    private weak var candidate: NodeView?
    private var downEvent: NSEvent?
    private var origin = CGPoint.zero
    private var last = CGPoint.zero
    private var timestamp = 0.0
    private var velocity = 0.0
    private var hold: SwipeHold?
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
                guard let self, let window = note.object as? NSWindow,
                      window === self.presenter?.viewport.window else { return }
                cancel()
            }
    }
    deinit {
        if let escape { NSEvent.removeMonitor(escape) }
        if let inactive { NotificationCenter.default.removeObserver(inactive) }
    }
    func down(_ node: NodeView, event: NSEvent) {
        guard downEvent !== event else { return }
        cancel()
        downEvent = event
        var ancestor: NSView? = node
        while let current = ancestor {
            if let view = current as? NodeView {
                if view.disabled { return }
                if view.handlers.contains("swiperight") {
                    let action = view.style["touch_action"]?.string ?? "auto"
                    guard action == "none" || action.split(separator: " ").contains("pan-y") else { return }
                    candidate = view; origin = event.locationInWindow; last = origin
                    timestamp = event.timestamp; velocity = 0
                    return
                }
                if view.handlers.contains("press") || view.field != nil { return }
            }
            ancestor = current.superview
        }
    }
    func drag(_ event: NSEvent) -> Bool {
        guard let candidate, let presenter, presenter.views[candidate.id] === candidate else { return false }
        let point = event.locationInWindow
        if hold == nil {
            let dx = point.x - origin.x, dy = point.y - origin.y
            if abs(dy) > 4 && abs(dy) >= abs(dx) { self.candidate = nil; return false }
            guard abs(dx) > 4, SwipeRecognition.accepts(x: Double(dx), y: Double(dy), presentedX: Double(candidate.translate.x)) else { return false }
            guard let started = SwipeHold(candidate) else { self.candidate = nil; return false }
            hold = started; origin = point; last = point; timestamp = event.timestamp
            presenter.selection.clear()
            // A drag must not also dispatch the click armed before recognition.
            for node in presenter.views.values where node.pressed { node.pressed = false }
        }
        sample(event)
        guard let hold else { return false }
        if !hold.move(Double(point.x - origin.x)) { cancel() }
        return true
    }
    private func sample(_ event: NSEvent) {
        let elapsed = event.timestamp - timestamp
        if elapsed > 0 { velocity = Double(event.locationInWindow.x - last.x) / elapsed }
        last = event.locationInWindow; timestamp = event.timestamp
    }
    func up(_ event: NSEvent) -> Bool {
        defer { candidate = nil; downEvent = nil }
        guard let held = hold else { return false }
        hold = nil
        sample(event)
        held.finish(displacement: Double(event.locationInWindow.x - origin.x), fingerVelocity: velocity, cancel: false)
        return true
    }
    func cancel() {
        let prior = hold
        hold = nil; candidate = nil; downEvent = nil
        prior?.cancel()
    }
    func retire(_ id: UInt32) {
        guard candidate?.id == id else { return }
        let prior = hold
        hold = nil; candidate = nil; downEvent = nil
        // Destruction is inside a presenter batch; finish after its full receipt.
        DispatchQueue.main.async { prior?.cancel() }
    }
}
#endif

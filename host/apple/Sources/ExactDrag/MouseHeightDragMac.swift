import ExactKit
import CExact
#if os(macOS)
import AppKit

/// Only the authored header subtree recognizes a vertical mouse drag. The inner
/// scrollport is a sibling and keeps ordinary AppKit wheel/selection handling.
final class MouseHeightDrag {
    weak var presenter: Presenter?
    private(set) weak var candidate: NodeView?
    private var downEvent: NSEvent?
    private var origin = CGPoint.zero
    private(set) var hold: HeightDragHold?
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
        cancel(); downEvent = event
        var ancestor: NSView? = node
        while let current = ancestor {
            if let view = current as? NodeView {
                guard SwipeInput.allows(view) else { return }
                if presenter?.heightBindings[view.id]?.target != nil {
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
        if hold == nil {
            let dx = point.x - origin.x, dy = point.y - origin.y
            if abs(dx) > Gesture.slop && abs(dx) >= abs(dy) { self.candidate = nil; return false }
            guard abs(dy) > Gesture.slop, abs(dy) > abs(dx) else { return false }
            guard let started = HeightDragHold(candidate, time: event.timestamp) else {
                self.candidate = nil; return false
            }
            hold = started; origin = point
            presenter.selection.clear()
            for node in presenter.views.values where node.pressed { node.pressed = false }
        }
        // AppKit window coordinates grow upward; shared displacement grows down.
        if hold?.move(downward: Double(origin.y - point.y), time: event.timestamp) != true { cancel() }
        return true
    }
    func up(_ event: NSEvent) -> Bool {
        defer { candidate = nil; downEvent = nil }
        guard let held = hold else { return false }
        hold = nil
        held.finish(downward: Double(origin.y - event.locationInWindow.y), time: event.timestamp, cancel: false)
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
        DispatchQueue.main.async { prior?.cancel() }
    }
}
#endif

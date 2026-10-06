#if os(macOS)
import AppKit

/// TooltipScrollDismissArea (T3 Code 1e2ecbd975, MIT, LICENSE-T3).
/// Hook a pointer-events:none button inside the timeline Tip trigger wrapper.
/// Its press toggles Contract's dismissed state. Native changes no Exact view
/// properties: only actual clip origin changes dismiss; physical mouse motion
/// resets the latch, so the host hovering a resting pointer again after a scroll (exact2 #139)
/// cannot reopen a stationary pointer's tip.
final class T3TimelineTooltip {
    private final class Entry {
        weak var element: ExactElement?
        var origin: NSPoint?
        var dismissed = false
        init(_ element: ExactElement) { self.element = element; origin = element.view?.enclosingScrollView?.contentView.bounds.origin }
        var trigger: NSView? { element?.view?.superview }
        func setDismissed(_ value: Bool) {
            guard value != dismissed else { return }
            dismissed = value
            let element = element
            DispatchQueue.main.async { [weak element] in element?.click() }
        }
    }
    private var entries: [ObjectIdentifier: Entry] = [:]
    private var observation: NSObjectProtocol?
    private var monitor: Any?

    func install(_ element: ExactElement) {
        guard element.hook == .t3TimelineTip, let view = element.view else { return }
        let key = ObjectIdentifier(view)
        if entries[key] == nil { entries[key] = Entry(element) }
        if observation == nil {
            observation = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification, object: nil, queue: .main) { [weak self] note in
                guard let clip = note.object as? NSClipView else { return }
                self?.scrolled(clip)
            }
        }
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged, .rightMouseDragged]) { [weak self] event in
                self?.pointerMoved(in: event.window)
                return event
            }
        }
    }
    func scrolled(_ clip: NSClipView) {
        for entry in entries.values {
            guard let trigger = entry.trigger, trigger.enclosingScrollView?.contentView === clip else { continue }
            let previous = entry.origin
            entry.origin = clip.bounds.origin
            guard let previous, previous != clip.bounds.origin else { continue }
            if let focus = trigger.window?.firstResponder as? NSView, focus === trigger || focus.isDescendant(of: trigger) { continue }
            entry.setDismissed(true)
        }
    }
    func pointerMoved(in window: NSWindow?) {
        for entry in entries.values where entry.trigger?.window === window { entry.setDismissed(false) }
    }
    func remove(_ element: ExactElement) {
        guard element.hook == .t3TimelineTip, let view = element.view else { return }
        entries.removeValue(forKey: ObjectIdentifier(view))
        if entries.isEmpty { destroy() }
    }
    func destroy() {
        if let observation { NotificationCenter.default.removeObserver(observation) }
        if let monitor { NSEvent.removeMonitor(monitor) }
        observation = nil; monitor = nil; entries.removeAll()
    }
}
#endif

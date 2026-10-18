// When the reader last pointed, typed or scrolled: the activity reporter's `recentlyInteracted`
// (backgroundActivityReporter.ts's pointermove, keydown and wheel listeners on the window) and the
// pull request panel's idle rule (pages-pr-refresh.ts). The reporter itself is the data module's
// (src/transport/transport-activity.ts); a data module sees no input outside its elements, so this
// monitor stays native. Becoming recent again after 45 s announces `t3.status`, so the reporter,
// which reads the time with each status, reports at once as the reference's listeners do.
import AppKit

final class T3ActivityInput {
    private let lock = NSLock()
    private var last = Date().timeIntervalSince1970 * 1000
    private let changed: (String) -> Void
    // AppKit objects are used only on the main queue.
    private var observers: [NSObjectProtocol] = []
    private var eventMonitor: Any?
    private let pointerAreas = NSMapTable<NSWindow, NSTrackingArea>.weakToStrongObjects()
    private lazy var pointer = T3ActivityPointer { [weak self] in self?.interaction() }
    private var stopped = false

    init(observeWindows: Bool = true, changed: @escaping (String) -> Void) {
        self.changed = changed
        if observeWindows { DispatchQueue.main.async { [weak self] in self?.observe() } }
    }

    /// Unix ms.
    func lastInteractionAt() -> Double { lock.lock(); defer { lock.unlock() }; return last }

    /// AppKit delivers mouse moves only where something asks for them: each main-capable window's content gets a
    /// tracking area that hears every move (T3ActivityPointer), not `acceptsMouseMovedEvents`, which sends the window's
    /// moves to its first responder.
    private func observe() {
        guard !stopped else { return }
        for name in [NSApplication.didBecomeActiveNotification, NSWindow.didBecomeKeyNotification, NSWindow.didBecomeMainNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in self?.trackPointer() })
        }
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .scrollWheel]) { [weak self] event in
            self?.interaction(); return event
        }
        trackPointer()
    }
    private func trackPointer() {
        for window in NSApp.windows where window.canBecomeMain && window.isVisible && pointerAreas.object(forKey: window) == nil {
            guard let content = window.contentView else { continue }
            let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeAlways, .inVisibleRect], owner: pointer, userInfo: nil)
            content.addTrackingArea(area)
            pointerAreas.setObject(area, forKey: window)
        }
    }
    private func interaction() {
        let now = Date().timeIntervalSince1970 * 1000
        lock.lock()
        let recent = last <= now && now - last <= 45_000
        last = now
        lock.unlock()
        if !recent { changed("t3.status") }
    }

    func destroy() {
        DispatchQueue.main.async { [self] in
            stopped = true
            for window in pointerAreas.keyEnumerator().allObjects.compactMap({ $0 as? NSWindow }) {
                if let area = pointerAreas.object(forKey: window) { window.contentView?.removeTrackingArea(area) }
            }
            pointerAreas.removeAllObjects()
            for observer in observers { NotificationCenter.default.removeObserver(observer) }; observers.removeAll()
            if let eventMonitor { NSEvent.removeMonitor(eventMonitor) }; eventMonitor = nil
        }
    }
}

/// The owner of the monitor's window-wide tracking areas: each move over a window's content is an
/// interaction. AppKit sends a tracking area's moves to its owner alone. A window that accepts mouse
/// moved events sends each one to its first responder instead, and a focused ExactKit node with a
/// `hover` takes it as the pointer over it wherever the pointer is: the Usage page's pressed Cost
/// segment held the hover of the unpriced (i) beside the pointer, its popover closing on each 1 pt
/// move and then not opening at all, under a real pointer.
final class T3ActivityPointer: NSResponder {
    private let moved: () -> Void
    init(_ moved: @escaping () -> Void) { self.moved = moved; super.init() }
    required init?(coder: NSCoder) { nil }
    override func mouseMoved(with event: NSEvent) { moved() }
}

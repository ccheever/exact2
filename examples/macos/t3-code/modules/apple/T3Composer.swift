#if os(macOS)
import AppKit

/// Adds chat Return semantics to Exact's ordinary textarea. Exact keeps its
/// text view, delegate, editing state and authored Send action.
final class T3Composer {
    private weak var composer: ExactElement?
    private weak var send: ExactElement?
    private var keyMonitor: Any?

    func install(_ element: ExactElement) {
        if element.hook == .t3Send {
            send = element
        } else if element.hook == .t3Composer {
            composer = element
            if keyMonitor == nil {
                keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                    guard let self else { return event }
                    return self.handle(event)
                }
            }
        }
    }

    func remove(_ element: ExactElement) {
        if send === element { send = nil }
        if composer === element {
            composer = nil
            stopMonitoring()
        }
    }

    func destroy() {
        stopMonitoring()
        composer = nil
        send = nil
    }

    private func stopMonitoring() {
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        keyMonitor = nil
    }

    /// Internal so focused tests can exercise the actual window/focus guard.
    func handle(_ event: NSEvent) -> NSEvent? {
        switch action(for: event) {
        case .passThrough: return event
        case .send:
            // click() queues the Contract action after any pending input batch;
            // its existing disabled state and canSend validation remain in charge.
            send?.click()
            return nil
        case .consumeRepeat: return nil
        }
    }

    private func action(for event: NSEvent) -> ReturnAction {
        guard let composer, composer.isLive, let editor = composer.textView,
              let window = editor.window, event.window === window,
              window.firstResponder === editor, window.attachedSheet == nil,
              editor.isEditable, let send, send.isLive, send.view?.window === window else { return .passThrough }
        return Self.returnAction(event, hasMarkedText: editor.hasMarkedText())
    }

    enum ReturnAction { case passThrough, send, consumeRepeat }

    static func returnAction(_ event: NSEvent, hasMarkedText: Bool) -> ReturnAction {
        guard event.type == .keyDown, event.keyCode == 36 || event.keyCode == 76,
              !hasMarkedText,
              event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty else {
            return .passThrough
        }
        // Holding Return must neither dispatch twice nor insert a newline after
        // the first send. Return used to commit IME text always reaches AppKit.
        return event.isARepeat ? .consumeRepeat : .send
    }

    deinit { stopMonitoring() }
}
#endif

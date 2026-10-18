// @ref LLP 1126.001 WP12 — store changes cross sessions as a re-ask.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactCore

extension ExactApp {
    func observeSharedStore() {
        #if canImport(UIKit)
        let foreground = UIApplication.willEnterForegroundNotification
        #else
        let foreground = NSApplication.didBecomeActiveNotification
        #endif
        WidgetContainer.observeStore(foreground: foreground) { [weak self] in
            self?.refreshStore { DispatchQueue.main.async { WidgetContainer.reloadAfterStoreChange() } }
        }
    }

    /// @ref LLP 1126.001 WP12 — every live window re-reads before timelines reload.
    package func refreshStore(completion: @escaping () -> Void) {
        let active = sessions.filter { $0.booted && $0.state != .destroyed }
        var remaining = active.count
        guard remaining > 0 else { completion(); return }
        for session in active {
            session.whenIdle {
                if session.state != .destroyed { session.apply(session.runtime.storeChanged()) }
                remaining -= 1
                if remaining == 0 { completion() }
            }
        }
    }
}

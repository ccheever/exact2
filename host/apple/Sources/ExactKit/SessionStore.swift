// @ref LLP 1126.001 WP12 — store changes cross sessions as a re-ask.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactCore

final class SharedStoreRefresh {
    var observer: WidgetStoreObserver?
    private var drawn = false
    private var reload: (() -> Void)?
    func firstPixel() { drawn = true; let work = reload; reload = nil; work?() }
    func reloadAfterPixel(_ work: @escaping () -> Void) { if drawn { work() } else { reload = work } }
}

extension ExactApp {
    func observeSharedStore(root: URL? = nil, appID: String? = nil, kinds: [String]? = nil, reload: (([String]?) -> Void)? = nil) {
        #if canImport(UIKit)
        let foreground = [UIApplication.willEnterForegroundNotification, UIApplication.didBecomeActiveNotification]
        #else
        let foreground = [NSApplication.didBecomeActiveNotification]
        #endif
        let writtenKinds = kinds ?? Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") as? [String]
        sharedStore.observer = WidgetContainer.observeStore(root: root, appID: appID, kinds: kinds, foreground: foreground) { [weak self] ownWrite, finished in
            guard let self else { finished(true); return }
            let after = { (success: Bool) in
                if success {
                    self.sharedStore.reloadAfterPixel {
                        DispatchQueue.main.async {
                            if let reload { reload(writtenKinds) } else { WidgetContainer.reloadAfterStoreChange(kinds: writtenKinds) }
                        }
                    }
                }
                finished(success)
            }
            if ownWrite { after(true) } else { self.refreshStore(completion: after) }
        }
    }

    /// Every live window finishes its refresh before timelines reload.
    package func refreshStore(completion: @escaping (Bool) -> Void) {
        let active = sessions.filter { $0.booted && $0.state != .destroyed }
        var remaining = active.count
        var success = true
        guard remaining > 0 else { completion(true); return }
        for session in active {
            session.refreshSharedStore { refreshed in
                success = success && refreshed
                remaining -= 1
                if remaining == 0 { completion(success) }
            }
        }
    }
}

extension ExactSession {
    func refreshSharedStore(completion: @escaping (Bool) -> Void) {
        guard state != .destroyed else { completion(true); return }
        whenIdle({ [weak self] in
            guard let self, state != .destroyed else { completion(true); return }
            let batch = runtime.storeChanged()
            if batch.busy {
                DispatchQueue.main.async { [weak self] in
                    guard let self else { completion(true); return }
                    refreshSharedStore(completion: completion)
                }
                return
            }
            // An unchanged store returns no commit sequence or timer update.
            if batch.seq != nil { applyUnlessEmpty(batch) }
            completion(batch.error == nil)
        }, cancelled: { [weak self] in
            guard let self else { completion(true); return }
            refreshSharedStore(completion: completion)
        })
    }
}

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
    private var reloads: [() -> Void] = []
    func firstPixel() { drawn = true; let work = reloads; reloads = []; work.forEach { $0() } }
    func reloadAfterPixel(_ work: @escaping () -> Void) { if drawn { work() } else { reloads.append(work) } }
}

extension ExactApp {
    func observeSharedStore(root: URL? = nil, appID: String? = nil, kinds: [String]? = nil, reload: (([String]?) -> Void)? = nil) {
        #if canImport(UIKit)
        let foreground = [UIApplication.willEnterForegroundNotification, UIApplication.didBecomeActiveNotification]
        #else
        let foreground = [NSApplication.didBecomeActiveNotification]
        #endif
        let writtenKinds = kinds ?? Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") as? [String]
        sharedStore.observer = WidgetContainer.observeStore(root: root, appID: appID, kinds: kinds, foreground: foreground, needsRefresh: { [weak self] in
            self?.sessions.contains { $0.booted && $0.state != .destroyed && $0.storeRefresh.pending } ?? false
        }) { [weak self] retryOnly, finished in
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
            self.refreshStore(retryOnly: retryOnly, completion: after)
        }
    }

    /// Every live window finishes its refresh before timelines reload.
    package func refreshStore(retryOnly: Bool = false, completion: @escaping (Bool) -> Void) {
        let active = sessions.filter { $0.booted && $0.state != .destroyed }
        var remaining = active.count
        var success = true
        guard remaining > 0 else { completion(true); return }
        for session in active {
            session.refreshSharedStore(retryOnly: retryOnly) { refreshed in
                success = success && refreshed
                remaining -= 1
                if remaining == 0 { completion(success) }
            }
        }
    }
}

/// Only store/activation replies update these flags; unrelated receipts cannot
/// discharge data_ready. The callback also covers a retry at a later foreground.
final class SessionStoreRefresh {
    var pending = false
    var activated: (() -> Void)?
    func record(_ batch: Batch) {
        pending = batch.storeReadPending || batch.storeActivationPending
        if !batch.storeActivationPending && batch.error == nil {
            let finish = activated; activated = nil; finish?()
        }
    }
}

extension ExactSession {
    func retryStoreActivation(attempts: Int = 3) {
        guard storeRefresh.pending, attempts > 0 else { return }
        DispatchQueue.main.async { [weak self] in
            self?.refreshSharedStore(retryOnly: true) { [weak self] _ in self?.retryStoreActivation(attempts: attempts - 1) }
        }
    }

    func refreshSharedStore(retryOnly: Bool = false, attempts: Int = 3, completion: @escaping (Bool) -> Void) {
        guard state != .destroyed else { completion(true); return }
        let retry = { [weak self] in
            guard let self, state != .destroyed else { completion(true); return }
            guard attempts > 1 else { completion(false); return }
            DispatchQueue.main.async { [weak self] in
                guard let self else { completion(true); return }
                refreshSharedStore(retryOnly: retryOnly, attempts: attempts - 1, completion: completion)
            }
        }
        whenIdle({ [weak self] in
            guard let self, state != .destroyed else { completion(true); return }
            let batch = runtime.storeChanged(retryOnly: retryOnly)
            if batch.busy { retry(); return }
            // seq belongs to optional work measurement. A timer-only commit
            // still carries scheduling state and must reach apply.
            if !batch.unchanged { apply(batch) }
            storeRefresh.record(batch)
            completion(batch.error == nil && !batch.storeActivationPending)
        }, cancelled: retry)
    }
}

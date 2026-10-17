// Awaited scratch-shell polling used by the unchanged shared client; no retained JS handle.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import Foundation

final class T3MobileScratchClock {
    private struct Pending { let work: DispatchWorkItem; let reply: (Bool) -> Void }
    private var pending: [UUID: Pending] = [:]
    private var alive = true

    /// Called on Exact's main-thread native-module lane. Destruction resolves every waiter once.
    func sleep(milliseconds: Double, reply: @escaping (Bool) -> Void) -> Bool {
        guard alive, milliseconds.isFinite, milliseconds >= 0, milliseconds <= 10_000 else { return false }
        let id = UUID()
        let work = DispatchWorkItem { [weak self] in
            guard let self, let pending = self.pending.removeValue(forKey: id) else { return }
            pending.reply(self.alive)
        }
        pending[id] = Pending(work: work, reply: reply)
        DispatchQueue.main.asyncAfter(deadline: .now() + milliseconds / 1_000, execute: work)
        return true
    }
    func destroy() {
        alive = false
        let captured = pending.values; pending.removeAll()
        for item in captured { item.work.cancel(); item.reply(false) }
    }
}

import Foundation

/// Foundation owns each task's wake, tolerance and repeating phase.
final class NativeTaskTimers {
    struct Delivery {
        let cutoff: Double
        fileprivate let entry: Entry
        var row: NativeTimerWake { entry.row }
    }
    fileprivate final class Entry {
        var row: NativeTimerWake
        let timer: Timer
        var pending = false
        init(_ row: NativeTimerWake, _ timer: Timer) { self.row = row; self.timer = timer }
    }
    private var entries: [UInt32: Entry] = [:]
    private var queue: [Delivery] = []
    private var epoch: Int?
    private var notification = false
    private let now: () -> Double
    private let date: () -> Date
    private let ready: () -> Void
    private(set) var rows: [NativeTimerWake] = []

    init(now: @escaping () -> Double, date: @escaping () -> Date = Date.init, ready: @escaping () -> Void) {
        self.now = now; self.date = date; self.ready = ready
    }
    deinit { for entry in entries.values { entry.timer.invalidate() } }
    var nextDue: Double? { rows.map(\.due).min() }
    var hasReady: Bool { !queue.isEmpty }
    func matches(_ rows: [NativeTimerWake]?) -> Bool { rows == nil || rows == self.rows }
    func cancel() {
        for entry in entries.values { entry.timer.invalidate() }
        entries.removeAll(); queue.removeAll(); rows.removeAll(); epoch = nil
    }
    func update(_ rows: [NativeTimerWake]?, epoch: Int, enabled: Bool) {
        guard enabled else { cancel(); return }
        if self.epoch != epoch { cancel(); self.epoch = epoch }
        guard let rows else { resume(); return }
        self.rows = rows
        let wanted = Set(rows.map(\.index))
        for index in entries.keys.filter({ !wanted.contains($0) }) {
            entries.removeValue(forKey: index)?.timer.invalidate()
        }
        for row in rows {
            if let entry = entries[row.index], entry.row.generation == row.generation {
                entry.row = row
                continue
            }
            entries.removeValue(forKey: row.index)?.timer.invalidate()
            // Keep the original deadline and full authored tolerance, including overdue firings
            let delay = (row.due - now()) / 1000
            let fireDate = date().addingTimeInterval(delay)
            let timer = SessionClockTimer.schedule(at: fireDate, every: row.once ? 0 : Double(row.interval) / 1000,
                                                  tolerance: Double(row.tolerance) / 1000) { [weak self] timer in
                guard let self, self.epoch == epoch, let entry = self.entries[row.index], entry.timer === timer, !entry.pending else { return }
                entry.pending = true
                self.queue.append(Delivery(cutoff: self.now(), entry: entry)); self.resume()
            }
            entries[row.index] = Entry(row, timer)
        }
        queue.removeAll { entries[$0.row.index] !== $0.entry }
        resume()
    }
    func resume(after delay: Double = 0) {
        guard hasReady, !notification else { return }
        notification = true
        let ask = { [weak self] in
            guard let self else { return }
            self.notification = false
            if self.hasReady { self.ready() }
        }
        if delay > 0 { DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: ask) } else { DispatchQueue.main.async(execute: ask) }
    }
    func take() -> Delivery? { queue.isEmpty ? nil : queue.removeFirst() }
    func complete(_ delivery: Delivery, retry: Bool) {
        let entry = delivery.entry
        guard entries[entry.row.index] === entry else { resume(); return }
        // A busy or held answer keeps the firing at the front and asks for it again, a frame later: its entry stays
        // pending, so no later firing of the timer is queued, and a retry left for another batch to resume would stop
        // the task in an idle app until something else happened. (A frame later, not at once: a busy answer inside a
        // nested run loop would otherwise ask again on every turn of it.)
        if retry { queue.insert(delivery, at: 0); resume(after: 1.0 / 60); return }
        entry.pending = false
        resume()
    }
    #if DEBUG
    func timerForTests(_ index: UInt32) -> Timer? { entries[index]?.timer }
    #endif
}

extension ExactSession {
    func clockUnchanged(_ batch: Batch) -> Bool {
        batch.timerDueMs == timerDue && taskTimers.matches(batch.nativeTimers) && !taskTimers.hasReady
    }
    func updateTimerState(_ batch: Batch) {
        taskTimers.update(batch.nativeTimers, epoch: generation, enabled: clock == nil && !ExactEnv.agentFreezes && state != .destroyed)
        timerAdmissions.update(batch.timerWaits)
    }
    func deliverNativeTimer() {
        guard state != .destroyed, clock == nil, !ExactEnv.agentFreezes, !applying, !ownerInFlight,
              let timer = taskTimers.take() else { return }
        if ExactSession.asyncFills {
            advanceInFlight = true
            let captured = generation
            runtime.fireNativeTimerAsync(timer.row, now: timer.cutoff) { [weak self] batch in
                self?.publish(.advance, batch, captured)
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.generation == captured else { return }
                    taskTimers.complete(timer, retry: batch.busy || batch.held)
                }
            }
        } else {
            let batch = runtime.fireNativeTimer(timer.row, now: timer.cutoff)
            applyUnlessEmpty(batch)
            taskTimers.complete(timer, retry: batch.busy || batch.held)
        }
    }
}

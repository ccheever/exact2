// Every presentation follows the owner's FIFO, including batches made by
// callbacks during a landing (LLP 1072 T4).
import Foundation
import QuartzCore

extension ExactSession {
    struct PendingBatch {
        let sequence: UInt64
        let generation: Int
        let views: Int?
        let batch: Batch
        var kind: Published? = nil
        var unlessEmpty = false
        var scrolled = false
        var timer = false
        var presented: (() -> Void)?
    }
    enum PendingWork {
        case batch(PendingBatch), idle(IdleWork)
    }

    private func insert(_ work: PendingWork, sequence: UInt64) {
        let index = pendingBatches.firstIndex { $0.sequence > sequence } ?? pendingBatches.endIndex
        pendingBatches.insert((sequence, work), at: index)
    }

    /// On the owner; the posting main turn captured its job's number.
    func publish(_ kind: Published, _ batch: Batch, _ generation: Int, sequence: UInt64) {
        published.lock()
        publishedQueue.append(PendingBatch(sequence: sequence, generation: generation, views: nil, batch: batch, kind: kind))
        published.unlock()
        DispatchQueue.main.async { [weak self] in self?.landFill() }
    }

    var hasPublished: Bool {
        published.lock()
        defer { published.unlock() }
        return !publishedQueue.isEmpty
    }

    private func collectPublished() {
        published.lock()
        let ready = publishedQueue
        publishedQueue.removeAll(keepingCapacity: true)
        published.unlock()
        for next in ready { insert(.batch(next), sequence: next.sequence) }
    }

    package func apply(_ batch: Batch) {
        enqueue(batch)
    }

    /// Empty timer and measurement commits keep their accounting and clock,
    /// without a presentation pass. Decide only when their turn arrives.
    func applyUnlessEmpty(_ batch: Batch, scrolled: Bool = false, timer: Bool = false, presented: (() -> Void)? = nil) {
        enqueue(batch, unlessEmpty: true, scrolled: scrolled, timer: timer, presented: presented)
    }

    private func enqueue(_ batch: Batch, unlessEmpty: Bool = false, scrolled: Bool = false, timer: Bool = false, presented: (() -> Void)? = nil) {
        guard state != .destroyed, !batch.busy, !batch.held else { return }
        // Dispatch consumes these decisions before its next listener or native
        // default action. They must not be replayed into a later event's scope.
        var batch = batch
        var stopped = false, prevented = false
        for op in batch.ops where op.op == .command {
            switch op.payload["name"] as? String {
            case "stopPropagation": stopped = true
            case "preventDefault": prevented = true
            default: break
            }
        }
        if stopped || prevented {
            batch.ops.removeAll { op in
                op.op == .command && ["stopPropagation", "preventDefault"].contains(op.payload["name"] as? String ?? "")
            }
        }
        defer {
            if stopped { presenter.propagationStopped = true }
            if prevented { presenter.defaultPrevented = true }
        }
        let next = PendingBatch(sequence: batch.ownerSequence ?? Owner.shared.enqueued, generation: generation, views: viewsEpoch,
                                batch: batch, unlessEmpty: unlessEmpty, scrolled: scrolled, timer: timer, presented: presented)
        collectPublished()
        // Native layout callbacks read the presenter before returning. A
        // nested batch can apply now unless older work is still waiting.
        if landing, pendingBatches.isEmpty { land(next); return }
        insert(.batch(next), sequence: next.sequence)
        landFill()
    }

    /// One drain selects waiting work. Nested batches with no older work
    /// apply inline; a nested drain cannot overtake work already queued.
    func landFill() {
        guard !landing else { return }
        landing = true
        defer { landing = false; advanceIfDue() }
        while true {
            collectPublished()
            if pendingBatches.isEmpty {
                guard state != .destroyed else { return }
                drainReports()
                if pendingBatches.isEmpty { presenter.collections.flush() }
                collectPublished()
                if pendingBatches.isEmpty {
                    guard !ownerInFlight, !awaitingTree, !presentationWaiters.isEmpty else { return }
                    let work = presentationWaiters.removeFirst()
                    work()
                    continue
                }
            }
            let next = pendingBatches.removeFirst()
            switch next.work {
            case .batch(let batch): land(batch)
            case .idle(let item): runIdle(item)
            }
        }
    }

    private func land(_ next: PendingBatch) {
        let began = CACurrentMediaTime(), views = viewsEpoch
        var submitted = 0.0
        switch next.kind {
        case .fill: fillInFlight = false
        case .tick: tickInFlight = false
        case .advance: advanceInFlight = false
        case .canvas: canvasInFlight = false
        case .notice: noticesInFlight -= 1
        case .reply, .cancelledReply: submitted = retireReply()
        case nil: break
        }
        let valid = state != .destroyed && next.generation == generation
            && (next.views == nil || next.views == viewsEpoch) && !next.batch.held
        // Release older waiters before the slice's callbacks can ask for more.
        // Their barrier includes the synchronous batch that triggered this drain.
        if case .fill = next.kind { releaseFillWaiters(stale: !valid) }
        if case .cancelledReply = next.kind { return }
        if valid {
            if case .advance = next.kind { applyEmptyOrNow(next.batch, timer: true) }
            else if next.unlessEmpty { applyEmptyOrNow(next.batch, scrolled: next.scrolled, timer: next.timer) }
            else if next.kind == nil || !(changesNothing(next.batch) && next.batch.timerDueMs == timerDue) {
                applyNow(next.batch)
            }
        }
        if valid, state != .destroyed, next.generation == generation,
           next.views == nil || next.views == viewsEpoch { next.presented?() }
        if case .reply = next.kind {
            if valid { sampler?.replyPump(main: submitted + (CACurrentMediaTime() - began) * 1000) }
            retryActivation(generation: next.generation)
        }
        if state != .destroyed, canvasOwed { askCanvasDraw() }
        if case .fill(let view) = next.kind, state != .destroyed, viewsEpoch == views {
            presenter.collections.landed(view, at: ProcessInfo.processInfo.systemUptime)
        }
    }

    /// Work that needs the replacement's reports and their presentation too.
    func afterPresentation(_ work: @escaping () -> Void) {
        let captured = generation, views = viewsEpoch
        presentationWaiters.append { [weak self] in
            guard let self, state != .destroyed, generation == captured, viewsEpoch == views else { return }
            work()
        }
        landFill()
    }

    private func applyEmptyOrNow(_ batch: Batch, scrolled: Bool = false, timer: Bool = false) {
        guard !batch.busy, !batch.held else { return }
        // An unpublished reply has changed no presentation state to reconcile.
        guard !fillInFlight, !tickInFlight, !advanceInFlight, !canvasInFlight,
              noticesInFlight == 0, changesNothing(batch) else { applyNow(batch); return }
        sampler?.batch(batch.seq, ms: 0); logAlerts(batch.alerts); logJournal(batch.journal)
        if let q = batch.seq { natives.hatchClock.seq = q.1 }
        if timer { quietTimerAdvances = min(quietTimerAdvances + 1, 2) }
        timerDue = batch.timerDueMs
        scheduleClock(due: batch.timerDueMs)
        if scrolled { presenter.scrolledWithoutPass() }
    }

    /// View reports expire after reset; executor work survives a plan commit.
    package func whenIdle(bound: Bool = false, _ work: @escaping () -> Void, cancelled: (() -> Void)? = nil) {
        let item: IdleWork = (bound ? viewsEpoch : nil, work, cancelled)
        collectPublished()
        if state == .destroyed { cancelled?() }
        else if fillInFlight || idleReleasePending { afterFill.append(item) }
        else if !pendingBatches.isEmpty { insert(.idle(item), sequence: Owner.shared.enqueued); landFill() }
        else { runIdle(item) }
    }

    private func runIdle(_ item: IdleWork) {
        if state == .destroyed || (item.views != nil && item.views != viewsEpoch) { item.cancelled?() }
        else { item.work() }
    }

    private func releaseFillWaiters(stale: Bool) {
        if stale {
            guard !idleReleasePending else { return }
            idleReleasePending = true
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                idleReleasePending = false
                releaseFillWaiters(stale: false)
                landFill()
            }
            return
        }
        guard !idleReleasePending else { return }
        let waiting = afterFill; afterFill.removeAll()
        for item in waiting { insert(.idle(item), sequence: Owner.shared.enqueued) }
    }

    func cancelPendingBatches() {
        presentationWaiters.removeAll()
        var waiting: [IdleWork] = []
        for next in pendingBatches {
            if case .idle(let item) = next.work { waiting.append(item) }
        }
        waiting.append(contentsOf: afterFill); afterFill.removeAll()
        // Keep publications until their flight counters have retired.
        pendingBatches.removeAll {
            if case .batch(let next) = $0.work { return next.kind == nil }
            return true
        }
        waiting.forEach { $0.cancelled?() }
    }

    #if DEBUG
    /// Publish through an owner job and wait for it without landing it.
    func publishForTests(_ kind: Published, _ batch: Batch, generation: Int) {
        let sequence = Owner.shared.enqueued + 1
        Owner.shared.post { [self] in publish(kind, batch, generation, sequence: sequence) }
        Owner.shared.sync {}
    }
    func publishTickForTests(_ batch: Batch) { tickInFlight = true; publishForTests(.tick, batch, generation: generation) }
    func holdFillForTests() { fillInFlight = true }
    func publishFillForTests(_ view: UInt32, _ batch: Batch, generation: Int) {
        fillInFlight = true; publishForTests(.fill(view), batch, generation: generation)
    }
    func oweColorsForTests() { colorsOwed = true }
    #endif
}

// One active job, one replaceable latest request, one completion awaiting UI.
// No CTLine/CTTypesetter crosses this queue. Immutable glyph/caret values do.
import Foundation

/// The existing raster workers also execute region preparation. Per-region
/// dependencies preserve CoreText confinement without a second worker pool.
enum RegionTextExecutor {
    static let queue: OperationQueue = {
        let q = OperationQueue()
        q.name = "exact.text"; q.qualityOfService = .userInitiated
        #if os(iOS)
        q.maxConcurrentOperationCount = TextRasterizer.concurrency
        #else
        q.maxConcurrentOperationCount = 2
        #endif
        return q
    }()
}

final class RegionArtifact: Sendable {
    let id: UInt64
    let sourceID: UInt64
    let metadata: RegionParagraph
    let generation: Int
    private let service: RegionService
    init(id: UInt64, sourceID: UInt64, metadata: RegionParagraph, generation: Int, service: RegionService) {
        self.id = id; self.sourceID = sourceID; self.metadata = metadata; self.generation = generation; self.service = service
    }
    deinit { service.retire(id) }
}
enum RegionJob: Sendable {
    case shape(RegionShapeRequest)
    case raster(RegionRasterRequest)
}
enum RegionAnswer: Sendable {
    case shape(RegionArtifact)
    case raster(RegionRaster)
    case abandoned(id: UInt64, generation: Int)
    case pixelsBusy(RegionRasterRequest)
    case refused(RegionJob, String)
}
private enum RegionShapeCheckpoint: Error { case abandoned }
// One binding per live request ID, not a retained width history. Multiple
// current requests may own the same immutable, queue-confined layout backing.
private struct RegionLayoutBinding {
    let sourceID: UInt64
    let generation: Int
    let layout: RegionWorkerLayout
}
/// Sendable admission endpoint, NOT a Sendable CoreText cache. Only queue bodies
/// access layouts/paint. close() retains self until those owners die on queue;
/// every controller calls it on reset/destroy, including undelivered completion.
final class RegionService: @unchecked Sendable {
    private let scheduling = NSLock()
    private weak var tail: Operation?
    private func enqueue() {
        scheduling.lock(); defer { scheduling.unlock() }
        let operation = BlockOperation { self.turn() }
        if let tail { operation.addDependency(tail) }
        tail = operation
        RegionTextExecutor.queue.addOperation(operation)
    }
    private let lock = NSLock()
    private var active = false
    private var closed = false
    private var latest: RegionJob?
    private var mailbox: RegionAnswer?
    private var waiting = false
    private var epoch: UInt64 = 0
    // Incarnation changes already reset epoch. This is only the current kernel
    // request, not a source/width cache or a second pending job.
    private var desiredShape: (id: UInt64, generation: Int)?
    private var mayAbandonShape = true
    private var resetStorage = false
    private var retired = Set<UInt64>()
    private let beforeShape: @Sendable () -> Void
    private let beforeLayoutConstruction: @Sendable () -> Void
    private let deliver: @MainActor @Sendable (RegionAnswer) -> Void
    // Queue-confined fields, never read even for UI diagnostics.
    private var layouts: [UInt64: RegionLayoutBinding] = [:]
    private var paint: RegionPaintIndex?
    let pixels = RegionPixelAccount()
    let ink = InkAccount()
    let hits = RegionHitAccount()

    init(beforeShape: @escaping @Sendable () -> Void = {},
         beforeLayoutConstruction: @escaping @Sendable () -> Void = {},
         deliver: @escaping @MainActor @Sendable (RegionAnswer) -> Void) {
        self.beforeShape = beforeShape
        self.beforeLayoutConstruction = beforeLayoutConstruction
        self.deliver = deliver
    }
    func submit(_ job: RegionJob) {
        lock.lock()
        guard !closed else { lock.unlock(); return }
        latest = job
        let start = !active
        if start { active = true }
        lock.unlock()
        if start { enqueue() }
    }
    /// Publish accepted receipt intent even while a previous shape owns the
    /// serial slot. No source capture, allocation refund or queue submission.
    func updateShapeRequest(_ id: UInt64, generation: Int) {
        lock.lock(); defer { lock.unlock() }
        guard !closed else { return }
        desiredShape = (id, generation)
    }
    private func abandonShape(_ request: RegionShapeRequest, epoch jobEpoch: UInt64) -> Bool {
        // Intrinsic WIDTH work retains its existing complete-answer contract;
        // a negative height does not make a definite-width shape intrinsic.
        guard request.width.isFinite, request.width >= 0 else { return false }
        lock.lock(); defer { lock.unlock() }
        if closed || epoch != jobEpoch { return true }
        guard mayAbandonShape, let desiredShape,
              desiredShape.id != request.id || desiredShape.generation != request.generation else { return false }
        mayAbandonShape = false
        return true
    }
    func retire(_ id: UInt64) {
        lock.lock()
        guard !closed else { lock.unlock(); return }
        retired.insert(id)
        let start = !active
        if start { active = true }
        lock.unlock()
        if start { enqueue() }
    }
    /// Generation reset never replaces queue/admission/accounts. An old active
    /// shape continues to occupy the slot until its actual allocations unwind.
    func reset() {
        lock.lock()
        guard !closed, epoch < UInt64.max else { lock.unlock(); return }
        epoch += 1; latest = nil; resetStorage = true
        desiredShape = nil; mayAbandonShape = true
        let abandoned = mailbox; mailbox = nil
        let start = !active || waiting
        waiting = false
        if start { active = true }
        lock.unlock()
        withExtendedLifetime(abandoned) {}
        if start { enqueue() }
    }
    func close() {
        lock.lock()
        guard !closed else { lock.unlock(); return }
        closed = true; latest = nil; desiredShape = nil
        let abandoned = mailbox; mailbox = nil
        let start = !active || waiting
        waiting = false
        if start { active = true }
        lock.unlock()
        withExtendedLifetime(abandoned) {} // payload destruction outside admission lock
        if start { enqueue() }
    }
    private func turn() {
        precondition(!Thread.isMainThread)
        lock.lock()
        let stop = closed, release = retired, job = latest, jobEpoch = epoch, clear = resetStorage
        resetStorage = false
        retired.removeAll(keepingCapacity: true); latest = nil
        if job == nil && !stop { active = false }
        lock.unlock()
        if clear { paint = nil; layouts.removeAll() }
        for id in release { layouts.removeValue(forKey: id) }
        if stop { paint = nil; layouts.removeAll(); return }
        guard let job else { return }
        let answer: RegionAnswer = autoreleasepool {
            do {
                switch job {
                case .shape(let request):
                    beforeShape()
                    guard layouts.count < 64 else { return .refused(job, "region live artifact cap") }
                    let layout: RegionWorkerLayout
                    var existing: RegionWorkerLayout?
                    var preparation: RegionPreparedSource?
                    if request.width.isFinite, request.width >= 0 {
                        // Same bounded live-ID lookup; never search paint-only
                        // owners or keep a separate source/width history.
                        for binding in layouts.values where binding.generation == request.generation &&
                            binding.sourceID == request.sourceID && binding.layout.source === request.source {
                            if preparation == nil { preparation = binding.layout.preparation }
                            if binding.layout.metadata.offeredWidth == request.width {
                                existing = binding.layout; break
                            }
                        }
                    }
                    if let existing {
                        // Shape depends on captured source and width, not the
                        // height offer. The fresh artifact still answers only
                        // this exact request ID/full kernel offer.
                        layout = existing
                    } else {
                        let width = request.width == -2 ? RegionWorkerLayout.minimumWidth(request.source)
                            : request.width < 0 ? CGFloat.infinity : request.width
                        beforeLayoutConstruction()
                        do {
                            layout = try RegionWorkerLayout.shape(request.source, width: width, retainHits: request.width >= 0,
                                                                 preparation: preparation, compact: request.compact, beforeMetadata: {
                                if self.abandonShape(request, epoch: jobEpoch) { throw RegionShapeCheckpoint.abandoned }
                            })
                        } catch RegionShapeCheckpoint.abandoned {
                            // The shape's partial allocations unwind inside
                            // this turn's autoreleasepool before UI delivery.
                            return .abandoned(id: request.id, generation: request.generation)
                        }
                    }
                    guard layout.metadata.width.isFinite, layout.metadata.height.isFinite,
                          layout.metadata.width >= 0, layout.metadata.height >= 0,
                          layout.metadata.width <= CGFloat(Float.greatestFiniteMagnitude),
                          layout.metadata.height <= CGFloat(Float.greatestFiniteMagnitude) else { return .refused(job, "invalid worker metrics") }
                    if request.width >= 0 {
                        layouts[request.id] = RegionLayoutBinding(sourceID: request.sourceID,
                            generation: request.generation, layout: layout)
                    }
                    return .shape(RegionArtifact(id: request.id, sourceID: request.sourceID, metadata: layout.metadata,
                                                 generation: request.generation, service: self))
                case .raster(let request):
                    if paint?.publication != request.publication {
                        paint = try RegionPaintIndex(request: request, lookup: { self.layouts[$0]?.layout }, account: ink)
                    }
                    guard let paint else { return .refused(job, "missing accepted worker paint") }
                    return .raster(try paint.render(request, account: pixels, hits: hits))
                }
            } catch RegionRasterRefusal.capacity {
                if case .raster(let request) = job { return .pixelsBusy(request) }
                return .refused(job, "region pixel capacity")
            } catch { return .refused(job, "region worker: \(error)") }
        }
        // Mailbox is detachable without a UI wake. The queued main block owns
        // only the endpoint; cancellation can drop pixels/source immediately.
        lock.lock()
        let stopAfterWork = closed, stale = jobEpoch != epoch
        // At least one valid complete shape (including an exact live-layout
        // alias) must finish between voluntary abandons. A stale epoch cannot
        // rearm the successor incarnation's allowance.
        if !stopAfterWork && !stale, case .shape = answer { mayAbandonShape = true }
        if !stopAfterWork && !stale { mailbox = answer; waiting = true }
        lock.unlock()
        if stopAfterWork { paint = nil; layouts.removeAll(); return }
        if stale { enqueue(); return }
        DispatchQueue.main.async { [self] in
            lock.lock()
            guard epoch == jobEpoch else { lock.unlock(); return }
            let answer = mailbox; mailbox = nil
            let resume = waiting && !closed
            waiting = false
            lock.unlock()
            if let answer, resume { deliver(answer) }
            if resume { enqueue() }
        }
    }
}

// UI-owned lifetime boundary, tested with an actual blocked serial worker.
final class RegionServiceLifetime {
    let service: RegionService
    init(beforeShape: @escaping @Sendable () -> Void = {}, deliver: @escaping @MainActor @Sendable (RegionAnswer) -> Void) {
        service = RegionService(beforeShape: beforeShape, deliver: deliver)
    }
    func reset() { service.reset() }
    deinit { service.close() }
}

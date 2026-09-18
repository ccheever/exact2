// One active job, one replaceable latest request, one completion awaiting UI.
// No CTLine/CTTypesetter crosses this queue. Immutable glyph/caret values do.
import Foundation

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
    case refused(RegionJob, String)
}
/// Sendable admission endpoint, NOT a Sendable CoreText cache. Only queue bodies
/// access layouts/paint. close() retains self until those owners die on queue;
/// every controller calls it on reset/destroy, including undelivered completion.
final class RegionService: @unchecked Sendable {
    private let queue = DispatchQueue(label: "exact.region.text")
    private let lock = NSLock()
    private var active = false
    private var closed = false
    private var latest: RegionJob?
    private var mailbox: RegionAnswer?
    private var waiting = false
    private var epoch: UInt64 = 0
    private var resetStorage = false
    private var retired = Set<UInt64>()
    private let beforeShape: @Sendable () -> Void
    private let deliver: @MainActor @Sendable (RegionAnswer) -> Void
    // Queue-confined fields, never read even for UI diagnostics.
    private var layouts: [UInt64: RegionWorkerLayout] = [:]
    private var paint: RegionPaintIndex?
    let pixels = RegionPixelAccount()
    let ink = InkAccount()

    init(beforeShape: @escaping @Sendable () -> Void = {}, deliver: @escaping @MainActor @Sendable (RegionAnswer) -> Void) { self.beforeShape = beforeShape; self.deliver = deliver }
    func submit(_ job: RegionJob) {
        lock.lock()
        guard !closed else { lock.unlock(); return }
        latest = job
        let start = !active
        if start { active = true }
        lock.unlock()
        if start { queue.async { self.turn() } }
    }
    func retire(_ id: UInt64) {
        lock.lock()
        guard !closed else { lock.unlock(); return }
        retired.insert(id)
        let start = !active
        if start { active = true }
        lock.unlock()
        if start { queue.async { self.turn() } }
    }
    /// Generation reset never replaces queue/admission/accounts. An old active
    /// shape continues to occupy the slot until its actual allocations unwind.
    func reset() {
        lock.lock()
        guard !closed, epoch < UInt64.max else { lock.unlock(); return }
        epoch += 1; latest = nil; resetStorage = true
        let abandoned = mailbox; mailbox = nil
        let start = !active || waiting
        waiting = false
        if start { active = true }
        lock.unlock()
        withExtendedLifetime(abandoned) {}
        if start { queue.async { self.turn() } }
    }
    func close() {
        lock.lock()
        guard !closed else { lock.unlock(); return }
        closed = true; latest = nil
        let abandoned = mailbox; mailbox = nil
        let start = !active || waiting
        waiting = false
        if start { active = true }
        lock.unlock()
        withExtendedLifetime(abandoned) {} // payload destruction outside admission lock
        if start { queue.async { self.turn() } }
    }
    private func turn() {
        dispatchPrecondition(condition: .onQueue(queue))
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
                    let width = request.width == -2 ? RegionWorkerLayout.minimumWidth(request.source)
                        : request.width < 0 ? CGFloat.infinity : request.width
                    let layout = RegionWorkerLayout.shape(request.source, width: width, retainHits: request.width >= 0)
                    guard layout.metadata.width.isFinite, layout.metadata.height.isFinite,
                          layout.metadata.width >= 0, layout.metadata.height >= 0,
                          layout.metadata.width <= CGFloat(Float.greatestFiniteMagnitude),
                          layout.metadata.height <= CGFloat(Float.greatestFiniteMagnitude) else { return .refused(job, "invalid worker metrics") }
                    if request.width >= 0 { layouts[request.id] = layout }
                    return .shape(RegionArtifact(id: request.id, sourceID: request.sourceID, metadata: layout.metadata,
                                                 generation: request.generation, service: self))
                case .raster(let request):
                    if paint?.publication != request.publication {
                        paint = try RegionPaintIndex(request: request, layouts: layouts, account: ink)
                    }
                    guard let paint else { return .refused(job, "missing accepted worker paint") }
                    return .raster(try paint.render(request, account: pixels))
                }
            } catch { return .refused(job, "region worker: \(error)") }
        }
        // Mailbox is detachable without a UI wake. The queued main block owns
        // only the endpoint; cancellation can drop pixels/source immediately.
        lock.lock()
        let stopAfterWork = closed, stale = jobEpoch != epoch
        if !stopAfterWork && !stale { mailbox = answer; waiting = true }
        lock.unlock()
        if stopAfterWork { paint = nil; layouts.removeAll(); return }
        if stale { queue.async { self.turn() }; return }
        DispatchQueue.main.async { [self] in
            lock.lock()
            guard epoch == jobEpoch else { lock.unlock(); return }
            let answer = mailbox; mailbox = nil
            let resume = waiting && !closed
            waiting = false
            lock.unlock()
            if let answer, resume { deliver(answer) }
            if resume { queue.async { self.turn() } }
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

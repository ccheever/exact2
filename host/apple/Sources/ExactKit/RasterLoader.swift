import Foundation
import CExact
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class NativeRasterCharge: RasterBackingCharge, @unchecked Sendable {
    let id: UInt64
    init(_ id: UInt64) { self.id = id }
    deinit { exact_raster_charge_release(id) }
}

/// View ownership pins the core lease; the provider separately owns its charge.
final class NativeRasterLease {
    let id: UInt64
    let image: RasterImage
    init?(_ ready: ExactRasterReady) {
        guard ready.lease != 0, let pointer = UnsafeRawPointer(bitPattern: UInt(ready.payload)) else { return nil }
        id = ready.lease
        image = Unmanaged<RasterImage>.fromOpaque(pointer).takeUnretainedValue()
    }
    deinit { exact_raster_lease_release(id) }
}

private struct RasterSourceKey: Hashable {
    let name: String
    let resolver: ObjectIdentifier
}
/// One decode of a source: the core's entry, less the source.
struct RasterDecodeKey: Hashable {
    let width: Int, height: Int, variant: UInt32
}

private final class RasterSource: RasterSourceOwner, @unchecked Sendable {
    let id: UInt64
    let key: RasterSourceKey
    var name: String { key.name }
    weak var resolver: AssetResolver?
    // Mutable preparation facts are accessed only under the backend lock.
    var users = 0
    var inspecting = false
    var metadata: RasterMetadata?
    var input: RasterInput?
    var cancellation = RasterCancellation()
    var failure: String?
    /// The decodes whose last attempt failed in ImageIO or Core Graphics over
    /// bytes whose size it read (`RasterFailure.decode`): a busy system does
    /// that, so their views ask again before the `error`
    /// (`RasterLoader.declineDelays`). One per decode, as two views can ask
    /// for two sizes and one can land while the other is declined.
    var declined: Set<RasterDecodeKey> = []
    init(id: UInt64, name: String, resolver: AssetResolver) {
        self.id = id; key = RasterSourceKey(name: name, resolver: ObjectIdentifier(resolver)); self.resolver = resolver
    }
}
/// What admission answered: a source, a cap that frees as inspections and
/// views finish (the caller waits), or a refusal no retry changes.
private enum RasterAcquire {
    case source(RasterSource)
    case busy
    case refused
}
private final class WeakRasterSource {
    weak var value: RasterSource?
    init(_ value: RasterSource) { self.value = value }
}

/// The backend holds bounded metadata/descriptors only. A worker obtains its
/// own bytes after admission. Sources never retain a resolver or encoded blob.
private final class RasterBackend: @unchecked Sendable {
    let id: UInt64
    let lock = NSLock()
    weak var loader: RasterLoader?
    var sources: [UInt64: WeakRasterSource] = [:]
    var byName: [RasterSourceKey: UInt64] = [:]
    private var cold: [RasterSource] = []
    static let defaultSourceLimit = 1152
    let sourceLimit: Int
    let metadataLimit: Int
    var serial: UInt64 = 0
    var stopped = false
    var paused = false
    var wakePending = false
    var metadataReads = 0
    var decoded = 0
    var refusals = 0
    #if DEBUG
    /// Decodes still to fail as ImageIO's do on a busy system.
    var testDeclines = 0
    #endif
    private var budgetNotice: [UInt64] = []
    init(id: UInt64, sourceLimit: Int = defaultSourceLimit, metadataLimit: Int = 64) {
        self.id = id; self.sourceLimit = sourceLimit; self.metadataLimit = metadataLimit
    }

    func acquire(_ name: String, resolver: AssetResolver) -> RasterAcquire {
        lock.lock(); defer { lock.unlock() }
        guard !stopped, name.utf8.count <= (name.hasPrefix("data:") ? RasterInput.dataLimit : 4096) else { return .refused }
        pruneLocked()
        let key = RasterSourceKey(name: name, resolver: ObjectIdentifier(resolver))
        if let id = byName[key], let source = sources[id]?.value, source.resolver === resolver,
           !source.cancellation.isCancelled {
            cold.removeAll { $0 === source }
            source.users += 1; return .source(source)
        }
        var live = sources.values.compactMap(\.value)
        // Cold metadata is a cache, never what keeps a shown image out at the
        // source cap: its identity goes first (a painted lease keeps pixels).
        if live.count >= sourceLimit {
            _ = evictColdLocked(live.count - sourceLimit + 1)
            live = sources.values.compactMap(\.value)
        }
        guard live.count < sourceLimit,
              live.filter({ $0.metadata == nil && $0.failure == nil }).count < metadataLimit else {
            refusals += 1; return .busy
        }
        serial += 1
        let source = RasterSource(id: serial, name: name, resolver: resolver)
        source.users = 1; sources[serial] = WeakRasterSource(source); byName[key] = serial
        return .source(source)
    }
    func release(_ id: UInt64) {
        lock.lock()
        let source = sources[id]?.value
        var cancellation: RasterCancellation?
        if let source {
            source.users = max(0, source.users - 1)
            if source.users == 0 && source.inspecting { cancellation = source.cancellation }
            if source.users == 0 && source.metadata != nil && !cold.contains(where: { $0 === source }) { cold.append(source) }
        }
        // Detach strong cold owners and their cache identities under lock;
        // destroy them after unlocking. A painted lease keeps its pixels.
        let evicted = evictColdLocked(max(0, cold.count - RasterLoader.coldSources))
        pruneLocked()
        lock.unlock(); cancellation?.cancel()
        withExtendedLifetime(evicted) {}
        withExtendedLifetime(source) {}
        // A freed source may admit an image waiting at a cap.
        wake()
    }
    private func pruneLocked() {
        sources = sources.filter { $0.value.value != nil }
        byName = byName.filter { sources[$0.value]?.value != nil }
    }
    private func evictColdLocked(_ count: Int) -> [RasterSource] {
        guard count > 0 else { return [] }
        var evicted: [RasterSource] = [], retained: [RasterSource] = []
        for source in cold {
            if evicted.count < count && source.users == 0 {
                sources.removeValue(forKey: source.id)
                if byName[source.key] == source.id { byName.removeValue(forKey: source.key) }
                evicted.append(source)
            } else { retained.append(source) }
        }
        cold = retained
        return evicted
    }
    func invalidate(_ name: String) {
        lock.lock(); byName = byName.filter { $0.key.name != name }; lock.unlock()
    }
    func metadata(_ source: RasterSource) -> (RasterMetadata?, String?) {
        lock.lock(); defer { lock.unlock() }; return (source.metadata, source.failure)
    }
    func declined(_ source: RasterSource, _ key: RasterDecodeKey) -> Bool {
        lock.lock(); defer { lock.unlock() }; return source.declined.contains(key)
    }
    func inspectOne() -> Bool {
        lock.lock()
        let source = stopped || paused ? nil : sources.values.compactMap(\.value).filter { $0.users > 0 && !$0.inspecting && $0.metadata == nil && $0.failure == nil }.min { $0.id < $1.id }
        source?.inspecting = true
        let cancellation = source?.cancellation
        lock.unlock()
        guard let source, let cancellation else { return false }
        var metadata: RasterMetadata?, input: RasterInput?, failure: String?
        do {
            guard let resolver = source.resolver else { throw RasterFailure.decode }
            input = try RasterInput.open(source.name, resolver: resolver, cancellation: cancellation)
            metadata = try input!.metadata()
        } catch { failure = error is RasterFailure || error is RasterHTTPStatus ? String(describing: error) : error.localizedDescription }
        lock.lock()
        if cancellation.isCancelled {
            source.cancellation = RasterCancellation()
        } else {
            source.metadata = metadata; source.input = input; source.failure = failure
        }
        source.inspecting = false
        metadataReads += 1
        if failure != nil { refusals += 1 }
        pruneLocked()
        lock.unlock()
        wake()
        return true
    }
    func decode(_ work: ExactRasterWork) {
        let charge = NativeRasterCharge(work.charge)
        lock.lock()
        let source = sources[work.source]?.value
        let metadata = source?.metadata
        let input = source?.input
        let stopped = self.stopped
        lock.unlock()
        guard !stopped, exact_raster_is_cancelled(work.permit) == 0,
              let source, let metadata, let input, let resolver = source.resolver else {
            exact_raster_fail(work.permit); return
        }
        let key = RasterDecodeKey(width: Int(work.width), height: Int(work.height), variant: work.variant)
        do {
            // Decode scope drops encoded data, source and staging before complete.
            let image: RasterImage = try autoreleasepool {
                let bytes = try withExtendedLifetime(resolver) { try input.bytes() }
                let plan = try RasterDecodePlan(metadata: metadata, maxPixel: Int(max(work.width, work.height)), variant: work.variant)
                guard plan.width == work.width, plan.height == work.height else { throw RasterFailure.reservation }
                guard exact_raster_is_cancelled(work.permit) == 0 else { throw RasterFailure.decode }
                #if DEBUG
                lock.lock(); let decline = testDeclines > 0; testDeclines = max(0, testDeclines - 1); lock.unlock()
                if decline { throw RasterFailure.decode }
                #endif
                return try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: charge, sourceOwner: source, url: input.url)
            }
            let owner = UInt64(UInt(bitPattern: Unmanaged.passRetained(image).toOpaque()))
            _ = exact_raster_complete(work.permit, owner, { value in
                if let pointer = UnsafeRawPointer(bitPattern: UInt(value)) {
                    Unmanaged<RasterImage>.fromOpaque(pointer).release()
                }
            }, UInt64(image.residentBytes))
            lock.lock(); decoded += 1; source.declined.remove(key); lock.unlock()
        } catch {
            // Set before the core's failure is visible to the pass that reads both.
            lock.lock(); refusals += 1
            if error as? RasterFailure == .decode { source.declined.insert(key) } else { source.declined.remove(key) }
            lock.unlock()
            exact_raster_fail(work.permit)
        }
        wake()
    }
    func wake() {
        lock.lock()
        guard !wakePending, !stopped else { lock.unlock(); return }
        wakePending = true; lock.unlock()
        // Only an ID/weak backend crosses to main; no image is captured here.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            lock.lock(); wakePending = false; let loader = self.loader; lock.unlock()
            loader?.reconcile()
        }
    }
    func notifyBudgetChange() {
        let s = exact_raster_stats(id)
        let signature = [s.waiting_budget, s.resident_bytes + s.reserved_bytes]
        lock.lock(); pruneLocked()
        let changed = signature != budgetNotice
        budgetNotice = signature
        lock.unlock()
        if changed && s.waiting_budget > 0 { wake() }
    }
    func reset(stop: Bool) {
        lock.lock(); stopped = stop
        let owners = sources.values.compactMap(\.value)
        let cancellations = owners.map(\.cancellation)
        let evicted = cold; cold.removeAll()
        sources.removeAll(); byName.removeAll(); lock.unlock()
        cancellations.forEach { $0.cancel() }
        withExtendedLifetime(evicted) {}
        withExtendedLifetime(owners) {}
    }
    func setPaused(_ paused: Bool) {
        lock.lock(); self.paused = paused
        let cancellations = paused ? sources.values.compactMap(\.value).filter(\.inspecting).map(\.cancellation) : []
        lock.unlock(); cancellations.forEach { $0.cancel() }
    }
    func trim() {
        lock.lock(); let evicted = evictColdLocked(cold.count); lock.unlock()
        exact_raster_session_control(id, 4)
        withExtendedLifetime(evicted) {}
        wake()
    }
}

private final class WeakRasterBackend {
    weak var value: RasterBackend?
    init(_ value: RasterBackend) { self.value = value }
}

/// Exactly two process workers do both bounded metadata inspection and pixel
/// decode. The gate's wait does not demand UI frames or retain Runtime objects.
/// While any list travels fast (`travelling`), only the first decodes: a
/// second decode in flight doubles ImageIO's buffers at the peak, and at
/// that speed most rows pass before either lands.
final class RasterWorkers: @unchecked Sendable {
    static let shared = RasterWorkers()
    private let lock = NSLock()
    private var backends: [UInt64: WeakRasterBackend] = [:]
    private var cursor: UInt64 = 0
    private var fast = Set<ObjectIdentifier>()
    /// Whether `owner` (a scroll pump) has a list travelling fast.
    func travelling(_ owner: AnyObject, _ value: Bool) {
        lock.lock()
        if value { fast.insert(ObjectIdentifier(owner)) } else { fast.remove(ObjectIdentifier(owner)) }
        lock.unlock()
    }
    var single: Bool { lock.lock(); defer { lock.unlock() }; return !fast.isEmpty }
    private init() {
        for i in 0..<2 { Thread.detachNewThread { [self] in run(i) } }
    }
    fileprivate func add(_ backend: RasterBackend) { lock.lock(); backends[backend.id] = WeakRasterBackend(backend); lock.unlock() }
    private func backend(_ id: UInt64) -> RasterBackend? {
        lock.lock(); defer { lock.unlock() }; return backends[id]?.value
    }
    private func metadataTurn() -> Bool {
        lock.lock()
        backends = backends.filter { $0.value.value != nil }
        let all = backends.keys.sorted()
        let order = all.filter { $0 > cursor } + all.filter { $0 <= cursor }
        let candidates = order.compactMap { backends[$0]?.value }
        if let first = candidates.first { cursor = first.id }
        lock.unlock()
        for item in candidates {
            item.notifyBudgetChange()
            if item.inspectOne() { return true }
        }
        return false
    }
    private func run(_ index: Int) {
        var metadataFirst = false
        while true {
            autoreleasepool {
                // The second worker only reads metadata while one decodes.
                if index > 0 && single {
                    if !metadataTurn() { Thread.sleep(forTimeInterval: 0.005) }
                    return
                }
                // Each decode earns a metadata turn, even under continuous
                // ready-metadata traffic from another session.
                if metadataFirst { metadataFirst = false; if metadataTurn() { return } }
                let work = exact_raster_next_decode(0)
                if work.permit != 0 { metadataFirst = true; deliver(work); return }
                if metadataTurn() { return }
                let waited = exact_raster_next_decode(50)
                if waited.permit != 0 { metadataFirst = true; deliver(waited) }
            }
        }
    }
    private func deliver(_ work: ExactRasterWork) {
        if let backend = backend(work.session) { backend.decode(work) }
        else { exact_raster_fail(work.permit); exact_raster_charge_release(work.charge) }
    }
}

/// Main-thread view interests. Replacement keeps the old lease/natural size
/// until this exact generation accepts a new lease; metadata alone never paints.
final class RasterLoader {
    /// An image waiting at the source or metadata cap. Weak: it keeps no
    /// removed view, resolver generation or session alive while it waits.
    private final class DeferredInterest {
        weak var view: NodeView?
        weak var resolver: AssetResolver?
        let generation: Int
        let source: String
        init(view: NodeView, resolver: AssetResolver, source: String) {
            self.view = view; self.resolver = resolver; generation = view.loadGeneration; self.source = source
        }
    }
    private struct Interest {
        weak var view: NodeView?
        let generation: Int
        let source: RasterSource
        var request: UInt64 = 0
        var variant = RasterVariant.own8
        var requestedPixel = 0
        /// The decode its request asked for.
        var decode: RasterDecodeKey?
        var offeredPixel = 0
        var failure: String?
        var delivered = false
        /// Its `load` or `error` went out: once per source, as `<img>`'s.
        var announced = false
        /// The core's request queue was full: asked again on the next pass.
        var admissionDeferred = false
        /// Declined decodes since it last painted, and the delay it waits out
        /// before asking again: its token, which a reset or a new wait retires.
        var declines = 0
        var waiting: Int?
    }
    /// How long a view waits after each declined decode before asking again;
    /// past the last, the decline is its `error`, as a page's `<img>` errs
    /// for a file and never for a busy machine. ImageIO says only that a
    /// decode failed; a JPEG or PNG it cannot decode fails its header read
    /// first (macOS 27), so a file that fails here anyway errs 1.75 s late,
    /// inside the agent's 3 s bound.
    static let declineDelays: [Double] = [0.25, 0.5, 1]
    /// Eight viewport-sized RGBA bitmaps, bounded to 32–192 MiB. Start at
    /// the floor until the owning view has geometry, never a process screen.
    static let minimumBudget: UInt64 = 32 * 1024 * 1024
    static func viewportBudget(size: CGSize, scale: CGFloat) -> UInt64 {
        let pixels = size.width * size.height * scale * scale
        guard pixels.isFinite, pixels >= 0 else { return minimumBudget }
        return UInt64(min(192 * 1024 * 1024, max(CGFloat(minimumBudget), pixels * 4 * 8)))
    }
    /// Sources no view shows whose metadata stays for the cache's keys.
    static let coldSources = 256
    private(set) var budget: UInt64
    let id: UInt64
    private let backend: RasterBackend
    private var interests: [UInt32: Interest] = [:]
    private var deferred: [UInt32: DeferredInterest] = [:]
    /// The source each view's last refusal before a fetch named: its `error`
    /// goes out once, though every props op asks for that source again.
    private var refused: [UInt32: String] = [:]
    private var deferredCursor: UInt32 = 0
    private var paused = false
    private var destroyed = false
    private var pressure: DispatchSourceMemoryPressure?
    #if DEBUG
    private(set) var testReconciliations = 0
    /// Stands in for the core's request: a request id, or 0 and its refusal.
    var testRequest: ((ExactRasterDemand) -> (request: UInt64, refusal: UInt64?))?
    /// The next `count` decodes fail as ImageIO's do on a busy system.
    func testDecline(next count: Int) { backend.lock.lock(); backend.testDeclines = count; backend.lock.unlock() }
    #endif
    init(budget: UInt64 = RasterLoader.minimumBudget,
         sourceLimit: Int = 1152, metadataLimit: Int = 64) {
        self.budget = max(Self.minimumBudget, budget)
        id = exact_raster_session_create(self.budget)
        backend = RasterBackend(id: id, sourceLimit: sourceLimit, metadataLimit: metadataLimit); backend.loader = self
        RasterWorkers.shared.add(backend)
        let pressure = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .global(qos: .utility))
        pressure.setEventHandler { [weak backend] in backend?.trim() }
        pressure.resume(); self.pressure = pressure
    }
    deinit { shutdown() }
    func fit(size: CGSize, scale: CGFloat) {
        let next = Self.viewportBudget(size: size, scale: scale)
        guard !destroyed, next != budget else { return }
        budget = next
        exact_raster_session_budget(id, next)
    }
    /// Reconsider decode resolution even when the boxes kept their point
    /// sizes, and HDR against the display (LLP 1100 D9).
    func displayChanged() {
        for interest in Array(interests.values) {
            if let view = interest.view { resized(view); rangeChanged(view) }
        }
    }
    /// Re-plan an HDR picture whose decode no longer matches its node's
    /// limit or display (LLP 1100 D8, D9).
    func rangeChanged(_ view: NodeView) {
        guard let interest = interests[view.id], interest.request != 0 || interest.delivered,
              let metadata = backend.metadata(interest.source).0, metadata.hdr else { return }
        if Self.wantsHDR(view, metadata) != (interest.variant == RasterVariant.hdr) { retry(view.id) }
    }
    @discardableResult func load(_ view: NodeView, source: String, resolver: AssetResolver) -> Bool {
        func refuse(_ reason: String) {
            if refused[view.id] != source { refused[view.id] = source; Self.announce([(view, nil, reason)]) }
        }
        if source.hasPrefix("data:"), source.utf8.count > RasterInput.dataLimit {
            view.presenter?.session?.log("image refused: a data: source is over \(RasterInput.dataLimit) bytes (LLP 1011 §2)")
            refuse("a data: source over \(RasterInput.dataLimit) bytes"); return false
        }
        let retained = interests[view.id] != nil || deferred[view.id] != nil
        guard !destroyed, retained || interests.count + deferred.count < 1024 else {
            view.presenter?.session?.log("image deferred: raster subscriber limit")
            refuse("raster subscriber limit"); return false
        }
        switch backend.acquire(source, resolver: resolver) {
        case .source(let record):
            cancel(view.id)
            interests[view.id] = Interest(view: view, generation: view.loadGeneration, source: record)
        case .busy:
            // The caps free as inspections land and views go; each wakes a
            // pass that admits it (`admitDeferred`), with no prop change.
            cancel(view.id)
            deferred[view.id] = DeferredInterest(view: view, resolver: resolver, source: source)
            view.presenter?.session?.log("image queued: raster metadata/source admission")
        case .refused:
            view.presenter?.session?.log("image deferred: invalid raster source or stopped loader")
            refuse("invalid raster source"); return false
        }
        refused.removeValue(forKey: view.id)
        // Initial props run before Presenter registers the new view. Reconcile
        // after the batch, when identity checks can distinguish it from removal.
        if !paused { backend.wake() }
        return true
    }
    func cancel(_ view: UInt32) {
        deferred.removeValue(forKey: view)
        refused.removeValue(forKey: view)
        guard let interest = interests.removeValue(forKey: view) else { return }
        if interest.request != 0 { exact_raster_cancel(id, interest.request) }
        backend.release(interest.source.id)
    }
    func invalidate(_ source: String) { backend.invalidate(source) }
    /// Images on screen still loading (the agent's `clock` and `screenshot`
    /// wait for them).
    var loadingOnScreen: Int {
        interests.values.filter { i in !i.delivered && i.failure == nil && i.view.map(Self.mayShow) == true }.count
            + deferred.values.filter { $0.view.map(Self.mayShow) == true }.count
    }
    /// A loading image's box is on screen. An image sized on one axis
    /// (`width=96`, no height) has an empty box until its natural size
    /// lands, so an empty side counts as one point.
    private static func mayShow(_ view: NodeView) -> Bool {
        guard view.window != nil else { return false }
        let probe = CGRect(origin: view.bounds.origin, size: CGSize(width: max(1, view.bounds.width), height: max(1, view.bounds.height)))
        var root: MediaPlatformView = view
        while let parent = root.superview {
            if root.isHidden { return false }
            root = parent
        }
        var clipped = view.convert(probe, to: root).intersection(root.bounds)
        var ancestor = view.superview
        while let current = ancestor, !clipped.isEmpty {
            #if os(macOS)
            let clips = current is NSClipView || current.clipsToBounds || current.layer?.masksToBounds == true
            #else
            let clips = current.clipsToBounds
            #endif
            if clips { clipped = clipped.intersection(current.convert(current.bounds, to: root)) }
            ancestor = current.superview
        }
        return !clipped.isEmpty
    }
    /// Drop every decoded image no view shows (as memory pressure does).
    func trimCold() { backend.trim() }
    func resized(_ view: NodeView) {
        guard let interest = interests[view.id], interest.offeredPixel > 0 else { return }
        let pixel = requestedPixel(view, backend.metadata(interest.source).0)
        // Avoid re-decoding for every one-pixel live resize or small rounding.
        if pixel > interest.offeredPixel * 5 / 4 || pixel * 5 / 4 < interest.offeredPixel { retry(view.id) }
    }
    /// The decoded image's longest side: enough device pixels for the box
    /// at its object-fit (`cover` and `fill` scale the image until both axes
    /// fill the box, so the longer one overflows it), as a browser decodes
    /// for the size it paints. Before the metadata, the box's longest side.
    private func requestedPixel(_ view: NodeView, _ metadata: RasterMetadata?) -> Int {
        #if os(macOS)
        let scale = view.window?.backingScaleFactor ?? 1
        #else
        let scale = view.traitCollection.displayScale
        #endif
        let box = view.bounds.size
        var longest = max(box.width, box.height)
        if let natural = metadata?.naturalSize, natural.width > 0, natural.height > 0, box.width > 0, box.height > 0 {
            let x = box.width / natural.width, y = box.height / natural.height
            let factor: CGFloat
            switch view.style["object_fit"]?.string ?? "fill" {
            case "contain": factor = min(x, y)
            case "scale-down": factor = min(1, min(x, y))
            case "none": factor = 1
            default: factor = max(x, y)
            }
            longest = max(natural.width, natural.height) * factor
        }
        return max(64, min(4096, Int(ceil(longest * scale))))
    }
    private func retry(_ view: UInt32) {
        guard var item = interests[view] else { return }
        if item.request != 0 { exact_raster_cancel(id, item.request) }
        item.request = 0; item.requestedPixel = 0; item.failure = nil; item.delivered = false
        item.admissionDeferred = false; item.waiting = nil; interests[view] = item
        reconcile()
    }
    /// Ask again for a decode ImageIO declined, unless the wait was retired
    /// (a reset, a new source or view under the same id, another wait).
    private func decodeAgain(_ view: UInt32, token: Int) {
        guard var item = interests[view], item.waiting == token else { return }
        item.waiting = nil; interests[view] = item
        reconcile()
    }
    private var nextWait = 0
    func reconcile() {
        #if DEBUG
        testReconciliations += 1
        #endif
        guard !destroyed, !paused else { return }
        admitDeferred()
        // Every image this turn lands reports its natural size together,
        // after the loop: one layout for them all, not one each.
        var landed: [(view: NodeView, generation: Int, size: CGSize)] = []
        var settled: [(view: NodeView, generation: Int?, failure: String?)] = []
        defer { report(landed); Self.announce(settled) }
        // Each source's first outcome, its `load` or its `error`.
        func settle(_ item: inout Interest, _ view: NodeView) {
            if !item.announced { item.announced = true; settled.append((view, item.generation, item.failure)) }
        }
        for viewID in Array(interests.keys) {
            guard var item = interests[viewID], let view = item.view,
                  view.loadGeneration == item.generation, view.presenter?.views[viewID] === view else { cancel(viewID); continue }
            if item.failure != nil || item.delivered || item.waiting != nil { continue }
            if item.request != 0 {
                let state = exact_raster_status(id, item.request)
                if state == 4, let lease = NativeRasterLease(exact_raster_take_ready(id, item.request)) {
                    item.delivered = true; item.declines = 0
                    if let size = view.acceptRaster(lease, generation: item.generation) { landed.append((view, item.generation, size)); settle(&item, view) }
                    interests[viewID] = item
                } else if state == 115, item.declines < Self.declineDelays.count,
                          let decode = item.decode, backend.declined(item.source, decode) {
                    // Cancelled now, so the core forgets the failed decode once
                    // every view of it has let go, and the next request decodes.
                    let delay = Self.declineDelays[item.declines]
                    exact_raster_cancel(id, item.request); item.request = 0; item.requestedPixel = 0
                    nextWait += 1
                    item.declines += 1; item.waiting = nextWait; interests[viewID] = item
                    view.presenter?.session?.log("image queued: ImageIO declined a decode; again in \(Int(delay * 1000)) ms")
                    let token = nextWait
                    DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
                        self?.decodeAgain(viewID, token: token)
                    }
                    continue
                } else if state >= 100 {
                    item.failure = Self.refusal(UInt64(state - 100)); settle(&item, view)
                    // A failed decode is let go of, its error kept: held, it
                    // is the core's answer to the next view that asks for it.
                    if state == 115 { exact_raster_cancel(id, item.request); item.request = 0 }
                    interests[viewID] = item
                    view.presenter?.session?.log("image deferred: \(item.failure!)")
                }
                if state != 2 || item.requestedPixel <= 1 { continue }
                // Admission can become capacity-blocked after initially free
                // metadata was queued. A worker's changed-budget notice, never
                // a layout callback, reduces this pending demand in place,
                // unless it fits once the decodes holding reservations land:
                // then it waits for them, at the size it asked for.
                if let metadata = backend.metadata(item.source).0,
                   let plan = try? RasterDecodePlan(metadata: metadata, maxPixel: item.requestedPixel, variant: item.variant),
                   plan.peakBytes <= available() { continue }
                exact_raster_cancel(id, item.request); item.request = 0
                item.requestedPixel = max(1, item.requestedPixel / 2)
                interests[viewID] = item
            }
            let (metadata, failure) = backend.metadata(item.source)
            if let failure {
                item.failure = failure; settle(&item, view); interests[viewID] = item
                view.presenter?.session?.log("image deferred: \(failure)"); continue
            }
            guard let metadata else { continue }
            let offered = requestedPixel(view, metadata)
            var pixel = item.requestedPixel > 0 ? min(offered, item.requestedPixel) : offered
            let available = available()
            let hdr = Self.wantsHDR(view, metadata)
            var plan = Self.fit(metadata, pixel: pixel, available: available, hdr: hdr)
            while pixel > 1 && (plan == nil || plan!.peakBytes > available) {
                pixel = max(1, pixel / 2); plan = Self.fit(metadata, pixel: pixel, available: available, hdr: hdr)
            }
            guard let plan else { item.failure = "decode plan too large"; settle(&item, view); interests[viewID] = item; continue }
            var demand = ExactRasterDemand()
            demand.view = UInt64(viewID); demand.view_generation = UInt64(item.generation)
            demand.source = item.source.id; demand.generation = item.source.id
            demand.width = UInt32(plan.width); demand.height = UInt32(plan.height)
            #if os(macOS)
            demand.priority = view.visibleRect.isEmpty ? 1 : 0
            #else
            demand.priority = view.window.map { view.convert(view.bounds, to: $0).intersects($0.bounds) } == true ? 0 : 1
            #endif
            demand.natural_width = UInt32(metadata.naturalSize.width); demand.natural_height = UInt32(metadata.naturalSize.height)
            demand.encoded_bytes = UInt64(metadata.encodedBytes); demand.header_bytes = UInt64(metadata.headerBytes)
            demand.stride = UInt64(plan.stride); demand.scratch_bytes = UInt64(plan.scratchBytes)
            demand.variant = plan.variant
            #if DEBUG
            let submitted = testRequest?(demand)
            item.request = submitted?.request ?? exact_raster_request(id, demand)
            #else
            item.request = exact_raster_request(id, demand)
            #endif
            item.requestedPixel = pixel; item.offeredPixel = offered
            item.variant = plan.variant
            item.decode = RasterDecodeKey(width: plan.width, height: plan.height, variant: plan.variant)
            if item.request == 0 {
                #if DEBUG
                let refusal = submitted?.refusal ?? exact_raster_stats(id).last_refusal
                #else
                let refusal = exact_raster_stats(id).last_refusal
                #endif
                if Self.transientRequestRefusal(refusal) {
                    if !item.admissionDeferred { view.presenter?.session?.log("image queued: raster request admission") }
                    item.admissionDeferred = true
                } else {
                    item.failure = Self.refusal(refusal); settle(&item, view)
                    view.presenter?.session?.log("image deferred: \(item.failure!)")
                }
            } else {
                item.admissionDeferred = false
            }
            interests[viewID] = item
            // A cache hit has no worker completion to wake delivery. Queue one
            // coalesced UI turn, keeping intrinsic publication outside a batch.
            if item.request != 0 && exact_raster_status(id, item.request) >= 4 { backend.wake() }
        }
    }
    /// The plan for a picture at `pixel` (LLP 1100 D7): HDR if wanted and it
    /// fits, else its own variant, else for a deep picture 8 bits before any
    /// resolution is given up.
    private static func fit(_ metadata: RasterMetadata, pixel: Int, available: Int, hdr: Bool) -> RasterDecodePlan? {
        if hdr, let shown = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: RasterVariant.hdr),
           shown.peakBytes <= available { return shown }
        let full = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel)
        if let full, full.peakBytes <= available || !full.deep { return full }
        return (try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: RasterVariant.reduced8)) ?? full
    }
    private static func wantsHDR(_ view: NodeView, _ metadata: RasterMetadata) -> Bool {
        metadata.hdr && DisplayRange.showsHDR(view, limit: view.style["dynamic_range_limit"]?.string)
    }
    /// The metadata and source caps are an admission queue, not a refusal.
    /// Taken in turn from after the last admitted view, so one source that
    /// stays busy does not hold back the rest.
    private func admitDeferred() {
        let keys = deferred.keys.sorted()
        for viewID in keys.filter({ $0 > deferredCursor }) + keys.filter({ $0 <= deferredCursor }) {
            guard let item = deferred[viewID], let view = item.view,
                  view.loadGeneration == item.generation, let resolver = item.resolver else {
                deferred.removeValue(forKey: viewID); continue
            }
            switch backend.acquire(item.source, resolver: resolver) {
            case .source(let source):
                deferred.removeValue(forKey: viewID)
                interests[viewID] = Interest(view: view, generation: item.generation, source: source)
                deferredCursor = viewID
            case .busy:
                continue
            case .refused:
                deferred.removeValue(forKey: viewID)
                view.presenter?.session?.log("image deferred: invalid raster source or stopped loader")
                Self.announce([(view, item.generation, "invalid raster source")])
            }
        }
    }
    /// What a decode can have without lowering its resolution: the budget
    /// less the pixels views hold (pinned, retiring). Cold pixels are
    /// evicted before a decode waits (LLP 1010 §6.3), and a reservation is a
    /// decode in flight, released as it lands; the core queues a request
    /// until they make room.
    private func available() -> Int {
        let used = exact_raster_stats(id)
        let held = used.resident_bytes - min(used.resident_bytes, used.cold_bytes)
        return Int(budget - min(budget, held))
    }
    /// HTML `<img>`'s `load`, or its `error` with the reason (LLP 1011 §4),
    /// to a node that hears it: on the next main turn, after the sizes this
    /// turn reported and the batch that set the source, and only while the
    /// view still shows that load (`generation`; nil for a source refused
    /// before it had one).
    private static func announce(_ events: [(view: NodeView, generation: Int?, failure: String?)]) {
        for (view, generation, failure) in events {
            DispatchQueue.main.async { [weak view] in
                guard let view, generation.map({ $0 == view.loadGeneration }) ?? true,
                      let session = view.presenter?.session, view.presenter?.views[view.id] === view,
                      view.handlers.contains(failure == nil ? "load" : "error") else { return }
                session.apply(failure.map { session.runtime.media(view.id, event: "error", payload: $0, now: session.now()) }
                              ?? session.runtime.load(view.id, now: session.now()))
            }
        }
    }
    private func report(_ landed: [(view: NodeView, generation: Int, size: CGSize)]) {
        guard let presenter = landed.first?.view.presenter else { return }
        let sizes = landed.compactMap { item -> (UInt32, CGSize?)? in
            item.view.loadGeneration == item.generation && presenter.views[item.view.id] === item.view ? (item.view.id, item.size) : nil
        }
        if !sizes.isEmpty { presenter.onIntrinsic?(sizes) }
    }
    func setPaused(_ paused: Bool) {
        guard self.paused != paused, !destroyed else { return }
        self.paused = paused
        backend.setPaused(paused)
        exact_raster_session_control(id, paused ? 1 : 2)
        for key in interests.keys {
            interests[key]?.request = 0; interests[key]?.failure = nil; interests[key]?.delivered = false
            interests[key]?.admissionDeferred = false; interests[key]?.waiting = nil
        }
        if !paused { reconcile() }
    }
    func reset() {
        interests.removeAll(); deferred.removeAll(); refused.removeAll(); deferredCursor = 0
        backend.reset(stop: false); exact_raster_session_control(id, 0)
    }
    func shutdown() {
        guard !destroyed else { return }; destroyed = true
        pressure?.cancel(); pressure = nil
        interests.removeAll(); deferred.removeAll(); backend.reset(stop: true); exact_raster_session_control(id, 3)
    }
    var diagnostics: [String: Any] {
        let s = exact_raster_stats(id)
        backend.lock.lock(); defer { backend.lock.unlock() }
        return ["residentBytes": s.resident_bytes, "reservedBytes": s.reserved_bytes, "peakBytes": s.peak_bytes,
            "pinnedBytes": s.pinned_bytes, "coldBytes": s.cold_bytes, "retiringBytes": s.retiring_bytes,
            "running": s.process_running, "sessionRunning": s.running, "ready": s.ready, "deliveryCells": s.delivery_cells,
            "pending": s.pending_jobs, "subscribers": s.subscribers, "coldEntries": s.cold_entries,
            "sources": backend.sources.count, "metadataReads": backend.metadataReads, "decoded": backend.decoded,
            "cancelled": s.cancelled, "sourceLimit": backend.sourceLimit,
            "refusals": backend.refusals, "viewInterests": interests.count, "lastRefusal": Self.refusal(s.last_refusal),
            "images": interests.map { view, interest -> [String: Any] in
                ["view": view, "source": String(interest.source.id), "offeredPixel": interest.offeredPixel,
                 "admittedPixel": interest.requestedPixel, "status": exact_raster_status(id, interest.request),
                 "failure": interest.failure ?? "", "declines": interest.declines, "pixels": [interest.view?.raster?.image.image.width ?? 0, interest.view?.raster?.image.image.height ?? 0],
                 "color": Self.colorFacts(interest)]
            },
            "deferred": interests.values.filter { $0.admissionDeferred || $0.failure != nil || ($0.request != 0 && exact_raster_status(id, $0.request) == 2) }.count,
            "deferredAdmission": deferred.count,
            "scope": "Exact-owned RGBA storage plus conservative thumbnail/conversion reservation; ImageIO internals excluded"]
    }
    /// What the agent sees of a picture's storage (LLP 1100 D12).
    private static func colorFacts(_ interest: Interest) -> [String: Any] {
        let names: [UInt32: String] = [RasterVariant.srgb8: "srgb8", RasterVariant.own8: "own8",
                                       RasterVariant.deep: "deep", RasterVariant.hdr: "hdr", RasterVariant.reduced8: "reduced8"]
        var facts: [String: Any] = ["variant": names[interest.variant] ?? String(interest.variant),
                                    "fallback": interest.variant == RasterVariant.reduced8 ? "budget" : NSNull()]
        if let image = interest.view?.raster?.image.image {
            facts["space"] = image.colorSpace.map(colorSpaceName) ?? "none"
            facts["bitsPerComponent"] = image.bitsPerComponent
            facts["bytesPerPixel"] = image.bitsPerPixel / 8
            if let raster = interest.view?.raster?.image, raster.isHDR { facts["headroom"] = raster.headroom }
            if let layer = interest.view?.imageLayer {
                var shown = layer.dynamicRangeFacts
                shown["current"] = (layer.contents as AnyObject?) === image
                facts["layer"] = shown
            }
        }
        return facts
    }
    private static func refusal(_ code: UInt64) -> String {
        let names = ["none", "overflow", "invalid dimensions", "encoded limit", "header limit", "source pixels",
            "too large", "budget", "queue full", "subscriber limit", "conflicting metadata", "paused", "shutdown",
            "stale", "actual exceeds reservation", "decode failed"]
        return code < names.count ? names[Int(code)] : "unknown refusal"
    }
    /// A full request queue is temporary: every decode that lands or fails
    /// wakes reconciliation, which asks again. Any other refusal stands.
    static func transientRequestRefusal(_ code: UInt64) -> Bool { code == 8 }
}

import Foundation
import CExact
import ExactCore
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
    /// A remote source's downloaded body, or why not, for a worker to read
    /// the header of (`RasterBackend.inspectOne`).
    var fetched: Result<RasterInput, Error>?
    var cancellation = RasterCancellation()
    var failure: String?
    /// Its download failed in a way asking again may mend: not before this
    /// (its server's `Retry-After`, or the failure's time) (`RasterRetry`).
    var transient: DispatchTime?
    /// A view that fills the screen shows it: its download goes first.
    var urgent = false
    /// The decodes whose last attempt failed in ImageIO or Core Graphics over
    /// bytes whose size it read (`RasterFailure.decode`): a busy system does
    /// that, so their views ask again before the `error`
    /// (`RasterLoader.declineDelays`). One per decode, as two views can ask
    /// for two sizes and one can land while the other is declined.
    var declined: Set<RasterDecodeKey> = []
    /// An SVG's device pixels per CSS pixel by decode, which the core's work
    /// does not carry (LLP 1011.001 D3): the worker draws the scene at the
    /// plan's size over this. Under the backend lock.
    var svgScales: [RasterDecodeKey: Double] = [:]
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
        RasterWorkers.shared.start()
        lock.lock(); defer { lock.unlock() }
        guard !stopped, name.utf8.count <= (name.hasPrefix("data:") ? RasterInput.dataLimit : 4096) else { return .refused }
        pruneLocked()
        let key = RasterSourceKey(name: name, resolver: ObjectIdentifier(resolver))
        if let known = byName[key], let source = sources[known]?.value, source.resolver === resolver,
           !source.cancellation.isCancelled {
            cold.removeAll { $0 === source }
            source.users += 1
            // Released before its header was read: a worker reads it now.
            if source.users == 1 && source.metadata == nil && source.failure == nil { exact_raster_session_control(self.id, 5) }
            return .source(source)
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
        // A worker waiting on the gate wakes to read this source's metadata.
        exact_raster_session_control(id, 5)
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
    /// The scale an SVG's decode draws at, for the worker (`svgScales`).
    func noteScale(_ source: RasterSource, _ key: RasterDecodeKey, _ scale: Double) {
        lock.lock(); source.svgScales[key] = scale; lock.unlock()
    }
    func metadata(_ source: RasterSource) -> (RasterMetadata?, String?) {
        lock.lock(); defer { lock.unlock() }; return (source.metadata, source.failure)
    }
    func declined(_ source: RasterSource, _ key: RasterDecodeKey) -> Bool {
        lock.lock(); defer { lock.unlock() }; return source.declined.contains(key)
    }
    /// Why `source` failed, if it did, and the earliest it may be asked
    /// again if asking again may mend it: read together.
    func retry(_ source: RasterSource) -> (failure: String?, after: DispatchTime?) {
        lock.lock(); defer { lock.unlock() }; return (source.failure, source.failure == nil ? nil : source.transient)
    }
    /// Asks again for `source`, whose download failed in a way asking again
    /// may mend: a worker downloads it anew. Whether it was still failed
    /// (another view's retry may have asked already).
    func reset(_ source: RasterSource) -> Bool {
        lock.lock()
        guard source.failure != nil, source.transient != nil, !source.inspecting else { lock.unlock(); return false }
        // Within the metadata cap, as a first ask is (`acquire`): else it
        // waits, its attempt not spent (`askAgain`).
        let pending = sources.values.compactMap(\.value).filter { $0.metadata == nil && $0.failure == nil }.count
        guard pending < metadataLimit else { lock.unlock(); return false }
        source.failure = nil; source.transient = nil; source.fetched = nil; source.metadata = nil; source.input = nil
        // Asked again as its views are now (`urge`), not as they were.
        source.urgent = false
        lock.unlock()
        exact_raster_session_control(id, 5); wake()
        return true
    }
    /// A view that fills the screen shows `source`: its download goes first,
    /// one under way too.
    func urge(_ source: RasterSource) {
        lock.lock(); let was = source.urgent; source.urgent = true; let running = source.inspecting; lock.unlock()
        if !was, running, let url = RasterInput.remote(source.name) { RasterInput.promote(url) }
    }
    func inspectOne() -> Bool {
        lock.lock()
        let source = stopped || paused ? nil : sources.values.compactMap(\.value).filter { $0.users > 0 && !$0.inspecting && $0.metadata == nil && $0.failure == nil }.min { $0.id < $1.id }
        source?.inspecting = true
        let cancellation = source?.cancellation
        let fetched = source?.fetched
        source?.fetched = nil
        lock.unlock()
        guard let source, let cancellation else { return false }
        // A remote source downloads on URLSession's queue, never on a worker,
        // and stays `inspecting` (so its release cancels it) until its body
        // is here: the workers go on reading headers and decoding meanwhile.
        if fetched == nil, let url = RasterInput.remote(source.name) {
            // Asked as the download starts, so a view that came to fill the screen meanwhile counts.
            let urgent = { [weak self, weak source] () -> Bool in
                guard let self, let source else { return false }
                self.lock.lock(); defer { self.lock.unlock() }; return source.urgent
            }
            RasterInput.fetch(url, urgent: urgent, cancellation: cancellation) { [weak self] result in self?.downloaded(source, cancellation, result) }
            return true
        }
        // A package's picture is asked of its implementation, on the main thread.
        if fetched == nil, let pictures = loader?.pictures, pictures.serving(source.name) != nil {
            DispatchQueue.main.async { [weak self] in
                guard let self, let loader = self.loader else { return }
                loader.askPicture(source.name) { [weak self] result in self?.downloaded(source, cancellation, result) }
            }
            return true
        }
        var metadata: RasterMetadata?, input: RasterInput?, failure: String?, transient: DispatchTime?
        do {
            guard let resolver = source.resolver else { throw RasterFailure.decode }
            input = try fetched?.get() ?? RasterInput.open(source.name, resolver: resolver, cancellation: cancellation)
            metadata = try input!.metadata()
        } catch {
            // (A link the reason quotes loses its query: a CDN's signature.)
            failure = RasterRetry.redacted(error is RasterFailure || error is RasterHTTPStatus || error is RasterEmptyResponse
                ? String(describing: error) : error.localizedDescription, source: source.name)
            // A download's only: a header that fails to read fails again.
            if RasterInput.remote(source.name) != nil, input == nil, RasterRetry.transient(error) {
                transient = (error as? RasterHTTPStatus)?.retryAfter ?? .now()
            }
        }
        lock.lock()
        if cancellation.isCancelled {
            source.cancellation = RasterCancellation()
        } else {
            source.metadata = metadata; source.input = input; source.failure = failure; source.transient = transient
        }
        source.inspecting = false
        metadataReads += 1
        if failure != nil { refusals += 1 }
        pruneLocked()
        lock.unlock()
        wake()
        return true
    }
    /// A remote source's body arrived: a worker reads its header next. One
    /// let go of meanwhile is asked for again from the start, and the room
    /// it held at the metadata cap may admit a waiting image.
    private func downloaded(_ source: RasterSource, _ cancellation: RasterCancellation, _ result: Result<RasterInput, Error>) {
        lock.lock()
        if cancellation.isCancelled { source.cancellation = RasterCancellation() } else { source.fetched = result }
        source.inspecting = false
        lock.unlock()
        exact_raster_session_control(id, 5)
        wake()
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
                // An SVG's plan is the work's own size (the box's, under `fill`), at the scale the request noted.
                var plan = try RasterDecodePlan(metadata: metadata, maxPixel: Int(max(work.width, work.height)), variant: work.variant,
                                                aspect: metadata.svg ? (Int(work.width), Int(work.height)) : nil)
                guard plan.width == work.width, plan.height == work.height else { throw RasterFailure.reservation }
                if metadata.svg { lock.lock(); plan.scale = source.svgScales[key] ?? 1; lock.unlock() }
                if let name = input.catalog, exact_raster_is_cancelled(work.permit) == 0,
                   let image = RasterImage.catalog(name, metadata: metadata, plan: plan, charge: charge, sourceOwner: source) {
                    return image
                }
                if let picture = input.decoded {
                    return try RasterImage.picture(picture, metadata: metadata, plan: plan, charge: charge, sourceOwner: source)
                }
                let bytes = try withExtendedLifetime(resolver) { try input.bytes() }
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
        // Asked again on resume as its views are then (`urge`).
        if paused { sources.values.compactMap(\.value).forEach { $0.urgent = false } }
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
    private var started = false
    /// The workers start with the process's first image source: an app that
    /// shows no picture keeps no decode threads.
    func start() {
        lock.lock(); let first = !started; started = true; lock.unlock()
        guard first else { return }
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
        // Whether this worker decoded or inspected since it last went idle.
        var worked = false
        while true {
            autoreleasepool {
                // The second worker only reads metadata while one decodes.
                if index > 0 && single {
                    if metadataTurn() { worked = true } else { Thread.sleep(forTimeInterval: 0.005) }
                    return
                }
                // Each decode earns a metadata turn, even under continuous
                // ready-metadata traffic from another session.
                if metadataFirst { metadataFirst = false; if metadataTurn() { worked = true; return } }
                let work = exact_raster_next_decode(0, 0)
                if work.permit != 0 { metadataFirst = true; worked = true; deliver(work); return }
                // Read before looking: a source acquired while this worker
                // looks wakes the gate after it, and the wait returns at once.
                let since = exact_raster_wake_sequence()
                if metadataTurn() { worked = true; return }
                // Going idle: the pages ImageIO's frames reached (up to 64 KB
                // in euxy's launch) are dead until the next decode.
                if worked { worked = false; releaseStackBelowCaller() }
                // Work wakes the gate: a decode admitted or freed, a source
                // acquired, a session resumed. The timeout is a backstop, long
                // enough not to be a poll, so an idle app's workers sleep.
                let waited = exact_raster_next_decode(1000, since)
                if waited.permit != 0 { metadataFirst = true; worked = true; deliver(waited) }
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
        /// Off screen, its picture let go of or its decode not asked for
        /// (`park`): asked for again once its view is on screen.
        var parked = false
        /// Its download failed in a way asking again may mend, its `error`
        /// held while it is asked again on screen (`hold`): the retries this
        /// showing, the wait's token, and whether it waits off screen.
        var held = false
        var retries = 0
        var retryWaiting: Int?
        var offscreen = false
        /// A retry's download is under way: still watched, so leaving the
        /// screen meanwhile starts the count over.
        var retrying = false
        /// A first download whose box could fill the screen, off screen:
        /// watched, so a scroll that brings it on screen hurries it (`urge`).
        var watching = false
        /// An SVG under `object-fit: fill`: the box's pixel size its plan
        /// took, a different picture once either axis moves (LLP 1011.001 D1).
        var aspect: [Int] = []
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
    /// Package pictures admitted and revoked (PackagePictures.swift).
    let pictures = PackagePictures()
    private var interests: [UInt32: Interest] = [:]
    private var deferred: [UInt32: DeferredInterest] = [:]
    /// The source each view's last refusal before a fetch named: its `error`
    /// goes out once, though every props op asks for that source again.
    private var refused: [UInt32: String] = [:]
    /// Package pictures refused admission, by view: asked again after each
    /// commit, since an implementation bound or a permission granted since
    /// may let one in (LLP 1133.019 D2).
    private var refusedPictures: [UInt32: (view: () -> NodeView?, source: String, resolver: AssetResolver)] = [:]
    private var deferredCursor: UInt32 = 0
    /// The views parked off screen (`park`), for the scroll check.
    private var parked = Set<UInt32>()
    #if os(macOS)
    private var scrollObserver: NSObjectProtocol?
    #else
    private final class ScrollWatch {
        weak var scroller: UIScrollView?
        let observation: NSKeyValueObservation
        init(_ scroller: UIScrollView, _ observation: NSKeyValueObservation) { self.scroller = scroller; self.observation = observation }
    }
    private var scrollWatches: [ObjectIdentifier: ScrollWatch] = [:]
    #endif
    private var paused = false
    private var destroyed = false
    private var pressure: DispatchSourceMemoryPressure?
    #if DEBUG
    private(set) var testReconciliations = 0
    /// Each `load` (nil) or `error` (its reason) a pass announced, by view.
    private(set) var testAnnounced: [(view: UInt32, failure: String?)] = []
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
        refusedPictures[view.id] = nil
        if let session = view.presenter?.session, let refusal = pictures.admit(source, session: session) {
            view.presenter?.session?.log("image refused: \(RasterRetry.withoutQueries(source)): \(refusal) (LLP 1133.019 D2)")
            refusedPictures[view.id] = ({ [weak view] in view }, source, resolver)
            refuse(refusal); return false
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
            timings.asked(view.id); timings.admitted(view.id)
        case .busy:
            // The caps free as inspections land and views go; each wakes a
            // pass that admits it (`admitDeferred`), with no prop change.
            cancel(view.id)
            deferred[view.id] = DeferredInterest(view: view, resolver: resolver, source: source)
            timings.asked(view.id)
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
        timings.retired(view)
        deferred.removeValue(forKey: view)
        refusedPictures.removeValue(forKey: view)
        refused.removeValue(forKey: view)
        unpark(view)
        guard let interest = interests.removeValue(forKey: view) else { return }
        if interest.request != 0 { exact_raster_cancel(id, interest.request) }
        backend.release(interest.source.id)
    }
    func invalidate(_ source: String) { backend.invalidate(source) }
    /// Each refused package picture whose admission now passes loads. Main
    /// thread, after a commit applied.
    func readmitPictures() {
        for (id, refusal) in refusedPictures {
            guard let view = refusal.view(), let session = view.presenter?.session else { refusedPictures[id] = nil; continue }
            guard pictures.admit(refusal.source, session: session) == nil else { continue }
            refused[id] = nil
            load(view, source: refusal.source, resolver: refusal.resolver)
        }
    }
    /// Ask the implementation for package picture `name` at the size of a
    /// box that shows it. Main thread.
    fileprivate func askPicture(_ name: String, done: @escaping (Result<RasterInput, Error>) -> Void) {
        let view = interests.values.first { $0.source.name == name && $0.view != nil }?.view
        guard let view, let session = view.presenter?.session else { return done(.failure(RasterFailure.unresolved)) }
        let scale = Self.displayScale(view)
        // Before layout, the platform's own thumbnail size.
        let box = view.bounds.size == .zero ? CGSize(width: 512, height: 512) : view.bounds.size
        pictures.ask(name, session: session, width: max(1, Int((box.width * scale).rounded(.up))),
                     height: max(1, Int((box.height * scale).rounded(.up))),
                     fit: view.style["object_fit"]?.string ?? "fill", done: done)
    }
    /// The implementation took package picture `source` back (LLP 1133.019
    /// D2): every view showing it is emptied and fires `error`, its loads
    /// and cache entries go, and a reply in flight is dropped.
    func revokePicture(_ source: String) {
        pictures.revoke(source)
        backend.invalidate(source)
        var gone: [(view: NodeView, generation: Int?, failure: String?)] = []
        for (id, interest) in interests where interest.source.name == source {
            if let view = interest.view { view.withdrawPicture(); gone.append((view, nil, "revoked")) }
            cancel(id)
            refused[id] = source
        }
        Self.announce(gone)
    }
    /// The implementation says package picture `source` changed: each view
    /// showing it loads it again.
    func pictureChanged(_ source: String) {
        backend.invalidate(source)
        for (id, interest) in interests where interest.source.name == source {
            guard let view = interest.view, let resolver = interest.source.resolver else { continue }
            cancel(id)
            load(view, source: source, resolver: resolver)
        }
    }
    /// When each image arrived, and which arrived late (LLP 1145 D2).
    let timings = RasterTimings()
    /// A presented frame (the frame sampler's): each loading image whose box
    /// shows starts its blank time.
    func sampleBlank() {
        guard HatchDiagnostics.available else { return }
        let loading = interests.filter { !$0.value.delivered && ($0.value.failure == nil || $0.value.held) }
        guard !loading.isEmpty || !deferred.isEmpty else { return }
        // A decode that finished waits here for the main thread to deliver it:
        // its `decoded` is the first frame that saw it ready.
        for (key, i) in loading where i.request != 0 && exact_raster_status(id, i.request) == 4 { timings.decoded(key) }
        timings.showing(loading.compactMap { key, i in i.view.map(Self.mayShow) == true ? key : nil }
            + deferred.compactMap { key, d in d.view.map(Self.mayShow) == true ? key : nil })
    }
    /// Images on screen still loading (the agent's `clock` and `screenshot`
    /// wait for them).
    var loadingOnScreen: Int {
        // (One held for a retry is still loading.)
        interests.values.filter { i in !i.delivered && (i.failure == nil || i.held) && i.view.map(Self.mayShow) == true }.count
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
        guard let interest = interests[view.id] else { return }
        // Laid out to fill the screen while it loads (a viewer's picture): its download goes first.
        if !interest.delivered, Self.fillsScreen(view) { backend.urge(interest.source) }
        // A held picture laid out on or off screen: a pass sees it (`hold`).
        if interest.held || interest.retrying { backend.wake(); return }
        // A first download laid out large enough to fill the screen: a pass
        // watches it off screen, or hurries it on (`watching`).
        if !interest.delivered, !interest.watching, interest.offeredPixel == 0, Self.fillsScreen(view, anywhere: true) { backend.wake() }
        // Laid out on screen, a parked view asks for its picture.
        if interest.parked { if Self.mayShow(view) { backend.wake() }; return }
        guard interest.offeredPixel > 0 else { return }
        let metadata = backend.metadata(interest.source).0
        let pixel = requestedPixel(view, metadata)
        // Avoid re-decoding for every one-pixel live resize or small rounding.
        if pixel > interest.offeredPixel * 5 / 4 || pixel * 5 / 4 < interest.offeredPixel { retry(view.id); return }
        // An SVG filling its box is drawn at the box's ratio: either axis past the band is another picture.
        if let aspect = metadata.flatMap({ Self.svgAspect(view, $0) }), interest.aspect.count == 2 {
            let moved = zip([aspect.0, aspect.1], interest.aspect).contains { now, was in now > was * 5 / 4 || now * 5 / 4 < was }
            if moved { retry(view.id) }
        }
    }
    /// The view's device pixels per point.
    private static func displayScale(_ view: NodeView) -> CGFloat {
        #if os(macOS)
        return view.window?.backingScaleFactor ?? 1
        #else
        return view.traitCollection.displayScale
        #endif
    }
    /// For an SVG under `object-fit: fill` (CSS's initial value), the box's
    /// pixel size, which is the concrete object size the picture is drawn at
    /// (LLP 1011.001 D1); nil for a bitmap or another fit.
    private static func svgAspect(_ view: NodeView, _ metadata: RasterMetadata) -> (Int, Int)? {
        #if os(iOS) || os(tvOS)
        let box = view.buttonRasterView == nil ? view.bounds.size : metadata.naturalSize
        #else
        let box = view.bounds.size
        #endif
        guard metadata.svg, (view.style["object_fit"]?.string ?? "fill") == "fill", box.width > 0, box.height > 0 else { return nil }
        let scale = displayScale(view)
        return (max(1, Int((box.width * scale).rounded())), max(1, Int((box.height * scale).rounded())))
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
        #if os(iOS) || os(tvOS)
        // A native face displays the image at its natural dimensions, even
        // before metadata has enlarged the button's image-free layout box.
        if view.buttonRasterView != nil, let natural = metadata?.naturalSize {
            return max(64, min(4096, Int(ceil(max(natural.width, natural.height) * scale))))
        }
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
        item.admissionDeferred = false; item.waiting = nil
        if item.held || item.retrying || item.watching { unwatch(view, &item) }
        interests[view] = item
        reconcile()
    }
    /// Ask again for a decode ImageIO declined, unless the wait was retired
    /// (a reset, a new source or view under the same id, another wait).
    private func decodeAgain(_ view: UInt32, token: Int) {
        guard var item = interests[view], item.waiting == token else { return }
        item.waiting = nil; interests[view] = item
        reconcile()
    }
    /// A picture whose download failed in a way asking again may mend, its
    /// `error` held (LLP 1011 §4, the same rules as the Linux host's): on
    /// screen it is asked again after the next of `RasterRetry.steps`, never
    /// before its server's `Retry-After`; off screen it waits, its count
    /// from nothing, until it shows. Whether it is still held; false once
    /// its retries are spent, or its server's wait is past what a picture
    /// waits: its `error` goes out.
    private func hold(_ viewID: UInt32, _ item: inout Interest, _ view: NodeView) -> Bool {
        let (failure, after) = backend.retry(item.source)
        guard let failure else {
            // Another view of the URL asked again: this one joins that
            // download, with no retry of its own counted.
            item.held = false; item.failure = nil; item.retryWaiting = nil; item.offscreen = false; item.retrying = true
            interests[viewID] = item; backend.wake()
            return true
        }
        item.failure = failure
        guard let after else { return false }
        // Watched, so leaving the screen or coming back is seen (`scrolled`).
        if !parked.contains(viewID) { watch(viewID, view) }
        defer { interests[viewID] = item }
        guard Self.mayShow(view) else {
            item.retryWaiting = nil; item.retries = 0; item.offscreen = true
            return true
        }
        item.offscreen = false
        if item.retryWaiting != nil { return true }
        let now = DispatchTime.now()
        let wait = after > now ? Double(after.uptimeNanoseconds - now.uptimeNanoseconds) / 1e9 : 0
        guard item.retries < RasterRetry.steps.count else { return false }
        // Further off than a picture waits: no more this showing.
        guard wait <= RasterRetry.most else { return false }
        let delay = max(RasterRetry.steps[item.retries], wait)
        nextWait += 1; item.retryWaiting = nextWait
        let token = nextWait
        view.presenter?.session?.log("image queued: \(failure); asked again in \(Int(delay * 1000)) ms")
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in self?.askAgain(viewID, token: token) }
        return true
    }
    /// A held picture's wait is out: asked again if it is still on screen
    /// (the next pass sees it left otherwise), unless the wait was retired.
    private func askAgain(_ viewID: UInt32, token: Int) {
        guard var item = interests[viewID], item.held, item.retryWaiting == token, let view = item.view else { return }
        item.retryWaiting = nil
        if Self.mayShow(view), backend.reset(item.source) {
            item.retries += 1; item.held = false; item.failure = nil; item.retrying = true
        }
        interests[viewID] = item
        reconcile()
    }
    /// Its retry tracking or its watch for the screen ends: the watch goes,
    /// unless it is parked for real (`park`).
    private func unwatch(_ viewID: UInt32, _ item: inout Interest) {
        item.held = false; item.retryWaiting = nil; item.retrying = false; item.watching = false
        if !item.parked { unpark(viewID) }
    }
    private func watch(_ viewID: UInt32, _ view: NodeView) {
        #if os(macOS)
        if parked.isEmpty { watchScrolls() }
        #else
        watchScrolls(above: view)
        #endif
        parked.insert(viewID)
    }
    /// A box on screen that fills it (`RasterRetry.fills`); `anywhere`, one
    /// that would if it were on screen.
    private static func fillsScreen(_ view: NodeView, anywhere: Bool = false) -> Bool {
        #if os(macOS)
        guard let root = view.window?.contentView else { return false }
        #else
        guard let root = view.window else { return false }
        #endif
        return (anywhere || mayShow(view)) && RasterRetry.fills(view.convert(view.bounds, to: root), window: root.bounds)
    }
    private var nextWait = 0
    func reconcile() {
        #if DEBUG
        testReconciliations += 1
        #endif
        guard !destroyed, !paused else { return }
        // Decoded pictures put on their layers: a late frame's `image` (LLP 1144 D2).
        let began = HostPhases.shared.begin(.image)
        defer { HostPhases.shared.end(began) }
        admitDeferred()
        // Every image this turn lands reports its natural size together,
        // after the loop: one layout for them all, not one each.
        var landed: [(view: NodeView, generation: Int, size: CGSize)] = []
        var settled: [(view: NodeView, generation: Int?, failure: String?)] = []
        // Whether this pass gave the core's queue room: a picture taken, a
        // request let go of.
        var progress = false
        defer {
            report(landed); Self.announce(settled)
            #if DEBUG
            testAnnounced = Array((testAnnounced + settled.map { ($0.view.id, $0.failure) }).suffix(64))
            #endif
            // The interests a full queue refused ask again on the next pass.
            // A decode landing wakes one, but the last may already have
            // landed, so a pass that made room wakes one itself.
            if progress && interests.values.contains(where: \.admissionDeferred) { backend.wake() }
        }
        // Each source's first outcome, its `load` or its `error`.
        func settle(_ item: inout Interest, _ view: NodeView) {
            // Its `load` or `error`: retry tracking ends.
            if item.retrying || item.held || item.watching { unwatch(view.id, &item) }
            if !item.announced { item.announced = true; settled.append((view, item.generation, item.failure)) }
        }
        for viewID in Array(interests.keys) {
            guard var item = interests[viewID], let view = item.view,
                  view.loadGeneration == item.generation, view.presenter?.views[viewID] === view else { cancel(viewID); continue }
            if item.parked {
                guard Self.mayShow(view) else { continue }
                item.parked = false; interests[viewID] = item; unpark(viewID)
            }
            if item.held {
                if hold(viewID, &item, view) { continue }
                item.held = false; unpark(viewID); settle(&item, view); interests[viewID] = item
                view.presenter?.session?.log("image deferred: \(item.failure ?? "")"); continue
            }
            // A retry under way whose picture left the screen: the count starts over.
            if item.retrying, !Self.mayShow(view) { item.retries = 0; interests[viewID] = item }
            if item.failure != nil || item.delivered || item.waiting != nil { continue }
            if Self.fillsScreen(view) { backend.urge(item.source) }
            // A first download that could fill the screen, off screen: a scroll may bring it on.
            else if !item.watching, item.request == 0, !Self.mayShow(view), Self.fillsScreen(view, anywhere: true),
                    RasterInput.remote(item.source.name) != nil, backend.metadata(item.source).0 == nil,
                    backend.metadata(item.source).1 == nil {
                item.watching = true; watch(viewID, view); interests[viewID] = item
            }
            if item.request != 0 {
                let state = exact_raster_status(id, item.request)
                if state == 4, let lease = NativeRasterLease(exact_raster_take_ready(id, item.request)) {
                    item.delivered = true; item.declines = 0; progress = true
                    if item.retrying || item.watching { unwatch(viewID, &item) }
                    timings.decoded(viewID) // if no frame saw it ready first (`sampleBlank`)
                    if let size = view.acceptRaster(lease, generation: item.generation) { landed.append((view, item.generation, size)); settle(&item, view) }
                    timings.delivered(viewID)
                    interests[viewID] = item
                } else if state == 115, item.declines < Self.declineDelays.count,
                          let decode = item.decode, backend.declined(item.source, decode) {
                    // Cancelled now, so the core forgets the failed decode once
                    // every view of it has let go, and the next request decodes.
                    let delay = Self.declineDelays[item.declines]
                    exact_raster_cancel(id, item.request); item.request = 0; item.requestedPixel = 0; progress = true
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
                    if state == 115 { exact_raster_cancel(id, item.request); item.request = 0; progress = true }
                    interests[viewID] = item
                    view.presenter?.session?.log("image deferred: \(item.failure!)")
                }
                if state != 2 { continue }
                // Waiting on the budget. The decodes in flight hold
                // reservations `available` does not count: once they land
                // it may fit, at the size it asked for.
                let plan = backend.metadata(item.source).0.flatMap {
                    try? RasterDecodePlan(metadata: $0, maxPixel: item.requestedPixel, variant: item.variant, aspect: Self.svgAspect(view, $0))
                }
                if let plan, plan.peakBytes <= available() { continue }
                // The pictures views hold leave it no room. Off screen it
                // holds no place in the queue and takes no lower resolution:
                // it asks again on screen. On screen, the pictures held off
                // screen make room, and the core retries it as they go.
                if !Self.mayShow(view) { park(viewID, &item, view); progress = true; continue }
                if let plan, release(plan.peakBytes - available()) > 0 { progress = true; continue }
                if item.requestedPixel <= 1 { continue }
                // Nothing off screen to let go of: a lower resolution, in
                // place. A worker's changed-budget notice, never a layout
                // callback, brought it here.
                exact_raster_cancel(id, item.request); item.request = 0; progress = true
                item.requestedPixel = max(1, item.requestedPixel / 2)
                interests[viewID] = item
            }
            let (metadata, failure) = backend.metadata(item.source)
            if let failure {
                item.failure = failure
                // Asking again may mend it: its `error` waits (`hold`).
                if !item.announced, backend.retry(item.source).after != nil {
                    item.held = true; item.retrying = false
                    if hold(viewID, &item, view) { continue }
                    item.held = false
                }
                if item.retrying { item.retrying = false }
                unpark(viewID)
                settle(&item, view); interests[viewID] = item
                view.presenter?.session?.log("image deferred: \(failure)"); continue
            }
            guard let metadata else { continue }
            if item.watching { unwatch(viewID, &item); interests[viewID] = item }
            let offered = requestedPixel(view, metadata)
            var pixel = item.requestedPixel > 0 ? min(offered, item.requestedPixel) : offered
            var available = available()
            let hdr = Self.wantsHDR(view, metadata)
            let aspect = Self.svgAspect(view, metadata)
            var plan = Self.fit(metadata, pixel: pixel, available: available, hdr: hdr, aspect: aspect)
            if let need = plan?.peakBytes, need > available {
                // The pictures views hold leave it no room at its size: off
                // screen it waits for its turn on screen; on screen the
                // pictures held off screen make room, and it is planned again.
                guard Self.mayShow(view) else { park(viewID, &item, view); continue }
                if release(need - available) > 0 {
                    available = self.available(); plan = Self.fit(metadata, pixel: pixel, available: available, hdr: hdr, aspect: aspect)
                }
            }
            while pixel > 1 && (plan == nil || plan!.peakBytes > available) {
                pixel = max(1, pixel / 2); plan = Self.fit(metadata, pixel: pixel, available: available, hdr: hdr, aspect: aspect)
            }
            guard var plan else { item.failure = "decode plan too large"; settle(&item, view); interests[viewID] = item; continue }
            if metadata.svg { plan.scale = Double(Self.displayScale(view)) }
            item.aspect = aspect.map { [$0.0, $0.1] } ?? []
            var demand = ExactRasterDemand()
            demand.view = UInt64(viewID); demand.view_generation = UInt64(item.generation)
            demand.source = item.source.id; demand.generation = item.source.id
            demand.width = UInt32(plan.width); demand.height = UInt32(plan.height)
            demand.priority = Self.mayShow(view) ? 0 : 1
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
            if item.request != 0 { timings.requested(viewID) }
            item.requestedPixel = pixel; item.offeredPixel = offered
            item.variant = plan.variant
            item.decode = RasterDecodeKey(width: plan.width, height: plan.height, variant: plan.variant)
            if metadata.svg, let key = item.decode { backend.noteScale(item.source, key, plan.scale) }
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
    /// The view lets go of its picture and its request while off screen,
    /// keeping its interest. The pixels it held go cold, for the core to
    /// evict when a decode needs their room, and `reconcile` asks for them
    /// again once the view is on screen (`scrolled`, `resized`). Returns
    /// the bytes the picture held.
    @discardableResult private func park(_ viewID: UInt32, _ item: inout Interest, _ view: NodeView) -> Int {
        let bytes = view.raster?.image.residentBytes ?? 0
        if item.request != 0 { exact_raster_cancel(id, item.request) }
        item.request = 0; item.requestedPixel = 0; item.delivered = false
        item.admissionDeferred = false; item.waiting = nil; item.parked = true
        interests[viewID] = item
        if view.raster != nil {
            // The sublayer's contents and a player's frames hold the pixels too.
            view.raster = nil; view.applyImageLayer(); AnimatedRasters.shared.attach(view)
            #if os(macOS)
            view.needsDisplay = true
            #else
            view.setNeedsDisplay()
            #endif
        }
        #if os(macOS)
        if parked.isEmpty { watchScrolls() }
        #else
        watchScrolls(above: view)
        #endif
        parked.insert(viewID)
        return bytes
    }
    private func unpark(_ viewID: UInt32) {
        guard parked.remove(viewID) != nil, parked.isEmpty else { return }
        stopWatchingScrolls()
    }
    /// Pictures held by views off screen, let go of until `bytes` are
    /// covered, the view farthest from the window first: a view on screen
    /// waits on the budget they fill. Returns the bytes let go of.
    private func release(_ bytes: Int) -> Int {
        guard bytes > 0 else { return 0 }
        var held: [(distance: CGFloat, id: UInt32)] = []
        for (viewID, item) in interests where item.delivered {
            // A flying picture is a flight's stand-in until it lands.
            guard let view = item.view, view.raster != nil, view.flightLook == nil, !Self.mayShow(view) else { continue }
            held.append((Self.distance(view), viewID))
        }
        held.sort { $0.distance > $1.distance }
        var freed = 0
        for (_, viewID) in held where freed < bytes {
            guard var item = interests[viewID], let view = item.view else { continue }
            freed += park(viewID, &item, view)
        }
        return freed
    }
    /// How far a view's box is from the window, in points: 0 inside it.
    private static func distance(_ view: NodeView) -> CGFloat {
        #if os(macOS)
        guard let root = view.window?.contentView else { return .greatestFiniteMagnitude }
        #else
        guard let root = view.window else { return .greatestFiniteMagnitude }
        #endif
        let box = view.convert(view.bounds, to: root), port = root.bounds
        return max(0, box.minX - port.maxX, port.minX - box.maxX) + max(0, box.minY - port.maxY, port.minY - box.maxY)
    }
    /// A scroll container moved: a parked view may be on screen now, and a
    /// pass asks for its picture. Only the parked views are looked at.
    private func scrolled() {
        guard !parked.isEmpty else { return }
        // A held picture may have left the screen or come back (`hold`).
        if parked.contains(where: { interests[$0].map { $0.held || $0.retrying || $0.watching } == true }) { backend.wake(); return }
        for viewID in parked where interests[viewID]?.view.map(Self.mayShow) == true { backend.wake(); return }
    }
    #if os(macOS)
    /// Every clip view that posts its bounds (the viewport's, a node's
    /// scroller's), while a view is parked; none otherwise.
    private func watchScrolls() {
        scrollObserver = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification, object: nil, queue: .main) { [weak self] _ in self?.scrolled() }
    }
    private func stopWatchingScrolls() {
        if let scrollObserver { NotificationCenter.default.removeObserver(scrollObserver) }
        scrollObserver = nil
    }
    #else
    /// Each scroll view above a parked view (the viewport, a node's, a
    /// grouped list's), while one is parked; none otherwise.
    private func watchScrolls(above view: NodeView) {
        scrollWatches = scrollWatches.filter { $0.value.scroller != nil }
        var ancestor = view.superview
        while let current = ancestor {
            if let scroller = current as? UIScrollView, scrollWatches[ObjectIdentifier(scroller)] == nil {
                scrollWatches[ObjectIdentifier(scroller)] = ScrollWatch(scroller, scroller.observe(\.contentOffset) { [weak self] _, _ in self?.scrolled() })
            }
            ancestor = current.superview
        }
    }
    private func stopWatchingScrolls() { scrollWatches.removeAll() }
    #endif
    /// The plan for a picture at `pixel` (LLP 1100 D7): HDR if wanted and it
    /// fits, else its own variant, else for a deep picture 8 bits before any
    /// resolution is given up.
    private static func fit(_ metadata: RasterMetadata, pixel: Int, available: Int, hdr: Bool, aspect: (Int, Int)? = nil) -> RasterDecodePlan? {
        if hdr, let shown = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: RasterVariant.hdr, aspect: aspect),
           shown.peakBytes <= available { return shown }
        let full = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, aspect: aspect)
        if let full, full.peakBytes <= available || !full.deep { return full }
        return (try? RasterDecodePlan(metadata: metadata, maxPixel: pixel, variant: RasterVariant.reduced8, aspect: aspect)) ?? full
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
                timings.admitted(viewID)
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
            guard item.view.loadGeneration == item.generation, presenter.views[item.view.id] === item.view else { return nil }
            #if os(iOS) || os(tvOS)
            return (item.view.buttonRasterView ?? item.view.id, item.size)
            #else
            return (item.view.id, item.size)
            #endif
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
            // Paused, a picture is off screen: its count starts over.
            if var item = interests[key] {
                if item.held || item.retrying || item.watching { unwatch(key, &item) }
                item.retries = 0; interests[key] = item
            }
        }
        if !paused { reconcile() }
    }
    func reset() {
        interests.removeAll(); deferred.removeAll(); refused.removeAll(); refusedPictures.removeAll(); deferredCursor = 0
        parked.removeAll(); stopWatchingScrolls(); timings.reset()
        #if DEBUG
        testAnnounced.removeAll()
        #endif
        backend.reset(stop: false); exact_raster_session_control(id, 0)
    }
    func shutdown() {
        guard !destroyed else { return }; destroyed = true
        pressure?.cancel(); pressure = nil
        interests.removeAll(); deferred.removeAll(); parked.removeAll(); stopWatchingScrolls(); timings.reset()
        backend.reset(stop: true); exact_raster_session_control(id, 3)
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
                 "failure": interest.failure ?? "", "declines": interest.declines, "parked": interest.parked,
                 "held": interest.held, "retries": interest.retries, "retrying": interest.retrying, "pixels": [interest.view?.raster?.image.image.width ?? 0, interest.view?.raster?.image.image.height ?? 0],
                 "color": Self.colorFacts(interest), "timing": timings.stamps(view) ?? NSNull()]
            },
            "lateImages": timings.summary,
            "deferred": interests.values.filter { $0.admissionDeferred || $0.failure != nil || ($0.request != 0 && exact_raster_status(id, $0.request) == 2) }.count,
            "deferredAdmission": deferred.count, "parked": interests.values.filter(\.parked).count,
            "watched": parked.count,
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
            if let layer = interest.view?.imageContentLayer {
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
    /// wakes reconciliation, which asks again, and a pass that makes room
    /// wakes one itself. Any other refusal stands.
    static func transientRequestRefusal(_ code: UInt64) -> Bool { code == 8 }
}

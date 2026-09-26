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
    init(id: UInt64, name: String, resolver: AssetResolver) {
        self.id = id; key = RasterSourceKey(name: name, resolver: ObjectIdentifier(resolver)); self.resolver = resolver
    }
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
    static let sourceLimit = 1152
    var serial: UInt64 = 0
    var stopped = false
    var paused = false
    var wakePending = false
    var metadataReads = 0
    var decoded = 0
    var refusals = 0
    private var budgetNotice: [UInt64] = []
    init(id: UInt64) { self.id = id }

    func acquire(_ name: String, resolver: AssetResolver) -> RasterSource? {
        lock.lock(); defer { lock.unlock() }
        guard !stopped, name.utf8.count <= 4096 else { return nil }
        pruneLocked()
        let key = RasterSourceKey(name: name, resolver: ObjectIdentifier(resolver))
        if let id = byName[key], let source = sources[id]?.value, source.resolver === resolver,
           !source.cancellation.isCancelled {
            source.users += 1; return source
        }
        let live = sources.values.compactMap(\.value)
        guard live.count < Self.sourceLimit,
              live.filter({ $0.metadata == nil && $0.failure == nil }).count < 64 else { refusals += 1; return nil }
        serial += 1
        let source = RasterSource(id: serial, name: name, resolver: resolver)
        source.users = 1; sources[serial] = WeakRasterSource(source); byName[key] = serial
        return source
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
        // Detach strong cold owners under lock; destroy them after unlocking.
        let evicted = Array(cold.prefix(max(0, cold.count - RasterLoader.coldSources)))
        cold.removeFirst(evicted.count)
        pruneLocked()
        lock.unlock(); cancellation?.cancel()
        withExtendedLifetime(evicted) {}
        withExtendedLifetime(source) {}
    }
    private func pruneLocked() {
        sources = sources.filter { $0.value.value != nil }
        byName = byName.filter { sources[$0.value]?.value != nil }
    }
    func invalidate(_ name: String) {
        lock.lock(); byName = byName.filter { $0.key.name != name }; lock.unlock()
    }
    func metadata(_ source: RasterSource) -> (RasterMetadata?, String?) {
        lock.lock(); defer { lock.unlock() }; return (source.metadata, source.failure)
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
        } catch { failure = String(describing: error) }
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
        do {
            // Decode scope drops encoded data, source and staging before complete.
            let image: RasterImage = try autoreleasepool {
                let bytes = try withExtendedLifetime(resolver) { try input.bytes() }
                let plan = try RasterDecodePlan(metadata: metadata, maxPixel: Int(max(work.width, work.height)))
                guard plan.width == work.width, plan.height == work.height else { throw RasterFailure.reservation }
                guard exact_raster_is_cancelled(work.permit) == 0 else { throw RasterFailure.decode }
                return try RasterImage.decode(bytes, metadata: metadata, plan: plan, charge: charge, sourceOwner: source)
            }
            let owner = UInt64(UInt(bitPattern: Unmanaged.passRetained(image).toOpaque()))
            _ = exact_raster_complete(work.permit, owner, { value in
                if let pointer = UnsafeRawPointer(bitPattern: UInt(value)) {
                    Unmanaged<RasterImage>.fromOpaque(pointer).release()
                }
            }, UInt64(image.residentBytes))
            lock.lock(); decoded += 1; lock.unlock()
        } catch {
            exact_raster_fail(work.permit)
            lock.lock(); refusals += 1; lock.unlock()
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
        lock.lock(); let evicted = cold; cold.removeAll(); lock.unlock()
        exact_raster_session_control(id, 4)
        withExtendedLifetime(evicted) {}
    }
}

private final class WeakRasterBackend {
    weak var value: RasterBackend?
    init(_ value: RasterBackend) { self.value = value }
}

/// Exactly two process workers do both bounded metadata inspection and pixel
/// decode. The gate's wait does not demand UI frames or retain Runtime objects.
private final class RasterWorkers: @unchecked Sendable {
    static let shared = RasterWorkers()
    private let lock = NSLock()
    private var backends: [UInt64: WeakRasterBackend] = [:]
    private var cursor: UInt64 = 0
    private init() {
        for _ in 0..<2 { Thread.detachNewThread { [self] in run() } }
    }
    func add(_ backend: RasterBackend) { lock.lock(); backends[backend.id] = WeakRasterBackend(backend); lock.unlock() }
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
    private func run() {
        var metadataFirst = false
        while true {
            autoreleasepool {
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
    private struct Interest {
        weak var view: NodeView?
        let generation: Int
        let source: RasterSource
        var request: UInt64 = 0
        var requestedPixel = 0
        var offeredPixel = 0
        var failure: String?
        var delivered = false
    }
    /// Decoded pixels a session may hold: what its views pin, and a cache of
    /// what they let go, so a row that shows a source again, or another row
    /// showing it at the same size, takes the pixels without a decode. Eight
    /// screens of pixels, from 32 MiB to 192 MiB.
    static let screenBudget: UInt64 = {
        #if os(macOS)
        let screen = NSScreen.main.map { $0.frame.size.width * $0.frame.size.height * $0.backingScaleFactor * $0.backingScaleFactor } ?? 0
        #else
        let screen = UIScreen.main.nativeBounds.width * UIScreen.main.nativeBounds.height
        #endif
        return UInt64(min(192 * 1024 * 1024, max(32 * 1024 * 1024, screen * 4 * 8)))
    }()
    /// Sources no view shows whose metadata stays for the cache's keys.
    static let coldSources = 256
    let budget: UInt64
    let id: UInt64
    private let backend: RasterBackend
    private var interests: [UInt32: Interest] = [:]
    private var paused = false
    private var destroyed = false
    private var pressure: DispatchSourceMemoryPressure?
    init(budget: UInt64 = RasterLoader.screenBudget) {
        self.budget = budget
        id = exact_raster_session_create(budget)
        backend = RasterBackend(id: id); backend.loader = self
        RasterWorkers.shared.add(backend)
        let pressure = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .global(qos: .utility))
        pressure.setEventHandler { [weak backend] in backend?.trim() }
        pressure.resume(); self.pressure = pressure
    }
    deinit { shutdown() }
    @discardableResult func load(_ view: NodeView, source: String, resolver: AssetResolver) -> Bool {
        guard !destroyed, (interests[view.id] != nil || interests.count < 1024), let record = backend.acquire(source, resolver: resolver) else {
            view.presenter?.session?.log("image deferred: raster metadata/subscriber/source limit"); return false
        }
        cancel(view.id)
        interests[view.id] = Interest(view: view, generation: view.loadGeneration, source: record)
        // Initial props run before Presenter registers the new view. Reconcile
        // after the batch, when identity checks can distinguish it from removal.
        if !paused { backend.wake() }
        return true
    }
    func cancel(_ view: UInt32) {
        guard let interest = interests.removeValue(forKey: view) else { return }
        if interest.request != 0 { exact_raster_cancel(id, interest.request) }
        backend.release(interest.source.id)
    }
    func invalidate(_ source: String) { backend.invalidate(source) }
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
        let scale = view.window?.screen.scale ?? 1
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
        item.request = 0; item.requestedPixel = 0; item.failure = nil; item.delivered = false; interests[view] = item
        reconcile()
    }
    func reconcile() {
        guard !destroyed, !paused else { return }
        // Every image this turn lands reports its natural size together,
        // after the loop: one layout for them all, not one each.
        var landed: [(view: NodeView, generation: Int, size: CGSize)] = []
        defer { report(landed) }
        for viewID in Array(interests.keys) {
            guard var item = interests[viewID], let view = item.view,
                  view.loadGeneration == item.generation, view.presenter?.views[viewID] === view else { cancel(viewID); continue }
            if item.failure != nil || item.delivered { continue }
            if item.request != 0 {
                let state = exact_raster_status(id, item.request)
                if state == 4, let lease = NativeRasterLease(exact_raster_take_ready(id, item.request)) {
                    item.delivered = true; interests[viewID] = item
                    if let size = view.acceptRaster(lease, generation: item.generation) { landed.append((view, item.generation, size)) }
                } else if state >= 100 {
                    item.failure = Self.refusal(UInt64(state - 100)); interests[viewID] = item
                    view.presenter?.session?.log("image deferred: \(item.failure!)")
                }
                if state != 2 || item.requestedPixel <= 1 { continue }
                // Admission can become capacity-blocked after initially free
                // metadata was queued. A worker's changed-budget notice, never
                // a layout callback, reduces this pending demand in place,
                // unless it fits once the decodes holding reservations land:
                // then it waits for them, at the size it asked for.
                if let metadata = backend.metadata(item.source).0,
                   let plan = try? RasterDecodePlan(metadata: metadata, maxPixel: item.requestedPixel),
                   plan.peakBytes <= available() { continue }
                exact_raster_cancel(id, item.request); item.request = 0
                item.requestedPixel = max(1, item.requestedPixel / 2)
                interests[viewID] = item
            }
            let (metadata, failure) = backend.metadata(item.source)
            if let failure {
                item.failure = failure; interests[viewID] = item
                view.presenter?.session?.log("image deferred: \(failure)"); continue
            }
            guard let metadata else { continue }
            let offered = requestedPixel(view, metadata)
            var pixel = item.requestedPixel > 0 ? min(offered, item.requestedPixel) : offered
            let available = available()
            var plan = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel)
            while pixel > 1 && (plan == nil || plan!.peakBytes > available) {
                pixel = max(1, pixel / 2); plan = try? RasterDecodePlan(metadata: metadata, maxPixel: pixel)
            }
            guard let plan else { item.failure = "decode plan too large"; interests[viewID] = item; continue }
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
            item.request = exact_raster_request(id, demand); item.requestedPixel = pixel; item.offeredPixel = offered
            if item.request == 0 {
                item.failure = Self.refusal(exact_raster_stats(id).last_refusal)
                view.presenter?.session?.log("image deferred: \(item.failure!)")
            }
            interests[viewID] = item
            // A cache hit has no worker completion to wake delivery. Queue one
            // coalesced UI turn, keeping intrinsic publication outside a batch.
            if item.request != 0 && exact_raster_status(id, item.request) >= 4 { backend.wake() }
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
        for key in interests.keys { interests[key]?.request = 0; interests[key]?.failure = nil; interests[key]?.delivered = false }
        if !paused { reconcile() }
    }
    func reset() {
        interests.removeAll(); backend.reset(stop: false); exact_raster_session_control(id, 0)
    }
    func shutdown() {
        guard !destroyed else { return }; destroyed = true
        pressure?.cancel(); pressure = nil
        interests.removeAll(); backend.reset(stop: true); exact_raster_session_control(id, 3)
    }
    var diagnostics: [String: Any] {
        let s = exact_raster_stats(id)
        backend.lock.lock(); defer { backend.lock.unlock() }
        return ["residentBytes": s.resident_bytes, "reservedBytes": s.reserved_bytes, "peakBytes": s.peak_bytes,
            "pinnedBytes": s.pinned_bytes, "coldBytes": s.cold_bytes, "retiringBytes": s.retiring_bytes,
            "running": s.process_running, "sessionRunning": s.running, "ready": s.ready, "deliveryCells": s.delivery_cells,
            "pending": s.pending_jobs, "subscribers": s.subscribers, "coldEntries": s.cold_entries,
            "sources": backend.sources.count, "metadataReads": backend.metadataReads, "decoded": backend.decoded,
            "cancelled": s.cancelled, "sourceLimit": RasterBackend.sourceLimit,
            "refusals": backend.refusals, "viewInterests": interests.count, "lastRefusal": Self.refusal(s.last_refusal),
            "images": interests.map { view, interest -> [String: Any] in
                ["view": view, "source": String(interest.source.id), "offeredPixel": interest.offeredPixel,
                 "admittedPixel": interest.requestedPixel, "status": exact_raster_status(id, interest.request),
                 "failure": interest.failure ?? "", "pixels": [interest.view?.raster?.image.image.width ?? 0, interest.view?.raster?.image.image.height ?? 0]]
            },
            "deferred": interests.values.filter { $0.failure != nil || ($0.request != 0 && exact_raster_status(id, $0.request) == 2) }.count,
            "scope": "Exact-owned RGBA storage plus conservative thumbnail/conversion reservation; ImageIO internals excluded"]
    }
    private static func refusal(_ code: UInt64) -> String {
        let names = ["none", "overflow", "invalid dimensions", "encoded limit", "header limit", "source pixels",
            "too large", "budget", "queue full", "subscriber limit", "conflicting metadata", "paused", "shutdown",
            "stale", "actual exceeds reservation", "decode failed"]
        return code < names.count ? names[Int(code)] : "unknown refusal"
    }
}

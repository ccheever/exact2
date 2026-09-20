// The Swift face of the C ABI (host/apple/include/exact.h, v8): one
// `Runtime` per handle — created by `exact_create`, freed by
// `exact_destroy` — and one typed batch per call. Runtime exports take the
// handle (LLP 1031 D2), so a session that owns a runtime owns everything
// the library attributes to it, and nothing here is process-global but the
// buffer discipline: the app never hands the library a pointer it did not
// hand out.
import CExact
import Foundation

/// One batch from the library: the ops, and whether the presenter should keep
/// the clock (timers) or the display link (motion) running.
public struct Batch {
    public let ops: [[String: Any]]
    public let timers: Bool
    public let motion: Bool
    /// The runner's clock after the call, milliseconds (LLP 1012 `clock`).
    public let clock: Double?
    public let error: String?
    /// @ref LLP 1043.000 §3 D8 — absolute runner deadline, absent without timers.
    public var timerDueMs: Double? = nil
    public var pending = false
}

/// A runtime handle and its calls. `destroy` is idempotent at this layer and
/// one-shot at the C boundary; a call after it is refused by the library by
/// name, never a trap.
final class Runtime {
    let rt: ExactRuntime
    private(set) var destroyed = false

    init() {
        rt = exact_create()
    }

    deinit { destroy() }

    func destroy() {
        guard !destroyed else { return }
        destroyed = true
        exact_destroy(rt)
    }

    /// The text measurer (`TextEngine.measure`) and its context.
    func setMeasure(_ measure: ExactMeasureFn?, ctx: UnsafeMutableRawPointer?) { exact_set_measure(rt, measure, ctx) }
    /// The plan-font hook a boot calls synchronously, with its context.
    func setFonts(_ fonts: ExactFontsFn?, ctx: UnsafeMutableRawPointer?) { exact_set_fonts(rt, fonts, ctx) }
    /// The wake for a request's reply (LLP 1016 D2), on the executor's thread.
    func setWake(_ wake: ExactWakeFn?, ctx: UnsafeMutableRawPointer?) { exact_set_wake(rt, wake, ctx) }

    func read(_ len: UInt32) -> Batch {
        let data = Data(bytes: exact_out(rt), count: Int(len))
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return Batch(ops: [], timers: false, motion: false, clock: nil, error: "unreadable batch")
        }
        return Batch(ops: obj["ops"] as? [[String: Any]] ?? [], timers: obj["timers"] as? Bool ?? false, motion: obj["motion"] as? Bool ?? false, clock: obj["clock"] as? Double, error: obj["error"] as? String, timerDueMs: obj["timer_due_ms"] as? Double, pending: obj["pending"] as? Bool ?? false)
    }
    /// A payload into the runtime's input buffer; its length. An empty
    /// payload clears the buffer without dereferencing anything.
    func write(_ text: String) -> Int { write(Data(text.utf8)) }
    func write(_ data: Data) -> Int {
        guard !data.isEmpty, let p = exact_in(rt, data.count) else { _ = exact_in(rt, 0); return 0 }
        data.withUnsafeBytes { src in if let base = src.baseAddress { p.update(from: base.assumingMemoryBound(to: UInt8.self), count: data.count) } }
        return data.count
    }
    /// Boot the plan baked into the library under a viewport; the first batch.
    func boot(width: CGFloat, height: CGFloat) -> Batch { read(exact_boot(rt, Float(width), Float(height))) }
    /// Boot from plan bytes (the dev loop's restart; LLP 1007 §6): state
    /// carried — transactional, so a refused candidate leaves the running
    /// app exactly as it was.
    func bootPlan(_ bytes: Data, width: CGFloat, height: CGFloat) -> Batch {
        let n = write(bytes)
        return read(exact_boot_plan(rt, n, Float(width), Float(height)))
    }
    func preparePlan(_ bytes: Data, width: CGFloat, height: CGFloat, token: UInt64 = 0) -> Batch {
        let n = write(bytes)
        return read(exact_prepare_plan(rt, token, n, Float(width), Float(height)))
    }
    func commitPlan() -> Batch { read(exact_commit_plan(rt)) }
    func prepareModule(_ plan: Data, module: ExactModule, token: UInt64 = 0, width: CGFloat, height: CGFloat) -> Batch {
        var payload = plan
        payload.append(module.receipt)
        payload.append(module.bytecode)
        _ = write(payload)
        return read(exact_prepare_module(rt, token, plan.count, module.receipt.count, module.bytecode.count, Float(width), Float(height)))
    }
    func dataReady() -> Batch { read(exact_data_ready(rt)) }
    func discardPlan() { exact_discard_plan(rt) }

    /// Every queued reply into the runner: the batch of their commits.
    func pump(now: Double) -> Batch { read(exact_pump(rt, now)) }
    func press(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 0, 0, now)) }
    /// The pointer over the view (`true`) or gone from it.
    func hover(_ view: UInt32, over: Bool, now: Double) -> Batch { read(exact_dispatch(rt, view, over ? 2 : 3, 0, now)) }
    func focus(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 4, 0, now)) }
    func blur(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 5, 0, now)) }
    func contextmenu(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 10, 0, now)) }
    func holdBegin(_ view: UInt32, property: UInt32, now: Double) -> (NativeHold?, Batch) {
        let batch = read(exact_hold_begin(rt, view, property, now))
        let start = batch.ops.first { $0["op"] as? String == "hold" }.flatMap(NativeHold.init)
        return (start, batch)
    }
    func heightDragBegin(_ handleKey: UInt64, targetKey: UInt64, now: Double) -> (NativeHold?, Batch) {
        let batch = read(exact_height_drag_begin(rt, handleKey, targetKey, now))
        return (batch.ops.first { $0["op"] as? String == "hold" }.flatMap(NativeHold.init), batch)
    }
    func heightDragUpdate(_ token: UInt64, height: Double, now: Double) -> Batch {
        read(exact_height_drag_update(rt, token, height, now))
    }
    func heightDragRelease(_ token: UInt64, height: Double, velocity: Double, now: Double) -> Batch {
        read(exact_height_drag_release(rt, token, height, velocity, now))
    }
    func hasHold(_ token: UInt64) -> Bool { !destroyed && exact_has_hold(rt, token) != 0 }
    func holdUpdate(_ token: UInt64, x: Double, y: Double, now: Double) -> Batch {
        read(exact_hold_update(rt, token, x, y, now))
    }
    func holdEnd(_ token: UInt64, cancel: Bool, vx: Double = 0, vy: Double = 0, now: Double) -> Batch {
        read(exact_hold_end(rt, token, cancel ? 1 : 0, vx, vy, now))
    }
    func swiperight(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 12, 0, now)) }
    func pan(_ view: UInt32, dx: Double, dy: Double, now: Double) -> Batch {
        read(exact_dispatch(rt, view, 20, write("\(dx),\(dy)"), now))
    }
    func scroll(_ view: UInt32, left: Double, top: Double, now: Double) -> Batch {
        let n = write("\(left),\(top)")
        return read(exact_dispatch(rt, view, 13, n, now))
    }
    /// Actual viewport/row observations using the runner's versioned LE wire.
    func collectionFeedback(_ bytes: Data, now: Double) -> Batch {
        let n = write(bytes)
        return read(exact_collection_feedback(rt, n, now))
    }
    func dblclick(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 11, 0, now)) }
    func submit(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 7, 0, now)) }
    func media(_ view: UInt32, event: String, payload: String, now: Double) -> Batch {
        let n = write(event + "\n" + payload)
        return read(exact_dispatch(rt, view, 19, n, now))
    }
    func load(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 8, 0, now)) }
    func message(_ view: UInt32, _ value: String, now: Double) -> Batch {
        let n = write(value)
        return read(exact_dispatch(rt, view, 9, n, now))
    }
    /// A key down at the view, by the web's key name.
    func key(_ view: UInt32, _ name: String, now: Double) -> Batch {
        let n = write(name)
        return read(exact_dispatch(rt, view, 6, n, now))
    }
    func change(_ view: UInt32, _ value: String, now: Double) -> Batch {
        let n = write(value)
        return read(exact_dispatch(rt, view, 1, n, now))
    }
    // @ref LLP 1038 D8/D11 — Rust owns URL interpretation on every host.
    func location(of href: String) -> String {
        let n = write(href)
        let len = exact_location_of(rt, n)
        return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
    }
    func launch(_ location: String) { let n = write(location); _ = exact_set_launch_location(rt, n) }
    func navigate(_ view: UInt32, _ location: String, now: Double) -> Batch {
        let n = write(location)
        return read(exact_dispatch(rt, view, 14, n, now))
    }
    func advance(now: Double) -> Batch { read(exact_advance(rt, now)) }
    func resize(width: CGFloat, height: CGFloat) -> Batch { read(exact_resize(rt, Float(width), Float(height))) }
    /// `limit` rations the report (`exact.h`): 0 is the whole window.
    func list(_ view: UInt32, top: Double, height: Double, width: Double, origin: Double, focus: UInt32, interaction: UInt32, limit: UInt32 = 0) -> Batch {
        read(exact_list(rt, view, top, height, width, origin, focus, interaction, limit))
    }
    func listPending(_ view: UInt32) -> Bool { exact_list_pending(rt, view) != 0 }
    func listIndex(_ view: UInt32, key: String) -> Int? {
        let n = write(key)
        let index = exact_list_index(rt, view, UInt32(n))
        return index == UInt32.max ? nil : Int(index)
    }
    func listText(_ view: UInt32, first: (String, Int, Int)?, last: (String, Int, Int)?) -> String {
        let a = first?.0 ?? "", b = last?.0 ?? ""
        let n = write(a + b)
        let len = exact_list_text(rt, view, UInt32(a.utf8.count), UInt32(n), UInt32(first?.1 ?? 0), UInt32(first?.2 ?? 0), UInt32(last?.1 ?? 0), UInt32(last?.2 ?? 0))
        return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
    }
    func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) -> Batch { read(exact_insets(rt, Float(top), Float(right), Float(bottom), Float(left))) }
    func tick(now: Double) -> Batch { read(exact_tick(rt, now)) }
    func intrinsic(_ view: UInt32, width: CGFloat, height: CGFloat) -> Batch { read(exact_intrinsic(rt, view, Float(width), Float(height))) }
    /// Refresh the runner's delivery facts after an app-level event (LLP 1030 D7).
    func deliverySync() -> Batch { read(exact_delivery_sync(rt)) }
    /// The returned JSON is copied before the runtime output buffer is reused.
    func regionRequest(_ id: UInt64, knownSource: UInt64) -> [String: Any]? {
        let length = exact_region_request(rt, id, knownSource)
        let data = Data(bytes: exact_out(rt), count: Int(length))
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }
    func regionComplete(_ artifact: RegionArtifact) -> Batch {
        let p = artifact.metadata
        let retained = Unmanaged.passRetained(artifact).toOpaque()
        return read(exact_region_complete(rt, artifact.id,
            ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline)),
            retained, { pointer in
                if let pointer { Unmanaged<RegionArtifact>.fromOpaque(pointer).release() }
            }))
    }
    /// The agent API (LLP 1012): a request in, its reply out — JSON, not a batch.
    func agent(_ request: String) -> String {
        let n = write(request)
        let len = exact_agent(rt, n)
        return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
    }
    /// A line for the runner's journal (LLP 1012 §3): what this host refused, and why.
    func log(_ line: String) {
        _ = exact_log(rt, write(line))
    }
}

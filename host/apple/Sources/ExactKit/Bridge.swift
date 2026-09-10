// The Swift face of the C ABI (host/apple/include/exact.h, v4): one
// `Runtime` per handle — created by `exact_create`, freed by
// `exact_destroy` — and one typed batch per call. Every export takes the
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
        return Batch(ops: obj["ops"] as? [[String: Any]] ?? [], timers: obj["timers"] as? Bool ?? false, motion: obj["motion"] as? Bool ?? false, clock: obj["clock"] as? Double, error: obj["error"] as? String)
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
    func prepareModule(_ plan: Data, module: ExactModule, width: CGFloat, height: CGFloat) -> Batch {
        var payload = plan
        payload.append(module.receipt)
        payload.append(module.bytecode)
        _ = write(payload)
        return read(exact_prepare_module(rt, plan.count, module.receipt.count, module.bytecode.count, Float(width), Float(height)))
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
    func dragX(_ view: UInt32, delta: Double, velocity: Double, release: Bool, now: Double) -> Batch { read(exact_drag_x(rt, view, delta, velocity, release ? 1 : 0, now)) }
    func swiperight(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 12, 0, now)) }
    func scroll(_ view: UInt32, left: Double, top: Double, now: Double) -> Batch {
        let n = write("\(left),\(top)")
        return read(exact_dispatch(rt, view, 13, n, now))
    }
    func dblclick(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 11, 0, now)) }
    func submit(_ view: UInt32, now: Double) -> Batch { read(exact_dispatch(rt, view, 7, 0, now)) }
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
    func advance(now: Double) -> Batch { read(exact_advance(rt, now)) }
    func resize(width: CGFloat, height: CGFloat) -> Batch { read(exact_resize(rt, Float(width), Float(height))) }
    func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) -> Batch { read(exact_insets(rt, Float(top), Float(right), Float(bottom), Float(left))) }
    func tick(now: Double) -> Batch { read(exact_tick(rt, now)) }
    func intrinsic(_ view: UInt32, width: CGFloat, height: CGFloat) -> Batch { read(exact_intrinsic(rt, view, Float(width), Float(height))) }
    /// Refresh the runner's delivery facts after an app-level event (LLP 1030 D7).
    func deliverySync() -> Batch { read(exact_delivery_sync(rt)) }
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

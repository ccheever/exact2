// The Swift face of the C ABI (host/apple/include/exact.h, v9): one
// `Runtime` per handle — created by `exact_create`, freed by
// `exact_destroy` — and one typed batch per call. Runtime exports take the
// handle (LLP 1031 D2), so a session that owns a runtime owns everything
// the library attributes to it, and nothing here is process-global but the
// buffer discipline: the app never hands the library a pointer it did not
// hand out.
// Every runtime lives on the owner thread (`Owner.swift`, LLP 1071): each
// call below runs there as one job while the caller waits.
import CExact
import Foundation

/// One batch from the library: the ops, and whether the presenter should keep
/// the clock (timers) or the display link (motion) running.
public struct Batch {
    public let ops: [BatchOp]
    public let timers: Bool
    public let motion: Bool
    /// The runner's clock after the call, milliseconds (LLP 1012 `clock`).
    public let clock: Double?
    public let error: String?
    /// @ref LLP 1043.000 §3 D8 — absolute runner deadline, absent without timers.
    public var timerDueMs: Double? = nil
    public var pending = false
    /// What moves changes place or size: the panel's full rate (LLP 1061 D4).
    public var spatial = false
    /// A 2D canvas wants the next display frame (LLP 1056 D5).
    public var canvas = false
    /// Image handles a 2D canvas asked for, to decode (LLP 1056 D9).
    public var canvasImages: [String] = []
    init(ops: [BatchOp], timers: Bool, motion: Bool, clock: Double?, error: String?, timerDueMs: Double? = nil, pending: Bool = false) {
        self.ops = ops; self.timers = timers; self.motion = motion; self.clock = clock
        self.error = error; self.timerDueMs = timerDueMs; self.pending = pending
    }
    static func decode(_ data: Data) -> Batch {
        data.withUnsafeBytes { decode($0.bindMemory(to: UInt8.self)) }
    }
    static func decode(_ bytes: UnsafeBufferPointer<UInt8>) -> Batch {
        var reader = BatchReader(bytes: bytes)
        do {
            let batch = try reader.batch()
            try reader.end()
            return batch
        } catch {
            return Batch(ops: [], timers: false, motion: false, clock: nil, error: "unreadable batch")
        }
    }

}

/// A runtime handle and its calls. `destroy` is idempotent at this layer and
/// one-shot at the C boundary; a call after it is refused by the library by
/// name, never a trap.
final class Runtime {
    let rt: ExactRuntime
    private(set) var destroyed = false
    #if DEBUG
    // Per-runtime observation for differential tests of actual session traffic.
    // Release builds have neither the callback nor a copy of the wire bytes.
    var observeBatch: ((Data, Batch) -> Void)?
    #endif

    init() {
        rt = Owner.shared.sync { exact_create() }
    }

    deinit { destroy() }

    func destroy() {
        guard !destroyed else { return }
        destroyed = true
        let rt = rt
        on { exact_destroy(rt) }
    }

    // @ref LLP 1071 T1/T2 — every runtime lives on the owner thread; each
    // call below is one owner job (input, operation, output decode) that
    // main waits for. A call made while main runs a callback the owner is
    // waiting on cannot be served (T5): a batch is refused as `busy`, a
    // notification runs after the owner's current job, anything else
    // answers `busy`.
    static let busy = Batch(ops: [], timers: false, motion: false, clock: nil,
        error: "busy: the owner is waiting on this callback (LLP 1071 T5)")
    static let busyAgent = "{\"error\":\"busy: the owner is waiting on this callback (LLP 1071 T5)\"}"
    func on(_ body: () -> Batch) -> Batch { Owner.shared.sync(body, busy: Runtime.busy) }
    func on<T>(busy: @autoclosure () -> T, _ body: () -> T) -> T { Owner.shared.sync(body, busy: busy()) }
    func on(_ body: () -> Void) { Owner.shared.sync(body, busy: ()) }

    /// The text measurer (`TextEngine.measure`) and its context.
    func setMeasure(_ measure: ExactMeasureFn?, ctx: UnsafeMutableRawPointer?) {
        let ctx = UInt(bitPattern: ctx)
        on { exact_set_measure(rt, measure, UnsafeMutableRawPointer(bitPattern: ctx)) }
    }
    /// The plan-font hook a boot calls synchronously, with its context.
    func setFonts(_ fonts: ExactFontsFn?, ctx: UnsafeMutableRawPointer?) {
        let ctx = UInt(bitPattern: ctx)
        on { exact_set_fonts(rt, fonts, UnsafeMutableRawPointer(bitPattern: ctx)) }
    }
    /// The wake for a request's reply (LLP 1016 D2), on the executor's thread.
    func setWake(_ wake: ExactWakeFn?, ctx: UnsafeMutableRawPointer?) {
        let ctx = UInt(bitPattern: ctx)
        on { exact_set_wake(rt, wake, UnsafeMutableRawPointer(bitPattern: ctx)) }
    }
    /// This binary's baked `compat.json`, read on a throwaway runtime.
    static func bakedCompat() -> Data {
        let runtime = Runtime()
        defer { runtime.destroy() }
        return runtime.on(busy: Data()) {
            let length = exact_baked_compat(runtime.rt)
            return Data(bytes: exact_out(runtime.rt), count: Int(length))
        }
    }

    func read(_ len: UInt32) -> Batch {
        // The runtime owns these bytes until its next call. The reader copies
        // strings into Swift values before returning; no batch borrows the buffer.
        let bytes = UnsafeBufferPointer(start: exact_out(rt), count: Int(len))
        let batch = Batch.decode(bytes)
        #if DEBUG
        observeBatch?(Data(bytes), batch)
        #endif
        return batch
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
    /// The display's scale and memory for Canvas 2D (LLP 1056 D4).
    func canvasDisplay(scale: CGFloat, memory: UInt64) -> Batch { on { read(exact_canvas_display(rt, Double(scale), Double(memory))) } }
    /// A Canvas 2D image handle decoded (its size in pixels) or not (LLP 1056 D9).
    func canvasImage(_ src: String, width: Int, height: Int, ok: Bool) -> Batch {
        return on {
            let n = write(src)
            return read(exact_canvas_image(rt, n, UInt32(max(0, width)), UInt32(max(0, height)), ok ? 1 : 0))
        }
    }
    /// A 2D canvas's replay is behind or caught up (LLP 1056 D5).
    func canvasHeld(_ view: UInt32, _ held: Bool) { on { exact_canvas_held(rt, view, held ? 1 : 0) } }
    /// The Canvas 2D text measurer (LLP 1056 D8), with the measurer's context.
    func setCanvasText(_ measure: ExactCanvasTextFn?) { on { exact_set_canvas_text(rt, measure) } }
    func boot(width: CGFloat, height: CGFloat) -> Batch { islands(on { read(exact_boot(rt, Float(width), Float(height))) }) }
    /// A plan that can show an SVG island opens the island module off the
    /// main thread now, before its first mask or filter needs it (LLP
    /// 1055.000 §8 ruling 4); a plan without one never loads it.
    private func islands(_ batch: Batch) -> Batch {
        if on(busy: false, { exact_svg_islands(rt) != 0 }) { SvgRasterModule.prewarm() }
        return batch
    }
    /// Boot from plan bytes (the dev loop's restart; LLP 1007 §6): state
    /// carried — transactional, so a refused candidate leaves the running
    /// app exactly as it was.
    func bootPlan(_ bytes: Data, width: CGFloat, height: CGFloat) -> Batch {
        let batch = on { () -> Batch in
            let n = write(bytes)
            return read(exact_boot_plan(rt, n, Float(width), Float(height)))
        }
        return islands(batch)
    }
    func preparePlan(_ bytes: Data, width: CGFloat, height: CGFloat, token: UInt64 = 0) -> Batch {
        return on {
            let n = write(bytes)
            return read(exact_prepare_plan(rt, token, n, Float(width), Float(height)))
        }
    }
    func commitPlan() -> Batch { islands(on { read(exact_commit_plan(rt)) }) }
    func prepareModule(_ plan: Data, module: ExactModule, token: UInt64 = 0, width: CGFloat, height: CGFloat) -> Batch {
        return on {
            var payload = plan
            payload.append(module.receipt)
            payload.append(module.bytecode)
            _ = write(payload)
            return read(exact_prepare_module(rt, token, plan.count, module.receipt.count, module.bytecode.count, Float(width), Float(height)))
        }
    }
    func dataReady() -> Batch { on { read(exact_data_ready(rt)) } }
    func discardPlan() { on { exact_discard_plan(rt) } }
    func trim() { on { exact_trim(rt) } }

    /// Every queued reply into the runner: the batch of their commits.
    func pump(now: Double) -> Batch { on { read(exact_pump(rt, now)) } }
    func requestActive(_ ticket: UInt64) -> Bool { on(busy: false) { exact_request_active(rt, ticket) != 0 } }
    func fulfillSurface(_ ticket: UInt64, kind: UInt32, body: Data = Data(), now: Double) -> Batch {
        return on {
            let n = write(body)
            return read(exact_fulfill_surface(rt, ticket, kind, n, now))
        }
    }
    func press(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 0, 0, now)) } }
    /// The pointer over the view (`true`) or gone from it.
    func hover(_ view: UInt32, over: Bool, now: Double) -> Batch { on { read(exact_dispatch(rt, view, over ? 2 : 3, 0, now)) } }
    func focus(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 4, 0, now)) } }
    func blur(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 5, 0, now)) } }
    func contextmenu(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 10, 0, now)) } }
    func holdBegin(_ view: UInt32, property: UInt32, now: Double) -> (NativeHold?, Batch) {
        return on(busy: (nil, Runtime.busy)) {
            let batch = read(exact_hold_begin(rt, view, property, now))
            let start = batch.ops.first { $0.op == .hold }.flatMap { NativeHold($0.payload) }
            return (start, batch)
        }
    }
    func heightDragBegin(_ handleKey: UInt64, targetKey: UInt64, now: Double) -> (NativeHold?, Batch) {
        return on(busy: (nil, Runtime.busy)) {
            let batch = read(exact_height_drag_begin(rt, handleKey, targetKey, now))
            return (batch.ops.first { $0.op == .hold }.flatMap { NativeHold($0.payload) }, batch)
        }
    }
    func heightDragUpdate(_ token: UInt64, height: Double, now: Double) -> Batch {
        return on {
            read(exact_height_drag_update(rt, token, height, now))
        }
    }
    func heightDragRelease(_ token: UInt64, height: Double, now: Double) -> Batch {
        return on {
            read(exact_height_drag_release(rt, token, height, now))
        }
    }
    func reorderBegin(_ handle: UInt32, scrollTop: Double, now: Double) -> Batch {
        return on {
            read(exact_reorder_begin(rt, handle, scrollTop, now))
        }
    }
    func reorderMove(_ token: UInt64, dy: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch {
        return on {
            read(exact_reorder_move(rt, token, dy, scrollTop, inside ? 1 : 0, now))
        }
    }
    func reorderEnd(_ token: UInt64, drop: Bool, dy: Double, scrollTop: Double, inside: Bool, velocity: Double, now: Double) -> Batch {
        return on {
            read(exact_reorder_end(rt, token, drop ? 1 : 0, dy, scrollTop, inside ? 1 : 0, velocity, now))
        }
    }
    func hasHold(_ token: UInt64) -> Bool { on(busy: false) { !destroyed && exact_has_hold(rt, token) != 0 } }
    func holdUpdate(_ token: UInt64, x: Double, y: Double, now: Double) -> Batch {
        return on {
            read(exact_hold_update(rt, token, x, y, now))
        }
    }
    /// `measured`: release at the engine's own velocity estimate (LLP 1057.001 §3).
    func holdEnd(_ token: UInt64, cancel: Bool, measured: Bool = false, vx: Double = 0, vy: Double = 0, now: Double) -> Batch {
        return on {
            read(exact_hold_end(rt, token, cancel ? 1 : measured ? 2 : 0, vx, vy, now))
        }
    }
    func swiperight(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 12, 0, now)) } }
    func refresh(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 22, 0, now)) } }
    func pan(_ view: UInt32, dx: Double, dy: Double, now: Double) -> Batch {
        return on {
            read(exact_dispatch(rt, view, 20, write("\(dx),\(dy)"), now))
        }
    }
    /// A pan that began ended at (vx, vy) viewport px/s (LLP 1057 §10.6).
    func panRelease(_ view: UInt32, vx: Double, vy: Double, now: Double) -> Batch {
        read(exact_dispatch(rt, view, 28, write("\(vx),\(vy)"), now))
    }
    /// The pan contact's samples, where the platform measures no velocity:
    /// the engine's tracker (LLP 1057.001 §3), viewport px at `t` seconds.
    func panSample(first: Bool, x: Double, y: Double, t: Double) { _ = exact_pan_sample(rt, first ? 1 : 0, x, y, t) }
    func panVelocity(at t: Double) -> (Double, Double) { (exact_pan_velocity(rt, 0, t), exact_pan_velocity(rt, 1, t)) }
    func scroll(_ view: UInt32, left: Double, top: Double, now: Double) -> Batch {
        return on {
            let n = write("\(left),\(top)")
            return read(exact_dispatch(rt, view, 13, n, now))
        }
    }
    /// The agent's `tap <list> into <key>` (LLP 1070.000 §5).
    func intoView(_ view: UInt32, key: String, block: String, inline: String) -> Batch {
        return on {
            let n = write(key + "\n" + block + "\n" + inline)
            return read(exact_into_view(rt, view, n))
        }
    }
    /// A build-only report on the owner, not waited for (LLP 1071 §3):
    /// `done` runs on the owner with the decoded batch.
    func collectionFeedbackAsync(_ bytes: Data, now: Double, done: @escaping (Batch) -> Void) {
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            let n = write(bytes)
            done(read(exact_collection_feedback(rt, n, now)))
        }
    }
    /// Actual viewport/row observations using the runner's versioned LE wire.
    func collectionFeedback(_ bytes: Data, now: Double) -> Batch {
        return on {
            let n = write(bytes)
            return read(exact_collection_feedback(rt, n, now))
        }
    }
    func dblclick(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 11, 0, now)) } }
    func submit(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 7, 0, now)) } }
    func media(_ view: UInt32, event: String, payload: String, now: Double) -> Batch {
        return on {
            let n = write(event + "\n" + payload)
            return read(exact_dispatch(rt, view, 19, n, now))
        }
    }
    func load(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 8, 0, now)) } }
    func surfaceRecord(_ name: String, _ json: String?) -> Batch {
        return on {
            let n = write(name + (json.map { "\0" + $0 } ?? ""))
            return read(exact_surface_record(rt, n))
        }
    }
    func message(_ view: UInt32, _ value: String, now: Double) -> Batch {
        return on {
            let n = write(value)
            return read(exact_dispatch(rt, view, 9, n, now))
        }
    }
    /// A key down at the view, by the web's key name.
    func key(_ view: UInt32, _ name: String, now: Double) -> Batch {
        return on {
            let n = write(name)
            return read(exact_dispatch(rt, view, 6, n, now))
        }
    }
    func change(_ view: UInt32, _ value: String, now: Double) -> Batch {
        return on {
            let n = write(value)
            return read(exact_dispatch(rt, view, 1, n, now))
        }
    }
    /// A text field's value as it moves: HTML's `input` (LLP 1069.001 D4).
    func input(_ view: UInt32, _ value: String, now: Double) -> Batch {
        return on {
            let n = write(value)
            return read(exact_dispatch(rt, view, 23, n, now))
        }
    }
    /// A checkbox's state: `change` when `commit`, else `input`.
    func checked(_ view: UInt32, _ checked: Bool, commit: Bool, now: Double) -> Batch {
        return on {
            let n = write(checked ? "true" : "false")
            return read(exact_dispatch(rt, view, commit ? 24 : 25, n, now))
        }
    }
    /// A file input's picked files, one tab-separated line each (LLP 1069.002 D3).
    func picked(_ view: UInt32, _ payload: String, now: Double) -> Batch {
        return on {
            let n = write(payload)
            return read(exact_dispatch(rt, view, 26, n, now))
        }
    }
    /// A file input's picker was dismissed: HTML's `cancel` (LLP 1069.002 D2).
    func pickerCancel(_ view: UInt32, now: Double) -> Batch {
        return on {
            let n = write("")
            return read(exact_dispatch(rt, view, 27, n, now))
        }
    }
    /// Shared Markdown selection facts; the editor retains its own range.
    func selection(_ view: UInt32, json: String, now: Double) -> Batch? {
        return on(busy: nil) {
            guard let data = json.data(using: .utf8),
                  let state = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
                  let formats = state["formats"] as? String,
                  let mixed = state["mixed"] as? Bool,
                  let link = state["link"] as? String,
                  let unavailable = state["unavailable"] as? String else { return nil }
            let n = write(formats + "\n" + (mixed ? "1" : "0") + "\n" + unavailable + "\n" + link)
            return read(exact_dispatch(rt, view, 21, n, now))
        }
    }
    // @ref LLP 1038 D8/D11 — Rust owns URL interpretation on every host.
    func location(of href: String) -> String {
        return on(busy: "") {
            let n = write(href)
            let len = exact_location_of(rt, n)
            return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    func launch(_ location: String) { on { () -> Void in let n = write(location); _ = exact_set_launch_location(rt, n) } }
    func navigate(_ view: UInt32, _ location: String, now: Double) -> Batch {
        return on {
            let n = write(location)
            return read(exact_dispatch(rt, view, 14, n, now))
        }
    }
    func advance(now: Double, untilRequest: Bool = false) -> Batch { on { read(exact_advance(rt, now, untilRequest ? 1 : 0)) } }
    func resize(width: CGFloat, height: CGFloat) -> Batch { on { read(exact_resize(rt, Float(width), Float(height))) } }
    func setTime(epochAtZero: Double, utcOffset: Double) -> Batch { on { read(exact_set_time(rt, epochAtZero, utcOffset)) } }
    func setPreferences(_ bits: UInt32) -> Batch { on { read(exact_set_preferences(rt, bits)) } }
    func setPage(_ bits: UInt32) -> Batch { on { read(exact_set_page(rt, bits)) } }
    func setRootFontSize(_ px: Double) -> Batch { on { read(exact_set_root_font_size(rt, px)) } }
    func setPlace(locale: String, timeZone: String, seed: UInt64) -> Batch { on { read(exact_set_place(rt, write(locale + "\0" + timeZone + "\0" + String(seed))) ) } }
    func listIndex(_ view: UInt32, key: String) -> Int? {
        return on(busy: nil) {
            let n = write(key)
            let index = exact_list_index(rt, view, UInt32(n))
            return index == UInt32.max ? nil : Int(index)
        }
    }
    func listText(_ view: UInt32, first: (String, Int, Int)?, last: (String, Int, Int)?) -> String {
        return on(busy: "") {
            let a = first?.0 ?? "", b = last?.0 ?? ""
            let n = write(a + b)
            let len = exact_list_text(rt, view, UInt32(a.utf8.count), UInt32(n), UInt32(first?.1 ?? 0), UInt32(first?.2 ?? 0), UInt32(last?.1 ?? 0), UInt32(last?.2 ?? 0))
            return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) -> Batch { on { read(exact_insets(rt, Float(top), Float(right), Float(bottom), Float(left))) } }
    func tick(now: Double) -> Batch { on { read(exact_tick(rt, now)) } }
    func scheme(dark: Bool) -> Batch { on { read(exact_scheme(rt, dark ? 1 : 0)) } }
    func viewScheme(_ view: UInt32, dark: Bool) -> Batch { on { read(exact_view_scheme(rt, view, dark ? 1 : 0)) } }
    /// A select's options and the one it shows (LLP 1069.001 D5).
    func selectOptions(_ view: UInt32) -> SelectMenu {
        return on(busy: SelectMenu(json: Data())) {
            let len = exact_select_options(rt, view)
            return SelectMenu(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
    }
    /// Host intrinsic sizes (nil clears one), under one layout.
    func intrinsics(_ sizes: [(UInt32, CGSize?)]) -> Batch {
        return on {
            var bytes = Data(capacity: sizes.count * 12)
            for (view, size) in sizes {
                for word in [view, Float(size?.width ?? 0).bitPattern, Float(size?.height ?? 0).bitPattern] {
                    withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) }
                }
            }
            return read(exact_intrinsics(rt, write(bytes)))
        }
    }
    /// Refresh the runner's delivery facts after an app-level event (LLP 1030 D7).
    func deliverySync() -> Batch { on { read(exact_delivery_sync(rt)) } }
    /// The returned JSON is copied before the runtime output buffer is reused.
    func textReady(index: UInt32, generation: UInt32, revision: UInt64) -> Batch {
        return on {
            read(exact_text_ready(rt, index, generation, revision))
        }
    }
    func regionRequest(_ id: UInt64, knownSource: UInt64) -> [String: Any]? {
        return on(busy: nil) {
            let length = exact_region_request(rt, id, knownSource)
            let data = Data(bytes: exact_out(rt), count: Int(length))
            return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        }
    }
    func regionComplete(_ artifact: RegionArtifact) -> Batch {
        return on {
            let p = artifact.metadata
            let retained = Unmanaged.passRetained(artifact).toOpaque()
            return read(exact_region_complete(rt, artifact.id,
                ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline)),
                retained, { pointer in
                    if let pointer { Unmanaged<RegionArtifact>.fromOpaque(pointer).release() }
                }))
        }
    }
    /// The agent API (LLP 1012): a request in, its reply out — JSON, not a batch.
    func agent(_ request: String) -> String {
        return on(busy: Runtime.busyAgent) {
            let n = write(request)
            let len = exact_agent(rt, n)
            return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    /// A command's ruling (`share`, LLP 1069.003; `saveFile`, LLP 1069.010):
    /// refused, held, or present it.
    func command(_ request: [String: Any]) -> [String: Any] {
        return on(busy: ["refused": "busy"]) {
            guard let json = try? JSONSerialization.data(withJSONObject: request) else { return ["refused": "unreadable"] }
            let len = exact_command(rt, write(String(decoding: json, as: UTF8.self)))
            let data = Data(bytes: exact_out(rt), count: Int(len))
            return (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? ["refused": "unreadable"]
        }
    }
    /// An auth session's word (LLP 1069.006): `hold` under the agent, or
    /// `done` with the callback URL or a status. The next pump delivers it.
    func auth(_ request: [String: Any]) {
        on {
            guard let json = try? JSONSerialization.data(withJSONObject: request) else { return }
            _ = exact_auth(rt, write(String(decoding: json, as: UTF8.self)))
        }
    }
    /// A line for the runner's journal (LLP 1012 §3): what this host refused, and why.
    /// An app module's topic changed (LLP 1024): a notification (T5).
    func appChanged(_ topic: Data) {
        Owner.shared.syncOrLater { [self] in
            guard !destroyed else { return }
            topic.withUnsafeBytes { exact_app_changed(rt, $0.bindMemory(to: UInt8.self).baseAddress, topic.count) }
        }
    }
    /// A notification (LLP 1071 T5): when main is serving a callback the
    /// owner waits on, the line is journaled after the owner's current job.
    func log(_ line: String) {
        Owner.shared.syncOrLater { [self] in
            guard !destroyed else { return }
            _ = exact_log(rt, write(line))
        }
    }
}

// The Swift face of the C ABI (host/apple/include/exact.h, v9): one
// `Runtime` per handle — created by `exact_create`, freed by
// `exact_destroy` — and one typed batch per call. Runtime exports take the
// handle (LLP 1031 D2), so a session that owns a runtime owns everything
// the library attributes to it, and nothing here is process-global but the
// buffer discipline: the app never hands the library a pointer it did not
// hand out.
// Every runtime lives on the owner thread (`Owner.swift`, LLP 1072): each
// call below runs there as one job while the caller waits.
import CExact
import Foundation

/// One batch from the library: the ops, and whether the presenter should keep
/// the clock (timers) or the display link (motion) running.
public struct Batch {
    public internal(set) var ops: [BatchOp]
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
    /// A frame task wants every display frame (LLP 1073 D5).
    public var frames = false
    /// A canvas draw is owed to a turn of its own (LLP 1072 §8.5).
    public var canvasOwed = false
    /// A control's viewless contents changed (LLP 1069.011 §9).
    public var controls = false
    /// Image handles a 2D canvas asked for, to decode (LLP 1056 D9).
    public var canvasImages: [String] = []
    /// The kernel transactions it carries, while the runner measures (LLP 1079 D3).
    public var seq: (UInt64, UInt64)?
    init(ops: [BatchOp], timers: Bool, motion: Bool, clock: Double?, error: String?, timerDueMs: Double? = nil, pending: Bool = false) {
        self.ops = ops; self.timers = timers; self.motion = motion; self.clock = clock
        self.error = error; self.timerDueMs = timerDueMs; self.pending = pending
    }
    static func decode(_ data: Data, resolver: AssetResolver? = nil) -> Batch {
        data.withUnsafeBytes { decode($0.bindMemory(to: UInt8.self), resolver: resolver) }
    }
    static func decode(_ bytes: UnsafeBufferPointer<UInt8>, resolver: AssetResolver? = nil) -> Batch {
        var reader = BatchReader(bytes: bytes, profileResolver: resolver)
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

    // @ref LLP 1072 T1/T2 — every runtime lives on the owner thread; each
    // call below is one owner job (input, operation, output decode) that
    // main waits for. A call made while main runs a callback the owner is
    // waiting on cannot be served (T5): a batch is refused as `busy`, a
    // notification runs after the owner's current job, anything else
    // answers `busy`.
    static let busy = Batch(ops: [], timers: false, motion: false, clock: nil,
        error: "busy: the owner is waiting on this callback (LLP 1072 T5)")
    static let busyAgent = "{\"error\":\"busy: the owner is waiting on this callback (LLP 1072 T5)\"}"
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

    private var profileResolver: AssetResolver?
    func setProfileResolver(_ resolver: AssetResolver?) { on { profileResolver = resolver } }

    func read(_ len: UInt32) -> Batch {
        // The runtime owns these bytes until its next call. The reader copies
        // strings into Swift values before returning; no batch borrows the buffer.
        let bytes = UnsafeBufferPointer(start: exact_out(rt), count: Int(len))
        var batch = Batch.decode(bytes, resolver: profileResolver)
        #if DEBUG
        // What was decoded, before `prepare` adds the owner's values.
        observeBatch?(Data(bytes), batch)
        #endif
        batch.prepare()
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
    /// 1055.000 §8 ruling 4); a plan without one never loads it. One that
    /// can show a filter makes the GPU filter pipelines now too: a first
    /// picture is drawn in the commit that shows it.
    private func islands(_ batch: Batch) -> Batch {
        let svg = on(busy: UInt8(0)) { exact_svg_islands(rt) }
        if svg & 1 != 0 { SvgRasterModule.prewarm() }
        #if os(iOS) || os(tvOS)
        if svg & 2 != 0 { SvgFilterMetal.prewarm() }
        #endif
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
    /// A press, with the modifiers held as a chord prefix (`KeyCodes.held`).
    func press(_ view: UInt32, held: String = "", now: Double) -> Batch { on { read(exact_dispatch(rt, view, 0, write(held), now)) } }
    /// The pointer over the view (`true`) or gone from it.
    func hover(_ view: UInt32, over: Bool, now: Double) -> Batch { on { read(exact_dispatch(rt, view, over ? 2 : 3, 0, now)) } }
    func focus(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 4, 0, now)) } }
    func blur(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 5, 0, now)) } }
    func contextmenu(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 10, 0, now)) } }
    /// DOM's `beforeunload` (36) at a node that hears it (studio diary R17).
    func beforeunload(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 36, 0, now)) } }
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
        on { read(exact_dispatch(rt, view, 28, write("\(vx),\(vy)"), now)) }
    }
    /// The pan contact's samples, where the platform measures no velocity:
    /// the engine's tracker (LLP 1057.001 §3), viewport px at `t` seconds.
    func panSample(first: Bool, x: Double, y: Double, t: Double) { on { () -> Void in _ = exact_pan_sample(rt, first ? 1 : 0, x, y, t) } }
    func panVelocity(at t: Double) -> (Double, Double) { on(busy: (0, 0)) { (exact_pan_velocity(rt, 0, t), exact_pan_velocity(rt, 1, t)) } }
    /// A scroller (nil: the page) now stands at `left`, `top` (LLP 1051.000
    /// D1): posted, never waited for, since AppKit calls from inside its
    /// scroll synchronizer.
    func scrolled(_ view: UInt32?, left: Double, top: Double) {
        Owner.shared.notify { [self] in
            guard !destroyed else { return }
            exact_scrolled(rt, view == nil ? 1 : 0, view ?? 0, left, top)
        }
    }
    /// A scroll event: left, top, then the scroller's `scrollWidth`,
    /// `scrollHeight`, `clientWidth` and `clientHeight` (`ScrollEvent`, chat F4).
    func scroll(_ view: UInt32, metrics: [Double], now: Double) -> Batch {
        return on {
            let n = write(metrics.map { "\($0)" }.joined(separator: ","))
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
    /// A build-only report on the owner, not waited for (LLP 1072 §3):
    /// `done` runs on the owner with the decoded batch.
    func collectionFeedbackAsync(_ bytes: Data, now: Double, done: @escaping (Batch) -> Void) {
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            let n = write(bytes)
            done(read(exact_collection_feedback(rt, n, now)))
        }
    }
    /// Canvas draws in a turn of their own (LLP 1072 §8.5).
    func canvasDefer(_ deferred: Bool) { on { exact_canvas_defer(rt, deferred ? 1 : 0) } }
    /// The owed canvas draws on the owner, not waited for (LLP 1072 §8.5).
    func canvasDrawAsync(done: @escaping (Batch) -> Void) {
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            done(read(exact_canvas_draw(rt)))
        }
    }
    /// A frame's tick on the owner, not waited for (LLP 1072 §7.1).
    func tickAsync(now: Double, done: @escaping (Batch) -> Void) {
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            done(read(exact_tick(rt, now)))
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
    /// `pointerdown` (29), `pointerup` (30) or `pointermove` (31) with its
    /// record (LLP 1005 §3, LLP 1056 §3 stage 3).
    func pointer(_ view: UInt32, _ kind: PointerKind, _ sample: PointerSample, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, kind.rawValue, write(sample.line), now)) }
    }
    /// The clipboard's `copy` (32), `cut` (33) or `paste` (34) with its
    /// plain text; a `contextmenu` (10) with its pointer line, a `wheel`
    /// (37) or a `drop` (38) with theirs (`Event::of_host_kind`).
    func clipboard(_ view: UInt32, _ kind: UInt32, _ text: String, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, kind, write(text), now)) }
    }
    /// `selectionchange` (35): a `text`'s part of the selection, its source
    /// UTF-16 offsets and then its text.
    func selectionChange(_ view: UInt32, _ text: String, start: Int, end: Int, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, 35, write("\(start),\(end),\(text)"), now)) }
    }
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
    /// Whether `location` names one of the app's declared routes (LLP 1038 §7).
    func routeMatches(_ location: String) -> Bool {
        return on(busy: false) { exact_route_matches(rt, write(location)) != 0 }
    }
    func launch(_ location: String) { on { () -> Void in let n = write(location); _ = exact_set_launch_location(rt, n) } }
    func navigate(_ view: UInt32, _ location: String, now: Double) -> Batch {
        return on {
            let n = write(location)
            return read(exact_dispatch(rt, view, 14, n, now))
        }
    }
    func advance(now: Double, untilRequest: Bool = false) -> Batch { on { read(exact_advance(rt, now, untilRequest ? 1 : 0)) } }
    /// The `then`s an agent's input settled, the clock unmoved (LLP 1012 §2).
    func landThen() -> Batch { on { read(exact_advance(rt, 0, 2)) } }
    func frame(now: Double) -> Batch { on { read(exact_frame(rt, now)) } }
    func presentFrames(_ yes: Bool) { on { () -> Void in _ = exact_present_frames(rt, yes ? 1 : 0) } }
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
    /// The posture and the viewport segments (LLP 1078 D4): the rects as `x y w h` floats in the input buffer, none for one segment.
    func segments(_ fold: ViewportFold) -> Batch {
        on {
            var data = Data(capacity: fold.rects.count * 16)
            for r in fold.rects { for v in [Float(r.minX), Float(r.minY), Float(r.width), Float(r.height)] { withUnsafeBytes(of: v.bitPattern.littleEndian) { data.append(contentsOf: $0) } } }
            _ = write(data)
            return read(exact_segments(rt, fold.posture == "folded" ? 1 : 0, UInt32(fold.cols), UInt32(fold.rows), UInt32(fold.rects.count)))
        }
    }
    func tick(now: Double) -> Batch { on { read(exact_tick(rt, now)) } }
    func scheme(dark: Bool) -> Batch { on { read(exact_scheme(rt, dark ? 1 : 0)) } }
    func viewScheme(_ view: UInt32, dark: Bool) -> Batch { on { read(exact_view_scheme(rt, view, dark ? 1 : 0)) } }
    /// @ref LLP 1095 D1 — every colour reference the kernel resolves itself
    /// (paint motion, gradients, SVG scenes), resolved by the platform in
    /// both appearances under the current contrast and reported, so none of
    /// them is a frozen fallback. A name the platform lacks is left out: the
    /// kernel keeps its fallback pair. `tint` is the app's tint,
    /// light then dark (`SystemColor.tintPair`), read on main: the owner
    /// thread has no window to read it from; without one `@tint` is not
    /// reported, and the kernel keeps what it had.
    func reportColors(tint: [[Double]]?) -> Batch {
        on {
            let len = exact_color_references(rt)
            let json = Data(bytes: exact_out(rt), count: Int(len))
            let refs = (try? JSONSerialization.jsonObject(with: json)) as? [[Any]] ?? []
            var bytes = Data(capacity: refs.count * 16)
            for r in refs {
                guard r.count == 3, let kind = r[0] as? Int, let id = r[1] as? Int, let name = r[2] as? String,
                      tint != nil || !name.hasPrefix("@tint") else { continue }
                for dark in [false, true] {
                    guard let c = SystemColor.channels(name, dark: dark, tint: tint?[dark ? 1 : 0], fallback: nil), c.count == 4 else { continue }
                    bytes.append(UInt8(kind)); bytes.append(dark ? 1 : 0)
                    withUnsafeBytes(of: UInt16(id).littleEndian) { bytes.append(contentsOf: $0) }
                    for v in c { bytes.append(UInt8(max(0, min(255, v.rounded())))) }
                }
            }
            return read(exact_colors(rt, write(bytes)))
        }
    }
    /// A button's face, custom or native (LLP 1069.011.000 D1).
    func buttonFace(_ view: UInt32) -> ButtonFace {
        return on(busy: ButtonFace()) {
            let len = exact_press_face(rt, view)
            return ButtonFace(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
    }
    #if os(iOS)
    /// A grouped list's sections and rows (LLP 1084 D4).
    func groupedList(_ view: UInt32) -> GroupedListModel? {
        return on(busy: nil) {
            let len = exact_grouped_list(rt, view)
            return GroupedListModel(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
    }
    #endif
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
    /// What native containers cover of boxes (LLP 1075.003 §3.5), under one
    /// layout: `edges` add to a box's padding; `whole` lays it out as
    /// `display: none`; nil clears.
    func covers(_ covers: [(UInt32, HostCover?)]) -> Batch {
        return on {
            var bytes = Data(capacity: covers.count * 24)
            for (view, cover) in covers {
                let (kind, edges): (UInt32, [CGFloat]) = switch cover {
                case .none: (0, [0, 0, 0, 0])
                case .edges(let e)?: (1, [e.top, e.right, e.bottom, e.left])
                case .whole?: (2, [0, 0, 0, 0])
                }
                for word in [view, kind] + edges.map({ Float($0).bitPattern }) {
                    withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) }
                }
            }
            return read(exact_host_covers(rt, write(bytes)))
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
        let rt = rt
        Owner.shared.notify {
            topic.withUnsafeBytes { exact_app_changed(rt, $0.bindMemory(to: UInt8.self).baseAddress, topic.count) }
        }
    }
    /// A notification (LLP 1072 T5): queued to the owner, never waited for,
    /// so a line main journals mid-apply does not wait behind a slice being
    /// built. A later call runs after it; a destroyed runtime refuses it.
    func log(_ line: String) {
        Owner.shared.notify { [self] in _ = exact_log(rt, write(line)) }
    }
}

/// The pointer's three events and their ABI kinds (LLP 1005 §3).
enum PointerKind: UInt32 { case down = 29, up = 30, move = 31 }

/// DOM's `PointerEvent`, the subset the runner's record carries (LLP 1056 §3
/// stage 3): the point from the node's content box in its own points, DOM's
/// button bits (AppKit's `pressedMouseButtons` uses the same), 0 to 1 of
/// pressure (DOM's 0.5 while pressed where nothing measures it), the device
/// (`mouse`, `pen`, `touch`), its id (the mouse is 1, as browsers number it),
/// and the point from the viewport, `frame()`'s space (LLP 1094 D11).
struct PointerSample {
    var x: Double, y: Double, buttons: Int, pressure: Double, type: String, id: Int
    var clientX: Double, clientY: Double
    /// The modifiers held, a chord prefix (`KeyCodes.held`): a `MouseEvent`'s.
    var held = ""
    var line: String { "\(x),\(y),\(buttons),\(min(1, max(0, pressure))),\(type),\(id),\(clientX),\(clientY),\(held)" }
}

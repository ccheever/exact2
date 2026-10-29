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
import QuartzCore

/// One batch from the library: the ops, and whether the presenter should keep
/// the clock (timers) or the display link (motion) running.
public struct Batch {
    public package(set) var ops: [BatchOp]
    public let timers: Bool
    public let motion: Bool
    /// The runner's clock after the call, milliseconds (LLP 1012 `clock`).
    public let clock: Double?
    public let error: String?
    /// Re-entry reached no runner turn; the presenter keeps its scheduling state.
    public let busy: Bool
    /// Held over a restart's reset (`Runtime.held`): reached no runtime, and nothing applies it.
    public var held = false
    /// @ref LLP 1043.000 §3 D8 — absolute runner deadline, absent without timers.
    public var timerDueMs: Double? = nil
    /// A data module's timer continuation an advance released is still out: the agent's jump waits for it.
    public var landing = false
    /// Store refresh acknowledged no runner commit; keep the current timers.
    public var unchanged = false
    public var storeReadPending = false
    public var storeActivationPending = false
    public var pending = false
    /// A module generation refused while requests are in flight: ask again (LLP 1007 §6).
    public var requests = false
    /// What moves changes place or size: the panel's full rate (LLP 1061 D4).
    public var spatial = false
    /// A 2D canvas wants the next display frame (LLP 1056 D5).
    public var canvas = false
    /// A frame task wants every display frame (LLP 1073 D5).
    public var frames = false
    /// A canvas draw is owed to a turn of its own (LLP 1072 §8.5).
    public var canvasOwed = false
    /// A control's viewless contents changed (LLP 1069.011 §9).
    public var layoutProvisional = false
    public var controls = false
    /// Image handles a 2D canvas asked for, to decode (LLP 1056 D9).
    public var canvasImages: [String] = []
    /// The window's app modes after a `setMode` (LLP 1121 D5), when the call was one.
    public var modes: [String: String]? = nil
    /// After a live design batch (LLP 1121 stage 4): applied, stale,
    /// structure or refused; and whether a colour met a full wide table.
    public var live: String? = nil
    public var unsupported = false
    /// The design digest the runner runs after it (the build channel's `design`, LLP 1122 D4).
    public var design: String? = nil
    /// The root font size or the window's modes moved, the app's `setMode` and `setRootFontSize` included (LLP 1122 D4).
    public var facts = false
    /// Whether a displayed element holds the wake lock, when that changed (LLP 1125 D11).
    public var wakelock: Bool? = nil
    /// The session's orientation lock, when it changed (LLP 1133.029 D3).
    public var orientation: OrientationRequest? = nil
    /// The kernel transactions it carries, while the runner measures (LLP 1079 D3).
    public var seq: (UInt64, UInt64)?
    /// When `ExactSession.applyInput` marked it as a person's input: on iOS
    /// a glass it adds to a group in the window materializes.
    public var input: Double? = nil
    /// Failure lines the session writes to the unified log, each once (LLP 1142 D4).
    public var alerts: [String] = []
    /// The journal's other lines, for the unified log at a lower level (LLP 1143).
    public var journal: [String] = []
    package var frameWorkTiming: FrameWorkTiming?
    package init(ops: [BatchOp], timers: Bool, motion: Bool, clock: Double?, error: String?, timerDueMs: Double? = nil, pending: Bool = false, busy: Bool = false) {
        self.ops = ops; self.timers = timers; self.motion = motion; self.clock = clock
        self.error = error; self.timerDueMs = timerDueMs; self.pending = pending
        self.busy = busy
    }
    package static func decodeWire(_ data: Data, resolver: AssetResolver? = nil, colorChannels: ((BatchValue, Bool) -> [Double]?)? = nil) -> Batch {
        data.withUnsafeBytes { decodeWire($0.bindMemory(to: UInt8.self), resolver: resolver, colorChannels: colorChannels) }
    }
    package static func decodeWire(_ bytes: UnsafeBufferPointer<UInt8>, resolver: AssetResolver? = nil, colorChannels: ((BatchValue, Bool) -> [Double]?)? = nil) -> Batch {
        var reader = BatchReader(bytes: bytes, colorChannels: colorChannels, profileResolver: resolver)
        do {
            let batch = try reader.batch()
            try reader.end()
            return batch
        } catch {
            return Batch(ops: [], timers: false, motion: false, clock: nil, error: "unreadable batch")
        }
    }

}

/// Immutable composition services, installed before the runtime can serve a turn.
package struct RuntimeHooks {
    package let decodeBatch: (UnsafeBufferPointer<UInt8>, AssetResolver?) -> Batch
    package let prepareBatch: ((inout Batch) -> Void)?
    package let symbolMeasure: ExactSymbolFn?
    package let prewarmIslands: ((UInt8) -> Void)?
    package init(decodeBatch: @escaping (UnsafeBufferPointer<UInt8>, AssetResolver?) -> Batch,
                 prepareBatch: ((inout Batch) -> Void)? = nil, symbolMeasure: ExactSymbolFn? = nil,
                 prewarmIslands: ((UInt8) -> Void)? = nil) {
        self.decodeBatch = decodeBatch; self.prepareBatch = prepareBatch
        self.symbolMeasure = symbolMeasure; self.prewarmIslands = prewarmIslands
    }
    package static let headless = RuntimeHooks(decodeBatch: { Batch.decodeWire($0, resolver: $1) }, symbolMeasure: SymbolMeasure.measure)
}

/// A top-layer view, its viewport frame and optional viewport clip (LLP 1123 D2).
/// Clipping bounds reads without changing the box's mapping.
package typealias TopLayerFrame = (UInt32, CGRect, CGRect?)

/// A runtime handle and its calls. `destroy` is idempotent at this layer and
/// one-shot at the C boundary; a call after it is refused by the library by
/// name, never a trap.
package final class Runtime {
    package let rt: ExactRuntime
    private(set) package var destroyed = false
    package var topLayer: (() -> (frames: [TopLayerFrame], modal: Int?))?
    package var inlineFrame: ((UInt32) -> CGRect?)?
    package var rangeFrame: ((UInt32, UInt32, UInt32) -> CGRect?)?
    #if DEBUG
    // Per-runtime observation for differential tests of actual session traffic.
    // Release builds have neither the callback nor a copy of the wire bytes.
    package var observeBatch: ((Data, Batch) -> Void)?
    #endif

    private let hooks: RuntimeHooks
    package init(hooks: RuntimeHooks = .headless) {
        self.hooks = hooks
        rt = Owner.shared.sync { exact_create() }
    }

    deinit { destroy() }

    package func destroy() {
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
    package static let busy = Batch(ops: [], timers: false, motion: false, clock: nil,
        error: "busy: the owner is waiting on this callback (LLP 1072 T5)", busy: true)
    package static let busyAgent = "{\"error\":\"busy: the owner is waiting on this callback (LLP 1072 T5)\"}"
    /// While a restart's reset tears the old tree's views down (Session `presentCommitted`): the one entry every
    /// batch-answering call takes, so this runtime, which replaces that tree, takes none of them before its committed
    /// batch lands. Each answers an empty batch, the runtime untouched, and is named in `heldCalls` (its method, by
    /// `#function`), which ExactKit's ResetGate drops or replays once the batch is in.
    package var held = false
    package private(set) var heldCalls: [String] = []
    /// The batch-answering calls `on` let through on main: a test's spy on what reached the runtime.
    package private(set) var entered = 0
    package static let heldBatch = { var batch = Batch(ops: [], timers: false, motion: false, clock: nil, error: nil); batch.held = true; return batch }()
    package func takeHeldCalls() -> [String] { defer { heldCalls = [] }; return heldCalls }
    /// Whether held; if so, `caller` is named in `heldCalls` (a call held outside `on`: `selection`, a notice's `later`).
    package func holds(_ caller: String = #function) -> Bool { if held { heldCalls.append(caller) }; return held }
    package func on(_ body: () -> Batch, caller: String = #function) -> Batch {
        if holds(caller) { return Runtime.heldBatch }
        if Thread.isMainThread { entered &+= 1 } // (`later` calls on the owner: the spy counts main's)
        let (open, modal) = topLayer?() ?? ([], nil)
        return Owner.shared.sync({
            installTopLayer(open, modal: modal)
            return body()
        }, busy: Runtime.busy)
    }
    private func installTopLayer(_ open: [TopLayerFrame], modal: Int?) {
        let ids = open.map(\.0)
        let rects = open.flatMap { [Double($0.1.minX), Double($0.1.minY), Double($0.1.width), Double($0.1.height)] }
        let clips = open.flatMap { entry -> [Double] in
            guard let clip = entry.2 else { return [.nan, 0, 0, 0] }
            return [Double(clip.minX), Double(clip.minY), Double(clip.width), Double(clip.height)]
        }
        ids.withUnsafeBufferPointer { ids in
            rects.withUnsafeBufferPointer { rects in
                clips.withUnsafeBufferPointer { clips in
                    exact_top_layer(rt, ids.baseAddress, rects.baseAddress, clips.baseAddress, UInt32(open.count), modal.map(UInt32.init) ?? .max)
                }
            }
        }
    }
    package func on<T>(busy: @autoclosure () -> T, _ body: () -> T) -> T { Owner.shared.sync(body, busy: busy()) }
    package func on(_ body: () -> Void) { Owner.shared.sync(body, busy: ()) }

    /// The text measurer (`TextEngine.measure`) and its context.
    package func setMeasure(_ measure: ExactMeasureFn?, ctx: UnsafeMutableRawPointer?) {
        let ctx = UInt(bitPattern: ctx)
        // Its paragraphs' line boxes too, for multi-column flows (LLP 1093 D6).
        on { () -> Void in
            exact_set_measure(rt, measure, UnsafeMutableRawPointer(bitPattern: ctx))
            exact_set_time_format(rt, TimeFormatting.format)
            exact_set_lines(rt, measure == nil ? nil : TextEngine.linesText)
            exact_set_inline_boxes(rt, measure == nil ? nil : TextEngine.inlineBoxesText)
            // A system symbol's box, measured before layout as a paragraph's is
            exact_set_symbol_measure(rt, measure == nil ? nil : hooks.symbolMeasure)
        }
    }
    /// The plan-font hook a boot calls synchronously, with its context.
    package func setFonts(_ fonts: ExactFontsFn?, ctx: UnsafeMutableRawPointer?) {
        let ctx = UInt(bitPattern: ctx)
        on { exact_set_fonts(rt, fonts, UnsafeMutableRawPointer(bitPattern: ctx)) }
    }
    /// The host's clock in the runner's milliseconds, or nil for the seekable one that
    /// tests and the agent move (the timers RFC, D1); later boots take it too.
    package func setLiveClock(_ wall: (@convention(c) () -> Double)?) {
        on { () -> Void in _ = exact_set_live_clock(rt, wall) }
    }
    /// The wake for a request's reply (LLP 1016 D2), on the executor's thread.
    package func setWake(_ wake: ExactWakeFn?, ctx: UnsafeMutableRawPointer?) {
        let ctx = UInt(bitPattern: ctx)
        on { exact_set_wake(rt, wake, UnsafeMutableRawPointer(bitPattern: ctx)) }
    }
    /// This binary's baked `compat.json`, read on a throwaway runtime.
    package static func bakedCompat() -> Data {
        let runtime = Runtime(hooks: .headless)
        defer { runtime.destroy() }
        return runtime.on(busy: Data()) {
            let length = exact_baked_compat(runtime.rt)
            return Data(bytes: exact_out(runtime.rt), count: Int(length))
        }
    }

    private var profileResolver: AssetResolver?
    package func setProfileResolver(_ resolver: AssetResolver?) { on { profileResolver = resolver } }

    package func read(_ len: UInt32) -> Batch {
        // The runtime owns these bytes until its next call. The reader copies
        // strings into Swift values before returning; no batch borrows the buffer.
        let bytes = UnsafeBufferPointer(start: exact_out(rt), count: Int(len))
        var batch = hooks.decodeBatch(bytes, profileResolver)
        #if DEBUG
        // What was decoded, before `prepare` adds the owner's values.
        observeBatch?(Data(bytes), batch)
        #endif
        hooks.prepareBatch?(&batch)
        return batch
    }
    /// A payload into the runtime's input buffer; its length. An empty
    /// payload clears the buffer without dereferencing anything.
    package func write(_ text: String) -> Int { write(Data(text.utf8)) }
    package func write(_ data: Data) -> Int {
        guard !data.isEmpty, let p = exact_in(rt, data.count) else { _ = exact_in(rt, 0); return 0 }
        data.withUnsafeBytes { src in if let base = src.baseAddress { p.update(from: base.assumingMemoryBound(to: UInt8.self), count: data.count) } }
        return data.count
    }
    /// Boot the plan baked into the library under a viewport; the first batch.
    /// The display's scale and memory for Canvas 2D (LLP 1056 D4).
    package func canvasDisplay(scale: CGFloat, memory: UInt64) -> Batch { on { read(exact_canvas_display(rt, Double(scale), Double(memory))) } }
    /// A Canvas 2D image handle decoded (its size in pixels) or not (LLP 1056 D9).
    package func canvasImage(_ src: String, width: Int, height: Int, ok: Bool) -> Batch {
        return on {
            let n = write(src)
            return read(exact_canvas_image(rt, n, UInt32(max(0, width)), UInt32(max(0, height)), ok ? 1 : 0))
        }
    }
    /// A 2D canvas's replay is behind or caught up (LLP 1056 D5).
    package func canvasHeld(_ view: UInt32, _ held: Bool) { on { exact_canvas_held(rt, view, held ? 1 : 0) } }
    /// The Canvas 2D text measurer (LLP 1056 D8), with the measurer's context.
    package func setCanvasText(_ measure: ExactCanvasTextFn?) { on { exact_set_canvas_text(rt, measure) } }
    package func boot(width: CGFloat, height: CGFloat) -> Batch { islands(on { read(exact_boot(rt, Float(width), Float(height))) }) }
    /// A plan that can show an SVG island opens the island module off the
    /// main thread now, before its first mask or filter needs it (LLP
    /// 1055.000 §8 ruling 4); a plan without one never loads it. One that
    /// can show a filter makes the GPU filter pipelines now too: a first
    /// picture is drawn in the commit that shows it.
    private func islands(_ batch: Batch) -> Batch {
        let svg = on(busy: UInt8(0)) { exact_svg_islands(rt) }
        hooks.prewarmIslands?(svg)
        return batch
    }
    /// Boot from plan bytes (the dev loop's restart; LLP 1007 §6): state
    /// carried — transactional, so a refused candidate leaves the running
    /// app exactly as it was.
    package func bootPlan(_ bytes: Data, width: CGFloat, height: CGFloat) -> Batch {
        let batch = on { () -> Batch in
            let n = write(bytes)
            return read(exact_boot_plan(rt, n, Float(width), Float(height)))
        }
        return islands(batch)
    }
    /// Boot a snapshot entry's plan by its kind (LLP 1126 D2): the headless
    /// session's boot, which a running host's `bootPlan` refuses.
    package func bootEntry(_ bytes: Data, kind: String, props: Data? = nil, width: CGFloat, height: CGFloat) -> Batch {
        let batch = on { () -> Batch in
            let name = Data(kind.utf8)
            var payload = bytes; payload.append(name); payload.append(props ?? Data())
            _ = write(payload)
            return read(exact_boot_entry(rt, bytes.count, name.count, props?.count ?? 0, Float(width), Float(height)))
        }
        return islands(batch)
    }
    package func previewTimeline(at now: Double) -> Data {
        on(busy: Data()) {
            let count = exact_refresh_entry(rt, now)
            return Data(bytes: exact_out(rt), count: Int(count))
        }
    }
    package func entryTimeline(at now: Double, deadline: DispatchTime, receipt: Data = Data(), module: Data = Data()) -> Data {
        on(busy: Data()) {
            let current = DispatchTime.now().uptimeNanoseconds
            let remaining = Double(deadline.uptimeNanoseconds > current ? deadline.uptimeNanoseconds - current : 0) / 1_000_000
            return receipt.withUnsafeBytes { receipt in module.withUnsafeBytes { module in
                let count = exact_entry_timeline(rt, now, remaining, receipt.bindMemory(to: UInt8.self).baseAddress, receipt.count, module.bindMemory(to: UInt8.self).baseAddress, module.count)
                return Data(bytes: exact_out(rt), count: Int(count))
            } }
        }
    }
    package func preparePlan(_ bytes: Data, width: CGFloat, height: CGFloat, token: UInt64 = 0) -> Batch {
        return on {
            let n = write(bytes)
            return read(exact_prepare_plan(rt, token, n, Float(width), Float(height)))
        }
    }
    package func commitPlan() -> Batch { islands(on { read(exact_commit_plan(rt)) }) }
    /// The owner excludes replies between the final freshness check and every
    /// commit. The durable callback runs on main through the usual T5 door.
    package static func commitTogether(_ runtimes: [Runtime], commit: () -> Bool) -> (batches: [Batch]?, pending: Bool, requests: Bool) {
        let result: (batches: [Batch]?, pending: Bool, requests: Bool) = Owner.shared.sync({
            let checks = runtimes.map { $0.read(exact_check_prepared($0.rt)) }
            guard checks.allSatisfy({ $0.error == nil && !$0.pending }) else {
                return (nil, checks.contains { $0.pending || $0.requests }, checks.contains { $0.requests })
            }
            guard Owner.shared.callMain(commit) else { return (nil, false, false) }
            return (runtimes.map { $0.read(exact_commit_plan($0.rt)) }, false, false)
        }, busy: (nil, false, false))
        guard let batches = result.batches else { return result }
        return (zip(runtimes, batches).map { $0.islands($1) }, false, false)
    }
    /// `letGo`: the dev loop's, past its settle window: requests in flight do not refuse it (LLP 1007 §6).
    package func prepareModule(_ plan: Data, module: ExactModule, token: UInt64 = 0, width: CGFloat, height: CGFloat, letGo: Bool = false) -> Batch {
        return on {
            var payload = plan
            payload.append(module.receipt)
            payload.append(module.bytecode)
            _ = write(payload)
            return read(exact_prepare_module(rt, token, plan.count, module.receipt.count, module.bytecode.count, Float(width), Float(height), letGo ? 1 : 0))
        }
    }
    /// Activates the data source without committing (`exact_prepare_data`); run it on the owner.
    package func prepareData() { on { exact_prepare_data(rt) } }
    package func dataReady() -> Batch { on { read(exact_data_ready(rt)) } }
    package func storeChanged(retryOnly: Bool = false) -> Batch { on { read(exact_store_changed(rt, retryOnly ? 1 : 0)) } }
    package func discardPlan() { on { exact_discard_plan(rt) } }
    package func trim() { on { exact_trim(rt) } }

    /// Every queued reply into the runner: the batch of their commits.
    package func pump(now: Double) -> Batch { on { read(exact_pump(rt, now)) } }
    /// Capture native presentation on the caller before parsing replies on the owner.
    package func pumpAsync(now: Double, done: @escaping (Batch?) -> Void) {
        if holds() { Owner.shared.post { done(nil) }; return } // (held: answered on the owner as a cancelled reply, `held`)
        let (open, modal) = topLayer?() ?? ([], nil)
        Owner.shared.post { [self] in
            guard !destroyed else { done(nil); return }
            installTopLayer(open, modal: modal)
            done(read(exact_pump(rt, now)))
        }
    }
    package func requestActive(_ ticket: UInt64) -> Bool { on(busy: false) { exact_request_active(rt, ticket) != 0 } }
    package func fulfillSurface(_ ticket: UInt64, kind: UInt32, body: Data = Data(), now: Double) -> Batch {
        return on {
            let n = write(body)
            return read(exact_fulfill_surface(rt, ticket, kind, n, now))
        }
    }
    /// A press, with the modifiers held as a chord prefix (`KeyCodes.held`),
    /// after a pointer's point from the viewport where it had one: its
    /// `MouseEvent`'s `clientX`/`clientY` (LLP 1021 D2).
    package func press(_ view: UInt32, held: String = "", at client: CGPoint? = nil, now: Double) -> Batch {
        let line = client.map { "\(Double($0.x)),\(Double($0.y)),\(held)" } ?? held
        return on { read(exact_dispatch(rt, view, 0, write(line), now)) }
    }
    /// The pointer over the view (`true`) or gone from it.
    package func hover(_ view: UInt32, over: Bool, now: Double) -> Batch { on { read(exact_dispatch(rt, view, over ? 2 : 3, 0, now)) } }
    package func focus(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 4, 0, now)) } }
    package func blur(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 5, 0, now)) } }
    package func focusEvent(_ view: UInt32, entering: Bool, target: String, related: String?, now: Double) -> Batch {
        let payload = "\(target.utf8.count):\(target)" + (related.map { "1\($0)" } ?? "0")
        return on { read(exact_dispatch(rt, view, entering ? 46 : 47, write(payload), now)) }
    }
    package func contextmenu(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 10, 0, now)) } }
    /// DOM's `beforeunload` (36) at a node that hears it (studio diary R17).
    package func beforeunload(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 36, 0, now)) } }

    package func heightDragUpdate(_ token: UInt64, height: Double, now: Double) -> Batch {
        return on {
            read(exact_height_drag_update(rt, token, height, now))
        }
    }
    package func heightDragRelease(_ token: UInt64, height: Double, now: Double) -> Batch {
        return on {
            read(exact_height_drag_release(rt, token, height, now))
        }
    }
    package func reorderBegin(_ handle: UInt32, scrollTop: Double, now: Double) -> Batch {
        return on {
            read(exact_reorder_begin(rt, handle, scrollTop, now))
        }
    }
    package func reorderMove(_ token: UInt64, dy: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch {
        return on {
            read(exact_reorder_move(rt, token, dy, scrollTop, inside ? 1 : 0, now))
        }
    }
    package func reorderEnd(_ token: UInt64, drop: Bool, dy: Double, scrollTop: Double, inside: Bool, velocity: Double, now: Double) -> Batch {
        return on {
            read(exact_reorder_end(rt, token, drop ? 1 : 0, dy, scrollTop, inside ? 1 : 0, velocity, now))
        }
    }
    package func hasHold(_ token: UInt64) -> Bool { on(busy: false) { !destroyed && exact_has_hold(rt, token) != 0 } }
    package func holdUpdate(_ token: UInt64, x: Double, y: Double, now: Double) -> Batch {
        return on {
            read(exact_hold_update(rt, token, x, y, now))
        }
    }
    /// `measured`: release at the engine's own velocity estimate (LLP 1057.001 §3).
    package func holdEnd(_ token: UInt64, cancel: Bool, measured: Bool = false, vx: Double = 0, vy: Double = 0, now: Double) -> Batch {
        return on {
            read(exact_hold_end(rt, token, cancel ? 1 : measured ? 2 : 0, vx, vy, now))
        }
    }
    package func swiperight(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 12, 0, now)) } }
    package func refresh(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 22, 0, now)) } }
    package func pan(_ view: UInt32, dx: Double, dy: Double, now: Double) -> Batch {
        return on {
            read(exact_dispatch(rt, view, 20, write("\(dx),\(dy)"), now))
        }
    }
    /// A pan that began ended at (vx, vy) viewport px/s (LLP 1057 §10.6).
    package func panRelease(_ view: UInt32, vx: Double, vy: Double, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, 28, write("\(vx),\(vy)"), now)) }
    }
    /// The pan contact's samples, where the platform measures no velocity:
    /// the engine's tracker (LLP 1057.001 §3), viewport px at `t` seconds.
    package func panSample(first: Bool, x: Double, y: Double, t: Double) { on { () -> Void in _ = exact_pan_sample(rt, first ? 1 : 0, x, y, t) } }
    package func panVelocity(at t: Double) -> (Double, Double) { on(busy: (0, 0)) { (exact_pan_velocity(rt, 0, t), exact_pan_velocity(rt, 1, t)) } }
    /// A scroller (nil: the page) now stands at `left`, `top` (LLP 1051.000
    /// D1): posted, never waited for, since AppKit calls from inside its
    /// scroll synchronizer.
    package func scrolled(_ view: UInt32?, left: Double, top: Double) {
        if holds() { return }
        Owner.shared.notify { [self] in
            guard !destroyed else { return }
            exact_scrolled(rt, view == nil ? 1 : 0, view ?? 0, left, top)
        }
    }
    /// A scroll event: left, top, then the scroller's `scrollWidth`,
    /// `scrollHeight`, `clientWidth` and `clientHeight` (`ScrollEvent`, chat F4).
    package func scroll(_ view: UInt32, metrics: [Double], now: Double) -> Batch {
        return on {
            let n = write(metrics.map { "\($0)" }.joined(separator: ","))
            return read(exact_dispatch(rt, view, 13, n, now))
        }
    }
    /// A scroll finished (DOM's `scrollend`), after its last `scroll`.
    package func scrollend(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 51, 0, now)) } }
    /// The agent's `tap <list> into <key>` (LLP 1070.000 §5).
    package func intoView(_ view: UInt32, key: String, block: String, inline: String) -> Batch {
        return on {
            let n = write(key + "\n" + block + "\n" + inline)
            return read(exact_into_view(rt, view, n))
        }
    }
    /// What UIKit covers of list `view`'s port at its start and end, on the
    /// owner before the reports after it, not waited for.
    package func listCovered(_ view: UInt32, start: Double, end: Double) {
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            exact_list_covered(rt, view, start, end)
        }
    }
    /// A build-only report on the owner, not waited for (LLP 1072 §3):
    /// `done` runs on the owner with the decoded batch.
    package func collectionFeedbackAsync(_ bytes: Data, now: Double, done: @escaping (Batch) -> Void) {
        if holds() { Owner.shared.post { done(Runtime.heldBatch) }; return } // (held: answered on the owner, so what waits on it ends)
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            let n = write(bytes)
            done(read(exact_collection_feedback(rt, n, now)))
        }
    }
    /// Count each plan site's work for `perf` from now, if not yet (LLP 1079 D1).
    package func measureWork() { on { _ = exact_measure_work(rt) } }
    /// Canvas draws in a turn of their own (LLP 1072 §8.5).
    package func canvasDefer(_ deferred: Bool) { on { exact_canvas_defer(rt, deferred ? 1 : 0) } }
    /// The owed canvas draws on the owner, not waited for (LLP 1072 §8.5).
    package func canvasDrawAsync(done: @escaping (Batch) -> Void) {
        if holds() { Owner.shared.post { done(Runtime.heldBatch) }; return }
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            done(read(exact_canvas_draw(rt)))
        }
    }
    /// A frame's tick on the owner, not waited for (LLP 1072 §7.1); `frame`
    /// is the display frame it is for (LLP 1003.001 D5).
    /// `calls`, in order, on the owner and not waited for: each batch goes to
    /// `done` there (LLP 1027.007 P1).
    package func later(_ calls: [(Runtime) -> Batch], done: @escaping (Batch) -> Void) {
        if holds() { Owner.shared.post { for _ in calls { done(Runtime.heldBatch) } }; return }
        Owner.shared.post { [self] in
            for call in calls {
                guard !destroyed else { return }
                done(call(self))
            }
        }
    }
    package func tickAsync(now: Double, frame: Double? = nil, capture: UInt64 = 0, measureFrame: Bool = false, done: @escaping (Batch) -> Void) {
        if holds() { Owner.shared.post { done(Runtime.heldBatch) }; return }
        let queued = capture == 0 ? 0 : CACurrentMediaTime()
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            let began = capture == 0 ? 0 : CACurrentMediaTime()
            var batch = read(frame.map { exact_tick_at(rt, now, $0) } ?? exact_tick(rt, now))
            if measureFrame || capture != 0 { batch.frameWorkTiming = FrameWorkTiming(capture: capture, frame: frame,
                queued: queued, began: began, finished: capture == 0 ? 0 : CACurrentMediaTime()) }
            done(batch)
        }
    }
    /// Actual viewport/row observations using the runner's versioned LE wire.
    package func collectionFeedback(_ bytes: Data, now: Double) -> Batch {
        return on {
            let n = write(bytes)
            return read(exact_collection_feedback(rt, n, now))
        }
    }
    package func dblclick(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 11, 0, now)) } }
    /// `pointerdown` (29), `pointerup` (30) or `pointermove` (31) with its
    /// record (LLP 1005 §3, LLP 1056 §3 stage 3).
    package func pointer(_ view: UInt32, _ kind: PointerKind, _ sample: PointerSample, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, kind.rawValue, write(sample.line), now)) }
    }
    /// The clipboard's `copy` (32), `cut` (33) or `paste` (34) with its
    /// plain text; a `contextmenu` (10) with its pointer line, a `wheel`
    /// (37), a `drop` (38), a file drag's `dragenter` (55) or `dragleave` (56), a
    /// paste with files (57), or a popover `toggle` (50) with theirs (`Event::of_host_kind`).
    package func clipboard(_ view: UInt32, _ kind: UInt32, _ text: String, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, kind, write(text), now)) }
    }
    /// `selectionchange` (35): a `text`'s part of the selection, its source
    /// UTF-16 offsets, rectangle count and coordinates, then its text.
    package func selectionChange(_ view: UInt32, _ text: String, start: Int, end: Int, rects: [CGRect], now: Double) -> Batch {
        let boxes = rects.flatMap { [$0.minX, $0.minY, $0.width, $0.height].map { String(Double($0)) } }
        let payload = ([String(start), String(end), String(rects.count)] + boxes + [text]).joined(separator: ",")
        return on { read(exact_dispatch(rt, view, 35, write(payload), now)) }
    }
    package func submit(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 7, 0, now)) } }
    package func media(_ view: UInt32, event: String, payload: String, now: Double) -> Batch {
        return on {
            let n = write(event + "\n" + payload)
            return read(exact_dispatch(rt, view, 19, n, now))
        }
    }
    package func load(_ view: UInt32, now: Double) -> Batch { on { read(exact_dispatch(rt, view, 8, 0, now)) } }
    /// An iframe's `navigate`, `currententrychange`, `navigatesuccess` or
    /// `navigateerror` (58) with its `FrameNavigation` record as JSON (@ref
    /// LLP 1020.002 D2), or its `newwindow` or `download` with theirs (D5).
    package func frameNavigation(_ view: UInt32, event: String, record: String, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, 58, write(event + "\n" + record), now)) }
    }
    package func surfaceRecord(_ name: String, _ json: String?) -> Batch {
        return on {
            let n = write(name + (json.map { "\0" + $0 } ?? ""))
            return read(exact_surface_record(rt, n))
        }
    }
    package func message(_ view: UInt32, _ value: String, now: Double) -> Batch {
        return on {
            let n = write(value)
            return read(exact_dispatch(rt, view, 9, n, now))
        }
    }
    /// A key down (kind 6), up (43) or capture (44, #140) at the view: its chord by the
    /// web's key name, then its code and repeat (`KeyPress.payload`).
    package func key(_ view: UInt32, _ payload: String, up: Bool = false, capture: Bool = false, now: Double) -> Batch {
        return on {
            let n = write(payload)
            return read(exact_dispatch(rt, view, capture ? 44 : up ? 43 : 6, n, now))
        }
    }

    package func fieldEvent(_ view: UInt32, _ kind: UInt32, _ payload: String, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, kind, write(payload), now)) }
    }
    /// A checkbox's state: `change` when `commit`, else `input`.
    package func checked(_ view: UInt32, _ checked: Bool, commit: Bool, now: Double) -> Batch {
        return on {
            let n = write(checked ? "true" : "false")
            return read(exact_dispatch(rt, view, commit ? 24 : 25, n, now))
        }
    }
    /// A file input's picked files, one tab-separated line each (LLP 1069.002 D3).
    package func picked(_ view: UInt32, _ payload: String, now: Double) -> Batch {
        return on {
            let n = write(payload)
            return read(exact_dispatch(rt, view, 26, n, now))
        }
    }
    /// A dialog closed after restoring its focus (LLP 1021).
    package func dialogClose(_ view: UInt32, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, 49, write(""), now)) }
    }
    /// A file input's picker was dismissed: HTML's `cancel` (LLP 1069.002 D2).
    package func pickerCancel(_ view: UInt32, now: Double) -> Batch {
        return on {
            let n = write("")
            return read(exact_dispatch(rt, view, 27, n, now))
        }
    }
    /// Shared Markdown selection facts; the editor retains its own range.
    package func selection(_ view: UInt32, json: String, now: Double) -> Batch? {
        if holds() { return nil } // (the Batch barrier's, above)
        return on(busy: nil) {
            guard let data = json.data(using: .utf8),
                  let state = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
                  let formats = state["formats"] as? String,
                  let mixed = state["mixed"] as? Bool,
                  let link = state["link"] as? String,
                  let unavailable = state["unavailable"] as? String else { return nil }
            // The selected atom's source (LLP 1045.000 D3) before the link, which may hold a line break.
            let atom = state["atom"] as? String ?? ""
            let n = write(formats + "\n" + (mixed ? "1" : "0") + "\n" + unavailable + "\n" + atom + "\n" + link)
            return read(exact_dispatch(rt, view, 21, n, now))
        }
    }
    // @ref LLP 1038 D8/D11 — Rust owns URL interpretation on every host.
    package func location(of href: String) -> String {
        return on(busy: "") {
            let n = write(href)
            let len = exact_location_of(rt, n)
            return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    package func protocolLocation(of href: String, protocol name: String, template: String) -> String? {
        return on(busy: nil) {
            let n = write(href + "\0" + name + "\0" + template)
            let len = exact_protocol_location_of(rt, n)
            return len == 0 ? nil : String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    /// Whether `location` names one of the app's declared routes (LLP 1038 §7).
    package func routeMatches(_ location: String) -> Bool {
        return on(busy: false) { exact_route_matches(rt, write(location)) != 0 }
    }
    /// The location of the visit beneath the route keyed `key` on its stack,
    /// `nil` for a stack's root or a key that names no visit (LLP 1115 D5).
    package func locationBeneath(_ key: String) -> String? {
        guard let id = UInt64(key) else { return nil }
        return on(busy: nil) {
            let len = exact_location_beneath(rt, id)
            return len == 0 ? nil : String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    package func visitLocation(_ key: String) -> String? {
        guard let id = UInt64(key) else { return nil }
        return on(busy: nil) {
            let len = exact_visit_location(rt, id)
            return len == 0 ? nil : String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    package func hostBack(from source: String, to destination: String, now: Double) -> Batch? {
        guard let source = UInt64(source), let destination = UInt64(destination) else { return nil }
        return on { read(exact_host_back_to(rt, source, destination, now)) }
    }
    package func launch(_ location: String) { on { () -> Void in let n = write(location); _ = exact_set_launch_location(rt, n) } }
    package func navigate(_ view: UInt32, _ location: String, now: Double) -> Batch {
        return on {
            let n = write(location)
            return read(exact_dispatch(rt, view, 14, n, now))
        }
    }
    /// The platform's own Back from the visit keyed `key` (LLP 1115 D5).
    package func hostBack(_ key: String, now: Double) -> Batch? {
        guard let id = UInt64(key) else { return nil }
        return on { read(exact_host_back(rt, id, now)) }
    }
    package func hostGo(_ location: String, now: Double) -> Batch {
        on { read(exact_host_go(rt, write(location), now)) }
    }
    /// A received share, `{"root", "target", "share", "inbox"?}` (LLP 1133.024 D2–D4).
    package func share(_ json: String, now: Double) -> Batch { on { read(exact_share(rt, write(json), now)) } }
    /// The files of a share claimed before the boot, once storage is configured.
    package func landShare() -> Batch { on { read(exact_share_land(rt)) } }
    /// A package's call of the app's handler (LLP 1133.016 D15 item 1),
    /// `{"extension", "handle", "handler", "args"}`: the runner's answer,
    /// JSON, goes to `reply(ctx, …)` once, now or when the handler's work lands.
    package func invokeModule(_ body: String, reply: ExactInvokeReply, ctx: UnsafeMutableRawPointer?) {
        let ran = on(busy: false) { () -> Bool in exact_module_invoke(rt, write(body), reply, ctx); return true }
        if !ran {
            let busy = Array("{\"error\":\"InvalidStateError\",\"message\":\"the runtime is busy\"}".utf8)
            busy.withUnsafeBufferPointer { reply(ctx, $0.baseAddress, UInt32($0.count)) }
        }
    }
    package func advance(now: Double, untilRequest: Bool = false) -> Batch { on { read(exact_advance(rt, now, untilRequest ? 1 : 0)) } }
    /// The `then`s an agent's input settled, the clock unmoved (LLP 1012 §2).
    package func landThen() -> Batch { on { read(exact_advance(rt, 0, 2)) } }
    /// Those `then`s only when one is the earliest work due (LLP 1072.000 §4 B).
    package func landThenFirst() -> Batch { on { read(exact_advance(rt, 0, 3)) } }
    /// A timer advance on the owner, not waited for (LLP 1072.000): what is
    /// due by `now`, or with `frame` that display frame's (`frame(now:wall:)`).
    package func advanceAsync(now: Double, frame: Double? = nil, capture: UInt64 = 0, measureFrame: Bool = false, done: @escaping (Batch) -> Void) {
        if holds() { Owner.shared.post { done(Runtime.heldBatch) }; return }
        let queued = capture == 0 ? 0 : CACurrentMediaTime()
        Owner.shared.post { [self] in
            guard !destroyed else { return }
            let began = capture == 0 ? 0 : CACurrentMediaTime()
            var batch = read(frame.map { exact_frame_at(rt, $0, now) } ?? exact_advance(rt, now, 0))
            if measureFrame || capture != 0 { batch.frameWorkTiming = FrameWorkTiming(capture: capture, frame: frame,
                queued: queued, began: began, finished: capture == 0 ? 0 : CACurrentMediaTime()) }
            done(batch)
        }
    }
    package func frame(now: Double, wall: Double? = nil) -> Batch { on { read(wall.map { exact_frame_at(rt, now, $0) } ?? exact_frame(rt, now)) } }
    /// Whether motion a commit begins waits for the first presented frame
    /// (LLP 1003.001 D7); off, what waits starts at `at`.
    package func startOnFrame(_ yes: Bool, at: Double) -> Batch { on { read(exact_start_on_frame(rt, yes ? 1 : 0, at)) } }
    package func presentFrames(_ yes: Bool) { on { () -> Void in _ = exact_present_frames(rt, yes ? 1 : 0) } }

    package func resize(width: CGFloat, height: CGFloat) -> Batch { on { read(exact_resize(rt, Float(width), Float(height))) } }
    /// A placement root's resolved margins (left, top, right, bottom) and 1 for an authored width, then the resolved minimum width
    /// (LLP 1126.001 WP10); nil when the view is not laid out.
    package func placementBox(_ view: UInt32) -> [CGFloat]? {
        on(busy: nil as [CGFloat]?) {
            let count = exact_placement_box(rt, view)
            let bytes = Data(bytes: exact_out(rt), count: Int(count))
            guard let values = try? JSONSerialization.jsonObject(with: bytes) as? [Double], values.count == 6 else { return nil }
            return values.map { CGFloat($0) }
        }
    }
    package func setTime(epochAtZero: Double, utcOffset: Double) -> Batch { on { read(exact_set_time(rt, epochAtZero, utcOffset)) } }
    package func setLaunchTime(epochAtZero: Double, utcOffset: Double) { on { () -> Void in _ = exact_set_launch_time(rt, epochAtZero, utcOffset) } }
    @discardableResult
    package func setLaunchPlace(locale: String, timeZone: String, seed: UInt64, formatLocale: String = "") -> Batch { on {
        let count = exact_set_launch_place(rt, write(locale + "\0" + timeZone + "\0" + String(seed) + "\0" + formatLocale))
        return count == 0 ? Batch(ops: [], timers: false, motion: false, clock: nil, error: nil) : read(count)
    } }
    package func setPreferences(_ bits: UInt32) -> Batch { on { read(exact_set_preferences(rt, bits)) } }
    package func setPage(_ bits: UInt32) -> Batch { on { read(exact_set_page(rt, bits)) } }
    /// Whether the running plan reads `onLine`; true when the owner is busy.
    package func readsOnLine() -> Bool { on(busy: true) { exact_reads_on_line(rt) != 0 } }
    package func setRootFontSize(_ px: Double) -> Batch { on { read(exact_set_root_font_size(rt, px)) } }
    package func setRootFontZoom(_ factor: Double) -> Batch { on { read(exact_set_root_font_zoom(rt, factor)) } }
    /// A live design batch or snapshot frame (LLP 1121 stage 4), applied in place.
    package func designRows(_ frame: String) -> Batch { on { read(exact_design_rows(rt, write(frame))) } }
    /// The design digest the running app runs; nil before boot.
    package func designDigest() -> String? {
        on(busy: nil) {
            let len = exact_design_digest(rt)
            return len == 0 ? nil : String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    package func planDigest() -> String? {
        on(busy: nil) {
            let len = exact_plan_digest(rt)
            return len == 0 ? nil : String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    package func linkActivities() { on { exact_link_activities(rt) } }
    /// Activity snapshots or a send completion from the demand-loaded arm.
    package func activities(_ json: String) -> Batch { on { read(exact_activities(rt, write(json))) } }
    /// A MIDI message (LLP 1125 D6–D8): the arm's ports or inputs, or the agent's `prefer midi` and `tap midi`.
    package func midi(_ json: String) -> Batch { on { read(exact_midi(rt, write(json))) } }
    package func preferExtension(_ json: String) -> Batch { on { read(exact_extension(rt, write(json))) } }
    /// A module event the platform reported (LLP 1133.004 D6), heard now.
    package func moduleEvent(_ json: String) -> Batch { on { read(exact_module_event(rt, write(json))) } }
    package func bindExtensions(_ names: [String]) { on { exact_bind_extensions(rt, write(names.joined(separator: "\n"))) } }
    /// A package picture's admission (LLP 1133.019 D11 item 1): nil for a
    /// source of no package's scheme.
    package func picture(_ source: String) -> [String: Any]? {
        on(busy: ["error": "NotAllowedError", "message": "busy"]) {
            let len = exact_picture(rt, write(source))
            guard len > 0 else { return nil }
            let data = Data(bytes: exact_out(rt), count: Int(len))
            return (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? ["error": "NotAllowedError"]
        }
    }
    package func setModes(_ pins: [String: String], check: Bool = false) -> Batch { on { read(exact_set_mode(rt, write((check ? "?" : "") + pins.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" }.joined(separator: ",")))) } }
    package func setDisplayScale(_ scale: Double) -> Batch { on { read(exact_set_display_scale(rt, scale)) } }
    package func setPlace(locale: String, timeZone: String, seed: UInt64, formatLocale: String = "") -> Batch { on { read(exact_set_place(rt, write(locale + "\0" + timeZone + "\0" + String(seed) + "\0" + formatLocale)) ) } }
    package func listIndex(_ view: UInt32, key: String) -> Int? {
        return on(busy: nil) {
            let n = write(key)
            let index = exact_list_index(rt, view, UInt32(n))
            return index == UInt32.max ? nil : Int(index)
        }
    }
    package func listText(_ view: UInt32, first: (String, Int, Int)?, last: (String, Int, Int)?) -> String {
        return on(busy: "") {
            let a = first?.0 ?? "", b = last?.0 ?? ""
            let n = write(a + b)
            let len = exact_list_text(rt, view, UInt32(a.utf8.count), UInt32(n), UInt32(first?.1 ?? 0), UInt32(first?.2 ?? 0), UInt32(last?.1 ?? 0), UInt32(last?.2 ?? 0))
            return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    package func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) -> Batch { on { read(exact_insets(rt, Float(top), Float(right), Float(bottom), Float(left))) } }
    /// The layout regions' insets (LLP 1123 D10): twenty, or none when every region is the safe area.
    package func regions(_ insets: [CGFloat]?) -> Batch {
        let values = (insets ?? []).map(Float.init)
        return on { read(values.withUnsafeBufferPointer { exact_regions(rt, $0.baseAddress, UInt32($0.count)) }) }
    }
    /// The title-bar area beside the window's controls (Window Controls
    /// Overlay): x, y, width, height, or none, when every `titlebar-area-*` is undefined.
    package func titlebarArea(_ rect: [CGFloat]?) -> Batch {
        let values = (rect ?? []).map(Float.init)
        return on { read(values.withUnsafeBufferPointer { exact_titlebar_area(rt, $0.baseAddress, UInt32($0.count)) }) }
    }
    /// What changed of the view at once (`exact_view`): each part given is
    /// told, the window's size (`.some(nil)` clears it), the safe-area
    /// insets, the layout regions (`.some(nil)` when every region is the
    /// safe area), the viewport and the segments' rects on the grid the
    /// kernel has (a keyboard's), in one batch under one layout.
    package struct ViewChange {
        package var screen: CGSize?? = nil
        package var insets: (top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat)? = nil
        package var regions: [CGFloat]?? = nil
        package var size: CGSize? = nil
        package var segments: [CGRect]? = nil
        package var isEmpty: Bool { screen == nil && insets == nil && regions == nil && size == nil && segments == nil }
        package init(screen: CGSize?? = nil, insets: (top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat)? = nil,
                     regions: [CGFloat]?? = nil, size: CGSize? = nil) {
            self.screen = screen; self.insets = insets; self.regions = regions; self.size = size
        }
    }
    package func view(_ change: ViewChange) -> Batch {
        let regions = (change.regions ?? nil) ?? []
        var mask: UInt32 = 0
        if change.screen != nil { mask |= 1 }
        if change.insets != nil { mask |= 2 }
        if change.regions != nil { mask |= 4 }
        if change.size != nil { mask |= 8 }
        if change.segments != nil { mask |= 16 }
        let screen = (change.screen ?? nil) ?? .zero, insets = change.insets ?? (0, 0, 0, 0), size = change.size ?? .zero
        let floats = [screen.width, screen.height, insets.top, insets.right, insets.bottom, insets.left, size.width, size.height]
        let rects = change.segments.map { rects in [UInt32(rects.count)] + rects.flatMap { [$0.minX, $0.minY, $0.width, $0.height].map { Float($0).bitPattern } } } ?? []
        var bytes = Data(capacity: (10 + regions.count + rects.count) * 4)
        for word in [mask] + floats.map({ Float($0).bitPattern }) + [UInt32(regions.count)] + regions.map({ Float($0).bitPattern }) + rects {
            withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) }
        }
        return on { read(exact_view(rt, write(bytes))) }
    }

    package func tick(now: Double, frame: Double? = nil) -> Batch { on { read(frame.map { exact_tick_at(rt, now, $0) } ?? exact_tick(rt, now)) } }
    package func scheme(dark: Bool) -> Batch { on { read(exact_scheme(rt, dark ? 1 : 0)) } }
    package func viewScheme(_ view: UInt32, dark: Bool) -> Batch { on { read(exact_view_scheme(rt, view, dark ? 1 : 0)) } }

    /// Host intrinsic sizes (nil clears one), under one layout.
    package func intrinsics(_ sizes: [(UInt32, CGSize?)]) -> Batch {
        HostPhases.shared.count(.intrinsicReports, by: sizes.count)
        HostPhases.shared.count(.intrinsicLayoutRequests)
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
    package func deliverySync() -> Batch { on { read(exact_delivery_sync(rt)) } }
    /// The returned JSON is copied before the runtime output buffer is reused.
    package func textReady(index: UInt32, generation: UInt32, revision: UInt64) -> Batch {
        return on {
            read(exact_text_ready(rt, index, generation, revision))
        }
    }
    package func regionRequest(_ id: UInt64, knownSource: UInt64) -> [String: Any]? {
        return on(busy: nil) {
            let length = exact_region_request(rt, id, knownSource)
            let data = Data(bytes: exact_out(rt), count: Int(length))
            return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        }
    }

    /// The agent API (LLP 1012): a request in, its reply out — JSON, not a batch.
    package func agent(_ request: String) -> String {
        return on(busy: Runtime.busyAgent) {
            let n = write(request)
            let len = exact_agent(rt, n)
            return String(decoding: Data(bytes: exact_out(rt), count: Int(len)), as: UTF8.self)
        }
    }
    /// A command's ruling (`share`, LLP 1069.003; `saveFile`, LLP 1069.010):
    /// refused, held, or present it.
    package func command(_ request: [String: Any]) -> [String: Any] {
        return on(busy: ["refused": "busy"]) {
            guard let json = try? JSONSerialization.data(withJSONObject: request) else { return ["refused": "unreadable"] }
            let len = exact_command(rt, write(String(decoding: json, as: UTF8.self)))
            let data = Data(bytes: exact_out(rt), count: Int(len))
            return (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? ["refused": "unreadable"]
        }
    }
    /// An auth session's word (LLP 1069.006): `hold` under the agent, or
    /// `done` with the callback URL or a status. The next pump delivers it.
    package func auth(_ request: [String: Any]) {
        on {
            guard let json = try? JSONSerialization.data(withJSONObject: request) else { return }
            _ = exact_auth(rt, write(String(decoding: json, as: UTF8.self)))
        }
    }
    /// How a presence check ended (LLP 1133.010 D13): on the store's
    /// completion path, which carries an unlocked value to its call alone.
    package func presence(_ answer: [String: Any]) {
        on {
            guard let json = try? JSONSerialization.data(withJSONObject: answer) else { return }
            _ = exact_presence(rt, write(String(decoding: json, as: UTF8.self)))
        }
    }
    /// A line for the runner's journal (LLP 1012 §3): what this host refused, and why.
    /// An app module's topic changed (LLP 1024): a notification (T5).
    package func appChanged(_ topic: Data) {
        let rt = rt
        Owner.shared.notify {
            topic.withUnsafeBytes { exact_app_changed(rt, $0.bindMemory(to: UInt8.self).baseAddress, topic.count) }
        }
    }
    /// A notification (LLP 1072 T5): queued to the owner, never waited for,
    /// so a line main journals mid-apply does not wait behind a slice being
    /// built. A later call runs after it; a destroyed runtime refuses it.
    package func log(_ line: String) {
        Owner.shared.notify { [self] in _ = exact_log(rt, write(line)) }
    }
    /// What the host applied of the orientation lock, JSON, and whether its
    /// resolution found a conflict (LLP 1133.029 §8): queued as `log` is.
    package func orientationApplied(_ json: String, conflict: Bool) {
        Owner.shared.notify { [self] in _ = exact_orientation_applied(rt, write(json), conflict ? 1 : 0) }
    }
}

/// The session's orientation lock as the runner tells it (LLP 1133.029 D3).
public struct OrientationRequest: Equatable {
    /// The orientations the holders' types allow, as bits in the Screen
    /// Orientation API's order: portrait-primary 1, portrait-secondary 2,
    /// landscape-primary 4, landscape-secondary 8. 0 when none is held.
    public var types = 0
    /// Whether a holder says `natural`, and `current`.
    public var natural = false, current = false
    /// Each counted holder, `view 12 "current"`, for a line the host journals.
    public var holders: [String] = []
    public init() {}
    init(_ o: [String: BatchValue]) {
        types = Int(o["types"]?.number ?? 0)
        natural = o["natural"] == .bool(true)
        current = o["current"] == .bool(true)
        holders = (o["by"]?.array ?? []).compactMap { pair in
            guard let p = pair.array, p.count == 2, let view = p[0].number, let word = p[1].string else { return nil }
            return "view \(Int(view)) \"\(word)\""
        }
    }
}

/// The pointer's three events and their ABI kinds (LLP 1005 §3).
package enum PointerKind: UInt32 { case down = 29, up = 30, move = 31 }

/// DOM's `PointerEvent`, the subset the runner's record carries (LLP 1056 §3
/// stage 3): the point from the node's content box in its own points, DOM's
/// button bits (AppKit's `pressedMouseButtons` uses the same), 0 to 1 of
/// pressure (DOM's 0.5 while pressed where nothing measures it), the device
/// (`mouse`, `pen`, `touch`), its id (the mouse is 1, as browsers number it),
/// and the point from the viewport, `frame()`'s space (LLP 1094 D11).
package struct PointerSample {
    package var x: Double, y: Double, buttons: Int, pressure: Double, type: String, id: Int
    package var clientX: Double, clientY: Double
    /// The modifiers held, a chord prefix (`KeyCodes.held`): a `MouseEvent`'s.
    package var held = ""
    package var line: String { "\(x),\(y),\(buttons),\(min(1, max(0, pressure))),\(type),\(id),\(clientX),\(clientY),\(held)" }
    package init(x: Double, y: Double, buttons: Int, pressure: Double, type: String, id: Int, clientX: Double, clientY: Double, held: String = "") {
        self.x = x; self.y = y; self.buttons = buttons; self.pressure = pressure; self.type = type; self.id = id
        self.clientX = clientX; self.clientY = clientY; self.held = held
    }
}

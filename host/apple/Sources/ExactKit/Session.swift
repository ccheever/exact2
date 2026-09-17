// The owners (LLP 1031 D1): `ExactApp` — one per process, the Exact app
// the archive links: its asset root, its sessions, its one dev connection —
// `ExactSession` — any number: one runtime handle, one plan, one clock,
// its presenter, text engine, canvases, and web views — and, per
// platform, `ExactView`, the ordinary view that presents a session. The
// standalone apps and a brownfield host both build on exactly these; the
// dev menu, the environment variables, and the agent carrier stay in the
// adapters.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import CExact
import Foundation
import QuartzCore

/// The process facts every session reads: the agent drives the app
/// (LLP 1012 — the driver owns the clock), a smoke run prints and exits.
public enum ExactEnv {
    public static let environment = ProcessInfo.processInfo.environment
    public static let agentMode = environment["EXACT_AGENT"] == "1"
    /// `EXACT_AGENT_TIMING=platform` (LLP 1035.003 D5, opt-in): under the
    /// agent carrier, UIKit's own transitions, presentations and keyboard
    /// animations keep their natural timing — the ordinary app with a
    /// socket, for observing a gesture's native motion. The default freezes
    /// them, which the smoke depends on.
    public static let agentTiming = environment["EXACT_AGENT_TIMING"] ?? "agent"
    /// Whether the agent's world is settled between calls: native animation
    /// applies at once. False under `platform` timing.
    public static let agentFreezes = agentMode && agentTiming != "platform"
    public static let smoke = environment["EXACT_SMOKE"] == "1"
    /// Baked host metadata: a bundle normally; the existing product sidecar in bare development builds.
    nonisolated(unsafe) public static let appMetadata: [String: Any] = {
        if let info = Bundle.main.infoDictionary, info["CFBundleIdentifier"] != nil { return info }
        let executable = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
        let path = executable.deletingLastPathComponent().appendingPathComponent(executable.lastPathComponent + "-Info.plist")
        guard let data = try? Data(contentsOf: path),
              let info = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any] else { return [:] }
        return info
    }()
    public static let appName = appMetadata["CFBundleDisplayName"] as? String ?? appMetadata["CFBundleName"] as? String ?? "Exact"
    /// Milliseconds since the process's `main`: the wall clock for startup stamps.
    public static let t0 = CACurrentMediaTime()
    public static func wall() -> Double { (CACurrentMediaTime() - t0) * 1000 }
    /// Startup stamps, milliseconds from `main`, in order (the smoke prints them).
    nonisolated(unsafe) public static var stamps: [(String, Double)] = []
    public static func stamp(_ label: String) { stamps.append((label, wall())) }
}

/// What a session tells its host: a capability an action called (LLP 1005
/// §3), after the batch that carried it was applied; and its state.
public protocol ExactSessionDelegate: AnyObject {
    func exactSession(_ session: ExactSession, command name: String, args: [Any])
    func exactSession(_ session: ExactSession, didChange state: ExactSession.State)
}

public extension ExactSessionDelegate {
    func exactSession(_ session: ExactSession, command name: String, args: [Any]) {}
    func exactSession(_ session: ExactSession, didChange state: ExactSession.State) {}
}

/// Optional app behavior supplied by a higher composition. Every callback
/// identifies the generation that caused it; the core owns no store policy.
public protocol ExactAppLifecycle: AnyObject {
    /// An initial selected launch starts before preparation; live activation
    /// starts after every session accepts. Repeated marks must be idempotent.
    func generationStarted(_ app: ExactApp, token: UInt64)
    func firstPixel(_ app: ExactApp, token: UInt64)
    func initialGenerationRefused(_ app: ExactApp, token: UInt64, reason: String)
    func handleCommand(_ name: String, app: ExactApp) -> Bool
}

/// A plan and its complete asset namespace prepared by an app composition.
/// The opaque token is meaningful only to that composition; zero is an
/// ordinary core/dev plan, with no delivery selection to count or bless.
/// A module's pairing receipt and compiled bytes. The caller authenticates
/// a signed update or explicitly admits a development origin before preparation.
public struct ExactModule {
    public let receipt: Data
    public let bytecode: Data
    public init(receipt: Data, bytecode: Data) { self.receipt = receipt; self.bytecode = bytecode }
}

public struct ExactGeneration {
    public let plan: Data
    public let assets: AssetResolver
    public let token: UInt64
    public let module: ExactModule?
    public init(plan: Data, assets: AssetResolver, token: UInt64 = 0, module: ExactModule? = nil) {
        self.plan = plan; self.assets = assets; self.token = token
        self.module = module
    }
}

/// The one Exact app this process links (LLP 1031 D1, D11).
public final class ExactApp {
    public static let shared = ExactApp()

    /// Where an image source, a declared font, or a deck page resolves:
    /// `EXACT_ASSETS`, else the bundle (iOS) or the working directory
    /// (macOS) — the way a page resolves against its URL.
    public var assetRoot: URL {
        didSet {
            if resolver != nil && !resolver.isComplete { resolver = AssetResolver(root: assetRoot) }
        }
    }
    /// The complete generation last accepted by every live session. A new
    /// connection can recognize it without restarting carried app state.
    private var devGeneration: String?
    private var devProgram: String?
    /// The plan last applied across the app, also used by newly created sessions.
    private(set) var lastPlan: Data?
    private(set) var lastModule: ExactModule?
    private(set) var resolver: AssetResolver!
    private var transaction = false
    /// A retryable image preparation; the current sessions remain live.
    public private(set) var generationPending = false
    private var notifications: [() -> Void] = []

    func deliver(_ body: @escaping () -> Void) {
        if transaction { notifications.append(body) } else { body() }
    }
    /// Retained for the app lifetime; embedded-only apps supply none.
    public var lifecycle: ExactAppLifecycle?
    private(set) var selectedToken: UInt64 = 0

    private var sessionRefs: [WeakSession] = []
    /// Every live session, in creation order.
    public var sessions: [ExactSession] { sessionRefs.compactMap(\.session) }

    /// The one dev connection (D11): the app URL `dev.mjs` prints, resolved
    /// and subscribed once; every `{seq}` applies to every session.
    private(set) var connection: PlanURL?

    private init() {
        #if canImport(UIKit)
        let fallback = Bundle.main.bundlePath
        #else
        let fallback = Bundle.main.bundleURL.pathExtension == "app"
            ? (Bundle.main.resourcePath ?? Bundle.main.bundlePath) : FileManager.default.currentDirectoryPath
        #endif
        assetRoot = URL(fileURLWithPath: ExactEnv.environment["EXACT_ASSETS"] ?? fallback, isDirectory: true)
        resolver = AssetResolver(root: assetRoot)
    }

    /// The immutable Rust executor policy carried by this binary's bake.
    public var rustPolicy: (mode: String, target: String) {
        let runtime = Runtime()
        defer { runtime.destroy() }
        let length = exact_baked_compat(runtime.rt)
        let bytes = Data(bytes: exact_out(runtime.rt), count: Int(length))
        let json = (try? JSONSerialization.jsonObject(with: bytes) as? [String: Any]) ?? [:]
        let inputs = json["inputs"] as? [String: Any] ?? [:]
        return (inputs["rustMode"] as? String ?? "off", json["target"] as? String ?? "")
    }

    /// Install a composition's launch generation before creating sessions.
    public func installInitial(_ generation: ExactGeneration) {
        precondition(sessions.isEmpty, "install the initial generation before creating sessions")
        resolver = generation.assets
        lastPlan = generation.plan
        lastModule = generation.module
        selectedToken = generation.token
    }

    func fallBackFromInitial(reason: String) -> Bool {
        guard selectedToken != 0, !sessions.contains(where: \.booted) else { return false }
        lifecycle?.initialGenerationRefused(self, token: selectedToken, reason: reason)
        resolver = AssetResolver(root: assetRoot)
        lastPlan = nil
        lastModule = nil
        selectedToken = 0
        return true
    }

    func firstPixel(_ token: UInt64) { lifecycle?.firstPixel(self, token: token) }

    /// Refresh generic delivery facts after a composition-owned event.
    public func refreshDelivery() {
        for session in sessions { session.apply(session.runtime.deliverySync()) }
    }

    func handleCommand(_ name: String) -> Bool { lifecycle?.handleCommand(name, app: self) ?? false }

    /// A session of this app: one runtime, unbooted until `boot` or its view's first layout.
    /// Creates platform views: on iOS call from the application/scene/controller
    /// lifecycle, not top-level code before UIApplicationMain (LLP 1012's timing reduction).
    public func makeSession(delegate: ExactSessionDelegate? = nil, label: String = "") -> ExactSession {
        let s = ExactSession(app: self, label: label.isEmpty ? "session-\(sessionRefs.count + 1)" : label)
        s.delegate = delegate
        sessionRefs.append(WeakSession(s))
        sessionRefs.removeAll { $0.session == nil }
        return s
    }

    func forget(_ session: ExactSession) {
        sessionRefs.removeAll { $0.session == nil || $0.session === session }
    }

    /// Connect to the app URL (LLP 1023 D1–D3): the envelope resolved, the
    /// plan fetched and verified, applied to every session, and the
    /// server's events subscribed so an edit re-fetches. Replaces a prior
    /// connection.
    public func connect(_ url: String) {
        connection?.close()
        connection = PlanURL.open(url, acceptProgram: { [weak self] program in
            guard let self else { return false }
            if let old = self.devProgram { return old == program }
            self.devProgram = program
            return true
        }, current: { [weak self] in self?.devGeneration }, waiting: { [weak self] in self?.generationPending == true }, apply: { [weak self] candidate, label in
            guard let self else { return false }
            let resolver = AssetResolver(root: self.assetRoot, names: Array(candidate.assets.keys), read: { candidate.assets[$0] })
            return self.applyTogether(candidate.plan, label: label, resolver: resolver, token: 0, identity: candidate.identity, module: candidate.module, commit: { true })
        })
    }

    public func disconnect() {
        connection?.close()
        connection = nil
    }

    /// The connection's page URL, when connected (what a deck's `//` source resolves against).
    public var connectedPage: URL? { connection?.page }
    /// The connection's status line for a dev menu, or nil.
    public var connectionStatus: String? { connection.map { "\($0.page)\($0.terminal != nil ? " (rebuild the host)" : generationPending ? " (preparing Rust update)" : " (live)")" } }
    /// Re-resolve the connection (the menu's Reload; clears a `{rebuilt}` stop).
    public func reloadConnection() { connection?.reload() }

    /// A plan is one app generation: every session accepts before any swaps.
    @discardableResult
    public func apply(_ bytes: Data, label: String = "plan") -> Bool {
        applyTogether(bytes, label: label, resolver: resolver, token: 0, commit: { true })
    }

    /// Every session and local consumer accepts before the composition's
    /// durable commit runs. Refusal leaves the previous generation intact.
    @discardableResult
    public func applyGeneration(_ candidate: ExactGeneration, label: String = "generation", commit: () -> Bool) -> Bool {
        applyTogether(candidate.plan, label: label, resolver: candidate.assets, token: candidate.token, module: candidate.module, commit: commit)
    }

    private func applyTogether(_ bytes: Data, label: String, resolver candidateResolver: AssetResolver, token: UInt64, identity: String? = nil, module: ExactModule? = nil, commit: () -> Bool) -> Bool {
        generationPending = false
        guard !transaction else { return false }
        let module = module ?? lastModule
        let participants = sessions.filter { $0.state != .destroyed }
        // An app may stage before creating a view. Validate its plan now,
        // without committing a hidden runner or starting its requests.
        if participants.isEmpty {
            let validator = ExactSession(app: self, label: "")
            defer { validator.destroy() }
            guard validator.prepare(bytes, resolver: candidateResolver, token: token, module: module, size: CGSize(width: 1, height: 1)) != nil else { return false }
            validator.runtime.discardPlan()
        }
        var prepared: [(ExactSession, ExactSession.Prepared)] = []
        for session in participants {
            guard let candidate = session.prepare(bytes, resolver: candidateResolver, token: token, module: module) else {
                generationPending = session.modulePending
                for (session, _) in prepared { session.runtime.discardPlan() }
                return false
            }
            prepared.append((session, candidate))
        }
        let shaderSources = GpuModule.loaded == nil ? nil : candidateResolver.shaderSources()
        let shadersAccepted = shaderSources.map { GpuModule.loaded?.accepts($0) == true } ?? true
        guard candidateResolver.refusal == nil, shadersAccepted, commit() else {
            for (session, _) in prepared { session.runtime.discardPlan() }
            return false
        }
        transaction = true
        let batches = prepared.map { (session, candidate) in (session, session.commit(candidate)) }
        resolver = candidateResolver
        devGeneration = identity
        lastPlan = bytes
        lastModule = module
        selectedToken = token
        if let shaderSources { GpuModule.loaded?.replaceShaders(shaderSources) }
        for (session, batch) in batches { session.presentCommitted(batch, label: label) }
        for session in participants { session.apply(session.runtime.deliverySync()) }
        transaction = false
        let callbacks = notifications
        notifications = []
        for callback in callbacks { callback() }
        return true
    }

    /// A local asset as pinned bytes, verified before any native consumer uses it.
    func assetBytes(_ name: String) -> Data? { resolver.bytes(name) }

    /// Path-only native consumers receive a private materialization of those
    /// same pinned bytes; removed names never reach the embedded directory.
    public func resolveAsset(_ name: String) -> URL? { resolver.url(name) }

    /// Explicit session replacement invalidates the connection's claim that
    /// every session still runs its last accepted complete generation.
    func invalidateDevGeneration() { devGeneration = nil }

}

private struct WeakSession {
    weak var session: ExactSession?
    init(_ s: ExactSession) { session = s }
}

/// One runner, one plan, one clock (LLP 1031 D1).
public final class ExactSession {
    public enum State: Equatable {
        case created
        case ready
        case failed(String)
        case destroyed
    }

    public let app: ExactApp
    /// A host-owned name the agent carrier routes by (D9) and logs carry.
    public let label: String
    public weak var delegate: ExactSessionDelegate?
    public private(set) var state: State = .created {
        didSet { if state != oldValue { let changed = state; app.deliver { [weak self] in guard let self else { return }; delegate?.exactSession(self, didChange: changed) } } }
    }
    /// Bumped by every reboot; a callback from an older generation is dropped.
    public private(set) var generation = 0
    private var activatedGeneration: Int?
    private var updateToken: UInt64 = 0

    let runtime: Runtime
    let rasters = RasterLoader()
    var text: TextEngine
    let presenter: Presenter
    let canvases: Canvases
    let webviews: WebViews
    let frames: Frames
    var clockTimer: Timer?
    /// The agent's clock (milliseconds) when the driver owns time; nil runs
    /// on the wall clock.
    public var clock: Double?
    /// The view presenting this session, while one is mounted (D1).
    weak var view: ExactView?
    /// This session's agent, once a carrier asked for it (`Agent.swift`).
    var agentBox: Agent?
    /// Milliseconds from `main` to this session's first node draw and layout.
    public internal(set) var firstDrawMs: Double?
    public internal(set) var firstLayoutMs: Double?
    /// The first batch and its numbers, once booted (the smoke prints them).
    public private(set) var booted = false
    public private(set) var bootError: String?
    public private(set) var bootMs = 0.0
    public private(set) var rustMs = 0.0
    public private(set) var applyMs = 0.0
    /// Commands from the batch being applied, delivered after it (D2).
    private var pendingCommands: [(String, [Any])] = []
    private var applying = false
    var isApplyingPresentation: Bool { applying }
    // Weak live gesture ownership only; no historical tokens or row registry.
    private let inputHolds = NSHashTable<SwipeHold>.weakObjects()
    func trackInputHold(_ hold: SwipeHold) { inputHolds.add(hold) }
    func retireInputHold(_ hold: SwipeHold) { inputHolds.remove(hold) }

    /// Live sessions by handle: what a wake looks up (a stranger's is dropped).
    nonisolated(unsafe) private static var live: [ExactRuntime: WeakSession] = [:]

    init(app: ExactApp, label: String) {
        self.app = app
        self.label = label
        runtime = Runtime()
        text = TextEngine(resolve: { [weak app] source in app?.resolveAsset(source) }, read: { [weak app] source in app?.assetBytes(source) })
        presenter = Presenter()
        canvases = Canvases()
        webviews = WebViews()
        frames = Frames()
        presenter.session = self
        canvases.session = self
        webviews.session = self
        frames.session = self
        runtime.setMeasure(TextEngine.measureText, ctx: text.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: text.opaque)
        ExactSession.live[runtime.rt] = WeakSession(self)
        runtime.setWake(ExactSession.wake, ctx: UnsafeMutableRawPointer(bitPattern: UInt(runtime.rt)))
        wire()
    }

    deinit { destroy() }

    /// A request's reply is in (LLP 1016 D2): the executor's thread says so;
    /// the pump runs on the main thread, where the runner lives. `ctx` is
    /// the handle; a session that is gone is a stranger, dropped.
    private static let wake: ExactWakeFn = { ctx in
        let rt = ExactRuntime(UInt(bitPattern: ctx))
        DispatchQueue.main.async {
            guard let s = ExactSession.live[rt]?.session else { return }
            s.apply(s.runtime.pump(now: s.now()))
        }
    }

    /// The app's clock: what events, timers, motion, and canvases see.
    public func now() -> Double { clock ?? ExactEnv.wall() }

    private func wire() {
        presenter.collections.onFeedback = { [weak self] bytes in
            guard let self, state != .destroyed else { return }
            apply(runtime.collectionFeedback(bytes, now: now()))
        }
        presenter.onPress = { [unowned self] id in apply(runtime.press(id, now: now())) }
        presenter.onChange = { [unowned self] id, value in apply(runtime.change(id, value, now: now())) }
        presenter.onIntrinsic = { [unowned self] id, size in apply(runtime.intrinsic(id, width: size?.width ?? 0, height: size?.height ?? 0)) }
        presenter.onHover = { [unowned self] id, over in apply(runtime.hover(id, over: over, now: now())) }
        presenter.onFocus = { [unowned self] id in apply(runtime.focus(id, now: now())) }
        presenter.onBlur = { [unowned self] id in apply(runtime.blur(id, now: now())) }
        presenter.onKey = { [unowned self] id, name in apply(runtime.key(id, name, now: now())) }
        presenter.onContextmenu = { [unowned self] id in apply(runtime.contextmenu(id, now: now())) }
        presenter.onSwiperight = { [unowned self] id in apply(runtime.swiperight(id, now: now())) }
        presenter.onScroll = { [unowned self] id, left, top in apply(runtime.scroll(id, left: left, top: top, now: now())) }
        presenter.onDblclick = { [unowned self] id in apply(runtime.dblclick(id, now: now())) }
        presenter.onSubmit = { [unowned self] id in apply(runtime.submit(id, now: now())) }
        presenter.onLoad = { [unowned self] id in apply(runtime.load(id, now: now())) }
        presenter.onMessage = { [unowned self] id, value in apply(runtime.message(id, value, now: now())) }
        // Commands are queued here and delivered once the batch is applied
        // (D2): a delegate then runs against a settled tree.
        presenter.onCommand = { [unowned self] name, args in pendingCommands.append((name, args)) }
    }

    /// Boot the plan baked into the library (or `EXACT_PLAN`'s file, an
    /// adapter's choice, through `boot(plan:)`) under a viewport.
    @discardableResult
    public func boot(size: CGSize) -> Batch {
        let t = CACurrentMediaTime()
        if let bytes = app.lastPlan {
            // A selected launch can crash in runner/font/asset preparation.
            // Record the attempt first; an integrity refusal clears it below.
            if app.selectedToken != 0 { app.lifecycle?.generationStarted(app, token: app.selectedToken) }
            if let candidate = prepare(bytes, resolver: app.resolver, token: app.selectedToken, module: app.lastModule, size: size) {
                let batch = commit(candidate)
                presentCommitted(batch, label: "selected")
                return batch
            }
            let reason = app.resolver.refusal ?? "initial plan refused"
            if !app.fallBackFromInitial(reason: reason) {
                return finishBoot(Batch(ops: [], timers: false, motion: false, clock: nil, error: reason), started: t)
            }
        }
        let cp = text.checkpoint()
        let batch = runtime.boot(width: size.width, height: size.height)
        if batch.error != nil { text.restore(cp) }
        return finishBoot(batch, started: t)
    }

    /// Boot from plan bytes as the first boot (a fixture, `EXACT_PLAN`).
    @discardableResult
    public func boot(plan bytes: Data, size: CGSize) -> Batch {
        let t = CACurrentMediaTime()
        let cp = text.checkpoint()
        let batch = runtime.bootPlan(bytes, width: size.width, height: size.height)
        if batch.error == nil { updateToken = 0; app.invalidateDevGeneration() }
        if batch.error != nil { text.restore(cp) }
        return finishBoot(batch, started: t)
    }

    private func finishBoot(_ batch: Batch, started: Double) -> Batch {
        rustMs = (CACurrentMediaTime() - started) * 1000
        ExactEnv.stamp("runner + layout")
        let tApply = CACurrentMediaTime()
        if batch.error == nil {
            // A fresh boot over a running app (the dev menu's reload from
            // the baked plan) starts the views over; the library already
            // replaced its host.
            routerOp = nil
            generation += 1
            if booted { presenter.reset() }
            booted = true
            text.commitFonts()
        }
        apply(batch)
        // A fresh runner must receive the view's current viewport and insets.
        if batch.error == nil { view?.rebooted() }
        applyMs = (CACurrentMediaTime() - tApply) * 1000
        bootMs = ExactEnv.wall()
        ExactEnv.stamp("first frame applied")
        bootError = batch.error
        state = batch.error.map(State.failed) ?? .ready
        return batch
    }

    struct Prepared {
        let text: TextEngine
        let resolver: AssetResolver
        let token: UInt64
    }

    private(set) var modulePending = false

    func prepare(_ bytes: Data, resolver: AssetResolver, token: UInt64 = 0, module: ExactModule? = nil, size: CGSize? = nil) -> Prepared? {
        modulePending = false
        guard state != .destroyed else { return nil }
        let module = module ?? app.lastModule
        let candidate = TextEngine(resolve: { resolver.url($0) }, read: { resolver.bytes($0) })
        runtime.setMeasure(TextEngine.measureText, ctx: candidate.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: candidate.opaque)
        let viewport = size ?? presenter.viewportSize
        let batch: Batch
        if let module { batch = runtime.prepareModule(bytes, module: module, token: token, width: viewport.width, height: viewport.height) }
        else { batch = runtime.preparePlan(bytes, width: viewport.width, height: viewport.height, token: token) }
        runtime.setMeasure(TextEngine.measureText, ctx: text.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: text.opaque)
        if batch.pending { modulePending = true; return nil }
        // Resolve initially used local payloads before first pixel, without
        // applying a presenter batch or starting an image/web/GPU operation.
        for op in batch.ops {
            let props = (op["props"] as? [String: String]) ?? (op["set"] as? [String: String]) ?? [:]
            if let source = props["src"] ?? props["imageSource"], URL(string: source)?.scheme == nil, !source.hasPrefix("//") {
                let path = source.components(separatedBy: "?")[0].components(separatedBy: "#")[0]
                let name = path.hasPrefix("/") ? String(path.dropFirst()) : path
                if !name.isEmpty { _ = resolver.bytes(name) }
            }
        }
        if let error = batch.error ?? resolver.refusal {
            runtime.discardPlan()
            fputs("exact: prepare refused: \(error)\n", stderr)
            return nil
        }
        return Prepared(text: candidate, resolver: resolver, token: token)
    }

    func commit(_ candidate: Prepared) -> Batch {
        text = candidate.text
        updateToken = candidate.token
        runtime.setMeasure(TextEngine.measureText, ctx: text.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: text.opaque)
        text.commitFonts()
        let batch = runtime.commitPlan()
        precondition(batch.error == nil, "an accepted session candidate must remain commit-ready")
        generation += 1
        return batch
    }

    func presentCommitted(_ batch: Batch, label: String) {
        routerOp = nil
        presenter.reset()
        booted = true
        app.lifecycle?.generationStarted(app, token: updateToken)
        apply(batch)
        view?.rebooted()
        state = .ready
        fputs("reloaded \(label)\n", stderr)
    }

    /// A host may replace one session's plan transactionally; app delivery
    /// uses prepare/commit across every attached session instead.
    @discardableResult
    public func apply(_ bytes: Data, label: String = "plan") -> Bool {
        guard let candidate = prepare(bytes, resolver: app.resolver) else { return false }
        let batch = commit(candidate)
        app.invalidateDevGeneration()
        presentCommitted(batch, label: label)
        return true
    }

    /// A batch into the presenter; frames and the clock follow it; its
    /// commands go to the delegate after it.
    /// @ref LLP 1038 D7/D11 — observation only; Swift never interprets slots.
    private(set) var routerOp: [String: Any]?

    func apply(_ batch: Batch) {
        guard state != .destroyed else { return }
        let outermost = !applying
        applying = true
        for op in batch.ops where op["op"] as? String == "router" { routerOp = op }
        presenter.apply(batch)
        frames.motion = batch.motion
        // The GPU module: after the first painted frame, only when a canvas exists.
        if firstDrawMs != nil { canvases.loadIfNeeded() } else { DispatchQueue.main.async { [weak self] in guard let self else { return }; canvases.loadIfNeeded(); frames.run(frames.motion || canvases.wantsFrames) } }
        frames.run(batch.motion || canvases.wantsFrames)
        if batch.timers, clockTimer == nil, !ExactEnv.agentMode {
            clockTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
                guard let self else { return }
                apply(runtime.advance(now: now()))
            }
        }
        if outermost {
            presenter.collections.flush()
            // Route projection and all structural/style changes are now final.
            // Ineligible recognizers may never receive another mouse/touch event.
            for hold in inputHolds.allObjects { hold.cancelIfInputIneligible() }
            applying = false
            let queued = pendingCommands
            pendingCommands = []
            for (name, args) in queued {
                if name == "copyText" {
                    guard args.count == 1, let text = args.first as? String else {
                        fputs("exact: copyText requires one string\n", stderr)
                        continue
                    }
                    #if canImport(UIKit)
                    UIPasteboard.general.string = text
                    #else
                    NSPasteboard.general.clearContents()
                    if !NSPasteboard.general.setString(text, forType: .string) {
                        fputs("exact: copyText failed\n", stderr)
                    }
                    #endif
                    continue
                }
                if name == "focus" || name == "selectText" {
                    app.deliver { [weak self] in self?.presenter.focusElement(args, selectText: name == "selectText") }
                    continue
                }
                if app.handleCommand(name) { continue }
                app.deliver { [weak self] in guard let self else { return }; delegate?.exactSession(self, command: name, args: args) }
            }
        }
    }

    /// The first node drew: the GPU module may load now (LLP 1009 D4), on
    /// the next turn; the update store hears first pixel (LLP 1026 D11).
    /// Capture at node creation. A delayed old draw cannot bless its successor.
    func drawReceipt() -> () -> Void {
        let drawnGeneration = generation
        let token = updateToken
        return { [weak self] in self?.firstDrawn(generation: drawnGeneration, token: token) }
    }

    private func firstDrawn(generation drawnGeneration: Int, token: UInt64) {
        guard state != .destroyed, generation == drawnGeneration else { return }
        if firstDrawMs == nil { firstDrawMs = ExactEnv.wall() }
        guard activatedGeneration != drawnGeneration else { return }
        activatedGeneration = drawnGeneration
        DispatchQueue.main.async { [weak self] in
            guard let self, state != .destroyed, generation == drawnGeneration else { return }
            let batch = runtime.dataReady()
            if batch.pending {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in
                    guard let self, generation == drawnGeneration, state != .destroyed else { return }
                    activatedGeneration = nil
                    firstDrawn(generation: drawnGeneration, token: token)
                }
                return
            }
            apply(batch)
            if batch.error == nil { app.firstPixel(token) }
            canvases.loadIfNeeded()
            frames.run(frames.motion || canvases.wantsFrames)
        }
    }

    public func resize(_ size: CGSize) { guard booted, state != .destroyed else { return }; apply(runtime.resize(width: size.width, height: size.height)) }
    public func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) { guard booted, state != .destroyed else { return }; apply(runtime.insets(top: top, right: right, bottom: bottom, left: left)) }
    /// The agent API's runner half (LLP 1012): `tree`, `state`, `logs`, `settle`.
    public func agent(_ request: String) -> String { runtime.agent(request) }
    /// A line for the runner's journal (LLP 1012 §3; LLP 1035.001 D6): a
    /// refused intent and its reason, read back through `logs`.
    public func log(_ line: String) { runtime.log(line) }
    /// Deliver an embedder's value through a declared change handler. The
    /// selector must name exactly one live node; file contents stay data.
    @discardableResult public func change(testId: String, value: String) -> Bool {
        guard state != .destroyed, booted else { return false }
        let matches = presenter.views.values.filter { $0.props["testId"] == testId && $0.handlers.contains("change") }
        guard matches.count == 1, let node = matches.first else { return false }
        let batch = runtime.change(node.id, value, now: now())
        apply(batch)
        return batch.error == nil
    }
    /// A host URL before boot is a launch fact; afterwards it is one event.
    /// @ref LLP 1038 D8/D11 — development links are consumed by the adapter first.
    @discardableResult public func openURL(_ url: URL) -> Bool {
        guard state != .destroyed else { return false }
        let location = runtime.location(of: url.absoluteString)
        if !booted { runtime.launch(location); return true }
        return navigate(location)
    }
    @discardableResult public func navigate(_ location: String) -> Bool {
        guard state != .destroyed, booted else { return false }
        guard let node = presenter.views.values.first(where: { $0.props["navigationBack"] != nil && $0.handlers.contains("navigate") }) else {
            log("navigate refused: no navigation root handler")
            return false
        }
        let batch = runtime.navigate(node.id, location, now: now())
        apply(batch)
        return batch.error == nil
    }
    /// The number of live views the presenter holds (the smoke reads it).
    public var viewCount: Int { presenter.views.count }
    /// The first root's frame size (the smoke reads it).
    public var rootSize: CGSize { presenter.rootSize }
    /// The viewport's size in points: what the kernel lays out under.
    public var viewportSize: CGSize { presenter.viewportSize }
    /// The text engine's counters since this session started (the smoke reads them).
    public var measureCount: Int { text.measureCount }
    public var measureHits: Int { text.measureHits }
    public var measureSeconds: Double { text.measureSeconds }
    /// The GPU module's and the web arm's status lines (the smoke reads them).
    public var gpuStatus: String { canvases.status }
    public var webStatus: String { webviews.status }
    /// The window's occlusion changed (macOS): the canvases follow.
    public func occlusionChanged() {
        #if os(macOS)
        canvases.occlusionChanged()
        #endif
        frames.run(frames.motion || canvases.wantsFrames)
    }
    /// The scene became active (iOS): the canvases follow.
    public func becameActive() { frames.run(frames.motion || canvases.wantsFrames) }
    #if canImport(UIKit)
    /// EXACT_FPS (iOS): the measure's report line, once a second.
    public var onFrameReport: ((String) -> Void)? {
        get { frames.onReport }
        set { frames.onReport = newValue }
    }
    /// Keep the display link running regardless (the fps measure).
    public func runFramesAlways() { frames.run(true) }
    #endif

    /// Everything attributable to this session goes (D2): the display link
    /// and clock, the surface instances, the web views, the views, the
    /// runtime — and its handle is a stranger from here on.
    public func destroy() {
        guard state != .destroyed else { return }
        state = .destroyed
        generation += 1
        clockTimer?.invalidate()
        clockTimer = nil
        frames.run(false)
        presenter.reset()
        ExactSession.live.removeValue(forKey: runtime.rt)
        rasters.shutdown()
        runtime.destroy()
        app.forget(self)
    }
}

/// Frames come from the display link, only while motion runs or a canvas
/// has something to render (LLP 1009 D4), per session.
final class Frames: NSObject {
    weak var session: ExactSession?
    var link: CADisplayLink?
    var motion = false
    #if canImport(UIKit)
    /// EXACT_FPS=1 (iOS): the display link runs always and the measure
    /// reports once a second (`main.swift` prints it).
    let fpsMode = ExactEnv.environment["EXACT_FPS"] == "1"
    var ticks = 0
    var lastTick = 0.0
    var longest = 0.0
    var reported = 0.0
    var onReport: ((String) -> Void)?
    #endif

    @objc func tick(_ link: CADisplayLink) {
        guard let s = session else { return }
        #if canImport(UIKit)
        if fpsMode { measure(link.timestamp) }
        #endif
        if motion { s.apply(s.runtime.tick(now: s.now())) }
        let more = s.canvases.tick(now: s.now())
        run(motion || more || s.canvases.wantsFrames)
    }

    #if canImport(UIKit)
    func measure(_ t: Double) {
        if lastTick > 0 { longest = max(longest, t - lastTick) }
        lastTick = t
        ticks += 1
        if reported == 0 { reported = t }
        guard t - reported >= 1, let c = session?.canvases else { return }
        var line = String(format: "fps %.0f · longest gap %.1f ms · renders %d avg %.1f ms · captures %d avg %.1f ms", Double(ticks) / (t - reported), longest * 1000, c.windowRenders, c.windowRenderSeconds * 1000 / Double(max(1, c.windowRenders)), c.windowCaptures, c.windowCaptureSeconds * 1000 / Double(max(1, c.windowCaptures)))
        if !Capture.cpu, let sh = Shadow.shared, c.windowCaptures > 0 { line += String(format: " (mirror %.1f, gpu %.1f, read %.1f)", sh.lastMirrorMs, sh.lastRenderMs, sh.lastReadMs) }
        onReport?(line)
        ticks = 0; longest = 0; reported = t
        c.windowRenders = 0; c.windowRenderSeconds = 0; c.windowCaptures = 0; c.windowCaptureSeconds = 0
    }
    #endif

    func run(_ on: Bool) {
        #if canImport(UIKit)
        let on = on || fpsMode
        #endif
        if on, link == nil {
            #if canImport(UIKit)
            let l = CADisplayLink(target: self, selector: #selector(tick(_:)))
            // The measure wants the display's real rate (a ProMotion phone's
            // 120), so a dropped frame is a dropped frame; the bundle's plist
            // opts in (CADisableMinimumFrameDurationOnPhone) or this is 60.
            if fpsMode { l.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120) }
            #else
            guard let viewport = session?.presenter.viewport else { return }
            let l = viewport.displayLink(target: self, selector: #selector(tick(_:)))
            #endif
            l.add(to: .main, forMode: .common)
            link = l
        } else if !on, let l = link {
            l.invalidate()
            link = nil
        }
    }
}

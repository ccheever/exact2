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

/// Supplied once per session launch and reused by every replacement runner.
struct LaunchPlace: Equatable {
    let locale: String
    let timeZone: String
    let seed: UInt64
    /// Under the agent, the Unix milliseconds at the clock's zero (LLP
    /// 1027.000.000 D3; default 2026-01-01T00:00:00Z); nil reads the machine.
    let epoch: Double?

    init(environment: [String: String] = ExactEnv.environment) {
        if environment["EXACT_AGENT"] == "1" {
            locale = environment["EXACT_AGENT_LOCALE"] ?? "en-US"
            timeZone = environment["EXACT_AGENT_TIME_ZONE"] ?? "UTC"
            seed = UInt64(environment["EXACT_AGENT_SEED"] ?? "1") ?? 1
            epoch = Double(environment["EXACT_AGENT_EPOCH"] ?? "1767225600000") ?? 1_767_225_600_000
        } else {
            epoch = nil
            locale = Locale.preferredLanguages.first ?? Locale.current.identifier(.bcp47)
            timeZone = TimeZone.current.identifier
            seed = UInt64.random(in: 0..<(1 << 53))
        }
    }
}

/// The process facts every session reads: the agent drives the app
/// (LLP 1012 — the driver owns the clock), a smoke run prints and exits.
public enum ExactEnv {
    /// The process's environment. A production bake never enters agent mode
    /// (LLP 1069.007 D2, ruled): its `EXACT_AGENT` and `EXACT_AGENT_*`
    /// variables are dropped here, from the process too, before this host
    /// or its library reads one.
    public static let environment: [String: String] = {
        var environment = ProcessInfo.processInfo.environment
        let agent = environment.keys.filter { $0 == "EXACT_AGENT" || $0.hasPrefix("EXACT_AGENT_") }.sorted()
        guard !agent.isEmpty, productionBake else { return environment }
        FileHandle.standardError.write(Data("exact: a production build ignores \(agent.joined(separator: ", ")) (LLP 1069.007 D2)\n".utf8))
        for key in agent { environment[key] = nil; unsetenv(key) }
        return environment
    }()
    /// Whether this binary was baked with `EXACT_UPDATE_TRUST=production`:
    /// its own `compat.json`'s `inputs.trust`.
    static var productionBake: Bool {
        let bytes = Runtime.bakedCompat()
        let json = (try? JSONSerialization.jsonObject(with: bytes) as? [String: Any]) ?? [:]
        return (json["inputs"] as? [String: Any])?["trust"] as? String == "production"
    }
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
        let bytes = Runtime.bakedCompat()
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
            let resolver = candidate.assets
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
    func bundledAsset(_ name: String) -> URL? { resolver.bundledURL(name) }

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
    /// The generation whose deferred data has activated (or failed to): the
    /// agent's `clock data` waits for it before a test's first step.
    private var dataGeneration: Int?
    var dataActivated: Bool { dataGeneration == generation }
    private var updateToken: UInt64 = 0

    let runtime: Runtime
    let rasters = RasterLoader()
    #if os(macOS)
    lazy var regions = RegionController(self)
    #endif
    var text: TextEngine
    let presenter: Presenter
    private var textPressure: DispatchSourceMemoryPressure?
    let canvases: Canvases
    let webviews: WebViews
    let natives = NativeViews()
    /// The file picker (LLP 1069.002).
    lazy var picker = Picker(session: self)
    let frames: Frames
    /// A development session's presented frames (LLP 1079 D3); a production bake has none.
    private(set) var sampler: FrameSampler?
    var clockTimer: Timer?
    /// The runner deadline `clockTimer` fires for.
    private var clockDue: Double?
    /// The agent's clock (milliseconds) when the driver owns time; nil runs
    /// on the wall clock.
    public var clock: Double?
    /// The runner's soonest timer, from the last batch (absent without timers).
    var timerDue: Double?
    /// The view presenting this session, while one is mounted (D1).
    weak var view: ExactView?
    #if os(iOS) || os(tvOS)
    private var systemDark = false
    #endif
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
    private var pendingCommands: [(String, [Any], UInt32?)] = []
    private var applying = false
    // @ref LLP 1072 §3 — a collection slice built on the owner thread. While
    // one is in flight, work that would wait behind it on the owner (a
    // frame's tick, a timer, a reply, an intrinsic size, a report) waits on
    // main instead, and runs after the slice lands, in order.
    private(set) var fillInFlight = false
    private var afterFill: [() -> Void] = []
    private let published = NSLock()
    /// What the owner committed without main waiting.
    private enum Published { case fill(UInt32), tick, canvas }
    /// Batches the owner committed without main waiting, in its order: a
    /// slice's (with its list), a frame's tick's and a canvas draw's.
    private var publishedQueue: [(kind: Published, batch: Batch, generation: Int)] = []
    /// A frame's tick is on the owner (LLP 1072 §7.1): the next one waits.
    private(set) var tickInFlight = false
    /// Canvas draws run in a turn of their own on the owner (LLP 1072 §8.5):
    /// the last batch applied says whether one is owed, one is in flight
    /// at a time, and one is asked for once per main-queue turn.
    private(set) var canvasInFlight = false
    private var canvasOwed = false
    private var canvasAsked = false
    private var landing = false
    /// Slices build off main on iOS unless the agent drives the app, or
    /// `EXACT_FILL_SYNC=1` asks for the synchronous path (LLP 1072 T12).
    /// macOS follows once physical scrolling there is measured (stage 5).
    /// Read when a session wires itself; tests set it to drive the
    /// asynchronous path on macOS.
    #if os(iOS) || os(tvOS)
    nonisolated(unsafe) static var asyncFills = !ExactEnv.agentMode && ExactEnv.environment["EXACT_FILL_SYNC"] != "1"
    #else
    nonisolated(unsafe) static var asyncFills = false
    #endif
    var isApplyingPresentation: Bool { applying }
    // Weak live gesture ownership only; no historical tokens or row registry.
    private let inputHolds = NSHashTable<SwipeHold>.weakObjects()
    weak var heightInputHold: HeightDragHold?
    weak var transformInputHold: TransformDragHold?
    func trackInputHold(_ hold: SwipeHold) { inputHolds.add(hold) }
    func retireInputHold(_ hold: SwipeHold) { inputHolds.remove(hold) }
    private var pendingSurfaceRecords: [(String, String?)] = []
    private var pendingSurfaceWork: [([String: Any], Int)] = []
    /// Authentication sessions this session opened (`Auth.swift`, LLP 1069.006).
    lazy var authSessions: AuthSessions = { let s = AuthSessions(); s.owner = self; return s }()

    /// Live sessions by handle: what a wake looks up (a stranger's is dropped).
    nonisolated(unsafe) private static var live: [ExactRuntime: WeakSession] = [:]
    private var preferenceObservers: [NSObjectProtocol] = []
    private var pageObservers: [NSObjectProtocol] = []

    init(app: ExactApp, label: String) {
        self.app = app
        self.label = label
        runtime = Runtime()
        text = TextEngine.pair(resolve: { [weak app] source in app?.resolveAsset(source) }, read: { [weak app] source in app?.assetBytes(source) },
                               bundled: { [weak app] source in app?.bundledAsset(source) })
        presenter = Presenter()
        canvases = Canvases()
        webviews = WebViews()
        frames = Frames()
        presenter.session = self
        canvases.session = self
        webviews.session = self
        natives.session = self
        frames.session = self
        sampler = FrameSampler.measured ? FrameSampler(session: self) : nil
        runtime.setMeasure(TextEngine.measureText, ctx: text.measuring.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: text.measuring.opaque)
        // LLP 1056 D8, D9: Canvas 2D measures with this engine and draws the
        // handles this session decodes.
        runtime.setCanvasText(CanvasText.measureRun)
        presenter.canvas2d.textEngine = { [weak self] in self?.text }
        presenter.canvas2d.assetBytes = { [weak app] in app?.assetBytes($0) }
        presenter.canvas2d.onImage = { [weak self] src, image in
            guard let self, self.state != .destroyed else { return }
            self.apply(self.runtime.canvasImage(src, width: image?.width ?? 0, height: image?.height ?? 0, ok: image != nil))
        }
        presenter.canvas2d.onHeld = { [weak self] view, held in
            guard let self, self.state != .destroyed else { return }
            self.runtime.canvasHeld(view, held)
        }
        // LLP 1056 D4: Canvas 2D backs its bitmaps at the display's scale.
        #if canImport(UIKit)
        let scale = presenter.viewport.traitCollection.displayScale
        #else
        let scale = presenter.viewport.window?.backingScaleFactor ?? 1
        #endif
        _ = runtime.canvasDisplay(scale: max(1, scale), memory: ProcessInfo.processInfo.physicalMemory)
        // A window on another display corrects it; the canvases redraw (D4).
        presenter.canvas2d.onScale = { [weak self] scale in
            DispatchQueue.main.async {
                guard let self, self.state != .destroyed else { return }
                self.apply(self.runtime.canvasDisplay(scale: scale, memory: ProcessInfo.processInfo.physicalMemory))
            }
        }
        ExactSession.live[runtime.rt] = WeakSession(self)
        runtime.setWake(ExactSession.wake, ctx: UnsafeMutableRawPointer(bitPattern: UInt(runtime.rt)))
        natives.installAppModule()
        let pressure = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .main)
        pressure.setEventHandler { [weak self] in
            guard let self else { return }
            text.dropCold()
            // The measurer's shaped text is the owner's to drop (LLP 1072 §8.4).
            if let m = text.measurer { Owner.shared.post { m.dropCold() } }
        }
        pressure.resume(); textPressure = pressure
        wire()
        preferenceObservers = DisplayPreferences.observe { [weak self] in self?.tellPreferences() }
        pageObservers = PageFacts.observe { [weak self] in self?.tellPage() }
    }

    deinit { destroy() }

    /// Scrolling has settled (iOS: `ScrollPump.restDelay`): the decoded
    /// images, shaped text and text pixels no view shows go, the tree's
    /// storage comes back to its live nodes, and the allocator returns the
    /// pages they leave. What shows keeps its own.
    func rest() {
        runtime.trim()
        rasters.trimCold()
        text.dropColdShaped()
        if let m = text.measurer { Owner.shared.post { m.dropColdShaped() } }
        #if os(iOS) || os(tvOS)
        presenter.textRasters.dropKept()
        #endif
        DispatchQueue.global(qos: .utility).async { malloc_zone_pressure_relief(nil, 0) }
    }

    /// A request's reply is in (LLP 1016 D2): the executor's thread says so;
    /// the pump runs on the main thread, where the runner lives. `ctx` is
    /// the handle; a session that is gone is a stranger, dropped.
    /// The live session behind a runtime handle, or nil for a stranger's.
    static func session(for rt: ExactRuntime) -> ExactSession? { live[rt]?.session }

    private static let wake: ExactWakeFn = { ctx in
        let rt = ExactRuntime(UInt(bitPattern: ctx))
        DispatchQueue.main.async {
            guard let s = ExactSession.live[rt]?.session else { return }
            s.whenIdle { [weak s] in guard let s else { return }; s.apply(s.runtime.pump(now: s.now())) }
        }
    }

    /// The app's clock: what events, timers, motion, and canvases see.
    public func now() -> Double { clock ?? ExactEnv.wall() }

    /// Run `work` now, or after the slice in flight lands (LLP 1072 §3).
    func whenIdle(_ work: @escaping () -> Void) {
        if fillInFlight { afterFill.append(work) } else { work() }
    }

    /// Send a build-only slice to the owner; its batch comes back published.
    private func sendFill(_ view: UInt32, _ bytes: Data) {
        fillInFlight = true
        let captured = generation
        runtime.collectionFeedbackAsync(bytes, now: now()) { [weak self] batch in
            self?.publish(.fill(view), batch, captured)
        }
    }

    /// A frame's tick on the owner, not waited for: motion sampled at this
    /// frame lands with the next main-queue turn (LLP 1072 §7.1).
    func sendTick(now: Double) {
        tickInFlight = true
        let captured = generation
        runtime.tickAsync(now: now) { [weak self] batch in self?.publish(.tick, batch, captured) }
    }

    /// The owed canvas draws, after this main-queue turn's calls and not
    /// waited for (LLP 1072 §8.5): main is not awake for a draw. A tick in
    /// flight draws in its own turn, so none is asked for meanwhile; its
    /// batch says whether one is still owed.
    private func askCanvasDraw() {
        guard canvasOwed, !canvasAsked, !canvasInFlight else { return }
        canvasAsked = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            canvasAsked = false
            guard state != .destroyed, canvasOwed, !canvasInFlight, !tickInFlight else { return }
            canvasInFlight = true
            let captured = generation
            runtime.canvasDrawAsync { [weak self] batch in self?.publish(.canvas, batch, captured) }
        }
    }

    /// On the owner: queue a batch for main, in the owner's order.
    private func publish(_ kind: Published, _ batch: Batch, _ generation: Int) {
        published.lock()
        publishedQueue.append((kind, batch, generation))
        published.unlock()
        DispatchQueue.main.async { [weak self] in self?.landFill() }
    }

    private var hasPublished: Bool {
        published.lock()
        defer { published.unlock() }
        return !publishedQueue.isEmpty
    }

    /// Apply the slice the owner published, then what it owes and what
    /// waited for it. Before any other batch applies, so batches apply in
    /// the owner's order (T4).
    func landFill() {
        guard !landing else { return }
        landing = true
        defer { landing = false }
        while true {
            published.lock()
            let next = publishedQueue.isEmpty ? nil : publishedQueue.removeFirst()
            published.unlock()
            guard let next else { return }
            switch next.kind {
            case .fill: fillInFlight = false
            case .tick: tickInFlight = false
            case .canvas: canvasInFlight = false
            }
            guard state != .destroyed, next.generation == generation else {
                if case .fill = next.kind { afterFill.removeAll() }
                continue
            }
            let batch = next.batch
            if !(batch.ops.isEmpty && batch.error == nil && batch.motion == frames.motion && batch.spatial == frames.spatial && batch.timerDueMs == timerDue && batch.canvasOwed == canvasOwed) {
                apply(batch)
            }
            // A draw owed while this one ran, or one a tick left owed.
            if canvasOwed { askCanvasDraw() }
            guard case .fill(let view) = next.kind else { continue }
            presenter.collections.landed(view, at: ProcessInfo.processInfo.systemUptime)
            let waiting = afterFill
            afterFill.removeAll()
            for work in waiting { work() }
        }
    }

    /// Land every session's slice in flight.
    static func drainAll() { for weak in live.values { weak.session?.drainFill() } }

    /// Wait for the slice in flight and land it: what shows, the agent and
    /// every read that must see it (T5, T7, T9).
    public func drainFill() {
        guard fillInFlight || tickInFlight || canvasInFlight || hasPublished else { return }
        Owner.shared.sync {}
        landFill()
    }

    private func wire() {
        if ExactSession.asyncFills {
            // @ref LLP 1072 §8.5 — main does not wait for a canvas's draw.
            runtime.canvasDefer(true)
            presenter.collections.onFill = { [weak self] view, bytes in self?.sendFill(view, bytes) }
            presenter.collections.filling = { [weak self] in self?.fillInFlight ?? false }
            presenter.collections.drain = { [weak self] in self?.drainFill() }
        }
        presenter.collections.onFeedback = { [weak self] bytes in
            guard let self, state != .destroyed else { return }
            let batch = runtime.collectionFeedback(bytes, now: now())
            // A report inside the built window commits nothing: while a list
            // moves that is most frames. Skip the presenter's finalization
            // pass for it, unless the clock or the motion it reports is news.
            if batch.ops.isEmpty && batch.error == nil && batch.motion == frames.motion && batch.spatial == frames.spatial && batch.timerDueMs == timerDue && batch.canvasOwed == canvasOwed { return }
            apply(batch)
        }
        presenter.onPress = { [unowned self] id in apply(runtime.press(id, held: presenter.pressHeld, now: now())) }
        presenter.onChange = { [unowned self] id, value in apply(runtime.change(id, documentValue(id, value), now: now())) }
        presenter.onInput = { [unowned self] id, value in apply(runtime.input(id, value, now: now())) }
        // @ref LLP 1069.001 D4 — a toggle is HTML's `input` then `change`,
        // each where the node hears it.
        presenter.onChecked = { [unowned self] id, on in
            let handlers = presenter.views[id]?.handlers ?? []
            if handlers.contains("input") { apply(runtime.checked(id, on, commit: false, now: now())) }
            if handlers.contains("change") { apply(runtime.checked(id, on, commit: true, now: now())) }
        }
        presenter.onControlValue = { [unowned self] id, value, input, change in
            let handlers = presenter.views[id]?.handlers ?? []
            if input, handlers.contains("input") { apply(runtime.input(id, value, now: now())) }
            if change, handlers.contains("change") { apply(runtime.change(id, value, now: now())) }
        }
        presenter.selectOptions = { [unowned self] id in runtime.selectOptions(id) }
        presenter.buttonFace = { [unowned self] id in runtime.buttonFace(id) }
        presenter.onIntrinsic = { [unowned self] sizes in whenIdle { [unowned self] in apply(runtime.intrinsics(sizes)) } }
        #if os(iOS)
        presenter.groupedList = { [unowned self] id in runtime.groupedList(id) }
        #endif
        #if os(iOS) || os(tvOS)
        // @ref LLP 1075.003 §3.5, Q3 (c) — what a bar covers reaches layout
        // as an intrinsic size does; the hooks replay once the module connects.
        presenter.onCovers = { [unowned self] covers in whenIdle { [unowned self] in apply(runtime.covers(covers)) } }
        natives.onHooksConnected = { [weak self] in
            self?.presenter.navigation.replayHooks()
            self?.presenter.elements.replay()
        }
        #else
        natives.onHooksConnected = { [weak self] in
            self?.presenter.elements.replay()
            self?.presenter.toolbar.hookToolbar()
        }
        #endif
        presenter.onHover = { [unowned self] id, over in apply(runtime.hover(id, over: over, now: now())) }
        presenter.onFocus = { [unowned self] id in apply(runtime.focus(id, now: now())) }
        presenter.onBlur = { [unowned self] id in apply(runtime.blur(id, now: now())) }
        presenter.onKey = { [unowned self] id, name in apply(runtime.key(id, name, now: now())) }
        presenter.onContextmenu = { [unowned self] id in apply(runtime.contextmenu(id, now: now())) }
        presenter.onSwiperight = { [unowned self] id in apply(runtime.swiperight(id, now: now())) }
        presenter.onRefresh = { [unowned self] id in apply(runtime.refresh(id, now: now())) }
        presenter.onPan = { [unowned self] id, dx, dy in apply(runtime.pan(id, dx: dx, dy: dy, now: now())) }
        presenter.onPanRelease = { [unowned self] id, vx, vy in apply(runtime.panRelease(id, vx: vx, vy: vy, now: now())) }
        presenter.onPanSample = { [unowned self] first, x, y, t in runtime.panSample(first: first, x: x, y: y, t: t) }
        presenter.panVelocity = { [unowned self] t in runtime.panVelocity(at: t) }
        presenter.onScroll = { [unowned self] id, metrics in apply(runtime.scroll(id, metrics: metrics, now: now())) }
        presenter.onScrolled = { [unowned self] id, left, top in runtime.scrolled(id, left: left, top: top) }
        #if canImport(AppKit)
        presenter.onListIndex = { [unowned self] id, key in runtime.listIndex(id, key: key) }
        presenter.onListText = { [unowned self] id, first, last in runtime.listText(id, first: first, last: last) }
        #endif
        presenter.onDblclick = { [unowned self] id in apply(runtime.dblclick(id, now: now())) }
        presenter.onPointer = { [unowned self] id, kind, sample in apply(runtime.pointer(id, kind, sample, now: now())) }
        presenter.onSubmit = { [unowned self] id in apply(runtime.submit(id, now: now())) }
        presenter.onLoad = { [unowned self] id in apply(runtime.load(id, now: now())) }
        presenter.onMessage = { [unowned self] id, value in apply(runtime.message(id, value, now: now())) }
        presenter.onClipboard = { [unowned self] id, kind, text in apply(runtime.clipboard(id, kind, text, now: now())) }
        #if os(macOS)
        presenter.onSelectionChange = { [unowned self] id, text, start, end in
            apply(runtime.selectionChange(id, text, start: start, end: end, now: now()))
        }
        #endif
        // Commands are queued here and delivered once the batch is applied
        // (D2): a delegate then runs against a settled tree.
        presenter.onCommand = { [unowned self] name, args, source in pendingCommands.append((name, args, source)) }
    }

    /// Boot the plan baked into the library (or `EXACT_PLAN`'s file, an
    /// adapter's choice, through `boot(plan:)`) under a viewport.
    @discardableResult
    public func boot(size: CGSize) -> Batch {
        let t = CACurrentMediaTime()
        primePreferences()
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
        primePreferences()
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
            sampler?.reset() // a new runner numbers its transactions afresh (LLP 1079 D3)
            // A fresh boot over a running app (the dev menu's reload from
            // the baked plan) starts the views over; the library already
            // replaced its host.
            routerOp = nil
            generation += 1
            if booted { presenter.reset() }
            booted = true
            text.commitFonts()
            AppFiles.learn(runtime) // before the first frame's `app:/` images load (LLP 1069.002 D7)
        }
        apply(batch)
        if batch.error == nil { tellTime() }
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
    /// While a carried restart applies its tree, autofocus waits; the restart
    /// then puts focus back at its place (`Presenter.restoreFocus`).
    var autofocusHeld = false
    private var keptFocus: FocusPlace?

    func prepare(_ bytes: Data, resolver: AssetResolver, token: UInt64 = 0, module: ExactModule? = nil, size: CGSize? = nil) -> Prepared? {
        modulePending = false
        guard state != .destroyed else { return nil }
        // The running tree's focus, read before the candidate replaces it.
        keptFocus = booted ? presenter.focusPlace(tree: agent("{\"op\":\"tree\"}")) : nil
        let module = module ?? app.lastModule
        let candidate = TextEngine.pair(resolve: { resolver.url($0) }, read: { resolver.bytes($0) }, bundled: { resolver.bundledURL($0) })
        runtime.setMeasure(TextEngine.measureText, ctx: candidate.measuring.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: candidate.measuring.opaque)
        let viewport = size ?? presenter.viewportSize
        let batch: Batch
        if let module { batch = runtime.prepareModule(bytes, module: module, token: token, width: viewport.width, height: viewport.height) }
        else { batch = runtime.preparePlan(bytes, width: viewport.width, height: viewport.height, token: token) }
        runtime.setMeasure(TextEngine.measureText, ctx: text.measuring.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: text.measuring.opaque)
        if batch.pending { modulePending = true; return nil }
        // Resolve initially used local payloads before first pixel, without
        // applying a presenter batch or starting an image/web/GPU operation.
        for op in batch.ops {
            let props = op.props
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
        runtime.setMeasure(TextEngine.measureText, ctx: text.measuring.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: text.measuring.opaque)
        text.commitFonts()
        let batch = runtime.commitPlan()
        precondition(batch.error == nil, "an accepted session candidate must remain commit-ready")
        generation += 1
        return batch
    }

    func presentCommitted(_ batch: Batch, label: String) {
        routerOp = nil
        let restart = booted, kept = keptFocus
        keptFocus = nil
        presenter.reset()
        booted = true
        app.lifecycle?.generationStarted(app, token: updateToken)
        autofocusHeld = restart
        sampler?.reset() // a new runner numbers its transactions afresh (LLP 1079 D3)
        AppFiles.learn(runtime)
        apply(batch)
        tellTime()
        view?.rebooted()
        autofocusHeld = false
        if restart { presenter.restoreFocus(kept, tree: agent("{\"op\":\"tree\"}")) }
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

    func surfaceRecord(_ name: String, _ json: String?) {
        guard state != .destroyed else { return }
        if applying { pendingSurfaceRecords.append((name, json)); return }
        apply(runtime.surfaceRecord(name, json))
    }

    func apply(_ batch: Batch) {
        guard state != .destroyed else { return }
        // A slice the owner committed before this batch applies first (T4).
        if !applying, !landing, fillInFlight || tickInFlight || canvasInFlight, hasPublished { landFill() }
        let outermost = !applying
        applying = true
        // What applying it cost, with its transactions, for the next sampled frame (LLP 1079 D3).
        let began = sampler == nil ? 0 : CACurrentMediaTime()
        defer { sampler?.batch(batch.seq, ms: outermost ? (CACurrentMediaTime() - began) * 1000 : 0) }
        // Each batch says what its turn left owed; batches apply in the
        // owner's order, so the last one applied is the runner's now.
        canvasOwed = batch.canvasOwed
        if canvasOwed { askCanvasDraw() }
        for op in batch.ops where op.op == .router { routerOp = op.payload }
        // @ref LLP 1048.003 D1 — the head's title, for the app that owns the chrome.
        for op in batch.ops where op.op == .title { presenter.headTitle(op.payload["title"] as? String) }
        #if os(macOS)
        regions.prepare(batch)
        #endif
        for op in batch.ops where op.op == .surfaceWork { pendingSurfaceWork.append((op.payload, generation)) }
        // Opened once the batch is applied, off this session's window (LLP 1069.006 D3).
        for op in batch.ops where op.op == .auth {
            let payload = op.payload
            DispatchQueue.main.async { [weak self] in if let self, state != .destroyed { authOp(payload) } }
        }
        presenter.apply(batch)
        if !batch.canvasImages.isEmpty { presenter.canvas2d.load(batch.canvasImages) }
        AnimatedRasters.shared.poke()
        for op in batch.ops where op.op == .reorder { presenter.reorder?.observe(ReorderState(op.payload)) }
        presenter.reorder?.raiseLifted()
        frames.motion = batch.motion
        frames.spatial = batch.spatial
        frames.canvas2d = batch.canvas
        frames.tasks = batch.frames
        // The GPU module: after the first painted frame, only when a canvas exists.
        if firstDrawMs != nil { canvases.loadIfNeeded(); natives.loadIfNeeded(); drainSurfaceWork() } else { DispatchQueue.main.async { [weak self] in guard let self else { return }; canvases.loadIfNeeded(); drainSurfaceWork(); frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames) } }
        timerDue = batch.timerDueMs
        scheduleClock(due: batch.timerDueMs)
        if outermost {
            while !pendingSurfaceRecords.isEmpty {
                let (name, json) = pendingSurfaceRecords.removeFirst()
                apply(runtime.surfaceRecord(name, json))
            }
            while !pendingViewDark.isEmpty {
                let (id, dark) = pendingViewDark.removeFirst()
                apply(runtime.viewScheme(id, dark: dark))
            }
            presenter.collections.flush()
            // Route projection and all structural/style changes are now final.
            // Ineligible recognizers may never receive another mouse/touch event.
            for hold in inputHolds.allObjects { hold.cancelIfInputIneligible() }
            heightInputHold?.cancelIfInputIneligible()
            transformInputHold?.cancelIfInputIneligible()
            presenter.transformGeometry.changed()
            applying = false
            #if os(macOS)
            regions.flush()
            #endif
            let queued = pendingCommands
            pendingCommands = []
            for (name, args, source) in queued {
                if name == "copyText" {
                    guard args.count == 1, let text = args.first as? String else {
                        fputs("exact: copyText requires one string\n", stderr)
                        continue
                    }
                    #if os(tvOS)
                    // tvOS has no pasteboard.
                    fputs("exact: copyText: no pasteboard\n", stderr)
                    _ = text
                    #elseif canImport(UIKit)
                    UIPasteboard.general.string = text
                    #else
                    NSPasteboard.general.clearContents()
                    if !NSPasteboard.general.setString(text, forType: .string) {
                        fputs("exact: copyText failed\n", stderr)
                    }
                    #endif
                    continue
                }
                if name == "reload" {
                    // The dev menu's Reload, from the app; a build without the dev menu refuses it.
                    guard DevMenu.enabled else { fputs("exact: reload: no dev menu in this build\n", stderr); continue }
                    app.deliver { DevMenu.reload() }
                    continue
                }
                if name == "haptic" {
                    // @ref LLP 1077 D14 — feedback from app logic.
                    app.deliver { Haptics.play(args.first as? String ?? "") }
                    continue
                }
                if name == "share" {
                    app.deliver { [weak self] in self?.share(args, source: source) }
                    continue
                }
                if name == "postMessage" {
                    // The inverse of `message=`: text into the named surface, in order.
                    let text = args.first as? String ?? "", surface = args.count > 1 ? args[1] as? String ?? "" : ""
                    app.deliver { [weak self] in self?.canvases.post(surface, text) }
                    continue
                }
                if name == "focus" || name == "selectText" {
                    app.deliver { [weak self] in self?.presenter.focusElement(args, selectText: name == "selectText") }
                    continue
                }
                if name == "blur" {
                    app.deliver { [weak self] in self?.presenter.blurElement(args) }
                    continue
                }
                if name == "scrollIntoView" {
                    app.deliver { [weak self] in self?.presenter.scrollElementIntoView(args) }
                    continue
                }
                if name == "showPicker" {
                    app.deliver { [weak self] in self?.picker.show(args) }
                    continue
                }
                if name == "saveFile" {
                    app.deliver { [weak self] in self?.picker.save(args) }
                    continue
                }
                if ["showOpenFilePicker", "showDirectoryPicker", "showSaveFilePicker"].contains(name) {
                    app.deliver { [weak self] in self?.picker.document(name, args) }
                    continue
                }
                if name == "format" {
                    app.deliver { [weak self] in self?.presenter.formatElement(args) }
                    continue
                }
                if app.handleCommand(name) { continue }
                app.deliver { [weak self] in guard let self else { return }; delegate?.exactSession(self, command: name, args: args) }
            }
        }
    }

    private func drainSurfaceWork() {
        guard canvases.ready || canvases.failed != nil || canvases.entries.isEmpty,
              !pendingSurfaceWork.isEmpty else { return }
        let work = pendingSurfaceWork
        pendingSurfaceWork = []
        DispatchQueue.main.async { [weak self] in
            guard let self, state != .destroyed else { return }
            for (op, owner) in work { canvases.surfaceWork(op, generation: owner) }
        }
    }

    func completeSurface(_ ticket: UInt64, generation owner: Int, kind: UInt32, body: Data = Data()) {
        guard state != .destroyed, generation == owner, runtime.requestActive(ticket) else { return }
        apply(runtime.fulfillSurface(ticket, kind: kind, body: body, now: now()))
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
            // The app module is ready before any source can call it: its load
            // never lands inside a `native.call`'s budget (LLP 1067.000 D8).
            natives.prepareAppModule()
            let batch = runtime.dataReady()
            if batch.pending {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in
                    guard let self, generation == drawnGeneration, state != .destroyed else { return }
                    activatedGeneration = nil
                    firstDrawn(generation: drawnGeneration, token: token)
                }
                return
            }
            AppFiles.learn(runtime) // the roots storage configured
            apply(batch)
            dataGeneration = drawnGeneration
            if batch.error == nil {
                presenter.collections.dataReady()
                app.firstPixel(token)
            }
            canvases.loadIfNeeded()
            natives.loadIfNeeded() // @ref LLP 1024 D3 — the turn after first draw
            drainSurfaceWork()
            frames.run(frames.motion || canvases.wantsFrames)
            frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames)
        }
    }

    // @ref LLP 1043.000 §3 D8, §6 ruling 5 — one advance per display frame,
    // or one distant wake. Runner retains ordered catch-up and its 4096-commit cap.
    func scheduleClock(due: Double?) {
        let agent = ExactEnv.agentMode || clock != nil
        let wake = SessionClockTimer.wake(due: due, now: now(), agent: agent)
        // A frame task (LLP 1073 D5) keeps the link running; the agent's clock fires its virtual frames.
        frames.timerSoon = wake == .frame || (frames.tasks && !agent)
        // A timer armed for this same deadline stays: most batches leave the
        // runner's next deadline where it was, and a new timer each batch
        // cost more than the batch's other bookkeeping.
        if case .timeout = wake, let armed = clockTimer, armed.isValid, clockDue == due {
            frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames)
            return
        }
        clockTimer?.invalidate()
        clockTimer = nil
        clockDue = nil
        if case .timeout(let delay) = wake {
            clockDue = due
            clockTimer = SessionClockTimer.schedule(after: delay / 1000) { [weak self] _ in
                guard let self, state != .destroyed else { return }
                clockTimer = nil
                whenIdle { [weak self] in guard let self, state != .destroyed else { return }; followOffset(); apply(runtime.advance(now: now())) }
            }
        }
        frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames)
    }

    /// Drawn once per launch; the same across a dev reload's new runner.
    let launchPlace = LaunchPlace()
    /// @ref LLP 1027.000.000 — the date, against the clock `now()` reads.
    func tellTime() {
        if launchPlace.epoch != nil {
            tellAgentOffset()
        } else {
            toldOffset = nil
            followOffset()
        }
        apply(runtime.setPlace(locale: launchPlace.locale, timeZone: launchPlace.timeZone, seed: launchPlace.seed))
        tellPreferences()
        tellPage()
    }
    /// The machine zone's offset now, told when it is not the one last told:
    /// at boot and before each advance, so a DST change or a new zone reaches
    /// the timer that fires after it (habits F6). Under the agent, the
    /// drive's zone moves it instead (`tellAgentOffset`).
    var toldOffset: Double?
    func followOffset() {
        guard launchPlace.epoch == nil else { return }
        let offset = Double(TimeZone.autoupdatingCurrent.secondsFromGMT()) / 60
        guard offset != toldOffset else { return }
        toldOffset = offset
        apply(runtime.setTime(epochAtZero: Date().timeIntervalSince1970 * 1000 - now(), utcOffset: offset))
    }
    /// Under the agent, the drive's date at the clock's zero and its zone's
    /// offset at the virtual instant the clock reads: told at boot and after
    /// every `clock`, so a move across a DST change re-answers it (LLP
    /// 1069.007 D2). An unchanged offset commits nothing.
    func tellAgentOffset() {
        guard let epoch = launchPlace.epoch else { return }
        let zone = TimeZone(identifier: launchPlace.timeZone) ?? TimeZone(secondsFromGMT: 0)!
        let offset = Double(zone.secondsFromGMT(for: Date(timeIntervalSince1970: (epoch + now()) / 1000))) / 60
        apply(runtime.setTime(epochAtZero: epoch, utcOffset: offset))
    }
    /// @ref LLP 1061 D5 — told after every boot, as the date is, and on each
    /// change: in the same main-thread turn as the boot batch, so the first
    /// frame on screen already reads the user's preferences.
    func tellPreferences() {
        guard booted, state != .destroyed else { return }
        apply(runtime.setPreferences(preferenceBits()))
    }
    /// Before a first boot the runtime keeps them, so the first frame is laid
    /// out with the device's preferences rather than a mouse's and then again
    /// (`pointer: none` on tvOS sets a different layout).
    private func primePreferences() {
        guard !booted, state != .destroyed else { return }
        _ = runtime.setPreferences(preferenceBits())
    }
    private func preferenceBits() -> UInt32 {
        #if os(iOS) || os(tvOS)
        // The scene owns system appearance; a window's app override does not.
        if let scene = view?.window?.windowScene {
            systemDark = scene.traitCollection.userInterfaceStyle == .dark
        }
        let dark = systemDark
        #else
        let dark = DisplayPreferences.systemDark
        #endif
        return DisplayPreferences.bits(systemDark: dark)
    }
    /// @ref LLP 1069.000 D2 — told after every boot and on each change; a
    /// change while iOS suspends the process lands with the foreground
    /// notification, in the same turn.
    func tellPage() {
        guard booted, state != .destroyed else { return }
        apply(runtime.setPage(PageFacts.bits))
        apply(runtime.setRootFontSize(PageFacts.rootFontSize))
    }
    public func resize(_ size: CGSize) { guard booted, state != .destroyed else { return }; apply(runtime.resize(width: size.width, height: size.height)) }
    public func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) { guard booted, state != .destroyed else { return }; apply(runtime.insets(top: top, right: right, bottom: bottom, left: left)) }
    /// The device's posture and the viewport segments a fold makes (LLP 1078 D4, D5): the view's reading,
    /// kept for the agent's `layout.env` and told to the kernel and the runner in one batch.
    /// Returns the batch's error, when the runtime refused the grid; the fold is kept only when it took it.
    @discardableResult func segments(_ fold: ViewportFold) -> String? {
        guard booted, state != .destroyed else { return nil }
        let batch = runtime.segments(fold)
        if batch.error == nil { presenter.fold = fold }
        apply(batch)
        return batch.error
    }
    /// The view's appearance, for paint motion's `light-dark()` (LLP 1062).
    public func scheme(dark: Bool) { guard booted, state != .destroyed else { return }; schemeDark = dark; apply(runtime.scheme(dark: dark)) }
    /// The appearance last reported for the session, and each node view
    /// found painting motion in another (a sheet's override, say), by id.
    private(set) var schemeDark: Bool?
    private(set) var viewDark: [UInt32: Bool] = [:]
    private var pendingViewDark: [(UInt32, Bool)] = []
    /// A view painting motion: when its own appearance is not the one its
    /// node's colours resolve by, say so after the batch (LLP 1062 D4).
    func noteAppearance(_ view: NodeView) {
        guard let session = schemeDark else { return }
        let dark = view.drawsDark
        guard dark != viewDark[view.id] ?? session else { return }
        viewDark[view.id] = dark == session ? nil : dark
        pendingViewDark.append((view.id, dark))
    }
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
        let batch = runtime.change(node.id, documentValue(node.id, value), now: now())
        apply(batch)
        return batch.error == nil
    }
    /// Deliver toolbar facts only when the authored editor has a select handler.
    func selection(node: UInt32, json: String) {
        guard booted, state != .destroyed,
              presenter.views[node]?.handlers.contains("select") == true,
              let batch = runtime.selection(node, json: json, now: now()) else { return }
        apply(batch)
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
    public var measureCount: Int { Owner.shared.sync { text.measuring.measureCount } }
    public var measureHits: Int { Owner.shared.sync { text.measuring.measureHits } }
    public var measureSeconds: Double { Owner.shared.sync { text.measuring.measureSeconds } }
    /// The GPU module's and the web arm's status lines (the smoke reads them).
    public var gpuStatus: String { canvases.status }
    public var webStatus: String { webviews.status }
    /// The window's occlusion changed (macOS): the canvases follow.
    public func occlusionChanged() {
        #if os(macOS)
        canvases.occlusionChanged()
        #endif
        frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames)
    }
    /// The scene became active (iOS): the canvases follow.
    public func becameActive() { frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames) }
    #if os(iOS) || os(tvOS)
    /// A hardware keyboard's Tab (or Shift-Tab) when no node of this session
    /// holds the focus: an app's last responder forwards it here, as macOS's
    /// window starts its key-view loop at the view.
    public func moveFocus(backward: Bool) { presenter.moveFocus(backward: backward) }
    #endif

    /// Everything attributable to this session goes (D2): the display link
    /// and clock, the surface instances, the web views, the views, the
    /// runtime — and its handle is a stranger from here on.
    public func destroy() {
        guard state != .destroyed else { return }
        state = .destroyed
        generation += 1
        cancelAuthSessions()
        clockTimer?.invalidate()
        clockTimer = nil
        frames.run(false)
        sampler?.stop()
        DisplayPreferences.forget(preferenceObservers)
        PageFacts.forget(pageObservers)
        presenter.reset()
        ExactSession.live.removeValue(forKey: runtime.rt)
        forgetDocuments()
        rasters.shutdown()
        textPressure?.cancel(); textPressure = nil
        runtime.destroy()
        natives.destroyModule()
        app.forget(self)
    }
}

/// @ref LLP 1043.000 §3 D8 — deadlines, in milliseconds, never a repeating poll.
enum SessionClockTimer {
    enum Wake: Equatable { case none, frame, timeout(Double) }
    static func wake(due: Double?, now: Double, agent: Bool = false) -> Wake {
        guard !agent, let due else { return .none }
        let delay = max(0, due - now)
        return delay <= 8 * 1000 / 60 ? .frame : .timeout(delay)
    }
    static func schedule(after seconds: TimeInterval, _ fire: @escaping @Sendable (Timer) -> Void) -> Timer {
        precondition(Thread.isMainThread)
        let timer = Timer(timeInterval: seconds, repeats: false, block: fire)
        RunLoop.main.add(timer, forMode: .common)
        return timer
    }
}

/// Frames come from the display link, only while motion runs or a canvas
/// has something to render (LLP 1009 D4), per session.
final class Frames: NSObject {
    weak var session: ExactSession?
    var link: CADisplayLink?
    var motion = false, spatial = false
    /// A 2D canvas asked for a frame (LLP 1056 D5): ticks run while it does.
    var canvas2d = false
    var timerSoon = false
    /// A frame task (LLP 1073 D5): each tick is the runtime's frame at the tick's target time.
    var tasks = false
    private var canvasRequested = false

    /// Input and reads ask for one frame; an agent-owned clock never self-reschedules.
    func requestCanvas() {
        guard let s = session else { return }
        if s.clock == nil { run(true); return }
        guard !canvasRequested else { return }
        canvasRequested = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            canvasRequested = false
            guard let s = session, s.state != .destroyed else { return }
            s.canvases.settle(now: s.now())
        }
    }
    @objc func tick(_ link: CADisplayLink) {
        guard let s = session else { return }
        s.canvases.lifecycle.frame()
        // Motion keeps its existing sampling clock; canvas frames target presentation.
        let frameNow = s.clock ?? (link.targetTimestamp - ExactEnv.t0) * 1000
        // ProMotion changes callback cadence (e.g. 120 → 80 Hz) while duration
        // can remain the nominal base interval. The target interval is actual;
        // canvases quantizes it and republishes this session’s stable class before rendering.
        s.canvases.period((link.targetTimestamp - link.timestamp) * 1000)
        let previous = s.canvases.frameNow
        s.canvases.frameNow = frameNow
        defer { s.canvases.frameNow = previous }
        // Frame-precise timers: the first frame at or past the deadline fires
        // them. A frame before it would advance to no timer, and its commit
        // and presenter pass cost a list in motion a report a frame.
        // A slice building on the owner holds this frame's timers and tick:
        // they run on the first frame after it lands (LLP 1072 §7.1).
        if !s.fillInFlight {
            if timerSoon, !ExactEnv.agentMode, s.clock == nil {
                let now = s.now()
                s.followOffset()
                if tasks { s.apply(s.runtime.frame(now: frameNow)) }
                else if s.timerDue.map({ now >= $0 }) ?? true { s.apply(s.runtime.advance(now: now)) }
            }
            if motion || canvas2d {
                if ExactSession.asyncFills {
                    if !s.tickInFlight { s.sendTick(now: s.now()) }
                } else {
                    s.apply(s.runtime.tick(now: s.now()))
                }
            }
        }
        let more = s.canvases.tick(now: frameNow)
        run(motion || canvas2d || timerSoon || more || s.canvases.wantsFrames || s.canvases.lifecycle.needsRetry)
    }

    func run(_ wanted: Bool) {
        if wanted, session?.clock != nil { requestCanvas() }
        let on = wanted && session?.clock == nil
        if on, link == nil {
            #if canImport(UIKit)
            let l = CADisplayLink(target: self, selector: #selector(tick(_:)))
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
        #if canImport(UIKit)
        if let link {
            // Motion that changes place or size asks for the panel's full
            // rate while it runs, as a canvas does: at `.default` a ProMotion
            // iPhone presents a slide at 60 Hz. A fade or a colour change
            // reads the same at 60, so paint-only motion — a breathing loop
            // that runs for minutes — asks no more. The link exists only
            // while something wants frames, so an idle app drops to no link
            // at all (LLP 1061 D4).
            let fullRate = (motion && spatial) || session?.canvases.wantsFrames == true
            let rate = Float(min(120, session?.presenter.viewport.window?.screen.maximumFramesPerSecond ?? 60))
            link.preferredFrameRateRange = fullRate ? CAFrameRateRange(minimum: min(80, rate), maximum: rate, preferred: rate)
                : motion ? CAFrameRateRange(minimum: min(30, rate), maximum: min(60, rate), preferred: min(60, rate)) : .default
        }
        #endif
    }
}

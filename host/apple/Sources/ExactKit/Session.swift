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
    /// The process's environment. A production bake never enters agent mode
    /// (LLP 1069.007 D2, ruled) and takes no module override (LLP 1024 D5):
    /// its `EXACT_AGENT`, `EXACT_AGENT_*`, `EXACT_HATCHES` and
    /// `EXACT_MODULES` variables are dropped here, from the process too,
    /// before this host or its library reads one.
    public static let environment: [String: String] = {
        var environment = ProcessInfo.processInfo.environment
        let agent = environment.keys.filter { ["EXACT_AGENT", "EXACT_HATCHES", "EXACT_MODULES"].contains($0) || $0.hasPrefix("EXACT_AGENT_") }.sorted()
        guard !agent.isEmpty, productionBake else { return environment }
        FileHandle.standardError.write(Data("exact: a production build ignores \(agent.joined(separator: ", ")) (LLP 1069.007 D2, LLP 1024 D5)\n".utf8))
        for key in agent { environment[key] = nil; unsetenv(key) }
        return environment
    }()
    /// Whether this binary was baked with `EXACT_UPDATE_TRUST=production`
    /// (`BakedCompatibility.production`).
    static let productionBake = BakedCompatibility.production(BakedCompatibility.json)
    public static let agentMode = environment["EXACT_AGENT"] == "1"
    /// `EXACT_AGENT_BIND` (`--bind`, LLP 1133.001 D7): extensions answered by their implementations under the agent.
    static let agentBind = Set((environment["EXACT_AGENT_BIND"] ?? "").split(separator: ",").map(String.init))
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
    private(set) package var resolver: AssetResolver!
    private var transaction = false
    /// A retryable image preparation; the current sessions remain live.
    public private(set) var generationPending = false; private(set) var requestsHeld = false
    private var notifications: [() -> Void] = []
    func deliver(_ body: @escaping () -> Void) {
        if transaction { notifications.append(body) } else { body() }
    }
    /// Retained for the app lifetime; embedded-only apps supply none.
    public var lifecycle: ExactAppLifecycle?
    private(set) var selectedToken: UInt64 = 0
    private var sessionRefs: [WeakSession] = []
    let notificationProcessID = UUID().uuidString
    let sharedStore = SharedStoreRefresh()
    /// Every live session, in creation order.
    public var sessions: [ExactSession] { sessionRefs.compactMap(\.session) }
    /// The one dev connection (D11): the app URL `dev.mjs` prints, resolved
    /// and subscribed once; every `{seq}` applies to every session.
    private(set) var connection: PlanURL?
    private init() {
        WidgetContainer.configure()
        #if canImport(UIKit)
        let fallback = Bundle.main.bundlePath
        #else
        let fallback = Bundle.main.bundleURL.pathExtension == "app"
            ? (Bundle.main.resourcePath ?? Bundle.main.bundlePath) : FileManager.default.currentDirectoryPath
        #endif
        assetRoot = URL(fileURLWithPath: ExactEnv.environment["EXACT_ASSETS"] ?? fallback, isDirectory: true)
        resolver = AssetResolver(root: assetRoot)
        observeSharedStore()
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

    func firstPixel(_ token: UInt64) {
        lifecycle?.firstPixel(self, token: token)
        WidgetContainer.publishAfterFirstPixel(resolver)
    }

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
        connection = PlanURL.open(url, program: devProgram, acceptProgram: { [weak self] program in
            guard let self else { return false }
            if let old = self.devProgram { return old == program }
            self.devProgram = program
            return true
        }, current: { [weak self] in self?.devGeneration }, plan: { [weak self] in self?.sessions.first { $0.booted && $0.state != .destroyed }?.runtime.planDigest() }, waiting: { [weak self] in self?.generationPending == true }, held: { [weak self] in self?.requestsHeld == true }, design: { [weak self] in self?.design($0) ?? "closed" }, channel: { [weak self] in self?.channel($0) }, apply: { [weak self] candidate, label, letGo in
            guard let self else { return false }
            let resolver = candidate.assets
            return self.applyTogether(candidate.plan, label: label, resolver: resolver, token: 0, identity: candidate.identity, module: candidate.module, letGo: letGo, commit: { true })
        })
        connection?.onDevStatus = { [weak self] in self?.showDevStatus($0) } // the old one's went at its close
    }

    public func disconnect() {
        connection?.close()
        connection = nil
    }

    /// The connection's page URL, when connected (what a deck's `//` source resolves against).
    public var connectedPage: URL? { connection?.page }
    /// The connection's status line for a dev menu, or nil.
    public var connectionStatus: String? { connection.map { "\($0.page)\($0.devStatus.map { " (\($0.said))" } ?? ($0.terminal != nil ? " (rebuild the host)" : generationPending ? " (preparing Rust update)" : " (live)"))" } }
    /// Re-resolve the connection (the menu's Reload; clears a `{rebuilt}` stop).
    public func reloadConnection() { connection?.reload() }

    /// A plan is one app generation: every session accepts before any swaps.
    @discardableResult
    public func apply(_ bytes: Data, label: String = "plan") -> Bool {
        applyTogether(bytes, label: label, resolver: resolver, token: 0, commit: { true })
    }

    /// Every session accepts before the durable commit. Its callback runs on
    /// main while the runtime owner holds replies; it cannot reenter a runtime.
    @discardableResult
    public func applyGeneration(_ candidate: ExactGeneration, label: String = "generation", commit: () -> Bool) -> Bool {
        applyTogether(candidate.plan, label: label, resolver: candidate.assets, token: candidate.token, module: candidate.module, commit: commit)
    }

    private func applyTogether(_ bytes: Data, label: String, resolver candidateResolver: AssetResolver, token: UInt64, identity: String? = nil, module: ExactModule? = nil, letGo: Bool = false, commit: () -> Bool) -> Bool {
        generationPending = false; requestsHeld = false
        guard !transaction else { return false }
        let module = module ?? lastModule
        let participants = sessions.filter { $0.state != .destroyed }
        // Validate a staged plan without starting a hidden runner's requests.
        if participants.isEmpty {
            let validator = ExactSession(app: self, label: "")
            defer { validator.destroy() }
            guard validator.prepare(bytes, resolver: candidateResolver, token: token, module: module, size: CGSize(width: 1, height: 1), letGo: letGo) != nil else { return false }
            validator.runtime.discardPlan()
        }
        guard let prepared = prepareGeneration(participants, prepare: { session in
            session.prepare(bytes, resolver: candidateResolver, token: token, module: module, letGo: letGo)
        }) else {
            generationPending = participants.contains { $0.modulePending || $0.requestsPending }
            requestsHeld = participants.contains { $0.requestsPending }
            for session in participants { session.runtime.discardPlan() }
            return false
        }
        let surfaces = SurfacesLink.installed
        let shaderSources = surfaces?.shadersLoaded == true ? candidateResolver.shaderSources() : nil
        let shadersAccepted = shaderSources.map { surfaces?.acceptsShaders($0) == true } ?? true
        guard candidateResolver.refusal == nil, shadersAccepted else {
            for (session, _) in prepared { session.runtime.discardPlan() }
            return false
        }
        for (session, candidate) in prepared { session.configureCommit(candidate) }
        transaction = true
        let result = Runtime.commitTogether(prepared.map { $0.0.runtime }, commit: commit)
        guard let committed = result.batches else {
            transaction = false; generationPending = result.pending; requestsHeld = result.requests
            for (session, _) in prepared { session.cancelCommit() }
            return false
        }
        let batches = zip(prepared, committed).map { pair, batch in (pair.0, pair.0.commit(pair.1, batch: batch)) }
        resolver = candidateResolver
        #if EXACT_WIDGET_GALLERY
        NotificationCenter.default.post(name: Notification.Name("ExactWidgetGeneration"), object: self)
        #endif
        devGeneration = identity
        lastPlan = bytes
        lastModule = module
        selectedToken = token
        if let shaderSources { surfaces?.replaceShaders(shaderSources) }
        for (session, _) in batches { session.awaitingTree = true; session.runtime.held = true } // each report waits for its own tree: one tree may retint the window; each runtime held till its own reset ends (ResetGate)
        MainActor.assumeIsolated { HeldState.swap() } // the hold let go unsaid before the old tree goes, real input none till after (HeldState.swift)
        for (session, batch) in batches { session.presentCommitted(batch, label: label) }
        for session in participants { session.apply(session.runtime.deliverySync()) }
        MainActor.assumeIsolated { HeldState.swapping = false }
        rehold()
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
    let notificationOwnerID: String
    /// Under the agent the session's own pasteboard, which `copyText` and @exact/clipboard write, registered for
    /// removal when the session is made, so a process that exits removes it too (AgentPasteboard.swift).
    let agentPasteboard: String?
    public weak var delegate: ExactSessionDelegate?
    public private(set) var state: State = .created {
        didSet { if state != oldValue { let changed = state; app.deliver { [weak self] in guard let self else { return }; delegate?.exactSession(self, didChange: changed) } } }
    }
    /// Bumped by every reboot; a callback from an older generation is dropped.
    public private(set) var generation = 0
    let storeRefresh = SessionStoreRefresh()
    var activatedGeneration: Int?
    /// The generation whose deferred data has activated (or failed to): the
    /// agent's `clock data` waits for it before a test's first step.
    var dataGeneration: Int?
    var dataActivated: Bool { dataGeneration == generation }
    var pendingActivation: (generation: Int, token: UInt64)? // retried on the session's wake
    /// The generation whose data source `prepareData` activated on the owner, and whether it still runs there.
    var preparedGeneration: Int?, dataPreparing = false
    var nativesOpened: Int? // the generation whose native views may attach (LLP 1024 D3)
    private(set) var updateToken: UInt64 = 0; private var receipt: DrawReceipt?
    package let core: SessionCore
    package var runtime: Runtime { core.runtime }
    /// This session's file roots, captured outside a native module callback.
    var appFileRoots: [String: String] = [:]
    let rasters = RasterLoader()
    #if os(macOS)
    lazy var regions = RegionController(self)
    #endif
    var text: TextEngine { get { core.text } set { core.text = newValue } }
    let fieldChrome = FieldChromeCache()
    let buttonMeasurements = ButtonMeasureCache()
    package let presenter: Presenter
    var launchLocation: String? // a pre-boot `openURL`'s location, until the first frame (LaunchURL.swift)
    var pendingLaunchLocations: [String] = []
    var heldModuleEvents: [String] = [] // module events before first pixel (SessionActivation.swift)
    var protocolHandlers: [[String: String]] = ExactEnv.appMetadata["ExactProtocolHandlers"] as? [[String: String]] ?? []
    private var textPressure: DispatchSourceMemoryPressure?
    let canvases: Canvases
    let webviews: WebViews
    let natives = NativeViews()
    /// The file picker (LLP 1069.002).
    lazy var picker = Picker(session: self)
    /// The voice table's output (LLP 1096 D8); settable for tests.
    lazy var sound = SoundOutput(self)
    lazy var midi = MIDIOutput(self)
    var activityOutput: ActivityOutput?
    package let frames: Frames
    /// A session's presented frames (LLP 1079 D3); a production bake has one only while an app records.
    var sampler: FrameSampler?
    var clockTimer: Timer?
    /// The runner deadline `clockTimer` fires for.
    var clockDue: Double?
    /// The last two timer advances changed nothing the presenter shows,
    /// and no batch has applied since (`scheduleClock`).
    var timersQuiet: Bool { quietTimerAdvances >= 2 }
    private(set) var quietTimerAdvances = 0
    /// An advance of a deadline already due is running (`advanceIfDue`)
    var advancingDue = false
    /// The agent's clock (milliseconds) when the driver owns time; nil runs
    /// on the wall clock. Taking it over ends the first-frame rule (LLP
    /// 1003.001 D7): what waits for a frame starts there.
    public var clock: Double? {
        didSet { runtime.liveClock(clock == nil); if oldValue == nil, let at = clock { apply(runtime.startOnFrame(false, at: at)) } }
    }
    /// The runner's soonest timer, from the last batch (absent without timers).
    var timerDue: Double?
    /// A data module's timer continuation an advance released is still out (the batch's `landing`).
    var timerLanding = false
    /// The view presenting this session, while one is mounted (D1).
    weak package var view: ExactView? { didSet { if view !== oldValue { natives.sessionViewChanged() } } }
    #if os(iOS) || os(tvOS)
    private var systemDark = false
    var launchViewport: (plan: Data?, cover: Bool)?
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
    private(set) var applying = false
    // @ref LLP 1072 §3 — a collection slice built on the owner thread. While
    // one is in flight, work that would wait behind it on the owner (a
    // frame's tick, a timer, a reply, an intrinsic size, a report) waits on
    // main instead, and runs after the slice lands, in order.
    private(set) var fillInFlight = false
    private typealias IdleWork = (views: Int?, work: () -> Void, cancelled: (() -> Void)?)
    private var afterFill: [IdleWork] = []
    private var viewsEpoch = 0
    private let published = NSLock()
    /// What the owner committed without main waiting.
    enum Published { case fill(UInt32), tick, advance, canvas, notice, reply, cancelledReply }
    /// Batches the owner committed without main waiting, in its order: a
    /// slice's (with its list), a frame's tick's and a canvas draw's.
    private var publishedQueue: [(kind: Published, batch: Batch, generation: Int)] = []
    /// A frame's tick is on the owner (LLP 1072 §7.1): the next one waits.
    var tickInFlight = false
    /// A timer advance is on the owner (LLP 1072.000): the next one waits.
    var advanceInFlight = false
    /// Canvas draws run in a turn of their own on the owner (LLP 1072 §8.5):
    /// the last batch applied says whether one is owed, one is in flight
    /// at a time, and one is asked for once per main-queue turn.
    private(set) var canvasInFlight = false
    /// A system notice's batches on the owner (`tellSoon`), each landing in its order.
    var noticesInFlight = 0
    var repliesInFlight = 0
    var replySubmissions: [Double] = []
    /// Owner work main did not wait for, whose batches land before any made later (T4).
    var ownerInFlight: Bool { fillInFlight || tickInFlight || advanceInFlight || canvasInFlight || noticesInFlight > 0 || repliesInFlight > 0 }
    /// Whether this generation's plan reads `onLine`, learned by a synchronous `tellPage`.
    var readsOnLineKnown: (generation: Int, value: Bool)?
    private var canvasOwed = false
    private var canvasAsked = false
    private(set) var landing = false
    /// Landing slices ahead of a batch already made: their appearance
    /// reports wait for it, so no batch made later applies before it.
    private(set) var holdingReports = false
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
    package var isApplyingPresentation: Bool { applying }
    // Weak live gesture ownership only; no historical tokens or row registry.
    private let inputHolds = NSHashTable<SwipeHold>.weakObjects()
    weak package var heightInputHold: DragInput?
    weak package var transformInputHold: DragInput?
    func trackInputHold(_ hold: SwipeHold) { inputHolds.add(hold) }
    func retireInputHold(_ hold: SwipeHold) { inputHolds.remove(hold) }
    private var pendingSurfaceRecords: [(String, String?)] = []
    private var pendingSurfaceWork: [([String: Any], Int)] = []
    /// Authentication sessions this session opened (`Auth.swift`, LLP 1069.006).
    let authSessions = AuthSessions()

    /// Live sessions by handle: what a wake looks up (a stranger's is dropped).
    nonisolated(unsafe) private static var live: [ExactRuntime: WeakSession] = [:]
    private var preferenceObservers: [NSObjectProtocol] = []
    private var pageObservers: [NSObjectProtocol] = []
    #if os(macOS)
    private var colorObserver: NSObjectProtocol?
    #endif

    init(app: ExactApp, label: String, launchPlace: LaunchPlace = LaunchPlace()) {
        let owner = UUID().uuidString
        notificationOwnerID = owner; agentPasteboard = AgentPasteboard.name(owner)
        self.launchPlace = launchPlace
        self.app = app
        self.label = label
        let runtime = Runtime()
        PresenceKeychain.install(); LocalNetworkFact.install() // before the first boot reads the backend (LLP 1133.010 D6), the fact (LLP 1133.036 D8)
        if ActivityLibrary.packaged { runtime.linkActivities() }
        // Motion starts at the first frame that shows it, until an agent owns the clock (LLP 1003.001 D7).
        if !ExactEnv.agentFreezes { _ = runtime.startOnFrame(true, at: 0) }
        core = SessionCore(runtime: runtime, text: TextEngine.pair(resolve: { [weak app] source in app?.resolveAsset(source) }, read: { [weak app] source in app?.assetBytes(source) },
                               bundled: { [weak app] source in app?.bundledAsset(source) }))
        presenter = Presenter()
        canvases = SurfacesLink.installed?.canvases() ?? NoCanvases()
        webviews = WebViews()
        frames = Frames()
        presenter.session = self
        canvases.session = self
        webviews.session = self
        natives.session = self
        frames.session = self
        sampler = FrameSampler.fromBoot(self)
        core.measure(with: text)
        installControlText()
        // LLP 1056 D8, D9: Canvas 2D measures with this engine and draws the
        // handles this session decodes.
        if let measure = SurfacesLink.installed?.canvasTextMeasure { runtime.setCanvasText(measure) }
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
        preferenceObservers = DisplayPreferences.observe { [weak self] in self?.tellPreferencesSoon() }
        pageObservers = PageFacts.observe { [weak self] in self?.tellPageSoon() }
        #if os(macOS)
        // @ref LLP 1095 D5 — the accent or another system colour changed:
        // what was resolved goes, the report is made again, and every view
        // applies its colours again.
        colorObserver = NotificationCenter.default.addObserver(forName: NSColor.systemColorsDidChangeNotification, object: nil, queue: .main) { [weak self] _ in
            guard let self, state != .destroyed else { return }
            controlTextChanged()
            SystemColor.invalidate()
            reportColors()
            presenter.views.values.forEach { $0.systemColorsChanged() }
        }
        #endif
    }

    deinit { destroy() }

    /// The live session behind a runtime handle, or nil for a stranger's.
    static func session(for rt: ExactRuntime) -> ExactSession? { live[rt]?.session }

    private static let wake: ExactWakeFn = { ctx in
        let rt = ExactRuntime(UInt(bitPattern: ctx))
        DispatchQueue.main.async {
            guard let s = ExactSession.live[rt]?.session else { return }
            s.whenIdle { [weak s] in s?.pumpReplies() }
        }
    }

    /// The app's clock: what events, timers, motion, and canvases see.
    public func now() -> Double { clock ?? ExactEnv.wall() }

    /// View reports expire after reset; executor work survives a plan commit.
    package func whenIdle(bound: Bool = false, _ work: @escaping () -> Void, cancelled: (() -> Void)? = nil) {
        idle((bound ? viewsEpoch : nil, work, cancelled))
    }
    private func idle(_ item: IdleWork) {
        if state == .destroyed || (item.views != nil && item.views != viewsEpoch) { item.cancelled?() }
        else if fillInFlight { afterFill.append(item) }
        else { item.work() }
    }

    /// Send a build-only slice to the owner; its batch comes back published.
    private func sendFill(_ view: UInt32, _ bytes: Data) {
        fillInFlight = true
        let captured = generation
        runtime.collectionFeedbackAsync(bytes, now: now()) { [weak self] batch in
            self?.publish(.fill(view), batch, captured)
        }
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
    func publish(_ kind: Published, _ batch: Batch, _ generation: Int) {
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
        // A reply the slices released (`afterFill`) may have left a deadline
        // due: it advances once they have landed, unless a batch waits on them
        defer { landing = false; if !holdingReports { advanceIfDue() } }
        while true {
            published.lock()
            let next = publishedQueue.isEmpty ? nil : publishedQueue.removeFirst()
            published.unlock()
            guard let next else { return }
            let began = CACurrentMediaTime()
            var submitted = 0.0
            switch next.kind {
            case .fill: fillInFlight = false
            case .tick: tickInFlight = false
            case .advance: advanceInFlight = false
            case .canvas: canvasInFlight = false
            case .notice: noticesInFlight -= 1
            case .reply, .cancelledReply: submitted = retireReply()
            }
            if case .cancelledReply = next.kind { continue }; if next.batch.held { if case .fill(let view) = next.kind { landedFill(view, stale: true) }; continue } // (held over a reset: as stale, ResetGate replays its work)
            guard state != .destroyed, next.generation == generation else {
                if case .reply = next.kind { retryActivation(generation: next.generation) }
                if case .fill(let view) = next.kind { landedFill(view, stale: true) }
                continue
            }
            let batch = next.batch
            if case .advance = next.kind { applyUnlessEmpty(batch, timer: true) } else if !(changesNothing(batch) && batch.timerDueMs == timerDue) {
                apply(batch)
            }
            if case .reply = next.kind {
                sampler?.replyPump(main: submitted + (CACurrentMediaTime() - began) * 1000)
                retryActivation(generation: next.generation)
            }
            // A draw owed while this one ran, or one a tick left owed.
            if canvasOwed { askCanvasDraw() }
            guard case .fill(let view) = next.kind else { continue }
            landedFill(view)
        }
    }

    /// A stale slice can land inside a commit, before its replacement tree.
    private func landedFill(_ view: UInt32, stale: Bool = false) {
        let waiting = afterFill; afterFill.removeAll()
        if state != .destroyed { presenter.collections.landed(view, at: ProcessInfo.processInfo.systemUptime) }
        guard stale else { for item in waiting { idle(item) }; return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { waiting.forEach { $0.cancelled?() }; return }
            for item in waiting { idle(item) }
        }
    }

    /// Land every session's slice in flight.
    static func drainAll() { for weak in live.values { weak.session?.drainFill() } }

    /// Wait for the slice in flight and land it: what shows, the agent and
    /// every read that must see it (T5, T7, T9).
    public func drainFill() {
        guard ownerInFlight || hasPublished else { return }
        Owner.shared.sync {}
        landFill()
    }
    private func wire() {
        runtime.topLayer = { [weak self] in self?.presenter.openTopLayer ?? ([], nil) }
        runtime.setInlineGeometry({ [weak self] id in self?.presenter.inlineFrame(id) },
                                  range: { [weak self] id, start, end in self?.presenter.rangeFrame(id, start: start, end: end) })
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
            if changesNothing(batch) && batch.timerDueMs == timerDue { return }
            apply(batch)
        }
        presenter.onClose = { [unowned self] id in apply(runtime.dialogClose(id, now: now())) }
        presenter.onPress = { [unowned self] id in applyInput(runtime.press(id, held: presenter.pressHeld, at: presenter.pressPoint, now: now())) }
        // A text field's `input` and `change` carry the selection the edit
        // left, its `select` the one the person or a script made (x2apps
        // codeedit #2, `FieldSelections`).
        presenter.onChange = { [unowned self] id, value in
            let value = documentValue(id, value)
            applyInput(runtime.change(id, value, selection: presenter.fieldSelections.reported(id, value), now: now()))
        }
        presenter.onInput = { [unowned self] id, value in
            if let edit = presenter.fieldSelections.input(id), let selection = presenter.fieldSelections.reported(id, value) {
                applyInput(runtime.fieldEvent(id, 54, edit.payload(value, selection), now: now()))
            } else { applyInput(runtime.input(id, value, selection: presenter.fieldSelections.reported(id, value), now: now())) }
        }
        presenter.fieldSelections.onReplacement = { [unowned self] id, value, selection in apply(runtime.fieldEvent(id, 54, (presenter.fieldSelections.input(id) ?? FieldInput(inputType: "insertReplacementText", cancelable: false)).payload(value, selection), now: now())) }
        presenter.fieldSelections.onChange = { [unowned self] id, value, selection in apply(runtime.fieldEvent(id, 52, selection.payload(value), now: now())) }
        presenter.fieldSelections.onBeforeInput = { [unowned self] id, value, selection, edit in applyInput(runtime.fieldEvent(id, 53, edit.payload(value, selection), now: now())) }
        presenter.fieldSelections.onSelect = { [unowned self] id, value, selection in apply(runtime.fieldSelect(id, value, selection, now: now())) }
        // @ref LLP 1069.001 D4 — a toggle is HTML's `input` then `change`,
        // each where the node hears it.
        presenter.onChecked = { [unowned self] id, on in
            let handlers = presenter.views[id]?.handlers ?? []
            if handlers.contains("input") { applyInput(runtime.checked(id, on, commit: false, now: now())) }
            if handlers.contains("change") { applyInput(runtime.checked(id, on, commit: true, now: now())) }
        }
        presenter.onControlValue = { [unowned self] id, value, input, change in
            let handlers = presenter.views[id]?.handlers ?? []
            if input, handlers.contains("input") { applyInput(runtime.input(id, value, now: now())) }
            if change, handlers.contains("change") { applyInput(runtime.change(id, value, now: now())) }
        }
        presenter.selectOptions = { [unowned self] id in runtime.selectOptions(id) }
        presenter.controls.radioGroup = { [unowned self] id in runtime.radioGroup(id) }
        presenter.buttonFace = { [unowned self] id in runtime.buttonFace(id) }
        presenter.onIntrinsic = { [unowned self] sizes in whenIdle(bound: true) { [unowned self] in applyUnlessEmpty(runtime.intrinsics(sizes)) } }
        // @ref LLP 1075.003 §3.5, Q3 (c) — what a bar covers reaches layout as an intrinsic
        // size does, on AppKit too (LLP 1115.002 D8.1); the hatches replay once the module connects.
        presenter.onCovers = { [unowned self] covers in whenIdle(bound: true) { [unowned self] in applyUnlessEmpty(runtime.covers(covers)) } }
        #if os(iOS) || os(tvOS)
        natives.onHatchesConnected = { [weak self] in
            self?.presenter.navigation.replayHatches()
            self?.presenter.elements.replay()
        }
        #else
        natives.onHatchesConnected = { [weak self] in
            self?.presenter.elements.replay()
            self?.presenter.toolbar.hatchToolbar()
        }
        presenter.rootTabs = { [unowned self] id in runtime.rootTabs(id) }
        #endif
        presenter.onHover = { [unowned self] id, over in applyInput(runtime.hover(id, over: over, now: now())) }
        presenter.onFocus = { [unowned self] id in apply(runtime.focus(id, now: now())) }
        presenter.onBlur = { [unowned self] id in apply(runtime.blur(id, now: now())) }
        presenter.onFocusEvent = { [unowned self] id, kind, target, related in
            apply(runtime.focusEvent(id, entering: kind == "focusin", target: target, related: related, now: now()))
        }
        presenter.onKey = { [unowned self] id, press in applyInput(runtime.key(id, press.payload, up: press.up, capture: press.capture, now: now())) }
        presenter.onContextmenu = { [unowned self] id in applyInput(runtime.contextmenu(id, now: now())) }
        presenter.onSwiperight = { [unowned self] id in applyInput(runtime.swiperight(id, now: now())) }
        presenter.onRefresh = { [unowned self] id in applyInput(runtime.refresh(id, now: now())) }
        presenter.onPan = { [unowned self] id, dx, dy in applyInput(runtime.pan(id, dx: dx, dy: dy, now: now())) }
        presenter.onPanRelease = { [unowned self] id, vx, vy in applyInput(runtime.panRelease(id, vx: vx, vy: vy, now: now())) }
        presenter.onPanSample = { [unowned self] first, x, y, t in runtime.panSample(first: first, x: x, y: y, t: t) }
        presenter.panVelocity = { [unowned self] t in runtime.panVelocity(at: t) }
        presenter.onScroll = { [unowned self] id, metrics in applyUnlessEmpty(runtime.scroll(id, metrics: metrics, now: now()), scrolled: true) }
        presenter.onScrollend = { [unowned self] id in applyUnlessEmpty(runtime.scrollend(id, now: now())) }
        presenter.onScrolled = { [unowned self] id, left, top in runtime.scrolled(id, left: left, top: top) }
        #if canImport(AppKit)
        presenter.onListIndex = { [unowned self] id, key in runtime.listIndex(id, key: key) }
        presenter.onListText = { [unowned self] id, first, last in runtime.listText(id, first: first, last: last) }
        #endif
        presenter.onDblclick = { [unowned self] id in applyInput(runtime.dblclick(id, now: now())) }
        presenter.onPointer = { [unowned self] id, kind, sample in applyInput(runtime.pointer(id, kind, sample, now: now())) }
        presenter.onSubmit = { [unowned self] id in applyInput(runtime.submit(id, now: now())) }
        presenter.onLoad = { [unowned self] id in apply(runtime.load(id, now: now())) }
        presenter.onMessage = { [unowned self] id, value in apply(runtime.message(id, value, now: now())) }
        presenter.onClipboard = { [unowned self] id, kind, text in let batch = runtime.clipboard(id, kind, text, now: now()); if kind == 50 { apply(batch) } else { applyInput(batch) } } // 50: a popover's `toggle`, which an app's command fires too
        presenter.onSelectionChange = { [unowned self] id, text, start, end, rects in
            apply(runtime.selectionChange(id, text, start: start, end: end, rects: rects, now: now()))
        }
        // Commands are queued here and delivered once the batch is applied
        // (D2): a delegate then runs against a settled tree.
        presenter.onCommand = { [unowned self] name, args, source in pendingCommands.append((name, args, source)) }
    }

    /// Boot the plan baked into the library (or `EXACT_PLAN`'s file, an
    /// adapter's choice, through `boot(plan:)`) under a viewport.
    @discardableResult
    public func boot(size: CGSize) -> Batch { boot(size: size, fallbackSize: nil) }

    @discardableResult
    func boot(size: CGSize, fallbackSize: (() -> CGSize)?) -> Batch {
        var size = size
        let t = CACurrentMediaTime()
        openStartLocation()
        primePreferences()
        primeTime(); claimColdShare()  // `prepare` primes again; the fallback boot reads this
        if let bytes = app.lastPlan {
            // A selected launch can crash in runner/font/asset preparation.
            // Record the attempt first; an integrity refusal clears it below.
            if app.selectedToken != 0 { app.lifecycle?.generationStarted(app, token: app.selectedToken) }
            if let candidate = prepare(bytes, resolver: app.resolver, token: app.selectedToken, module: app.lastModule, size: size), let batch = commit(candidate) {
                presentCommitted(batch, label: "selected")
                return batch
            }
            let reason = app.resolver.refusal ?? "initial plan refused"
            if !app.fallBackFromInitial(reason: reason) {
                return finishBoot(Batch(ops: [], timers: false, motion: false, clock: nil, error: reason), started: t)
            }
            size = fallbackSize?() ?? size
        }
        let cp = text.checkpoint()
        runtime.setProfileResolver(app.resolver)
        let batch = runtime.boot(width: size.width, height: size.height)
        if batch.error != nil { text.restore(cp) }
        return finishBoot(batch, started: t)
    }

    /// Boot from plan bytes as the first boot (a fixture, `EXACT_PLAN`).
    @discardableResult
    public func boot(plan bytes: Data, size: CGSize) -> Batch {
        let t = CACurrentMediaTime()
        openStartLocation()
        primePreferences()
        primeTime()
        let cp = text.checkpoint()
        runtime.setProfileResolver(app.resolver)
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
            // A fresh boot over a running app (the dev menu's reload from the baked plan) starts the views over; the library already replaced its host.
            routerOp = nil
            generation += 1
            if booted { awaitingTree = true; resetHeld(); viewsEpoch += 1; forgetAppearances(); presenter.launchAutofocusReleased = false } // a fresh boot's autofocus waits again
            booted = true; app.liveBooted(self)
            text.commitFonts()
            appFileRoots = AppFiles.learn(runtime) // before the first frame's `app:/` images load (LLP 1069.002 D7)
        }
        apply(batch)
        treeIsIn()
        if batch.error == nil { tellTime(); refuseUnheardLaunch() }
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

    private(set) var modulePending = false, requestsPending = false
    /// While a carried restart applies its tree, autofocus waits; the restart
    /// then puts focus back at its place (`Presenter.restoreFocus`).
    var autofocusHeld = false
    private var keptFocus: FocusPlace?

    func prepare(_ bytes: Data, resolver: AssetResolver, token: UInt64 = 0, module: ExactModule? = nil, size: CGSize? = nil, letGo: Bool = false) -> Prepared? {
        modulePending = false; requestsPending = false
        guard state != .destroyed else { return nil }
        // The running tree's focus, read before the candidate replaces it.
        keptFocus = booted ? presenter.focusPlace(tree: agent("{\"op\":\"tree\"}")) : nil
        let module = module ?? app.lastModule
        runtime.setProfileResolver(resolver)
        defer { runtime.setProfileResolver(app.resolver) }
        let candidate = TextEngine.pair(resolve: { resolver.url($0) }, read: { resolver.bytes($0) }, bundled: { resolver.bundledURL($0) })
        core.measure(with: candidate)
        installControlText(on: candidate)
        let viewport = size ?? presenter.viewportSize
        primeTime()
        let batch: Batch
        if let module { batch = runtime.prepareModule(bytes, module: module, token: token, width: viewport.width, height: viewport.height, letGo: letGo) }
        else { batch = runtime.preparePlan(bytes, width: viewport.width, height: viewport.height, token: token) }
        core.measure(with: text)
        installControlText()
        requestsPending = batch.requests; if batch.pending { modulePending = true; return nil }
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
            if !batch.requests { fputs("exact: prepare refused: \(error)\n", stderr) } // asked again every 50 ms, unlogged
            return nil
        }
        return Prepared(text: candidate, resolver: resolver, token: token)
    }

    func commit(_ candidate: Prepared) -> Batch? {
        configureCommit(candidate)
        let result = Runtime.commitTogether([runtime], commit: { true })
        guard let batch = result.batches?.first else {
            modulePending = result.pending; requestsPending = result.requests
            cancelCommit(); return nil
        }
        return commit(candidate, batch: batch)
    }

    func configureCommit(_ candidate: Prepared) {
        runtime.setProfileResolver(candidate.resolver)
        core.measure(with: candidate.text)
        installControlText(on: candidate.text)
    }

    func cancelCommit() {
        runtime.discardPlan()
        runtime.setProfileResolver(app.resolver)
        core.measure(with: text)
        installControlText()
    }

    func commit(_ candidate: Prepared, batch: Batch) -> Batch {
        precondition(batch.error == nil, "an accepted session candidate must remain commit-ready")
        text = candidate.text
        text.commitFonts()
        updateToken = candidate.token
        generation += 1
        return batch
    }

    func presentCommitted(_ batch: Batch, label: String) {
        routerOp = nil
        let restart = booted, kept = keptFocus
        keptFocus = nil
        awaitingTree = true // the tint a reset gives back is reported with the new tree
        resetHeld(); viewsEpoch += 1 // the teardown reaches no runtime; what it set off waits for `treeIsIn` (ResetGate)
        forgetAppearances()
        booted = true; app.liveBooted(self)
        app.lifecycle?.generationStarted(app, token: updateToken)
        autofocusHeld = restart
        sampler?.reset() // a new runner numbers its transactions afresh (LLP 1079 D3)
        appFileRoots = AppFiles.learn(runtime)
        apply(batch)
        treeIsIn()
        tellTime()
        refuseUnheardLaunch()
        view?.rebooted()
        autofocusHeld = false
        if restart { presenter.restoreFocus(kept, tree: agent("{\"op\":\"tree\"}")) }
        state = .ready
        if restart { fputs("reloaded \(label)\n", stderr) } // a launch generation's first boot is no reload
    }

    /// A host may replace one session's plan transactionally; app delivery
    /// uses prepare/commit across every attached session instead.
    @discardableResult
    public func apply(_ bytes: Data, label: String = "plan") -> Bool {
        guard let candidate = prepare(bytes, resolver: app.resolver), let batch = commit(candidate) else { return false }
        app.invalidateDevGeneration()
        presentCommitted(batch, label: label)
        return true
    }

    /// A batch into the presenter; frames and the clock follow it; its
    /// commands go to the delegate after it.
    /// @ref LLP 1038 D7/D11 — observation only; Swift never interprets slots.
    private(set) var routerOp: [String: Any]?

    package func surfaceRecord(_ name: String, _ json: String?) {
        guard state != .destroyed else { return }
        if applying { pendingSurfaceRecords.append((name, json)); return }
        apply(runtime.surfaceRecord(name, json))
    }

    /// A batch that changes nothing the presenter shows: no ops, no error,
    /// no control's viewless contents (`controls`, LLP 1069.011 §9), no
    /// page facts the app moved (`setMode`, `setRootFontSize`), wake or orientation lock,
    /// and the motion, spatial, frame-task, canvas and owed-draw state as
    /// they stand. Its timer deadline is the caller's to compare.
    func changesNothing(_ batch: Batch) -> Bool {
        batch.ops.isEmpty && !batch.controls && batch.error == nil && batch.canvasImages.isEmpty
            && !batch.facts && batch.wakelock == nil && batch.orientation == nil
            && batch.motion == frames.motion && batch.spatial == frames.spatial && batch.canvasOwed == canvasOwed
            && batch.canvas == frames.canvas2d && batch.frames == frames.tasks
    }

    /// A batch from a timer, a scroll event or a host's measurement: one that
    /// changes nothing only moves the clock. An app's timer that writes nothing
    /// (a poll that finds no news), a `scroll=` handler whose writes show
    /// nowhere (every frame of a fling) and a size layout already had (a list
    /// row's symbol, which the kernel measured) still commit, and the presenter's
    /// whole pass cost ~4.5 ms a batch on the simulator. Other events
    /// still apply an empty batch: a native control that changed itself
    /// before its handler ran is reconciled by the pass (a refused tab), and
    /// the agent's clock seeks native animations through it. Slices published
    /// before it land first (T4), and it is asked again (a fling's fills).
    /// `timer`: a clock advance's, which `timersQuiet` counts.
    func applyUnlessEmpty(_ batch: Batch, scrolled: Bool = false, timer: Bool = false) {
        guard !batch.busy, !batch.held else { return }
        guard !applying else { apply(batch); return }
        if !landing, ownerInFlight, hasPublished {
            // What the slices leave owed is asked after this batch, as in `apply`.
            holdingReports = true; landFill(); holdingReports = false
            guard state != .destroyed else { return }
        }
        guard !fillInFlight, !tickInFlight, !advanceInFlight, !canvasInFlight, noticesInFlight == 0, changesNothing(batch) else { apply(batch); return }
        // Its transactions are reported once (LLP 1079 D3): account for them, at no presentation cost.
        sampler?.batch(batch.seq, ms: 0); logAlerts(batch.alerts); logJournal(batch.journal)
        if let q = batch.seq { natives.hatchClock.seq = q.1 } // its commits are the hatches' latest
        if timer { quietTimerAdvances = min(quietTimerAdvances + 1, 2) }
        timerDue = batch.timerDueMs; timerLanding = batch.landing
        scheduleClock(due: batch.timerDueMs)
        // What the pass did that a scroll moves (`scrolledWithoutPass`).
        if scrolled { presenter.scrolledWithoutPass() }
        drainReports()
        advanceIfDue()
    }

    /// The appearance reports a landing held back (`holdingReports`), asked
    /// once the batch the owner made before them has applied or skipped.
    private func drainReports() {
        if colorsOwed, !holdingReports, !awaitingTree { colorsOwed = false; apply(runtime.reportColors(tint: SystemColor.appTint.flatMap { SystemColor.tintPair($0) })) }
        while !holdingReports, !pendingViewDark.isEmpty {
            let (id, dark) = pendingViewDark.removeFirst()
            let batch = runtime.viewScheme(id, dark: dark)
            // Appearance already changed natively; retain its transaction accounting without rescheduling.
            if !ownerInFlight, changesNothing(batch), batch.timerDueMs == timerDue {
                sampler?.batch(batch.seq, ms: 0)
                if let q = batch.seq { natives.hatchClock.seq = q.1 }
            } else { apply(batch) }
        }
    }

    #if DEBUG
    /// Tests: a tick's slice the owner published, not yet landed.
    func publishTickForTests(_ batch: Batch) { tickInFlight = true; publish(.tick, batch, generation) }
    func holdFillForTests() { fillInFlight = true }
    func commitForTests() { generation += 1; viewsEpoch += 1 }
    func publishFillForTests(_ view: UInt32, _ batch: Batch, generation: Int) { fillInFlight = true; publish(.fill(view), batch, generation) }
    func oweColorsForTests() { colorsOwed = true }
    #endif

    /// Batches that reached `apply` (`IdleTickTests` read it, and painted popovers keep their measures for one: `PositionArea.MeasureCache`).
    private(set) var appliedBatches = 0
    package func apply(_ batch: Batch) {
        guard state != .destroyed, !batch.busy, !batch.held else { return }
        appliedBatches += 1
        quietTimerAdvances = 0
        // A slice the owner committed before this batch applies first (T4).
        if !applying, !landing, ownerInFlight, hasPublished {
            // What the landed slices leave owed is asked after this batch,
            // which the owner made after them (T4; LLP 1034 §8).
            holdingReports = true; landFill(); holdingReports = false
            guard state != .destroyed else { return }
        }
        let outermost = !applying
        // After the batch's own accounting below, which a later defer runs first
        defer { if outermost { advanceIfDue(); rasters.readmitPictures() } }
        applying = true
        // What applying it cost, with its transactions, for the next sampled frame (LLP 1079 D3), told to the sampler there was when it began while the session keeps it: a recording the batch starts makes a new one, and one it stops lets a production bake's go.
        let timed = sampler, began = CACurrentMediaTime()
        defer { if let timed, timed === sampler { timed.batch(batch.seq, ms: outermost ? (CACurrentMediaTime() - began) * 1000 : 0) }; if let q = batch.seq { natives.hatchClock.seq = q.1 }; logAlerts(batch.alerts); logJournal(batch.journal) }
        if batch.facts { app.scheduleStatus() } // the app's own `setMode` or `setRootFontSize` (LLP 1122 D4)
        // Each batch says what its turn left owed; batches apply in the owner's order, so the last one applied is the runner's now.
        canvasOwed = batch.canvasOwed
        if canvasOwed { askCanvasDraw() }
        for op in batch.ops where op.op == .router { routerOp = op.payload }
        // @ref LLP 1048.003 D1 — the head's title, for the app that owns the chrome.
        for op in batch.ops where op.op == .title {
            presenter.headTitle(op.payload["title"] as? String)
            #if os(macOS)
            presenter.headEdited(op.payload["edited"] as? Bool ?? false)
            #endif
        }
        #if os(macOS)
        regions.prepare(batch)
        #endif
        for op in batch.ops where op.op == .activity {
            if activityOutput == nil { activityOutput = ActivityOutput(self) }
            activityOutput?.enqueue(op.payload)
        }
        for op in batch.ops where op.op == .surfaceWork { pendingSurfaceWork.append((op.payload, generation)) }
        if !ExactEnv.agentMode { for op in batch.ops where op.op == .sound || op.op == .midi { if op.op == .sound { sound.apply(op.payload) } else { midi.apply(op.payload) } } } // LLP 1096 D8, LLP 1125 D7: the voice table's and the MIDI queue's ops, never under the agent's clock
        // Opened once the batch is applied, off this session's window (LLP 1069.006 D3).
        for op in batch.ops where op.op == .auth || op.op == .presence {
            let (payload, auth) = (op.payload, op.op == .auth)
            DispatchQueue.main.async { [weak self] in if let self, state != .destroyed { if auth { authOp(payload) } else { presenceOp(payload) } } }
        }
        // Each presenter takes its boot's table, a second one with the same too.
        ThemeRoles.apply(batch, root: presenter.root); if batch.ops.contains(where: { $0.op == .roles }) { presenter.applyThemeTint() }
        ThemeRoles.within(presenter.root) { presenter.apply(batch) }; Tambers.moved(self) // bindings' places (LLP 1121 D12)
        if !batch.canvasImages.isEmpty { presenter.canvas2d.load(batch.canvasImages) }
        AnimatedRasters.shared.poke()
        for op in batch.ops where op.op == .reorder { presenter.reorder?.observe(ReorderState(op.payload)); presenter.reorderGroup?.observe(ReorderGroupState(op.payload)) }
        presenter.reorder?.raiseLifted()
        frames.motion = batch.motion
        frames.spatial = batch.spatial
        frames.canvas2d = batch.canvas
        frames.tasks = batch.frames
        // The GPU module: after the first painted frame, only when a canvas exists.
        if firstDrawMs != nil { canvases.loadIfNeeded(); natives.loadIfNeeded(); drainSurfaceWork() } else { DispatchQueue.main.async { [weak self] in guard let self else { return }; canvases.loadIfNeeded(); drainSurfaceWork(); frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames) } }
        timerDue = batch.timerDueMs; timerLanding = batch.landing
        scheduleClock(due: batch.timerDueMs)
        if outermost {
            while !pendingSurfaceRecords.isEmpty {
                let (name, json) = pendingSurfaceRecords.removeFirst()
                apply(runtime.surfaceRecord(name, json))
            }
            drainReports()
            presenter.collections.flush()
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
                    AgentPasteboard.copy(text, to: agentPasteboard, log: log)
                    continue
                }
                if name == "reload" && args.isEmpty {
                    // The dev menu's Reload, from the app, in a release build too (a Mac one has no menu, LLP 1115.000 D7.8); `EXACT_DEV_MENU=0`, at build or launch, refuses it.
                    guard ExactEnv.devMenu != false else { fputs("exact: reload: no dev menu in this build\n", stderr); continue }
                    app.deliver { DevMenu.reload() }
                    continue
                }
                if ["navigate", "traverseHistory", "reload", "stopLoading", "captureScreenshot"].contains(name) { app.deliver { [weak self] in self?.webviews.command(name, args) }; continue } // an iframe's, by id (LLP 1020.002 D3, D7)
                if name == "relaunch" { app.deliver { [weak self] in self?.relaunch() }; continue }
                if name == "startTrace_EXPERIMENTAL" || name == "stopTrace_EXPERIMENTAL" { app.deliver { [weak self] in self?.traceCommand(name, args) }; continue }
                if name == "haptic" {
                    // @ref LLP 1077 D14 — feedback from app logic.
                    app.deliver { Haptics.play(args.first as? String ?? "") }
                    continue
                }
                if name == "share" { app.deliver { [weak self] in self?.share(args, source: source) }; continue }
                if name == "postMessage" {
                    let text = args.first as? String ?? "", surface = args.count > 1 ? args[1] as? String ?? "" : "", iframe = args.count > 2 && args[2] as? String == "iframe"
                    app.deliver { [weak self] in self?.post(text, to: surface, iframe: iframe) }
                    continue
                }
                if (name == "focus" && !args.isEmpty) || name == "selectText" { // bare `focus()` is the window's: the delegate's (LLP 1133.025 §8)
                    app.deliver { [weak self] in self?.presenter.focusElement(args, selectText: name == "selectText") }
                    continue
                }
                if name == "removeAllRanges" {
                    #if os(macOS)
                    app.deliver { [weak self] in self?.presenter.selection.clear() }
                    #else
                    log("removeAllRanges refused: reader selection commands are not supported on this platform")
                    #endif
                    continue
                }
                if name == "setSelectionRange" || name == "setRangeText" {
                    app.deliver { [weak self] in if name == "setRangeText" { self?.presenter.fieldSelections.setRangeText(args) } else { self?.presenter.fieldSelections.setSelectionRange(args) } }
                    continue
                }
                if name == "blur" { app.deliver { [weak self] in self?.presenter.blurElement(args) }; continue }
                if ["showPopover", "hidePopover", "togglePopover"].contains(name) { app.deliver { [weak self] in self?.presenter.menus.action(name, args) }; continue }
                if name == "showModal" || (name == "close" && args.first is String) { let id = args.first as? String ?? ""; app.deliver { [weak self] in self?.presenter.dialogCommand(name, id) }; continue } // a dialog's, by id (LLP 1115 D6); bare `close()` is the window's
                if name == "scrollIntoView" {
                    app.deliver { [weak self] in self?.presenter.scrollElementIntoView(args) }
                    continue
                }
                if name == "fastSeek" || name == "load" || name == "requestFullscreen" {
                    app.deliver { [weak self] in self?.presenter.mediaCommand(name, args) }
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
                if ["showOpenFilePicker", "showDirectoryPicker", "showSaveFilePicker", "requestPermission"].contains(name) {
                    app.deliver { [weak self] in if name == "requestPermission" { self?.picker.permission(args) } else { self?.picker.document(name, args) } }
                    continue
                }
                if name == "format" {
                    app.deliver { [weak self] in self?.presenter.formatElement(args) }
                    continue
                }
                if ["playSound", "playSounds", "stopSounds", "sendMIDI", "requestMIDIAccess", "setRootFontSize"].contains(name) { continue } // the runner's own: its voice table's `sound` op plays them (LLP 1096 D8), its MIDI queue's `midi` op sends (LLP 1125 D7); the root font size is laid out already (LLP 1069.000 D3)
                if app.handleCommand(name) { continue }
                app.deliver { [weak self] in guard let self else { return }; delegate?.exactSession(self, command: name, args: args) }
            }
        }
    }

    /// `drainSurfaceWork` from outside a batch (a deferred module's load).
    package func drainSurfaceWorkNow() { if !applying { drainSurfaceWork() } }

    private func drainSurfaceWork() {
        guard canvases.ready || canvases.failed != nil || canvases.isEmpty,
              !pendingSurfaceWork.isEmpty else { return }
        let work = pendingSurfaceWork
        pendingSurfaceWork = []
        DispatchQueue.main.async { [weak self] in
            guard let self, state != .destroyed else { return }
            for (op, owner) in work { canvases.surfaceWork(op, generation: owner) }
        }
    }

    package func completeSurface(_ ticket: UInt64, generation owner: Int, kind: UInt32, body: Data = Data()) {
        guard state != .destroyed, generation == owner, runtime.requestActive(ticket) else { return }
        apply(runtime.fulfillSurface(ticket, kind: kind, body: body, now: now()))
    }

    /// The first node drew: the GPU module may load now (LLP 1009 D4), on
    /// the next turn; the update store hears first pixel (LLP 1026 D11).
    /// Capture at node creation, one for a generation's and an update's nodes: a delayed old draw cannot bless its successor.
    func drawReceipt() -> DrawReceipt {
        if let receipt, receipt.generation == generation, receipt.token == updateToken { return receipt }
        let made = DrawReceipt(self, generation: generation, token: updateToken); receipt = made
        return made
    }

    /// Drawn once per launch; the same across a dev reload's new runner.
    let launchPlace: LaunchPlace
    /// The zone's offset last told (`followOffset`, before each advance, so a
    /// DST change reaches the timer after it, habits F6; `tellAgentOffset`).
    var toldOffset: Double?, offsetHeld: (zone: String, until: Date)? // `followOffset`
    /// Before a first boot the runtime keeps them, so the first frame is laid
    /// out with the device's preferences rather than a mouse's and then again
    /// (`pointer: none` on tvOS sets a different layout).
    private func primePreferences() {
        guard !booted, state != .destroyed else { return }
        primeControlText()
        _ = runtime.setPreferences(preferenceBits())
    }
    var displayModeOverride: UInt32?
    func preferenceBits() -> UInt32 {
        #if os(iOS) || os(tvOS)
        // The scene owns system appearance; a window's app override does not.
        if let scene = view?.window?.windowScene {
            systemDark = scene.traitCollection.userInterfaceStyle == .dark
        }
        let dark = systemDark
        #else
        let dark = DisplayPreferences.systemDark
        #endif
        let bits = DisplayPreferences.bits(systemDark: dark, view: view)
        return displayModeOverride.map { (bits & ~(7 << 11)) | ($0 << 11) } ?? bits
    }
    public func resize(_ size: CGSize) { guard booted, state != .destroyed else { return }; apply(runtime.resize(width: size.width, height: size.height)) }
    public func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) { guard booted, state != .destroyed else { return }; apply(runtime.insets(top: top, right: right, bottom: bottom, left: left)) }
    /// What changed of the view (`Runtime.ViewChange`); the window's size is what every viewport unit resolves against everywhere (LLP 1075.003 §9.11).
    /// `fold`: a fold on the kernel's grid whose rects go with the change (a keyboard's), kept as `segments` keeps one.
    func view(_ change: Runtime.ViewChange, fold: ViewportFold? = nil) {
        var change = change
        change.segments = fold?.rects
        guard booted, state != .destroyed, !change.isEmpty else { return }
        if let screen = change.screen { screenSize = screen }
        let batch = runtime.view(change)
        if let fold, batch.error == nil { presenter.fold = fold }
        apply(batch)
    }
    private(set) var screenSize: CGSize? // the screen last told, for the agent's synthetic segments
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
    /// The platform's colours are reported first: before the first scheme
    /// that fills the kernel's table without motion, so nothing eases from a
    /// fallback to the platform's colour at startup (LLP 1095 D9).
    public func scheme(dark: Bool) {
        guard booted, state != .destroyed else { return }
        let first = schemeDark == nil
        schemeDark = dark
        reportColors()
        // Views found painting in an appearance of their own before the
        // session had a scheme (a `color-scheme` subtree at boot, LLP 1034
        // §8): queued now, and said in the scheme's own batch, after it.
        if first {
            let early = unreported; unreported.removeAll()
            for id in early {
                if let v = presenter.views[id] { noteAppearance(v); continue }
                #if os(iOS) || os(tvOS)
                if let dark = presenter.flats.appearance(of: id) { noteAppearance(id, dark: dark) }
                #endif
            }
        }
        // A subtree with a `color-scheme` of its own keeps its appearance when the session's changes, so no trait
        // callback says it: said here, after the scheme. Its views' appearance is the row's, already settled,
        // whatever the platform's walk has reached.
        for v in presenter.views.values where v.style["color_scheme"] != nil { noteAppearance(v) }
        apply(runtime.scheme(dark: dark))
    }
    /// @ref LLP 1095 D9 — the platform's resolution of every reference the kernel paints itself, with the app's tint
    /// read here, on main: one for every session (`SystemColor.appTint`), since the table is the process's; a view's
    /// own tint is the host's to paint (`ownTint`).
    func reportColors() {
        guard booted, state != .destroyed else { return }
        #if os(iOS) || os(tvOS)
        // No window, no report: attaching reports (`didMoveToWindow`), before its scheme, so nothing eases from a stand-in blue.
        guard view?.window != nil else { return }
        #endif
        if applying || awaitingTree { colorsOwed = true; return } // a tint the batch being applied set, reported once the batch is in
        // No app tint: `@tint` is left out, and the kernel keeps the last.
        apply(runtime.reportColors(tint: SystemColor.appTint.flatMap { SystemColor.tintPair($0) }))
    }
    /// The appearance last reported for the session, and each node view found painting motion in another (a sheet's override, say), by id.
    private(set) var schemeDark: Bool?
    private(set) var viewDark: [UInt32: Bool] = [:]
    private var pendingViewDark: [(UInt32, Bool)] = []
    var colorsOwed = false
    fileprivate(set) var awaitingTree = false // committed to a new runner whose tree is not in: a report would reach the old views
    private func treeIsIn() { awaitingTree = false; replayHeld(); if colorsOwed { colorsOwed = false; reportColors() } } // only the replacement batch, not a reset's cleanup
    /// A view painting motion: when its own appearance is not the one its node's colours resolve by, say so after
    /// the batch (LLP 1062 D4).
    func noteAppearance(_ view: NodeView) { noteAppearance(view.id, dark: view.drawsDark) }
    /// A node painting in `dark`: a view, or a flat leaf in its parent's appearance.
    func noteAppearance(_ id: UInt32, dark: Bool) {
        guard let session = schemeDark else { unreported.insert(id); return }
        guard dark != viewDark[id] ?? session else { return }
        viewDark[id] = dark == session ? nil : dark
        pendingViewDark.append((id, dark))
    }
    /// Views noted before the session had a scheme.
    private var unreported = Set<UInt32>()
    /// A new runtime has heard no view's appearance: forget what was said,
    /// and say it again after its first scheme (LLP 1062 D4, LLP 1034 §8).
    private func forgetAppearances() {
        schemeDark = nil
        viewDark = [:]
        pendingViewDark = []
        unreported = []
    }
    /// The agent API's runner half (LLP 1012): `tree`, `state`, `logs`, `settle`.
    public func agent(_ request: String) -> String { runtime.agent(request) }
    /// A line for the runner's journal (LLP 1012 §3; LLP 1035.001 D6): a
    /// refused intent and its reason, read back through `logs`.
    public func log(_ line: String) { runtime.log(line) }
    /// Deliver an embedder's value through a declared change handler. The
    /// selector must name exactly one live node; file contents stay data.
    @discardableResult public func change(testId: String, value: String) -> Bool {
        changeRefusal = nil
        guard state != .destroyed, booted else { changeRefusal = "it has not started"; return false }
        let matches = presenter.views.values.filter { $0.props["testId"] == testId && $0.handlers.contains("change") }
        guard matches.count == 1, let node = matches.first else {
            changeRefusal = matches.isEmpty ? "it has no `\(testId)` field with a `change` handler" : "it has \(matches.count) `\(testId)` fields"
            return false
        }
        let batch = runtime.change(node.id, documentValue(node.id, value), now: now())
        apply(batch)
        if let error = batch.error { changeRefusal = "its `change` was refused: \(error)" }
        return batch.error == nil
    }
    /// Why the last `change(testId:value:)` delivered nothing: the app has
    /// no such field, or its action refused the value (studio diary R14).
    public private(set) var changeRefusal: String?
    /// Deliver toolbar facts only when the authored editor has a select handler.
    package func selection(node: UInt32, json: String) {
        guard booted, state != .destroyed,
              presenter.views[node]?.handlers.contains("select") == true,
              let batch = runtime.selection(node, json: json, now: now()) else { return }
        apply(batch)
    }
    /// @ref LLP 1038 D8/D11 — pre-boot URLs are launch facts; the adapter consumes development links first.
    @discardableResult public func openURL(_ url: URL) -> Bool {
        guard state != .destroyed else { return false }
        return openIncoming(incomingLocation(url))
    }
    /// A link the reader followed — a `link href`, a text run's `href`, a
    /// Markdown link. A path naming one of the app's routes is a location
    /// for the navigation root, as the web's same-document link is (LLP 1038
    /// §7); anything else — a page, a file beside a document — is the
    /// containing app's to open (`openURL`).
    @discardableResult public func follow(_ href: String) -> Bool {
        guard state != .destroyed, !href.isEmpty else { return false }
        if href.hasPrefix("/"), !href.hasPrefix("//"), booted, runtime.routeMatches(href) { return navigate(href) }
        delegate?.exactSession(self, command: "openURL", args: [href])
        return true
    }
    @discardableResult public func navigate(_ location: String) -> Bool {
        guard state != .destroyed, booted else { return false }
        let batch: Batch
        if let node = navigationRoot { batch = runtime.navigate(node.id, location, now: now()) }
        else if runtime.routeMatches(location) { batch = runtime.hostGo(location, now: now()) }
        else { log("navigate refused: no navigation root handler or matching route"); return false }
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
        agentPasteboard.map(AgentPasteboard.remove)
        let waiting = afterFill; afterFill.removeAll()
        waiting.forEach { $0.cancelled?() }
        generation += 1
        cancelAuthSessions()
        clockTimer?.invalidate()
        clockTimer = nil
        frames.run(false)
        sampler?.stop(); releaseSampler()
        DisplayPreferences.forget(preferenceObservers)
        PageFacts.forget(pageObservers)
        #if os(macOS)
        colorObserver.map(NotificationCenter.default.removeObserver)
        #endif
        endModuleEvents()
        runtime.held = true; presenter.reset(); runtime.held = false; _ = runtime.takeHeldCalls(); viewsEpoch += 1; presenter.showDevStatus(nil); app.refreshDevPills() // its pill to another session in its window; the dying runtime told nothing its teardown sets off (ResetGate, none replayed)
        ExactSession.live.removeValue(forKey: runtime.rt)
        forgetDocuments()
        rasters.shutdown(); sound.shutdown(); midi.shutdown() // the sound arm stopped and let go, its session claim given back (LLP 1096 D8)
        webviews.endSession() // its frames and in-memory partitions (LLP 1020.002 D1)
        textPressure?.cancel(); textPressure = nil
        runtime.destroy()
        natives.destroyModule()
        app.forget(self)
    }
}

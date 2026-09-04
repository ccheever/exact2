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

/// The one Exact app this process links (LLP 1031 D1, D11).
public final class ExactApp {
    public static let shared = ExactApp()

    /// Where an image source, a declared font, or a deck page resolves:
    /// `EXACT_ASSETS`, else the bundle (iOS) or the working directory
    /// (macOS) — the way a page resolves against its URL.
    public var assetRoot: URL
    /// Assets that arrived by name since launch (the dev connection's asset
    /// row, LLP 1030 D10; the update store's entry, 1026 D11): a name
    /// resolves here before the root. Files live in the app's cache by
    /// digest.
    public private(set) var assetOverrides: [String: URL] = [:]
    /// The plan bytes last applied to every session, so a font edit can
    /// restart them from the same plan and re-register the faces.
    private(set) var lastPlan: Data?

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
        let fallback = FileManager.default.currentDirectoryPath
        #endif
        assetRoot = URL(fileURLWithPath: ExactEnv.environment["EXACT_ASSETS"] ?? fallback, isDirectory: true)
    }

    /// A session of this app: one runtime, unbooted until `boot` or its view's first layout.
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
        connection = PlanURL.open(url, apply: { [weak self] bytes, label in self?.apply(bytes, label: label) ?? false }, asset: { [weak self] name, sha, bytes in self?.assetArrived(name: name, sha256: sha, bytes: bytes) })
    }

    public func disconnect() {
        connection?.close()
        connection = nil
    }

    /// The connection's page URL, when connected (what a deck's `//` source resolves against).
    public var connectedPage: URL? { connection?.page }
    /// The connection's status line for a dev menu, or nil.
    public var connectionStatus: String? { connection.map { "\($0.page)\($0.terminal != nil ? " (rebuild the host)" : " (live)")" } }
    /// Re-resolve the connection (the menu's Reload; clears a `{rebuilt}` stop).
    public func reloadConnection() { connection?.reload() }

    /// Plan bytes into every session, each transactionally: true when every
    /// session took them (a session that refused keeps its last good app).
    @discardableResult
    public func apply(_ bytes: Data, label: String = "plan") -> Bool {
        var all = true
        for s in sessions { all = s.apply(bytes, label: label) && all }
        if all { lastPlan = bytes }
        return all
    }

    /// A name (`assets/logo.png`, `shaders/aurora.wgsl`, a declared face's
    /// source) as a file URL: an override that arrived by digest first, else
    /// the file under the root — never outside it, as a page resolves `src`.
    public func resolveAsset(_ name: String) -> URL? {
        if let u = assetOverrides[name] { return u }
        let root = assetRoot.standardizedFileURL.resolvingSymlinksInPath()
        let url = root.appendingPathComponent(name).standardizedFileURL.resolvingSymlinksInPath()
        let rootPath = root.path.hasSuffix("/") ? root.path : root.path + "/"
        return url.path == root.path || url.path.hasPrefix(rootPath) ? url : nil
    }

    /// The cache an arriving asset is written into, by digest.
    static var assetCache: URL {
        let base = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first ?? URL(fileURLWithPath: NSTemporaryDirectory())
        return base.appendingPathComponent("exact/assets", isDirectory: true)
    }

    /// An asset arrived (verified by its digest): kept by digest, named,
    /// and every session refreshes what referenced it — an image repaints,
    /// a shader is handed to the module (which validates it against the
    /// interface it binds), a declared face restarts the sessions from the
    /// current plan so the catalog re-registers, a deck page is left to the
    /// arm (owed).
    public func assetArrived(name: String, sha256: String, bytes: Data) {
        let dir = ExactApp.assetCache
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let ext = (name as NSString).pathExtension
        let file = dir.appendingPathComponent(ext.isEmpty ? sha256 : "\(sha256).\(ext)")
        do { try bytes.write(to: file, options: .atomic) } catch { print("exact asset: \(name): \(error)"); return }
        assetOverrides[name] = file
        assetsChanged([name])
    }

    /// The names whose bytes changed: each session refreshes what it shows.
    public func assetsChanged(_ names: [String]) {
        var fonts = false
        for name in names {
            if name.hasPrefix("shaders/"), name.hasSuffix(".wgsl"), let url = assetOverrides[name] ?? resolveAsset(name), let text = FileManager.default.contents(atPath: url.path) {
                let stem = String(name.dropFirst("shaders/".count).dropLast(".wgsl".count))
                for s in sessions { s.canvases.shaderChanged(stem, text: text) }
            } else if ["ttf", "otf", "woff", "woff2"].contains((name as NSString).pathExtension.lowercased()) {
                fonts = true
            } else {
                for s in sessions { s.presenter.assetChanged(name) }
            }
        }
        if fonts, let plan = lastPlan { for s in sessions { _ = s.apply(plan, label: "fonts") } }
    }
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
        didSet { if state != oldValue { delegate?.exactSession(self, didChange: state) } }
    }
    /// Bumped by every reboot; a callback from an older generation is dropped.
    public private(set) var generation = 0

    let runtime: Runtime
    let text: TextEngine
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

    /// Live sessions by handle: what a wake looks up (a stranger's is dropped).
    nonisolated(unsafe) private static var live: [ExactRuntime: WeakSession] = [:]

    init(app: ExactApp, label: String) {
        self.app = app
        self.label = label
        runtime = Runtime()
        text = TextEngine { [weak app] source in app?.resolveAsset(source) }
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
        presenter.onPress = { [unowned self] id in apply(runtime.press(id, now: now())) }
        presenter.onChange = { [unowned self] id, value in apply(runtime.change(id, value, now: now())) }
        presenter.onIntrinsic = { [unowned self] id, size in apply(runtime.intrinsic(id, width: size?.width ?? 0, height: size?.height ?? 0)) }
        presenter.onHover = { [unowned self] id, over in apply(runtime.hover(id, over: over, now: now())) }
        presenter.onFocus = { [unowned self] id in apply(runtime.focus(id, now: now())) }
        presenter.onBlur = { [unowned self] id in apply(runtime.blur(id, now: now())) }
        presenter.onKey = { [unowned self] id, name in apply(runtime.key(id, name, now: now())) }
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
            if booted { presenter.reset() }
            booted = true
            generation += 1
        }
        apply(batch)
        applyMs = (CACurrentMediaTime() - tApply) * 1000
        bootMs = ExactEnv.wall()
        ExactEnv.stamp("first frame applied")
        bootError = batch.error
        state = batch.error.map(State.failed) ?? .ready
        return batch
    }

    /// Plan bytes into a running session: the restart with carry (LLP 1007
    /// §6; D11) — transactional, so a refused candidate leaves the running
    /// app, its fonts, and its views exactly as they were. Returns whether
    /// the candidate became the running app.
    @discardableResult
    public func apply(_ bytes: Data, label: String = "plan") -> Bool {
        guard state != .destroyed else { return false }
        let started = CACurrentMediaTime()
        let size = presenter.viewportSize
        let cp = text.checkpoint()
        let batch = runtime.bootPlan(bytes, width: size.width, height: size.height)
        if let error = batch.error {
            text.restore(cp)
            FileHandle.standardError.write(Data("exact: \(error)\n".utf8))
        } else {
            generation += 1
            presenter.reset()
            booted = true
            apply(batch)
            view?.rebooted()
            state = .ready
        }
        print("reloaded \(label) in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")")
        return batch.error == nil
    }

    /// A batch into the presenter; frames and the clock follow it; its
    /// commands go to the delegate after it.
    func apply(_ batch: Batch) {
        guard state != .destroyed else { return }
        let outermost = !applying
        applying = true
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
            applying = false
            let queued = pendingCommands
            pendingCommands = []
            for (name, args) in queued { delegate?.exactSession(self, command: name, args: args) }
        }
    }

    /// The first node drew: the GPU module may load now (LLP 1009 D4), on
    /// the next turn.
    func firstDrawn() {
        guard firstDrawMs == nil else { return }
        firstDrawMs = ExactEnv.wall()
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            canvases.loadIfNeeded()
            frames.run(frames.motion || canvases.wantsFrames)
        }
    }

    public func resize(_ size: CGSize) { guard booted, state != .destroyed else { return }; apply(runtime.resize(width: size.width, height: size.height)) }
    public func insets(top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) { guard booted, state != .destroyed else { return }; apply(runtime.insets(top: top, right: right, bottom: bottom, left: left)) }
    /// The agent API's runner half (LLP 1012): `tree`, `state`, `logs`, `settle`.
    public func agent(_ request: String) -> String { runtime.agent(request) }
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

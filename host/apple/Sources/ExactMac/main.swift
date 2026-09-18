// ExactMac: the standalone macOS app as an adapter over ExactKit (LLP 1031
// D1) — a window, one session, one view, the default services, the dev
// menu. Host glue; the app is the static library (runner + kernel + data
// crate + baked plan).
//
// EXACT_SMOKE=1 prints the boot time and the startup phases after the first
// frames and exits — what `scripts/metrics.mjs` reads. EXACT_AGENT=1 is the
// agent API (LLP 1012, `Agent.swift`): the driver owns the clock and drives
// the app over stdio; `scripts/smoke.mjs` is a script of its operations.
import AppKit
import ExactKit
import ExactComposition

// The process's own start (exec), from the kernel: what happened before
// `main` — dyld, the Swift runtime, the static library's initializers.
func processStart() -> Double? {
    var info = kinfo_proc()
    var size = MemoryLayout<kinfo_proc>.stride
    var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
    guard sysctl(&mib, 4, &info, &size, nil, 0) == 0 else { return nil }
    let t = info.kp_proc.p_starttime
    return Double(t.tv_sec) + Double(t.tv_usec) / 1e6
}
let mainAt = Date().timeIntervalSince1970
let execToMainMs = processStart().map { (mainAt - $0) * 1000 }

let smoke = ExactEnv.smoke
let agentMode = ExactEnv.agentMode
let agentReadable = agentMode || ExactEnv.environment["EXACT_AGENT"] == "live"
setvbuf(stdout, nil, _IOLBF, 0)
let app = NSApplication.shared
ExactEnv.stamp("NSApplication.shared")
// Under a script (LLP 1012) the app is an accessory — no Dock tile, no
// activation, so a running smoke never takes the focus from whoever is
// typing; its window is made key for a `type` when one comes (`AgentMac`).
app.setActivationPolicy(agentMode ? .accessory : .regular)
ExactEnv.stamp("setActivationPolicy")
let appReadyMs = ExactEnv.wall()

/// The one session and its view; the session's clock is the agent's under a script.
let exact = ExactComposition.app
final class Adapter: ExactSessionDelegate {
    /// The capabilities: `setScheme` is the app's appearance — `light`,
    /// `dark`, or `system`, as the web's `color-scheme`, where following the
    /// user's preference is the absence of an override rather than a third
    /// appearance; `openURL` is a link the reader followed
    /// (`TextSelectionMac`); anything else is named and refused.
    func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
        switch name {
        case "setScheme":
            switch args.first as? String {
            case "dark": app.appearance = NSAppearance(named: .darkAqua)
            case "light": app.appearance = NSAppearance(named: .aqua)
            default: app.appearance = nil
            }
        case "openURL": open(args.first as? String ?? "", from: session)
        default: FileHandle.standardError.write(Data("exact: unknown command \(name)\n".utf8))
        }
    }

    /// A followed link. One that names a local file or directory is a
    /// document for this app (a Markdown link to the file beside it); one
    /// that names a page goes to whatever handles pages. Nothing else opens:
    /// a link comes out of a document this app did not write, and handing an
    /// arbitrary scheme to Launch Services is handing it whatever is
    /// registered for that scheme (LLP 0382 — fail closed, loudly).
    private func open(_ target: String, from session: ExactSession) {
        guard !target.isEmpty else { return }
        let url = URL(string: target)
        let local = url?.isFileURL == true ? url!.standardizedFileURL.path
            : target.hasPrefix("/") ? URL(fileURLWithPath: target).standardizedFileURL.path : nil
        if let local, FileManager.default.fileExists(atPath: local) {
            ExactDocuments.deliver([local], to: session)
            return
        }
        guard let url, ["http", "https", "mailto"].contains(url.scheme?.lowercased() ?? "") else {
            FileHandle.standardError.write(Data("exact: refused to open \(target)\n".utf8))
            return
        }
        NSWorkspace.shared.open(url)
    }
}
let adapter = Adapter()
let session = exact.makeSession(delegate: adapter, label: "main")
if agentMode { session.clock = 0 }
let launchURL = ExactEnv.environment["EXACT_LAUNCH_URL"].flatMap { URL(string: $0) }
var launchDevelopmentURL: URL?
if let url = launchURL {
    if ExactDevelopmentLink.page(url) != nil { launchDevelopmentURL = url }
    else { session.openURL(url) }
}
let view = ExactView(session: session)
ExactEnv.stamp("Presenter (NSScrollView)")

let windowConfig = ExactEnv.appMetadata["ExactWindow"] as? [String: Any] ?? [:]
func windowDimension(_ name: String, fallback: Double) -> CGFloat {
    let override = ExactEnv.environment["EXACT_WINDOW_" + name.uppercased()].flatMap(Double.init)
    let declared = agentMode || smoke ? nil : (windowConfig[name] as? NSNumber)?.doubleValue
    let value = override ?? declared ?? fallback
    return CGFloat(value.isFinite && value > 0 && value <= 16384 ? value : fallback)
}
let size = NSSize(width: windowDimension("width", fallback: 420), height: windowDimension("height", fallback: 860))
let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
ExactEnv.stamp("NSWindow")
window.title = ExactEnv.appName
if !agentMode && !smoke && !windowConfig.isEmpty {
    let minimum = NSSize(width: windowDimension("minWidth", fallback: 1), height: windowDimension("minHeight", fallback: 1))
    window.contentMinSize = minimum
    window.setContentSize(NSSize(width: max(size.width, minimum.width), height: max(size.height, minimum.height)))
}
// Nothing is focused at launch — the web's rule (a page focuses no field on
// load). AppKit would otherwise make the first key view the first responder
// when the window becomes key, and a canvas holding an input would show a
// caret from its first frame (found by the readback fixture, LLP 1014).
window.initialFirstResponder = view
window.autorecalculatesKeyViewLoop = false
window.center()
if !agentMode && !smoke && !windowConfig.isEmpty,
   let identity = ExactEnv.appMetadata["CFBundleIdentifier"] as? String {
    let frameName = identity + ".main"
    window.setFrameUsingName(frameName)
    window.setFrameAutosaveName(frameName)
}
// Agent-driven apps run side by side (every session's smoke launches one):
// centred, each would cover the last and starve its Metal layer of drawables.
// Spread them by pid so no window is fully hidden.
if agentMode {
    let k = CGFloat(Int(getpid()) % 6)
    window.setFrameOrigin(NSPoint(x: window.frame.origin.x - 120 + 48 * k, y: window.frame.origin.y + 60 - 24 * k))
}
ExactEnv.stamp("center")

final class Delegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationDidFinishLaunching(_ notification: Notification) {
        ExactEnv.stamp("didFinishLaunching")
        finishLaunching()
    }
    /// Launch Services brought something: a development link, or documents —
    /// a Finder double-click, an Open With, `open -a`, or a second `mdview`
    /// while this one runs. A document arriving now is why the app comes
    /// forward; one that opens nothing leaves the window where it was.
    func application(_ application: NSApplication, open urls: [URL]) {
        if let url = urls.first, ExactDevelopmentLink.page(url) != nil {
            if !session.booted { launchDevelopmentURL = url; return }
            ExactDevelopmentLink.open(url)
            front(application)
            return
        }
        // @ref LLP 1038 D8 — Launch Services delivers cold URLs before didFinishLaunching.
        if let url = urls.first(where: { !$0.isFileURL }) {
            if session.openURL(url) { front(application) }
            return
        }
        let documents = ExactDocuments.paths(of: urls)
        if !session.booted { launchDocuments += documents; return }
        guard !documents.isEmpty, ExactDocuments.deliver(documents, to: session) else { return }
        if let first = documents.first { application.windows.first?.title = ExactDocuments.windowTitle(for: first) }
        front(application)
    }

    private func front(_ application: NSApplication) {
        guard session.booted else { return }
        application.windows.first?.makeKeyAndOrderFront(nil)
        application.activate(ignoringOtherApps: true)
    }
    func windowDidBecomeKey(_ notification: Notification) {
        if !ExactEnv.stamps.contains(where: { $0.0 == "windowDidBecomeKey" }) { ExactEnv.stamp("windowDidBecomeKey") }
        agentReady()
    }
    /// Seen again, or no longer: the canvases follow (`Canvases.visible`).
    func windowDidChangeOcclusionState(_ notification: Notification) {
        session.occlusionChanged()
    }
}
/// `viewport-fit=cover` (LLP 1008 §9): the window's content includes the
/// titlebar, the titlebar is transparent, and its height is the top safe-area
/// inset — the same mapping a phone uses for the status bar. Anything else
/// keeps a normal titled window and zero insets. The window is the adapter's;
/// the insets are the view's (`ExactView.syncInsets`).
func coverChrome() {
    let cover = view.viewportFit == "cover"
    if cover {
        if !window.styleMask.contains(.fullSizeContentView) { window.styleMask.insert(.fullSizeContentView) }
        window.titlebarAppearsTransparent = !view.hasWindowToolbar
        window.titleVisibility = view.hasWindowToolbar ? .visible : .hidden
        window.backgroundColor = session.pageBackground
        if #available(macOS 11.0, *) { window.titlebarSeparatorStyle = view.hasWindowToolbar ? .automatic : .none }
    } else {
        if window.styleMask.contains(.fullSizeContentView) { window.styleMask.remove(.fullSizeContentView) }
        window.titlebarAppearsTransparent = false
        window.titleVisibility = .visible
        if #available(macOS 11.0, *) { window.titlebarSeparatorStyle = .automatic }
    }
    if view.hasWindowToolbar { window.toolbarStyle = .unifiedCompact }
    view.syncInsets()
}
view.onViewportFit = { coverChrome() }
// Attaching the view can boot its embedded plan before this hook is installed.
// Apply the current value even when the explicit boot keeps the same value.
coverChrome()

let delegate = Delegate()
app.delegate = delegate
window.delegate = delegate

var planWatch: DispatchSourceTimer?
var devPlanPath: String?
var appearanceWatch: NSObjectProtocol?
var launchDocuments: [String] = []
nonisolated(unsafe) var readySent = false

// Delay both attachment and boot until Launch Services has delivered launch URLs.
// @ref LLP 1038 D5/D8
func finishLaunching() {
    window.contentView = view
    view.attachWindowToolbar(to: window)
    ExactEnv.stamp("contentView")
    ExactEnv.stamp("before boot")
    let tBoot = CACurrentMediaTime()
    // The dev loop (LLP 1007 §6, here): EXACT_DEV_PLAN names the plan the
    // resident compiler writes; when it changes, restart from it, state carried.
    // A URL instead of a path is the wire form (LLP 1023 Stage 1): the app URL,
    // resolved and re-fetched by the app's one connection (LLP 1031 D11).
    if let planPath = ExactEnv.environment["EXACT_DEV_PLAN"] {
        if planPath.hasPrefix("http://") || planPath.hasPrefix("https://") {
            exact.connect(planPath)
        } else {
            devPlanPath = planPath
            let candidate = ExactDevelopmentPlan(planPath)
            var last = candidate.hasModule ? [] : candidate.revision
            let t = DispatchSource.makeTimerSource(queue: .main)
            t.schedule(deadline: .now() + 0.1, repeating: 0.1)
            t.setEventHandler {
                let revision = candidate.revision
                guard revision != last else { return }
                if candidate.apply(to: exact) || !exact.generationPending { last = revision }
            }
            t.resume()
            planWatch = t
        }
    }
    DevMenu.install(session: session, planPath: devPlanPath ?? ExactEnv.environment["EXACT_PLAN"])

    // EXACT_PLAN=<file> boots that plan instead of the one baked into the
    // library — any compiled contract, no rebuild (smokes, fixtures).
    let boot: Batch = {
        let size = session.viewportSize
        let path = ExactEnv.environment["EXACT_PLAN"] ?? devPlanPath
        if let path, ExactDevelopmentPlan(path).hasModule, ExactEnv.environment["EXACT_PLAN"] != nil {
            DispatchQueue.main.async { ExactDevelopmentPlan(path).apply(to: exact) }
        }
        if let path, !ExactDevelopmentPlan(path).hasModule, let bytes = FileManager.default.contents(atPath: path) {
            return session.boot(plan: bytes, size: size)
        }
        return session.boot(size: size)
    }()
    let rustMs = session.rustMs
    let applyMs = session.applyMs
    let bootMs = session.bootMs
    // Becoming key can synchronously announce readiness. Initialize the guard
    // before ordering the window, not afterward (two stdin readers otherwise).
    coverChrome()
    window.makeKeyAndOrderFront(nil)
    if let url = launchDevelopmentURL {
        launchDevelopmentURL = nil
        DispatchQueue.main.async { ExactDevelopmentLink.open(url) }
    }
    ExactEnv.stamp("makeKeyAndOrderFront")
    // Under a script: in front regardless, so the window is seen (a covered
    // window's canvases render nothing, LLP 1009 D4) — but never activated.
    if agentMode { window.orderFrontRegardless() } else { app.activate(ignoringOtherApps: true) }
    ExactEnv.stamp("activate")

    if !launchDocuments.isEmpty {
        ExactDocuments.deliver(launchDocuments, to: session)
        window.title = ExactDocuments.windowTitle(for: launchDocuments[0])
        launchDocuments.removeAll()
    }
    // The documents named on the command line, now that the first frame has
    // mounted the app's own nodes. Launch Services' route into a *running* app is
    // `application(_:open urls:)` above; a terminal's is this, and the two are
    // the same from here down. Not under a script or the smoke: those drive the
    // app themselves and a stray argument is not a document. @ref LLP 1033 D3
    if !agentMode && !smoke {
        let opened = ExactDocuments.paths(in: CommandLine.arguments)
        ExactDocuments.deliver(opened, to: session)
        // The window says which project is open, not just which app this is
        // (LLP 1033 D7). The path the OS handed over is the one to name it by.
        if let first = opened.first { window.title = ExactDocuments.windowTitle(for: first) }
    }
    // What the system is set to, now and whenever it changes (LLP 1033 D6). An
    // app that draws its own palette needs this to follow the system at all: the
    // window's appearance is the host's, and the page's colours are the app's.
    // Delivered under a script too — a reader's palette is part of what a driver
    // reads, and unlike a command-line argument this is not a stray.
    ExactDocuments.reportAppearance(to: session)
    appearanceWatch = ExactDocuments.watchAppearance(session)

    /// Agent mode: the driver owns the process from here — one JSON line in,
    /// one out. `ready` goes out once the first frame is applied and the window
    /// ordered front; an accessory app's window is not key until something
    /// asks, and a `type` asks (`AgentMac`).

    if agentReadable {
        DispatchQueue.main.async { agentReady() }
    }
    if smoke {
        print("boot \(String(format: "%.1f", bootMs)) ms; \(session.viewCount) views; root \(Int(session.rootSize.width))x\(Int(session.rootSize.height)); error \(boot.error ?? "none")")
        print("startup: exec→main \(execToMainMs.map { String(format: "%.1f", $0) } ?? "?") ms; main→NSApplication \(String(format: "%.1f", appReadyMs)) ms; →window \(String(format: "%.1f", (tBoot - ExactEnv.t0) * 1000 - appReadyMs)) ms")
        print("phases: process→boot \(String(format: "%.1f", (tBoot - ExactEnv.t0) * 1000)) ms; runner+layout \(String(format: "%.1f", rustMs)) ms of which \(session.measureCount) text measurements (\(session.measureHits) cached) \(String(format: "%.1f", session.measureSeconds * 1000)) ms in CoreText; apply \(String(format: "%.1f", applyMs)) ms")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
            print("painted \(session.firstDrawMs.map { String(format: "%.1f", $0) } ?? "?") ms")
            print("stamps: " + ExactEnv.stamps.map { "\($0.0) \(String(format: "%.1f", $0.1))" }.joined(separator: " · ") + " · first layout \(session.firstLayoutMs.map { String(format: "%.1f", $0) } ?? "?") · first draw \(session.firstDrawMs.map { String(format: "%.1f", $0) } ?? "?")")
            print("gpu: \(session.gpuStatus)")
            print("web: \(session.webStatus)")
            print("smoke ok")
            exit(0)
        }
    }
}

func agentReady() {
    guard agentReadable, !readySent else { return }
    readySent = true
    Agent.reply(["ready": true, "boot": session.bootMs, "views": session.viewCount, "error": session.bootError ?? NSNull()])
    Agent.startStdio(sessions: [("main", session)])
}
app.run()

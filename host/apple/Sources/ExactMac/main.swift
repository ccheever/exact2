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
// Nothing restores this app: a session boots its plan, and the window below is
// not restorable. AppKit would still save the application's own state some
// fifteen seconds after launch, and to do that it asks the window server for
// the order of its windows and waits for the reply on the main thread: 20 ms
// while a scroll kept the server busy, a dropped frame, and the longest wait
// left in a scroll. Encoding nothing in an `NSApplication` subclass only moves
// the wait to the next private caller of the same question. `ApplePersistence`
// is AppKit's own switch for all of it, read when the application is made; a
// registered default, so a user's `defaults write` still wins.
UserDefaults.standard.register(defaults: ["ApplePersistence": false])
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
        // A link beside a document the person chose (LLP 1069.010 D1).
        if target.hasPrefix("doc:/") { ExactDocuments.deliver([target], to: session); return }
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
let launchURL = ExactEnv.environment["EXACT_LAUNCH_URL"].flatMap { URL(string: $0) }
var launchDevelopmentURL: URL?

let windowConfig = ExactEnv.appMetadata["ExactWindow"] as? [String: Any] ?? [:]
func windowDimension(_ name: String, fallback: Double) -> CGFloat {
    let override = ExactEnv.environment["EXACT_WINDOW_" + name.uppercased()].flatMap(Double.init)
    let declared = agentMode || smoke ? nil : (windowConfig[name] as? NSNumber)?.doubleValue
    let value = override ?? declared ?? fallback
    return CGFloat(value.isFinite && value > 0 && value <= 16384 ? value : fallback)
}
let size = NSSize(width: windowDimension("width", fallback: 420), height: windowDimension("height", fallback: 860))

/// The process's physical footprint, what Activity Monitor calls its
/// memory: what a second session costs is read as the difference.
func footprint() -> UInt64 {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
    let kr = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
    }
    return kr == KERN_SUCCESS ? info.phys_footprint : 0
}

/// Under a script the window is the size asked for, as headless Chrome's
/// `--window-size` is, whatever the screen: AppKit would shorten an 860-point
/// window to fit a 720-point display, and a node the script reads at y 630
/// would then be below the viewport's edge, where no tap reaches it.
final class ExactWindow: NSWindow {
    override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect {
        agentMode ? frameRect : super.constrainFrameRect(frameRect, to: screen)
    }
}

/// One window: its own session and view (LLP 1031 D1). The first is the
/// app's; under `launch_handler`'s `navigate-new` every further document,
/// and every File ▸ New Window, gets one of these (LLP 1069.010 D4) — a ⌘N
/// window shows no document until one lands in it — and the windows share one
/// `tabbingIdentifier`, so AppKit's window tabs come with them.
final class DocumentWindow: NSObject, NSWindowDelegate {
    let label: String
    let session: ExactSession
    let view: ExactView
    let window: NSWindow
    /// The document delivered here last; `nil` while the window shows the
    /// app's own start, which the next document takes rather than opening
    /// a window beside it.
    var document: String?
    /// When this window was asked for (ms on the process clock) and the
    /// process's footprint just before: a later session's time to first
    /// pixel and its memory are measured from these.
    let openedMs = ExactEnv.wall()
    let footprintBefore = footprint()
    private var stamped = false

    init(label: String, first: Bool) {
        self.label = label
        session = exact.makeSession(delegate: adapter, label: label)
        if ExactEnv.agentFreezes { session.clock = 0 }
        if first, let url = launchURL {
            if ExactDevelopmentLink.claims(url) { launchDevelopmentURL = url }
            else { session.openURL(url) }
        }
        view = ExactView(session: session)
        if first { ExactEnv.stamp("Presenter (NSScrollView)") }
        window = ExactWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        super.init()
        // Present the reading surface at its final size as soon as it is ready.
        window.animationBehavior = .none
        // Nothing restores this window — a session boots its plan and the frame has
        // its own autosave below — so AppKit's restoration has nothing to keep for it.
        // Left on, a scroll view invalidates restorable state as it scrolls and AppKit
        // flushes it every so often on the main thread, waiting on the window server
        // as it does: 19 and 25 ms, measured, at the same second of two scrolls.
        window.isRestorable = false
        // An embedder's window is released by its owner; this one is held by
        // `windows` until it closes.
        window.isReleasedWhenClosed = false
        if first { ExactEnv.stamp("NSWindow") }
        // Until the app's `head` names it (LLP 1069.010 D6), the app's name.
        window.title = ExactEnv.appName
        window.tabbingIdentifier = ExactEnv.appMetadata["CFBundleIdentifier"] as? String ?? ExactEnv.appName
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
        window.delegate = self
        view.onViewportFit = { [weak self] in self?.coverChrome() }
        // Attaching the view can boot its embedded plan before this hook is installed.
        // Apply the current value even when the explicit boot keeps the same value.
        coverChrome()
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

    /// The view goes into the window once its session has booted (the
    /// first window's boot is `finishLaunching`'s), then the window's
    /// chrome follows the app's.
    func attach() {
        window.contentView = view
        view.attachWindowToolbar(to: window)
    }

    /// Hand documents to this window's session.
    @discardableResult
    func deliver(_ paths: [String]) -> Bool {
        guard !paths.isEmpty, ExactDocuments.deliver(paths, to: session) else { return false }
        document = paths.last
        return true
    }

    /// In front — never activated under a script (LLP 1012), so a running
    /// smoke never takes the focus from whoever is typing.
    func front() {
        if agentMode { window.orderFrontRegardless() } else {
            window.makeKeyAndOrderFront(nil)
            app.activate(ignoringOtherApps: true)
        }
    }

    /// What the agent's `state` says of this window (LLP 1069.010).
    var observed: [String: Any] {
        var row: [String: Any] = [
            "session": label, "title": window.title, "document": document ?? NSNull(),
            "key": window.isKeyWindow, "tabs": window.tabbedWindows?.count ?? 1,
            "footprintBefore": footprintBefore,
        ]
        // The first window's is from the process's start (what `metrics`
        // reports); a later one's from when it was asked for.
        if let drawn = session.firstDrawMs { row["firstPixelMs"] = ((drawn - (label == "main" ? 0 : openedMs)) * 10).rounded() / 10 }
        return row
    }

    func windowDidBecomeKey(_ notification: Notification) {
        DevMenu.session = session
        if !stamped, label == "main" { stamped = true; ExactEnv.stamp("windowDidBecomeKey") }
        agentReady()
    }
    /// Seen again, or no longer: the canvases follow (`Canvases.visible`).
    func windowDidChangeOcclusionState(_ notification: Notification) {
        session.occlusionChanged()
    }
    /// A closed window takes its session with it (LLP 1031 D2): its runner,
    /// its views, its label on the carrier.
    func windowWillClose(_ notification: Notification) {
        Agent.route(label, nil)
        // After AppKit has finished closing the window this holds.
        DispatchQueue.main.async { [self] in
            windows.removeAll { $0 === self }
            if DevMenu.session === session { DevMenu.session = windows.last?.session }
            session.destroy()
        }
    }
}

let firstWindow = DocumentWindow(label: "main", first: true)
/// Every open window, oldest first.
var windows = [firstWindow]
var windowCount = 1
let session = firstWindow.session
let view = firstWindow.view
let window = firstWindow.window
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

/// The window documents go to without a new one: the key window's, else
/// the newest.
func frontWindow() -> DocumentWindow? {
    windows.first { $0.window.isKeyWindow } ?? windows.first { $0.window.isMainWindow } ?? windows.last
}

/// A window of its own (LLP 1069.010 D4): a new session, booted at the
/// window's size from the plan the first one runs, then shown beside the
/// window in front — or as a tab of it when its tab bar is showing.
@discardableResult
func openWindow(asTab: Bool = false) -> DocumentWindow {
    windowCount += 1
    let beside = frontWindow()
    let w = DocumentWindow(label: "window-\(windowCount)", first: false)
    let size = w.window.contentLayoutRect.size
    if let path = ExactEnv.environment["EXACT_PLAN"], !ExactDevelopmentPlan(path).hasModule,
       let bytes = FileManager.default.contents(atPath: path) {
        w.session.boot(plan: bytes, size: size)
    } else {
        w.session.boot(size: size)
    }
    w.attach()
    w.coverChrome()
    windows.append(w)
    Agent.route(w.label, w.session)
    if let beside {
        if asTab || beside.window.tabGroup?.isTabBarVisible == true {
            beside.window.addTabbedWindow(w.window, ordered: .above)
        } else {
            w.window.setFrameTopLeftPoint(beside.window.cascadeTopLeft(from: NSPoint(x: beside.window.frame.minX, y: beside.window.frame.maxY)))
        }
    } else {
        w.window.center()
    }
    w.front()
    return w
}

/// Every route in ends here (LLP 1033 D3; the router `ExactDocuments.route`
/// names): the manifest's `launch_handler.client_mode` decides where each
/// document lands (LLP 1069.010 D4).
func route(_ paths: [String]) {
    guard !paths.isEmpty else { return }
    let front = frontWindow()
    switch ExactDocuments.launchMode {
    case "navigate-new":
        for path in paths {
            // The window in front takes the document when it shows none yet;
            // otherwise the document has a window of its own.
            let target = frontWindow().flatMap { $0.document == nil ? $0 : nil } ?? openWindow()
            if target.deliver([path]) { target.front() }
        }
    case "focus-existing":
        guard let front else { return }
        if front.document == nil { front.deliver(paths) }
        front.front()
    default:
        guard let front, front.deliver(paths) else { return }
        front.front()
    }
}
ExactDocuments.route = { route($0) }
Agent.hostState = {
    ["documents": [
        "launchMode": ExactDocuments.launchMode,
        "windows": windows.map(\.observed),
        "recent": ExactDocuments.recent,
        "openRecentMenu": DevMenu.openRecentTitles,
        "footprint": footprint(),
    ] as [String: Any]]
}

final class Delegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationDidFinishLaunching(_ notification: Notification) {
        ExactEnv.stamp("didFinishLaunching")
        finishLaunching()
    }
    /// Launch Services brought something: a development link, or documents —
    /// a Finder double-click, an Open With, `open -a`, a second `mdview`
    /// while this one runs, or File ▸ Open Recent. A document arriving now is
    /// why the app comes forward; one that opens nothing leaves the window
    /// where it was.
    func application(_ application: NSApplication, open urls: [URL]) {
        if let url = urls.first, ExactDevelopmentLink.claims(url) {
            if !session.booted { launchDevelopmentURL = url; return }
            ExactDevelopmentLink.open(url)
            frontWindow()?.front()
            return
        }
        // @ref LLP 1038 D8 — Launch Services delivers cold URLs before didFinishLaunching.
        if let url = urls.first(where: { !$0.isFileURL }) {
            if session.openURL(url) { frontWindow()?.front() }
            return
        }
        let documents = ExactDocuments.paths(of: urls)
        if !session.booted { launchDocuments += documents; return }
        route(documents)
    }

    /// File ▸ New Window (⌘N) and the tab bar's +: an empty window with a
    /// session of its own, whether or not the app opens documents (LLP
    /// 1069.010 D4).
    @objc func newWindowForTab(_ sender: Any?) {
        guard ExactDocuments.launchMode == "navigate-new" else { return }
        openWindow(asTab: sender is NSWindow)
    }
}

let delegate = Delegate()
app.delegate = delegate

var planWatch: DispatchSourceTimer?
var devPlanPath: String?
var launchDocuments: [String] = []
nonisolated(unsafe) var readySent = false

// Delay both attachment and boot until Launch Services has delivered launch URLs.
// @ref LLP 1038 D5/D8
func finishLaunching() {
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
    let bootError: String? = {
        // The view is not in the window yet, so its presenter has no size:
        // boot at the window's content layout size, which it reports once
        // attached. A zero viewport is refused (Runner(InvalidViewport)).
        let presented = session.viewportSize
        let size = presented.width > 0 && presented.height > 0 ? presented : window.contentLayoutRect.size
        let path = ExactEnv.environment["EXACT_PLAN"] ?? devPlanPath
        if let path, ExactDevelopmentPlan(path).hasModule, ExactEnv.environment["EXACT_PLAN"] != nil {
            DispatchQueue.main.async { ExactDevelopmentPlan(path).apply(to: exact) }
        }
        if let path, !ExactDevelopmentPlan(path).hasModule, let bytes = FileManager.default.contents(atPath: path) {
            planRefusal = session.boot(plan: bytes, size: size).error
            return planRefusal
        }
        if session.booted { return session.bootError }
        return session.boot(size: size).error
    }()
    // Boot the selected plan before attachment can auto-boot the embedded one.
    // A session mounts once, including its one autofocus attempt.
    firstWindow.attach()
    ExactEnv.stamp("contentView")
    let rustMs = session.rustMs
    let applyMs = session.applyMs
    let bootMs = session.bootMs
    // Becoming key can synchronously announce readiness. Initialize the guard
    // before ordering the window, not afterward (two stdin readers otherwise).
    if !launchDocuments.isEmpty {
        route(launchDocuments)
        launchDocuments.removeAll()
    }
    // The documents named on the command line, now that the first frame has
    // mounted the app's own nodes. Launch Services' route into a *running* app is
    // `application(_:open urls:)` above; a terminal's is this, and the two are
    // the same from here down — and a script's (`agent.mjs --open`), whose
    // driver names the whole command line. Not under the boot smoke, which
    // measures the app's own start. @ref LLP 1033 D3
    if !smoke {
        route(ExactDocuments.paths(in: CommandLine.arguments))
    }
    firstWindow.coverChrome()
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

    /// Agent mode: the driver owns the process from here — one JSON line in,
    /// one out. `ready` goes out once the first frame is applied and the window
    /// ordered front; an accessory app's window is not key until something
    /// asks, and a `type` asks (`AgentMac`).

    if agentReadable {
        DispatchQueue.main.async { agentReady() }
    }
    if smoke {
        print("boot \(String(format: "%.1f", bootMs)) ms; \(session.viewCount) views; root \(Int(session.rootSize.width))x\(Int(session.rootSize.height)); error \(bootError ?? "none")")
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

/// A refused EXACT_PLAN: attaching the view then boots the embedded plan,
/// which clears the session's error, but the driver asked for this one.
nonisolated(unsafe) var planRefusal: String?
func agentReady() {
    guard agentReadable, !readySent else { return }
    readySent = true
    Agent.reply(["ready": true, "boot": session.bootMs, "views": session.viewCount, "error": planRefusal ?? session.bootError ?? NSNull()])
    Agent.startStdio(sessions: [("main", session)])
}
app.run()

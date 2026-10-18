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

ProcessRelaunch.waitIfRequested()

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
        // `window.close()` (studio diary R17): the window this session
        // shows, without asking its `beforeunload` again.
        case "close": windows.first { $0.session === session }?.close()
        case "relaunch":
            guard !delegate.relaunching, delegate.quitting?.holding != true else { return }
            delegate.relaunching = true
            ProcessRelaunch.requestQuit(holding: { delegate.quitting?.holding == true }) { app.terminate(nil) }
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
/// Undeclared, a Mac document window's size (LLP 1115 wave 1), not a
/// phone's; a script keeps the phone-sized window its coordinates assume.
let scripted = agentMode || smoke
let size = NSSize(width: windowDimension("width", fallback: scripted ? 420 : 900),
                  height: windowDimension("height", fallback: scripted ? 860 : 640))
/// The smallest the window goes, undeclared: room for a phone-width column
/// and a few rows, as a hand-built window has a minimum (never AppKit's 1×1).
let minimumSize = NSSize(width: min(windowDimension("minWidth", fallback: 320), size.width),
                         height: min(windowDimension("minHeight", fallback: 240), size.height))

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
    var document: String? {
        // The proxy icon in the title bar (LLP 1069.010 D6): the document's
        // own file, for a path; a folder's shows the folder.
        didSet { window.representedURL = document.flatMap { $0.hasPrefix("/") ? URL(fileURLWithPath: $0) : nil } }
    }
    /// The app asked to close (`close()`): its `beforeunload` is not asked again.
    private(set) var closing = false
    /// When this window was asked for (ms on the process clock) and the
    /// process's footprint just before: a later session's time to first
    /// pixel and its memory are measured from these.
    let openedMs = ExactEnv.wall()
    let footprintBefore = footprint()
    private var stamped = false

    init(label: String, first: Bool) {
        self.label = label
        session = exact.makeSession(delegate: adapter, label: label)
        // Each document's window is its session's own; the process is several
        // sessions', so none owns it (LLP 1075.003.000.001 §2.1.1).
        session.hatchesOwnWindow = true
        if ExactEnv.agentFreezes { session.clock = 0 }
        if first, let url = launchURL {
            if ExactDevelopmentLink.claims(url) { launchDevelopmentURL = url }
            else { session.openURL(url) }
        }
        view = ExactView(session: session)
        if first { ExactEnv.stamp("Presenter (NSScrollView)") }
        window = ExactWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        super.init()
        // AppKit's own opening animation, and window tabs only where documents
        // have windows of their own (LLP 1115.000 D7.9).
        ExactDocuments.furnish(window)
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
        if !scripted {
            window.contentMinSize = minimumSize
            window.setContentSize(NSSize(width: max(size.width, minimumSize.width), height: max(size.height, minimumSize.height)))
        }
        // A new window puts the caret in its first key view (LLP 1115.000 D7.5),
        // unless the author's `autofocus` says where. The session does it the turn
        // after its first frame, when its launch autofocus runs, so no field editor
        // stands before the first pixel (`ExactView.focusesFirstKeyView`). Until
        // then nothing is focused, and a window whose first key view is in a canvas
        // that takes input stays so: AppKit's own pick as the window came on screen
        // gave such a canvas a caret in its first frame (found by the readback
        // fixture, LLP 1014).
        window.initialFirstResponder = view
        window.autorecalculatesKeyViewLoop = false
        view.focusesFirstKeyView = true
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
        sidebar?.viewportFitChanged()
        if cover {
            if !window.styleMask.contains(.fullSizeContentView) { window.styleMask.insert(.fullSizeContentView) }
            window.titlebarAppearsTransparent = !view.hasWindowToolbar
            window.titleVisibility = view.hasWindowToolbar ? .visible : .hidden
            window.backgroundColor = session.pageBackground
            if #available(macOS 11.0, *) { window.titlebarSeparatorStyle = view.hasWindowToolbar ? .automatic : .none }
        } else {
            // A sidebar runs the window's full height, under the title bar
            // (LLP 1115.002 D3): the content is full size for it, and the
            // page starts under the bar in its own pane.
            if sidebar != nil, !window.styleMask.contains(.fullSizeContentView) { window.styleMask.insert(.fullSizeContentView) }
            if sidebar == nil, window.styleMask.contains(.fullSizeContentView) { window.styleMask.remove(.fullSizeContentView) }
            window.titlebarAppearsTransparent = false
            window.titleVisibility = .visible
            if #available(macOS 11.0, *) { window.titlebarSeparatorStyle = .automatic }
        }
        // The bar a hand-built window has on this macOS, SwiftUI's own (LLP
        // 1115.002 D2): `.unified`, about 52 points on macOS 27, which is what
        // AppKit gives the style and not a number set here. A route's header
        // is this bar, so it has room for a title beside its items.
        if view.hasWindowToolbar { window.toolbarStyle = .unified }
        view.syncInsets()
    }

    /// The view goes into the window once its session has booted (the
    /// first window's boot is `finishLaunching`'s), then the window's
    /// chrome follows the app's.
    func attach() {
        view.onWindowSidebar = { [weak self] in self?.compose() }
        compose()
    }

    /// The window's content while the app's root tab bar is its sidebar.
    private var sidebar: ExactSidebarController?

    /// The window's content: the view; or, for an app whose root tab bar is
    /// the window's sidebar (LLP 1115.002 D3), a split view of that sidebar
    /// and the view. Composed when the view is attached and again when the
    /// app gains or loses such a tab bar. The window keeps its width and its
    /// top edge, and the area under its bars its height, as when a toolbar
    /// comes or goes. The page keeps the minimum width the window had, as
    /// its pane's: with the sidebar's own that is the narrowest the window
    /// goes. A script's window is the size asked for, so it has a minimum
    /// only where the script names one.
    private func compose() {
        // A sidebar for the root tab bar, a pane for a list's detail (LLP
        // 1115.002 D4), or both: one split view holds them.
        let rail = view.wantsWindowSidebar, wanted = rail || view.wantsWindowSplit
        if !wanted, sidebar == nil {
            // Neither, before or now: the view is the window's content.
            guard window.contentView !== view else { return }
            window.contentView = view
            view.attachWindowToolbar(to: window)
            return
        }
        // The same columns as before: nothing to compose.
        if let sidebar, wanted, sidebar.hasSidebar == rail { return }
        let was = window.frame, under = window.contentLayoutRect.height
        if let old = sidebar {
            old.release()
            sidebar = nil
            window.contentViewController = nil
        }
        if wanted {
            // Out of the window's content before the split view takes it.
            if window.contentView === view || window.contentView == nil { window.contentView = NSView() }
            let minimum = scripted && ExactEnv.environment["EXACT_WINDOW_MINWIDTH"] == nil ? 0 : minimumSize.width
            // A window that shows a list beside its detail is as wide as its
            // columns need, a script's too: Q6's "always, with a minimum
            // window width" has no narrower form to fall back to, and a
            // detail squeezed to what a 420-point window leaves shows nothing.
            let split = ExactSidebarController(view: view, autosaveName: sidebarName, minimumPageWidth: minimum, sidebar: rail,
                                               minimumDetailWidth: windowDimension("minWidth", fallback: 320))
            sidebar = split
            // Full-size content, then the split view: the order AppKit
            // places a source list's first row by.
            split.install(in: window)
        } else {
            window.contentView = view
        }
        coverChrome()
        view.attachWindowToolbar(to: window)
        coverChrome()
        guard !window.styleMask.contains(.fullScreen) else { return }
        let height = window.frame.height + under - window.contentLayoutRect.height
        window.setFrame(NSRect(x: was.minX, y: was.maxY - height, width: was.width, height: height), display: false)
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

    /// The app's `close()`: the window goes, unasked.
    func close() {
        closing = true
        window.close()
    }

    /// What the agent's `state` says of this window (LLP 1069.010).
    var observed: [String: Any] {
        var row: [String: Any] = [
            "session": label, "title": window.title, "document": document ?? NSNull(),
            "edited": window.isDocumentEdited, "representedURL": window.representedURL?.path ?? NSNull(),
            "key": window.isKeyWindow, "tabs": window.tabbedWindows?.count ?? 1,
            "footprintBefore": footprintBefore,
        ]
        // The first window's is from the process's start (what `metrics`
        // reports); a later one's from when it was asked for.
        if let drawn = session.firstDrawMs { row["firstPixelMs"] = ((drawn - (label == "main" ? 0 : openedMs)) * 10).rounded() / 10 }
        return row
    }

    /// The red button, File ▸ Close Window and ⌘W ask the app first
    /// (`beforeunload`, studio diary R17); a window it keeps stays.
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        closing || session.beforeUnload()
    }

    func windowDidBecomeKey(_ notification: Notification) {
        DevMenu.select(session: session)
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
            if DevMenu.session === session {
                if let next = windows.last { DevMenu.select(session: next.session) }
                else { DevMenu.session = nil }
            }
            // A window closed while the quit is held, or the last window of
            // an app that quits with it: the session stays for the quit, which
            // asks its module for its quit work, waits for it and destroys the
            // session after the hold (`applicationWillTerminate`).
            if delegate.quitting?.holding == true
                || windows.isEmpty && delegate.applicationShouldTerminateAfterLastWindowClosed(app) {
                closedForQuit.append(self)
                return
            }
            session.destroy()
        }
    }
}

// EXACT_PLAN naming a local bake with its module (LLP 1116.003) is the launch
// generation: the first boot runs it fresh, as a build that baked it would.
let launchRefusal = ExactEnv.environment["EXACT_PLAN"].flatMap { path in
    ExactDevelopmentPlan(path).hasModule ? ExactDevelopmentPlan(path).installInitial(on: exact) : nil
}
let firstWindow = DocumentWindow(label: "main", first: true)
/// Every open window, oldest first.
var windows = [firstWindow]
/// Windows closed whose sessions wait for the quit (`windowWillClose`).
var closedForQuit: [DocumentWindow] = []
var windowCount = 1
let session = firstWindow.session
let view = firstWindow.view
let window = firstWindow.window
window.center()
/// The frame the window was left at (its autosave). AppKit keeps the content
/// rect when `.fullSizeContentView` or a toolbar goes in, so a frame restored
/// only before the window's chrome lost the titlebar's height at every launch
/// (#113). It is restored before boot, so the plan boots near its size, and
/// again once the window has its final style (`finishLaunching`), which is
/// when the name goes on: setting it saves the current frame. Every app's
/// is kept, declared size or not, as AppKit apps keep theirs (LLP 1115 D8).
let frameName = scripted ? nil : (ExactEnv.appMetadata["CFBundleIdentifier"] as? String).map { $0 + ".main" }
/// Where a sidebar's width, and whether the person hid it, are kept (LLP
/// 1115.002 D3): AppKit's autosave, as the frame's is, and not a script's.
let sidebarName = scripted ? nil : (ExactEnv.appMetadata["CFBundleIdentifier"] as? String).map { $0 + ".sidebar" }
if let frameName { window.setFrameUsingName(frameName) }
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
// A document chosen in the app's own picker is the window's, as a routed
// one is: its proxy icon, and the next document goes to a window of its own.
ExactDocuments.shown = { session, url in
    windows.first { $0.session === session }?.document = url.standardizedFileURL.path
}
Agent.hostState = {
    ["documents": [
        "launchMode": ExactDocuments.launchMode,
        "windows": windows.map(\.observed),
        "recent": ExactDocuments.recent,
        "openRecentMenu": DevMenu.openRecentTitles,
        "footprint": footprint(),
    ] as [String: Any], "menus": DevMenu.menuBar]
}

final class Delegate: NSObject, NSApplicationDelegate {
    func applicationWillFinishLaunching(_ notification: Notification) {
        exact.installNotificationClicks { session in windows.first { $0.session === session }?.front() }
    }
    /// A one-window app quits with its window; one whose documents each
    /// get a window (`navigate-new`, LLP 1069.010 D4) stays, as a Mac
    /// document app does, for File ▸ New Window, Open and the Dock (LLP 1115 D8).
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        ExactDocuments.launchMode != "navigate-new"
    }
    /// The Dock icon clicked with no window showing: a new one.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        guard !flag, windows.isEmpty, ExactDocuments.launchMode == "navigate-new" else { return true }
        openWindow()
        return false
    }
    /// ⌘Q asks each window's app first (`beforeunload`, studio diary R17):
    /// the first that keeps itself open comes forward with whatever it asks
    /// and the quit stops there; once answered, its `close()` closes it, and
    /// the last window closing ends the app. Nothing is held while the app
    /// asks (LLP 1069.010 D7). Once the quit is decided, each native module
    /// starts its quit work (`ExactModule.willQuit`), and the quit is held
    /// while that work runs or storage an answer started is still landing:
    /// one hold, five seconds at most for all of it (LLP 1097 D10, LLP
    /// 1069.010 Q4). AppKit's modal loop keeps the main thread running.
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        // A repeated quit joins the existing hold: it neither asks the app
        // again, nor the modules, nor starts a fresh five-second deadline.
        if quitting?.holding == true { return .terminateLater }
        for w in windows where !w.closing && !w.session.beforeUnload() {
            relaunching = false
            w.front()
            return .terminateCancel
        }
        // The open windows' sessions, and the last window's if its closing started the quit.
        let sessions = (windows + closedForQuit).map(\.session)
        for session in sessions { session.willQuit() }
        let hold = StorageHold(bound: 5, pending: { sessions.contains { $0.storageOperations > 0 || $0.quitWorkPending } },
                               begin: { _ in }, end: { NSApp.reply(toApplicationShouldTerminate: true) })
        guard hold.hold() else { return .terminateNow }
        quitting = hold
        return .terminateLater
    }
    var quitting: StorageHold?
    var relaunching = false
    /// The quit is decided, however it came (⌘Q, the app menu, an Apple
    /// Event, SIGTERM, the last window closing): every session still live goes now,
    /// synchronously, so each native module's `destroy()` runs before the
    /// process ends ("destroyed with the session", LLP 1067.000 D3). The
    /// last window's session, closed, is left for this (`windowWillClose`);
    /// `destroy()` runs once in any case. The hold, if any, has ended.
    func applicationWillTerminate(_ notification: Notification) {
        for session in exact.sessions { session.destroy() }
        if relaunching {
            do { try ProcessRelaunch.start() }
            catch { fputs("exact: relaunch failed: \(error)\n", stderr) }
        }
    }
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
        var files: [URL] = []
        // @ref LLP 1038 D8 — Launch Services may batch cold or warm links.
        IncomingURLBatch.deliver(urls, development: { url in
            guard ExactDevelopmentLink.claims(url) else { return false }
            if !session.booted { launchDevelopmentURL = url }
            else { ExactDevelopmentLink.open(url); frontWindow()?.front() }
            return true
        }, document: { files.append($0) }, location: { url in
            // The first window's session may have closed with its window.
            let target = frontWindow() ?? openWindow()
            if target.session.openURL(url) { target.front() }
        })
        let documents = ExactDocuments.paths(of: files)
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

// A supervisor's SIGTERM follows the same cancellable quit as the menu
// (#269). The signal handler does no AppKit work; the retained source runs
// on the main queue. A caught signal, unlike SIG_IGN, resets to the default
// when an executed helper starts, so the helper can still be terminated.
signal(SIGTERM) { _ in }
let quitSignal = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
quitSignal.setEventHandler {
    // terminateLater spins AppKit's modal loop. Leave the dispatch callback
    // first, so the main queue can deliver storage completions in that loop.
    RunLoop.main.perform(inModes: [.common]) {
        // AppKit may force a repeated terminate without asking the delegate.
        // The first request already owns the quit hold and its deadline.
        if delegate.quitting?.holding == true { return }
        app.terminate(nil)
    }
}
quitSignal.resume()

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
        if let path, !ExactDevelopmentPlan(path).hasModule, let bytes = FileManager.default.contents(atPath: path) {
            planRefusal = session.boot(plan: bytes, size: size).error
            return planRefusal
        }
        // A plan with its module boots here as the launch generation; a refused one, the embedded plan.
        planRefusal = launchRefusal
        let error = session.booted ? session.bootError : session.boot(size: size).error
        return planRefusal ?? error
    }()
    // Boot the selected plan before attachment can auto-boot the embedded one.
    // A session mounts once, including its one autofocus attempt.
    firstWindow.attach()
    ExactEnv.stamp("contentView")
    let rustMs = session.rustMs
    let applyMs = session.applyMs
    let bootMs = session.bootMs
    // Its final style, then the frame it was left at — before a document
    // routed below can bring the window forward.
    firstWindow.coverChrome()
    if let frameName {
        window.setFrameUsingName(frameName)
        window.setFrameAutosaveName(frameName)
    }
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

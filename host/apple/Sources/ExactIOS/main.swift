// ExactIOS: the standalone iOS app as an adapter over ExactKit (LLP 1031
// D1) — a scene, a window, one session, one view, the default services, the
// dev menu. Host glue; the app is the static library (runner + kernel +
// data crate + baked plan), the same archive the macOS adapter links, built
// for the iOS target.
//
// EXACT_SMOKE=1 prints the boot time and the startup phases after the first
// frames and exits. EXACT_AGENT=1 with EXACT_AGENT_SOCKET=<path> is the agent
// API (LLP 1012, `Agent.swift`, `AgentIOS.swift`): the driver owns the clock
// and drives the app over a Unix socket. `xcrun simctl launch` passes the
// environment through as SIMCTL_CHILD_*; `bun host/apple/build.mjs --ios`
// builds, bundles, installs, and launches.
import ExactKit
import ExactComposition
import UIKit

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

let environment = ExactEnv.environment
let smoke = ExactEnv.smoke
let agentMode = ExactEnv.agentMode
setvbuf(stdout, nil, _IOLBF, 0)

let exact = ExactComposition.app
final class Adapter: ExactSessionDelegate {
    weak var window: UIWindow?
    var announced = false
    /// The capabilities: `setScheme` is the window's interface style —
    /// `light`, `dark`, or `system`, as the web's `color-scheme`, where
    /// following the user's preference is `.unspecified` rather than a third
    /// style; anything else is named and refused.
    func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
        switch name {
        case "setScheme":
            switch args.first as? String {
            case "dark": window?.overrideUserInterfaceStyle = .dark
            case "light": window?.overrideUserInterfaceStyle = .light
            default: window?.overrideUserInterfaceStyle = .unspecified
            }
        case "openURL":
            guard args.count == 1, let value = args.first as? String,
                  let url = URL(string: value),
                  ["http", "https", "mailto", "tel"].contains(url.scheme?.lowercased() ?? "") else {
                fputs("exact: openURL requires an external web, email or telephone URL\n", stderr)
                return
            }
            UIApplication.shared.open(url)
        default: FileHandle.standardError.write(Data("exact: unknown command \(name)\n".utf8))
        }
    }
    /// The first batch is in (the view booted the session at its first
    /// layout): the numbers, the agent, the smoke.
    func exactSession(_ session: ExactSession, didChange state: ExactSession.State) {
        guard !announced, state != .created, state != .destroyed else { return }
        announced = true
        DispatchQueue.main.async {
            if agentMode { agentReady() }
            if smoke { printSmoke() }
        }
    }
}
let adapter = Adapter()
// A session creates UIKit scroll views. Construct it only after UIKit starts:
// pre-UIApplicationMain creation reproduces the physical delayed-touch crash
// even with a plain plan and no dev menu (@ref LLP 1012, timing reduction).
nonisolated(unsafe) var session: ExactSession!
/// The view, made when the scene connects: UIKit's window comes with its
/// scene, not before.
nonisolated(unsafe) var exactView: ExactView!
nonisolated(unsafe) var devPlanPath: String?
nonisolated(unsafe) var planWatch: DispatchSourceTimer?

/// The dev loop (LLP 1007 §6, here): EXACT_DEV_PLAN names the plan the
/// resident compiler writes; when it changes, restart from it, state carried.
/// A URL is the wire form (LLP 1023 Stage 1): the app's one connection.
func watchPlan() {
    guard let planPath = environment["EXACT_DEV_PLAN"] else { return }
    if planPath.hasPrefix("http://") || planPath.hasPrefix("https://") {
        exact.connect(planPath)
        return
    }
    devPlanPath = planPath
    let candidate = ExactDevelopmentPlan(planPath)
    var last = candidate.hasModule ? [] : candidate.revision
    let t = DispatchSource.makeTimerSource(queue: .main)
    t.schedule(deadline: .now() + 0.1, repeating: 0.1)
    t.setEventHandler {
        let revision = candidate.revision
        guard revision != last else { return }
        last = revision
        candidate.apply(to: exact)
    }
    t.resume()
    planWatch = t
}

/// Agent mode: the driver owns the process from here — one JSON line in,
/// one out. `ready` goes out once the driver has connected, after the first
/// frame is applied.
nonisolated(unsafe) var readySent = false
/// A refused EXACT_PLAN: the view's first layout then boots the embedded
/// plan, which clears the session's error, but the driver asked for this one.
nonisolated(unsafe) var planRefusal: String?
func agentReady() {
    guard agentMode, !readySent, session.booted || session.bootError != nil else { return }
    readySent = true
    Agent.startSocket(ready: ["ready": true, "boot": session.bootMs, "views": session.viewCount, "error": planRefusal ?? session.bootError ?? NSNull(), "pid": Int(getpid())], sessions: [("main", session)])
}

/// The one screen: the view fills the controller's view; the plan boots at
/// the view's first layout (`ExactView.fit`).
/// The build's launch colour (the manifest's `launch`), else white.
let launchColor = UIColor(named: "ExactLaunch") ?? .white

final class Controller: UIViewController {
    // tvOS has no pointer lock.
    #if !os(tvOS)
    override var prefersPointerLocked: Bool { ExactPointerLock.preferred }
    #endif
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = launchColor
        exactView.frame = view.bounds
        exactView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(exactView)
    }
}

func printSmoke() {
    print("boot \(String(format: "%.1f", session.bootMs)) ms; \(session.viewCount) views; root \(Int(session.rootSize.width))x\(Int(session.rootSize.height)); error \(session.bootError ?? "none")")
    print("startup: exec→main \(execToMainMs.map { String(format: "%.1f", $0) } ?? "?") ms; main→didFinishLaunching \(String(format: "%.1f", ExactEnv.stamps.first(where: { $0.0 == "didFinishLaunching" })?.1 ?? 0)) ms; →window \(String(format: "%.1f", ExactEnv.stamps.first(where: { $0.0 == "window" })?.1 ?? 0)) ms")
    print("phases: process→boot \(String(format: "%.1f", ExactEnv.stamps.first(where: { $0.0 == "before boot" })?.1 ?? 0)) ms; runner+layout \(String(format: "%.1f", session.rustMs)) ms of which \(session.measureCount) text measurements (\(session.measureHits) cached) \(String(format: "%.1f", session.measureSeconds * 1000)) ms in CoreText; apply \(String(format: "%.1f", session.applyMs)) ms")
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
        print("painted \(session.firstDrawMs.map { String(format: "%.1f", $0) } ?? "?") ms")
        print("stamps: " + ExactEnv.stamps.map { "\($0.0) \(String(format: "%.1f", $0.1))" }.joined(separator: " · ") + " · first layout \(session.firstLayoutMs.map { String(format: "%.1f", $0) } ?? "?") · first draw \(session.firstDrawMs.map { String(format: "%.1f", $0) } ?? "?")")
        print("gpu: \(session.gpuStatus)")
        print("web: \(session.webStatus)")
        print("smoke ok")
        exit(0)
    }
}

final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        ExactEnv.stamp("didFinishLaunching")
        session = exact.makeSession(delegate: adapter, label: "main")
        if ExactEnv.agentFreezes { session.clock = 0 }
        return true
    }
    func application(_ application: UIApplication, configurationForConnecting connectingSceneSession: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let c = UISceneConfiguration(name: nil, sessionRole: connectingSceneSession.role)
        c.delegateClass = SceneDelegate.self
        return c
    }
    /// The dev menu's keyboard (`DevMenu.swift`): the app delegate is the
    /// responder every chain ends at, so these fire whatever has focus. Tab
    /// reaches it only when no node holds the focus (a node's own command
    /// comes first): it starts the session's sequential focus.
    override var keyCommands: [UIKeyCommand]? {
        let tab = [UIKeyCommand(input: "\t", modifierFlags: [], action: #selector(focusFirst)),
                   UIKeyCommand(input: "\t", modifierFlags: .shift, action: #selector(focusLast))]
        tab.forEach { $0.wantsPriorityOverSystemBehavior = true }
        guard DevMenu.enabled else { return tab }
        #if os(tvOS)
        // tvOS key commands carry no title.
        return tab + [
            UIKeyCommand(input: "d", modifierFlags: .command, action: #selector(devMenu)),
            UIKeyCommand(input: "r", modifierFlags: .command, action: #selector(devReload)),
            UIKeyCommand(input: "r", modifierFlags: [.command, .shift], action: #selector(devReload)),
        ]
        #else
        return tab + [
            UIKeyCommand(title: "Exact Menu", action: #selector(devMenu), input: "d", modifierFlags: .command),
            UIKeyCommand(title: "Reload", action: #selector(devReload), input: "r", modifierFlags: .command),
            UIKeyCommand(title: "Reload", action: #selector(devReload), input: "r", modifierFlags: [.command, .shift]),
        ]
        #endif
    }
    @objc func focusFirst() { session.moveFocus(backward: false) }
    @objc func focusLast() { session.moveFocus(backward: true) }
    @objc func devMenu() { DevMenu.toggle() }
    @objc func devReload() { DevMenu.reload() }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let ws = scene as? UIWindowScene else { return }
        ExactEnv.stamp("scene")
        // @ref LLP 1038 D8 — consume the launch URL before constructing the view.
        let incoming = connectionOptions.urlContexts.first?.url
            ?? connectionOptions.userActivities.first(where: { $0.activityType == NSUserActivityTypeBrowsingWeb })?.webpageURL
            ?? environment["EXACT_LAUNCH_URL"].flatMap { URL(string: $0) }
        // A file is a document from Files ("Open in"), opened in place
        // (LLP 1069.010 slice 4); it lands once the session has booted.
        if let url = incoming, url.isFileURL { ExactIOS.session.openDocument(url) }
        else if let url = incoming, !ExactDevelopmentLink.claims(url) { ExactIOS.session.openURL(url) }
        ExactEnv.stamp("before boot")
        let w = ExactWindow(windowScene: ws)
        // The launch screen's colour until the first frame names the canvas,
        // so nothing lighter or darker shows between them.
        w.backgroundColor = launchColor
        adapter.window = w
        exactView = ExactView(session: ExactIOS.session)
        // The root's background into the safe areas (`Presenter.paintCanvas`).
        exactView.onCanvasColor = { [weak w] color in w?.backgroundColor = color; w?.rootViewController?.view.backgroundColor = color }
        // The head's title is the scene's (LLP 1048.003 D1): the app switcher's name for it.
        exactView.onTitle = { [weak ws] title in ws?.title = title }
        watchPlan()
        // EXACT_PLAN=<file> boots that plan instead of the one baked into the
        // library — any compiled contract, no rebuild (smokes, fixtures).
        // The view boots the session at its first layout; a file plan is
        // booted here first, at the screen's size the view will take.
        if let path = environment["EXACT_PLAN"], ExactDevelopmentPlan(path).hasModule {
            DispatchQueue.main.async { ExactDevelopmentPlan(path).apply(to: exact) }
        }
        if let path = environment["EXACT_PLAN"] ?? devPlanPath, !ExactDevelopmentPlan(path).hasModule, let bytes = FileManager.default.contents(atPath: path) {
            planRefusal = ExactIOS.session.boot(plan: bytes, size: ws.coordinateSpace.bounds.inset(by: w.safeAreaInsets).size).error
        }
        let c = Controller()
        w.rootViewController = c
        window = w
        DevMenu.install(on: w, session: ExactIOS.session, controller: c, planPath: devPlanPath ?? environment["EXACT_PLAN"])
        w.makeKeyAndVisible()
        // A cold development connection needs the mounted session and window.
        if let url = incoming, ExactDevelopmentLink.claims(url) {
            DispatchQueue.main.async { ExactDevelopmentLink.open(url) }
        }
        ExactEnv.stamp("window")
    }
    func scene(_ scene: UIScene, openURLContexts contexts: Set<UIOpenURLContext>) {
        if let url = contexts.first?.url { open(url) }
    }
    func scene(_ scene: UIScene, continue userActivity: NSUserActivity) {
        if userActivity.activityType == NSUserActivityTypeBrowsingWeb, let url = userActivity.webpageURL { open(url) }
    }
    private func open(_ url: URL) {
        if url.isFileURL { ExactIOS.session.openDocument(url); return }
        if !ExactDevelopmentLink.open(url) { ExactIOS.session.openURL(url) }
    }
    /// Seen again: the canvases follow (`Canvases.visible`).
    func sceneDidBecomeActive(_ scene: UIScene) {
        ExactIOS.session.becameActive()
    }
}

ExactEnv.stamp("main")
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))

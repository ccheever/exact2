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
// environment through as SIMCTL_CHILD_*; `node host/apple/build.mjs --ios`
// builds, bundles, installs, and launches.
import ExactKit
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
/// EXACT_FPS=1: the frame rate, measured on the device — the display link
/// runs always and, once a second, what it delivered and what the canvases
/// cost goes to stderr and to a readout in the corner. A diagnostic, off by
/// default.
let fpsMode = environment["EXACT_FPS"] == "1"
nonisolated(unsafe) var fpsLabel: UILabel?
setvbuf(stdout, nil, _IOLBF, 0)

let exact = ExactApp.shared
final class Adapter: ExactSessionDelegate {
    weak var window: UIWindow?
    var announced = false
    /// The capabilities: `setScheme` is the window's interface style —
    /// light or dark, as the web's `color-scheme`; anything else is named
    /// and refused.
    func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
        switch name {
        case "setScheme": window?.overrideUserInterfaceStyle = (args.first as? String) == "dark" ? .dark : .light
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
let session = exact.makeSession(delegate: adapter, label: "main")
if agentMode { session.clock = 0 }
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
    var last = (try? FileManager.default.attributesOfItem(atPath: planPath)[.modificationDate] as? Date) ?? .distantPast
    let t = DispatchSource.makeTimerSource(queue: .main)
    t.schedule(deadline: .now() + 0.1, repeating: 0.1)
    t.setEventHandler {
        guard let m = (try? FileManager.default.attributesOfItem(atPath: planPath)[.modificationDate] as? Date), m > last else { return }
        last = m
        guard let bytes = FileManager.default.contents(atPath: planPath) else { return }
        exact.apply(bytes, label: String(planPath.split(separator: "/").last ?? "plan"))
    }
    t.resume()
    planWatch = t
}

/// Agent mode: the driver owns the process from here — one JSON line in,
/// one out. `ready` goes out once the driver has connected, after the first
/// frame is applied.
nonisolated(unsafe) var readySent = false
func agentReady() {
    guard agentMode, !readySent, session.booted || session.bootError != nil else { return }
    readySent = true
    Agent.startSocket(ready: ["ready": true, "boot": session.bootMs, "views": session.viewCount, "error": session.bootError ?? NSNull(), "pid": Int(getpid())], sessions: [("main", session)])
}

/// The one screen: the view fills the controller's view; the plan boots at
/// the view's first layout (`ExactView.fit`).
final class Controller: UIViewController {
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .white
        exactView.frame = view.bounds
        exactView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(exactView)
        if fpsMode {
            let l = UILabel()
            l.font = .monospacedDigitSystemFont(ofSize: 9, weight: .medium)
            l.textColor = .white
            l.backgroundColor = UIColor.black.withAlphaComponent(0.65)
            l.numberOfLines = 2
            l.isUserInteractionEnabled = false
            l.text = "fps …"
            view.addSubview(l)
            fpsLabel = l
            session.onFrameReport = { line in
                FileHandle.standardError.write(Data((line + "\n").utf8))
                fpsLabel?.text = line
                // And into the app's own Documents, for a phone: `xcrun devicectl
                // device copy from … --domain-type appDataContainer` reads it back
                // when no console is attached.
                if let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first {
                    let url = dir.appendingPathComponent("fps.log")
                    if let h = try? FileHandle(forWritingTo: url) { h.seekToEndOfFile(); h.write(Data((line + "\n").utf8)); try? h.close() }
                    else { try? Data((line + "\n").utf8).write(to: url) }
                }
            }
            session.runFramesAlways()
        }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        if let l = fpsLabel { l.frame = CGRect(x: 0, y: view.safeAreaInsets.top, width: view.bounds.width, height: 26); view.bringSubviewToFront(l) }
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
        return true
    }
    func application(_ application: UIApplication, configurationForConnecting connectingSceneSession: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let c = UISceneConfiguration(name: nil, sessionRole: connectingSceneSession.role)
        c.delegateClass = SceneDelegate.self
        return c
    }
    /// The dev menu's keyboard (`DevMenu.swift`): the app delegate is the
    /// responder every chain ends at, so these fire whatever has focus.
    override var keyCommands: [UIKeyCommand]? {
        guard DevMenu.enabled else { return nil }
        return [
            UIKeyCommand(title: "Exact Menu", action: #selector(devMenu), input: "d", modifierFlags: .command),
            UIKeyCommand(title: "Reload", action: #selector(devReload), input: "r", modifierFlags: .command),
            UIKeyCommand(title: "Reload", action: #selector(devReload), input: "r", modifierFlags: [.command, .shift]),
        ]
    }
    @objc func devMenu() { DevMenu.toggle() }
    @objc func devReload() { DevMenu.reload() }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let ws = scene as? UIWindowScene else { return }
        ExactEnv.stamp("scene")
        ExactEnv.stamp("before boot")
        let w = UIWindow(windowScene: ws)
        w.backgroundColor = .white
        adapter.window = w
        exactView = ExactView(session: ExactIOS.session)
        // The root's background into the safe areas (`Presenter.paintCanvas`).
        exactView.onCanvasColor = { [weak w] color in w?.backgroundColor = color; w?.rootViewController?.view.backgroundColor = color }
        watchPlan()
        // EXACT_PLAN=<file> boots that plan instead of the one baked into the
        // library — any compiled contract, no rebuild (smokes, fixtures).
        // The view boots the session at its first layout; a file plan is
        // booted here first, at the screen's size the view will take.
        if let path = environment["EXACT_PLAN"] ?? devPlanPath, let bytes = FileManager.default.contents(atPath: path) {
            ExactIOS.session.boot(plan: bytes, size: ws.coordinateSpace.bounds.inset(by: w.safeAreaInsets).size)
        }
        let c = Controller()
        w.rootViewController = c
        window = w
        DevMenu.install(on: w, session: ExactIOS.session, controller: c, planPath: devPlanPath ?? environment["EXACT_PLAN"])
        w.makeKeyAndVisible()
        ExactEnv.stamp("window")
    }
    /// Seen again: the canvases follow (`Canvases.visible`).
    func sceneDidBecomeActive(_ scene: UIScene) {
        ExactIOS.session.becameActive()
    }
}

ExactEnv.stamp("main")
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))

// The hostile sample host, iOS (LLP 1031 D10): a native UIKit app that is
// not Exact's — a navigation controller whose root screen stacks a native
// header over two sessions of the one plan the archive carries — the fixture
// `scripts/smoke.mjs host-ios` drives over the agent socket, each request
// naming its session by label (routing, not a ninth operation). What it
// exercises, in the smoke's order: two sessions with overlapping node ids
// answering apart; interleaved operations; a command from a session pushing a
// native screen over session a (its view unmounted, the session alive) and
// popping it (remounted) with both sessions intact; a bad candidate plan
// refused with the running apps kept; a session destroyed under the other,
// its handle refused by name after. The control file
// (EXACT_HOST_CONTROL=<path>, one command per appended line) stands in for
// the native buttons a real host has: `destroy <label>`, `unmount <label>`,
// `mount <label>`, `apply <plan path>`.
import ExactKit
import ExactComposition
import UIKit

setvbuf(stdout, nil, _IOLBF, 0)
let exact = ExactComposition.app
/// The host's own lines go to stderr: under the agent, stdout is the protocol.
func log(_ line: String) { FileHandle.standardError.write(Data((line + "\n").utf8)) }

/// The delegate the host owns: a command is an intention (LLP 1031 D5).
/// `setScheme("dark")` from any session pushes a native screen over session
/// a, `setScheme("light")` pops it — after the batch that carried it (D2).
final class HostDelegate: ExactSessionDelegate {
    var announced: Set<String> = []
    func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
        log("host: command \(name)\(args.isEmpty ? "" : " \(args)") from \(session.label)")
        switch (name, args.first as? String) {
        case ("setScheme", "dark"): root?.push(over: "a")
        case ("setScheme", "light"): root?.pop("a")
        default: break
        }
    }
    func exactSession(_ session: ExactSession, didChange state: ExactSession.State) {
        log("host: \(session.label) is \(state)")
        guard state != .created, state != .destroyed else { return }
        announced.insert(session.label)
        if announced.count == sessions.count { DispatchQueue.main.async { agentReady() } }
    }
}
let delegate = HostDelegate()

/// Two sessions of the one plan (LLP 1031 D1), each in its own pane, each
/// with its own clock under the agent.
// Session construction creates UIKit views; defer it until didFinishLaunching
// (@ref LLP 1012, physical pre-UIApplicationMain delayed-touch reproduction).
nonisolated(unsafe) var a: ExactSession!
nonisolated(unsafe) var b: ExactSession!
nonisolated(unsafe) var sessions: [(String, ExactSession)] = []
nonisolated(unsafe) var root: RootController?

/// A native screen pushed over a session: the Exact view leaves its pane
/// (unmounted, its session alive and unmounted, D1) while the navigation
/// controller shows the host's own screen; a pop reverses both.
final class NativeScreen: UIViewController {
    let label: String
    init(over label: String) { self.label = label; super.init(nibName: nil, bundle: nil); title = "Native screen" }
    required init?(coder: NSCoder) { nil }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        let text = UILabel()
        text.text = "Native screen over \(label)"
        text.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(text)
        NSLayoutConstraint.activate([text.centerXAnchor.constraint(equalTo: view.centerXAnchor), text.centerYAnchor.constraint(equalTo: view.centerYAnchor)])
    }
}

/// One session's pane: a view controller the host owns, the root of its
/// own navigation stack, so a native screen pushed over this session covers
/// this pane and no other — the phone's shape of the macOS split view.
final class PaneController: UIViewController {
    let label: String
    let exactView: ExactView
    init(label: String, session: ExactSession) {
        self.label = label
        exactView = ExactView(session: session)
        super.init(nibName: nil, bundle: nil)
        title = "Session \(label)"
    }
    required init?(coder: NSCoder) { nil }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        exactView.frame = view.bounds
        exactView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(exactView)
    }
}

/// The root screen: a native header, then the two sessions stacked, each
/// inside its own navigation controller.
final class RootController: UIViewController {
    let header = UILabel()
    var panes: [String: PaneController] = [:]
    var navs: [String: UINavigationController] = [:]
    var pushed: [String: NativeScreen] = [:]
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        header.text = "Native header (not Exact) — two sessions below"
        header.font = .preferredFont(forTextStyle: .footnote)
        header.textAlignment = .center
        view.addSubview(header)
        for (label, session) in sessions {
            let pane = PaneController(label: label, session: session)
            let navigation = UINavigationController(rootViewController: pane)
            addChild(navigation)
            view.addSubview(navigation.view)
            navigation.didMove(toParent: self)
            panes[label] = pane
            navs[label] = navigation
        }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let safe = view.bounds.inset(by: view.safeAreaInsets)
        header.frame = CGRect(x: safe.minX, y: safe.minY, width: safe.width, height: 24)
        let top = safe.minY + 28
        let each = (safe.maxY - top - 8) / 2
        for (i, (label, _)) in sessions.enumerated() {
            navs[label]?.view.frame = CGRect(x: safe.minX, y: top + CGFloat(i) * (each + 8), width: safe.width, height: each)
        }
    }
    var views: [String: ExactView] { panes.mapValues(\.exactView) }
    func push(over label: String) {
        guard pushed[label] == nil, let pane = panes[label] else { return }
        pane.exactView.removeFromSuperview()
        let screen = NativeScreen(over: label)
        pushed[label] = screen
        navs[label]?.pushViewController(screen, animated: false)
        log("host: pushed a native screen over \(label)")
    }
    func pop(_ label: String) {
        guard let screen = pushed.removeValue(forKey: label), let pane = panes[label] else { return }
        if navs[label]?.topViewController === screen { navs[label]?.popViewController(animated: false) }
        pane.exactView.frame = pane.view.bounds
        pane.view.addSubview(pane.exactView)
        log("host: popped the native screen over \(label); \(label) remounted")
    }
    func unmount(_ label: String) { panes[label]?.exactView.removeFromSuperview(); log("host: unmounted \(label)") }
    func mount(_ label: String) {
        guard let pane = panes[label] else { return }
        pane.exactView.frame = pane.view.bounds
        pane.view.addSubview(pane.exactView)
        log("host: mounted \(label)")
    }
    func destroy(_ label: String) {
        guard let session = sessions.first(where: { $0.0 == label })?.1 else { return }
        session.destroy()
        panes[label]?.exactView.removeFromSuperview()
        log("host: destroyed \(label)")
    }
}

/// The control file: the native buttons a real host has, as lines the
/// smoke appends.
nonisolated(unsafe) var consumed = 0
nonisolated(unsafe) var control: DispatchSourceTimer?
func watchControl() {
    guard let path = ExactEnv.environment["EXACT_HOST_CONTROL"] else { return }
    let t = DispatchSource.makeTimerSource(queue: .main)
    t.schedule(deadline: .now() + 0.1, repeating: 0.1)
    t.setEventHandler {
        guard let text = try? String(contentsOfFile: path, encoding: .utf8) else { return }
        let lines = text.split(separator: "\n").map(String.init)
        guard lines.count > consumed else { return }
        for line in lines[consumed...] {
            let parts = line.split(separator: " ", maxSplits: 1).map(String.init)
            guard let verb = parts.first else { continue }
            let arg = parts.count > 1 ? parts[1] : ""
            switch verb {
            case "destroy": root?.destroy(arg)
            case "unmount": root?.unmount(arg)
            case "mount": root?.mount(arg)
            case "apply":
                let bytes = FileManager.default.contents(atPath: arg) ?? Data()
                let ok = exact.apply(bytes, label: String(arg.split(separator: "/").last ?? "plan"))
                log("host: apply \(arg): \(ok ? "every session took it" : "refused")")
            default: log("host: unknown control \(line)")
            }
        }
        consumed = lines.count
    }
    t.resume()
    control = t
}

nonisolated(unsafe) var readySent = false
func agentReady() {
    guard ExactEnv.agentMode, !readySent else { return }
    readySent = true
    let error: Any = (a.bootError ?? b.bootError).map { $0 as Any } ?? NSNull()
    Agent.startSocket(ready: ["ready": true, "boot": max(a.bootMs, b.bootMs), "views": a.viewCount + b.viewCount, "sessions": sessions.map(\.0), "error": error, "pid": Int(getpid())], sessions: sessions)
}

final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        a = exact.makeSession(delegate: delegate, label: "a")
        b = exact.makeSession(delegate: delegate, label: "b")
        if ExactEnv.agentFreezes { a.clock = 0; b.clock = 0 }
        sessions = [("a", a), ("b", b)]
        return true
    }
    // A scene, not an app-wide window: an app that adopts no scene
    // lifecycle traps at launch on iOS 27.
    func application(_ application: UIApplication, configurationForConnecting connectingSceneSession: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let c = UISceneConfiguration(name: nil, sessionRole: connectingSceneSession.role)
        c.delegateClass = SceneDelegate.self
        return c
    }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let ws = scene as? UIWindowScene else { return }
        let w = UIWindow(windowScene: ws)
        let rootController = RootController()
        root = rootController
        w.rootViewController = rootController
        w.makeKeyAndVisible()
        window = w
        // EXACT_PLAN boots a file plan into both; otherwise each view boots
        // the archive's plan at its first size.
        if let path = ExactEnv.environment["EXACT_PLAN"], let bytes = FileManager.default.contents(atPath: path) {
            rootController.view.layoutIfNeeded()
            for (label, exactView) in rootController.views where !exactView.session.booted {
                _ = exactView.session.boot(plan: bytes, size: exactView.bounds.size)
                log("host: \(label) booted the file plan")
            }
        }
        watchControl()
    }
}

UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))

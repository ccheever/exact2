// ExactIOS: a window, a presenter, the clock — UIKit. Host glue; the app is
// the static library (runner + kernel + data crate + baked plan), the same
// archive the macOS presenter links, built for the iOS target.
//
// EXACT_SMOKE=1 prints the boot time and the startup phases after the first
// frames and exits. EXACT_AGENT=1 with EXACT_AGENT_SOCKET=<path> is the agent
// API (LLP 1012, `Agent.swift`, `AgentIOS.swift`): the driver owns the clock
// and drives the app over a Unix socket. `xcrun simctl launch` passes the
// environment through as SIMCTL_CHILD_*; `node host/apple/build.mjs --ios`
// builds, bundles, installs, and launches.
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

let environment = ProcessInfo.processInfo.environment
let smoke = environment["EXACT_SMOKE"] == "1"
let agentMode = environment["EXACT_AGENT"] == "1"
/// EXACT_FPS=1: the frame rate, measured on the device — the display link
/// runs always and, once a second, what it delivered (frames, the longest
/// gap) and what the canvases cost (renders, captures, their times) goes
/// to stderr and to a readout in the corner. A diagnostic, off by default.
let fpsMode = environment["EXACT_FPS"] == "1"
nonisolated(unsafe) var fpsLabel: UILabel?
setvbuf(stdout, nil, _IOLBF, 0)
let t0 = CACurrentMediaTime()
/// Milliseconds since `main`: the wall clock, for startup stamps.
func wall() -> Double { (CACurrentMediaTime() - t0) * 1000 }
/// The agent's clock (milliseconds), when the driver owns time; `nil` runs
/// on the wall clock.
nonisolated(unsafe) var agentClock: Double? = agentMode ? 0 : nil
/// The app's clock: what events, timers, motion, and canvases see.
func now() -> Double { agentClock ?? wall() }
/// Startup stamps, milliseconds from `main`, in order.
nonisolated(unsafe) var stamps: [(String, Double)] = []
func stamp(_ label: String) { stamps.append((label, wall())) }

/// The presenter, made when the scene connects: UIKit's window comes with
/// its scene, not before.
nonisolated(unsafe) var presenter: Presenter!
let canvases = Canvases()
nonisolated(unsafe) var clockTimer: Timer?
/// The first batch and its numbers, once booted.
nonisolated(unsafe) var boot: Batch?
nonisolated(unsafe) var bootMs = 0.0
nonisolated(unsafe) var rustMs = 0.0
nonisolated(unsafe) var applyMs = 0.0
nonisolated(unsafe) var tBoot = 0.0

/// Frames come from the display link, only while motion runs or a canvas
/// has something to render (LLP 1009 D4).
final class Frames: NSObject {
    var link: CADisplayLink?
    var motion = false
    /// The measure (EXACT_FPS): ticks since the last report, the longest
    /// gap between two, and when the last report went out.
    var ticks = 0
    var lastTick = 0.0
    var longest = 0.0
    var reported = 0.0
    @objc func tick(_ link: CADisplayLink) {
        if fpsMode { measure(link.timestamp) }
        if motion { apply(Exact.tick(now: now())) }
        let more = canvases.tick(now: now())
        run(motion || more || canvases.wantsFrames)
    }
    func measure(_ t: Double) {
        if lastTick > 0 { longest = max(longest, t - lastTick) }
        lastTick = t
        ticks += 1
        if reported == 0 { reported = t }
        guard t - reported >= 1 else { return }
        let c = canvases
        var line = String(format: "fps %.0f · longest gap %.1f ms · renders %d avg %.1f ms · captures %d avg %.1f ms", Double(ticks) / (t - reported), longest * 1000, c.windowRenders, c.windowRenderSeconds * 1000 / Double(max(1, c.windowRenders)), c.windowCaptures, c.windowCaptureSeconds * 1000 / Double(max(1, c.windowCaptures)))
        if !Capture.cpu, let sh = Shadow.shared, c.windowCaptures > 0 { line += String(format: " (mirror %.1f, gpu %.1f, read %.1f)", sh.lastMirrorMs, sh.lastRenderMs, sh.lastReadMs) }
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
        ticks = 0; longest = 0; reported = t
        c.windowRenders = 0; c.windowRenderSeconds = 0; c.windowCaptures = 0; c.windowCaptureSeconds = 0
    }
    func run(_ on: Bool) {
        let on = on || fpsMode
        if on, link == nil {
            let l = CADisplayLink(target: self, selector: #selector(tick(_:)))
            // The measure wants the display's real rate (a ProMotion phone's
            // 120), so a dropped frame is a dropped frame; the bundle's plist
            // opts in (CADisableMinimumFrameDurationOnPhone) or this is 60.
            if fpsMode { l.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120) }
            l.add(to: .main, forMode: .common)
            link = l
        } else if !on, let l = link {
            l.invalidate()
            link = nil
        }
    }
}
let frames = Frames()

func apply(_ batch: Batch) {
    presenter.apply(batch)
    frames.motion = batch.motion
    // The GPU module: after the first painted frame, only when a canvas exists.
    if firstDrawMs != nil { canvases.loadIfNeeded() } else { DispatchQueue.main.async { canvases.loadIfNeeded(); frames.run(frames.motion || canvases.wantsFrames) } }
    frames.run(batch.motion || canvases.wantsFrames)
    if batch.timers, clockTimer == nil, !agentMode {
        clockTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in apply(Exact.advance(now: now())) }
    }
}

/// The dev loop (LLP 1007 §6, here): EXACT_DEV_PLAN names the plan the
/// resident compiler writes; when it changes, restart from it, state carried.
nonisolated(unsafe) var planWatch: DispatchSourceTimer?
func watchPlan() {
    guard let planPath = environment["EXACT_DEV_PLAN"] else { return }
    var last = (try? FileManager.default.attributesOfItem(atPath: planPath)[.modificationDate] as? Date) ?? .distantPast
    let t = DispatchSource.makeTimerSource(queue: .main)
    t.schedule(deadline: .now() + 0.1, repeating: 0.1)
    t.setEventHandler {
        guard let m = (try? FileManager.default.attributesOfItem(atPath: planPath)[.modificationDate] as? Date), m > last else { return }
        last = m
        guard let bytes = FileManager.default.contents(atPath: planPath) else { return }
        let started = CACurrentMediaTime()
        presenter.reset()
        let size = presenter.viewport.bounds.size
        let batch = Exact.bootPlan(bytes, width: size.width, height: size.height)
        apply(batch)
        print("reloaded \(planPath.split(separator: "/").last ?? "plan") in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")")
    }
    t.resume()
    planWatch = t
}

/// Agent mode: the driver owns the process from here — one JSON line in,
/// one out. `ready` goes out once the driver has connected, after the first
/// frame is applied.
nonisolated(unsafe) var readySent = false
func agentReady() {
    guard agentMode, !readySent else { return }
    readySent = true
    Agent.start(ready: ["ready": true, "boot": bootMs, "views": presenter.views.count, "error": boot?.error ?? NSNull(), "pid": Int(getpid())])
}

/// Boot the plan under the viewport's first real size — the safe area,
/// where a browser lays a page out on a phone (no `viewport-fit=cover`):
/// under the status bar and above the home indicator, nothing.
func bootNow(_ size: CGSize) {
    stamp("before boot")
    tBoot = CACurrentMediaTime()
    watchPlan()
    // EXACT_PLAN=<file> boots that plan instead of the one baked into the
    // library — any compiled contract, no rebuild (smokes, fixtures).
    let b: Batch = {
        if let path = environment["EXACT_PLAN"], let bytes = FileManager.default.contents(atPath: path) {
            return Exact.bootPlan(bytes, width: size.width, height: size.height)
        }
        return Exact.boot(width: size.width, height: size.height)
    }()
    rustMs = (CACurrentMediaTime() - tBoot) * 1000
    stamp("runner + layout")
    let tApply = CACurrentMediaTime()
    apply(b)
    applyMs = (CACurrentMediaTime() - tApply) * 1000
    bootMs = wall()
    stamp("first frame applied")
    boot = b
    if agentMode { DispatchQueue.main.async { agentReady() } }
    if smoke {
        print("boot \(String(format: "%.1f", bootMs)) ms; \(presenter.views.count) views; root \(Int(presenter.root.subviews.first?.frame.width ?? 0))x\(Int(presenter.root.subviews.first?.frame.height ?? 0)); error \(b.error ?? "none")")
        print("startup: exec→main \(execToMainMs.map { String(format: "%.1f", $0) } ?? "?") ms; main→didFinishLaunching \(String(format: "%.1f", stamps.first(where: { $0.0 == "didFinishLaunching" })?.1 ?? 0)) ms; →window \(String(format: "%.1f", stamps.first(where: { $0.0 == "window" })?.1 ?? 0)) ms")
        print("phases: process→boot \(String(format: "%.1f", (tBoot - t0) * 1000)) ms; runner+layout \(String(format: "%.1f", rustMs)) ms of which \(measureCount) text measurements (\(measureHits) cached) \(String(format: "%.1f", measureSeconds * 1000)) ms in CoreText; apply \(String(format: "%.1f", applyMs)) ms")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
            print("painted \(firstDrawMs.map { String(format: "%.1f", $0) } ?? "?") ms")
            print("stamps: " + stamps.map { "\($0.0) \(String(format: "%.1f", $0.1))" }.joined(separator: " · ") + " · first layout \(firstLayoutMs.map { String(format: "%.1f", $0) } ?? "?") · first draw \(firstDrawMs.map { String(format: "%.1f", $0) } ?? "?")")
            print("gpu: \(canvases.module != nil ? "module loaded in \(String(format: "%.1f", canvases.loadedMs ?? 0)) ms; \(canvases.entries.count) canvases; \(canvases.rendered) renders" : "not loaded: \(canvases.failed ?? (canvases.entries.isEmpty ? "no canvas" : "not requested"))")")
            print("smoke ok")
            exit(0)
        }
    }
}

/// The one screen: the viewport fills the safe area; the plan boots at the
/// first layout and follows every later size (a rotation, a split).
final class Controller: UIViewController {
    var booted = false
    var lastSize = CGSize.zero
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .white
        view.addSubview(presenter.viewport)
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
            frames.run(true)
        }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let frame = view.bounds.inset(by: view.safeAreaInsets)
        if presenter.viewport.frame != frame { presenter.viewport.frame = frame }
        if let l = fpsLabel { l.frame = CGRect(x: frame.minX, y: frame.minY, width: frame.width, height: 26); view.bringSubviewToFront(l) }
        let size = frame.size
        guard size.width > 0, size.height > 0 else { return }
        if !booted {
            booted = true
            lastSize = size
            bootNow(size)
        } else if size != lastSize {
            lastSize = size
            apply(Exact.resize(width: size.width, height: size.height))
        }
    }
}

final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        stamp("didFinishLaunching")
        return true
    }
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
        stamp("scene")
        presenter = Presenter()
        presenter.onPress = { id in apply(Exact.press(id, now: now())) }
        presenter.onChange = { id, value in apply(Exact.change(id, value, now: now())) }
        presenter.onIntrinsic = { id, size in apply(Exact.intrinsic(id, width: size?.width ?? 0, height: size?.height ?? 0)) }
        let w = UIWindow(windowScene: ws)
        w.backgroundColor = .white
        // The root's background into the safe areas (`Presenter.paintCanvas`).
        presenter.onCanvasColor = { [weak w] color in w?.backgroundColor = color; w?.rootViewController?.view.backgroundColor = color }
        w.rootViewController = Controller()
        window = w
        w.makeKeyAndVisible()
        stamp("window")
    }
    /// Seen again: the canvases follow (`Canvases.visible`).
    func sceneDidBecomeActive(_ scene: UIScene) {
        frames.run(frames.motion || canvases.wantsFrames)
    }
}

stamp("main")
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))

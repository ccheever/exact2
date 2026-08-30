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
    var motionPending = false
    /// The measure (EXACT_FPS): ticks since the last report, the longest
    /// gap between two, and when the last report went out.
    var ticks = 0
    var lastTick = 0.0
    var longest = 0.0
    var reported = 0.0
    @objc func tick(_ link: CADisplayLink) {
        if fpsMode { measure(link.timestamp) }
        if motion, !agentMode, !motionPending {
            motionPending = true
            runtime.tick(now: now()) { [weak self] batch in
                self?.motionPending = false
                apply(batch)
            }
        }
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

/// A request's reply is in (LLP 1016 D2): the executor's thread says so;
/// main captures the current clock, then the serial runtime owner pumps.
func exactWake(_ ctx: UnsafeMutableRawPointer?) {
    DispatchQueue.main.async { runtime.pump(now: now()) }
}

func apply(_ batch: Batch) {
    presenter.apply(batch)
    frames.motion = batch.motion
    // The GPU module: after the first painted frame, only when a canvas exists.
    if firstDrawMs != nil { canvases.loadIfNeeded() } else { DispatchQueue.main.async { canvases.loadIfNeeded(); frames.run(frames.motion || canvases.wantsFrames) } }
    frames.run(batch.motion || canvases.wantsFrames)
    if batch.timers, clockTimer == nil, !agentMode {
        clockTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in runtime.advance(now: now()) }
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
        let size = presenter.viewport.bounds.size
        runtime.bootPlan(bytes, width: size.width, height: size.height) { batch in
            presenter.reset()
            apply(batch)
            controller?.rebooted()
            print("reloaded \(planPath.split(separator: "/").last ?? "plan") in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")")
        }
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
/// under the status bar and above the home indicator, nothing. A root that
/// says `viewport-fit="cover"` is reframed to the screen right after
/// (`Controller.fit`), the insets going to the kernel.
func bootNow(_ size: CGSize) {
    stamp("before boot")
    tBoot = CACurrentMediaTime()
    watchPlan()
    // EXACT_PLAN=<file> boots that plan instead of the one baked into the
    // library — any compiled contract, no rebuild (smokes, fixtures).
    let finish: (Batch) -> Void = { b in
        rustMs = (CACurrentMediaTime() - tBoot) * 1000
        stamp("runner + layout")
        let tApply = CACurrentMediaTime()
        apply(b)
        applyMs = (CACurrentMediaTime() - tApply) * 1000
        bootMs = wall()
        stamp("first frame applied")
        boot = b
        controller?.fit()
        if agentMode { runtime.barrier { agentReady() } }
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
    if let path = environment["EXACT_PLAN"], let bytes = FileManager.default.contents(atPath: path) {
        runtime.bootPlan(bytes, width: size.width, height: size.height, then: finish)
    } else {
        runtime.present(width: size.width, height: size.height, then: finish)
    }
}

/// The one screen: the viewport fills the safe area — or, when the first
/// root says `viewport-fit="cover"`, the whole screen, the safe-area insets
/// handed to the kernel for its `env()` lengths (LLP 1008 §9).
/// Baked-plan preparation starts before UIApplicationMain; its first sized
/// layout uses the window scene's safe viewport before the window is visible.
/// Every later size (a rotation, a split) and inset change follows it.
final class Controller: UIViewController {
    var booted = false
    var lastSize = CGSize.zero
    var lastInsets = UIEdgeInsets.zero
    var lastKeyboard: CGFloat = 0
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
        fit()
        if !booted, lastSize.width > 0, lastSize.height > 0 {
            booted = true
            bootNow(lastSize)
        }
    }

    /// Frame the viewport to the safe area or the screen — and, under
    /// `interactive-widget="resizes-content"`, to the keyboard's top, where
    /// the bottom inset is the keyboard's and not the home indicator's (the
    /// web's rule); under `overlays-content` the viewport stays and the
    /// overlap is `env(keyboard-inset-height)`. Once booted, tell the kernel
    /// about new insets, a new size, or a new keyboard overlap. A keyboard
    /// change prepares kernel batches on the runtime owner, then publishes
    /// the viewport and frames in one animation with the keyboard's remaining
    /// duration (LLP 1008 §9).
    func fit(duration: Double = 0, curve: UInt = 0) {
        let started = CACurrentMediaTime()
        let safe = view.safeAreaInsets
        let cover = presenter.viewportFit == "cover"
        var frame = cover ? view.bounds : view.bounds.inset(by: safe)
        var insets = cover ? safe : .zero
        var keyboardInset: CGFloat = 0
        let widget = presenter.interactiveWidget
        if widget == "resizes-content" || widget == "overlays-content" {
            let top = presenter.keyboardTop ?? .infinity
            keyboardInset = min(max(0, frame.maxY - max(top, frame.minY)), frame.height)
            if widget == "resizes-content", top < frame.maxY {
                frame.size.height = max(0, top - frame.minY)
                insets.bottom = 0
            }
        }
        let size = frame.size
        guard size.width > 0, size.height > 0 else { return }
        let setFrame = {
            if presenter.viewport.frame != frame { presenter.viewport.frame = frame }
            if let l = fpsLabel {
                l.frame = CGRect(x: frame.minX, y: frame.minY + safe.top, width: frame.width, height: 26)
                self.view.bringSubviewToFront(l)
            }
        }
        if !booted {
            setFrame()
            lastSize = size
            lastInsets = insets
            lastKeyboard = keyboardInset
            return
        }
        let changedInsets = insets != lastInsets
        let changedSize = size != lastSize
        let changedKeyboard = keyboardInset != lastKeyboard
        // The agent has no animation transaction: expose the visual viewport
        // immediately, while its barrier still waits for the prepared kernel
        // batches below. Real keyboard motion publishes both together.
        if agentMode || duration <= 0 { setFrame() }
        let overlay = widget == "overlays-content"
        let publish: ([Batch]) -> Void = { batches in
            let changes = {
                setFrame()
                for batch in batches { apply(batch) }
                self.lastInsets = insets
                self.lastSize = size
                self.lastKeyboard = keyboardInset
                presenter.insets = insets
                presenter.keyboardInset = keyboardInset
                // overlays-content: the author pads; scrolling the viewport
                // would move a full-bleed canvas (the night, the moon).
                if !overlay {
                    presenter.reveal(presenter.editing ?? presenter.views.values.first { $0.field?.isFirstResponder == true })
                }
            }
            let remaining = max(0, duration - (CACurrentMediaTime() - started))
            if agentMode || remaining <= 0 { changes() }
            else {
                // The overlay is painted through the night: capture
                // presentation frames for the keyboard's duration so the
                // mark and the form travel with the keys (LLP 1008 §9).
                canvases.keyboardAnimatingUntil = CACurrentMediaTime() + remaining
                frames.run(true)
                UIView.animate(withDuration: remaining, delay: 0, options: [UIView.AnimationOptions(rawValue: curve << 16), .beginFromCurrentState], animations: changes, completion: { _ in
                    canvases.keyboardAnimatingUntil = 0
                    for e in canvases.entries.values where e.through { e.view.needsCapture = true }
                    canvases.captureIfNeeded()
                    frames.run(frames.motion || canvases.wantsFrames)
                })
            }
        }
        guard changedInsets || changedSize || changedKeyboard else {
            publish([])
            return
        }
        let nextInsets: (CGFloat, CGFloat, CGFloat, CGFloat)? = changedInsets ? (insets.top, insets.right, insets.bottom, insets.left) : nil
        runtime.viewport(insets: nextInsets, size: changedSize ? size : nil, keyboard: changedKeyboard ? keyboardInset : nil, then: publish)
    }

    /// After a restart from a new plan (the dev loop): the new runner knows
    /// nothing of the insets — hand them over again, and fit the root.
    func rebooted() {
        if lastInsets != .zero { runtime.insets(top: lastInsets.top, right: lastInsets.right, bottom: lastInsets.bottom, left: lastInsets.left) }
        if lastKeyboard != 0 { runtime.keyboard(lastKeyboard) }
        fit()
    }
}
nonisolated(unsafe) weak var controller: Controller?

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
        stamp("scene")
        let w = UIWindow(windowScene: ws)
        w.backgroundColor = .white

        // UIWindow knows its scene's bounds and safe-area insets before it is
        // visible. Submit boot before constructing the presenter/controller
        // so runtime preparation overlaps all remaining scene and window work.
        let initialFrame = w.bounds.inset(by: w.safeAreaInsets)
        bootNow(initialFrame.size)

        presenter = Presenter()
        let c = Controller()
        controller = c
        w.rootViewController = c
        window = w
        c.lastSize = initialFrame.size
        c.lastInsets = .zero
        c.booted = true

        presenter.onPress = { id in runtime.press(id, now: now()) }
        presenter.onChange = { id, value in runtime.change(id, value, now: now()) }
        presenter.onIntrinsic = { id, size in runtime.intrinsic(id, width: size?.width ?? 0, height: size?.height ?? 0) }
        presenter.onHover = { id, over in runtime.hover(id, over: over, now: now()) }
        presenter.onFocus = { id in runtime.focus(id, now: now()) }
        presenter.onBlur = { id in runtime.blur(id, now: now()) }
        presenter.onKey = { id, name in runtime.key(id, name, now: now()) }
        presenter.onSubmit = { id in runtime.submit(id, now: now()) }
        // The capabilities: `setScheme` is the window's interface style —
        // light or dark, as the web's `color-scheme`; anything else is
        // named and refused.
        presenter.onCommand = { [weak w] name, args in
            switch name {
            case "setScheme": w?.overrideUserInterfaceStyle = (args.first as? String) == "dark" ? .dark : .light
            default: FileHandle.standardError.write(Data("exact: unknown command \(name)\n".utf8))
            }
        }
        // The root's background into the safe areas (`Presenter.paintCanvas`).
        presenter.onCanvasColor = { [weak w] color in w?.backgroundColor = color; w?.rootViewController?.view.backgroundColor = color }
        // The first root's `viewport-fit` changed (a restart, a prop): the
        // controller frames the viewport again.
        presenter.onViewportFit = { [weak w] in w?.rootViewController?.view.setNeedsLayout() }
        presenter.onKeyboardResize = { duration, curve in controller?.fit(duration: duration, curve: curve) }
        presenter.observeKeyboard()
        DevMenu.install(on: w)
        w.makeKeyAndVisible()
        stamp("window")
        w.layoutIfNeeded()
        runtime.publishReady()
    }
    func sceneWillEnterForeground(_ scene: UIScene) {
        runtime.publishReady()
    }
    /// Seen again: the canvases follow (`Canvases.visible`).
    func sceneDidBecomeActive(_ scene: UIScene) {
        runtime.publishReady()
        frames.run(frames.motion || canvases.wantsFrames)
    }
}

runtime.prepare()
stamp("main")
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))

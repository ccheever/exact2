// ExactMac: a window, a presenter, the clock. Host glue; the app is the
// static library (runner + kernel + data crate + baked plan).
//
// EXACT_SMOKE=1 prints the boot time and the startup phases after the first
// frames and exits — what `scripts/metrics.mjs` reads. EXACT_AGENT=1 is the
// agent API (LLP 1012, `Agent.swift`): the driver owns the clock and drives
// the app over stdio; `host/apple/smoke.mjs` is a script of its operations.
import AppKit

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

let smoke = ProcessInfo.processInfo.environment["EXACT_SMOKE"] == "1"
let agentMode = ProcessInfo.processInfo.environment["EXACT_AGENT"] == "1"
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
let app = NSApplication.shared
stamp("NSApplication.shared")
app.setActivationPolicy(.regular)
stamp("setActivationPolicy")
let appReadyMs = wall()

let presenter = Presenter()
let canvases = Canvases()
stamp("Presenter (NSScrollView)")
var clockTimer: Timer?

/// Frames come from the display link, only while motion runs or a canvas
/// has something to render (LLP 1009 D4).
final class Frames: NSObject {
    var link: CADisplayLink?
    var motion = false
    @objc func tick(_ link: CADisplayLink) {
        if motion { apply(Exact.tick(now: now())) }
        let more = canvases.tick(now: now())
        run(motion || more || canvases.wantsFrames)
    }
    func run(_ on: Bool) {
        if on, link == nil {
            let l = presenter.viewport.displayLink(target: self, selector: #selector(tick(_:)))
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
presenter.onPress = { id in apply(Exact.press(id, now: now())) }
presenter.onChange = { id, value in apply(Exact.change(id, value, now: now())) }
presenter.onIntrinsic = { id, size in apply(Exact.intrinsic(id, width: size?.width ?? 0, height: size?.height ?? 0)) }

let size = NSSize(width: 420, height: 860)
let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
stamp("NSWindow")
window.title = "Exact"
window.contentView = presenter.viewport
// Nothing is focused at launch — the web's rule (a page focuses no field on
// load). AppKit would otherwise make the first key view the first responder
// when the window becomes key, and a canvas holding an input would show a
// caret from its first frame (found by the readback fixture, LLP 1014).
window.initialFirstResponder = presenter.viewport
window.autorecalculatesKeyViewLoop = false
stamp("contentView")
window.center()
// Agent-driven apps run side by side (every session's smoke launches one):
// centred, each would cover the last and starve its Metal layer of drawables.
// Spread them by pid so no window is fully hidden.
if agentMode {
    let k = CGFloat(Int(getpid()) % 6)
    window.setFrameOrigin(NSPoint(x: window.frame.origin.x - 120 + 48 * k, y: window.frame.origin.y + 60 - 24 * k))
}
stamp("center")

final class Delegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationDidFinishLaunching(_ notification: Notification) { stamp("didFinishLaunching") }
    func windowDidBecomeKey(_ notification: Notification) {
        if !stamps.contains(where: { $0.0 == "windowDidBecomeKey" }) { stamp("windowDidBecomeKey") }
        agentReady()
    }
    func windowDidResize(_ notification: Notification) {
        let s = presenter.viewport.contentSize
        apply(Exact.resize(width: s.width, height: s.height))
    }
    /// Seen again, or no longer: the canvases follow (`Canvases.visible`).
    func windowDidChangeOcclusionState(_ notification: Notification) {
        canvases.occlusionChanged()
        frames.run(frames.motion || canvases.wantsFrames)
    }
}
let delegate = Delegate()
app.delegate = delegate
window.delegate = delegate

stamp("before boot")
let tBoot = CACurrentMediaTime()
// The dev loop (LLP 1007 §6, here): EXACT_DEV_PLAN names the plan the
// resident compiler writes; when it changes, restart from it, state carried.
var planWatch: DispatchSourceTimer?
if let planPath = ProcessInfo.processInfo.environment["EXACT_DEV_PLAN"] {
    var last = (try? FileManager.default.attributesOfItem(atPath: planPath)[.modificationDate] as? Date) ?? .distantPast
    let t = DispatchSource.makeTimerSource(queue: .main)
    t.schedule(deadline: .now() + 0.1, repeating: 0.1)
    t.setEventHandler {
        guard let m = (try? FileManager.default.attributesOfItem(atPath: planPath)[.modificationDate] as? Date), m > last else { return }
        last = m
        guard let bytes = FileManager.default.contents(atPath: planPath) else { return }
        let started = CACurrentMediaTime()
        presenter.reset()
        let size = presenter.viewport.contentSize
        let batch = Exact.bootPlan(bytes, width: size.width, height: size.height)
        apply(batch)
        print("reloaded \(planPath.split(separator: "/").last ?? "plan") in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")")
    }
    t.resume()
    planWatch = t
}

// EXACT_PLAN=<file> boots that plan instead of the one baked into the
// library — any compiled contract, no rebuild (smokes, fixtures).
let boot: Batch = {
    let size = presenter.viewport.contentSize
    if let path = ProcessInfo.processInfo.environment["EXACT_PLAN"], let bytes = FileManager.default.contents(atPath: path) {
        return Exact.bootPlan(bytes, width: size.width, height: size.height)
    }
    return Exact.boot(width: size.width, height: size.height)
}()
let rustMs = (CACurrentMediaTime() - tBoot) * 1000
stamp("runner + layout")
let tApply = CACurrentMediaTime()
apply(boot)
let applyMs = (CACurrentMediaTime() - tApply) * 1000
let bootMs = wall()
stamp("first frame applied")
window.makeKeyAndOrderFront(nil)
stamp("makeKeyAndOrderFront")
app.activate(ignoringOtherApps: true)
stamp("activate")

/// Agent mode: the driver owns the process from here — one JSON line in,
/// one out. `ready` goes out once the window is key (a `type` as the first
/// request needs the field editor, which needs a key window), or after a
/// second regardless, so a session never hangs where no window can be key.
nonisolated(unsafe) var readySent = false
func agentReady() {
    guard agentMode, !readySent else { return }
    readySent = true
    Agent.reply(["ready": true, "boot": bootMs, "views": presenter.views.count, "error": boot.error ?? NSNull()])
    Agent.start()
}
if agentMode {
    DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { agentReady() }
}
if smoke {
    print("boot \(String(format: "%.1f", bootMs)) ms; \(presenter.views.count) views; root \(Int(presenter.root.subviews.first?.frame.width ?? 0))x\(Int(presenter.root.subviews.first?.frame.height ?? 0)); error \(boot.error ?? "none")")
    print("startup: exec→main \(execToMainMs.map { String(format: "%.1f", $0) } ?? "?") ms; main→NSApplication \(String(format: "%.1f", appReadyMs)) ms; →window \(String(format: "%.1f", (tBoot - t0) * 1000 - appReadyMs)) ms")
    print("phases: process→boot \(String(format: "%.1f", (tBoot - t0) * 1000)) ms; runner+layout \(String(format: "%.1f", rustMs)) ms of which \(measureCount) text measurements (\(measureHits) cached) \(String(format: "%.1f", measureSeconds * 1000)) ms in CoreText; apply \(String(format: "%.1f", applyMs)) ms")
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
        print("painted \(firstDrawMs.map { String(format: "%.1f", $0) } ?? "?") ms")
        print("stamps: " + stamps.map { "\($0.0) \(String(format: "%.1f", $0.1))" }.joined(separator: " · ") + " · first layout \(firstLayoutMs.map { String(format: "%.1f", $0) } ?? "?") · first draw \(firstDrawMs.map { String(format: "%.1f", $0) } ?? "?")")
        print("gpu: \(canvases.module != nil ? "module loaded in \(String(format: "%.1f", canvases.loadedMs ?? 0)) ms; \(canvases.entries.count) canvases; \(canvases.rendered) renders" : "not loaded: \(canvases.failed ?? (canvases.entries.isEmpty ? "no canvas" : "not requested"))")")
        print("smoke ok")
        exit(0)
    }
}
app.run()

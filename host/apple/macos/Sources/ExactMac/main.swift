// ExactMac: a window, a presenter, the clock. Host glue; the app is the
// static library (runner + kernel + data crate + baked plan).
//
// EXACT_SMOKE=1 prints the boot time and a summary after the first frames
// and exits — the headless check `host/apple/smoke.mjs` reads.
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
setvbuf(stdout, nil, _IOLBF, 0)
let t0 = CACurrentMediaTime()
func now() -> Double { (CACurrentMediaTime() - t0) * 1000 }
/// Startup stamps, milliseconds from `main`, in order.
nonisolated(unsafe) var stamps: [(String, Double)] = []
func stamp(_ label: String) { stamps.append((label, now())) }
let app = NSApplication.shared
stamp("NSApplication.shared")
app.setActivationPolicy(.regular)
stamp("setActivationPolicy")
let appReadyMs = now()

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
    if batch.timers, clockTimer == nil {
        clockTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in apply(Exact.advance(now: now())) }
    }
}
presenter.onPress = { id in apply(Exact.press(id, now: now())) }
presenter.onChange = { id, value in apply(Exact.change(id, value, now: now())) }

let size = NSSize(width: 420, height: 860)
let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
stamp("NSWindow")
window.title = "Exact"
window.contentView = presenter.viewport
stamp("contentView")
window.center()
stamp("center")

final class Delegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationDidFinishLaunching(_ notification: Notification) { stamp("didFinishLaunching") }
    func windowDidBecomeKey(_ notification: Notification) { if !stamps.contains(where: { $0.0 == "windowDidBecomeKey" }) { stamp("windowDidBecomeKey") } }
    func windowDidResize(_ notification: Notification) {
        let s = presenter.viewport.contentSize
        apply(Exact.resize(width: s.width, height: s.height))
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

let boot = Exact.boot(width: presenter.viewport.contentSize.width, height: presenter.viewport.contentSize.height)
let rustMs = (CACurrentMediaTime() - tBoot) * 1000
stamp("runner + layout")
let tApply = CACurrentMediaTime()
apply(boot)
let applyMs = (CACurrentMediaTime() - tApply) * 1000
let bootMs = now()
stamp("first frame applied")
window.makeKeyAndOrderFront(nil)
stamp("makeKeyAndOrderFront")
app.activate(ignoringOtherApps: true)
stamp("activate")

if smoke {
    let ids = presenter.views.values.compactMap { $0.props["testId"] }.sorted()
    print("boot \(String(format: "%.1f", bootMs)) ms; \(presenter.views.count) views; root \(Int(presenter.root.subviews.first?.frame.width ?? 0))x\(Int(presenter.root.subviews.first?.frame.height ?? 0)); error \(boot.error ?? "none")")
    print("startup: exec→main \(execToMainMs.map { String(format: "%.1f", $0) } ?? "?") ms; main→NSApplication \(String(format: "%.1f", appReadyMs)) ms; →window \(String(format: "%.1f", (tBoot - t0) * 1000 - appReadyMs)) ms")
    print("phases: process→boot \(String(format: "%.1f", (tBoot - t0) * 1000)) ms; runner+layout \(String(format: "%.1f", rustMs)) ms of which \(measureCount) text measurements (\(measureHits) cached) \(String(format: "%.1f", measureSeconds * 1000)) ms in CoreText; apply \(String(format: "%.1f", applyMs)) ms")
    print("testIds \(ids.joined(separator: " "))")
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
        print("painted \(firstDrawMs.map { String(format: "%.1f", $0) } ?? "?") ms")
        print("stamps: " + stamps.map { "\($0.0) \(String(format: "%.1f", $0.1))" }.joined(separator: " · ") + " · first layout \(firstLayoutMs.map { String(format: "%.1f", $0) } ?? "?") · first draw \(firstDrawMs.map { String(format: "%.1f", $0) } ?? "?")")
        let texts = presenter.views.values.filter { $0.kind == "text" }.compactMap { $0.props["text"] }
        let station = presenter.views.values.first { $0.props["testId"] == "station-name" }
        print("gpu: \(canvases.module != nil ? "module loaded in \(String(format: "%.1f", canvases.loadedMs ?? 0)) ms; \(canvases.entries.count) canvases; \(canvases.rendered) renders" : "not loaded: \(canvases.failed ?? (canvases.entries.isEmpty ? "no canvas" : "not requested"))")")
        print("station \(station?.props["text"] ?? "?") frame \(station.map { "\(Int($0.frame.origin.x)),\(Int($0.frame.origin.y)) \(Int($0.frame.width))x\(Int($0.frame.height))" } ?? "?"); \(texts.count) texts")
        if let path = ProcessInfo.processInfo.environment["EXACT_SHOT"] {
            let v = presenter.viewport
            if let rep = v.bitmapImageRepForCachingDisplay(in: v.bounds) {
                v.cacheDisplay(in: v.bounds, to: rep)
                try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: path))
                print("shot \(path)")
            }
        }
        if let path = ProcessInfo.processInfo.environment["EXACT_SHOT_WINDOW"] {
            // The window server's picture of this window — Metal layers included,
            // which cacheDisplay cannot see. Needs screen-capture permission.
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            p.arguments = ["-x", "-o", "-l", String(window.windowNumber), path]
            try? p.run()
            p.waitUntilExit()
            print("window shot \(path) (\(p.terminationStatus == 0 ? "ok" : "failed"))")
        }
        print("smoke ok")
        exit(0)
    }
}
app.run()

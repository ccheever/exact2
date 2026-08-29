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
    // Scrolling: synthesize wheel events over the first `scroll` node and
    // report which scroll view moved — the window's document (the page) or
    // the node's own.
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
        guard let inner = presenter.views.values.first(where: { $0.kind == "scroll" || $0.kind == "list" }), let sv = inner.scroll, let win = inner.window else { print("scroll: no scroll node"); return }
        let before = (presenter.viewport.contentView.bounds.origin.y, sv.contentView.bounds.origin.y)
        let point = inner.convert(NSPoint(x: inner.bounds.midX, y: inner.bounds.minY + 40), to: nil)
        let screen = win.convertPoint(toScreen: point)
        let flippedY = (NSScreen.screens.first?.frame.height ?? 0) - screen.y
        // Phase-less wheel events: a gesture's phases would put AppKit's
        // top-level scroll view into a tracking loop that a synchronous
        // sendEvent cannot feed. AppKit declines to scroll a *nested* scroll
        // view for these — the presenter's fallback covers that case.
        func wheel() -> NSEvent? {
            guard let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: -40, wheel2: 0, wheel3: 0) else { return nil }
            cg.location = CGPoint(x: screen.x, y: flippedY)
            return NSEvent(cgEvent: cg)
        }
        let page = { Int(presenter.viewport.contentView.bounds.origin.y) }
        var report = "scroll: page document \(Int(presenter.root.frame.height)) tall in \(Int(presenter.viewport.contentSize.height)); inner document \(Int(sv.documentView?.frame.height ?? 0)) in \(Int(sv.bounds.height));"
        let p0 = page()
        if let e = wheel() { presenter.viewport.scrollWheel(with: e) }
        report += " direct→viewport \(p0)→\(page());"
        let p1 = page()
        if let e = wheel() { sv.scrollWheel(with: e) }
        report += " direct→inner: page \(p1)→\(page()), inner \(Int(before.1))→\(Int(sv.contentView.bounds.origin.y));"
        let p2 = page()
        let i2 = Int(sv.contentView.bounds.origin.y)
        // What the window does for a real trackpad: the hit-tested view gets it,
        // and the responder chain carries it up.
        let hit = win.contentView?.hitTest(point)
        if let e = wheel() { hit?.scrollWheel(with: e) }
        report += " via hit view (\(hit.map { String(describing: type(of: $0)) } ?? "none")): page \(p2)→\(page()), inner \(i2)→\(Int(sv.contentView.bounds.origin.y));"
        // An overflowing inner view: wheels over it scroll it until its edge,
        // then chain to the page.
        let p3 = page()
        let i0 = Int(sv.contentView.bounds.origin.y)
        let innerMax = Int(max(0, (sv.documentView?.frame.height ?? 0) - sv.contentView.bounds.height))
        var pageMovedEarly = false
        for _ in 0..<200 {
            let pBefore = page(), iBefore = Int(sv.contentView.bounds.origin.y)
            if let e = wheel() { hit?.scrollWheel(with: e) }
            if iBefore < innerMax && page() != pBefore { pageMovedEarly = true }
        }
        let pageMax = Int(max(0, presenter.root.frame.height - presenter.viewport.contentSize.height))
        report += " after 200 more: inner \(i0)→\(Int(sv.contentView.bounds.origin.y)) (limit \(innerMax)), page \(p3)→\(page()) (limit \(pageMax)), page moved before the inner limit: \(pageMovedEarly ? "yes" : "no"); viewport \(Int(presenter.viewport.contentSize.width)) wide"
        sv.contentView.scroll(to: NSPoint(x: 0, y: 100))
        sv.reflectScrolledClipView(sv.contentView)
        if let e = wheel() { report += "; deltas: scrolling \(e.scrollingDeltaY) delta \(e.deltaY) precise \(e.hasPreciseScrollingDeltas) phase \(e.phase.rawValue)" }
        report += "; diag: sv.frame \(sv.frame.size), clip \(sv.contentView.bounds), doc \(sv.documentView?.frame ?? .zero), programmatic scroll(to:100) → \(Int(sv.contentView.bounds.origin.y)), scroller \(sv.verticalScroller.map { "\($0.isEnabled)" } ?? "none"), inner.frame \(inner.frame)"
        print(report)
    }
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

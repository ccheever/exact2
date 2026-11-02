// scrollbench: black-box scroll/jump/resize benchmark for a macOS app.
// Launches the app, places its window through Accessibility, drives input through
// the window server (CGEvents posted at the HID tap with the pointer over the
// window, so AppKit's responsive scrolling engages), and observes presented frames
// with ScreenCaptureKit: cadence of changed frames, per-frame content shift, and
// the longest band without ink. Identical treatment for any app under test.
// Driven by run.mjs (README.md); the terminal needs Accessibility and Screen
// Recording.
import AppKit
import ApplicationServices
import CoreMedia
import CoreVideo
import CoreImage
import Foundation
import ScreenCaptureKit

setvbuf(stdout, nil, _IOLBF, 0)
_ = NSApplication.shared
NSApp.setActivationPolicy(.prohibited)
_ = CGMainDisplayID()
if !AXIsProcessTrusted() || !CGPreflightScreenCaptureAccess() || !CGPreflightPostEventAccess() {
    fputs("[scrollbench] grant this terminal Accessibility and Screen Recording (System Settings → Privacy & Security)\n", stderr)
    exit(3)
}

// ------------------------------------------------------------------ arguments
var opts: [String: String] = [:]
var appArgs: [String] = []
do {
    var it = CommandLine.arguments.dropFirst().makeIterator()
    while let a = it.next() {
        if a == "--" { while let r = it.next() { appArgs.append(r) }; break }
        if a.hasPrefix("--") { opts[String(a.dropFirst(2))] = it.next() ?? "" }
    }
}
func opt(_ k: String, _ d: String) -> String { opts[k] ?? d }
let execPath = opt("exec", "")
let outPath = opt("out", "/dev/stdout")
let mode = opt("mode", "trackpad")            // trackpad | wheel | jump | resize | idle | keys
let speed = Double(opt("speed", "3600"))!      // points/second of finger travel
let rate = Double(opt("rate", "120"))!         // input events/second
let duration = Double(opt("duration", "8"))!   // seconds of input
let settle = Double(opt("settle", "3"))!       // seconds between window-ready and input
let tail = Double(opt("tail", "1.5"))!
let direction = opt("direction", "down")       // down (toward end) | up | alternate
let frameSpec = opt("frame", "120,80,900,700").split(separator: ",").map { Double($0)! }
let jumps = opt("jumps", "0.5,0.1,0.9,0.3,0.7").split(separator: ",").map { Double($0)! }
let resizeWidths = opt("widths", "900,640").split(separator: ",").map { Double($0)! }
let envPairs = opt("env", "").split(separator: ";").map(String.init).filter { !$0.isEmpty }

guard !execPath.isEmpty else { fputs("--exec required\n", stderr); exit(2) }

func now() -> Double { Double(DispatchTime.now().uptimeNanoseconds) / 1e9 }
var tbInfo = mach_timebase_info_data_t(); mach_timebase_info(&tbInfo)
func machToSec(_ t: UInt64) -> Double { Double(t) * Double(tbInfo.numer) / Double(tbInfo.denom) / 1e9 }
func log(_ s: String) { fputs("[scrollbench] \(s)\n", stderr) }

// ------------------------------------------------------------------ launch
let proc = Process()
proc.executableURL = URL(fileURLWithPath: execPath)
proc.arguments = appArgs
var env = ProcessInfo.processInfo.environment
for p in envPairs { let kv = p.split(separator: "=", maxSplits: 1).map(String.init); if kv.count == 2 { env[kv[0]] = kv[1] } }
proc.environment = env
proc.standardOutput = FileHandle(forWritingAtPath: "/dev/null")
proc.standardError = FileHandle(forWritingAtPath: opt("applog", "/dev/null")) ?? FileHandle(forWritingAtPath: "/dev/null")
let launchAt = now()
let launchMach = machToSec(mach_absolute_time())
try! proc.run()
let pid = proc.processIdentifier
log("launched pid \(pid)")
var result: [String: Any] = ["pid": Int(pid), "exec": execPath, "args": appArgs, "mode": mode]

func finish(_ code: Int32) -> Never {
    if proc.isRunning { kill(pid, SIGTERM); usleep(300_000); if proc.isRunning { kill(pid, SIGKILL) } }
    let data = try! JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
    if outPath == "/dev/stdout" { print(String(data: data, encoding: .utf8)!) } else { try! data.write(to: URL(fileURLWithPath: outPath)) }
    exit(code)
}

// ------------------------------------------------------------------ window via AX
let axApp = AXUIElementCreateApplication(pid)
func axWindows() -> [AXUIElement] {
    var v: CFTypeRef?
    guard AXUIElementCopyAttributeValue(axApp, kAXWindowsAttribute as CFString, &v) == .success, let arr = v as? [AXUIElement] else { return [] }
    return arr
}
func axFrame(_ w: AXUIElement) -> CGRect? {
    var p: CFTypeRef?, s: CFTypeRef?
    guard AXUIElementCopyAttributeValue(w, kAXPositionAttribute as CFString, &p) == .success,
          AXUIElementCopyAttributeValue(w, kAXSizeAttribute as CFString, &s) == .success else { return nil }
    var pt = CGPoint.zero, sz = CGSize.zero
    AXValueGetValue(p as! AXValue, .cgPoint, &pt); AXValueGetValue(s as! AXValue, .cgSize, &sz)
    return CGRect(origin: pt, size: sz)
}
func axSetFrame(_ w: AXUIElement, _ r: CGRect) {
    var pt = r.origin, sz = r.size
    AXUIElementSetAttributeValue(w, kAXPositionAttribute as CFString, AXValueCreate(.cgPoint, &pt)!)
    AXUIElementSetAttributeValue(w, kAXSizeAttribute as CFString, AXValueCreate(.cgSize, &sz)!)
}
var window: AXUIElement?
let deadline = now() + 30
while now() < deadline {
    if let w = axWindows().max(by: { (axFrame($0)?.width ?? 0) * (axFrame($0)?.height ?? 0) < (axFrame($1)?.width ?? 0) * (axFrame($1)?.height ?? 0) }), let f = axFrame(w), f.width > 200 { window = w; break }
    usleep(20_000)
}
guard let win = window else { result["error"] = "no window"; finish(1) }
let windowAt = now()
result["window_after_launch_ms"] = (windowAt - launchAt) * 1000
let target = CGRect(x: frameSpec[0], y: frameSpec[1], width: frameSpec[2], height: frameSpec[3])
axSetFrame(win, target)
usleep(200_000)
axSetFrame(win, target)
let placed = axFrame(win) ?? target
result["window_frame"] = [placed.origin.x, placed.origin.y, placed.width, placed.height]

// ------------------------------------------------------------------ capture
struct Frame { var t: Double; var status: Int; var shift: Double?; var inkless: Double; var inkRows: Int; var sig: UInt64 }
final class Capture: NSObject, SCStreamOutput, SCStreamDelegate {
    var frames: [Frame] = []
    var prevProfile: [Float]? = nil
    let lock = NSLock()
    var width = 0, height = 0
    var chromeTop = 0          // rows to skip (title bar/toolbar), in capture pixels
    var bandLeft = 0, bandRight = 0
    var snapTimes: [Double] = []      // absolute times; the first complete frame after each is saved
    var snapDir = ""
    var snapped = 0
    func saveSnap(_ px: CVPixelBuffer, _ name: String) {
        let ci = CIImage(cvPixelBuffer: px)
        let ctx = CIContext()
        if let cg = ctx.createCGImage(ci, from: ci.extent) {
            let rep = NSBitmapImageRep(cgImage: cg)
            if let data = rep.representation(using: .png, properties: [:]) { try? data.write(to: URL(fileURLWithPath: snapDir + "/" + name)) }
        }
    }
    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen else { return }
        guard let atts = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]], let a = atts.first else { return }
        let statusRaw = a[.status] as? Int ?? -1
        let disp = (a[.displayTime] as? UInt64).map(machToSec) ?? now()
        guard statusRaw == SCFrameStatus.complete.rawValue, let px = CMSampleBufferGetImageBuffer(sb) else {
            lock.lock(); frames.append(Frame(t: disp, status: statusRaw, shift: nil, inkless: 0, inkRows: 0, sig: 0)); lock.unlock(); return
        }
        if snapped < snapTimes.count && disp >= snapTimes[snapped] && !snapDir.isEmpty { saveSnap(px, "snap\(snapped).png"); snapped += 1 }
        CVPixelBufferLockBaseAddress(px, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(px, .readOnly) }
        let w = CVPixelBufferGetWidth(px), h = CVPixelBufferGetHeight(px), bpr = CVPixelBufferGetBytesPerRow(px)
        let base = CVPixelBufferGetBaseAddress(px)!.assumingMemoryBound(to: UInt8.self)
        // Background luminance: mode of a coarse histogram over a sample.
        var hist = [Int](repeating: 0, count: 64)
        let y0 = min(chromeTop, h - 1)
        var yy = y0
        while yy < h { var xx = bandLeft; while xx < min(bandRight, w) { let p = base + yy * bpr + xx * 4; let l = (Int(p[2]) * 3 + Int(p[1]) * 6 + Int(p[0])) / 10; hist[l >> 2] += 1; xx += 7 }; yy += 5 }
        let bg = (hist.enumerated().max { $0.element < $1.element }!.offset << 2) + 2
        var profile = [Float](repeating: 0, count: h)
        var sig: UInt64 = 1469598103934665603
        for y in y0..<h {
            var ink = 0
            let row = base + y * bpr
            var x = bandLeft
            while x < min(bandRight, w) { let p = row + x * 4; let l = (Int(p[2]) * 3 + Int(p[1]) * 6 + Int(p[0])) / 10; if abs(l - bg) > 48 { ink += 1 }; x += 2 }
            profile[y] = Float(ink)
            sig = (sig ^ UInt64(ink)) &* 1099511628211
        }
        // Longest run of rows without ink, inside the band.
        var run = 0, best = 0, inkRows = 0
        for y in y0..<h { if profile[y] < 1 { run += 1; best = max(best, run) } else { run = 0; inkRows += 1 } }
        // Shift vs previous frame: content moving up by s means row y now shows old row y+s.
        var shift: Double? = nil
        if let prev = prevProfile, prev.count == h {
            var bestS = 0; var bestE = Float.greatestFiniteMagnitude
            let maxS = min(240, (h - y0) / 3)
            for s in -maxS...maxS {
                var e: Float = 0; var n = 0
                var y = y0 + maxS
                while y < h - maxS { e += abs(profile[y] - prev[y + s]); n += 1; y += 1 }
                if n > 0 { e /= Float(n) }
                if e < bestE { bestE = e; bestS = s }
            }
            shift = Double(bestS)
        }
        prevProfile = profile
        lock.lock(); frames.append(Frame(t: disp, status: statusRaw, shift: shift, inkless: Double(best), inkRows: inkRows, sig: sig)); lock.unlock()
    }
    func stream(_ stream: SCStream, didStopWithError error: Error) { log("stream stopped: \(error)") }
}

let cap = Capture()
var stream: SCStream?
let sem = DispatchSemaphore(value: 0)
var scWindow: SCWindow?
Task {
    do {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        scWindow = content.windows.filter { $0.owningApplication?.processID == pid }.max { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height }
    } catch { log("shareable content: \(error)") }
    sem.signal()
}
sem.wait()
guard let scw = scWindow else { result["error"] = "no SC window"; finish(1) }
let cfg = SCStreamConfiguration()
cfg.width = Int(scw.frame.width); cfg.height = Int(scw.frame.height)   // 1 capture pixel per point
cfg.minimumFrameInterval = CMTime(value: 1, timescale: 120)
cfg.queueDepth = 8
cfg.showsCursor = false
cfg.pixelFormat = kCVPixelFormatType_32BGRA
cap.width = cfg.width; cap.height = cfg.height
cap.chromeTop = Int(opt("chrome", "60"))!
cap.bandLeft = Int(Double(cfg.width) * Double(opt("bandleft", "0.30"))!)
cap.bandRight = Int(Double(cfg.width) * Double(opt("bandright", "0.95"))!)
let filter = SCContentFilter(desktopIndependentWindow: scw)
let s = SCStream(filter: filter, configuration: cfg, delegate: cap)
try! s.addStreamOutput(cap, type: .screen, sampleHandlerQueue: DispatchQueue(label: "cap", qos: .userInteractive))
stream = s
Task { do { try await s.startCapture() } catch { log("start capture: \(error)") }; sem.signal() }
sem.wait()
log("capturing \(cfg.width)x\(cfg.height)")

// ------------------------------------------------------------------ input
let center = CGPoint(x: placed.midX + placed.width * Double(opt("xoff", "0.12"))!, y: placed.midY)
let targetWindowID = Int64(scw.windowID)
let viaHID = opt("hid", "1") == "1"
func post(_ e: CGEvent) {
    e.location = center
    e.setIntegerValueField(.mouseEventWindowUnderMousePointer, value: targetWindowID)
    e.setIntegerValueField(.mouseEventWindowUnderMousePointerThatCanHandleThisEvent, value: targetWindowID)
    if viaHID { e.post(tap: .cghidEventTap) } else { e.postToPid(pid) }
}
func scrollEvent(_ dy: Int32, phase: Int64 = 0, momentum: Int64 = 0, continuous: Bool = true) -> CGEvent {
    let e = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: dy, wheel2: 0, wheel3: 0)!
    e.setIntegerValueField(.scrollWheelEventIsContinuous, value: continuous ? 1 : 0)
    if phase != 0 { e.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase) }
    if momentum != 0 { e.setIntegerValueField(.scrollWheelEventMomentumPhase, value: momentum) }
    // Point deltas (fixed-point too) so AppKit sees the same numbers either way.
    e.setIntegerValueField(.scrollWheelEventPointDeltaAxis1, value: Int64(dy))
    e.setDoubleValueField(.scrollWheelEventFixedPtDeltaAxis1, value: Double(dy))
    return e
}
func waitUntil(_ t: Double) { while now() < t { let d = t - now(); if d > 0.002 { usleep(UInt32((d - 0.001) * 1e6)) } } }

Thread.sleep(forTimeInterval: settle)
// Human-input guard and pointer placement: HID-routed events go to the window under the pointer.
func humanIdle() -> Double {
    [CGEventType.mouseMoved, .keyDown, .leftMouseDown, .rightMouseDown, .leftMouseDragged].map { CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: $0) }.min()!
}
let idleBefore = humanIdle()
result["human_idle_before_s"] = idleBefore
let previousFront = NSWorkspace.shared.frontmostApplication
let savedCursor = CGEvent(source: nil)?.location ?? .zero
if viaHID {
    NSRunningApplication(processIdentifier: pid)?.activate()
    AXUIElementPerformAction(win, kAXRaiseAction as CFString)
    usleep(300_000)
    CGWarpMouseCursorPosition(center)
    CGAssociateMouseAndMouseCursorPosition(1)
    usleep(200_000)
}
let guardStart = now()
var inputs: [[Double]] = []   // [due, posted]
let inputStart = now() + 0.05
result["input_start"] = inputStart
cap.snapDir = opt("snapdir", "")
cap.snapTimes = opt("snaps", "").split(separator: ",").compactMap { Double($0) }.map { inputStart + $0 }
let perEvent = speed / rate
let n = Int(duration * rate)
var jumpTimes: [Double] = []

func axScrollBar() -> AXUIElement? {
    // Depth-first: the first vertical scroll bar under the window whose parent area is the largest.
    var best: (AXUIElement, CGFloat)? = nil
    func visit(_ e: AXUIElement, _ depth: Int) {
        if depth > 14 { return }
        var role: CFTypeRef?; AXUIElementCopyAttributeValue(e, kAXRoleAttribute as CFString, &role)
        if (role as? String) == kAXScrollAreaRole as String {
            var sb: CFTypeRef?
            if AXUIElementCopyAttributeValue(e, kAXVerticalScrollBarAttribute as CFString, &sb) == .success, let bar = sb {
                var sz: CFTypeRef?; AXUIElementCopyAttributeValue(e, kAXSizeAttribute as CFString, &sz)
                var size = CGSize.zero; if let z = sz { AXValueGetValue(z as! AXValue, .cgSize, &size) }
                let area = size.width * size.height
                if best == nil || area > best!.1 { best = ((bar as! AXUIElement), area) }
            }
        }
        var kids: CFTypeRef?
        if AXUIElementCopyAttributeValue(e, kAXChildrenAttribute as CFString, &kids) == .success, let arr = kids as? [AXUIElement] { for k in arr { visit(k, depth + 1) } }
    }
    visit(win, 0)
    return best?.0
}

switch mode {
case "trackpad":
    // One continuous two-finger gesture per 2 s, then a momentum tail; repeated for the duration.
    var i = 0
    var sign: Int32 = direction == "up" ? 1 : -1
    let gestureEvents = Int(rate * 2.0)
    while i < n {
        let startI = i
        for k in 0..<gestureEvents where i < n {
            let due = inputStart + Double(i) / rate
            waitUntil(due)
            let phase: Int64 = k == 0 ? 1 : (k == gestureEvents - 1 || i == n - 1 ? 4 : 2)
            let dy = phase == 4 ? 0 : sign * Int32(perEvent.rounded())
            post(scrollEvent(dy, phase: phase))
            inputs.append([due, now()]); i += 1
        }
        _ = startI
        // Momentum: 0.35 s of decaying deltas, part of the budget of events.
        let momentumEvents = Int(rate * 0.35)
        for k in 0..<momentumEvents where i < n {
            let due = inputStart + Double(i) / rate
            waitUntil(due)
            let m: Int64 = k == 0 ? 1 : (k == momentumEvents - 1 || i == n - 1 ? 3 : 2)
            let decay = pow(0.9, Double(k))
            let dy = m == 3 ? 0 : sign * Int32((perEvent * decay).rounded())
            post(scrollEvent(dy, momentum: m))
            inputs.append([due, now()]); i += 1
        }
        if direction == "alternate" { sign = -sign }
    }
case "wheel":
    var sign: Int32 = direction == "up" ? 1 : -1
    for i in 0..<n {
        let due = inputStart + Double(i) / rate
        waitUntil(due)
        if direction == "alternate" && i > 0 && i % Int(rate) == 0 { sign = -sign }
        post(scrollEvent(sign * Int32(perEvent.rounded()), continuous: opt("continuous", "1") == "1"))
        inputs.append([due, now()])
    }
case "jump":
    guard let bar = axScrollBar() else { result["error"] = "no scroll bar"; finish(1) }
    let gap = duration / Double(max(1, jumps.count))
    for (k, v) in jumps.enumerated() {
        let due = inputStart + Double(k) * gap
        waitUntil(due)
        let t0 = now()
        AXUIElementSetAttributeValue(bar, kAXValueAttribute as CFString, NSNumber(value: v))
        jumpTimes.append(t0)
        inputs.append([due, t0])
    }
case "resize":
    let steps = Int(duration * 60)
    let lo = resizeWidths.min()!, hi = resizeWidths.max()!
    for k in 0..<steps {
        let due = inputStart + Double(k) / 60
        waitUntil(due)
        let phase = Double(k % 120) / 120
        let wv = lo + (hi - lo) * (0.5 + 0.5 * cos(phase * 2 * .pi))
        var sz = CGSize(width: wv.rounded(), height: placed.height)
        AXUIElementSetAttributeValue(win, kAXSizeAttribute as CFString, AXValueCreate(.cgSize, &sz)!)
        inputs.append([due, now()])
    }
default: // idle
    Thread.sleep(forTimeInterval: duration)
}
let inputEnd = now()
Thread.sleep(forTimeInterval: tail)
let humanDuring = humanIdle() < (now() - guardStart) - 0.05
result["human_interference"] = humanDuring
if viaHID {
    CGWarpMouseCursorPosition(savedCursor)
    if let f = previousFront, f.processIdentifier != pid { f.activate() }
}
Task { try? await s.stopCapture(); sem.signal() }
sem.wait()

// footprint of the app after the run
func footprintMB() -> Double? {
    let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/bin/footprint"); p.arguments = ["-p", String(pid)]
    let pipe = Pipe(); p.standardOutput = pipe; p.standardError = FileHandle(forWritingAtPath: "/dev/null")
    try? p.run(); p.waitUntilExit()
    let out = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
    // "Footprint: 123 MB" style line
    for line in out.split(separator: "\n") where line.contains("phys_footprint:") || line.contains("Footprint:") {
        let parts = line.split(whereSeparator: { $0 == " " || $0 == "\t" })
        for (i, p) in parts.enumerated() where Double(p) != nil && i + 1 < parts.count {
            let v = Double(p)!; let u = parts[i + 1]
            if u.hasPrefix("GB") { return v * 1024 }; if u.hasPrefix("MB") { return v }; if u.hasPrefix("KB") { return v / 1024 }
        }
    }
    return nil
}
result["footprint_mb"] = opt("footprint", "1") == "1" ? (footprintMB() ?? -1) : -1

// ------------------------------------------------------------------ analysis
let inkThreshold0 = Int(opt("inkrows", "60"))!
cap.lock.lock(); let frames = cap.frames; cap.lock.unlock()
let complete = frames.filter { $0.status == SCFrameStatus.complete.rawValue }
let activeLo = inputStart, activeHi = inputEnd + 0.1
var changed: [Frame] = []
do { var last: UInt64 = 0; for f in complete { if f.sig != last { changed.append(f); last = f.sig } } }
let act = changed.filter { $0.t >= activeLo && $0.t <= activeHi }
var gaps: [Double] = []
for i in 1..<max(1, act.count) { gaps.append((act[i].t - act[i - 1].t) * 1000) }
let sortedG = gaps.sorted()
func pct(_ a: [Double], _ p: Double) -> Double { a.isEmpty ? 0 : a[min(a.count - 1, Int(Double(a.count - 1) * p))] }
let refresh = 1000.0 / Double(NSScreen.main?.maximumFramesPerSecond ?? 60)
var hitchMs = 0.0, dropped = 0, longGaps = 0
for g in gaps where g > refresh * 1.5 { hitchMs += g - refresh; dropped += Int((g / refresh).rounded()) - 1; longGaps += 1 }
let activeDur = max(0.001, (act.last?.t ?? activeHi) - (act.first?.t ?? activeLo))
let shifts = act.compactMap { $0.shift }
let moving = shifts.filter { $0 != 0 }.count
let inklessMax = act.map { $0.inkless }.max() ?? 0
let inkless250 = act.filter { $0.inkless > 250 }.count
var jumpLand: [[Double]] = []
// Per jump: ms to the first changed frame, ms to the first changed frame with no
// blank band over 250 px (content at the destination), ms to the last change
// within 800 ms (settled), and how many changed frames showed a blank band.
for t in jumpTimes {
    let after = changed.filter { $0.t > t && $0.t < t + 0.8 }
    guard let f = after.first else { jumpLand.append([-1, -1, -1, 0]); continue }
    let content = after.first { $0.inkless <= 250 && $0.inkRows >= inkThreshold0 }
    jumpLand.append([(f.t - t) * 1000, content.map { ($0.t - t) * 1000 } ?? -1, (after.last!.t - t) * 1000, Double(after.filter { $0.inkless > 250 || $0.inkRows < inkThreshold0 }.count)])
}
let inputLag = inputs.map { ($0[1] - $0[0]) * 1000 }
result["refresh_ms"] = refresh
result["frames_total"] = frames.count
result["changed_in_active"] = act.count
result["active_s"] = activeDur
result["presented_fps"] = Double(act.count) / activeDur
result["gap_ms_p50_p95_p99_max"] = [pct(sortedG, 0.5), pct(sortedG, 0.95), pct(sortedG, 0.99), sortedG.last ?? 0]
result["hitch_ms_per_s"] = hitchMs / activeDur
result["dropped_frames"] = dropped
result["long_gaps"] = longGaps
result["moving_frames"] = moving
result["total_shift_px"] = shifts.reduce(0, +)
result["expected_travel_pt"] = mode == "trackpad" || mode == "wheel" ? speed * duration : 0
result["inkless_max_px"] = inklessMax
result["frames_inkless_over_250"] = inkless250
result["post_lag_ms_p99_max"] = [pct(inputLag.sorted(), 0.99), inputLag.max() ?? 0]
result["jump_first_last_count_inkless"] = jumpLand
let inkThreshold = Int(opt("inkrows", "60"))!
result["first_content_after_launch_ms"] = (complete.first { $0.inkRows >= inkThreshold }.map { ($0.t - launchMach) * 1000 }) ?? -1
result["first_frame_after_launch_ms"] = (frames.first.map { ($0.t - launchMach) * 1000 }) ?? -1
result["clock_check_ms"] = (launchAt - launchMach) * 1000
result["ink_rows_at_input"] = (complete.last { $0.t <= inputStart }?.inkRows) ?? -1
result["first_move_after_input_ms"] = (act.first { ($0.shift ?? 0) != 0 }.map { ($0.t - inputStart) * 1000 }) ?? -1
if opt("dumpframes", "0") == "1" {
    result["frames"] = frames.map { [$0.t - inputStart, Double($0.status), $0.shift ?? -9999, $0.inkless, Double($0.inkRows)] }
}
finish(0)

// launchbench: launch-to-content timing observed on the display.
// Starts a ScreenCaptureKit stream on the main display that excludes every window
// existing before launch, launches the app, and records every frame's ink profile
// in the expected window band. Reports the first frame that differs from the
// pre-launch capture (a window appeared) and the first frame whose document band
// matches the app's own settled content (first correct content). The window has
// to open inside --region. Driven by run.mjs (README.md); the terminal needs
// Screen Recording.
import AppKit
import CoreMedia
import CoreVideo
import Foundation
import ScreenCaptureKit

setvbuf(stdout, nil, _IOLBF, 0)
_ = NSApplication.shared
NSApp.setActivationPolicy(.prohibited)
if !CGPreflightScreenCaptureAccess() {
    fputs("[launchbench] grant this terminal Screen Recording (System Settings → Privacy & Security)\n", stderr)
    exit(3)
}

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
let region = opt("region", "120,80,900,700").split(separator: ",").map { Double($0)! }   // points, top-left origin
let duration = Double(opt("duration", "3"))!
let envPairs = opt("env", "").split(separator: ";").map(String.init).filter { !$0.isEmpty }
var tb = mach_timebase_info_data_t(); mach_timebase_info(&tb)
func machToSec(_ t: UInt64) -> Double { Double(t) * Double(tb.numer) / Double(tb.denom) / 1e9 }
func nowMach() -> Double { machToSec(mach_absolute_time()) }

struct Frame { var t: Double; var profile: [Float] }
final class Cap: NSObject, SCStreamOutput {
    var frames: [Frame] = []
    let lock = NSLock()
    let bandLeft: Double, bandRight: Double, top: Int
    init(bandLeft: Double, bandRight: Double, top: Int) { self.bandLeft = bandLeft; self.bandRight = bandRight; self.top = top }
    func stream(_ s: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, let atts = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let a = atts.first, (a[.status] as? Int) == SCFrameStatus.complete.rawValue, let px = CMSampleBufferGetImageBuffer(sb) else { return }
        let t = (a[.displayTime] as? UInt64).map(machToSec) ?? nowMach()
        CVPixelBufferLockBaseAddress(px, .readOnly); defer { CVPixelBufferUnlockBaseAddress(px, .readOnly) }
        let w = CVPixelBufferGetWidth(px), h = CVPixelBufferGetHeight(px), bpr = CVPixelBufferGetBytesPerRow(px)
        let base = CVPixelBufferGetBaseAddress(px)!.assumingMemoryBound(to: UInt8.self)
        let x0 = Int(Double(w) * bandLeft), x1 = Int(Double(w) * bandRight)
        var profile = [Float](repeating: 0, count: max(0, h - top))
        for y in top..<h {
            let row = base + y * bpr
            var sum = 0
            var x = x0
            while x < x1 { let p = row + x * 4; sum += (Int(p[2]) * 3 + Int(p[1]) * 6 + Int(p[0])) / 10; x += 3 }
            profile[y - top] = Float(sum) / Float(max(1, (x1 - x0) / 3))
        }
        lock.lock(); frames.append(Frame(t: t, profile: profile)); lock.unlock()
    }
}

let sem = DispatchSemaphore(value: 0)
var content: SCShareableContent?
Task { content = try? await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false); sem.signal() }
sem.wait()
guard let content, let display = content.displays.first(where: { $0.displayID == CGMainDisplayID() }) ?? content.displays.first else { print("{\"error\":\"no display\"}"); exit(1) }
let filter = SCContentFilter(display: display, excludingWindows: content.windows)
let cfg = SCStreamConfiguration()
cfg.sourceRect = CGRect(x: region[0], y: region[1], width: region[2], height: region[3])
cfg.width = Int(region[2]); cfg.height = Int(region[3])
cfg.minimumFrameInterval = CMTime(value: 1, timescale: 120)
cfg.queueDepth = 8
cfg.showsCursor = false
cfg.pixelFormat = kCVPixelFormatType_32BGRA
let cap = Cap(bandLeft: Double(opt("bandleft", "0.30"))!, bandRight: Double(opt("bandright", "0.95"))!, top: Int(opt("chrome", "90"))!)
let stream = SCStream(filter: filter, configuration: cfg, delegate: nil)
try! stream.addStreamOutput(cap, type: .screen, sampleHandlerQueue: DispatchQueue(label: "cap", qos: .userInteractive))
Task { try? await stream.startCapture(); sem.signal() }
sem.wait()
Thread.sleep(forTimeInterval: 0.4)   // baseline frames of whatever is behind

let proc = Process()
proc.executableURL = URL(fileURLWithPath: execPath)
proc.arguments = appArgs
var env = ProcessInfo.processInfo.environment
for p in envPairs { let kv = p.split(separator: "=", maxSplits: 1).map(String.init); if kv.count == 2 { env[kv[0]] = kv[1] } }
proc.environment = env
proc.standardOutput = FileHandle(forWritingAtPath: "/dev/null")
proc.standardError = FileHandle(forWritingAtPath: "/dev/null")
let launch = nowMach()
try! proc.run()
Thread.sleep(forTimeInterval: duration)
Task { try? await stream.stopCapture(); sem.signal() }
sem.wait()
kill(proc.processIdentifier, SIGTERM); usleep(300_000); if proc.isRunning { kill(proc.processIdentifier, SIGKILL) }

cap.lock.lock(); let frames = cap.frames; cap.lock.unlock()
func diff(_ a: [Float], _ b: [Float]) -> Float {
    let n = min(a.count, b.count); if n == 0 { return 0 }
    var s: Float = 0; for i in 0..<n { s += abs(a[i] - b[i]) }; return s / Float(n)
}
let before = frames.filter { $0.t < launch }
let after = frames.filter { $0.t >= launch }
var result: [String: Any] = ["exec": execPath, "frames_before": before.count, "frames_after": after.count]
if let bg = before.last, let final = after.last {
    let firstChange = after.first { diff($0.profile, bg.profile) > 2 }
    let tol = Float(opt("tol", "1.5"))!
    let firstContent = after.first { diff($0.profile, final.profile) < tol && diff($0.profile, bg.profile) > 2 }
    result["first_window_ms"] = firstChange.map { ($0.t - launch) * 1000 } ?? -1
    result["first_content_ms"] = firstContent.map { ($0.t - launch) * 1000 } ?? -1
    result["final_vs_background"] = diff(final.profile, bg.profile)
} else { result["error"] = "no frames" }
let data = try! JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
if outPath == "/dev/stdout" { print(String(data: data, encoding: .utf8)!) } else { try! data.write(to: URL(fileURLWithPath: outPath)) }

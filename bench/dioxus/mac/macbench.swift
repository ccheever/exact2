// macbench: the same outside-the-app measurement for any macOS app window.
//
//   macbench shot <pid> <out.png>                 window screenshot (ScreenCaptureKit)
//   macbench place <pid> <x> <y> <w> <h>          AX: move/resize the app's main window
//   macbench cold <out.json> <exe> [K=V ...]      spawn, first display frame with text
//                                                 ink in the window below its top 140 pt
//   macbench scroll <pid> <out.json> <pt/s,...> [seconds]
//                                                 per speed: wheel events (pixel units)
//                                                 posted to the pid at 120 Hz with the
//                                                 cursor untouched; the window's frames
//                                                 from an SCStream at 120 fps (a frame
//                                                 marked complete = new content), white
//                                                 bands >= 150 pt in the content column,
//                                                 the process's CPU (task info) and
//                                                 phys_footprint
//   macbench rest <pid> <out.json> <seconds>      the same without input (live mode)
import AppKit
import ApplicationServices
import CoreMedia
import Foundation
import ScreenCaptureKit

setvbuf(stdout, nil, _IONBF, 0)
let nsapp = NSApplication.shared
nsapp.setActivationPolicy(.accessory)
let args = CommandLine.arguments
func die(_ s: String) -> Never { FileHandle.standardError.write((s + "\n").data(using: .utf8)!); exit(1) }

func machNs() -> UInt64 { clock_gettime_nsec_np(CLOCK_UPTIME_RAW) }
var tb = mach_timebase_info_data_t(); mach_timebase_info(&tb)
func machToNs(_ t: UInt64) -> UInt64 { t * UInt64(tb.numer) / UInt64(tb.denom) }

func cpuNs(_ pid: pid_t) -> UInt64 {
  var info = proc_taskinfo()
  let n = proc_pidinfo(pid, PROC_PIDTASKINFO, 0, &info, Int32(MemoryLayout<proc_taskinfo>.size))
  if n <= 0 { return 0 }
  return machToNs(info.pti_total_user + info.pti_total_system)
}
/// Another user's process (WindowServer): `ps` reports its CPU time.
func psCpuMs(_ pid: pid_t) -> Double {
  let p = Process(); p.executableURL = URL(fileURLWithPath: "/bin/ps"); p.arguments = ["-o", "time=", "-p", String(pid)]
  let pipe = Pipe(); p.standardOutput = pipe; try? p.run(); p.waitUntilExit()
  let t = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)!.trimmingCharacters(in: .whitespacesAndNewlines)
  let parts = t.split(separator: ":").map { Double($0) ?? 0 }
  return parts.count == 3 ? (parts[0] * 3600 + parts[1] * 60 + parts[2]) * 1000 : parts.count == 2 ? (parts[0] * 60 + parts[1]) * 1000 : 0
}
let windowServer: pid_t = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.WindowServer").first?.processIdentifier ?? {
  let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep"); p.arguments = ["-x", "WindowServer"]
  let pipe = Pipe(); p.standardOutput = pipe; try? p.run(); p.waitUntilExit()
  return pid_t(String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)!.trimmingCharacters(in: .whitespacesAndNewlines).split(separator: "\n").first ?? "0") ?? 0
}()
func footprintMB(_ pid: pid_t) -> (Double, Double) {
  var ri = rusage_info_v4()
  let r = withUnsafeMutablePointer(to: &ri) { p in p.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { proc_pid_rusage(pid, RUSAGE_INFO_V4, $0) } }
  if r != 0 { return (0, 0) }
  return (Double(ri.ri_phys_footprint) / 1048576, Double(ri.ri_lifetime_max_phys_footprint) / 1048576)
}

func axWindow(_ pid: pid_t) -> AXUIElement? {
  let app = AXUIElementCreateApplication(pid)
  var value: CFTypeRef?
  for attr in [kAXMainWindowAttribute, kAXFocusedWindowAttribute] {
    if AXUIElementCopyAttributeValue(app, attr as CFString, &value) == .success, let v = value { return (v as! AXUIElement) }
  }
  if AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value) == .success, let arr = value as? [AXUIElement], let w = arr.first { return w }
  return nil
}
func place(_ pid: pid_t, _ x: Double, _ y: Double, _ w: Double, _ h: Double) {
  guard let win = axWindow(pid) else { die("no AX window for \(pid)") }
  var p = CGPoint(x: x, y: y), s = CGSize(width: w, height: h)
  AXUIElementSetAttributeValue(win, kAXPositionAttribute as CFString, AXValueCreate(.cgPoint, &p)!)
  AXUIElementSetAttributeValue(win, kAXSizeAttribute as CFString, AXValueCreate(.cgSize, &s)!)
  NSRunningApplication(processIdentifier: pid)?.activate()
}

func scWindow(_ pid: pid_t) async -> SCWindow? {
  for _ in 0..<400 {
    if let content = try? await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true),
       let w = content.windows.filter({ $0.owningApplication?.processID == pid && $0.windowLayer == 0 && $0.frame.width > 100 }).max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height }) { return w }
    try? await Task.sleep(nanoseconds: 5_000_000)
  }
  return nil
}

/// Rows of the frame (in points from the window top) that are pure white
/// across the content column, merged into bands; returns the tallest band.
func tallestWhiteBand(_ px: CVPixelBuffer, scale: Double, from topPt: Double) -> Double {
  CVPixelBufferLockBaseAddress(px, .readOnly); defer { CVPixelBufferUnlockBaseAddress(px, .readOnly) }
  let w = CVPixelBufferGetWidth(px), h = CVPixelBufferGetHeight(px), bpr = CVPixelBufferGetBytesPerRow(px)
  let base = CVPixelBufferGetBaseAddress(px)!.assumingMemoryBound(to: UInt8.self)
  let x0 = Int(70 * scale), x1 = w - Int(18 * scale)
  var best = 0, run = 0
  for y in Int(topPt * scale)..<(h - Int(4 * scale)) {
    let row = base + y * bpr
    var white = true
    var x = x0
    while x < x1 { let p = row + x * 4; if p[0] < 250 || p[1] < 250 || p[2] < 250 { white = false; break }; x += 3 }
    if white { run += 1; best = max(best, run) } else { run = 0 }
  }
  return Double(best) / scale
}
/// Dark (text) pixels below `topPt` in the window.
func inkCount(_ px: CVPixelBuffer, scale: Double, from topPt: Double) -> Int {
  CVPixelBufferLockBaseAddress(px, .readOnly); defer { CVPixelBufferUnlockBaseAddress(px, .readOnly) }
  let w = CVPixelBufferGetWidth(px), h = CVPixelBufferGetHeight(px), bpr = CVPixelBufferGetBytesPerRow(px)
  let base = CVPixelBufferGetBaseAddress(px)!.assumingMemoryBound(to: UInt8.self)
  var n = 0
  var y = Int(topPt * scale)
  while y < h { let row = base + y * bpr; var x = 0; while x < w { let p = row + x * 4; if Int(p[0]) + Int(p[1]) + Int(p[2]) < 200 { n += 1 }; x += 2 }; y += 2 }
  return n
}

final class Recorder: NSObject, SCStreamOutput {
  var frames: [(ns: UInt64, band: Double)] = []
  var recording = false
  var measureBands = true
  var scale = 1.0
  var onFrame: ((CVPixelBuffer, UInt64) -> Void)?
  let lock = NSLock()
  func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
    guard type == .screen,
          let att = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
          let raw = att.first?[.status] as? Int, SCFrameStatus(rawValue: raw) == .complete,
          let px = CMSampleBufferGetImageBuffer(sb) else { return }
    let disp = (att.first?[.displayTime] as? UInt64).map(machToNs) ?? machNs()
    if let f = onFrame { f(px, disp) }
    lock.lock(); let rec = recording; lock.unlock()
    if rec { let band = measureBands ? tallestWhiteBand(px, scale: scale, from: 130) : 0; lock.lock(); frames.append((disp, band)); lock.unlock() }
  }
}

func startStream(_ filter: SCContentFilter, width: Int, height: Int, rec: Recorder, fps: Int = 120) async throws -> SCStream {
  let cfg = SCStreamConfiguration()
  cfg.width = width; cfg.height = height
  cfg.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(fps))
  cfg.queueDepth = 8
  cfg.showsCursor = false
  cfg.pixelFormat = kCVPixelFormatType_32BGRA
  let s = SCStream(filter: filter, configuration: cfg, delegate: nil)
  try s.addStreamOutput(rec, type: .screen, sampleHandlerQueue: DispatchQueue(label: "frames", qos: .userInteractive))
  try await s.startCapture()
  return s
}

/// Wheel events at 120 Hz, pixel units, posted to `pid` at the window's centre.
func driveScroll(pid: pid_t, at point: CGPoint, ptPerS: Double, seconds: Double, hid: Bool) {
  let src = CGEventSource(stateID: .privateState)
  let period: UInt64 = 1_000_000_000 / 120
  let start = machNs()
  var k: UInt64 = 0
  var carry = 0.0
  while true {
    let target = start + k * period
    while machNs() < target { usleep(200) }
    if Double(machNs() - start) / 1e9 >= seconds { break }
    carry += ptPerS / 120
    let d = Int32(carry.rounded(.towardZero)); carry -= Double(d)
    if let e = CGEvent(scrollWheelEvent2Source: src, units: .pixel, wheelCount: 1, wheel1: -d, wheel2: 0, wheel3: 0) {
      e.location = point
      if hid { e.post(tap: .cghidEventTap) } else { e.postToPid(pid) }
    }
    k += 1
  }
}

func summarize(_ frames: [(ns: UInt64, band: Double)], seconds: Double, refreshMs: Double = 1000.0 / 120) -> [String: Any] {
  let iv = zip(frames.dropFirst(), frames).map { Double($0.ns - $1.ns) / 1e6 }
  let sorted = iv.sorted()
  return ["frames": frames.count, "fps": Double(frames.count) / seconds,
          "late": iv.filter { $0 > refreshMs * 1.5 }.count,
          "medianMs": sorted.isEmpty ? 0 : sorted[sorted.count / 2], "p99Ms": sorted.isEmpty ? 0 : sorted[min(sorted.count - 1, Int(Double(sorted.count) * 0.99))],
          "worstMs": sorted.last ?? 0,
          "blankFrames": frames.filter { $0.band >= 150 }.count, "tallestBandPt": frames.map(\.band).max() ?? 0]
}

func writeJSON(_ obj: Any, _ path: String) {
  let d = try! JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys])
  FileManager.default.createFile(atPath: path, contents: d)
}

func measure(pid: pid_t, out: String, speeds: [Double], seconds: Double, rest: Bool) async {
  guard let win = await scWindow(pid) else { die("no window") }
  let scale = 2.0
  let rec = Recorder(); rec.scale = scale
  let filter = SCContentFilter(desktopIndependentWindow: win)
  let stream = try! await startStream(filter, width: Int(win.frame.width * scale), height: Int(win.frame.height * scale), rec: rec)
  try? await Task.sleep(nanoseconds: 500_000_000)
  let centre = CGPoint(x: win.frame.midX, y: win.frame.midY + 60)
  await MainActor.run { _ = NSRunningApplication(processIdentifier: pid)?.activate(options: [.activateAllWindows]) }
  try? await Task.sleep(nanoseconds: 300_000_000)
  let viaHID = ProcessInfo.processInfo.environment["MACBENCH_HID"] == "1"
  if viaHID { CGWarpMouseCursorPosition(centre) }
  var results: [[String: Any]] = []
  for speed in (rest ? [0] : speeds) {
    let m0 = footprintMB(pid)
    rec.lock.lock(); rec.frames = []; rec.recording = true; rec.lock.unlock()
    let c0 = cpuNs(pid), w0 = psCpuMs(windowServer), t0 = machNs()
    if rest { try? await Task.sleep(nanoseconds: UInt64(seconds * 1e9)) }
    else { await Task.detached(priority: .userInitiated) { driveScroll(pid: pid, at: centre, ptPerS: speed, seconds: seconds, hid: viaHID) }.value }
    let wall = Double(machNs() - t0) / 1e9, c1 = cpuNs(pid), w1 = psCpuMs(windowServer)
    rec.lock.lock(); rec.recording = false; let frames = rec.frames; rec.lock.unlock()
    var r = summarize(frames, seconds: wall)
    let m1 = footprintMB(pid)
    r["speed"] = speed; r["wallS"] = wall; r["cpuMsPerS"] = Double(c1 - c0) / 1e6 / wall; r["windowServerMsPerS"] = (w1 - w0) / wall
    r["footprintMB"] = m1.0; r["footprintStartMB"] = m0.0; r["lifetimePeakMB"] = m1.1
    results.append(r)
    print(String(format: "%6.0f pt/s  %5.1f fps  late %3d  worst %6.1f ms  blank %3d  cpu %6.1f ms/s  ws %6.1f ms/s  mem %6.1f MB", speed, r["fps"] as! Double, r["late"] as! Int, r["worstMs"] as! Double, r["blankFrames"] as! Int, r["cpuMsPerS"] as! Double, r["windowServerMsPerS"] as! Double, m1.0))
    try? await Task.sleep(nanoseconds: 1_200_000_000)
  }
  try? await stream.stopCapture()
  writeJSON(["pid": pid, "window": ["w": win.frame.width, "h": win.frame.height], "runs": results], out)
}

func cold(out: String, exe: String, env: [String]) async {
  // The display stream runs before the spawn; once the window exists, each
  // frame's window rect is checked for text ink below its top 140 pt.
  let content = try! await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
  let display = content.displays.first!
  let rec = Recorder(); rec.measureBands = false
  let scale = 1.0
  let filter = SCContentFilter(display: display, excludingWindows: [])
  var winRect: CGRect? = nil
  var firstInk: UInt64? = nil
  var firstWindowFrame: UInt64? = nil
  let lock = NSLock()
  rec.onFrame = { px, ns in
    lock.lock(); let r = winRect; let done = firstInk != nil; lock.unlock()
    guard let r, !done else { return }
    lock.lock(); if firstWindowFrame == nil { firstWindowFrame = ns }; lock.unlock()
    // Crop: count ink inside the window rect (display points; scale 1).
    CVPixelBufferLockBaseAddress(px, .readOnly); defer { CVPixelBufferUnlockBaseAddress(px, .readOnly) }
    let bpr = CVPixelBufferGetBytesPerRow(px), base = CVPixelBufferGetBaseAddress(px)!.assumingMemoryBound(to: UInt8.self)
    let W = CVPixelBufferGetWidth(px), H = CVPixelBufferGetHeight(px)
    var n = 0
    var y = Int(r.minY + 140); while y < min(H, Int(r.maxY) - 4) { let row = base + y * bpr; var x = max(0, Int(r.minX) + 60); while x < min(W, Int(r.maxX) - 10) { let p = row + x * 4; if Int(p[0]) + Int(p[1]) + Int(p[2]) < 200 { n += 1 }; x += 2 }; y += 2 }
    if n > 400 { lock.lock(); firstInk = ns; lock.unlock() }
  }
  let stream = try! await startStream(filter, width: Int(display.frame.width * scale), height: Int(display.frame.height * scale), rec: rec, fps: 120)
  try? await Task.sleep(nanoseconds: 600_000_000)
  let p = Process()
  p.executableURL = URL(fileURLWithPath: exe)
  var e = ProcessInfo.processInfo.environment
  for kv in env { let parts = kv.split(separator: "=", maxSplits: 1).map(String.init); e[parts[0]] = parts[1] }
  p.environment = e
  p.standardOutput = FileHandle.nullDevice; p.standardError = FileHandle.nullDevice
  let t0 = machNs()
  try! p.run()
  let pid = p.processIdentifier
  var windowNs: UInt64? = nil
  // Poll the window list for the app's window and its rect.
  while machNs() - t0 < 15_000_000_000 {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
    if let w = list.first(where: { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }),
       let b = w[kCGWindowBounds as String] as? [String: Double] {
      if windowNs == nil { windowNs = machNs() }
      lock.lock(); winRect = CGRect(x: b["X"]!, y: b["Y"]!, width: b["Width"]!, height: b["Height"]!); lock.unlock()
    }
    lock.lock(); let done = firstInk != nil; lock.unlock()
    if done { break }
    usleep(1000)
  }
  try? await Task.sleep(nanoseconds: 1_500_000_000)
  let mem = footprintMB(pid)
  try? await stream.stopCapture()
  lock.lock(); let ink = firstInk; lock.unlock()
  let res: [String: Any] = ["exe": exe, "windowMs": windowNs.map { Double($0 - t0) / 1e6 } ?? -1, "firstInkMs": ink.map { Double(Int64($0) - Int64(t0)) / 1e6 } ?? -1, "footprintMB": mem.0, "cpuMs": Double(cpuNs(pid)) / 1e6]
  print(res)
  p.terminate(); p.waitUntilExit()
  writeJSON(res, out)
}

func shot(_ pid: pid_t, _ out: String) async {
  guard let win = await scWindow(pid) else { die("no window") }
  let cfg = SCStreamConfiguration(); cfg.width = Int(win.frame.width * 2); cfg.height = Int(win.frame.height * 2); cfg.showsCursor = false
  let img = try! await SCScreenshotManager.captureImage(contentFilter: SCContentFilter(desktopIndependentWindow: win), configuration: cfg)
  let rep = NSBitmapImageRep(cgImage: img)
  try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
  print("shot \(win.frame)")
}

let sema = DispatchSemaphore(value: 0)
Task {
  switch args.count > 1 ? args[1] : "" {
  case "shot": await shot(pid_t(args[2])!, args[3])
  case "place": place(pid_t(args[2])!, Double(args[3])!, Double(args[4])!, Double(args[5])!, Double(args[6])!)
  case "scroll": await measure(pid: pid_t(args[2])!, out: args[3], speeds: args[4].split(separator: ",").map { Double($0)! }, seconds: args.count > 5 ? Double(args[5])! : 3, rest: false)
  case "rest": await measure(pid: pid_t(args[2])!, out: args[3], speeds: [], seconds: Double(args[4])!, rest: true)
  case "cold": await cold(out: args[2], exe: args[3], env: Array(args.dropFirst(4)))
  default: die("usage: macbench shot|place|scroll|rest|cold …")
  }
  exit(0)
}
dispatchMain()

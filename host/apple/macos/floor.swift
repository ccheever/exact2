// The platform floor: an empty AppKit app — NSApplication, one NSScrollView,
// one titled window, one view that draws — with the same stamps ExactMac
// prints, so every number the presenter reports can be read against what
// the OS costs with nothing in the window (LLP 1008 §6). Built and run by
// scripts/metrics.mjs: `swiftc -O -o <out> host/apple/macos/floor.swift`.
import AppKit
let t0 = CACurrentMediaTime()
func now() -> Double { (CACurrentMediaTime() - t0) * 1000 }
var stamps: [(String, Double)] = []
func stamp(_ l: String) { stamps.append((l, now())) }
let app = NSApplication.shared
stamp("NSApplication.shared")
app.setActivationPolicy(.regular)
let sv = NSScrollView(frame: .zero)
stamp("NSScrollView")
let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 420, height: 860), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
stamp("NSWindow")
let v = NSView(frame: NSRect(x: 0, y: 0, width: 420, height: 860))
v.wantsLayer = true
window.contentView = v
window.center()
stamp("contentView")
final class D: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ n: Notification) { stamp("didFinishLaunching") }
}
let d = D()
app.delegate = d
final class Drawn: NSView {
    var done = false
    override func draw(_ r: NSRect) { if !done { done = true; stamp("first draw") } }
}
let drawn = Drawn(frame: NSRect(x: 10, y: 10, width: 100, height: 100))
v.addSubview(drawn)
window.makeKeyAndOrderFront(nil)
stamp("makeKeyAndOrderFront")
app.activate(ignoringOtherApps: true)
stamp("activate")
DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
    print("floor: " + stamps.map { "\($0.0) \(String(format: "%.1f", $0.1))" }.joined(separator: " · "))
    exit(0)
}
app.run()

import AppKit

// Logs every mouse event the view gets, and every one an app-level local monitor sees,
// to NSLog (the drive's logs.host) and to an on-screen log view.
func line(_ who: String, _ e: NSEvent, in v: NSView?) -> String {
    let p = v.map { $0.convert(e.locationInWindow, from: nil) } ?? e.locationInWindow
    let mods = [(NSEvent.ModifierFlags.shift, "Shift"), (.control, "Control"), (.option, "Alt"), (.command, "Meta")]
        .filter { e.modifierFlags.contains($0.0) }.map(\.1).joined(separator: "+")
    let clicks = [.scrollWheel, .mouseMoved].contains(e.type) ? "-" : "\(e.clickCount)"
    let wheel = e.type == .scrollWheel ? " d=(\(Int(e.scrollingDeltaX)),\(Int(e.scrollingDeltaY)))" : ""
    return "probe \(who) type=\(e.type.rawValue) button=\(e.buttonNumber) clicks=\(clicks) mods=\(mods.isEmpty ? "none" : mods) at=(\(Int(p.x)),\(Int(p.y)))\(wheel)"
}

enum Shown {
    nonisolated(unsafe) static var lines: [String] = []
    nonisolated(unsafe) static var views: [LogView] = []
    static func add(_ s: String) {
        NSLog("%@", s)
        lines.append(s); if lines.count > 26 { lines.removeFirst(lines.count - 26) }
        for v in views { v.needsDisplay = true }
    }
}

final class MouseModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["mouse-probe": ExactNativeFactory { _, events in Probe(events: events) },
         "mouse-log": ExactNativeFactory { _, events in LogBox(events: events) }]
    }
    var monitor: Any?
    required init(context: ExactModuleContext) {
        super.init(context: context)
        let mask: NSEvent.EventTypeMask = [.leftMouseDown, .leftMouseUp, .leftMouseDragged, .rightMouseDown, .rightMouseUp, .otherMouseDown, .otherMouseUp, .scrollWheel]
        monitor = NSEvent.addLocalMonitorForEvents(matching: mask) { e in Shown.add(line("monitor", e, in: nil)); return e }
    }
}

final class View: NSView {
    override var isFlipped: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with e: NSEvent) { Shown.add(line("view", e, in: self)) }
    override func mouseDragged(with e: NSEvent) { Shown.add(line("view", e, in: self)) }
    override func mouseUp(with e: NSEvent) { Shown.add(line("view", e, in: self)) }
    override func rightMouseDown(with e: NSEvent) { Shown.add(line("view", e, in: self)) }
    override func rightMouseUp(with e: NSEvent) { Shown.add(line("view", e, in: self)) }
    override func otherMouseDown(with e: NSEvent) { Shown.add(line("view", e, in: self)) }
    override func otherMouseUp(with e: NSEvent) { Shown.add(line("view", e, in: self)) }
    override func scrollWheel(with e: NSEvent) { Shown.add(line("view", e, in: self)) }
    override func draw(_ r: NSRect) {
        NSColor(calibratedRed: 0.85, green: 0.9, blue: 1, alpha: 1).setFill(); bounds.fill()
        NSColor.systemBlue.setStroke(); NSBezierPath(rect: bounds.insetBy(dx: 0.5, dy: 0.5)).stroke()
        ("probe 200×120" as NSString).draw(at: NSPoint(x: 8, y: 8), withAttributes: [.font: NSFont.systemFont(ofSize: 12), .foregroundColor: NSColor.black])
    }
}

final class LogView: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ r: NSRect) {
        NSColor(white: 0.97, alpha: 1).setFill(); bounds.fill()
        NSColor.gray.setStroke(); NSBezierPath(rect: bounds.insetBy(dx: 0.5, dy: 0.5)).stroke()
        let a: [NSAttributedString.Key: Any] = [.font: NSFont.monospacedSystemFont(ofSize: 10, weight: .regular), .foregroundColor: NSColor.black]
        for (i, s) in Shown.lines.enumerated() { (s as NSString).draw(at: NSPoint(x: 6, y: 4 + CGFloat(i) * 15.5), withAttributes: a) }
    }
}

final class Probe: ExactNativeInstance {
    private let v = View(frame: .zero)
    override var view: ExactNativeView { v }
}
final class LogBox: ExactNativeInstance {
    private let v = LogView(frame: .zero)
    override init(events: ExactNativeEvents) { super.init(events: events); Shown.views.append(v) }
    override var view: ExactNativeView { v }
}

let exactModule: ExactModule.Type = MouseModule.self

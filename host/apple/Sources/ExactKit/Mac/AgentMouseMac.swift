// The agent's mouse buttons and wheel on AppKit (LLP 1012 §1; #107). Each
// event goes through `NSApplication.sendEvent`, as a hand's comes from the
// window server, so the app's local monitors see it before its window does.
// A right or middle button and a wheel are made as the window server makes
// them — a `CGEvent` placed in the window — so `buttonNumber` is the
// button's own (1 right, 2 middle; `NSEvent.mouseEvent` leaves both 0) and
// `locationInWindow` is the point the driver named (a window-less wheel
// read it in screen space).
#if os(macOS)
import AppKit

extension Agent {
    /// The modifiers a request names (`"Shift+Meta"`), as AppKit's flags;
    /// none when it names none, nil when a word is no modifier.
    static func heldModifiers(_ request: [String: Any]) -> NSEvent.ModifierFlags? {
        var flags: NSEvent.ModifierFlags = []
        for name in (request["modifiers"] as? String ?? "").split(separator: "+") {
            guard let flag = ["Shift": NSEvent.ModifierFlags.shift, "Control": .control, "Alt": .option, "Meta": .command][String(name)] else { return nil }
            flags.insert(flag)
        }
        return flags
    }

    /// The event's window number: an undocumented field, as the magnify
    /// fields `tap … pinch` sets are.
    private static let windowField = CGEventField(rawValue: 51)!
    /// Where in its window an event happened, top-left origin, which
    /// `NSEvent(cgEvent:)` reads for `locationInWindow`: CoreGraphics' own
    /// setter, exported but not in its headers. Without it an event made here
    /// has no place in the window, and the request is refused.
    private static let placeInWindow: (@convention(c) (CGEvent, CGPoint) -> Void)? =
        dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGEventSetWindowLocation")
            .map { unsafeBitCast($0, to: (@convention(c) (CGEvent, CGPoint) -> Void).self) }

    /// `cg` at window point `p` of `win` (bottom-left origin, as
    /// `locationInWindow`), holding `flags`, as an `NSEvent` of that window.
    static func windowEvent(_ cg: CGEvent, at p: NSPoint, in win: NSWindow, flags: NSEvent.ModifierFlags) -> NSEvent? {
        guard let placeInWindow else { return nil }
        let screen = win.convertPoint(toScreen: p)
        cg.location = CGPoint(x: screen.x, y: (NSScreen.screens.first?.frame.height ?? 0) - screen.y)
        cg.setIntegerValueField(windowField, value: Int64(win.windowNumber))
        placeInWindow(cg, CGPoint(x: p.x, y: win.frame.height - p.y))
        cg.flags = CGEventFlags(rawValue: UInt64(flags.rawValue))
        return NSEvent(cgEvent: cg)
    }

    /// One press and release of the right or middle button at `p`, the
    /// `clicks`th of a run, through the application.
    static func otherClick(_ button: CGMouseButton, at p: NSPoint, in win: NSWindow, clicks: Int, flags: NSEvent.ModifierFlags) -> Bool {
        let (down, up): (CGEventType, CGEventType) = button == .right ? (.rightMouseDown, .rightMouseUp) : (.otherMouseDown, .otherMouseUp)
        var events: [NSEvent] = []
        for (type, pressure) in [(down, 1.0), (up, 0.0)] {
            guard let cg = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: .zero, mouseButton: button) else { return false }
            cg.setIntegerValueField(.mouseEventClickState, value: Int64(clicks))
            cg.setDoubleValueField(.mouseEventPressure, value: pressure)
            guard let e = windowEvent(cg, at: p, in: win, flags: flags) else { return false }
            events.append(e)
        }
        for e in events { NSApp.sendEvent(e) }
        return true
    }

    /// A wheel's steps at `p`, through the application: the window hit-tests
    /// the point and the responder chain carries it up, as a trackpad's.
    /// CGScrollPhase: began 1, changed 2, ended 4; 0 is a phase-less wheel.
    static func wheel(_ steps: [(phase: Int64, dx: Double, dy: Double)], gesture: Bool, at p: NSPoint, in win: NSWindow, flags: NSEvent.ModifierFlags) -> Bool {
        let whole = { (d: Double) -> Int32 in Int32(min(max(d.rounded(), -1_000_000), 1_000_000)) }
        var events: [NSEvent] = []
        for (phase, dx, dy) in steps {
            guard let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: -whole(dy), wheel2: -whole(dx), wheel3: 0) else { return false }
            if gesture {
                // A trackpad's deltas are continuous; without this the
                // event reads as a wheel's notches and the phase is moot.
                cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
                cg.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
            }
            guard let e = windowEvent(cg, at: p, in: win, flags: flags) else { return false }
            events.append(e)
        }
        for e in events { NSApp.sendEvent(e) }
        return true
    }
}
#endif

// The desktop pointer for the iOS Simulator (LLP 1035.003 §3, candidate 1;
// approved as apparatus by Charlie, 2026-09-10). UIKit offers no public
// touch synthesis, so a held contact on a simulator is a real mouse on the
// Mac's desktop, posted into the simulator's window — the path the Messages
// record's hand-run scripts took, now the driver's, with the window-to-device
// mapping in one place (`scripts/agent.mjs`, `openIOS`'s `phaseSim`).
//
// The simulator's window is Simulator.app's, or, under Xcode 27, which ships
// no Simulator.app, the window Device Hub opens for one device
// (`showSimulator` in `host/apple/build.mjs` asks for it and raises it).
//
// Built by the driver with `swiftc` on first use. Speaks JSON lines on stdio:
//   {"op":"trusted"}                    {"trusted":bool,"locked":bool} — Accessibility is granted to
//                                       this process; the Mac's screen is locked
//   {"op":"window","title":"iPhone 17"} {"x","y","w","h","title","named","windows":[…]}: the simulator
//                                       windows on screen, in global display coordinates, top-left
//                                       origin — those whose title contains it first, front to
//                                       back, then the rest, largest first (window titles are
//                                       unreadable without Screen Recording permission, and Device
//                                       Hub's own window carries a device's name too); the first is
//                                       repeated at the top level. The driver takes the first one
//                                       its app sees a hover in.
//   {"op":"hover","x":X,"y":Y}          posts a mouseMoved there (the driver's calibration: the app
//                                       reports where its viewport saw the pointer)
//   {"op":"down"|"move"|"up","x":X,"y":Y}   posts leftMouseDown / leftMouseDragged / leftMouseUp there
// Nothing here knows the device: the driver maps viewport points to these
// coordinates and owns the contact's state.
import AppKit
import CoreGraphics

func reply(_ object: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: object)
    print(String(decoding: data, as: UTF8.self))
    fflush(stdout)
}

/// The apps whose windows show a simulator's screen. By bundle identifier:
/// a window's owner name is the app's localized name.
let simulatorApps: Set<String> = ["com.apple.iphonesimulator", "com.apple.dt.Devices"]
var bundles: [pid_t: String] = [:]

func showsSimulators(_ window: [String: Any]) -> Bool {
    guard let pid = window[kCGWindowOwnerPID as String] as? pid_t else { return false }
    if bundles[pid] == nil { bundles[pid] = NSRunningApplication(processIdentifier: pid)?.bundleIdentifier ?? "" }
    return simulatorApps.contains(bundles[pid] ?? "")
}

func ordinaryWindows() -> [[String: Any]] {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else { return [] }
    return list.filter { (($0[kCGWindowLayer as String] as? Int) ?? 0) == 0 }
}

func bounds(_ window: [String: Any]) -> CGRect? {
    guard let b = window[kCGWindowBounds as String] as? [String: Any] else { return nil }
    return CGRect(dictionaryRepresentation: b as CFDictionary)
}

func area(_ window: [String: Any]) -> CGFloat {
    bounds(window).map { $0.width * $0.height } ?? 0
}

func name(_ window: [String: Any]) -> String {
    (window[kCGWindowName as String] as? String) ?? ""
}

/// The topmost ordinary window under a desktop point — the window a posted
/// event would land in. Nothing is posted unless it is a simulator's: an
/// operator's other window over it must never receive a synthesized click
/// (the record's misses, 2026-09-10).
func top(under point: CGPoint) -> [String: Any]? {
    ordinaryWindows().first { bounds($0)?.contains(point) ?? false }
}

/// Whether the Mac's screen is locked: no window comes to the front then,
/// and a posted event reaches no app's.
func screenLocked() -> Bool {
    ((CGSessionCopyCurrentDictionary() as? [String: Any])?["CGSSessionScreenIsLocked"] as? Bool) ?? false
}

while let line = readLine() {
    guard let data = line.data(using: .utf8),
          let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let op = req["op"] as? String else { reply(["error": "unreadable request"]); continue }
    switch op {
    case "trusted":
        reply(["trusted": AXIsProcessTrusted(), "locked": screenLocked()])
    case "window":
        let title = req["title"] as? String ?? ""
        let windows = ordinaryWindows().filter(showsSimulators)
        let named = windows.filter { !title.isEmpty && name($0).contains(title) }
        let rest = windows.filter { title.isEmpty || !name($0).contains(title) }.sorted { area($0) > area($1) }
        let found: [[String: Any]] = (named.map { ($0, true) } + rest.map { ($0, false) }).compactMap { window, named in
            guard let r = bounds(window) else { return nil }
            return ["x": r.origin.x, "y": r.origin.y, "w": r.width, "h": r.height, "title": name(window), "named": named]
        }
        guard var first = found.first else {
            reply(["error": "no simulator window on screen (Simulator's, or Device Hub's for the device)"])
            continue
        }
        first["windows"] = found
        reply(first)
    case "hover", "down", "move", "up":
        guard let x = req["x"] as? Double, let y = req["y"] as? Double, x.isFinite, y.isFinite else {
            reply(["error": "\(op) needs finite x and y"])
            continue
        }
        let point = CGPoint(x: x, y: y)
        let under = top(under: point)
        // A release is posted wherever it lands: a held button must never
        // be left down. Anything else lands only in a simulator's window.
        if op != "up", !(under.map(showsSimulators) ?? false) {
            let by = (under?[kCGWindowOwnerName as String] as? String) ?? "nothing"
            reply(["error": "the simulator's window is not the topmost at \(Int(x)),\(Int(y)): \(by) is; raise it and keep it unobscured", "covered": true, "by": by])
            continue
        }
        let type: CGEventType = op == "hover" ? .mouseMoved : op == "down" ? .leftMouseDown : op == "move" ? .leftMouseDragged : .leftMouseUp
        guard let event = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .left) else {
            reply(["error": "no event"])
            continue
        }
        event.post(tap: .cghidEventTap)
        reply(["posted": op, "x": x, "y": y])
    default:
        reply(["error": "unknown op \(op) (trusted, window, hover, down, move, up)"])
    }
}

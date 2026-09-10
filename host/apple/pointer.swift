// The desktop pointer for the iOS Simulator (LLP 1035.003 §3, candidate 1;
// approved as apparatus by Charlie, 2026-09-10). UIKit offers no public
// touch synthesis, so a held contact on a simulator is a real mouse on the
// Mac's desktop, posted into the Simulator's window — the path the Messages
// record's hand-run scripts took, now the driver's, with the window-to-device
// mapping in one place (`scripts/agent.mjs`, `openIOS`'s `phaseSim`).
//
// Built by the driver with `swiftc` on first use. Speaks JSON lines on stdio:
//   {"op":"trusted"}                    {"trusted":bool} — Accessibility is granted to this process
//   {"op":"activate"}                   Simulator.app to the front (its window must be unobscured)
//   {"op":"window","title":"iPhone 17"} {"x","y","w","h","title"} of the Simulator window whose
//                                       title contains it — or, when window titles are unreadable
//                                       (no Screen Recording permission), the largest Simulator
//                                       window — in global display coordinates, top-left origin
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

func simulatorWindows() -> [[String: Any]] {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else { return [] }
    return list.filter {
        ($0[kCGWindowOwnerName as String] as? String) == "Simulator" && (($0[kCGWindowLayer as String] as? Int) ?? 0) == 0
    }
}

func bounds(_ window: [String: Any]) -> CGRect? {
    guard let b = window[kCGWindowBounds as String] as? [String: Any] else { return nil }
    return CGRect(dictionaryRepresentation: b as CFDictionary)
}

/// The owner of the topmost ordinary window under a desktop point — the
/// window a posted event would land in. Nothing is posted unless it is the
/// Simulator's: an operator's other window over the Simulator must never
/// receive a synthesized click (the record's misses, 2026-09-10).
func owner(under point: CGPoint) -> String? {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else { return nil }
    for window in list where (window[kCGWindowLayer as String] as? Int ?? 0) == 0 {
        if let r = bounds(window), r.contains(point) { return window[kCGWindowOwnerName as String] as? String }
    }
    return nil
}

while let line = readLine() {
    guard let data = line.data(using: .utf8),
          let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let op = req["op"] as? String else { reply(["error": "unreadable request"]); continue }
    switch op {
    case "trusted":
        reply(["trusted": AXIsProcessTrusted()])
    case "activate":
        if let app = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.iphonesimulator").first {
            // Activation from a process that is not frontmost is advisory;
            // Launch Services' `open` is the reliable raise from a terminal.
            let raised = app.activate(options: [.activateAllWindows])
            let open = Process()
            open.executableURL = URL(fileURLWithPath: "/usr/bin/open")
            open.arguments = ["-a", "Simulator"]
            try? open.run()
            open.waitUntilExit()
            reply(["activated": raised || open.terminationStatus == 0])
        } else {
            reply(["error": "Simulator is not running"])
        }
    case "window":
        let title = req["title"] as? String ?? ""
        let windows = simulatorWindows()
        let named = windows.first { (($0[kCGWindowName as String] as? String) ?? "").contains(title) }
        let largest = windows.max { (bounds($0)?.width ?? 0) * (bounds($0)?.height ?? 0) < (bounds($1)?.width ?? 0) * (bounds($1)?.height ?? 0) }
        guard let window = named ?? largest, let r = bounds(window) else {
            reply(["error": "no Simulator window on screen"])
            continue
        }
        reply(["x": r.origin.x, "y": r.origin.y, "w": r.width, "h": r.height,
               "title": (window[kCGWindowName as String] as? String) ?? "", "named": named != nil])
    case "hover", "down", "move", "up":
        guard let x = req["x"] as? Double, let y = req["y"] as? Double, x.isFinite, y.isFinite else {
            reply(["error": "\(op) needs finite x and y"])
            continue
        }
        let point = CGPoint(x: x, y: y)
        let top = owner(under: point)
        // A release is posted wherever it lands: a held button must never
        // be left down. Anything else lands only in the Simulator's window.
        if op != "up", top != "Simulator" {
            reply(["error": "the Simulator's window is not the topmost at \(Int(x)),\(Int(y)): \(top ?? "nothing") is; raise it and keep it unobscured", "covered": true, "by": top ?? ""])
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
        reply(["error": "unknown op \(op) (trusted, activate, window, down, move, up)"])
    }
}

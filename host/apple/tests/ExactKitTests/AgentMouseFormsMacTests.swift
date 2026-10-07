#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The agent's mouse forms reach a view and an app's local monitor as a
/// hand's do (#107): a middle click, a right click, a triple click, a wheel
/// at a point and a Shift-held drag, each with its button, click count,
/// modifiers and the point the driver named.
@MainActor
final class AgentMouseFormsMacTests: XCTestCase {
    private final class Recorder: NSView {
        var log: [String] = []
        override var isFlipped: Bool { true }
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
        static func line(_ e: NSEvent, in v: NSView) -> String {
            let p = v.convert(e.locationInWindow, from: nil)
            let mods = [(NSEvent.ModifierFlags.shift, "Shift"), (.control, "Control"), (.option, "Alt"), (.command, "Meta")]
                .filter { e.modifierFlags.contains($0.0) }.map(\.1).joined(separator: "+")
            let clicks = e.type == .scrollWheel ? "d=\(Int(e.scrollingDeltaY))" : "clicks=\(e.clickCount)"
            return "type=\(e.type.rawValue) button=\(e.buttonNumber) \(clicks) mods=\(mods.isEmpty ? "none" : mods) at=(\(Int(p.x)),\(Int(p.y)))"
        }
        private func note(_ e: NSEvent) { log.append(Self.line(e, in: self)) }
        override func mouseDown(with e: NSEvent) { note(e) }
        override func mouseDragged(with e: NSEvent) { note(e) }
        override func mouseUp(with e: NSEvent) { note(e) }
        override func rightMouseDown(with e: NSEvent) { note(e) }
        override func rightMouseUp(with e: NSEvent) { note(e) }
        override func otherMouseDown(with e: NSEvent) { note(e) }
        override func otherMouseUp(with e: NSEvent) { note(e) }
        override func scrollWheel(with e: NSEvent) { note(e) }
    }

    func testMiddleTripleWheelAtAndShiftDragReachTheViewAndTheMonitor() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "agent-mouse-forms")
        defer { session.destroy() }
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        // Away from the screen's origin, so a point in screen space is not one in the window.
        let window = NSWindow(contentRect: NSRect(x: 240, y: 160, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        // A window the server shows: AppKit hit-tests no event into one it does not.
        window.orderFrontRegardless()
        defer { window.close() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 16, "y": 16, "w": 200, "h": 120],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        // A module's view in the node's box, as `mouse-probe` is in #107's repro.
        let node = try XCTUnwrap(p.views[2])
        let view = Recorder(frame: node.bounds)
        node.addSubview(view)
        var monitored: [String] = []
        let mask: NSEvent.EventTypeMask = [.leftMouseDown, .leftMouseUp, .leftMouseDragged, .rightMouseDown, .rightMouseUp, .otherMouseDown, .otherMouseUp, .scrollWheel]
        let monitor = try XCTUnwrap(NSEvent.addLocalMonitorForEvents(matching: mask) { e in
            if e.window === window { monitored.append(Recorder.line(e, in: view)) }
            return e
        })
        defer { NSEvent.removeMonitor(monitor) }
        let agent = Agent(session: session)
        func tap(_ request: [String: Any]) -> [String: Any] {
            let wire = try! JSONSerialization.data(withJSONObject: request)
            return agent.tap(try! JSONSerialization.jsonObject(with: wire) as! [String: Any])
        }
        func drive(_ request: [String: Any], _ expected: [String], file: StaticString = #filePath, line: UInt = #line) {
            view.log.removeAll(); monitored.removeAll()
            let reply = tap(request)
            XCTAssertNil(reply["error"], "\(reply)", file: file, line: line)
            XCTAssertEqual(view.log, expected, "the view", file: file, line: line)
            XCTAssertEqual(monitored, expected, "the local monitor", file: file, line: line)
        }
        // NSEvent.EventType: 1/2 left down/up, 3/4 right, 6 left dragged, 22 wheel, 25/26 other.
        drive(["id": 2, "auxclick": true, "at": [30, 40], "modifiers": "Meta"],
              ["type=25 button=2 clicks=1 mods=Meta at=(30,40)", "type=26 button=2 clicks=1 mods=Meta at=(30,40)"])
        drive(["id": 2, "contextmenu": true, "at": [10, 10]],
              ["type=3 button=1 clicks=1 mods=none at=(10,10)", "type=4 button=1 clicks=1 mods=none at=(10,10)"])
        drive(["id": 2, "clicks": 3],
              (1...3).flatMap { n in ["type=1 button=0 clicks=\(n) mods=none at=(100,60)", "type=2 button=0 clicks=\(n) mods=none at=(100,60)"] })
        drive(["id": 2, "wheel": [0, 20], "at": [10, 10], "modifiers": "Shift"],
              ["type=22 button=0 d=-20 mods=Shift at=(10,10)"])
        // A Shift-held contact: down at (10, 20) in the view, a drag to (40, 20), up.
        drive(["id": 2, "phase": "down", "x": 26, "y": 36, "modifiers": "Shift"], ["type=1 button=0 clicks=1 mods=Shift at=(10,20)"])
        drive(["phase": "move", "x": 56, "y": 36], ["type=6 button=0 clicks=1 mods=Shift at=(40,20)"])
        drive(["phase": "up"], ["type=2 button=0 clicks=1 mods=Shift at=(40,20)"])
        XCTAssertNotNil(tap(["id": 2, "clicks": 4])["error"], "a fourth click is refused, not sent as one")
        XCTAssertNotNil(tap(["id": 2, "auxclick": true, "modifiers": "Hyper"])["error"], "a word that is no modifier is refused")
    }
}
#endif

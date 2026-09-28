#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1057 §10.6 on AppKit: a pan that began ends once with `panrelease`,
/// after its last delta, at the engine's tracker's velocity (LLP 1057.001 §3)
/// over the mouse's own timestamps; a cancelled one at rest; a press, none.
final class PanReleaseMacTests: XCTestCase {
    private var window: NSWindow?
    private var runtime: Runtime?
    override func tearDown() { window?.close(); window = nil; runtime = nil }

    private func host(_ handlers: [String]) -> (Presenter, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": handlers],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0]
        ]))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        // The library's tracker, as a session wires it.
        let rt = Runtime()
        runtime = rt
        p.onPanSample = { first, x, y, t in rt.panSample(first: first, x: x, y: y, t: t) }
        p.panVelocity = { t in rt.panVelocity(at: t) }
        return (p, p.views[1]!)
    }
    private func event(_ type: NSEvent.EventType, _ view: NSView, right: CGFloat, at t: Double) -> NSEvent {
        let at = view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil)
        return NSEvent.mouseEvent(with: type, location: NSPoint(x: at.x + right, y: at.y), modifierFlags: [],
            timestamp: t, windowNumber: window!.windowNumber, context: nil,
            eventNumber: 1, clickCount: 1, pressure: 1)!
    }

    func testAPanThatBeganReleasesOnceAfterItsLastDeltaAtTheTrackersVelocity() throws {
        let (p, node) = host(["pan", "panrelease"])
        var log: [String] = []
        var velocity: (Double, Double)?
        p.onPan = { _, dx, _ in log.append("pan \(dx)") }
        p.onPanRelease = { id, vx, vy in XCTAssertEqual(id, 1); log.append("release"); velocity = (vx, vy) }
        node.mouseDown(with: event(.leftMouseDown, node, right: 0, at: 1.0))
        for (right, t) in [(10.0, 1.016), (30, 1.032), (50, 1.048)] {
            node.mouseDragged(with: event(.leftMouseDragged, node, right: right, at: t))
        }
        node.mouseUp(with: event(.leftMouseUp, node, right: 70, at: 1.064))
        XCTAssertEqual(log, ["pan 10.0", "pan 20.0", "pan 20.0", "pan 20.0", "release"])
        let (vx, vy) = try XCTUnwrap(velocity)
        // Recency-weighted least squares over (1.0, 0) … (1.064, 70), px/s.
        XCTAssertEqual(vx, 1158.0739, accuracy: 0.01)
        XCTAssertEqual(vy, 0, accuracy: 1e-9)
    }

    func testAContactInsideTheSlopPressesAndReleasesNothing() {
        let (p, node) = host(["press", "pan", "panrelease"])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        p.onPan = { _, dx, _ in log.append("pan \(dx)") }
        p.onPanRelease = { _, _, _ in log.append("release") }
        node.mouseDown(with: event(.leftMouseDown, node, right: 0, at: 2.0))
        node.mouseDragged(with: event(.leftMouseDragged, node, right: 2, at: 2.02))
        node.mouseUp(with: event(.leftMouseUp, node, right: 2, at: 2.04))
        XCTAssertEqual(log, ["press 1"])
    }

    func testACancelledContactReleasesAtRestOnce() {
        let (p, node) = host(["pan", "panrelease"])
        var releases: [[Double]] = []
        p.onPanRelease = { _, vx, vy in releases.append([vx, vy]) }
        node.mouseDown(with: event(.leftMouseDown, node, right: 0, at: 3.0))
        node.mouseDragged(with: event(.leftMouseDragged, node, right: 40, at: 3.016))
        p.mouseLayoutPan.cancel() // Escape, or the window resigning key
        node.mouseUp(with: event(.leftMouseUp, node, right: 60, at: 3.032))
        XCTAssertEqual(releases, [[0, 0]])
        // A restart forgets a live contact without releasing it.
        node.mouseDown(with: event(.leftMouseDown, node, right: 0, at: 4.0))
        node.mouseDragged(with: event(.leftMouseDragged, node, right: 40, at: 4.016))
        p.mouseLayoutPan.abandon()
        XCTAssertEqual(releases, [[0, 0]])
    }

    func testANodeWithoutPanreleaseHearsNone() {
        let (p, node) = host(["pan"])
        var releases = 0, pans = 0
        p.onPan = { _, _, _ in pans += 1 }
        p.onPanRelease = { _, _, _ in releases += 1 }
        node.mouseDown(with: event(.leftMouseDown, node, right: 0, at: 5.0))
        node.mouseDragged(with: event(.leftMouseDragged, node, right: 40, at: 5.016))
        node.mouseUp(with: event(.leftMouseUp, node, right: 60, at: 5.032))
        XCTAssertEqual(pans, 2)
        XCTAssertEqual(releases, 0)
    }
}
#endif

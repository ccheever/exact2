#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

private var pointerEvents: [[String: Any]] = []

/// A game canvas's mouse on AppKit (LLP 1046.002 S1): chorded buttons, motion,
/// and the pointer lock a `data-pointer-lock` canvas takes, driven with the
/// events AppKit delivers. The lock is the process's own cursor association;
/// no window server delivery is involved, so this runs at a login window too.
final class CanvasPointerMacTests: XCTestCase {
    private func fixture() -> (ExactSession, NodeView, NSWindow) {
        _ = NSApplication.shared
        pointerEvents = []
        let s = ExactApp.shared.makeSession(label: "pointer-test")
        let m = GpuModule(create: { _, _, _, _, _ in 1 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 0 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 0 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, _, _, _, _ in 0 }, childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 }, errorPtr: { nil },
            wantsInput: { _ in 1 }, input: { _, p, n in
                let row = (try? JSONSerialization.jsonObject(with: Data(bytes: p!, count: n))) as? [String: Any] ?? [:]
                if row["t"] as? String == "pointer" { pointerEvents.append(row) }
                return 0
            }, messages: nil, published: nil, agent: { _, _, _ in 0 }, outPtr: { nil })
        s.canvases.modules[""] = m; s.canvases.attempted = [""]
        let canvas = NodeView(id: 100, kind: "canvas", presenter: s.presenter)
        canvas.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        s.presenter.root.addSubview(canvas); s.presenter.views[100] = canvas
        let e = Canvases.Entry(view: canvas, name: "world", values: []); e.id = 1; e.wantsInput = true; e.module = m
        s.canvases.entries[100] = e; canvas.canvasInput = CanvasInput(view: canvas)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.borderless], backing: .buffered, defer: false)
        // The canvas is the window's content: the hit view for every point in it.
        window.contentView = canvas
        return (s, canvas, window)
    }
    private func mouse(_ type: NSEvent.EventType, _ window: NSWindow, at p: NSPoint) -> NSEvent {
        NSEvent.mouseEvent(with: type, location: p, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                           windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
    }
    private func at(_ canvas: NodeView, _ window: NSWindow, _ x: CGFloat, _ y: CGFloat) -> NSPoint {
        canvas.convert(NSPoint(x: x, y: y), to: nil)
    }
    private var rows: [(String, Int, Double, Double)] {
        pointerEvents.map { ($0["phase"] as? String ?? "", $0["buttons"] as? Int ?? -1, $0["dx"] as? Double ?? .nan, $0["dy"] as? Double ?? .nan) }
    }

    func testChordedButtonsAreMovesAndMotionIsThePositionsChange() {
        let (s, canvas, window) = fixture(); defer { s.destroy() }
        let input = canvas.canvasInput!
        XCTAssertTrue(input.pointer(mouse(.leftMouseDown, window, at: at(canvas, window, 50, 50)), phase: "down"))
        XCTAssertTrue(input.pointer(mouse(.rightMouseDown, window, at: at(canvas, window, 50, 50)), phase: "down"))
        XCTAssertTrue(input.pointer(mouse(.leftMouseDragged, window, at: at(canvas, window, 60, 50)), phase: "move"))
        XCTAssertTrue(input.pointer(mouse(.rightMouseUp, window, at: at(canvas, window, 60, 50)), phase: "up"))
        XCTAssertTrue(input.pointer(mouse(.leftMouseUp, window, at: at(canvas, window, 60, 50)), phase: "up"))
        let r = rows
        guard r.count == 5 else { return XCTFail("events: \(pointerEvents)") }
        XCTAssertEqual(r.map(\.0), ["down", "move", "move", "move", "up"], "a second button joins as a move")
        XCTAssertEqual(r.map(\.1), [1, 3, 3, 1, 0])
        XCTAssertEqual(r[2].2, 10, accuracy: 0.001, "unlocked motion is the position's change")
        XCTAssertEqual(r[0].2, 0, "a down carries no motion")
        pointerEvents = []
        XCTAssertTrue(input.pointer(mouse(.otherMouseDown, window, at: at(canvas, window, 60, 50)), phase: "down"))
        XCTAssertEqual(pointerEvents.first?["buttons"] as? Int, 4, "a made middle press (buttonNumber 0) is the middle button")
        withExtendedLifetime(window) {}
    }

    func testALockableCanvasLocksTakesDeviceDeltasAndEscapeOrResigningKeyUnlocks() throws {
        let (s, canvas, window) = fixture(); defer { s.destroy() }
        canvas.props["dataset"] = #"{"pointer-lock":"true"}"#
        let input = canvas.canvasInput!
        XCTAssertTrue(input.pointer(mouse(.leftMouseDown, window, at: at(canvas, window, 50, 50)), phase: "down"))
        // CGAssociateMouseAndMouseCursorPosition is per process; it succeeds
        // without input focus (at a login window), but needs a window server.
        guard input.locked else {
            throw XCTSkip("no window server connection: CGAssociateMouseAndMouseCursorPosition refused")
        }
        let cg = CGEvent(mouseEventSource: nil, mouseType: .leftMouseDragged, mouseCursorPosition: .zero, mouseButton: .left)!
        cg.setIntegerValueField(.mouseEventDeltaX, value: 25)
        cg.setIntegerValueField(.mouseEventDeltaY, value: -4)
        let drag = NSEvent(cgEvent: cg)!
        XCTAssertTrue(input.pointer(drag, phase: "move"))
        XCTAssertEqual(rows.last?.2 ?? .nan, 25, accuracy: 0.001, "locked motion is the device's")
        XCTAssertEqual(rows.last?.3 ?? .nan, -4, accuracy: 0.001)
        let escape = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                      context: nil, characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53)!
        _ = input.key(escape, down: true, source: canvas)
        XCTAssertFalse(input.locked, "Escape releases the lock")
        XCTAssertTrue(input.pointer(mouse(.leftMouseUp, window, at: at(canvas, window, 50, 50)), phase: "up"))
        XCTAssertTrue(input.pointer(mouse(.leftMouseDown, window, at: at(canvas, window, 50, 50)), phase: "down"))
        XCTAssertTrue(input.locked)
        NotificationCenter.default.post(name: NSWindow.didResignKeyNotification, object: window)
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertFalse(input.locked, "the window ceasing to be key releases the lock")
        input.unlock()
        withExtendedLifetime(window) {}
    }
}
#endif

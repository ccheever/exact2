#if os(macOS)
import AppKit
import GameController
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

private var padRows: [[String: Any]] = []

/// A held pad follows the focus and the key window on AppKit (LLP 1127.003
/// D9) though it sends nothing more: GameController's snapshot controller,
/// announced as the framework announces one, holds its south button.
final class CanvasPadsMacTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    /// A window whose key status the test sets: a test process's borderless
    /// window is never key.
    private final class Window: NSWindow {
        var key = true
        override var isKeyWindow: Bool { key }
    }
    private func settle() { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }

    func testAHeldPadLeavesWithTheFocusAndTheKeyWindowAndComesBack() throws {
        _ = NSApplication.shared
        padRows = []
        let s = ExactApp.shared.makeSession(label: "pads-test")
        defer { s.destroy() }
        let m = GpuModule(create: { _, _, _, _, _ in 1 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 0 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 0 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, _, _, _, _ in 0 }, childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 }, errorPtr: { nil },
            wantsInput: { _ in 1 }, input: { _, p, n in
                let row = (try? JSONSerialization.jsonObject(with: Data(bytes: p!, count: n))) as? [String: Any] ?? [:]
                if row["t"] as? String == "pad" { padRows.append(row) }
                return 0
            }, messages: nil, published: nil, agent: { _, _, _ in 0 }, outPtr: { nil })
        s.surfaceHost.modules[""] = m; s.surfaceHost.attempted = [""]
        let p = s.presenter
        let window = Window(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.borderless], backing: .buffered, defer: false)
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        p.root.frame = p.viewport.bounds
        window.contentView = p.viewport
        let canvas = NodeView(id: 100, kind: "canvas", presenter: p)
        canvas.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.root.addSubview(canvas); p.views[100] = canvas
        let e = CanvasesHost.Entry(view: canvas, name: "world", values: []); e.id = 1; e.wantsInput = true; e.module = m
        s.surfaceHost.entries[100] = e
        // A control in the canvas, and an editor outside it.
        let control = NodeView(id: 101, kind: "view", presenter: p)
        control.frame = CGRect(x: 10, y: 10, width: 60, height: 40)
        control.props["action"] = "primary"; control.props["tabIndex"] = "0"
        canvas.addSubview(control); p.views[101] = control
        let editor = NSTextField(frame: NSRect(x: 220, y: 10, width: 120, height: 24))
        p.viewport.addSubview(editor)
        canvas.canvasInput = CanvasInputHost(view: canvas)
        XCTAssertTrue(control.isSurfaceControl)
        XCTAssertTrue(window.makeFirstResponder(control))
        settle()

        let pad = GCController.withExtendedGamepad()
        pad.extendedGamepad?.buttonA.setValue(1)
        NotificationCenter.default.post(name: .GCControllerDidConnect, object: pad)
        defer { NotificationCenter.default.post(name: .GCControllerDidDisconnect, object: pad) }
        settle()
        let index = try XCTUnwrap(padRows.last?["index"] as? Int)
        func heard() -> [String] {
            defer { padRows = [] }
            return padRows.filter { $0["index"] as? Int == index }.map { row in
                guard row["connected"] as? Bool == true else { return "gone" }
                return (row["buttons"] as? [Double])?.first == 1 ? "held" : "up"
            }
        }
        XCTAssertEqual(heard(), ["held"], "a control in the canvas has the canvas's pads")
        XCTAssertTrue(window.makeFirstResponder(editor))
        settle()
        XCTAssertEqual(heard(), ["gone"], "the focus left the canvas's control for an editor outside it")
        XCTAssertTrue(canvas.focusCanvas())
        settle()
        XCTAssertEqual(heard(), ["held"], "the pad as it is now, though it sent nothing")
        window.key = false
        NotificationCenter.default.post(name: NSWindow.didResignKeyNotification, object: window)
        settle()
        XCTAssertEqual(heard(), ["gone"], "another window took the keyboard")
        window.key = true
        NotificationCenter.default.post(name: NSWindow.didBecomeKeyNotification, object: window)
        settle()
        XCTAssertEqual(heard(), ["held"])
    }
}
#endif

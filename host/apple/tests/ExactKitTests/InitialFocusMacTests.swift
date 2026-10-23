// @ref LLP 1115.000 D7.5 — a new window puts the caret in its first key
// view, unless the author wrote `autofocus` elsewhere; a canvas that takes
// input keeps the old rule (nothing focused).
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

final class InitialFocusMacTests: XCTestCase {
    private func plan(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    /// A button, then two fields; `autofocus` puts it on the second.
    private func source(autofocus: Bool) -> String {
        """
        component App
          state a = ""
          state b = ""
          state n = 0
          action setA(v: string)
            a = v
          action setB(v: string)
            b = v
          action go
            n = n + 1
          view
            main width="100%" height="100%" display="flex" flex-direction="column" gap=8
              button press=go testId="go"
                text "Go"
              input value=a input=setA aria-label="First" testId="first"
              input value=b input=setB aria-label="Second" testId="second"\(autofocus ? " autofocus=true" : "")

        """
    }

    private func read(_ op: String, from session: ExactSession) throws -> [String: Any] {
        let pipe = Pipe(), savedOut = Agent.out, savedRoutes = Agent.routes
        Agent.out = pipe.fileHandleForWriting
        Agent.routes = [("launch", session)]
        defer { Agent.out = savedOut; Agent.routes = savedRoutes }
        Agent.handle("{\"op\":\"\(op)\"}")
        try pipe.fileHandleForWriting.close()
        return try XCTUnwrap(try JSONSerialization.jsonObject(with: pipe.fileHandleForReading.readDataToEndOfFile()) as? [String: Any])
    }

    func testTheFirstAgentReadDrainsTheQueuedLaunchAutofocus() throws {
        _ = NSApplication.shared
        let bytes = try plan(source(autofocus: true))
        for op in ["tree", "state", "layout"] {
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 400, height: 300)).error)
            let view = ExactView(session: session)
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.contentView = view
            defer { window.close() }
            let p = session.presenter
            p.releaseLaunchAutofocus()
            XCTAssertTrue(p.launchAutofocusPending)
            let reply = try read(op, from: session)
            XCTAssertNil(reply["error"], "\(op): \(reply)")
            XCTAssertFalse(p.launchAutofocusPending, "\(op) drains the launch before replying")
            XCTAssertEqual(p.focusedNode?.props["testId"], "second", op)
            let expected = try XCTUnwrap(p.focusedNode).id
            let next = try read(op, from: session)
            for observation in [reply, next] {
                XCTAssertNil(observation["error"], "\(op): \(observation)")
                if op == "tree" {
                    let nodes = try XCTUnwrap(observation["nodes"] as? [[String: Any]])
                    XCTAssertEqual(nodes.first { $0["focused"] as? Bool == true }?["id"] as? Int, Int(expected))
                } else if op == "state" {
                    XCTAssertEqual((observation["focus"] as? [String: Any])?["logical"] as? Int, Int(expected))
                } else {
                    // Layout carries geometry, not focus; read its focus through state below.
                    let nodes = try XCTUnwrap(observation["nodes"] as? [[String: Any]])
                    XCTAssertTrue(nodes.contains { $0["id"] as? Int == Int(expected) })
                }
            }
            let state = try read("state", from: session)
            XCTAssertEqual((state["focus"] as? [String: Any])?["logical"] as? Int, Int(expected), "the next read reports the same focus")
        }
    }

    func testAnUnshownWindowAnswersAgentReadsWithoutAnExplicitAutofocusRelease() throws {
        _ = NSApplication.shared
        let bytes = try plan(source(autofocus: true))
        for op in ["tree", "state", "layout"] {
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 400, height: 300)).error)
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            let view = ExactView(session: session)
            view.isHidden = true
            window.contentView = view
            defer { window.close() }
            XCTAssertFalse(window.isVisible)
            XCTAssertNil(session.activatedGeneration)
            XCTAssertTrue(session.presenter.launchAutofocusPending)
            let reply = try read(op, from: session)
            guard reply["error"] == nil else { XCTFail("\(op): \(reply)"); return }
            XCTAssertFalse(window.isVisible)
            XCTAssertTrue(view.isHidden)
            XCTAssertEqual(session.activatedGeneration, session.generation, "AppKit drew the layers while the window stayed hidden")
            XCTAssertFalse(session.presenter.launchAutofocusPending)
        }
    }

    /// The launch autofocus, and with it the window's first key view, as the
    /// session runs them the turn after its first frame.
    private func launched(_ p: Presenter) {
        p.releaseLaunchAutofocus()
        let limit = Date(timeIntervalSinceNow: 20)
        while !p.launchAutofocusReleased, Date() < limit { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        XCTAssertTrue(p.launchAutofocusReleased)
    }

    func testANewWindowPutsTheCaretInItsFirstKeyViewUnlessTheAuthorSaidAutofocus() throws {
        _ = NSApplication.shared
        let plain = try plan(source(autofocus: false)), authored = try plan(source(autofocus: true))
        // (the plan, the window's owner asks, Keyboard Navigation, the node that ends with the focus)
        let cases: [(String, Data, Bool, Bool, String?)] = [
            ("the first text field, past the button before it", plain, true, false, "first"),
            ("the author's autofocus wins", authored, true, false, "second"),
            ("with Keyboard Navigation on, the first control", plain, true, true, "go"),
            ("an embedder's window is its own: nothing", plain, false, false, nil),
            ("an embedder's window still honours autofocus", authored, false, false, "second"),
        ]
        for (name, bytes, asks, navigation, expected) in cases {
            KeyboardNavigation.agent = navigation
            defer { KeyboardNavigation.agent = nil }
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 400, height: 300)).error)
            let view = ExactView(session: session)
            view.focusesFirstKeyView = asks
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.initialFirstResponder = view
            window.autorecalculatesKeyViewLoop = false
            defer { window.close() }
            window.contentView = view
            let p = session.presenter
            XCTAssertNil(p.focusedNode, "\(name): nothing is focused before the first frame")
            launched(p)
            XCTAssertEqual(p.focusedNode?.props["testId"], expected, name)
            if let expected, expected != "go" {
                let field = try XCTUnwrap(p.views.values.first { $0.props["testId"] == expected }?.field)
                XCTAssertTrue(window.firstResponder === field.currentEditor(), "\(name): the caret is in the field")
            }
            XCTAssertFalse(view.focusesFirstKeyView, "\(name): once")
            // Once: a later release (a reload's) does not take the focus again.
            XCTAssertTrue(window.makeFirstResponder(nil))
            p.launchAutofocusReleased = false
            launched(p)
            XCTAssertNil(p.focusedNode, "\(name): the window's first key view is given the focus once, as the window opens")
        }
    }

    /// A session's window whose own first responder is the window (after
    /// `blur()`, or a menu a click opened closed): Tab goes to the first key
    /// view, the one the window opened with, and Shift-Tab to the last, as
    /// AppKit goes from such a window to its `initialFirstResponder`. The
    /// window's initial first responder is the session's view, which is no
    /// key view, so AppKit alone focused nothing.
    func testTabFromTheWindowItselfGoesToTheFirstKeyViewAndShiftTabToTheLast() throws {
        _ = NSApplication.shared
        let bytes = try plan(source(autofocus: false))
        // (Keyboard Navigation, the node Tab reaches, the node Shift-Tab reaches)
        for (navigation, first, last) in [(false, "first", "second"), (true, "go", "second")] {
            KeyboardNavigation.agent = navigation
            defer { KeyboardNavigation.agent = nil }
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 400, height: 300)).error)
            let view = ExactView(session: session)
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.initialFirstResponder = view
            window.autorecalculatesKeyViewLoop = false
            defer { window.close() }
            window.contentView = view
            let p = session.presenter
            launched(p)
            func tab(shift: Bool) throws {
                let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: shift ? .shift : [], timestamp: 0, windowNumber: window.windowNumber,
                                                           context: nil, characters: "\t", charactersIgnoringModifiers: "\t", isARepeat: false, keyCode: 48))
                // As the view's key monitor routes it, then AppKit.
                if !p.routeKey(event, focused: view.ownsShortcutFocus(in: window), in: window) { window.sendEvent(event) }
            }
            XCTAssertTrue(window.makeFirstResponder(nil))
            try tab(shift: false)
            XCTAssertEqual(p.focusedNode?.props["testId"], first, "Keyboard Navigation \(navigation): Tab")
            XCTAssertTrue(window.makeFirstResponder(nil))
            try tab(shift: true)
            XCTAssertEqual(p.focusedNode?.props["testId"], last, "Keyboard Navigation \(navigation): Shift-Tab")
            let field = try XCTUnwrap(p.views.values.first { $0.props["testId"] == last }?.field)
            XCTAssertTrue(window.firstResponder === field.currentEditor(), "the caret is in the field")
        }
    }

    /// With no key view nothing is focused, and a first key view in a canvas
    /// that takes input is passed over, as before: its caret would be in the
    /// canvas's first frame (LLP 1014's readback).
    func testNoKeyViewAndACanvasThatTakesInputFocusNothing() throws {
        keyboardNavigation(false)
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        defer { window.close() }
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"], ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"]], ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 100, "h": 30],
            ["op": "create", "id": 3, "kind": "view"], ["op": "frame", "id": 3, "x": 0, "y": 40, "w": 300, "h": 100],
            ["op": "create", "id": 4, "kind": "input", "props": ["value": ""]], ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 200, "h": 30],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
        ]))
        XCTAssertNil(p.initialKeyView, "only a button, and Keyboard Navigation is off: no key view")
        p.apply(wireBatch([["op": "children", "id": 3, "ids": [4]], ["op": "children", "id": 1, "ids": [2, 3]]]))
        let field = try XCTUnwrap(p.views[4]?.field)
        XCTAssertTrue(p.initialKeyView === field, "the first text field")
        // The box the field is in becomes a canvas that takes input.
        let canvas = try XCTUnwrap(p.views[3])
        canvas.canvasInput = CanvasInputHost(view: canvas)
        XCTAssertNil(p.initialKeyView, "a field in a canvas that takes input is not where a new window's caret goes")
    }
}
#endif

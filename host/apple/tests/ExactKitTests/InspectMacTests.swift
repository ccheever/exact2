#if os(macOS) && EXACT_INSPECT
import AppKit
import XCTest
@testable import ExactKit

/// Inspect mode's hit test on the Mac (LLP 1122 D5, T2c; InspectMac.swift):
/// a click selects the nearest view, up from the window's own hit view, that
/// stands for a plan node; in the title bar, the walk stops: the header's
/// heading as the window's title and its toolbar items are selected,
/// AppKit's own parts (the close button) select nothing, and say so.
final class InspectMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private func view(_ id: Int, _ kind: String = "view", _ props: [String: String] = [:], press: Bool = false, frame: [Double] = [0, 0, 60, 30]) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": kind, "props": props, "handlers": press ? ["press"] : []],
         ["op": "frame", "id": id, "x": frame[0], "y": frame[1], "w": frame[2], "h": frame[3]]]
    }
    private func children(_ id: Int, _ ids: [Int]) -> [String: Any] { ["op": "children", "id": id, "ids": ids] }

    /// A navigation root (1) whose route (2) holds a header (10) with a
    /// level-1 heading (12) and an Add button (13), then a page (15) holding
    /// a card (16) at (100, 100).
    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 400), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        var ops: [[String: Any]] = view(1, "view", ["navigationBack": "back", "navigationKey": "a"], frame: [0, 0, 700, 400])
        ops += view(2, "view", ["navigationKey": "a"], frame: [0, 0, 700, 400]) + view(10, "view", ["semanticTag": "header"])
        ops += view(12, "text", ["text": "Library", "accessibilityRole": "heading", "accessibilityHeadingLevel": "1"])
        ops += view(13, "button", ["accessibilityLabel": "Add Book", "testId": "add"], press: true) + view(130, "image", ["symbolName": "plus"]) + [children(13, [130])]
        ops += view(15, "view", [:], frame: [0, 0, 700, 400]) + view(16, "view", ["testId": "card"], frame: [100, 100, 200, 80])
        ops += [children(10, [12, 13]), children(15, [16]), children(2, [10, 15]), children(1, [2]), ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        XCTAssertTrue(p.toolbar.attach(to: window))
        window.orderFrontRegardless()
        window.contentView?.superview?.layoutSubtreeIfNeeded()
        return p
    }

    private func centre(_ v: NSView) -> NSPoint { let r = v.convert(v.bounds, to: nil); return NSPoint(x: r.midX, y: r.midY) }

    func testAClickSelectsTheNodeUnderItAndTheTitleBarsHeadingButNotAppKitsOwnParts() throws {
        let p = fixture()
        // Content: the card under the point.
        let card = try XCTUnwrap(p.views[16])
        XCTAssertEqual(InspectHost.pick(at: centre(card), in: window, p).id, 16)
        // The window's title is the header's heading: selected as the heading.
        XCTAssertEqual(window.title, "Library")
        var pending: [NSView] = window.contentView?.superview?.subviews ?? [], title: NSTextField?
        while !pending.isEmpty { let v = pending.removeFirst(); if let t = v as? NSTextField, t.stringValue == "Library", !v.isHidden { title = t; break }; pending += v.subviews }
        let titleField = try XCTUnwrap(title, "AppKit draws the window's title in the title bar")
        XCTAssertEqual(InspectHost.pick(at: centre(titleField), in: window, p).id, 12)
        // AppKit's own close button: nothing, said, and the walk does not climb out of the bar.
        let closeButton = try XCTUnwrap(window.standardWindowButton(.closeButton))
        let none = InspectHost.pick(at: centre(closeButton), in: window, p)
        XCTAssertNil(none.id, "\(none)")
        XCTAssertEqual(none.why, "That is part of the window's title bar, not a view of the app.")
    }

    private func key(_ type: NSEvent.EventType, _ code: UInt16, _ chars: String) -> NSEvent {
        NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window?.windowNumber ?? 0,
                         context: nil, characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)!
    }

    func testInspectModeTakesEveryKeyAndClickAndEscapesReleaseAfterItEnds() throws {
        _ = fixture()
        defer { Inspect.left = nil }
        Inspect.set(true, seq: 7)
        InspectHost.show(ExactApp.shared, on: true)
        XCTAssertNil(InspectHost.take(key(.keyDown, 0, "a")), "a key reaches no handler or key equivalent")
        XCTAssertNil(InspectHost.take(key(.keyUp, 0, "a")))
        let flags = NSEvent.keyEvent(with: .flagsChanged, location: .zero, modifierFlags: [.shift], timestamp: 0, windowNumber: window.windowNumber, context: nil,
                                     characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 56)!
        XCTAssertNil(InspectHost.take(flags), "a modifier too")
        let click = NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 150, y: 150), modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                       context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
        XCTAssertNil(InspectHost.take(click), "a click in a window no session's overlay is on yet is taken too")
        // Escape leaves; its release, after, is still inspect mode's; then keys are the app's again.
        XCTAssertNil(InspectHost.take(key(.keyDown, 53, "\u{1b}")))
        XCTAssertFalse(Inspect.on)
        XCTAssertTrue(InspectHost.draining)
        XCTAssertNil(InspectHost.take(key(.keyUp, 53, "\u{1b}")), "Escape's release reaches nothing")
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertFalse(InspectHost.draining)
        XCTAssertEqual(Inspect.left, 7, "the exit is kept until the server hears it")
    }
}

/// `select {up}` (tamber/1.17) over a real session, as on iOS (InspectIOSTests) and the web pages: walking out by
/// the plan's copies, each new copy owning an ancestor a step (a slot's host among them), at step `up` that copy's
/// outermost node, past the last the root, with `moved`; an ask for a selection that moved, or with no copies for its
/// plan, is nil, the selection as it is now. The fixture is apps/select-walk-fixture's; its copies come from the
/// compiler's source map, as the server's would.
final class InspectSelectUpMacTests: XCTestCase {
    private func plan(_ source: String) throws -> (plan: Data, copies: [Int]) {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path, "--map"]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let map = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: dir.appendingPathComponent("app.plan.map.json"))) as? [String: Any])
        var ids: [String: Int] = [:]
        let copies = (map["nodes"] as? [[String: Any]] ?? []).map { node -> Int in
            let key = (node["chain"] as? [[String: Any]] ?? []).map { "\($0["file"] ?? ""):\($0["line"] ?? ""):\($0["col"] ?? "")" }.joined(separator: ">")
            if let id = ids[key] { return id }
            ids[key] = ids.count
            return ids.count - 1
        }
        return (try Data(contentsOf: dir.appendingPathComponent("app.plan")), copies)
    }

    func testSelectUpWalksTheTreeByCopiesSlotHostsIncludedToTheRootAndAStaleAskOnNone() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let source = """
        component App
          view
            column testId="root" width="100%" padding=4 padding-top=240
              Post()
              Panel()
                text "panel fill" testId="panel-fill"
              Outer()
                Card()
                  text "wrapped" testId="wrapped"

        component Frame
          slot
          view
            column testId="frame" padding=2
              children

        component Panel
          slot
          view
            Frame()
              children

        component Card
          slot
          view
            column testId="card" padding=2
              column testId="card-inner" padding=2
                children

        component Glyph
          view
            text "g" testId="glyph"

        component Controls
          view
            Glyph()

        component Outer
          slot
          view
            column testId="outer" padding=2
              children

        component Post
          view
            column testId="post" padding=2
              Card()
                text "body" testId="body"
                Controls()

        """
        let built = try plan(source)
        XCTAssertNil(session.boot(plan: built.plan, size: CGSize(width: 400, height: 600)).error)
        Inspect.set(true, seq: 4)
        defer { Inspect.set(false, seq: 4); Inspect.copies = nil }
        func view(_ testId: String) throws -> UInt32 { try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, testId).id }
        func picked(_ testId: String) throws -> [String: Any] { try XCTUnwrap(Inspect.select(session, id: view(testId), clicked: true)?["selected"] as? [String: Any]) }
        func ask(_ selected: [String: Any], _ up: Int, plan: Any? = nil) -> [String: Any] {
            ["up": up, "plan": plan ?? selected["planDigest"] ?? NSNull(), "from": selected["site"] ?? NSNull(), "instance": selected["instance"] ?? []]
        }
        func lands(_ from: String, _ up: Int) throws -> (testId: String?, moved: [String: Int]?) {
            let selected = try picked(from)
            Inspect.copies = (selected["planDigest"] as? String, built.copies)
            let body = try XCTUnwrap(Inspect.selectUp(session, ask(selected, up)), from)
            let sel = body["selected"] as? [String: Any]
            XCTAssertNil(body["click"], "a move is no press: its outline scrolls the copy into view")
            return ((sel?["node"] as? Int).flatMap { session.presenter.views[UInt32($0)]?.props["testId"] }, sel?["moved"] as? [String: Int])
        }
        // (a) a caller's fill: its slot's host; the like row (Controls holds no view): the host; (b) a host one component
        // deeper: Frame; (c) the nearer host, then its wrapper; (d) past the last copy, the root, three of nine.
        for (from, up, at, stepped) in [("body", 1, "card", 1), ("glyph", 1, "card", 1), ("panel-fill", 1, "frame", 1), ("wrapped", 1, "card", 1), ("wrapped", 2, "outer", 2), ("glyph", 9, "root", 3)] {
            let r = try lands(from, up)
            XCTAssertEqual(r.testId, at, "\(from) up \(up)")
            XCTAssertEqual(r.moved, ["up": up, "stepped": stepped], "\(from) up \(up)")
        }
        // Stale: another plan's ask, no copies for the plan, another site: nil, the selection left as it is.
        let selected = try picked("glyph")
        XCTAssertNil(Inspect.selectUp(session, ask(selected, 1, plan: "another plan")), "asked of another plan's selection")
        Inspect.copies = ("another plan", built.copies)
        XCTAssertNil(Inspect.selectUp(session, ask(selected, 1)), "no copies for this plan")
        Inspect.copies = (selected["planDigest"] as? String, built.copies)
        var moved = ask(selected, 1); moved["from"] = -1
        XCTAssertNil(Inspect.selectUp(session, moved), "asked of a selection at another site")
        XCTAssertEqual(Inspect.selected?.id, try view("glyph"), "a stale ask leaves the selection")
        // The wire's encoding, as scripts/tamber-inspect.mjs writes it: runs [0, 2], [1, 1].
        XCTAssertEqual(Inspect.decodeCopies("r1:AAIBAQ=="), [0, 0, 1])
        // Bounded, as the pages are: a run past 2^20 sites, a varint past 4 bytes, one cut off, half a pair.
        for bad in ["r1:AICAgIAQ", "r1:gICAgICAgICA", "r1:AIA=", "r1:AA=="] { XCTAssertNil(Inspect.decodeCopies(bad), bad) }
        XCTAssertNil(Inspect.decodeCopies("r2:AA=="))
        // An inspect frame's copies are kept for their plan; a bad encoding keeps none.
        ExactApp.shared.inspectCommand(["kind": "inspect", "on": true, "seq": 4, "copies": ["plan": "p", "data": "r1:AAIBAQ=="]])
        XCTAssertEqual(Inspect.copies?.plan, "p"); XCTAssertEqual(Inspect.copies?.list, [0, 0, 1])
        ExactApp.shared.inspectCommand(["kind": "inspect", "on": true, "seq": 4, "copies": ["plan": "p", "data": "nope"]])
        XCTAssertNil(Inspect.copies)
        // A plan's copies alone (a status naming a new plan): kept, inspect mode as it was; then off, they stay kept.
        ExactApp.shared.inspectCommand(["kind": "inspect", "copies": ["plan": "q", "data": "r1:AAIBAQ=="]])
        XCTAssertTrue(Inspect.on, "copies alone never turn inspect mode off")
        XCTAssertEqual(Inspect.copies?.plan, "q"); XCTAssertEqual(Inspect.copies?.list, [0, 0, 1])
        ExactApp.shared.inspectCommand(["kind": "inspect", "on": false, "seq": 5])
        XCTAssertEqual(Inspect.copies?.plan, "q", "a plan's copies outlast inspect mode off (they come once per plan)")
        ExactApp.shared.inspectCommand(["kind": "inspect", "on": true, "seq": 6])
        // A `select` that is no object (the web pages take any but null or false) is no move, and leaves inspect on.
        let was = Inspect.selected?.id
        ExactApp.shared.inspectCommand(["kind": "inspect", "select": "up"])
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertTrue(Inspect.on, "a select that is no object never turns inspect mode off")
        XCTAssertEqual(Inspect.selected?.id, was)
    }
}
#endif

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

/// `select {up}` (tamber/1.16) over a real session, as on iOS (InspectIOSTests): the first level with a node
/// above the selection, the outermost there, by the runner's own tree; an ask for a selection that moved, or
/// one with nothing of its levels above, is nil, the selection as it is now. (The levels are the node's own
/// ancestors' sites, standing in for the server's map.)
final class InspectSelectUpMacTests: XCTestCase {
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

    func testSelectUpLandsOnTheOutermostAncestorOfTheFirstLevelWithOneAndAStaleAskOnNone() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let source = """
        component App
          view
            column width="100%" height="100%"
              Card(label="one")
              Card(label="two")

        component Card
          props
            label: string
          view
            column testId="card"
              row testId="line"
                text label testId="leaf"

        """
        XCTAssertNil(session.boot(plan: try plan(source), size: CGSize(width: 400, height: 300)).error)
        Inspect.set(true, seq: 4)
        defer { Inspect.set(false, seq: 4) }
        func above(_ id: UInt32) -> [(view: Int, site: Int)] {
            let o = (try? JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"ancestors\",\"id\":\(id)}").utf8))) as? [String: Any]
            return (o?["ancestors"] as? [[Any]] ?? []).dropFirst().compactMap { p in (p[0] as? Int).flatMap { v in (p[1] as? Int).map { (v, $0) } } }
        }
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "leaf" })
        let ancestors = above(node.id)
        let selected = try XCTUnwrap(Inspect.select(session, id: node.id, clicked: true)?["selected"] as? [String: Any])
        func ask(_ levels: [[Int]], plan: Any? = nil, instance: Any? = nil) -> [String: Any] {
            ["up": 1, "levels": levels, "plan": plan ?? selected["planDigest"] ?? NSNull(), "from": selected["site"] ?? NSNull(), "instance": instance ?? selected["instance"] ?? []]
        }
        // The card's row and column: the column is the outermost of the two.
        let level = [ancestors[0].site, try XCTUnwrap(ancestors.first { $0.site != ancestors[0].site }, "\(ancestors)").site]
        XCTAssertNil(Inspect.selectUp(session, ask([[-1]])), "nothing of its levels above it")
        XCTAssertNil(Inspect.selectUp(session, ask([level], plan: "another plan")), "asked of another plan's selection")
        XCTAssertNil(Inspect.selectUp(session, ask([level], instance: [["key": "elsewhere"]])), "asked of another copy")
        var moved = ask([level]); moved["from"] = -1
        XCTAssertNil(Inspect.selectUp(session, moved), "asked of a selection at another site")
        XCTAssertEqual(Inspect.selected?.id, node.id, "a stale ask leaves the selection")
        let body = try XCTUnwrap(Inspect.selectUp(session, ask([[-1], level])))
        let outermost = try XCTUnwrap(ancestors.last { level.contains($0.site) })
        XCTAssertEqual((body["selected"] as? [String: Any])?["node"] as? Int, outermost.view, "\(ancestors)")
        XCTAssertEqual(session.presenter.views[UInt32(outermost.view)]?.props["testId"], "card")
        XCTAssertNil(body["click"], "a move is no press: its outline scrolls the copy into view")
        XCTAssertEqual(Inspect.selected?.id, UInt32(outermost.view))
        // A nearer level with a node above wins over a later one that reaches higher.
        _ = Inspect.select(session, id: node.id, clicked: true)
        let nearer = try XCTUnwrap(Inspect.selectUp(session, ask([[ancestors[0].site], level])))
        XCTAssertEqual((nearer["selected"] as? [String: Any])?["node"] as? Int, ancestors.last { $0.site == ancestors[0].site }?.view, "\(ancestors)")
    }
}
#endif

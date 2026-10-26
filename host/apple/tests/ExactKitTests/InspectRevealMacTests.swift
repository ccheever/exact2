#if os(macOS) && EXACT_INSPECT
import AppKit
import XCTest
@testable import ExactKit

/// An outline brings the selection's own copy into view (Apple pass #2; Inspect.revealing): over a real session, a
/// feed of twelve posts scrolled to one, a selection there and its move up leave the feed where it is; that copy
/// scrolled away is brought back, not another; with no selection the first copy is, as before; and a held selection
/// whose copy shows nowhere scrolls nothing. (A move's outline carries no click token, nor does a click the build
/// didn't take itself: each scrolled the feed to another copy, rows away.)
final class InspectRevealMacTests: XCTestCase {
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

    func testAnOutlineBringsTheSelectionsOwnCopyIntoViewAndNeverAnother() throws {
        _ = NSApplication.shared
        let source = """
        component App
          view
            scroll testId="feed" width=400 height=300
              column gap=8 padding=8
                for n in ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"] key=n
                  Post()

        component Card
          slot
          view
            column testId="card" padding=2
              column testId="card-inner" padding=2 height=60
                children

        component Glyph
          view
            text "g" testId="glyph"

        component Controls
          view
            Glyph()

        component Post
          view
            column testId="post" padding=2
              Card()
                text "body" testId="body"
                Controls()

        """
        let built = try plan(source)
        let session = ExactApp.shared.makeSession(label: "inspect-reveal")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: built.plan, size: CGSize(width: 400, height: 300)).error)
        let window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = ExactView(session: session)
        window.orderFrontRegardless()
        _ = session.agentInstance.clock(["settle": true])
        Inspect.set(true, seq: 9)
        defer { Inspect.set(false, seq: 9); Inspect.copies = nil }

        let feed = try XCTUnwrap(try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "feed" }).scroll)
        let document = try XCTUnwrap(feed.documentView)
        // Each row's copies, top to bottom.
        func row(_ testId: String) -> [NodeView] {
            session.presenter.views.values.filter { $0.props["testId"] == testId && $0.window != nil }
                .sorted { $0.convert($0.bounds, to: document).minY < $1.convert($1.bounds, to: document).minY }
        }
        let glyphs = row("glyph"), cards = row("card")
        XCTAssertEqual(glyphs.count, 12); XCTAssertEqual(cards.count, 12)
        let top = { feed.contentView.bounds.origin.y }
        func scroll(to y: CGFloat) { feed.contentView.scroll(to: NSPoint(x: 0, y: y)); feed.reflectScrolledClipView(feed.contentView) }
        func shows(_ v: NSView) -> Bool { feed.contentView.documentVisibleRect.contains(v.convert(v.bounds, to: document)) }
        func outline(_ selected: [String: Any]) {
            InspectHost.outline(ExactApp.shared, ["kind": "outline", "plan": selected["planDigest"] ?? NSNull(), "sites": [selected["site"] ?? NSNull()]])
        }
        func select(_ v: NodeView) throws -> [String: Any] { try XCTUnwrap(Inspect.select(session, id: v.id, clicked: false)?["selected"] as? [String: Any]) }

        // The row to select: away from the card copy the runner lists first (the copy an outline used to bring in), so
        // that bringing in the one would never show the other.
        let views = Inspect.sites(session).views
        let cardSite = try XCTUnwrap(views.first { $0.id == cards[0].id }?.site)
        let listedFirst = try XCTUnwrap(views.first { $0.site == cardSite }.flatMap { v in cards.firstIndex { $0.id == v.id } })
        let k = listedFirst < 6 ? 11 : 0, far: CGFloat = k == 0 ? document.bounds.maxY : 0
        func toRow() { scroll(to: max(0, min(cards[k].convert(cards[k].bounds, to: document).minY - 20, document.bounds.maxY - feed.contentView.bounds.height))) }

        // (a) Row k's glyph selected, the feed on that post: its outline (no click token) moves nothing.
        toRow()
        let at = top()
        XCTAssertTrue(shows(glyphs[k]))
        let glyph = try select(glyphs[k])
        outline(glyph)
        XCTAssertEqual(top(), at, accuracy: 0.5, "the selected copy shows: the feed stays")
        // (b) Up one, to its card: its outline moves nothing either, and the card is row k's.
        Inspect.copies = (glyph["planDigest"] as? String, built.copies)
        let moved = try XCTUnwrap(Inspect.selectUp(session, ["up": 1, "plan": glyph["planDigest"] ?? NSNull(), "from": glyph["site"] ?? NSNull(), "instance": glyph["instance"] ?? []])?["selected"] as? [String: Any])
        XCTAssertEqual((moved["node"] as? Int).map(UInt32.init), cards[k].id)
        outline(moved)
        XCTAssertEqual(top(), at, accuracy: 0.5, "the move's copy shows: the feed stays")
        XCTAssertTrue(shows(cards[k]))
        // (c) The feed scrolled to the other end: the outline brings row k's card back, not the copy listed first.
        scroll(to: far)
        XCTAssertFalse(shows(cards[k]))
        outline(moved)
        XCTAssertTrue(shows(cards[k]), "the selection's own copy is brought in (row \(k); listed first: row \(listedFirst))")
        // (e) A held selection whose copy shows nowhere (here hidden, as a collapsed row): nothing scrolls, never to another copy.
        _ = try select(glyphs[k])
        scroll(to: far)
        let still = top()
        glyphs[k].isHidden = true
        outline(glyph)
        XCTAssertEqual(top(), still, accuracy: 0.5, "no other copy is scrolled to")
        glyphs[k].isHidden = false
        // (d) No selection (the tool's Outline of a token's uses): the first copy that shows, as before.
        Inspect.set(true, seq: 10)
        XCTAssertNil(Inspect.selected)
        toRow()
        let order = Inspect.sites(session).views.filter { $0.site == glyph["site"] as? Int }.compactMap { session.presenter.views[$0.id] }
        let first = try XCTUnwrap(order.first { !$0.isHiddenOrHasHiddenAncestor })
        outline(glyph)
        XCTAssertTrue(shows(first), "the first copy in sites order")
    }
}
#endif

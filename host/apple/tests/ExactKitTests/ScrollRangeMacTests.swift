// What `layout` says about how far a scroller's offset travels (LLP 1012 `rx`/`ry`), on a real AppKit scroller: its
// document past its clip view, the clip view's content insets included, never negative.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class ScrollRangeMacTests: XCTestCase {
    private func scroller(document: CGSize, insets: NSEdgeInsets) -> NSScrollView {
        let sv = NSScrollView(frame: NSRect(x: 0, y: 0, width: 300, height: 300))
        sv.hasVerticalScroller = false
        sv.hasHorizontalScroller = false
        sv.automaticallyAdjustsContentInsets = false
        sv.contentInsets = insets
        sv.documentView = NSView(frame: NSRect(origin: .zero, size: document))
        return sv
    }

    /// A document as wide as its clip view has the sideways range its side insets give it, 42 points each.
    func testSideInsetsAreARangeWhereTheDocumentFits() {
        let sv = scroller(document: CGSize(width: 300, height: 700), insets: NSEdgeInsets(top: 20, left: 42, bottom: 10, right: 42))
        let clip = sv.contentView
        XCTAssertEqual(clip.contentInsets.left, 42, "the scroller's insets are its clip view's")
        let range = Agent.range(of: sv)
        XCTAssertEqual(range["rx"] as? Double, Double(300 + 84 - clip.bounds.width), "\(range), clip \(clip.bounds)")
        XCTAssertEqual(range["ry"] as? Double, Double(700 + 30 - clip.bounds.height), "\(range), clip \(clip.bounds)")
        XCTAssertGreaterThan(range["rx"] as? Double ?? 0, 0, "\(range)")
    }

    /// `layout` reports the range in `nodes` and in `layout <target>`'s scroll chain, the viewport's entry included: a
    /// booted page's scroller, given side insets.
    @MainActor func testLayoutReportsTheRangeInNodesAndInTheChain() throws {
        _ = NSApplication.shared
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        component App
          view
            column padding=24
              column height=100 width=300 overflow="scroll" testId="scroller"
                text "Row" testId="row"
                view height=400 flex-shrink=0

        """
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "scroll-range-mac")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 600, height: 400)).error)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = ExactView(session: session)
        window.contentView?.layoutSubtreeIfNeeded()
        let p = session.presenter, agent = Agent(session: session)
        func node(_ name: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == name }, name) }
        let scroller = try node("scroller"), row = try node("row")
        let sv = try XCTUnwrap(scroller.scroll)
        sv.automaticallyAdjustsContentInsets = false
        sv.contentInsets = NSEdgeInsets(top: 0, left: 42, bottom: 0, right: 42)
        let want = Agent.range(of: sv)
        XCTAssertGreaterThan(want["rx"] as? Double ?? 0, 0, "the side insets are a range: \(want)")
        let flat = try XCTUnwrap((agent.layout()["nodes"] as? [[String: Any]])?.first { $0["id"] as? Int == Int(scroller.id) })
        XCTAssertEqual(flat["rx"] as? Double, want["rx"] as? Double, "\(flat)")
        XCTAssertEqual(flat["ry"] as? Double, want["ry"] as? Double, "\(flat)")
        let target = try XCTUnwrap(agent.layout(["id": Int(row.id)])["node"] as? [String: Any])
        let chain = try XCTUnwrap(target["scroll"] as? [[String: Any]], "\(target)")
        let entry = try XCTUnwrap(chain.first { $0["id"] as? Int == Int(scroller.id) }, "\(chain)")
        XCTAssertEqual(entry["rx"] as? Double, want["rx"] as? Double, "\(entry)")
        XCTAssertEqual(entry["ry"] as? Double, want["ry"] as? Double, "\(entry)")
        XCTAssertEqual(chain.first?["viewport"] as? Bool, true, "\(chain)")
        XCTAssertNotNil(chain.first?["rx"], "the viewport has its range too: \(chain)")
    }

    /// A document smaller than its clip view, with no insets, has no range: never a negative one.
    func testASmallDocumentHasNoRange() {
        let range = Agent.range(of: scroller(document: CGSize(width: 200, height: 100), insets: NSEdgeInsets()))
        XCTAssertEqual(range["rx"] as? Double, 0)
        XCTAssertEqual(range["ry"] as? Double, 0)
    }
}
#endif

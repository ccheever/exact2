import WebKit
import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#endif

#if os(macOS)
/// scripts/fixtures/iframe-partition.contract: a partitioned frame's
/// `FrameNavigation` records reach its actions through the runner, its
/// commands reach the frame, and `tree` shows what a person sees of it
/// (LLP 1020.002 D2, D3, §10).
final class IframePartitionSessionTests: XCTestCase {
    nonisolated(unsafe) private static var server: FramePages?

    func testRecordsReachActionsAndCommandsMoveTheFrame() throws {
        guard let module = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        _ = NSApplication.shared
        if Self.server == nil { Self.server = try FramePages(port: 16585) }
        let session = ExactApp.shared.makeSession(label: "iframe-partition")
        session.webviews.moduleLocation = { module }
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try fixturePlan("IFRAME_PARTITION"), size: CGSize(width: 640, height: 500)).error)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.orderFrontRegardless()
        defer { window.close() }
        let seen = { () -> String in
            session.presenter.views.values.first { $0.props["testId"] == "seen" }?.props["text"] ?? ""
        }
        let deadline = { (seconds: TimeInterval, done: () -> Bool) in
            let end = Date(timeIntervalSinceNow: seconds)
            while !done() && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        }
        deadline(20) { seen().hasPrefix("Page A|http://127.0.0.1:16585/a|") }
        XCTAssert(seen().hasPrefix("Page A|http://127.0.0.1:16585/a|"), seen())

        let toB = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "to-b" })
        session.presenter.press(toB.id)
        deadline(20) { seen().hasPrefix("Page B|http://127.0.0.1:16585/b|") }
        XCTAssert(seen().hasPrefix("Page B|http://127.0.0.1:16585/b|"), seen())

        let never = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "never" })
        session.presenter.press(never.id)
        deadline(10) { seen().hasSuffix("|InvalidStateError") }
        XCTAssert(seen().hasSuffix("|InvalidStateError"), seen())

        let tree = session.webviews.tree()
        let frame = try XCTUnwrap((tree["nodes"] as? [[String: Any]])?.first { ($0["props"] as? [String: Any])?["testId"] as? String == "guest" })
        XCTAssertEqual(frame["partition"] as? String, "fixture")
        let navigation = try XCTUnwrap(frame["navigation"] as? [String: Any])
        XCTAssertEqual(navigation["url"] as? String, "http://127.0.0.1:16585/b")
        XCTAssertNil(navigation["icons"], "tree shows what a person sees, nothing more")
    }
}
#endif

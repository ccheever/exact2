#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
@testable import ExactKit

/// The agent's `layout <node>` for a live node the host mounted no view for:
/// a native button's face draws its svg, so the svg's path has none. It
/// answers with the runner's rows and says nothing was mounted; an id the
/// runner doesn't know is still refused as stale.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class AgentUnmountedTests: XCTestCase {
    #if os(iOS)
    private var window: UIWindow!
    #else
    private var window: NSWindow!
    #endif
    override func tearDown() { MainActor.assumeIsolated {
        #if os(macOS)
        window?.close()
        #endif
        window = nil
    } }

    @MainActor func testALiveNodeWithNoViewOfItsOwnAnswersWithTheRunnersRows() throws {
        let session = ExactApp.shared.makeSession(label: "agent-unmounted")
        defer { session.destroy() }
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_AGENT_UNMOUNTED_PLAN"], "build.mjs --test compiles tests/fixtures/agent-unmounted.contract")))
        #if os(iOS)
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        session.presenter.viewport.frame = window.bounds
        window.addSubview(session.presenter.viewport)
        window.makeKeyAndVisible()
        #else
        _ = NSApplication.shared
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        #endif
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 400, height: 400)).error)
        let tree = try XCTUnwrap((try? JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"tree\"}").utf8))) as? [String: Any])
        let nodes = try XCTUnwrap(tree["nodes"] as? [[String: Any]])
        let path = try XCTUnwrap(nodes.first { $0["type"] as? String == "SvgPath" }?["id"] as? Int)
        let plain = try XCTUnwrap(nodes.first { ($0["props"] as? [String: Any])?["testId"] as? String == "plain" }?["id"] as? Int)
        let p = session.presenter, agent = Agent(session: session)
        XCTAssertNil(p.textHost(UInt32(path)), "the button's face draws the path: no view of its own")
        let reply = agent.layout(["id": path])
        let node = try XCTUnwrap(reply["node"] as? [String: Any], "\(reply)")
        XCTAssertEqual((node["native"] as? [String: Any])?["mounted"] as? Bool, false, "\(node)")
        let fill = try XCTUnwrap((node["style"] as? [String: Any])?["fill"] as? [String: Any], "\(node)")
        XCTAssertEqual(fill["value"] as? String, "#ff0000ff")
        XCTAssertNil(node["space"], "no box to report")
        // A mounted node still answers with its box.
        let mounted = try XCTUnwrap(agent.layout(["id": plain])["node"] as? [String: Any])
        XCTAssertNotNil(mounted["space"])
        XCTAssertNil((mounted["native"] as? [String: Any])?["mounted"])
        // An id the runner doesn't know is refused as stale.
        let stale = agent.layout(["id": 99_999])
        XCTAssertTrue((stale["error"] as? String)?.contains("stale node #99999") == true, "\(stale)")
    }
}
#endif

#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// The agent's tap and a sticky box on UIKit (theme2's 14:49 ruling; tests/fixtures/sticky-tap.contract), hosted: a
/// row whose middle the scroller's sticky dock covers is scrolled into the band above it before the tap, and pressed;
/// a row in the clear scrolls nothing.
final class AgentStickyRevealIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    func testARowUnderAStickyDockIsScrolledClearAndPressed() throws {
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_STICKY_TAP_PLAN"], "build.mjs --test --ios compiles the sticky-tap fixture")))
        let session = ExactApp.shared.makeSession(label: "sticky-tap")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        let agent = Agent(session: session)
        let node = { (id: String) in session.presenter.views.values.first { $0.props["testId"] == id } }
        let pressed = { () -> String? in
            let state = try? JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any]
            return (state?["slots"] as? [String: Any])?["pressed"] as? String
        }
        let port = try XCTUnwrap(node("port"))
        let row = { (band: ClosedRange<CGFloat>) in
            (1...20).compactMap { node("row-\($0)") }.first { r in band.contains(r.convert(CGPoint(x: r.bounds.midX, y: r.bounds.midY), to: port).y - port.bounds.minY) }
        }
        let docked = try XCTUnwrap(row(300...399))
        XCTAssertEqual(agent.reveal(["id": Int(docked.id)])["scrolled"] as? Bool, true)
        XCTAssertNil(agent.tap(["id": Int(docked.id)])["error"])
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        XCTAssertEqual(pressed(), docked.props["testId"] as? String)
        let clear = try XCTUnwrap(row(80...280))
        XCTAssertEqual(agent.reveal(["id": Int(clear.id)])["scrolled"] as? Bool, false)
    }
}
#endif

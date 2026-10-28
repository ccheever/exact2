#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The agent's tap and a sticky box (theme2's 14:49 ruling, the design tool's edit dock; tests/fixtures/sticky-tap.contract):
/// a row whose middle the scroller's sticky dock or header covers is scrolled into the band the box leaves before the
/// tap, and pressed; a row in the clear scrolls nothing; a dock taller than the band it would leave scrolls nothing,
/// and the tap refuses there.
@MainActor
final class AgentStickyRevealMacTests: XCTestCase {
    private var window: NSWindow!

    override func tearDown() { window?.close(); window = nil }

    func testARowUnderAStickyBoxIsScrolledClearAndPressed() throws {
        _ = NSApplication.shared
        clicksArriveAsInTheKeyWindow() // (a test's process is never the active app: its clicks would press only a control, not a row)
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_STICKY_TAP_PLAN"], "build.mjs --test compiles the sticky-tap fixture")))
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let view = ExactView(session: session)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 440), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let mount = NSView(frame: NSRect(x: 0, y: 0, width: 320, height: 440))
        window.contentView = mount
        view.frame = mount.bounds; mount.addSubview(view)
        window.orderFrontRegardless()
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 320, height: 440)).error)
        window.layoutIfNeeded()
        let agent = Agent(session: session)
        let node = { (id: String) in session.presenter.views.values.first { $0.props["testId"] == id } }
        let pressed = { () -> String? in
            let state = try? JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any]
            return (state?["slots"] as? [String: Any])?["pressed"] as? String
        }
        // Each row's middle in the port, from its top.
        let inPort = { (r: NodeView) -> CGFloat? in
            guard let port = node("port") else { return nil }
            let mid = r.convert(NSPoint(x: r.bounds.midX, y: r.bounds.midY), to: port).y
            return port.isFlipped ? mid : port.bounds.height - mid
        }
        let row = { (band: ClosedRange<CGFloat>) in (1...20).compactMap { node("row-\($0)") }.first { inPort($0).map(band.contains) ?? false } }
        // Under the dock (the port's bottom 100 points): scrolled clear, pressed.
        let docked = try XCTUnwrap(row(300...399))
        XCTAssertEqual(agent.reveal(["id": Int(docked.id)])["scrolled"] as? Bool, true)
        XCTAssertNil(agent.tap(["id": Int(docked.id)])["error"])
        session.presenter.settlePump()
        XCTAssertEqual(pressed(), docked.props["testId"] as? String)
        // In the clear: nothing moves.
        let clear = try XCTUnwrap(row(80...280))
        XCTAssertEqual(agent.reveal(["id": Int(clear.id)])["scrolled"] as? Bool, false)
        // Under the header (its top 60 points): scrolled clear the other way, pressed.
        let headed = try XCTUnwrap(row(0...59))
        XCTAssertEqual(agent.reveal(["id": Int(headed.id)])["scrolled"] as? Bool, true)
        XCTAssertNil(agent.tap(["id": Int(headed.id)])["error"])
        session.presenter.settlePump()
        XCTAssertEqual(pressed(), headed.props["testId"] as? String)
        // A dock that lets the pointer through (theme2's condition): a click at a row under it lands on the row, so the
        // cover rule finds none there, as the window's hit test does: nothing moves, and the tap presses the row.
        XCTAssertNil(agent.tap(["id": Int(try XCTUnwrap(node("ghost")).id)])["error"])
        session.presenter.settlePump(); window.layoutIfNeeded()
        let through = try XCTUnwrap(row(300...399))
        XCTAssertEqual(agent.reveal(["id": Int(through.id)])["scrolled"] as? Bool, false)
        XCTAssertNil(agent.tap(["id": Int(through.id)])["error"])
        session.presenter.settlePump()
        XCTAssertEqual(pressed(), through.props["testId"] as? String)
        XCTAssertNil(agent.tap(["id": Int(try XCTUnwrap(node("ghost")).id)])["error"])
        session.presenter.settlePump(); window.layoutIfNeeded()
        // A dock taller than the band it would leave: nothing moves, and the tap refuses there.
        XCTAssertNil(agent.tap(["id": Int(try XCTUnwrap(node("grow")).id)])["error"])
        session.presenter.settlePump(); window.layoutIfNeeded()
        let under = try XCTUnwrap(row(300...399))
        XCTAssertEqual(agent.reveal(["id": Int(under.id)])["scrolled"] as? Bool, false)
        XCTAssertNotNil(agent.tap(["id": Int(under.id)])["error"])
    }
}
#endif

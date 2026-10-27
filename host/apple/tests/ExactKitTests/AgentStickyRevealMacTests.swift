#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The agent's tap and a sticky box (theme2's 14:49 ruling, the design tool's edit dock): a row whose middle the
/// scroller's sticky dock covers is scrolled into the band the dock leaves before the tap, and pressed; a row in the
/// clear scrolls nothing.
@MainActor
final class AgentStickyRevealMacTests: XCTestCase {
    private var window: NSWindow!

    override func tearDown() { window?.close(); window = nil }

    func testARowUnderAStickyDockIsScrolledClearAndPressed() throws {
        _ = NSApplication.shared
        let rows = (1...20).map(String.init).joined(separator: ", ")
        let source = """
        component App
          state pressed = ""
          action press(name: string)
            pressed = name
          view
            column testId="root" width=320
              scroll testId="port" height=400
                column testId="header" position="sticky" top=0 height=60 z-index=2
                  text "Header"
                for i in [\(rows)] key=i
                  button testId=`row-${i}` press=press(`row-${i}`) height=50 width="100%"
                    text `Row ${i}`
                column testId="dock" position="sticky" bottom=0 height=100 z-index=2
                  text "Dock"

        """
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let view = ExactView(session: session)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 420), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let mount = NSView(frame: NSRect(x: 0, y: 0, width: 320, height: 420))
        window.contentView = mount
        view.frame = mount.bounds; mount.addSubview(view)
        XCTAssertNil(session.boot(plan: try compile(source), size: CGSize(width: 320, height: 420)).error)
        window.layoutIfNeeded()
        let agent = Agent(session: session)
        let node = { (id: String) in session.presenter.views.values.first { $0.props["testId"] == id } }
        let pressed = { () -> String? in
            let state = try? JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any]
            return (state?["slots"] as? [String: Any])?["pressed"] as? String
        }
        // Row 7's middle (60 + 6 * 50 + 25 = 385) lies under the dock (300..400).
        let row = try XCTUnwrap(node("row-7"))
        let revealed = agent.reveal(["id": Int(row.id)])
        XCTAssertEqual(revealed["scrolled"] as? Bool, true, "\(revealed)")
        let tapped = agent.tap(["id": Int(row.id)])
        XCTAssertNil(tapped["error"], "\(tapped)")
        session.presenter.settlePump()
        XCTAssertEqual(pressed(), "row-7")
        // A row now in the clear: nothing moves.
        let clear = try XCTUnwrap((1...20).compactMap { node("row-\($0)") }.first { r in
            guard let port = node("port") else { return false }
            let mid = r.convert(NSPoint(x: r.bounds.midX, y: r.bounds.midY), to: port).y
            return (80...280).contains(port.isFlipped ? mid : port.bounds.height - mid)
        })
        XCTAssertEqual(agent.reveal(["id": Int(clear.id)])["scrolled"] as? Bool, false)
    }

    private func compile(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }
}
#endif

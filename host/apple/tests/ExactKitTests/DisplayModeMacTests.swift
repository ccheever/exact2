#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class DisplayModeMacTests: XCTestCase {
    private final class Window: NSWindow {
        var fullscreen = false
        override var styleMask: NSWindow.StyleMask {
            get { fullscreen ? super.styleMask.union(.fullScreen) : super.styleMask }
            set { super.styleMask = newValue }
        }
    }

    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "shape Viewport\n  displayMode: string\ncomponent App\n  resource viewport = exactViewport() as shape Viewport\n  view\n    text viewport.displayMode testId=\"mode\"\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    private func assertMode(_ session: ExactSession, _ expected: String, file: StaticString = #filePath, line: UInt = #line) throws {
        let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"state\"}").utf8)) as? [String: Any])
        XCTAssertEqual((state["device"] as? [String: Any])?["displayMode"] as? String, expected, file: file, line: line)
        let resources = state["resources"] as? [String: Any]
        XCTAssertEqual((resources?["viewport"] as? [String: Any])?["displayMode"] as? String, expected, file: file, line: line)
        XCTAssertEqual(session.presenter.views.values.first { $0.props["testId"] == "mode" }?.props["text"], expected, file: file, line: line)
    }

    func testFullscreenNotificationsReachTheResourceAndAgentStateOfOnlyTheirWindow() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "display-mode"), other = ExactApp.shared.makeSession(label: "display-mode-other")
        let window = Window(contentRect: NSRect(x: 100, y: 100, width: 600, height: 400), styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false)
        let otherWindow = NSWindow(contentRect: NSRect(x: 50, y: 50, width: 400, height: 300), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        otherWindow.isReleasedWhenClosed = false
        otherWindow.contentView = ExactView(session: other)
        window.isReleasedWhenClosed = false
        let view = ExactView(session: session)
        window.contentView = view
        defer { window.close(); otherWindow.close(); session.destroy(); other.destroy() }
        for s in [session, other] { XCTAssertNil(s.boot(plan: try Self.plan.get(), size: CGSize(width: 600, height: 400)).error) }
        try assertMode(session, "standalone")
        // Supply the window server's state and notification without a global Space transition.
        for (notification, expected) in [(NSWindow.didEnterFullScreenNotification, "fullscreen"), (NSWindow.didExitFullScreenNotification, "standalone")] {
            window.fullscreen = expected == "fullscreen"
            NotificationCenter.default.post(name: notification, object: window)
            try assertMode(session, expected)
            try assertMode(other, "standalone")
        }
        // The agent substitutes this session alone; a later preference leaves it.
        XCTAssertNil(session.agentInstance.prefer(["media": ["display-mode": "fullscreen"]])["error"])
        XCTAssertNil(session.agentInstance.prefer(["media": ["prefers-reduced-motion": "reduce"]])["error"])
        try assertMode(session, "fullscreen")
        try assertMode(other, "standalone")
    }
}
#endif

import WebKit
import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// When a frame's web view is made (LLP 1020.002 §9): at its create op, once
/// the node has its props, as it was told to show; a hatched one waits for the
/// module's hatches, or is made without them when the module never comes.
final class IframeEmbedTests: XCTestCase {
    private var sessions: [ExactSession] = []
    #if os(macOS)
    private var window: NSWindow?
    #else
    private var window: UIWindow?
    #endif

    override func tearDown() {
        sessions.forEach { $0.destroy() }
        sessions = []
        #if os(macOS)
        window?.close()
        #else
        window?.isHidden = true
        #endif
        window = nil
        super.tearDown()
    }

    private func until(_ seconds: Double, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
    }

    /// A session shown in a window, its first pixel drawn.
    private func session(_ label: String) throws -> ExactSession {
        guard let arm = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        session.webviews.moduleLocation = { arm }
        XCTAssertNil(session.boot(size: CGSize(width: 400, height: 300)).error)
        #if os(macOS)
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.orderFrontRegardless()
        #else
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        let host = UIViewController()
        window.rootViewController = host
        window.makeKeyAndVisible()
        let view = ExactView(session: session)
        view.frame = host.view.bounds
        host.view.addSubview(view)
        #endif
        self.window = window
        until(5) { session.firstDrawMs != nil }
        // First pixel's activation, the module's preparation with it, runs on the next turn.
        until(5) { !session.activationInFlight }
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        return session
    }

    private func mount(_ session: ExactSession, _ props: [String: Any], style: [String: Any] = [:], y: Double = 0) {
        session.presenter.apply(wireBatch([
            ["op": "create", "id": 7001, "kind": "iframe", "props": props, "style": style],
            ["op": "roots", "ids": [7001]],
            ["op": "frame", "id": 7001, "x": 0.0, "y": y, "w": 200.0, "h": 100.0],
        ]))
    }

    /// `visibility: hidden` at create, before the web view exists, hides it
    /// once made, held or not.
    func testAHiddenFramesWebViewStaysHidden() throws {
        let session = try session("frame-hidden")
        mount(session, [:], style: ["visibility": "hidden"])
        let node = try XCTUnwrap(session.presenter.views[7001])
        until(5) { node.web != nil }
        let web = try XCTUnwrap(node.web)
        XCTAssertTrue(web.isHidden, "the web view of a frame hidden at create")
        // A new partition is a new web view in the old one's place, hidden too.
        node.props["partition"] = "memory-hidden"
        session.webviews.update(node)
        until(5) { node.web != nil && node.web !== web }
        let replaced = try XCTUnwrap(node.web)
        XCTAssertFalse(replaced === web, "a partition change replaces the web view")
        XCTAssertTrue(replaced.isHidden, "the replacing web view of a hidden frame")
    }

    #if os(macOS)
    /// A hatched frame mounted while the module may still connect waits; a
    /// module that fails to load makes it, uncalled, though it is two
    /// viewports below the window's bottom, where a held leaf waits to come near.
    func testAHatchedFrameWaitsForTheModuleOrMadeWithoutIt() throws {
        let session = try session("frame-waits")
        let natives = session.natives
        NativeViews.uninstallTable()
        natives.hasAppModule = true
        defer { natives.hasAppModule = false; NativeViews.uninstallTable() }
        XCTAssertTrue(natives.hatchesMayConnect("frame"))
        mount(session, ["hatch": "frame"], y: 900)
        let node = try XCTUnwrap(session.presenter.views[7001])
        until(0.5) { node.web != nil }
        XCTAssertNil(node.web, "a hatched frame waits for the module's hatches")
        // No artifact at the module's path: the load fails, and the frame is made.
        natives.prepareAppModule()
        XCTAssertFalse(natives.hatchesMayConnect("frame"))
        until(5) { node.web != nil }
        XCTAssertNotNil(node.web, "made without its hatch once the module will not connect")
    }
    #endif
}

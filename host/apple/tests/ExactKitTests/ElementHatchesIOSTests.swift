#if os(iOS)
import UIKit
import WebKit
import XCTest
@testable import ExactKit

/// LLP 1075.003.000 Stage 1 over the native fixture: a node marked
/// `hatch="badge"` — a leaf box that would otherwise be drawn flat into its
/// parent's layer — is a view, the app module's `element` hatch receives it
/// after the batch that mounts it and adds an interaction Exact leaves to
/// the app, hears its `data-*` change, and hears `ended` when the plan
/// reloads, its successor `built`. (A list row's hatched node, which this
/// host has no rows for, is driven by `smoke.mjs ios --app native-fixture`.)
final class ElementHatchesIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        NativeViews.uninstallTable()
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    private func node(_ session: ExactSession, _ testId: String) -> NodeView? {
        session.presenter.views.values.first { $0.props["testId"] == testId }
    }

    func testAHatchedLeafIsAViewItsHatchRunsAtEachMomentAndAReloadEndsIt() throws {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"])))
        let session = ExactApp.shared.makeSession(label: "element")
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        session.natives.installArtifact(try XCTUnwrap(env["EXACT_FIXTURE_MODULE"]))
        let log = { session.agent(#"{"op":"logs","since":0}"#) }
        until("the badge's hatch built") { log().contains("hatch element badge #") && log().contains(": built") }
        let badge = try XCTUnwrap(node(session, "hatched-badge"), "a hatched leaf box is a view")
        XCTAssertFalse(session.presenter.flats.isFlat(badge.id), "never a flat leaf")
        XCTAssertTrue(badge.interactions.contains { $0 is UIContextMenuInteraction }, "the hatch's interaction, which Exact leaves to the app")
        XCTAssertTrue(log().contains("hatch element badge: a view, not a flat leaf; its row is not reused (LLP 1075.003.000)"))
        // `data-tone` follows `composed`: its change reaches the hatch.
        let compose = try XCTUnwrap(node(session, "compose-home"))
        session.presenter.press(compose.id)
        until("the badge's hatch heard its data change") { log().contains("hatch element badge #\(badge.id): changed") }
        let hatches = try XCTUnwrap((session.presenter.elements.observation(session.hatchDiagnostics)["words"] as? [String: Any])?["badge"] as? [String: Any])
        XCTAssertEqual((hatches["calls"] as? [String: Int])?["changed"], 1)
        XCTAssertEqual(hatches["live"] as? Int, 1)
        // The plan boots again in this session: the node ends, a new one is built.
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        until("the old badge ended") { log().contains("hatch element badge #\(badge.id): ended") }
        until("the new badge is built") { self.node(session, "hatched-badge").map { $0 !== badge && !$0.interactions.isEmpty } == true }
    }

    /// LLP 1020.002 §9: a hatched frame mounted once the module has connected
    /// hands the `frame` hatch its `WKWebViewConfiguration` before the web
    /// view is made, the arm's data store and content controller on it; what
    /// the hatch adds there is the web view's, and its `element` call that
    /// follows is still the node's first (`isNew`).
    func testAHatchedFrameHearsItsConfigurationBeforeItsWebViewIsMade() throws {
        let env = ProcessInfo.processInfo.environment
        let arm = try XCTUnwrap(env["EXACT_TEST_WEB_MODULE"], "build.mjs --test --ios builds the web arm")
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"])))
        let session = ExactApp.shared.makeSession(label: "frame-configuration")
        sessions.append(session)
        session.webviews.moduleLocation = { arm }
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        session.natives.installArtifact(try XCTUnwrap(env["EXACT_FIXTURE_MODULE"]))
        until("the module connected") { session.natives.hatchesConnected }
        session.presenter.press(try XCTUnwrap(node(session, "show-frame")).id)
        until("the frame shows a web view") { (self.node(session, "hatched-frame")?.web as? WKWebView) != nil }
        let frame = try XCTUnwrap(node(session, "hatched-frame"))
        let web = try XCTUnwrap(frame.web as? WKWebView)
        XCTAssertEqual(web.configuration.applicationNameForUserAgent, "NativeFixture/1", "what the hatch set on the configuration is the web view's")
        until("the frame's hatch built") { session.agent(#"{"op":"logs","since":0}"#).contains("hatch element frame #\(frame.id): built") }
        let log = session.agent(#"{"op":"logs","since":0}"#)
        let configuring = try XCTUnwrap(log.range(of: "hatch element frame #\(frame.id): configuring"), log)
        let built = try XCTUnwrap(log.range(of: "hatch element frame #\(frame.id): built"))
        XCTAssertLessThan(configuring.lowerBound, built.lowerBound, "the configuration before the web view")
        let word = try XCTUnwrap((session.presenter.elements.observation(session.hatchDiagnostics)["words"] as? [String: Any])?["frame"] as? [String: Any])
        XCTAssertEqual(word["calls"] as? [String: Int], ["configuring": 1, "built": 1])
        // The fixture counts `built` when `isNew`, else `changed`.
        XCTAssertEqual(word["counters"] as? NSDictionary, ["configuring": 1, "built": 1] as NSDictionary, "\(word)")
    }

    /// A hatched frame mounted before the module's hatches connect, as on a
    /// cold launch's first screen, waits: its web view is made once they
    /// connect, after `configuring` (LLP 1020.002 §11).
    func testAHatchedFrameOnTheFirstScreenWaitsForTheModule() throws {
        let env = ProcessInfo.processInfo.environment
        let arm = try XCTUnwrap(env["EXACT_TEST_WEB_MODULE"], "build.mjs --test --ios builds the web arm")
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"])))
        let session = ExactApp.shared.makeSession(label: "frame-waits")
        sessions.append(session)
        session.webviews.moduleLocation = { arm }
        // The module made before first pixel: its hatches connect at first pixel.
        session.natives.installArtifact(try XCTUnwrap(env["EXACT_FIXTURE_MODULE"]))
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        XCTAssertFalse(session.natives.hatchesConnected, "before first pixel")
        session.presenter.press(try XCTUnwrap(node(session, "show-frame")).id)
        let frame = try XCTUnwrap(node(session, "hatched-frame"))
        XCTAssertNil(frame.web, "its web view waits for the hatches")
        view.layoutIfNeeded()
        until("the frame's hatch built") { session.agent(#"{"op":"logs","since":0}"#).contains("hatch element frame #\(frame.id): built") }
        let web = try XCTUnwrap(frame.web as? WKWebView)
        XCTAssertEqual(web.configuration.applicationNameForUserAgent, "NativeFixture/1", "the hatch had its configuration first")
        let log = session.agent(#"{"op":"logs","since":0}"#)
        let connected = try XCTUnwrap(log.range(of: "hatch: connected"), log)
        let configuring = try XCTUnwrap(log.range(of: "hatch element frame #\(frame.id): configuring"), log)
        XCTAssertLessThan(connected.lowerBound, configuring.lowerBound)
    }
}
#endif

#if os(macOS)
import AppKit
import XCTest
import QuartzCore
@testable import ExactKit

final class FrameHealthMacTests: XCTestCase {
    func testPlatformTrackpadReplayIsRefusedBeforeInputOrClockChanges() throws {
        guard !ExactEnv.agentFreezes else { throw XCTSkip("platform-timing input regression") }
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "platform-trackpad-replay")
        defer { session.destroy() }
        session.clock = nil
        let health = session.frameHealth
        _ = health.setAgentCapture("start")
        let before = session.presenter.agentPointer
        let reply = session.agentInstance.tap(["id": 1, "wheel": [0.0, 100.0], "gesture": true])
        XCTAssertEqual(reply["code"] as? String, "platform-trackpad-replay-unqualified")
        XCTAssertNil(session.clock)
        XCTAssertEqual(session.presenter.agentPointer, before)
        XCTAssertTrue(health.liveCapture)
    }
    func testDesktopPressPolicyFollowsSixtyOneTwentyAndHigherRefreshDisplays() {
        for maximum in [60, 120, 240, 360] {
            let range = PressClock.rate(maximum: maximum)
            XCTAssertEqual(range.maximum, Float(maximum))
            XCTAssertEqual(range.preferred, Float(maximum))
            XCTAssertLessThanOrEqual(range.minimum, range.maximum)
            XCTAssertEqual(Frames.requestedRange(maximum: maximum, full: true, paint: false).preferred, Float(maximum))
            XCTAssertLessThanOrEqual(Frames.requestedRange(maximum: maximum, full: false, paint: true).maximum, 60)
        }
    }
    func testClosingThePressWindowClearsAnUnfinishedPress() {
        _ = NSApplication.shared
        let presenter = Presenter()
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 320, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        let node = NodeView(id: 1, kind: "view", presenter: presenter)
        presenter.root.addSubview(node)
        node.press.aim(0.94, at: CACurrentMediaTime())
        PressClock.shared.run(node)
        window.close()
        XCTAssertEqual(node.press.to, 1)
    }
    func testDesktopCardUsesHostSlotTakesClicksWithinItsBoundsAndClampsWithoutResizingTheApp() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "desktop-frame-health")
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 640, height: 480), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let host = NSView(frame: CGRect(x: 0, y: 0, width: 640, height: 480)), exact = ExactView(session: session)
        window.contentView = host
        exact.frame = host.bounds; host.addSubview(exact)
        defer { window.close(); session.destroy() }
        session.frameHealth.macContainer = host
        let before = exact.frame
        exact.toggleFrameHealth(nil)
        let overlay = try XCTUnwrap(session.frameHealth.desktopOverlay)
        overlay.layoutSubtreeIfNeeded()
        XCTAssertEqual(exact.frame, before)
        XCTAssertTrue(session.frameHealth.hudVisible)
        overlay.place(at: CGPoint(x: -10_000, y: -10_000))
        XCTAssertGreaterThanOrEqual(try XCTUnwrap(session.frameHealth.hudBounds).minX, 8)
        XCTAssertNil(overlay.hitTest(CGPoint(x: 600, y: 400)), "outside the card, clicks reach the app")
        XCTAssertNotNil(overlay.hitTest(CGPoint(x: 100, y: 100)), "the opaque card takes the clicks within its bounds")
        let item = NSMenuItem(title: "Show Frame Health", action: #selector(ExactView.toggleFrameHealth(_:)), keyEquivalent: "")
        XCTAssertTrue(exact.validateMenuItem(item))
        XCTAssertEqual(item.title, "Hide Frame Health")
        exact.toggleFrameHealth(nil)
        XCTAssertFalse(session.frameHealth.hudVisible)
        XCTAssertTrue(overlay.isHidden)
    }
}
#endif

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
    func testDesktopCardKeepsItsPositionAlongEveryEdgeAfterLayout() {
        _ = NSApplication.shared
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let overlay = FrameHealthOverlayMac(health: health)
        overlay.frame = CGRect(x: 0, y: 0, width: 900, height: 900)
        overlay.setVisible(true)
        overlay.layoutSubtreeIfNeeded()
        let right = overlay.bounds.maxX - 8 - overlay.card.frame.width
        let top = overlay.bounds.maxY - 8 - overlay.card.frame.height
        let x = 8 + (right - 8) * 0.37
        for target in [CGPoint(x: 8, y: 150), CGPoint(x: right, y: 180), CGPoint(x: 160, y: 8), CGPoint(x: x, y: top)] {
            overlay.card.grabbed?()
            overlay.card.moved?(target)
            overlay.card.released?(.zero)
            XCTAssertEqual(overlay.card.frame.minX, target.x, accuracy: 0.001)
            XCTAssertEqual(overlay.card.frame.minY, target.y, accuracy: 0.001)
        }
        overlay.card.layer?.removeAllAnimations()
        overlay.setVisible(false)
        overlay.setVisible(true)
        overlay.needsLayout = true
        overlay.layoutSubtreeIfNeeded()
        XCTAssertEqual(overlay.card.frame.minX, x, accuracy: 0.001)
        overlay.frame.size.width = 1200
        overlay.layout()
        XCTAssertEqual(overlay.card.frame.minX, 8 + (overlay.bounds.width - 16 - overlay.card.frame.width) * 0.37, accuracy: 0.001)
        XCTAssertEqual(overlay.card.frame.maxY, overlay.bounds.maxY - 8, accuracy: 0.001)
        overlay.setVisible(false)
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
        XCTAssertEqual(overlay.card.layer?.cornerCurve, .continuous, "the card's outline is Apple's continuous curve")
        // A drag past the window's edge carries the card part of the way, and on release it springs back
        overlay.card.moved?(CGPoint(x: -10_000, y: -10_000))
        XCTAssertLessThan(overlay.card.frame.minX, 8)
        XCTAssertGreaterThan(overlay.card.frame.minX, 8 - 640)
        overlay.card.released?(.zero)
        XCTAssertGreaterThanOrEqual(try XCTUnwrap(session.frameHealth.hudBounds).minX, 8)
        XCTAssertNil(overlay.hitTest(CGPoint(x: 600, y: 400)), "outside the card, clicks reach the app")
        XCTAssertNotNil(overlay.hitTest(CGPoint(x: 100, y: 100)), "the opaque card takes the clicks within its bounds")
        // Details takes the place of the charts at the same size; a second click returns them
        let size = overlay.card.frame.size
        overlay.card.activate?(.details)
        XCTAssertFalse(overlay.card.detailsScroll.isHidden)
        XCTAssertEqual(overlay.card.frame.size, size)
        XCTAssertEqual(overlay.card.detailsScroll.frame, FrameHealthCardFace.Layout(width: size.width, showsDetails: true).detailsArea)
        overlay.card.activate?(.details)
        XCTAssertTrue(overlay.card.detailsScroll.isHidden)
        let item = NSMenuItem(title: "Frame Health", action: #selector(ExactView.toggleFrameHealth(_:)), keyEquivalent: "")
        XCTAssertTrue(exact.validateMenuItem(item))
        XCTAssertEqual(item.title, "Frame Health")
        XCTAssertEqual(item.state, .on, "the shown card's item is checked")
        exact.toggleFrameHealth(nil)
        XCTAssertFalse(session.frameHealth.hudVisible)
        XCTAssertTrue(overlay.isHidden)
    }
}
#endif

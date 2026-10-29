#if os(iOS)
import UIKit
import QuartzCore
import XCTest
@testable import ExactKit

final class FrameHealthIOSTests: XCTestCase {
    func testPresentedMenuAndCardKeepAppGeometryAndSupportAccessibilityType() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "the developer sheet and frame-health card in a real host window")
        }
        let session = ExactApp.shared.makeSession(label: "frame-health-ui")
        let window = UIWindow(windowScene: scene)
        let root = UIViewController()
        window.rootViewController = root
        window.makeKeyAndVisible()
        let app = ExactView(session: session)
        root.view.addSubview(app)
        app.frame = root.view.bounds
        app.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        XCTAssertNil(session.boot(size: app.bounds.size).error)
        app.fit(); window.layoutIfNeeded()
        defer {
            root.dismiss(animated: false)
            window.isHidden = true
            session.destroy()
        }
        DevMenu.install(on: window, session: session, controller: root, planPath: nil, frameHealthContainer: root.view)
        let before = app.frame
        let overlay = try XCTUnwrap(DevMenu.frameHealthOverlay())
        XCTAssertTrue(overlay.isHidden)
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        XCTAssertTrue(overlay.superview === root.view)
        XCTAssertTrue(root.view.subviews.first === app)
        XCTAssertEqual(app.frame, before, "host controls cannot resize the app")

        let navigation = DevMenuController.navigation()
        let shown = expectation(description: "native developer sheet presented")
        root.present(navigation, animated: false) { shown.fulfill() }
        wait(for: [shown], timeout: 5)
        let menu = try XCTUnwrap(navigation.viewControllers.first as? DevMenuController)
        menu.tableView.layoutIfNeeded()
        XCTAssertEqual(menu.groups[1].first?.title, "Hide Frame Health")
        let light = XCTAttachment(image: UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        })
        light.name = "Developer menu — native sheet"
        light.lifetime = .keepAlways
        add(light)
        let handlePoint = overlay.handle.convert(CGPoint(x: 22, y: 22), to: window)
        XCTAssertFalse(window.hitTest(handlePoint, with: nil)?.isDescendant(of: overlay) == true,
            "the sheet and its dismissal surface take precedence over dragging")

        navigation.setOverrideTraitCollection(UITraitCollection(preferredContentSizeCategory: .accessibilityExtraExtraExtraLarge), forChild: menu)
        navigation.overrideUserInterfaceStyle = .dark
        menu.tableView.reloadData()
        menu.view.layoutIfNeeded()
        let row = IndexPath(row: 0, section: 0)
        let cell = try XCTUnwrap(menu.tableView.cellForRow(at: row))
        XCTAssertGreaterThan(cell.bounds.height, 56)
        XCTAssertEqual((cell.contentConfiguration as? UIListContentConfiguration)?.textProperties.numberOfLines, 0)
        let dismissed = expectation(description: "native developer sheet dismissed")
        root.dismiss(animated: false) { dismissed.fulfill() }
        wait(for: [dismissed], timeout: 5)
        root.overrideUserInterfaceStyle = .dark
        overlay.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
        overlay.setNeedsLayout(); overlay.layoutIfNeeded()
        XCTAssertLessThanOrEqual(overlay.values.convert(overlay.values.bounds, to: overlay).maxY, overlay.card.frame.maxY)
        let card = XCTAttachment(image: UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        })
        card.name = "Frame health — native card"
        card.lifetime = .keepAlways
        add(card)
        XCTAssertEqual(app.frame, before)
        overlay.close.sendActions(for: .touchUpInside)
        XCTAssertFalse(overlay.desiredVisible)
        overlay.setVisible(true)
        session.destroy()
        XCTAssertTrue(overlay.isHidden)
        XCTAssertNil(overlay.superview, "destroying the session removes its host readout")
    }

    func testReadoutUsesExistingClockPassesBodyTouchesAndPreservesItsPosition() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let links = FrameClock.shared.linksMade
        let overlay = FrameHealthOverlay(health: health)
        overlay.frame = CGRect(x: 0, y: 0, width: 430, height: 900)
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        XCTAssertEqual(FrameClock.shared.linksMade, links, "showing the readout never starts a sampler")
        XCTAssertFalse(overlay.card.frame.isEmpty)
        XCTAssertTrue(overlay.bounds.contains(overlay.card.frame))
        XCTAssertEqual(overlay.accessibilityIdentifier, "exact.host.frame-health-overlay")
        let grip = overlay.handle.convert(CGPoint(x: 22, y: 22), to: overlay)
        let close = overlay.close.convert(CGPoint(x: 22, y: 22), to: overlay)
        let body = overlay.values.convert(CGPoint(x: 10, y: 10), to: overlay)
        XCTAssertNotNil(overlay.hitTest(grip, with: nil))
        XCTAssertNotNil(overlay.hitTest(close, with: nil))
        XCTAssertNil(overlay.hitTest(body, with: nil), "reading the card does not consume the app's touches")
        XCTAssertGreaterThanOrEqual(overlay.handle.bounds.height, 44)
        XCTAssertGreaterThanOrEqual(overlay.close.bounds.width, 44)
        XCTAssertTrue(overlay.values.adjustsFontForContentSizeCategory)
        XCTAssertTrue(overlay.values.text?.contains("FPS avg: Idle") == true)

        let action = try XCTUnwrap(overlay.handle.accessibilityCustomActions?.first { $0.name == "Move down" })
        let old = overlay.card.frame.origin
        _ = overlay.perform(try XCTUnwrap(action.selector))
        XCTAssertEqual(overlay.card.frame.minY, old.y + 44, accuracy: 0.1)
        overlay.setVisible(false)
        XCTAssertNil(health.changed)
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        XCTAssertEqual(overlay.card.frame.minY, old.y + 44, accuracy: 0.1)
        overlay.frame.size = CGSize(width: 250, height: 400)
        overlay.layoutIfNeeded()
        XCTAssertTrue(overlay.bounds.contains(overlay.card.frame), "resizing clamps the card into its own window")
        overlay.close.sendActions(for: .touchUpInside)
        XCTAssertFalse(overlay.desiredVisible)
        XCTAssertFalse(health.hudVisible)
    }

    func testPublicationIsBoundedAndIdleReloadVirtualAndDisabledStayExplicit() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        var publications = 0
        health.changed = { publications += 1 }
        func drain(until count: Int) {
            let deadline = Date().addingTimeInterval(2)
            while publications < count, Date() < deadline { RunLoop.main.run(mode: .default, before: deadline) }
        }
        for i in 0..<120 {
            let t = 10 + Double(i) / 120
            health.record(arrival: t, target: t + 1 / 120, period: 1 / 120, finished: t + 0.001)
            if i == 0 { drain(until: 1) }
            if i == 60 { drain(until: 2) }
        }
        XCTAssertEqual(publications, 2)
        XCTAssertEqual(health.snapshot().callbacks, 120)
        XCTAssertEqual(health.snapshot().intervals, 119)
        XCTAssertTrue(health.reply(virtual: false)["presentedFrames"] is NSNull)
        XCTAssertEqual(health.reply(virtual: true)["status"] as? String, "virtual")
        health.stop()
        drain(until: 3)
        XCTAssertEqual(publications, 3)
        health.record(arrival: 100, target: 100.01, period: 1 / 120, finished: 100.001)
        XCTAssertEqual(health.snapshot().intervals, 119, "stopped time does not become a gap")
        health.reset()
        XCTAssertEqual(health.snapshot().callbacks, 0)
        XCTAssertEqual(health.snapshot().generation, 1)
        let disabled = FrameHealth(enabled: false)
        defer { disabled.destroy() }
        disabled.record(arrival: 1, target: 2, period: 1, finished: 1.1)
        XCTAssertEqual(disabled.snapshot().callbacks, 0)
        let overlay = FrameHealthOverlay(health: disabled)
        overlay.setVisible(true)
        XCTAssertFalse(disabled.enabled, "showing the UI cannot override collection policy")
        XCTAssertTrue(overlay.values.text?.contains("disabled") == true)
    }

    func testChartRendersLiveHistoryWithoutStartingAFrameSource() {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let made = FrameClock.shared.linksMade
        let start = CACurrentMediaTime() - 6
        for bucket in 0..<10 {
            for index in 0..<60 {
                let hitch = bucket > 5 || bucket == 5 && index == 59 ? 0.060 : 0
                let time = start + Double(bucket) * 0.5 + Double(index) / 120 + hitch
                health.record(arrival: time, target: time + 1 / 120, period: 1 / 120, finished: time + 0.001)
            }
            _ = health.window.reply(health, now: start + Double(bucket) * 0.5 + 0.6)
        }
        let overlay = FrameHealthOverlay(health: health)
        overlay.frame = CGRect(x: 0, y: 0, width: 393, height: 852)
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        XCTAssertEqual(FrameClock.shared.linksMade, made)
        XCTAssertTrue(overlay.chart.points.contains { ($0.slowest ?? 120) < 20 })
        XCTAssertTrue(overlay.chart.accessibilityValue?.contains("120 hertz") == true)
        XCTAssertTrue(overlay.values.text?.contains("FPS avg:") == true)
        let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: overlay.card.bounds).image { context in
            overlay.card.layer.render(in: context.cgContext)
        })
        attachment.name = "Frame health — callback chart with a hitch"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    func testScrollingKeepsTheAppChartLiveAndPublishesInTrackingMode() {
        let health = FrameHealth(enabled: true)
        let scroll = FrameHealth(enabled: true, source: "native-scroll-ui-update")
        health.nativeHealth = scroll
        defer { health.destroy() }
        let overlay = FrameHealthOverlay(health: health)
        overlay.frame = CGRect(x: 0, y: 0, width: 393, height: 852)
        overlay.setVisible(true)
        health.nativeScrollActive = true
        var published = false
        health.changed = {
            overlay.refresh()
            published = overlay.chart.points.last?.average != nil
        }
        CFRunLoopAddCommonMode(CFRunLoopGetMain(), CFRunLoopMode(rawValue: RunLoop.Mode.tracking.rawValue as CFString))
        let ticks = Timer(timeInterval: 1 / 120, repeats: true) { _ in
            let time = CACurrentMediaTime()
            health.record(arrival: time, target: time + 1 / 120, period: 1 / 120, finished: time + 0.001)
            scroll.record(arrival: time, target: time + 0.01, period: 0, finished: time + 0.001)
        }
        RunLoop.main.add(ticks, forMode: .tracking)
        defer { ticks.invalidate() }
        let end = Date().addingTimeInterval(5)
        while !published, Date() < end { RunLoop.main.run(mode: .tracking, before: end) }
        XCTAssertTrue(published, "the app chart refreshes before tracking ends, even with an active unknown-cadence scroll observer")
        XCTAssertFalse(overlay.values.text?.contains("Unknown") == true)
    }

    func testInspectorHeaderFitsNarrowAndAccessibilityWidths() {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let controller = FrameHealthInspectorController(health: health)
        controller.loadViewIfNeeded()
        for category in [UIContentSizeCategory.large, .accessibilityExtraExtraExtraLarge] {
            controller.traitOverrides.preferredContentSizeCategory = category
            for width: CGFloat in [240, 320, 393, 852] {
                controller.view.frame = CGRect(x: 0, y: 0, width: width, height: 800)
                controller.view.setNeedsLayout()
                controller.view.layoutIfNeeded()
                controller.viewDidLayoutSubviews()
                controller.tableView.tableHeaderView?.layoutIfNeeded()
                let source = controller.source.convert(controller.source.bounds, to: controller.view)
                XCTAssertGreaterThan(source.width, 0)
                XCTAssertGreaterThanOrEqual(source.minX, 0)
                XCTAssertLessThanOrEqual(source.maxX, width)
                XCTAssertGreaterThanOrEqual(source.height, 44)
                XCTAssertLessThanOrEqual(controller.chart.frame.maxY, controller.tableView.tableHeaderView?.bounds.height ?? 0)
                XCTAssertFalse(controller.source.hasAmbiguousLayout)
            }
        }
    }

    func testMenuUsesNativeRowsAndBuildDetailsHaveTheirOwnScreen() throws {
        let navigation = DevMenuController.navigation()
        navigation.view.frame = CGRect(x: 0, y: 0, width: 430, height: 900)
        navigation.view.layoutIfNeeded()
        let menu = try XCTUnwrap(navigation.viewControllers.first as? DevMenuController)
        menu.loadViewIfNeeded()
        menu.view.frame = navigation.view.bounds
        menu.view.layoutIfNeeded()
        XCTAssertEqual(menu.tableView.style, .insetGrouped)
        XCTAssertEqual(menu.tableView.numberOfSections, 3)
        XCTAssertNotNil(menu.navigationItem.rightBarButtonItem)
        let row = IndexPath(row: 0, section: 0)
        let cell = try XCTUnwrap(menu.tableView.cellForRow(at: row))
        XCTAssertGreaterThan(cell.bounds.width, 350, "commands use the native sheet width")
        XCTAssertGreaterThanOrEqual(cell.bounds.height, 44)
        XCTAssertEqual((cell.contentConfiguration as? UIListContentConfiguration)?.text, "Reload")
        XCTAssertEqual(menu.groups[1].first?.title, "Show Frame Health")
        XCTAssertFalse(menu.groups.flatMap { $0 }.contains { $0.title == "Copy Info" })
        menu.tableView(menu.tableView, didSelectRowAt: IndexPath(row: 0, section: 2))
        XCTAssertTrue(navigation.viewControllers.last?.view is UITextView)
        XCTAssertEqual(navigation.viewControllers.last?.title, "Build Information")
        XCTAssertEqual(navigation.sheetPresentationController?.widthFollowsPreferredContentSizeWhenEdgeAttached, false)
    }
}
#endif

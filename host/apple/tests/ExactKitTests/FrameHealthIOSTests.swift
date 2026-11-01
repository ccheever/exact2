#if os(iOS)
import UIKit
import QuartzCore
import XCTest
@testable import ExactKit

final class FrameHealthIOSTests: XCTestCase {
    /// A window for an overlay: the card reads only while it can be seen.
    private func hosted(_ overlay: FrameHealthOverlay, in scene: UIWindowScene, size: CGSize) -> UIWindow {
        let window = UIWindow(windowScene: scene)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        overlay.frame = CGRect(origin: .zero, size: size)
        window.rootViewController?.view.addSubview(overlay)
        addTeardownBlock { window.isHidden = true }
        return window
    }

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
        // Attached to the bottom edge in regular height, where a sheet can rest at the medium detent
        if window.traitCollection.horizontalSizeClass == .compact && window.traitCollection.verticalSizeClass == .regular,
           let sheet = navigation.sheetPresentationController {
            XCTAssertGreaterThan(navigation.view.frame.height, window.bounds.height * 0.8, "the sheet opens at full height, not half")
            sheet.selectedDetentIdentifier = .medium
            // UIKit moves the sheet at a later layout pass
            let deadline = Date().addingTimeInterval(5)
            while navigation.view.frame.height >= window.bounds.height * 0.7, Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
            XCTAssertLessThan(navigation.view.frame.height, window.bounds.height * 0.7, "and rests at half height too")
            sheet.selectedDetentIdentifier = .large
        }
        let menu = try XCTUnwrap(navigation.viewControllers.first as? DevMenuController)
        menu.tableView.layoutIfNeeded()
        XCTAssertEqual(menu.groups[1].first?.title, "Hide Frame Health")
        let light = XCTAttachment(image: UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        })
        light.name = "Developer menu — native sheet"
        light.lifetime = .keepAlways
        add(light)
        let cardPoint = overlay.card.convert(CGPoint(x: 160, y: 100), to: window)
        XCTAssertFalse(window.hitTest(cardPoint, with: nil)?.isDescendant(of: overlay) == true,
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
        XCTAssertTrue(overlay.bounds.contains(overlay.card.frame))
        let card = XCTAttachment(image: UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        })
        card.name = "Frame health — native card"
        card.lifetime = .keepAlways
        add(card)
        XCTAssertEqual(app.frame, before)
        overlay.hideReadout()
        XCTAssertFalse(overlay.desiredVisible)
        overlay.setVisible(true)
        session.destroy()
        XCTAssertTrue(overlay.isHidden)
        XCTAssertNil(overlay.superview, "destroying the session removes its host readout")
    }

    func testCardUsesExistingClockTakesTouchesWithinItsBoundsAndPreservesItsPosition() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let links = FrameClock.shared.linksMade
        let overlay = FrameHealthOverlay(health: health)
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "a real host window for the frame-health card")
        }
        _ = hosted(overlay, in: scene, size: CGSize(width: 430, height: 900))
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        XCTAssertEqual(FrameClock.shared.linksMade, links, "showing the readout never starts a sampler")
        XCTAssertFalse(overlay.card.frame.isEmpty)
        XCTAssertTrue(overlay.bounds.contains(overlay.card.frame))
        XCTAssertEqual(overlay.accessibilityIdentifier, "exact.host.frame-health-overlay")
        // The card is opaque: it takes the touches within its bounds, and the rest reach the app
        let body = overlay.card.convert(CGPoint(x: 160, y: 100), to: overlay)
        XCTAssertTrue(overlay.hitTest(body, with: nil) === overlay.card)
        let outside = CGPoint(x: overlay.card.frame.maxX + 20, y: overlay.card.frame.maxY + 20)
        XCTAssertNil(overlay.hitTest(outside, with: nil), "outside the card, touches reach the app")
        let layout = FrameHealthCardFace.Layout(width: overlay.card.bounds.width)
        for (control, rect) in [(FrameHealthCardFace.Control.close, layout.close), (.title, layout.title), (.status, layout.status),
                                (.copy, layout.copy)] {
            // Each button's touch target is 44 points tall and at least as wide
            XCTAssertEqual(layout.control(at: CGPoint(x: rect.midX, y: rect.midY - 21)), control)
            XCTAssertEqual(layout.control(at: CGPoint(x: rect.midX + 18, y: rect.midY)), control)
        }
        XCTAssertEqual(overlay.card.state.status, .idle)
        XCTAssertEqual(overlay.card.state.rows.map(\.1), ["—", "—"], "no figure without data")

        let elements = overlay.card.accessibilityElements?.compactMap { $0 as? UIAccessibilityElement } ?? []
        XCTAssertEqual(elements.first?.accessibilityLabel, "Close frame health", "Close comes first, in its corner")
        let title = try XCTUnwrap(elements.first { $0.accessibilityLabel == "Frame health" })
        XCTAssertTrue(title.accessibilityTraits.contains(.button), "the title opens the definitions")
        // The card rests in a corner; moving it down takes it to the bottom one, where it stays
        let action = try XCTUnwrap(title.accessibilityCustomActions?.first { $0.name == "Move down" })
        let bottom = overlay.bounds.inset(by: overlay.safeAreaInsets).maxY - 8
        XCTAssertTrue(action.actionHandler?(action) == true)
        XCTAssertEqual(overlay.card.frame.maxY, bottom, accuracy: 0.5)
        overlay.setVisible(false)
        XCTAssertNil(health.changed)
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        XCTAssertEqual(overlay.card.frame.maxY, bottom, accuracy: 0.5)
        overlay.frame.size = CGSize(width: 250, height: 400)
        overlay.layoutIfNeeded()
        XCTAssertTrue(overlay.bounds.contains(overlay.card.frame), "resizing clamps the card into its own window")
        XCTAssertTrue(overlay.card.compact, "where the whole card does not fit, it leaves out its charts")
        overlay.hideReadout()
        XCTAssertFalse(overlay.desiredVisible)
        XCTAssertFalse(health.hudVisible)
    }

    func testPublicationIsBoundedAndIdleReloadVirtualAndDisabledStayExplicit() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "a real host window for the frame-health card")
        }
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
        _ = hosted(overlay, in: scene, size: CGSize(width: 393, height: 852))
        overlay.setVisible(true)
        XCTAssertFalse(disabled.enabled, "showing the UI cannot override collection policy")
        XCTAssertTrue(overlay.card.state.request.0.contains("disabled"))
    }

    func testChartsRenderTheReadersSamplesWithoutStartingAFrameSource() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        health.requestedRate(120)
        let made = FrameClock.shared.linksMade
        let overlay = FrameHealthOverlay(health: health)
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "a real host window for the frame-health card")
        }
        _ = hosted(overlay, in: scene, size: CGSize(width: 393, height: 852))
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        let start = CACurrentMediaTime()
        var time = start
        for bucket in 1...10 {
            while time < start + Double(bucket) * 0.5 {
                // One 60 ms hitch in the sixth half second
                if bucket == 6, time - start > 2.7, time - start < 2.71 { time += 0.060 }
                health.record(arrival: time, target: time + 1 / 120, period: 1 / 120, finished: time + 0.002)
                time += 1.0 / 120
            }
            overlay.refresh(now: start + Double(bucket) * 0.5 + 0.001)
        }
        XCTAssertEqual(FrameClock.shared.linksMade, made)
        XCTAssertEqual(overlay.card.state.status, .live)
        XCTAssertEqual(overlay.card.state.rows[0].1, "120")
        XCTAssertEqual(overlay.card.state.pacing.0, "Uneven", "the hitch is in the last 5 seconds")
        // A pause freezes the figures: later frames change nothing until it resumes
        overlay.card.activate?(.status)
        XCTAssertEqual(overlay.card.state.status, .paused)
        health.record(arrival: time, target: time + 0.1, period: 0.1, finished: time + 0.001)
        overlay.refresh(now: start + 6)
        XCTAssertEqual(overlay.card.state.rows[0].1, "120")
        overlay.card.activate?(.status)
        XCTAssertEqual(overlay.card.state.status, .live)
        // Hidden, the card reads nothing and releases what it drew from
        overlay.setVisible(false)
        XCTAssertNil(overlay.card.presentation.input)
        let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: overlay.card.bounds).image { context in
            overlay.card.layer.render(in: context.cgContext)
        })
        attachment.name = "Frame health — charts with a hitch"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    func testScrollingKeepsTheAppChartLiveAndPublishesInTrackingMode() throws {
        let health = FrameHealth(enabled: true)
        let scroll = FrameHealth(enabled: true, source: "native-scroll-ui-update")
        health.nativeHealth = scroll
        defer { health.destroy() }
        let overlay = FrameHealthOverlay(health: health)
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "a real host window for the frame-health card")
        }
        _ = hosted(overlay, in: scene, size: CGSize(width: 393, height: 852))
        overlay.setVisible(true)
        health.nativeScrollActive = true
        var published = false
        health.changed = {
            overlay.refresh()
            published = overlay.card.state.rows[0].1 != "—"
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
    }
}
#endif

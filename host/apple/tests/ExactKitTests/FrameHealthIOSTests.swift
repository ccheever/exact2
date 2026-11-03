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
        XCTAssertEqual(menu.groups[1].first?.title, "Frame Health")
        XCTAssertEqual(menu.groups[1].first?.checked, true, "the shown card's row is checked")
        XCTAssertEqual(menu.tableView.cellForRow(at: IndexPath(row: 0, section: 1))?.accessoryType, .checkmark)
        XCTAssertFalse(menu.groups[1].contains { $0.title.contains("Details") || $0.title.contains("Share") }, "the card offers these itself")
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
        // Build Information's text clears the sheet's side insets, as the menu's rows do: on the iPhone Duo the sheet's
        // bar items stand in a strip at its side
        menu.tableView(menu.tableView, didSelectRowAt: IndexPath(row: 0, section: 2))
        let pushed = expectation(description: "Build Information pushed")
        if let transition = navigation.transitionCoordinator {
            transition.animate(alongsideTransition: nil) { _ in pushed.fulfill() }
        } else {
            pushed.fulfill()
        }
        wait(for: [pushed], timeout: 5)
        let information = try XCTUnwrap(navigation.topViewController?.view as? UITextView)
        navigation.view.layoutIfNeeded()
        let whole = try XCTUnwrap(information.textRange(from: information.beginningOfDocument, to: information.endOfDocument))
        let text = information.selectionRects(for: whole).map(\.rect).filter { $0.width > 0 }.reduce(CGRect.null) { $0.union($1) }
        let safe = information.safeAreaInsets
        XCTAssertGreaterThanOrEqual(text.minX, safe.left + 20 - 0.5, "the text starts clear of the left inset (\(safe))")
        XCTAssertLessThanOrEqual(text.maxX, information.bounds.width - safe.right - 20 + 0.5, "and ends clear of the right one (\(safe))")
        let buildInformation = XCTAttachment(image: UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        })
        buildInformation.name = "Developer menu — Build Information"
        buildInformation.lifetime = .keepAlways
        add(buildInformation)
        let dismissed = expectation(description: "native developer sheet dismissed")
        root.dismiss(animated: false) { dismissed.fulfill() }
        wait(for: [dismissed], timeout: 5)
        // Details takes the place of the card's charts at the same size, and Share asks what to share,
        // then opens the share sheet with that one item
        func presented<Controller: UIViewController>(_: Controller.Type) throws -> Controller {
            wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in root.presentedViewController is Controller }, object: nil)], timeout: 10)
            return try XCTUnwrap(root.presentedViewController as? Controller)
        }
        func close() {
            let closed = expectation(description: "presentation dismissed")
            root.dismiss(animated: false) { closed.fulfill() }
            wait(for: [closed], timeout: 5)
        }
        let size = overlay.card.frame.size
        overlay.card.activate?(.details)
        XCTAssertTrue(overlay.card.state.showsDetails)
        XCTAssertFalse(overlay.card.detailsScroll.isHidden)
        XCTAssertEqual(overlay.card.frame.size, size, "the card keeps its size")
        XCTAssertEqual(overlay.card.presentation.details.map(\.name), ["Cadence", "Target rate", "Content latency", "Frames",
            "Requested slots", "Missed frames", "Skipped refreshes", "Late frames", "Limited by the system"])
        XCTAssertNil(root.presentedViewController, "and opens no sheet")
        if #available(iOS 26.0, *) { XCTAssertTrue(overlay.card.detailsScroll.topEdgeEffect.isHidden, "no edge effect blends the rows") }
        XCTAssertEqual(overlay.card.detailsScroll.contentInsetAdjustmentBehavior, .never)
        let details = XCTAttachment(image: UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        })
        details.name = "Frame health — Details"
        details.lifetime = .keepAlways
        add(details)
        overlay.card.activate?(.details)
        XCTAssertTrue(overlay.card.detailsScroll.isHidden, "a second tap returns the charts")
        overlay.card.activate?(.share)
        let choices = try presented(UIAlertController.self)
        XCTAssertEqual(choices.actions.map(\.title), ["Agent Report", "Summary", "Image", "Cancel"])
        close()
        overlay.share(.image)
        // On a phone the sheet rises from the bottom edge, and UIKit replaces the popover's source
        XCTAssertEqual(try presented(UIActivityViewController.self).excludedActivityTypes, [.assignToContact])
        close()
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
                                (.details, layout.details), (.share, layout.share)] {
            // Each button's touch target is 44 points tall and at least as wide, out to the card's edge or
            // halfway to the next button
            XCTAssertEqual(layout.control(at: CGPoint(x: rect.midX, y: rect.midY - 21)), control)
            XCTAssertEqual(layout.control(at: CGPoint(x: rect.midX + (control == .share ? 18 : -18), y: rect.midY)), control)
        }
        // The card measures the screen while it shows, so it is live from the start
        XCTAssertEqual(overlay.card.state.status, .live)
        XCTAssertEqual(overlay.card.state.request.0, "This app's frames")
        XCTAssertEqual(overlay.card.state.unit, "late")

        let elements = overlay.card.accessibilityElements?.compactMap { $0 as? UIAccessibilityElement } ?? []
        XCTAssertEqual(elements.first?.accessibilityLabel, "Close frame health", "Close comes first, in its corner")
        let title = try XCTUnwrap(elements.first { $0.accessibilityLabel == "Frame health" })
        XCTAssertTrue(title.accessibilityTraits.contains(.button), "the title opens the definitions")
        // The card rests in a corner, 8 points from the window's edges past its safe area; moving it down
        // takes it to the bottom one, where it stays
        let action = try XCTUnwrap(title.accessibilityCustomActions?.first { $0.name == "Move down" })
        let bottom = overlay.bounds.maxY - 8
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
        let overlay = FrameHealthOverlay(health: health, measuresScreen: false)
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
        // A pause keeps the figures while the card goes on reading: later frames change nothing until it resumes
        overlay.card.activate?(.status)
        XCTAssertEqual(overlay.card.state.status, .paused)
        XCTAssertTrue(health.chart === overlay.card.chart, "the charts still take each frame")
        health.record(arrival: time, target: time + 0.1, period: 0.1, finished: time + 0.001)
        overlay.refresh(now: start + 6)
        XCTAssertEqual(overlay.card.state.rows[0].1, "120")
        overlay.card.activate?(.status)
        XCTAssertEqual(overlay.card.state.status, .live)
        XCTAssertEqual(overlay.card.state.rows[0].0, "Last 1.0s", "resumed, the face shows the read taken while it was paused")
        // Hidden and shown again, the card starts unpaused
        overlay.card.activate?(.status)
        overlay.setVisible(false)
        overlay.setVisible(true)
        XCTAssertNotEqual(overlay.card.state.status, .paused)
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
        let overlay = FrameHealthOverlay(health: health, measuresScreen: false)
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

    func testTheScreenSeesEachFrameAndTheMainThreadsWorkOnlyWhileItRuns() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("UIUpdateLink is available from iOS 18") }
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "a real host window for the screen's frames")
        }
        let window = UIWindow(windowScene: scene)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        addTeardownBlock { window.isHidden = true }
        let scroll = UIScrollView(frame: window.bounds), dot = CALayer()
        scroll.contentSize = CGSize(width: window.bounds.width, height: 20_000)
        window.rootViewController?.view.addSubview(scroll)
        window.layer.addSublayer(dot)
        func run(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
        run(0.3)
        let screen = FrameHealthScreen(), chart = FrameHealthChart(in: CALayer())
        chart.start()
        screen.chart = chart
        screen.start(scene)
        defer { screen.stop() }
        // A spell is the app's work less the card's own time within it, so an app stall the card's timers fire after
        // still counts, and the card's work alone does not; times are given, as the run loop's observers give them
        let t = CACurrentMediaTime()
        screen.wake(at: t)
        screen.card(took: 0.02)
        screen.wait(at: t + 0.08)
        var spell = screen.takeMainThread()
        XCTAssertEqual(spell.longest, 0.06, accuracy: 1e-9)
        XCTAssertEqual(spell.long, 1)
        screen.wake(at: t + 0.1)
        screen.card(took: 0.03)
        screen.wait(at: t + 0.13)
        spell = screen.takeMainThread()
        XCTAssertEqual(spell.longest, 0, accuracy: 1e-9)
        XCTAssertEqual(spell.long, 0)
        // The card's changes of its own, while the app draws nothing, are not the app's frames
        for x in 1...4 {
            screen.card { dot.position.x = CGFloat(x) }
            screen.ownChange()
            run(0.12)
        }
        XCTAssertEqual(screen.health.snapshot().callbacks, 0)
        // Scrolling, each update is a frame with its work, and the frames of the last half second give the rate; a frame
        // of the screen's bounds no frame time
        scroll.setContentOffset(CGPoint(x: 0, y: 3000), animated: true)
        run(0.6)
        let snapshot = screen.health.snapshot()
        XCTAssertGreaterThan(snapshot.callbacks, 10)
        XCTAssertEqual(snapshot.worked, snapshot.callbacks, "each frame's commit is its work")
        XCTAssertEqual(snapshot.intervals, 0)
        XCTAssertTrue(chart.columns.contains { $0.fps >= 20 && $0.budget > 0 })
        XCTAssertFalse(chart.columns.contains { $0.frame > 0 }, "the charts plot no frame time")
        _ = screen.takeMainThread()
        // A stall within a scroll is the main thread's long spell, and the charts' work
        run(0.3)
        scroll.setContentOffset(.zero, animated: true)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { usleep(60_000) }
        run(0.6)
        let main = screen.takeMainThread()
        XCTAssertGreaterThanOrEqual(main.longest, 0.06)
        XCTAssertGreaterThanOrEqual(main.long, 1)
        XCTAssertTrue(chart.columns.contains { $0.work >= 60 }, "the stall is the work chart's")
        // Stopped, it observes nothing
        screen.stop()
        let callbacks = screen.health.snapshot().callbacks
        scroll.setContentOffset(CGPoint(x: 0, y: 3000), animated: true)
        run(0.4)
        XCTAssertEqual(screen.health.snapshot().callbacks, callbacks)
    }

    func testADragGoesPastTheEdgesAndTheCardSpringsBackToAScreenCorner() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let overlay = FrameHealthOverlay(health: health)
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "the card in a real host window, whose corners it follows")
        }
        let window = hosted(overlay, in: scene, size: .zero)
        overlay.frame = window.bounds
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        XCTAssertEqual(overlay.card.frame.origin, CGPoint(x: 8, y: 8), "a new card rests in the top-left corner, past the safe area")
        overlay.card.moved?(CGPoint(x: -92, y: 8))
        XCTAssertLessThan(overlay.card.frame.minX, 8, "a drag carries the card past the edge")
        XCTAssertGreaterThan(overlay.card.frame.minX, -92, "but not as far as the press")
        overlay.card.released?(.zero)
        XCTAssertEqual(overlay.card.frame.origin, CGPoint(x: 8, y: 8), "on release it springs back to its corner")
        if #available(iOS 26.0, *) {
            // In a corner of a rounded screen the card's corner follows the screen's (54 points on an iPhone 18
            // Pro); a square screen corner, as the folded iPhone Duo's top-left, leaves the minimum
            XCTAssertEqual(overlay.card.cornerConfiguration, .corners(radius: .containerConcentric(minimum: 12)))
            overlay.card.layoutIfNeeded()
            XCTAssertEqual(overlay.card.effectiveRadius(corner: .bottomRight), 12, accuracy: 0.5)
            XCTAssertGreaterThanOrEqual(overlay.card.effectiveRadius(corner: .topLeft), 12)
        }
    }

    func testTheCardsHairlineAndHeaderKeepToItsRoundedCorners() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("a card's corners follow the screen's from iOS 26") }
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let overlay = FrameHealthOverlay(health: health)
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "the card in a real host window, drawn as the screen shows it")
        }
        let window = hosted(overlay, in: scene, size: .zero)
        window.overrideUserInterfaceStyle = .dark
        window.rootViewController?.view.backgroundColor = .black
        overlay.frame = window.bounds
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        // Beside the card, two views of its corners and hairline, the hairline their own layer's border: one that
        // draws nothing, whose hairline is whole, and one that draws, as the card did
        let card = overlay.card, below = card.frame.maxY + 8
        let whole = bordered(UIView(frame: CGRect(x: 8, y: below, width: 120, height: 120)))
        let fading = bordered(DrawsBlack(frame: CGRect(x: 136, y: below, width: 120, height: 120)))
        let scale = window.traitCollection.displayScale
        for view in [whole, fading] {
            window.rootViewController?.view.addSubview(view)
            view.layer.borderWidth = 1 / scale
        }
        window.layoutIfNeeded()
        XCTAssertTrue(window.bounds.contains(fading.frame), "both fit under the card")
        XCTAssertEqual(card.effectiveRadius(corner: .bottomRight), whole.effectiveRadius(corner: .bottomRight), accuracy: 0.5)
        // Captured only across the three bottom-right corners
        let region = CGRect(x: 0, y: card.frame.maxY - 16, width: window.bounds.width, height: whole.frame.maxY - card.frame.maxY + 20)
        let image = try XCTUnwrap(UIGraphicsImageRenderer(bounds: region).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }.cgImage)
        var pixels = [UInt8](repeating: 0, count: image.width * image.height * 4)
        let ctx = try XCTUnwrap(CGContext(data: &pixels, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        // The hairline's light summed across the bottom-right corner's diagonal, on black: a sum, not the brightest
        // pixel, since a card that grows with the text size can end between pixels, which share its hairline
        func hairline(_ view: UIView) -> Double {
            let r = view.effectiveRadius(corner: .bottomRight), frame = view.frame
            return stride(from: 0.5 * r, to: 1.6 * r, by: 0.25 / scale).reduce(0) { sum, s in
                let x = Int((frame.maxX - r + s / 2.squareRoot() - region.minX) * scale)
                let y = Int((frame.maxY - r + s / 2.squareRoot() - region.minY) * scale)
                guard x >= 0, y >= 0, x < image.width, y < image.height else { return sum }
                let i = (y * image.width + x) * 4
                return sum + Double(Int(pixels[i]) + Int(pixels[i + 1]) + Int(pixels[i + 2])) / 3 / 4
            }
        }
        let (drawn, reference, faded) = (hairline(card), hairline(whole), hairline(fading))
        let figures = "card \(Int(drawn)), a view that draws nothing \(Int(reference)), one that draws \(Int(faded))"
        XCTAssertGreaterThan(reference, 20, "the hairline shows: \(figures)")
        XCTAssertGreaterThanOrEqual(drawn, 0.9 * reference, "the card's is as strong around the corner: \(figures)")
        // Where this fails, this runtime keeps the border of a layer that draws, and the card needs no content view
        XCTAssertLessThan(faded, 0.4 * reference, "a layer that draws and clips fades its own border here: \(figures)")

        // Dragged right and released, the card rests in the screen's top-right corner; where that corner rounds, Share
        // moves in and Close, whose corner stays 12 points, keeps its place
        card.moved?(CGPoint(x: overlay.bounds.maxX, y: 8))
        card.released?(.zero)
        overlay.layoutIfNeeded()
        card.layoutIfNeeded()
        let radius = card.effectiveRadius(corner: .topRight)
        guard radius > 12.5 else { return }
        let moved = card.layout, square = FrameHealthCardFace.Layout(width: moved.width)
        XCTAssertEqual(moved.corners.topRight, (radius / card.scale).rounded())
        XCTAssertEqual(moved.corners.topLeft, 12, "the card's top-left corner is away from the screen's")
        XCTAssertLessThan(moved.share.maxX, square.share.maxX, "Share moves in from the rounder corner")
        XCTAssertEqual(moved.close, square.close)
    }

    func testTheCardFollowsDarkModeWithoutBeingMoved() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "a real host window whose appearance changes")
        }
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let overlay = FrameHealthOverlay(health: health)
        let window = hosted(overlay, in: scene, size: CGSize(width: 430, height: 900))
        window.overrideUserInterfaceStyle = .light
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        // The card's ground, systemBackground, is white in light mode and black in dark
        let dark = { (overlay.card.palette.background.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .defaultIntent,
            options: nil)?.components?.first ?? 1) < 0.5 }
        XCTAssertFalse(dark())
        // Only the appearance changes: nothing lays the card out or moves it
        window.overrideUserInterfaceStyle = .dark
        let followed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in dark() }, object: nil)
        wait(for: [followed], timeout: 2)
        XCTAssertEqual(overlay.card.backgroundColor?.cgColor, overlay.card.palette.background, "the card's ground follows too")
    }

    func testTheMenusOneFrameHealthRowIsDimmedAndExplainsWhereTheCardCannotRead() {
        let on = FrameHealth(enabled: true), off = FrameHealth(enabled: false)
        defer { on.destroy(); off.destroy() }
        let available = DevMenuController.frameHealth(on, container: true)
        XCTAssertEqual(available.title, "Frame Health")
        XCTAssertNil(available.unavailable)
        XCTAssertFalse(available.checked, "the card starts hidden")
        let disabled = DevMenuController.frameHealth(off, container: true)
        XCTAssertEqual(disabled.title, "Frame Health Disabled", "the same one row, dimmed")
        XCTAssertEqual(disabled.unavailable?.title, "Frame Health Is Off")
        XCTAssertTrue(disabled.unavailable?.message.hasPrefix("This app turns frame health off") == true, "it says what turned it off")
        XCTAssertEqual(DevMenuController.frameHealth(on, container: false).unavailable?.title, "Frame Health Is Unavailable")
        // A production build's collector, which collects only while the card shows, offers the row as available
        let demand = FrameHealth(policy: .onDemand)
        defer { demand.destroy() }
        XCTAssertEqual(DevMenuController.frameHealth(demand, container: true).title, "Frame Health")
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
        let frameHealth = try XCTUnwrap(menu.groups[1].first)
        XCTAssertEqual(frameHealth.symbol, "waveform.path.ecg")
        let content = try XCTUnwrap(menu.tableView.cellForRow(at: IndexPath(row: 0, section: 1))?.contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(content.text, frameHealth.title)
        XCTAssertNil(content.secondaryText, "the row has no subtext")
        XCTAssertFalse(menu.groups.flatMap { $0 }.contains { $0.title == "Copy Info" })
        menu.tableView(menu.tableView, didSelectRowAt: IndexPath(row: 0, section: 2))
        XCTAssertTrue(navigation.viewControllers.last?.view is UITextView)
        XCTAssertEqual(navigation.viewControllers.last?.title, "Build Information")
    }
}

/// `view` on black with the card's corners and, as its own layer's border, the card's hairline color; the caller
/// sets the width once the view is in its window
@available(iOS 26.0, *)
private func bordered<View: UIView>(_ view: View) -> View {
    view.cornerConfiguration = .corners(radius: .containerConcentric(minimum: 12))
    view.layer.masksToBounds = true
    view.backgroundColor = .black
    view.layer.borderColor = UIColor.separator.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)).cgColor
    return view
}

/// A view that draws its bounds black, so its layer has drawn contents, as the card's did
private final class DrawsBlack: UIView {
    override func draw(_ rect: CGRect) {
        UIGraphicsGetCurrentContext()?.setFillColor(UIColor.black.cgColor)
        UIGraphicsGetCurrentContext()?.fill(bounds)
    }
}
#endif

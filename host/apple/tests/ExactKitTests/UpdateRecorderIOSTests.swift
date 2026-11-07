#if os(iOS) && !targetEnvironment(macCatalyst)
import ExactCore
import QuartzCore
import UIKit
import XCTest
@testable import ExactKit

/// A recording on iOS 18 and later (`UpdateRecorder`): each UI update UIKit
/// makes, observed passively and timed by its phases, with the presentation
/// probe hearing when its frames reached the screen.
final class UpdateRecorderIOSTests: XCTestCase {
    /// A recording keeps each UI update as a frame, timed by its phases,
    /// with the recorder's own time and a probe drawable presented in each,
    /// and asks for no update or rate of its own; the simulator's Metal
    /// reports no presentation.
    func testARecordingKeepsEachUIUpdateTimedByItsPhases() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard hostedWindowScene() != nil else { throw missing("scene", "a UI update link observes a window scene's updates") }
        let (session, window) = try windowed()
        defer { _ = session.sampler?.endRecording(); session.destroy(); window.isHidden = true }
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let recorder = try XCTUnwrap(sampler.recorder as? UpdateRecorder, "a view in a window scene records its UI updates")
        XCTAssertFalse(recorder.link.requiresContinuousUpdates)
        XCTAssertEqual(recorder.link.preferredFrameRateRange, .default)
        let recording = try XCTUnwrap(sampler.recording)
        // The sampler's own display link, which a development build runs, makes updates of its own
        sampler.stop()
        let driver = updates(session.presenter.viewport)
        until("updates are kept") { recording.records.count >= 6 }
        driver.isEnabled = false
        let period = 1000 / Double(window.screen.maximumFramesPerSecond)
        for r in recording.records.suffix(4) {
            let u = try XCTUnwrap(r.update)
            XCTAssertEqual(r.period, (period * 100).rounded() / 100)
            // A simulator's display may run slower than the fastest rate its device has
            let periods = (r.interval / r.period).rounded()
            XCTAssertEqual(r.interval, periods * r.period, accuracy: 0.5, "a whole number of periods")
            XCTAssertEqual(r.missed, Int(periods) - 1)
            XCTAssertGreaterThanOrEqual(u.work + 0.05, (r.commit ?? 0) + u.events + u.links, "its work holds its phases, each rounded to 0.01 ms")
            XCTAssertGreaterThan(u.own, 0)
            XCTAssertNotNil(r.main)
        }
        XCTAssertGreaterThanOrEqual(try XCTUnwrap(recorder.probe).presentations, 4)
        #if targetEnvironment(simulator)
        XCTAssertEqual(recording.records.filter(\.probed).count, 0, "the simulator's Metal reports nothing")
        #endif
    }

    /// In a production bake a passive recording starts no display link of
    /// the sampler's, whose callbacks would make UI updates of their own.
    func testAProductionBakesRecordingRunsNoDisplayLink() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard hostedWindowScene() != nil else { throw missing("scene", "a UI update link observes a window scene's updates") }
        let measured = FrameSampler.measured
        FrameSampler.measured = false
        defer { FrameSampler.measured = measured }
        let (session, window) = try windowed()
        defer { _ = session.sampler?.endRecording(); session.destroy(); window.isHidden = true }
        session.sampler?.stop()
        session.sampler = FrameSampler(session: session)
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        XCTAssertNotNil(sampler.recorder)
        sampler.batch((1, 2), ms: 0.5)
        XCTAssertTrue(sampler.turnObservers.isEmpty, "a batch opens no segment")
    }

    /// An update whose display-link callbacks run past its completion
    /// deadline commits late, and its frame's cause is its update's work.
    func testAnUpdatesWorkPastItsDeadlineIsItsCause() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard hostedWindowScene() != nil else { throw missing("scene", "a UI update link observes a window scene's updates") }
        let (session, window) = try windowed()
        defer { _ = session.sampler?.endRecording(); session.destroy(); window.isHidden = true }
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let recording = try XCTUnwrap(sampler.recording)
        sampler.stop()
        let view = session.presenter.viewport
        var stall = false
        let ticker = Ticker {
            guard stall, let info = UIUpdateInfo.current(for: view) else { return }
            stall = false
            Thread.sleep(forTimeInterval: max(0, info.completionDeadlineTime - CACurrentMediaTime()) + 0.012)
        }
        let link = CADisplayLink(target: ticker, selector: #selector(Ticker.tick))
        link.add(to: .main, forMode: .common)
        defer { link.invalidate() }
        until("updates are kept") { recording.records.count >= 4 }
        stall = true
        until("the stalled update is kept") { !stall && recording.records.contains { $0.overrun > 8 } }
        let late = try XCTUnwrap(recording.records.first { $0.overrun > 8 })
        XCTAssertGreaterThan(try XCTUnwrap(late.update).links, 10, "the callbacks ran long")
        let report = FrameSampler.report(recording, identity: [:])
        let frames = try XCTUnwrap(report["late"] as? [[String: Any]])
        XCTAssertEqual(frames.first { ($0["overrun"] as? Double ?? 0) > 8 }?["cause"] as? String, "update")
    }

    /// A main thread busy between updates delays the next one: that frame
    /// is late, and its cause is the main thread.
    func testAMainThreadBusyBeforeAnUpdateIsItsCause() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard hostedWindowScene() != nil else { throw missing("scene", "a UI update link observes a window scene's updates") }
        let (session, window) = try windowed()
        defer { _ = session.sampler?.endRecording(); session.destroy(); window.isHidden = true }
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let recording = try XCTUnwrap(sampler.recording)
        sampler.stop()
        let driver = updates(session.presenter.viewport)
        defer { driver.isEnabled = false }
        until("updates are kept") { recording.records.count >= 4 }
        var blocked = false
        DispatchQueue.main.async { Thread.sleep(forTimeInterval: 0.03); blocked = true }
        let before = recording.records.count
        until("updates after the block are kept") { blocked && recording.records.count > before + 2 }
        let report = FrameSampler.report(recording, identity: [:])
        let causes = try XCTUnwrap(report["late"] as? [[String: Any]]).compactMap { $0["cause"] as? String }
        XCTAssertTrue(causes.contains("main"), "\(causes)")
        XCTAssertFalse(causes.contains("rate"), "the main thread was busy through the gap")
    }

    /// A gap in which nothing the recording follows moved and the main
    /// thread was idle owed no frame: quiet, and not late.
    func testAGapAtRestIsQuiet() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard hostedWindowScene() != nil else { throw missing("scene", "a UI update link observes a window scene's updates") }
        let (session, window) = try windowed()
        defer { _ = session.sampler?.endRecording(); session.destroy(); window.isHidden = true }
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let recording = try XCTUnwrap(sampler.recording)
        sampler.stop()
        let view = try XCTUnwrap(window.rootViewController?.view)
        for color in [UIColor.red, .blue, .green] {
            sampler.batch((1, 1), ms: 0.1)
            view.backgroundColor = color
            let wait = Date().addingTimeInterval(0.2)
            while Date() < wait { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        }
        let gaps = recording.records.filter { $0.missed > 0 }
        XCTAssertFalse(gaps.isEmpty, "the updates are far apart")
        XCTAssertTrue(gaps.allSatisfy(\.quiet))
        XCTAssertEqual((FrameSampler.report(recording, identity: [:])["recording"] as? [String: Any])?["lateFrames"] as? Int, 0)
    }

    func testAFirstBatchOrScrollAfterRestDoesNotOweTheIdleGap() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("UIUpdateLink requires iOS 18") }
        let period = 1000.0 / 120
        XCTAssertTrue(UpdateRecorder.quietGap(previous: 10, expected: 11, period: period, main: 2,
                                              batchAt: 10.99, continuingScroll: false), "a reply after rest starts work when it arrives")
        XCTAssertTrue(UpdateRecorder.quietGap(previous: 10, expected: 11, period: period, main: 2,
                                              batchAt: nil, continuingScroll: false), "a newly followed scroll owes no earlier frame")
        XCTAssertFalse(UpdateRecorder.quietGap(previous: 10, expected: 11, period: period, main: 995,
                                               batchAt: 10.99, continuingScroll: false), "a busy main thread delayed an update")
        XCTAssertFalse(UpdateRecorder.quietGap(previous: 10, expected: 11, period: period, main: 2,
                                               batchAt: 10.001, continuingScroll: false), "work pending throughout the gap was owed")
        XCTAssertFalse(UpdateRecorder.quietGap(previous: 10, expected: 11, period: period, main: 2,
                                               batchAt: nil, continuingScroll: true), "an ongoing glide still owes its frames")
    }

    func testInactivityAndWindowRemovalResetTheRecordingBaselineAndProbe() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("UIUpdateLink requires iOS 18") }
        guard hostedWindowScene() != nil else { throw missing("scene", "recording lifecycle follows its window") }
        let (session, window) = try windowed()
        defer { _ = session.sampler?.endRecording(); session.destroy(); window.isHidden = true }
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        sampler.stop()
        let recorder = try XCTUnwrap(sampler.recorder as? UpdateRecorder)
        let view = session.presenter.viewport
        let driver = updates(view)
        defer { driver.isEnabled = false }
        until("the recorder has a timing baseline and probe") { recorder.presented != nil && recorder.probe != nil }
        let center = NotificationCenter.default
        center.post(name: UIApplication.willResignActiveNotification, object: nil)
        XCTAssertFalse(recorder.link.isEnabled)
        XCTAssertNil(recorder.presented)
        XCTAssertNil(recorder.probe)
        sampler.batch((10, 10), ms: 0.1)
        center.post(name: UIApplication.didBecomeActiveNotification, object: nil)
        XCTAssertTrue(recorder.link.isEnabled)
        XCTAssertNil(recorder.presented, "the first foreground update establishes a new baseline")
        until("the resumed recording has a baseline") { recorder.presented != nil && recorder.probe != nil }
        view.removeFromSuperview()
        XCTAssertFalse(recorder.link.isEnabled)
        XCTAssertNil(recorder.presented)
        XCTAssertNil(recorder.probe)
        try XCTUnwrap(window.rootViewController?.view).addSubview(view)
        XCTAssertTrue(recorder.link.isEnabled)
        XCTAssertNil(recorder.presented, "a new window establishes its own baseline")
    }

    /// The probe's reports reach the frames whose updates presented their
    /// drawables, in whatever order they come: on time, late or never shown.
    /// On the simulator, whose Metal reports nothing, the test stands in for it.
    func testTheProbesReportsReachTheFramesThatPresentedTheirDrawables() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard hostedWindowScene() != nil else { throw missing("scene", "the presentation probe presents in a window scene's window") }
        #if !targetEnvironment(simulator)
        throw XCTSkip("n/a: a device's Metal reports for itself")
        #else
        let (session, window) = try windowed()
        defer { _ = session.sampler?.endRecording(); session.destroy(); window.isHidden = true }
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let recorder = try XCTUnwrap(sampler.recorder as? UpdateRecorder), recording = recorder.recording
        sampler.stop()
        let driver = updates(session.presenter.viewport)
        defer { driver.isEnabled = false }
        until("frames wait for their drawables' reports") { recording.awaiting.count >= 3 }
        let probe = try XCTUnwrap(recorder.probe)
        let waits = recording.awaiting.sorted { $0.key < $1.key }.prefix(3).map { (number: $0.key, frame: $0.value.frame, expected: $0.value.expected) }
        let period = recording.records[waits[1].frame].period
        probe.heard(waits[2].number, at: 0)
        probe.heard(waits[1].number, at: waits[1].expected + 2 * period / 1000)
        probe.heard(waits[0].number, at: waits[0].expected)
        let frames = waits.map(\.frame)
        until("the next update takes the reports in") { frames.allSatisfy { recording.records[$0].probed } }
        XCTAssertNil(recording.records[frames[0]].screenLate)
        XCTAssertEqual(try XCTUnwrap(recording.records[frames[1]].screenLate), 2 * period, accuracy: 0.01)
        XCTAssertEqual(frames.map { recording.records[$0].unshown }, [false, false, true])
        XCTAssertEqual(recording.records.filter(\.probed).count, 3, "and no other frame")
        let report = FrameSampler.report(recording, identity: [:])
        let causes = try XCTUnwrap(report["late"] as? [[String: Any]]).compactMap { $0["cause"] as? String }
        XCTAssertEqual(causes.filter { $0 == "render" }.count, 2, "the late and the unshown frame")
        #endif
    }

    /// A recording whose view moves into another window takes its probe
    /// there at its next update; the waits for the first probe's drawables go.
    func testTheProbeFollowsItsViewIntoAnotherWindow() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard let scene = hostedWindowScene() else { throw missing("scene", "the presentation probe presents in a window scene's window") }
        let (session, window) = try windowed()
        let other = UIWindow(windowScene: scene)
        other.frame = window.frame
        other.rootViewController = UIViewController()
        other.makeKeyAndVisible()
        defer { _ = session.sampler?.endRecording(); session.destroy(); window.isHidden = true; other.isHidden = true }
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let recorder = try XCTUnwrap(sampler.recorder as? UpdateRecorder)
        sampler.stop()
        let driver = updates(session.presenter.viewport)
        defer { driver.isEnabled = false }
        until("the probe presents") { (recorder.probe?.presentations ?? 0) >= 2 }
        let first = try XCTUnwrap(recorder.probe)
        recorder.recording.awaiting[Int.max] = (0, 0)
        other.rootViewController?.view.addSubview(session.presenter.viewport)
        until("the probe is in the view's new window") { recorder.probe.map { $0 !== first && $0.isIn(other) } ?? false }
        XCTAssertFalse(first.isInstalled, "and gone from the old one")
        XCTAssertNil(recorder.recording.awaiting[Int.max], "with the waits for its drawables")
    }

    /// The recorder and its probe go with the recording's end, and with its
    /// session, but not when the sampler stops on its own.
    func testTheRecorderGoesWithItsRecordingAndItsSession() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard hostedWindowScene() != nil else { throw missing("scene", "a UI update link observes a window scene's updates") }
        let (session, window) = try windowed()
        defer { session.destroy(); window.isHidden = true }
        let sampler = try XCTUnwrap(session.sampler)
        let driver = updates(session.presenter.viewport)
        defer { driver.isEnabled = false }
        XCTAssertTrue(sampler.beginRecording())
        let first = try XCTUnwrap(sampler.recorder as? UpdateRecorder)
        until("the probe presents") { first.probe?.isInstalled == true }
        sampler.stop()
        XCTAssertTrue(first.probe?.isInstalled == true, "a quiet stop leaves the recording running")
        let probe = try XCTUnwrap(first.probe)
        XCTAssertNotNil(sampler.endRecording())
        XCTAssertNil(sampler.recorder)
        XCTAssertFalse(probe.isInstalled, "the recording's end removes its probe")
        XCTAssertTrue(sampler.beginRecording())
        let second = try XCTUnwrap(sampler.recorder as? UpdateRecorder)
        until("the second probe presents") { second.probe?.isInstalled == true }
        let secondProbe = try XCTUnwrap(second.probe)
        session.destroy()
        XCTAssertFalse(secondProbe.isInstalled, "a destroyed session's recorder stops")
    }

    /// The render inventory names what costs the render server a pass of
    /// its own: a blur's backdrop and a shadow without a path in a scroller's
    /// content, at their content offsets, and a blur outside it in the
    /// window's; a hidden layer is left out.
    func testTheRenderInventoryNamesWhatCostsTheRenderServer() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "the inventory reads a window's layers") }
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        let root = try XCTUnwrap(window.rootViewController?.view)
        let scroll = UIScrollView(frame: root.bounds)
        scroll.contentSize = CGSize(width: 390, height: 3000)
        root.addSubview(scroll)
        let blur = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        blur.frame = CGRect(x: 0, y: 1200, width: 390, height: 80)
        scroll.addSubview(blur)
        let card = UIView(frame: CGRect(x: 20, y: 400, width: 350, height: 100))
        card.layer.shadowOpacity = 0.2
        scroll.addSubview(card)
        let hidden = UIView(frame: CGRect(x: 20, y: 600, width: 350, height: 100))
        hidden.layer.shadowOpacity = 0.2
        hidden.isHidden = true
        scroll.addSubview(hidden)
        let bar = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        bar.frame = CGRect(x: 0, y: 760, width: 390, height: 84)
        root.addSubview(bar)
        scroll.contentOffset.y = 500
        window.layoutIfNeeded()
        let inventory = FrameSampler.renderInventory(in: window, scroller: scroll)
        let rows = try XCTUnwrap(inventory["rows"] as? [[Any]])
        func row(_ kind: String, _ place: String) -> [Any]? { rows.first { $0[0] as? String == kind && $0[1] as? String == place } }
        XCTAssertEqual(try XCTUnwrap(row("backdrop", "content"))[3] as? Double, 1200, "the scrolled blur at its content offset")
        XCTAssertEqual(try XCTUnwrap(row("shadow", "content"))[3] as? Double, 400, "the shadow without a path")
        XCTAssertEqual(try XCTUnwrap(row("backdrop", "fixed"))[3] as? Double, 760, "the bar's blur in the window")
        XCTAssertEqual(rows.filter { $0[0] as? String == "shadow" }.count, 1, "not the hidden one")
        XCTAssertGreaterThan(inventory["layers"] as? Int ?? 0, 5)
        // A kind on more than four nodes in one place is one row of their span and count; four keep theirs
        let masks = (0..<6).map { FrameSampler.RenderRow(kind: "mask", place: "content", node: "text-\($0)", top: 1500 + Double($0) * 40, bottom: 1520 + Double($0) * 40, count: 1) }
        let shadows = (0..<4).map { FrameSampler.RenderRow(kind: "shadow", place: "content", node: "card-\($0)", top: Double($0) * 100, bottom: Double($0) * 100 + 80, count: 2) }
        let rowsOut = FrameSampler.collapsed(masks + shadows)
        XCTAssertEqual(rowsOut.filter { $0.kind == "shadow" }.count, 4)
        let mask = try XCTUnwrap(rowsOut.first { $0.kind == "mask" })
        XCTAssertEqual(rowsOut.filter { $0.kind == "mask" }.count, 1)
        XCTAssertEqual(mask.node, "6 nodes")
        XCTAssertEqual(mask.top, 1500)
        XCTAssertEqual(mask.bottom, 1720)
        XCTAssertEqual(mask.count, 6)
        XCTAssertEqual(rowsOut.map(\.top), rowsOut.map(\.top).sorted(), "down the scroller")
        let fixed = FrameSampler.RenderRow(kind: "backdrop", place: "fixed", node: "", top: 760, bottom: 844, count: 1)
        XCTAssertEqual(FrameSampler.collapsed(shadows + [fixed]).first?.place, "fixed", "what every frame shows first")
    }

    /// A session's viewport in a window of the hosted scene, and the window.
    private func windowed() throws -> (ExactSession, UIWindow) {
        let window = UIWindow(windowScene: try XCTUnwrap(hostedWindowScene()))
        window.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        let session = ExactApp.shared.makeSession()
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        session.presenter.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(session.presenter.viewport)
        return (session, window)
    }

    /// A link of the test's own that asks for an update every frame.
    @available(iOS 18.0, *)
    private func updates(_ view: UIView) -> UIUpdateLink {
        let link = UIUpdateLink(view: view)
        link.addAction(to: .afterUpdateComplete) { _, _ in }
        link.requiresContinuousUpdates = true
        link.isEnabled = true
        return link
    }

    /// Run the main run loop until `done`, for at most five seconds.
    private func until(_ what: String, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(5)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }
}

/// A display link's target that runs a closure at each callback.
private final class Ticker: NSObject {
    let body: () -> Void
    init(_ body: @escaping () -> Void) { self.body = body }
    @objc func tick() { body() }
}
#endif

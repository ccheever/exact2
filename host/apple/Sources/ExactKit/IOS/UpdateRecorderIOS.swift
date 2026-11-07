#if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
import ExactCore
import QuartzCore
import UIKit

/// A recording's frames on iOS and tvOS 18 and later: each UI update UIKit
/// makes for the session's view, observed by a passive `UIUpdateLink`. It
/// asks for no update and no frame rate of its own, so a scroll runs at the
/// rate the system gives it, as it does when nothing records, and a period at
/// rest has no frames. A frame's interval is the time between two updates'
/// expected presentations (`UIUpdateInfo.estimatedPresentationTime`) and its
/// period the display's fastest (`maximumFramesPerSecond`), so a lower rate
/// counts as missed frames; a gap in which nothing it follows moved and the
/// main thread was idle owed no frame (`quiet`).
///
/// Each update is timed by its phases (UIUpdateActionPhase.h): `start`, from
/// its `modelTime` to its event dispatch, which work the main thread was
/// busy with before it delays; its events; its display links' callbacks; and
/// its `work`, from event dispatch to the end of its Core Animation commit,
/// which UIKit judges against the completion deadline. A presentation probe
/// (`PresentationProbe`) hears when each frame reached the screen. The
/// recorder times its own main-thread work in each update (`own`).
@available(iOS 18.0, tvOS 18.0, *)
final class UpdateRecorder {
    let recording: FrameSampler.Recording
    private weak var session: ExactSession?
    let link: UIUpdateLink
    private(set) var probe: PresentationProbe?
    private var turns: [CFRunLoopObserver] = []
    private var notifications: [NSObjectProtocol] = []
    private let windowWatch = WindowWatch()
    private var active = UIApplication.shared.applicationState == .active
    /// The main thread's run-loop time since the last update's commit, and
    /// when the turn now running began
    private var busy = 0.0, turnBegan: CFTimeInterval?
    /// The update now running: when its phases began and ended, the
    /// recorder's own time in it, and the probe's drawable it presented
    private var began = 0.0, eventsEnded = 0.0, linksBegan = 0.0, linksEnded = 0.0, commitBegan = 0.0, own = 0.0
    private var drawable: Int?
    /// The batches applied since the last update, as the sampler's display link takes them
    private var pending: (first: UInt64?, last: UInt64?, batches: Int, apply: Double, at: CFTimeInterval?) = (nil, nil, 0, 0, nil)
    /// The last update's expected presentation and the end of its commit,
    /// and what the recording followed then
    private(set) var presented: CFTimeInterval?
    private var concluded: CFTimeInterval?
    private var followed: (offset: Double, phase: String, taken: Int)?

    init?(session: ExactSession, recording: FrameSampler.Recording) {
        let view = session.presenter.viewport
        guard view.window?.windowScene != nil else { return nil }
        self.session = session
        self.recording = recording
        link = UIUpdateLink(view: view)
        link.addAction(to: .beforeEventDispatch) { [weak self] _, _ in self?.mark { $0.began = $1 } }
        link.addAction(to: .afterEventDispatch) { [weak self] _, _ in self?.mark { $0.eventsEnded = $1 } }
        link.addAction(to: .beforeCADisplayLinkDispatch) { [weak self] _, _ in self?.mark { $0.linksBegan = $1 } }
        link.addAction(to: .afterCADisplayLinkDispatch) { [weak self] _, _ in self?.mark { $0.linksEnded = $1 } }
        link.addAction(to: .beforeCATransactionCommit) { [weak self] _, _ in
            self?.mark { recorder, _ in
                recorder.followProbe()
                recorder.drawable = recorder.probe?.present()
                recorder.commitBegan = CACurrentMediaTime()
            }
        }
        // It counts its own time into the frame it keeps
        link.addAction(to: .afterCATransactionCommit) { [weak self] _, info in self?.committed(at: CACurrentMediaTime(), info) }
        windowWatch.owner = self
        windowWatch.isUserInteractionEnabled = false
        view.addSubview(windowWatch)
        let center = NotificationCenter.default
        notifications.append(center.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
            self?.active = false
            self?.windowChanged()
        })
        notifications.append(center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            self?.active = true
            self?.windowChanged()
        })
        watchTurns()
    }

    /// Run one phase's step at the time it was called, counting what it took as the recorder's own.
    private func mark(_ step: (UpdateRecorder, CFTimeInterval) -> Void) {
        guard link.isEnabled else { return }
        let now = CACurrentMediaTime()
        step(self, now)
        own += CACurrentMediaTime() - now
    }

    /// A batch the session applied before this update's commit.
    func batch(_ seq: (UInt64, UInt64)?, ms: Double) {
        if let seq { pending.first = min(pending.first ?? seq.0, seq.0); pending.last = max(pending.last ?? seq.1, seq.1) }
        pending.batches += 1
        pending.apply += ms
        pending.at = pending.at ?? CACurrentMediaTime()
    }

    func stop() {
        link.isEnabled = false
        for o in turns { CFRunLoopRemoveObserver(CFRunLoopGetMain(), o, .commonModes) }
        turns = []
        for notification in notifications { NotificationCenter.default.removeObserver(notification) }
        notifications = []
        windowWatch.owner = nil
        windowWatch.removeFromSuperview()
        endProbe()
    }

    deinit { stop() }

    private func windowChanged() {
        link.isEnabled = active && windowWatch.window != nil
        presented = nil; concluded = nil; followed = nil
        busy = 0; turnBegan = nil
        began = 0; own = 0; drawable = nil
        pending = (nil, nil, 0, 0, nil)
        endProbe()
    }

    private final class WindowWatch: UIView {
        weak var owner: UpdateRecorder?
        override func didMoveToWindow() {
            super.didMoveToWindow()
            owner?.windowChanged()
        }
    }

    /// A batch arriving after rest starts work at its own time. A continuing
    /// drag or glide still owes the whole gap, as does a busy main thread.
    static func quietGap(previous: CFTimeInterval, expected: CFTimeInterval, period: Double, main: Double,
                         batchAt: CFTimeInterval?, continuingScroll: Bool) -> Bool {
        let interval = (expected - previous) * 1000
        return interval > period * 1.5 && main < interval - period && !continuingScroll
            && (batchAt.map { $0 > previous + period / 1000 } ?? true)
    }

    /// The main thread's turns, each timed from the run loop waking to it
    /// going back to sleep: a frame's `main`.
    private func watchTurns() {
        turnBegan = CACurrentMediaTime()
        let woke = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.afterWaiting.rawValue, true, 0) { [weak self] _, _ in
            self?.turnBegan = CACurrentMediaTime()
        }
        let sleeps = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, true, CFIndex.max) { [weak self] _, _ in
            guard let self, let began = turnBegan else { return }
            busy += CACurrentMediaTime() - began
            turnBegan = nil
        }
        for o in [woke, sleeps].compactMap({ $0 }) {
            CFRunLoopAddObserver(CFRunLoopGetMain(), o, .commonModes)
            turns.append(o)
        }
    }

    /// An update finished its commit at `now`: keep it as a frame, the first
    /// one only as the baseline of the next one's interval.
    private func committed(at now: CFTimeInterval, _ info: UIUpdateInfo) {
        guard link.isEnabled else { return }
        let expected = info.estimatedPresentationTime
        // The turn now running counts up to here, and its rest goes to the next frame
        let main = busy + (turnBegan.map { now - $0 } ?? 0)
        busy = 0
        if turnBegan != nil { turnBegan = now }
        let scroll = recording.scroller?.scrollSample, taken = recording.scrollers.count - 1
        let presentedBefore = presented, concludedBefore = concluded, followedBefore = followed
        let batches = pending
        presented = expected; concluded = now
        followed = scroll.map { ($0.0, $0.1, taken) }
        pending = (nil, nil, 0, 0, nil)
        defer { began = 0; own = 0; drawable = nil }
        guard let previous = presentedBefore, began > 0, let window = session?.presenter.viewport.window else { return }
        let r2 = { (x: Double) in (x * 100).rounded() / 100 }
        let period = 1000 / Double(max(1, window.screen.maximumFramesPerSecond))
        let interval = (expected - previous) * 1000
        var r = FrameSampler.Record(t: r2((expected - ExactEnv.t0) * 1000), interval: r2(interval), missed: max(0, Int((interval / period).rounded()) - 1),
                                    seq: batches.first.flatMap { first in batches.last.map { (first, $0) } }, batches: batches.batches, apply: r2(batches.apply))
        r.period = r2(period)
        r.overrun = r2(max(0, now - info.completionDeadlineTime) * 1000)
        r.ownOverrun = r.overrun
        r.commit = r2((now - commitBegan) * 1000)
        r.main = r2(main * 1000)
        if let scroll { r.scroll = (r2(scroll.0), scroll.1); r.taken = taken }
        let continuing = scroll.map { current in
            followedBefore.map { $0.taken == taken && $0.phase != "-" && current.1 != "-" } ?? false
        } ?? false
        r.quiet = Self.quietGap(previous: previous, expected: expected, period: period, main: main * 1000,
                               batchAt: batches.at, continuingScroll: continuing)
        r.update = (r2((began - info.modelTime) * 1000), r2((eventsEnded - began) * 1000), r2((linksEnded - linksBegan) * 1000),
                    r2((now - began) * 1000), r2((own + CACurrentMediaTime() - now) * 1000))
        if (r.missed > 0 && !r.quiet) || r.overrun > 0 {
            let from = concludedBefore ?? now - interval / 1000
            r.phases = HostPhases.shared.window(from: from, to: now)
            r.hatches = session?.hatchDiagnostics.window(from: from, to: now)
        }
        if recording.keep(r), let drawable { recording.awaiting[drawable] = (recording.records.count - 1, expected) }
    }

    /// The probe follows the session's view into the window it is in.
    private func followProbe() {
        guard let window = session?.presenter.viewport.window else { return }
        if let probe, !probe.isIn(window) { endProbe() }
        if probe == nil { probe = PresentationProbe(in: window) }
        drainProbe()
    }

    private func drainProbe() {
        for report in probe?.take() ?? [] {
            recording.shown(drawable: report.drawable, presented: report.presented) { [session] from, to in
                (HostPhases.shared.window(from: from, to: to), session?.hatchDiagnostics.window(from: from, to: to))
            }
        }
    }

    /// Remove the probe once its reports so far are in. The frames still
    /// waiting will hear nothing more, and a new probe numbers its drawables
    /// from 1 again.
    private func endProbe() {
        drainProbe()
        probe?.remove()
        probe = nil
        recording.awaiting.removeAll()
    }
}
#endif

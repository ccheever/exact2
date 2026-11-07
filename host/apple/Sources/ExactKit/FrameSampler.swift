import CExact
import Foundation
import OSLog
import QuartzCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// A session's display-link callbacks, the frames it judges presented (LLP
/// 1079 D3–D4): a display link of its own, run only inside active segments,
/// or throughout a recording an app
/// started (`startTrace_EXPERIMENTAL`). It watches; it never keeps the
/// session's frame source (`Frames`) running, so the measured app schedules
/// frames as the shipped one does. A production bake has one only while an
/// app records.
///
/// A segment starts on activity — a batch applied, the session's frame
/// source running — and ends 500 ms after the last, or when the app leaves
/// the foreground or its window is hidden. Its first callback is the
/// baseline, never a sample. The period is the link's own
/// `targetTimestamp − timestamp`, as the canvases read it (`duration` is
/// wrong under ProMotion), at the callback an interval starts from;
/// `missed = round(interval / period) − 1`.
final class FrameSampler: NSObject {
    /// Keep automatic sampling off for production bakes and power profiling
    /// (`EXACT_SAMPLE_FRAMES=0`); explicit recordings still work. Settable, so
    /// a test runs as a bake.
    static var measured = !ExactEnv.productionBake && ExactEnv.environment["EXACT_SAMPLE_FRAMES"] != "0"
    /// Whether it measures from boot: a development build, or one the agent
    /// launched. A release build starts at the first ask (`measurePerf`).
    static let atBoot = !ExactEnv.releaseBuild || ExactEnv.agentFreezes

    /// A new session's sampler, where it samples from boot. Where it starts
    /// at the first ask instead, so does the count of its runner's plan
    /// sites' work (LLP 1079 D1), which is told before the first runner
    /// boots.
    static func fromBoot(_ session: ExactSession, atBoot: Bool = atBoot) -> FrameSampler? {
        guard measured else { return nil }
        guard atBoot else { _ = workFromFirstAsk; return nil }
        return FrameSampler(session: session)
    }
    /// Told once a process: a later session must not undo an earlier ask,
    /// after which every runtime booted counts from boot.
    private static let workFromFirstAsk: Void = exact_measure_work_later()
    static let ring = 600, lateKept = 100, quiet = 0.5
    /// A release build's frames before anything asked for them.
    static let unmeasured = "a release build measures frames from the first agent request or trace; this is the first"

    struct Record {
        let t: Double, interval: Double, missed: Int, seq: (UInt64, UInt64)?, batches: Int, apply: Double
        var period = 0.0
        /// How late the frame's commit was, milliseconds (0: on time): on iOS
        /// and tvOS 18 and later, past its UI update's completion deadline
        /// (`committed`); elsewhere, how long past the frame's target the
        /// main thread's turn ended.
        var overrun = 0.0
        /// How long Core Animation's commit of the UI update whose lateness
        /// `overrun` holds took, ms, the layout and drawing of the layers that
        /// needed it included: on iOS and tvOS 18 and later, from the update
        /// link's `beforeCATransactionCommit` to its `afterCATransactionCommit`.
        var commit: Double?
        /// A late frame's hatch calls (LLP 1075.003.000.001 §3.1): those that
        /// overlapped its interval, each charged its overlap, with `coverage`.
        var hatches: [String: Any]?
        /// A late frame's main-thread host phases inside its interval, and
        /// what none of them took (LLP 1142 D1, `HostPhases.window`).
        var phases: [String: Any]?
        /// The tracked scroller's offset as the last turn committed it, and
        /// its phase: `t` dragged, `d` decelerating, `b` past its range,
        /// `-` at rest (LLP 1144 D1, `scrollSample`).
        var scroll: (Double, String)?
        /// In a recording: how long the main thread's turns that ended in
        /// this frame's interval ran, milliseconds.
        var main: Double?
        /// In a recording: which of its taken scrollers (`Recording.scrollers`)
        /// `scroll` was read from, as the turn committed it.
        var taken: Int?
        /// In a recording with a presentation probe: whether the probe
        /// reported this frame, how much later than UIKit expected it
        /// reached the screen when that was over half a period, ms, and
        /// whether it never did.
        var probed = false, screenLate: Double?, unshown = false
        /// How late the commit of the UI update this frame was sampled in
        /// was, ms, which ran after it: a stall the next callback's gap may
        /// hold again. `overrun` also holds a commit from an update that
        /// sampled no frame.
        var ownOverrun = 0.0
        /// A passive recording's UI update (`UpdateRecorder`), ms: from its
        /// `modelTime` to its event dispatch, its events, its display links'
        /// callbacks, its work from event dispatch to the end of its commit,
        /// and the recorder's own main-thread time in it.
        var update: (start: Double, events: Double, links: Double, work: Double, own: Double)?
        /// A passive recording's idle gap before new work, which owed no frame.
        var quiet = false
        var json: [String: Any] {
            var o: [String: Any] = ["t": t, "target": t + period, "period": period, "interval": interval, "missed": missed, "batches": batches, "apply": apply, "overrun": overrun]
            o["seq"] = seq.map { [$0.0, $0.1] } ?? NSNull()
            if let commit { o["commit"] = commit }
            if let main { o["main"] = main }
            if let screenLate { o["screenLate"] = screenLate }
            if unshown { o["unshown"] = true }
            if quiet { o["quiet"] = true }
            if let update { o["start"] = update.start; o["events"] = update.events; o["links"] = update.links; o["work"] = update.work; o["own"] = update.own }
            if let scroll { o["scroll"] = [scroll.0, scroll.1] }
            if let hatches { o.merge(hatches) { a, _ in a } }
            if let phases {
                o["phases"] = phases["phases"]; o["unattributed"] = phases["unattributed"]
                if let partial = phases["coverage"] { o["phasesCoverage"] = partial }
            }
            return o
        }
    }

    weak var session: ExactSession?
    private var link: CADisplayLink?
    private var last: CFTimeInterval?
    private var active = -Double.infinity
    private var observing = false
    var observationStopped: ((String) -> Void)?
    /// `perf frames hold` (LLP 1144 D1): the segment stays open with no batch,
    /// so a list built up front is sampled as it glides; released by `perf
    /// frames release`, or by anything that stops sampling.
    var held = false {
        didSet {
            if held { activity() } else { tracked = nil }
            #if os(macOS)
            // A held fling on the Mac is a person's, on the trackpad: AppKit scrolls it.
            if !ExactEnv.agentFreezes { ChainingScrollView.appliesAtOnce = Self.wheelAppliesAtOnce(agentMode: ExactEnv.agentMode, held: held) }
            #endif
        }
    }
    #if os(macOS)
    static func wheelAppliesAtOnce(agentMode: Bool, held: Bool) -> Bool { agentMode && !held }
    #endif
    /// The scroller a held fling watches (`perf frames hold` with a target):
    /// its offset and phase, read where each turn ends, after its commit.
    var tracked: (() -> (Double, String)?)?
    private var committed: (Double, String)?
    private var period: Double?
    private var records: [Record] = [], late: [Record] = []
    private var dropped = 0, presented = 0, lateCount = 0, missed = 0, segments = 0
    private var pending: (first: UInt64?, last: UInt64?, batches: Int, apply: Double) = (nil, nil, 0, 0)
    private var observers: [NSObjectProtocol] = []
    private var replyMain: [Double] = []
    private var replyCount = 0, replyTotal = 0.0

    /// A recording an app started (`startTrace_EXPERIMENTAL`): each callback until
    /// `stopTrace_EXPERIMENTAL`, and the scroller a person moved last, read where each
    /// turn ends, apart from what a held fling reads (`tracked`).
    final class Recording {
        var work: [String: Any] = [:]
        var records: [Record] = []
        var dropped = 0
        /// The scrollers the recording followed, in the order it took them:
        /// each one's name, its own in the recording, and its offset then: as
        /// a person took it, before it moved; when it `moved` with no finger,
        /// already past where it began.
        struct Taken { let name: String; var from: Double? = nil, moved = false }
        var scrollers: [Taken] = []
        #if canImport(UIKit)
        weak var scroller: UIScrollView?
        /// Each scroller the recording named, held weakly: an identifier
        /// names the same scroller only while it lives, and a new one at a
        /// released one's address is another scroller. `given` is every
        /// name given, which no later scroller takes.
        struct Named { weak var scroll: UIScrollView?; let name: String }
        var names: [ObjectIdentifier: Named] = [:]
        var given: Set<String> = []
        #endif
        /// The followed scroller's offset and phase as the last turn committed
        /// them, and which taken scroller it was then: one taken since is not
        /// the one this offset is from.
        var committed: (offset: Double, phase: String, taken: Int)?
        /// The kept frames whose presentation the probe has yet to report,
        /// by the number of the probe's drawable each holds: the frame's
        /// index, and when UIKit expected it on screen.
        var awaiting: [Int: (frame: Int, expected: CFTimeInterval)] = [:]
        /// The device's thermal state when the recording began
        let thermal = ProcessInfo.processInfo.thermalState
        @discardableResult func keep(_ record: Record) -> Bool {
            guard records.count < FrameSampler.recordingKept else { dropped += 1; return false }
            records.append(record)
            return true
        }
        /// The probe's report for the frame that holds drawable `drawable`:
        /// shown at `presented`, or never (0). A frame it finds late, and
        /// late by nothing else, gets what ran in its interval then from
        /// `timings`, as a frame late when it was sampled got it.
        func shown(drawable: Int, presented: CFTimeInterval,
                   timings: (_ from: CFTimeInterval, _ to: CFTimeInterval) -> (phases: [String: Any]?, hatches: [String: Any]?)) {
            guard let entry = awaiting.removeValue(forKey: drawable) else { return }
            let i = entry.frame, expected = entry.expected
            let wasLate = (records[i].missed > 0 && !records[i].quiet) || records[i].overrun > 0
            records[i].probed = true
            if presented > 0 {
                let late = ((presented - expected) * 1000 * 100).rounded() / 100
                if late > records[i].period / 2 { records[i].screenLate = late }
            } else {
                records[i].unshown = true
            }
            guard !wasLate, records[i].screenLate != nil || records[i].unshown else { return }
            let to = ExactEnv.t0 + records[i].t / 1000, ran = timings(to - records[i].interval / 1000, to)
            records[i].phases = ran.phases
            records[i].hatches = ran.hatches
        }
    }
    /// Five minutes at 120 Hz.
    static let recordingKept = 36_000
    private(set) var recording: Recording?
    private var busy = 0.0

    /// Main-thread wall time submitting and presenting each executor pump.
    /// A pump takes at most one outcome; wakes with no outcome count too.
    func replyPump(main ms: Double) {
        replyCount += 1; replyTotal += ms
        replyMain.append(ms)
        if replyMain.count > Self.ring { replyMain.removeFirst() }
    }
    /// The main thread's turns against the frame they serve (`watchTurns`):
    /// the last target the link gave, when this turn began, the worst
    /// overrun since the last sample, and how many there were.
    private var target: CFTimeInterval?
    private var turnBegan: CFTimeInterval?
    private var overrun = 0.0
    private var overruns = 0
    /// When the UI update now committing began its commit, and how long the
    /// commit of the update whose late commit `overrun` holds took.
    private var commitBegan: CFTimeInterval?
    private var overrunCommit: Double?
    private(set) var turnObservers: [CFRunLoopObserver] = []
    /// Whether a frame's lateness comes from UIKit's commit deadline
    /// (`watchUpdates`), not from the run loop's turns.
    var byDeadline = false
    private var updateLink: AnyObject?
    /// A passive recording's `UpdateRecorder` (iOS and tvOS 18 and later),
    /// which keeps the recording's frames in place of the display link.
    private(set) var recorder: AnyObject?
    /// The frame sampled in the UI update now running, with when and the
    /// recording's scroller as it stood then, until that update's commit
    /// says whether it was late: UIKit runs the update's display-link
    /// callbacks before it commits.
    private var open: (record: Record, at: CFTimeInterval, scrolled: Scrolled?)?
    typealias Scrolled = (offset: Double, phase: String, taken: Int)

    init(session: ExactSession) {
        self.session = session
        super.init()
        HostPhases.shared.on = true
        let center = NotificationCenter.default
        #if canImport(UIKit)
        observers.append(center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in self?.stop() })
        #else
        observers.append(center.addObserver(forName: NSWindow.didChangeOcclusionStateNotification, object: nil, queue: .main) { [weak self] note in
            if let window = note.object as? NSWindow, window === self?.session?.presenter.viewport.window, !window.occlusionState.contains(.visible) { self?.stop() }
        })
        #endif
    }

    deinit { for o in observers { NotificationCenter.default.removeObserver(o) } }

    /// A batch the session applied: its transactions (the trailer's `seq`)
    /// and the time applying it took, joined to the next sample.
    func batch(_ seq: (UInt64, UInt64)?, ms: Double) {
        // A nested apply reports before the batch around it: the range is the span.
        if let seq { pending.first = min(pending.first ?? seq.0, seq.0); pending.last = max(pending.last ?? seq.1, seq.1) }
        pending.batches += 1
        pending.apply += ms
        #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
        if #available(iOS 18.0, tvOS 18.0, *) { (recorder as? UpdateRecorder)?.batch(seq, ms: ms) }
        #endif
        activity()
    }

    /// Something visible may be happening: open a segment, or keep it open.
    func activity() {
        // A passive recording takes no display link where the binary does not measure on its own
        guard let session, (session.clock == nil || observing), session.state != .destroyed, recorder == nil || Self.measured else { return }
        active = CACurrentMediaTime()
        // A segment begun before the view was in a window reads UIKit's deadlines once it is
        guard link == nil else { watchUpdates(); return }
        #if canImport(UIKit)
        let l = CADisplayLink(target: self, selector: #selector(tick(_:)))
        #else
        let viewport = session.presenter.viewport
        guard viewport.window?.occlusionState.contains(.visible) == true else { return }
        let l = viewport.displayLink(target: self, selector: #selector(tick(_:)))
        #endif
        l.add(to: .main, forMode: .common)
        link = l
        last = nil
        segments += 1
        watchTurns()
        watchUpdates()
        followDisplay()
    }

    /// A display-link recording's link asks for the display's full rate, so
    /// that it ticks at every refresh a scroll draws; otherwise, a passive
    /// recording's included, it keeps the default range, which never raises
    /// the rate (LLP 1061 D4).
    private func followDisplay() {
        #if os(iOS)
        let range = recording == nil || recorder != nil ? CAFrameRateRange.default : FrameClock.full(on: session?.presenter.viewport.window?.screen)
        if let link, link.preferredFrameRateRange != range { link.preferredFrameRateRange = range }
        #endif
    }

    /// Forget every sample, and the link's baseline: a replaced runner
    /// numbers its transactions afresh. The next activity opens a segment.
    func reset() {
        stop()
        records = []; late = []
        dropped = 0; presented = 0; lateCount = 0; missed = 0; segments = 0; overruns = 0; overrun = 0; overrunCommit = nil
        pending = (nil, nil, 0, 0)
        period = nil
        replyMain = []; replyCount = 0; replyTotal = 0
    }

    func stop() {
        let interrupted = observationStopped
        observationStopped = nil
        concludeOpen()
        #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
        if #available(iOS 18.0, tvOS 18.0, *) {
            (updateLink as? UIUpdateLink)?.isEnabled = false
            // A passive recording goes on through the sampler's quiet stops, and ends with its session
            if session.map({ $0.state == .destroyed }) ?? true { (recorder as? UpdateRecorder)?.stop() }
        }
        #endif
        updateLink = nil; byDeadline = false
        link?.invalidate()
        link = nil
        last = nil
        for o in turnObservers { CFRunLoopRemoveObserver(CFRunLoopGetMain(), o, .commonModes) }
        turnObservers = []
        // A segment's timing is its own: an overrun not yet sampled goes
        // with it, not onto the next segment's first frame.
        target = nil; turnBegan = nil; overrun = 0; overrunCommit = nil; commitBegan = nil
        interrupted?("display observation stopped")
    }

    /// The main thread's turns, each timed from the run loop waking to it
    /// going back to sleep: a recording's `main`. Without an update link
    /// (`watchUpdates`), they also judge a frame the display link delivered
    /// on time that still missed the display: its turn, which commits on
    /// going to sleep, ended after the frame's target, and the render server
    /// showed the last frame again (a fling's rows built in one turn). The
    /// turn that installs them is timed from then: it may run on past the
    /// first target before it ever sleeps.
    private func watchTurns() {
        guard turnObservers.isEmpty else { return }
        turnBegan = CACurrentMediaTime()
        let woke = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.afterWaiting.rawValue, true, 0) { [weak self] _, _ in
            self?.turnBegan(at: CACurrentMediaTime())
        }
        let sleeps = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, true, CFIndex.max) { [weak self] _, _ in
            self?.turnEnded(at: CACurrentMediaTime())
        }
        for o in [woke, sleeps].compactMap({ $0 }) {
            CFRunLoopAddObserver(CFRunLoopGetMain(), o, .commonModes)
            turnObservers.append(o)
        }
    }

    /// On iOS and tvOS 18 and later (not Mac Catalyst, which has no
    /// `UIUpdateLink`), a frame is late when UIKit finishes
    /// its commit after the UI update's completion deadline, which delays
    /// the frame's presentation (`UIUpdateInfo.completionDeadlineTime`): a
    /// passive update link reads that after each commit. UIKit's update
    /// cycle need not commit where a turn ends, so the turns time only the
    /// main thread's work there.
    private func watchUpdates() {
        #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
        guard updateLink == nil, #available(iOS 18.0, tvOS 18.0, *), let view = session?.presenter.viewport, view.window?.windowScene != nil else { return }
        let link = UIUpdateLink(view: view)
        link.addAction(to: .beforeCATransactionCommit) { [weak self] _, _ in
            self?.commitBegins(at: CACurrentMediaTime())
        }
        link.addAction(to: .afterCATransactionCommit) { [weak self] _, info in
            self?.committed(at: CACurrentMediaTime(), deadline: info.completionDeadlineTime)
        }
        link.isEnabled = true
        updateLink = link
        byDeadline = true
        #endif
    }

    /// A UI update begins its commit at `now`.
    func commitBegins(at now: CFTimeInterval) { commitBegan = now }

    /// A UI update finished its commit at `now`, against its completion
    /// deadline: the frame sampled in that update is late by as much as it
    /// was past it, and keeps how long the commit took. A late commit in an
    /// update that sampled none goes to the next frame with its time.
    func committed(at now: CFTimeInterval, deadline: CFTimeInterval) {
        let past = max(0, ((now - deadline) * 1000 * 100).rounded() / 100)
        let took = commitBegan.map { max(0, ((now - $0) * 1000 * 100).rounded() / 100) }
        commitBegan = nil
        guard var frame = open else {
            if past > 0, past >= overrun { overrunCommit = took }
            overrun = max(overrun, past)
            return
        }
        open = nil
        // The commit that made the frame late, or its own when none did
        if past >= frame.record.overrun { frame.record.commit = took }
        frame.record.overrun = max(frame.record.overrun, past)
        frame.record.ownOverrun = past
        conclude(frame.record, at: frame.at, scrolled: frame.scrolled)
    }

    private func concludeOpen() {
        guard let frame = open else { return }
        open = nil
        conclude(frame.record, at: frame.at, scrolled: frame.scrolled)
    }

    func turnBegan(at now: CFTimeInterval) { turnBegan = now }

    /// A turn that began before the frame's target and ended after it.
    func turnEnded(at now: CFTimeInterval) {
        if let tracked { committed = tracked() }
        if let recording {
            recording.committed = recordedScroll().map { ($0.0, $0.1, recording.scrollers.count - 1) }
            if let began = turnBegan { busy += now - began }
        }
        defer { turnBegan = nil }
        guard !byDeadline, let target, let began = turnBegan, began < target, now > target else { return }
        overrun = max(overrun, (now - target) * 1000)
    }

    @objc func tick(_ link: CADisplayLink) {
        observe(now: link.timestamp, target: link.targetTimestamp, at: CACurrentMediaTime())
        // Which images show without their pixels this frame (LLP 1145 D2).
        session?.rasters.sampleBlank()
        // The session's own frame source running is activity too.
        if session?.frames.link != nil || held { active = CACurrentMediaTime() }
        if !observing && CACurrentMediaTime() - active > Self.quiet { stop() }
    }

    /// One display callback: its time and the next frame's target, seconds;
    /// `wall`, when it ran. A callback inside a turn that began before the
    /// last target and has not slept since is past that target already:
    /// the turn has not committed, so that frame is an overrun too.
    func observe(now: CFTimeInterval, target: CFTimeInterval, at wall: CFTimeInterval? = nil) {
        if !byDeadline, let wall, let old = self.target, let began = turnBegan, began < old, wall > old {
            overrun = max(overrun, (wall - old) * 1000)
        }
        self.target = target
        // Hold the interval to the period the last callback announced, so a
        // change of rate (a recording asking for the full one, ProMotion
        // lowering it) misses no frame
        let announced = period
        period = (target - now) * 1000
        if let last { sample(interval: (now - last) * 1000, period: announced ?? (target - now) * 1000, at: now) }
        last = now
    }

    private func sample(interval: Double, period p: Double, at now: CFTimeInterval) {
        let r2 = { (x: Double) in (x * 100).rounded() / 100 }
        let missedHere = max(0, Int((interval / p).rounded()) - 1)
        let seq = pending.first.flatMap { first in pending.last.map { (first, $0) } }
        var record = Record(t: r2((now - ExactEnv.t0) * 1000), interval: r2(interval), missed: missedHere, seq: seq, batches: pending.batches, apply: r2(pending.apply))
        record.period = r2(p)
        record.overrun = r2(overrun)
        record.commit = overrunCommit
        if tracked != nil, let c = committed { record.scroll = (r2(c.0), c.1) }
        if recording != nil { record.main = r2(busy * 1000); busy = 0 }
        overrun = 0; overrunCommit = nil
        pending = (nil, nil, 0, 0)
        // Its commit is still to come in this update
        if byDeadline { concludeOpen(); open = (record, now, recording?.committed); return }
        conclude(record, at: now, scrolled: recording?.committed)
    }

    /// A sampled frame, once whether it was late is known: kept, counted
    /// and, if late, journaled with what ran in it.
    private func conclude(_ sampled: Record, at now: CFTimeInterval, scrolled: Scrolled?) {
        var record = sampled
        records.append(record)
        if records.count > Self.ring { records.removeFirst(); dropped += 1 }
        presented += 1
        if record.overrun > 0 { overruns += 1 }
        guard record.missed > 0 || record.overrun > 0 else { keepRecorded(record, scrolled); return }
        lateCount += 1
        missed += record.missed
        // The hatches that ran in it: the sample's own interval, `(now − interval, now]`.
        record.hatches = session?.hatchDiagnostics.window(from: now - record.interval / 1000, to: now)
        record.phases = HostPhases.shared.window(from: now - record.interval / 1000, to: now)
        if record.hatches != nil || record.phases != nil { records[records.count - 1] = record }
        keepRecorded(record, scrolled)
        late.append(record)
        if late.count > Self.lateKept { late.removeFirst() }
        let range = record.seq.map { " seq \($0.0)..\($0.1)" } ?? ""
        let past = record.overrun <= 0 ? "" : byDeadline ? ", committed \(record.overrun) ms past its deadline" : ", main \(record.overrun) ms past the target"
        session?.log("frame late at \(record.t): \(record.missed) missed (\(record.interval) ms / \(record.period) target)\(past)\(range) apply \(record.apply)")
    }

    /// A display-link recording keeps the frame with its own scroller's
    /// offset and phase as they stood when it was sampled.
    private func keepRecorded(_ record: Record, _ scrolled: Scrolled?) {
        guard let recording, recorder == nil else { return }
        var r = record
        r.scroll = scrolled.map { (($0.offset * 100).rounded() / 100, $0.phase) }
        r.taken = scrolled?.taken
        recording.keep(r)
    }

    /// Keep every frame from now until `endRecording`: on iOS and tvOS 18
    /// and later, with the session's view in a window scene, each UI update
    /// (`UpdateRecorder`); elsewhere each display-link callback, at rest too.
    /// False while one runs, or with nothing to observe.
    func beginRecording() -> Bool {
        guard recording == nil else { return false }
        #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
        if #available(iOS 18.0, tvOS 18.0, *), let session {
            let passive = Recording()
            if let recorder = UpdateRecorder(session: session, recording: passive) {
                self.recorder = recorder
                recording = passive
                followDisplay()
                return true
            }
        }
        #endif
        guard beginObservation() else { return false }
        concludeOpen()
        recording = Recording()
        followDisplay()
        // The first frame is the recording's own, not what waited since the last segment
        busy = 0; overrun = 0; overrunCommit = nil
        pending = (nil, nil, 0, 0)
        return true
    }

    func endRecording() -> Recording? {
        guard let done = recording else { return nil }
        #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
        if #available(iOS 18.0, tvOS 18.0, *), let passive = recorder as? UpdateRecorder {
            passive.stop()
            recorder = nil
            recording = nil
            followDisplay()
            return done
        }
        #endif
        concludeOpen()
        recording = nil
        followDisplay()
        endObservation()
        return done
    }

    #if canImport(UIKit)
    /// `scroll` moved in `session`, by a person's drag when `person`: a
    /// recording follows it, unless a person is moving another. Every
    /// scroller's delegate tells it: an element's, a route's page and the
    /// document.
    static func scrolled(_ scroll: UIScrollView, in session: ExactSession?, person: Bool) {
        guard let sampler = session?.sampler, let recording = sampler.recording, recording.scroller !== scroll else { return }
        if !person, let current = recording.scroller, current.isTracking || current.isDecelerating { return }
        recording.scroller = scroll
        let id = ObjectIdentifier(scroll), name = recording.names[id].flatMap { $0.scroll === scroll ? $0.name : nil } ?? {
            // A second scroller of one name (two routes' pages) is told apart by a count
            let base = scrollerName(scroll)
            var name = base, count = 1
            while recording.given.contains(name) { count += 1; name = "\(base) (\(count))" }
            recording.names[id] = Recording.Named(scroll: scroll, name: name)
            recording.given.insert(name)
            return name
        }()
        recording.scrollers.append(Recording.Taken(name: name, from: person ? scroll.scrollSample.0 : nil, moved: !person))
    }

    private func recordedScroll() -> (Double, String)? { recording?.scroller?.scrollSample }
    #else
    /// No AppKit scroller is followed: a Mac recording's frames are all at rest.
    private func recordedScroll() -> (Double, String)? { nil }
    #endif

    /// Observe callbacks at rest too, without advancing or taking the session clock.
    func beginObservation() -> Bool {
        guard !observing else { return false }
        observing = true
        activity()
        if link == nil { observing = false; return false }
        return true
    }
    func endObservation() {
        observationStopped = nil
        observing = false
        if session?.clock != nil || (!held && CACurrentMediaTime() - active > Self.quiet) { stop() }
    }
    func observation(from: Double, before: Int) -> [String: Any] {
        let kept = records.filter { $0.t > from }
        return ["records": kept.map(\.json), "samples": presented - before,
                "dropped": max(0, presented - before - kept.count),
                "proxy": "CADisplayLink callback timestamps; not physical display presentation"]
    }
    var observedCount: Int { presented }

    /// `{"op":"perf","frames":true[,"late":N]}` (D4): lifetime counters, the
    /// ring's percentiles with the range they cover, the most recent late
    /// records; `all` adds every retained record (a trace, D5).
    func reply(late n: Int = 20, all: Bool = false) -> [String: Any] {
        let xs = records.map(\.interval).sorted()
        let q = { (f: Double) -> Any in xs.isEmpty ? NSNull() : xs[min(xs.count - 1, Int(f * Double(xs.count)))] }
        var out: [String: Any] = [
            "replyPumps": ["count": replyCount, "mainTotalMs": replyTotal,
                           "mainMs": replyMain, "includesEmptyWakes": true] as [String: Any],
            "period": ["ms": period.map { ($0 * 100).rounded() / 100 } ?? NSNull(), "source": "target"] as [String: Any],
            "covers": ["batches", "frame-source", "turns"],
            "lifetime": ["presented": presented, "late": lateCount, "missed": missed, "overruns": overruns, "segments": segments],
            "window": ["from": records.first?.t ?? NSNull(), "to": records.last?.t ?? NSNull(), "samples": records.count, "dropped": dropped,
                       "p50": q(0.5), "p95": q(0.95), "p99": q(0.99), "max": xs.last ?? NSNull()] as [String: Any],
            "late": late.suffix(max(0, min(Self.lateKept, n))).map(\.json),
        ]
        if all { out["records"] = records.map(\.json) }
        return out
    }
}

extension ExactSession {
    /// Save Trace (LLP 1079 D5): this session's journal, its presented frames,
    /// `perf` over every root and its hatches, with who made them and each timing's
    /// proxy, as `trace-<wallclock>.json` in the app's temporary directory;
    /// `agent.mjs trace <file>` reads it back. The path, or why not.
    /// `signalled`, it is also `trace-pid-<pid>.json`, the name `trace --pid`
    /// waits for (LLP 1142 D5).
    public func saveTrace(signalled: Bool = false) -> Result<URL, Error> {
        guard FrameSampler.measured else { return .failure(TraceError.production) }
        // A release build's first trace starts its measuring: it has the
        // journal and the sites' work, and frames and hatch timings empty.
        defer { measurePerf() }
        let frames = sampler?.reply(late: FrameSampler.lateKept, all: true)
            ?? FrameSampler(session: self).reply(late: 0, all: true).merging(["unmeasured": FrameSampler.unmeasured]) { a, _ in a }
        let raw = { (json: String) in (try? JSONSerialization.jsonObject(with: Data(json.utf8))) ?? ["error": "unreadable"] }
        // The sites' work, which a release build counts from here: its live
        // instances, and no work yet.
        runtime.measureWork()
        let perf = raw(agent("{\"op\":\"perf\"}"))
        #if canImport(UIKit)
        let host = "ios", device = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"].map { "\($0) (simulator)" } ?? UIDevice.current.model
        #else
        let host = "macos", device = Host.current().localizedName ?? "Mac"
        #endif
        let bundle = Bundle.main.bundleIdentifier ?? ProcessInfo.processInfo.processName
        let worlds = canvases.worlds(["op": "state", "trace_export": true])
        let events = worlds.flatMap { world -> [[String: Any]] in
            let canvas = (world["canvas"] as? NSNumber)?.intValue ?? 0
            return (world["traceEvents"] as? [[String: Any]] ?? []).map { row in
                var row = row; row["pid"] = canvas * 2 + (row["pid"] as? Int ?? 0); return row
            }
        }
        // The host binary's build, as `build.mjs` stamped it (LLP 1142 D2). It
        // is not the running plan's: a development host may run another plan.
        let info = ExactEnv.appMetadata
        var hostBuild: [String: Any] = ["bundlePath": Bundle.main.bundlePath]
        for (key, name) in [("ExactCommit", "commit"), ("ExactCommitDirty", "dirty"), ("ExactBuildHost", "machine"), ("ExactBuildTime", "at"),
                            ("ExactAppCommit", "appCommit"), ("ExactAppCommitDirty", "appDirty")] {
            if let value = info[key] { hostBuild[name] = value }
        }
        let identity: [String: Any] = ["host": host, "app": bundle.split(separator: ".").last.map(String.init) ?? bundle, "bundle": bundle, "trust": "development",
                                       "os": ProcessInfo.processInfo.operatingSystemVersionString, "device": device,
                                       "bootWall": ((Date().timeIntervalSince1970 - (CACurrentMediaTime() - ExactEnv.t0)) * 1000).rounded(),
                                       "pid": Int(ProcessInfo.processInfo.processIdentifier), "session": label,
                                       "savedAt": (Date().timeIntervalSince1970 * 1000).rounded(), "hostBuild": hostBuild]
        let trace: [String: Any] = [
            "traceEvents": events,
            "identity": identity,
            "proxies": ["period": "target: the sampler's own display link, targetTimestamp − timestamp",
                        "interval": "the sampler's display-link callback gap", "covers": ["batches", "frame-source", "turns"],
                        "overrun": "iOS and tvOS 18 and later: the UI update's commit past its completionDeadlineTime (UIUpdateLink, afterCATransactionCommit); elsewhere: a main run-loop turn that began before the frame's targetTimestamp and went to sleep after it",
                        "loaf": "none on this host"],
            "plan": (perf as? [String: Any])?["plan"] ?? NSNull(),
            "journal": raw(agent("{\"op\":\"logs\",\"since\":0}")),
            "frames": frames,
            "perf": perf,
            // The hatches (LLP 1075.003.000.001 §3.3), read in this same turn, so the sections agree.
            "hatches": ["state": presenter.elements.observation(hatchDiagnostics), "perf": hatchPerf()],
        ]
        do {
            let stamp = ISO8601DateFormatter().string(from: Date()).replacingOccurrences(of: ":", with: "-")
            let url = FileManager.default.temporaryDirectory.appendingPathComponent("trace-\(stamp).json")
            let data = try JSONSerialization.data(withJSONObject: trace)
            try data.write(to: url)
            // The last one under a name the driver knows, so a phone's is
            // copied off by name (`agent.mjs trace --phone`), as its
            // screenshots are: devicectl lists no app's files.
            try data.write(to: FileManager.default.temporaryDirectory.appendingPathComponent(Self.latestTrace), options: .atomic)
            if signalled {
                try data.write(to: FileManager.default.temporaryDirectory.appendingPathComponent(TraceSignal.file), options: .atomic)
            }
            log("trace saved: \(url.path)")
            FileHandle.standardError.write(Data("exact: trace saved to \(url.path)\n".utf8))
            return .success(url)
        } catch {
            log("trace refused: \(error.localizedDescription)")
            return .failure(error)
        }
    }

    enum TraceError: Error { case production }

    /// Start measuring what `perf` reads, where this binary can and has not
    /// yet (a release build, LLP 1079): the presented frames, the hatch
    /// calls' timing and the plan sites' work. The agent's requests, and
    /// Save Trace, ask.
    func measurePerf(frameSampler: Bool = true) {
        guard FrameSampler.measured else { return }
        HatchDiagnostics.measuring = true
        if frameSampler, sampler == nil { sampler = FrameSampler(session: self) }
        runtime.measureWork()
    }
    /// The last saved trace, beside the stamped one (`agent.mjs` reads the name).
    static let latestTrace = "trace-latest.json"
}

/// `SIGUSR1` saves the session's trace, with no alert (LLP 1142 D5), as Linux's
/// display loop does: `agent.mjs trace --pid <pid>` asks a running app for
/// one with no person at its menu. Not in a production bake. The handler only
/// marks the signal; the save runs on the main queue.
public enum TraceSignal {
    nonisolated(unsafe) private static var source: DispatchSourceSignal?
    private static let pid = ProcessInfo.processInfo.processIdentifier
    /// What a signalled save writes beside the stamped trace, in the
    /// temporary directory: the driver waits for it.
    static var file: String { "trace-pid-\(pid).json" }
    /// Written when the handler is installed, so the driver signals only a
    /// process that handles it: unhandled, `SIGUSR1` terminates it.
    static var ready: URL { FileManager.default.temporaryDirectory.appendingPathComponent("exact-trace-\(pid).ready") }

    /// `session` is asked at each signal: the one to save (a Mac app's front
    /// window's; the trace names it), nil for none.
    public static func install(_ session: @escaping () -> ExactSession?) {
        guard FrameSampler.measured, source == nil else { return }
        // Caught, not ignored: a caught signal resets to the default in an
        // executed helper, as SIGTERM's does in ExactMac.
        signal(SIGUSR1) { _ in }
        let s = DispatchSource.makeSignalSource(signal: SIGUSR1, queue: .main)
        s.setEventHandler { _ = session()?.saveTrace(signalled: true) }
        s.resume()
        source = s
        FileManager.default.createFile(atPath: ready.path, contents: Data("\(pid)\n".utf8))
        atexit { try? FileManager.default.removeItem(at: TraceSignal.ready) }
    }
}

extension ExactSession {
    static let journal = Logger(subsystem: "com.exact.app", category: "journal")

    /// A batch's failure lines (LLP 1142 D4: a resource refused, a request
    /// or a send failed), written where a person or an agent looks: the
    /// unified log (`log show --predicate 'subsystem == "com.exact.app"'`)
    /// and stderr, which `build.mjs --run` shows. The runner queues them only
    /// while it measures, so a production bake has none.
    func logAlerts(_ lines: [String]) {
        for line in lines {
            Self.journal.error("\(self.label, privacy: .public): \(line, privacy: .public)")
            FileHandle.standardError.write(Data("exact: \(line)\n".utf8))
        }
    }

    /// The journal's other lines (LLP 1143 open question 1), at a lower
    /// level than a failure and not on stderr: a run a person drives reads
    /// what `logs` would have said with `log stream --level info`. A host
    /// line (`host: …`) is HostLog's, already written as it happened.
    func logJournal(_ lines: [String]) {
        for line in lines where !line.hasPrefix("host: ") {
            Self.journal.info("\(self.label, privacy: .public): \(line, privacy: .public)")
        }
    }
}

/// One bounded capture's completion, including interruption. The wire's
/// reader waits on this callback while the main event pump keeps running.
final class FrameObservation {
    private var timer: Timer?
    private var completed: ((String?) -> Void)?
    init(ms: Int, completed: @escaping (String?) -> Void) {
        self.completed = completed
        let end = Timer(timeInterval: Double(ms) / 1000, repeats: false) { [self] _ in finish() }
        timer = end
        RunLoop.main.add(end, forMode: .common)
    }
    func finish(_ reason: String? = nil) {
        guard let completed else { return }
        self.completed = nil
        timer?.invalidate(); timer = nil
        completed(reason)
    }
}

// @ref LLP 1142 §13 D8 — independent real-time observation, never a clock loan.
extension Agent {
    func liveFrames(_ request: [String: Any], completed: @escaping ([String: Any]) -> Void) {
        guard let ms = Agent.integer(request["live"]), (1...120_000).contains(ms) else {
            completed(["malformed": true, "error": "perf frames live: milliseconds must be an integer in 1...120000"])
            return
        }
        guard FrameSampler.measured, let sampler = session.sampler else { completed(["unavailable": true]); return }
        guard sampler.beginObservation() else {
            completed(["unavailable": true, "reason": "capture already active or no visible window display link", "clockMode": session.clock == nil ? "real" : "virtual"])
            return
        }
        let start = CACurrentMediaTime(), from = session.now(), before = sampler.observedCount
        let observation = FrameObservation(ms: ms) { [self] reason in
            sampler.endObservation()
            let storageSubstituted = ExactEnv.environment["EXACT_AGENT"] != nil && ExactEnv.environment["EXACT_STORE"] != "real"
            var result = sampler.reply(late: request["late"] as? Int ?? 20, all: request["records"] as? Bool == true)
            result["live"] = ["ms": (CACurrentMediaTime() - start) * 1000, "from": from, "to": session.now(),
                              "clockMode": session.clock == nil ? "real" : "virtual", "clockChanged": false,
                              "execution": ExactEnv.agentMode ? "agent" : "normal",
                              "scheduling": ExactEnv.agentMode ? "agent" : "normal",
                              "performanceClaim": !ExactEnv.agentMode && !storageSubstituted && session.clock == nil,
                              "inputPath": "unknown", "observation": sampler.observation(from: (start - ExactEnv.t0) * 1000, before: before),
                              "substitutions": ExactEnv.agentMode ? ["timers", "collection-fill-scheduling", "scroll-velocity", "wheel-routing", "device-facts", "capabilities"] : storageSubstituted ? ["storage", "secret-presence"] : []] as [String: Any]
            if let reason { result["error"] = session.state == .destroyed ? "session destroyed during live observation" : reason }
            completed(result)
        }
        // A nested Foundation run loop does not dispatch AppKit's events.
        sampler.observationStopped = { observation.finish($0) }
    }
}

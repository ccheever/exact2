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
    /// Whether this binary measures on its own: not a production bake, which
    /// measures only while an app records.
    static let measured = !ExactEnv.productionBake
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
        /// How long past the frame's target the main thread's turn ended,
        /// milliseconds (0: within it): a commit that landed after it.
        var overrun = 0.0
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
        var json: [String: Any] {
            var o: [String: Any] = ["t": t, "target": t + period, "period": period, "interval": interval, "missed": missed, "batches": batches, "apply": apply, "overrun": overrun]
            o["seq"] = seq.map { [$0.0, $0.1] } ?? NSNull()
            if let main { o["main"] = main }
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
        func keep(_ record: Record) {
            if records.count < FrameSampler.recordingKept { records.append(record) } else { dropped += 1 }
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
    private(set) var turnObservers: [CFRunLoopObserver] = []

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
        activity()
    }

    /// Something visible may be happening: open a segment, or keep it open.
    func activity() {
        guard let session, (session.clock == nil || observing), session.state != .destroyed else { return }
        active = CACurrentMediaTime()
        guard link == nil else { return }
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
        followDisplay()
    }

    /// A recording's link asks for the display's full rate, so that it ticks
    /// at every refresh a scroll draws; otherwise it keeps the default range,
    /// which never raises the rate (LLP 1061 D4).
    private func followDisplay() {
        #if os(iOS)
        let range = recording == nil ? CAFrameRateRange.default : FrameClock.full(on: session?.presenter.viewport.window?.screen)
        if let link, link.preferredFrameRateRange != range { link.preferredFrameRateRange = range }
        #endif
    }

    /// Forget every sample, and the link's baseline: a replaced runner
    /// numbers its transactions afresh. The next activity opens a segment.
    func reset() {
        stop()
        records = []; late = []
        dropped = 0; presented = 0; lateCount = 0; missed = 0; segments = 0; overruns = 0; overrun = 0
        pending = (nil, nil, 0, 0)
        period = nil
        replyMain = []; replyCount = 0; replyTotal = 0
    }

    func stop() {
        let interrupted = observationStopped
        observationStopped = nil
        link?.invalidate()
        link = nil
        last = nil
        for o in turnObservers { CFRunLoopRemoveObserver(CFRunLoopGetMain(), o, .commonModes) }
        turnObservers = []
        // A segment's timing is its own: an overrun not yet sampled goes
        // with it, not onto the next segment's first frame.
        target = nil; turnBegan = nil; overrun = 0
        interrupted?("display observation stopped")
    }

    /// A frame the display link delivered on time can still miss the
    /// display: the main thread's turn that commits it ends after the
    /// frame's target, and the render server shows the last frame again
    /// (a fling's rows built in one turn). The link's own cadence does not
    /// see it; the turn's end against the target does. Each turn is
    /// timed from the run loop waking to it going back to sleep, after
    /// Core Animation's commit. The turn that installs them is timed from
    /// then: it may run on past the first target before it ever sleeps.
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

    func turnBegan(at now: CFTimeInterval) { turnBegan = now }

    /// A turn that began before the frame's target and ended after it.
    func turnEnded(at now: CFTimeInterval) {
        if let tracked { committed = tracked() }
        if let recording {
            recording.committed = recordedScroll().map { ($0.0, $0.1, recording.scrollers.count - 1) }
            if let began = turnBegan { busy += now - began }
        }
        defer { turnBegan = nil }
        guard let target, let began = turnBegan, began < target, now > target else { return }
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
        if let wall, let old = self.target, let began = turnBegan, began < old, wall > old {
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
        if tracked != nil, let c = committed { record.scroll = (r2(c.0), c.1) }
        if recording != nil { record.main = r2(busy * 1000); busy = 0 }
        overrun = 0
        pending = (nil, nil, 0, 0)
        records.append(record)
        if records.count > Self.ring { records.removeFirst(); dropped += 1 }
        presented += 1
        if record.overrun > 0 { overruns += 1 }
        guard missedHere > 0 || record.overrun > 0 else { keepRecorded(record); return }
        lateCount += 1
        missed += missedHere
        // The hatches that ran in it: the sample's own interval, `(now − interval, now]`.
        record.hatches = session?.hatchDiagnostics.window(from: now - interval / 1000, to: now)
        record.phases = HostPhases.shared.window(from: now - interval / 1000, to: now)
        if record.hatches != nil || record.phases != nil { records[records.count - 1] = record }
        keepRecorded(record)
        late.append(record)
        if late.count > Self.lateKept { late.removeFirst() }
        let range = seq.map { " seq \($0.0)..\($0.1)" } ?? ""
        let past = record.overrun > 0 ? ", main \(record.overrun) ms past the target" : ""
        session?.log("frame late at \(record.t): \(missedHere) missed (\(record.interval) ms / \(r2(p)) target)\(past)\(range) apply \(record.apply)")
    }

    /// A recording keeps the frame with its own scroller's offset and phase.
    private func keepRecorded(_ record: Record) {
        guard let recording else { return }
        var r = record
        r.scroll = recording.committed.map { (($0.offset * 100).rounded() / 100, $0.phase) }
        r.taken = recording.committed?.taken
        recording.keep(r)
    }

    /// Keep every frame from now, at rest too, until `endRecording`. False
    /// while one runs, or with no display link to observe.
    func beginRecording() -> Bool {
        guard recording == nil, beginObservation() else { return false }
        recording = Recording()
        followDisplay()
        // The first frame is the recording's own, not what waited since the last segment
        busy = 0; overrun = 0
        pending = (nil, nil, 0, 0)
        return true
    }

    func endRecording() -> Recording? {
        guard let done = recording else { return nil }
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
                        "overrun": "a main run-loop turn that began before the frame's targetTimestamp and went to sleep after it (after Core Animation's commit)",
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

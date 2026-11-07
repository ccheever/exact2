import CExact
import ExactCore
import Foundation
import QuartzCore
import XCTest
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

/// LLP 1079 D3–D5 on Apple: the trailer's `seq`, the sampler's arithmetic
/// against the link's own target period, its journal line, and Save Trace.
final class FrameSamplerTests: XCTestCase {
    func testObservationReturnsOnlyItsWindowAndReportsRingDrops() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        let p = 1.0 / 60, start = ExactEnv.t0 + 50
        sampler.observe(now: start - p, target: start)
        let before = sampler.observedCount
        for i in 0...FrameSampler.ring {
            let t = start + Double(i) * p
            sampler.observe(now: t, target: t + p)
        }
        let result = sampler.observation(from: (start - p / 2 - ExactEnv.t0) * 1000, before: before)
        XCTAssertEqual(result["samples"] as? Int, FrameSampler.ring + 1)
        XCTAssertEqual(result["dropped"] as? Int, 1)
        let records = try XCTUnwrap(result["records"] as? [[String: Any]])
        XCTAssertEqual(records.count, FrameSampler.ring)
        XCTAssertNotNil(records.first?["period"])
        XCTAssertNotNil(records.first?["target"])
        session.clock = 1234
        var invalid: [String: Any] = [:]
        Agent(session: session).liveFrames(["live": 0]) { invalid = $0 }
        XCTAssertNotNil(invalid["malformed"])
        XCTAssertEqual(session.clock, 1234, "a refused observation never takes or seeks the clock")
    }

    #if os(macOS)
    func testHeldSamplingRestoresTheExecutionModesWheelRouting() throws {
        XCTAssertFalse(FrameSampler.wheelAppliesAtOnce(agentMode: false, held: true))
        XCTAssertFalse(FrameSampler.wheelAppliesAtOnce(agentMode: false, held: false))
        XCTAssertFalse(FrameSampler.wheelAppliesAtOnce(agentMode: true, held: true))
        XCTAssertTrue(FrameSampler.wheelAppliesAtOnce(agentMode: true, held: false))
        let original = ChainingScrollView.appliesAtOnce
        let session = ExactApp.shared.makeSession()
        defer { session.destroy(); ChainingScrollView.appliesAtOnce = original }
        let sampler = try XCTUnwrap(session.sampler)
        sampler.held = true
        XCTAssertEqual(ChainingScrollView.appliesAtOnce, ExactEnv.agentFreezes ? original : false)
        sampler.held = false
        XCTAssertEqual(ChainingScrollView.appliesAtOnce, ExactEnv.agentFreezes ? original : ExactEnv.agentMode)
    }

    func testCaptureCompletionReturnsToAppKitsEventPumpAndRepliesOnce() throws {
        let app = NSApplication.shared
        var returned = false, heard = false, replies = 0
        var observation: FrameObservation?
        func wake(_ token: Int) {
            app.postEvent(NSEvent.otherEvent(with: .applicationDefined, location: .zero, modifierFlags: [], timestamp: 0,
                windowNumber: 0, context: nil, subtype: 0, data1: token, data2: 0)!, atStart: false)
        }
        let monitor = NSEvent.addLocalMonitorForEvents(matching: .applicationDefined) { event in
            if event.data1 == 1142 {
                XCTAssertTrue(returned, "capture must return before AppKit can dispatch the queued event")
                XCTAssertEqual(replies, 0, "the event is dispatched inside the capture interval")
                heard = true
                return nil
            }
            return event
        }
        let escape = Timer(timeInterval: 3, repeats: false) { _ in app.stop(nil); wake(1144) }
        RunLoop.main.add(escape, forMode: .common)
        defer { escape.invalidate(); observation?.finish(); if let monitor { NSEvent.removeMonitor(monitor) } }
        DispatchQueue.main.async {
            observation = FrameObservation(ms: 100) { reason in
                replies += 1
                XCTAssertNil(reason)
                XCTAssertTrue(heard)
                app.stop(nil); wake(1143)
            }
            returned = true
            wake(1142)
        }
        app.run()
        XCTAssertTrue(heard)
        XCTAssertEqual(replies, 1)
        observation?.finish("late interruption")
        XCTAssertEqual(replies, 1)
    }

    func testDestroyingASessionInterruptsItsCaptureCompletionOnce() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let sampler = try XCTUnwrap(session.sampler)
        var replies = 0
        let observation = FrameObservation(ms: 1000) { reason in
            replies += 1
            XCTAssertEqual(reason, "display observation stopped")
            XCTAssertEqual(session.state, .destroyed)
        }
        sampler.observationStopped = { observation.finish($0) }
        XCTAssertEqual(replies, 0)
        session.destroy()
        XCTAssertEqual(replies, 1)
        sampler.stop()
        observation.finish()
        XCTAssertEqual(replies, 1)
    }
    #endif

    func testTheTrailerCarriesTheTransactionsItApplies() {
        let batch = Batch.decode(Data(#"{"ops":[],"seq":[3,7],"timers":false,"motion":false,"canvas":false,"clock":0,"error":null}"#.utf8))
        XCTAssertNil(batch.error)
        XCTAssertEqual(batch.seq?.0, 3)
        XCTAssertEqual(batch.seq?.1, 7)
        XCTAssertNil(Batch.decode(Data(#"{"ops":[],"timers":false,"motion":false,"clock":0,"error":null}"#.utf8)).seq)
    }

    /// A recording's report (`stopTrace_EXPERIMENTAL`): a person's drag and
    /// glide are segments of their own on the scroller they took, each the
    /// distance from where its first interval began (for the first, where
    /// the scroller was when the recording took it), only a frame shown again
    /// counts as lateness (the link's jitter does not), and each late frame
    /// names its phase.
    func testARecordingReportsEachScrollPhaseAndItsLateFrames() throws {
        let recording = FrameSampler.Recording()
        recording.scrollers = [.init(name: "list", from: -4)]
        let p = 1000.0 / 120
        func frame(_ t: Double, _ interval: Double, _ phase: String?, _ y: Double = 0, overrun: Double = 0) {
            var r = FrameSampler.Record(t: t, interval: interval, missed: max(0, Int((interval / p).rounded()) - 1), seq: nil, batches: 0, apply: 0)
            r.period = p; r.overrun = overrun; r.main = 2
            if let phase { r.scroll = (y, phase); r.taken = 0 }
            recording.keep(r)
        }
        frame(8.33, p, nil)
        frame(16.67, p + 0.02, "t", 0)
        frame(33.33, p * 2, "t", 10)
        frame(41.67, p, "d", 20, overrun: 3)
        frame(50, p, "d", 25)
        frame(58.33, p, "-", 25)
        let report = FrameSampler.report(recording, identity: [:])
        let segments = try XCTUnwrap(report["segments"] as? [[String: Any]])
        XCTAssertEqual(segments.map { $0["phase"] as? String }, ["rest", "drag", "glide", "rest"])
        XCTAssertEqual(segments[1]["scroller"] as? String, "list")
        XCTAssertEqual(segments[1]["missed"] as? Int, 1)
        XCTAssertEqual(segments[1]["hitchMs"] as? Double, 8.33)
        XCTAssertEqual(segments[1]["scrolledPt"] as? Double, 14, "the drag from where the scroller was when taken")
        XCTAssertEqual(segments[2]["scrolledPt"] as? Double, 15, "the glide from where the drag left it")
        let late = try XCTUnwrap(report["late"] as? [[String: Any]])
        XCTAssertEqual(late.map { $0["phase"] as? String }, ["drag", "glide"])
        XCTAssertEqual(late[1]["overrun"] as? Double, 3)
    }

    /// A recording keeps its first 36,000 callbacks and counts the rest, and
    /// both reports say how many it did not keep.
    func testARecordingKeepsItsLimitAndCountsTheRest() throws {
        let recording = FrameSampler.Recording()
        let p = 1000.0 / 120
        for i in 0..<(FrameSampler.recordingKept + 5) {
            var r = FrameSampler.Record(t: Double(i) * p, interval: p, missed: 0, seq: nil, batches: 0, apply: 0)
            r.period = p
            recording.keep(r)
        }
        XCTAssertEqual(recording.records.count, FrameSampler.recordingKept)
        XCTAssertEqual(recording.dropped, 5)
        let report = FrameSampler.report(recording, identity: [:])
        XCTAssertEqual((report["recording"] as? [String: Any])?["droppedFrames"] as? Int, 5)
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: "trace.json", bytes: 1, maxBytes: 8000)) as? [String: Any])
        XCTAssertEqual(compact["droppedFrames"] as? Int, 5)
    }

    /// A compact late frame keeps its timings' coverage: `partial` when the
    /// host phases or hatch calls kept no longer reach back over the frame.
    func testACompactLateFrameKeepsItsTimingsCoverage() throws {
        let recording = FrameSampler.Recording()
        let p = 1000.0 / 120
        var r = FrameSampler.Record(t: p, interval: p * 3, missed: 2, seq: nil, batches: 0, apply: 0)
        r.period = p
        r.phases = ["phases": [["phase": "runtime", "ms": 4.0]], "unattributed": 20.0, "coverage": "partial"]
        r.hatches = ["hatches": [["hatch": "element avatar", "ms": 2.0]], "coverage": "partial"]
        recording.keep(r)
        let report = FrameSampler.report(recording, identity: [:])
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: "trace.json", bytes: 1, maxBytes: 8000)) as? [String: Any])
        let columns = try XCTUnwrap(compact["lateColumns"] as? [String])
        let row = try XCTUnwrap((compact["late"] as? [[Any]])?.first)
        for column in ["phasesCoverage", "coverage"] {
            XCTAssertEqual(row[try XCTUnwrap(columns.firstIndex(of: column), column)] as? String, "partial", column)
        }
    }

    /// A recording's whole trace is never written over another file: a name
    /// already taken (two recordings stopped in one millisecond) is tried
    /// again with a new suffix, and the first file is left as it was.
    func testATraceFileIsNeverWrittenOverAnother() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("trace-save-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = directory.appendingPathComponent("trace-recording-T-a.json")
        try Data("first".utf8).write(to: first)
        var suffixes = ["a", "b"].makeIterator()
        let url = try FrameSampler.save(Data("second".utf8), in: directory, stamp: "T") { suffixes.next() ?? "c" }
        XCTAssertEqual(url.lastPathComponent, "trace-recording-T-b.json")
        XCTAssertEqual(try Data(contentsOf: first), Data("first".utf8))
        XCTAssertEqual(try Data(contentsOf: url), Data("second".utf8))
    }

    /// A scroller that moves less than a point a frame with no finger on it
    /// (a slow animation) is moving in every frame its offset changes.
    func testASlowMoveIsMovingInEveryFrame() throws {
        let recording = FrameSampler.Recording()
        recording.scrollers = [.init(name: "list", from: 0)]
        let p = 1000.0 / 120
        for (i, y) in [0, 0.33, 0.67, 1.0, 1.0].enumerated() {
            var r = FrameSampler.Record(t: Double(i + 1) * p, interval: p, missed: 0, seq: nil, batches: 0, apply: 0)
            r.period = p; r.scroll = (y, "-"); r.taken = 0
            recording.keep(r)
        }
        let segments = try XCTUnwrap(FrameSampler.report(recording, identity: [:])["segments"] as? [[String: Any]])
        XCTAssertEqual(segments.map { $0["phase"] as? String }, ["rest", "moving", "rest"])
        XCTAssertEqual(segments[1]["frames"] as? Int, 3)
        XCTAssertEqual(segments[1]["scrolledPt"] as? Double, 1)
    }

    /// A scroller the recording took as it moved with no finger on it (the
    /// app's own scroll, the first `scrollViewDidScroll` already past where it
    /// began) is moving in its first frame, with no distance, since the
    /// recording never saw where it began.
    func testAScrollerTakenAsItMovedIsMovingFromItsFirstFrame() throws {
        let recording = FrameSampler.Recording()
        recording.scrollers = [.init(name: "page", moved: true)]
        let p = 1000.0 / 120
        for (i, y) in [nil, 40, 40].enumerated() {
            var r = FrameSampler.Record(t: Double(i + 1) * p, interval: p, missed: 0, seq: nil, batches: 0, apply: 0)
            r.period = p
            if let y { r.scroll = (Double(y), "-"); r.taken = 0 }
            recording.keep(r)
        }
        let segments = try XCTUnwrap(FrameSampler.report(recording, identity: [:])["segments"] as? [[String: Any]])
        XCTAssertEqual(segments.map { $0["phase"] as? String }, ["rest", "moving", "rest"])
        XCTAssertNil(segments[1]["scrolledPt"])
    }

    /// `stopTrace_EXPERIMENTAL(maxBytes)`: the report cut to fit, its late
    /// frames those late the longest first, as many as fit, and its scroll
    /// segments kept.
    func testACompactReportFitsItsBudgetWithTheLongestLateFramesFirst() throws {
        let recording = FrameSampler.Recording()
        recording.scrollers = [.init(name: "list")]
        let p = 1000.0 / 120
        for i in 0..<300 {
            let interval = i % 3 == 0 ? p * Double(2 + i % 4) : p
            var r = FrameSampler.Record(t: Double(i) * p, interval: interval, missed: max(0, Int((interval / p).rounded()) - 1), seq: nil, batches: 1, apply: 1.5)
            r.period = p; r.main = 4; r.scroll = (Double(i), i < 150 ? "t" : "d"); r.taken = 0
            recording.keep(r)
        }
        recording.dropped = 3
        let report = FrameSampler.report(recording, identity: ["device": "test"])
        let data = FrameSampler.compact(report, file: "trace.json", bytes: 1, maxBytes: 4000)
        XCTAssertLessThanOrEqual(data.count, 4000)
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertEqual(compact["droppedFrames"] as? Int, 3, "the callbacks past what a recording keeps")
        XCTAssertEqual((compact["segments"] as? [[Any]])?.map { $0.first as? String }, ["drag", "glide"])
        let late = try XCTUnwrap(compact["late"] as? [[Any]])
        XCTAssertFalse(late.isEmpty)
        let intervals = late.compactMap { $0[2] as? Double }
        XCTAssertEqual(intervals, intervals.sorted(by: >))
        let columns = try XCTUnwrap(compact["lateColumns"] as? [String])
        XCTAssertEqual(late.first?[try XCTUnwrap(columns.firstIndex(of: "scroller"))] as? String, "list", "each late frame names its scroller")
        XCTAssertGreaterThan(FrameSampler.compact(report, file: "trace.json", bytes: 1, maxBytes: 8000).count, data.count)
        let unsaved = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 4000)) as? [String: Any])
        XCTAssertEqual(unsaved["full"] as? String, "not saved")
    }

    /// A budget the totals alone exceed gets a note within it naming the
    /// smallest that fits; one between that and every segment keeps the
    /// segments that fit, the worst first.
    func testACompactReportKeepsToABudgetTooSmallForAllOfIt() throws {
        let recording = FrameSampler.Recording()
        recording.scrollers = [.init(name: "list")]
        let p = 1000.0 / 120
        for i in 0..<400 {
            let interval = i % 7 == 0 ? p * 2 : p
            var r = FrameSampler.Record(t: Double(i) * p, interval: interval, missed: interval > p ? 1 : 0, seq: nil, batches: 0, apply: 0)
            r.period = p; r.scroll = (Double(i), (i / 10) % 2 == 0 ? "t" : "d"); r.taken = 0
            recording.keep(r)
        }
        let report = FrameSampler.report(recording, identity: [:])
        let note = FrameSampler.compact(report, file: "trace.json", bytes: 1, maxBytes: 600)
        XCTAssertLessThanOrEqual(note.count, 600)
        let error = try XCTUnwrap((try JSONSerialization.jsonObject(with: note) as? [String: Any])?["error"] as? String)
        let smallest = try XCTUnwrap(error.split(separator: " ").compactMap { Int($0) }.last, error)
        XCTAssertGreaterThan(smallest, 600)
        let cut = FrameSampler.compact(report, file: "trace.json", bytes: 1, maxBytes: smallest + 400)
        XCTAssertLessThanOrEqual(cut.count, smallest + 400)
        let kept = try XCTUnwrap((try JSONSerialization.jsonObject(with: cut) as? [String: Any])?["segments"] as? [[Any]])
        XCTAssertGreaterThan(kept.count, 0)
        XCTAssertLessThan(kept.count, 40, "of 40 scroll segments")
    }

    /// The batch that starts a session's first recording, with no sampler
    /// before it (a production bake's), charges the recording's first frame
    /// nothing: the sampler was not there when the batch began.
    func testTheBatchThatStartsARecordingChargesNoTimeFromBeforeIt() throws {
        let session = ExactApp.shared.makeSession()
        defer { _ = session.sampler?.endRecording(); session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        session.sampler?.stop()
        session.sampler = nil
        session.apply(wireBatch([["op": "command", "name": "startTrace_EXPERIMENTAL", "args": []]]))
        let sampler = try XCTUnwrap(session.sampler, "the command made one")
        #if canImport(UIKit)
        XCTAssertNotNil(sampler.recording, "and started its recording")
        #endif
        // On macOS the window is not shown, so the recording is refused: the sampler the command made still times what follows
        let p = 1.0 / 60, t = ExactEnv.t0 + 40
        sampler.observe(now: t, target: t + p)
        sampler.observe(now: t + p, target: t + 2 * p)
        let first = try XCTUnwrap((sampler.reply(all: true)["records"] as? [[String: Any]])?.last)
        // A later batch the turn applies is the sampler's own, and short; the media clock since boot is not
        XCTAssertLessThan(try XCTUnwrap(first["apply"] as? Double), 100)
    }

    /// A link that changes rate (a recording asking for the display's full
    /// rate, ProMotion lowering it) holds each interval to the period the
    /// callback before it announced: the change misses no frame, and a frame
    /// shown again at the new rate is still one.
    func testARateChangeMissesNoFrame() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        let slow = 1.0 / 60, fast = 1.0 / 120
        var t = ExactEnv.t0 + 60
        sampler.observe(now: t, target: t + slow)
        t += slow; sampler.observe(now: t, target: t + fast)      // from here at 120 Hz
        t += fast; sampler.observe(now: t, target: t + slow)      // back to 60 Hz
        t += slow; sampler.observe(now: t, target: t + fast)
        let steady = try XCTUnwrap(sampler.reply()["lifetime"] as? [String: Int])
        XCTAssertEqual(steady["missed"], 0)
        XCTAssertEqual(steady["late"], 0)
        t += 2 * fast; sampler.observe(now: t, target: t + fast)  // one 120 Hz frame shown again
        XCTAssertEqual((sampler.reply()["lifetime"] as? [String: Int])?["missed"], 1)
    }

    /// The batch that lets the session's sampler go (a production bake's
    /// recording its command stopped) is not told to that sampler, which
    /// would start its display link again.
    func testABatchIsNotToldToTheSamplerItLetGo() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        sampler.stop()
        // The boot's batches go to a frame of their own first
        let p = 1.0 / 60, t = ExactEnv.t0 + 90
        sampler.observe(now: t, target: t + p)
        sampler.observe(now: t + p, target: t + 2 * p)
        #if canImport(UIKit)
        let segments = { (sampler.reply()["lifetime"] as? [String: Int])?["segments"] }
        let before = segments()
        #endif
        session.presenter.onCommand = { [unowned session] _, _, _ in session.sampler = nil }
        session.apply(wireBatch([["op": "command", "name": "stopTrace_EXPERIMENTAL", "args": []]]))
        XCTAssertNil(session.sampler)
        #if canImport(UIKit)
        XCTAssertEqual(segments(), before, "its display link stays stopped")
        #endif
        // The batch never reached it: no frame it samples after the boot's has one
        sampler.observe(now: t + 2 * p, target: t + 3 * p)
        sampler.observe(now: t + 3 * p, target: t + 4 * p)
        let after = try XCTUnwrap(sampler.reply(all: true)["records"] as? [[String: Any]]).filter { ($0["t"] as? Double ?? 0) > (t + p - ExactEnv.t0) * 1000 + 0.01 }
        XCTAssertFalse(after.isEmpty)
        XCTAssertEqual(after.map { $0["batches"] as? Int }, after.map { _ in 0 })
    }

    /// A production bake's sampler goes with its recording, and the host
    /// phases only with the last sampler any session keeps: letting one
    /// session's go leaves another's phases on.
    func testLettingOneSessionsSamplerGoLeavesAnothersPhasesOn() throws {
        let a = ExactApp.shared.makeSession(), b = ExactApp.shared.makeSession()
        defer { a.destroy(); b.destroy(); HostPhases.shared.on = true }
        XCTAssertNil(a.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertNil(b.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertNotNil(b.sampler)
        a.releaseSampler()
        XCTAssertNotNil(a.sampler, "a build that measures on its own keeps its sampler")
        a.releaseSampler(measured: false)
        XCTAssertNil(a.sampler)
        XCTAssertTrue(HostPhases.shared.on, "the other session's sampler still reads them")
    }

    #if os(macOS)
    /// A production bake has a sampler only while a recording runs: one that
    /// cannot start (no window shown, so no display link) leaves none.
    func testAProductionBakesSamplerGoesWhenItsRecordingCannotStart() throws {
        let measured = FrameSampler.measured
        FrameSampler.measured = false
        defer { FrameSampler.measured = measured }
        let session = ExactApp.shared.makeSession()
        defer { session.destroy(); HostPhases.shared.on = true }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertNil(session.sampler, "a bake measures nothing on its own")
        session.startTrace()
        XCTAssertNil(session.sampler)
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("startTrace_EXPERIMENTAL refused"))
    }
    #endif

    #if canImport(UIKit)
    /// A production bake's sampler goes when its recording ends and when its
    /// session is destroyed mid-recording.
    func testAProductionBakesSamplerGoesWithItsRecordingAndItsSession() throws {
        let measured = FrameSampler.measured
        FrameSampler.measured = false
        defer { FrameSampler.measured = measured }
        let a = ExactApp.shared.makeSession(), b = ExactApp.shared.makeSession()
        defer { a.destroy(); b.destroy(); HostPhases.shared.on = true }
        XCTAssertNil(a.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertNil(b.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertNil(a.sampler, "a bake measures nothing on its own")
        a.startTrace()
        XCTAssertNotNil(a.sampler?.recording)
        XCTAssertNotNil(a.stopTrace())
        XCTAssertNil(a.sampler, "the recording's end lets it go")
        b.startTrace()
        XCTAssertNotNil(b.sampler?.recording)
        b.destroy()
        XCTAssertNil(b.sampler, "so does its session's destruction")
    }

    /// A trace whose file cannot be written still goes on the clipboard, and
    /// both the journal and the compact copy say it was not saved.
    func testATraceThatCannotBeSavedSaysSo() throws {
        let directory = FrameSampler.traceDirectory
        FrameSampler.traceDirectory = directory.appendingPathComponent("no-such-directory-\(UUID().uuidString)/inside", isDirectory: true)
        defer { FrameSampler.traceDirectory = directory }
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        session.startTrace()
        let copied = try XCTUnwrap(session.stopTrace(maxBytes: 4000))
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(copied.utf8)) as? [String: Any])
        XCTAssertEqual(compact["full"] as? String, "not saved")
        let logs = session.agent(#"{"op":"logs","since":0}"#)
        XCTAssertTrue(logs.contains("the whole trace was not saved"))
        XCTAssertTrue(logs.contains(", not saved"))
    }
    #endif

    #if canImport(UIKit)
    /// A recording follows the scroller that moved last with no finger on
    /// any: the document's too, which the app's own scroll moves.
    func testARecordingFollowsTheDocumentsOwnScroll() throws {
        let session = ExactApp.shared.makeSession()
        defer { _ = session.sampler?.endRecording(); session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let page = session.presenter.viewport
        page.contentSize = CGSize(width: 390, height: 3000)
        page.setContentOffset(CGPoint(x: 0, y: 40), animated: false)
        XCTAssertTrue(sampler.recording?.scroller === page)
        XCTAssertEqual(sampler.recording?.scrollers.last?.name, "page")
    }

    /// Two scrollers of one name (two routes' pages) are two scrollers to
    /// the report: the second is told apart by a count, and keeps it.
    func testTwoScrollersOfOneNameAreToldApart() throws {
        let session = ExactApp.shared.makeSession()
        defer { _ = session.sampler?.endRecording(); session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let a = UIScrollView(), b = UIScrollView()
        for scroll in [a, b, a] { FrameSampler.scrolled(scroll, in: session, person: true) }
        XCTAssertEqual(sampler.recording?.scrollers.map(\.name), ["page", "page (2)", "page"])
        // A name an author wrote that reads like a count is not given again
        sampler.recording?.names = [:]
        sampler.recording?.given = ["page (2)"]
        for scroll in [b, a] { FrameSampler.scrolled(scroll, in: session, person: true) }
        XCTAssertEqual(sampler.recording?.scrollers.suffix(2).map(\.name), ["page", "page (3)"])
    }

    /// A frame sampled after the recording took another scroller, but before
    /// a turn committed it, is the old scroller's: its offset and phase were
    /// read from that one, so it is reported under that one's name.
    func testAFrameSampledBeforeItsTurnCommitsIsTheScrollerItWasReadFrom() throws {
        let session = ExactApp.shared.makeSession()
        defer { _ = session.sampler?.endRecording(); session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let a = UIScrollView(frame: CGRect(x: 0, y: 0, width: 100, height: 100)), b = UIScrollView(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        for scroll in [a, b] { scroll.contentSize = CGSize(width: 100, height: 1000) }
        a.contentOffset = CGPoint(x: 0, y: -50)                    // past its top: a bounce
        b.contentOffset = CGPoint(x: 0, y: 300)
        FrameSampler.scrolled(a, in: session, person: true)
        let p = 0.1
        var t = CACurrentMediaTime()
        sampler.observe(now: t, target: t + p)
        sampler.turnEnded(at: t + 0.001)                           // commits a's offset
        FrameSampler.scrolled(b, in: session, person: true)        // b taken before the next frame
        t += p; sampler.observe(now: t, target: t + p)             // read from a
        sampler.turnEnded(at: t + 0.001)                           // commits b's
        t += p; sampler.observe(now: t, target: t + p)
        let segments = try XCTUnwrap(FrameSampler.report(try XCTUnwrap(sampler.recording), identity: [:])["segments"] as? [[String: Any]])
        XCTAssertEqual(segments.first?["phase"] as? String, "bounce")
        XCTAssertEqual(segments.first?["scroller"] as? String, "page", "a's bounce, not b's")
    }

    /// A frame that waits for its UI update's commit (`byDeadline`) keeps
    /// the scroller as it stood when the frame was sampled: a turn that
    /// commits another scroller before that commit does not move the frame
    /// to it.
    func testAFrameWaitingForItsCommitKeepsTheScrollerItWasSampledWith() throws {
        let session = ExactApp.shared.makeSession()
        defer { _ = session.sampler?.endRecording(); session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        sampler.byDeadline = true
        let a = UIScrollView(frame: CGRect(x: 0, y: 0, width: 100, height: 100)), b = UIScrollView(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        for scroll in [a, b] { scroll.contentSize = CGSize(width: 100, height: 1000) }
        a.contentOffset = CGPoint(x: 0, y: -50)                    // past its top: a bounce
        b.contentOffset = CGPoint(x: 0, y: 300)
        FrameSampler.scrolled(a, in: session, person: true)
        let p = 0.1
        var t = CACurrentMediaTime()
        sampler.observe(now: t, target: t + p)
        sampler.turnEnded(at: t + 0.001)                           // commits a's offset
        sampler.committed(at: t + 0.002, deadline: t + 0.05)
        t += p; sampler.observe(now: t, target: t + p)             // read from a, open until its commit
        FrameSampler.scrolled(b, in: session, person: true)
        sampler.turnEnded(at: t + 0.001)                           // commits b's first
        sampler.committed(at: t + 0.002, deadline: t + 0.05)
        t += p; sampler.observe(now: t, target: t + p)
        sampler.committed(at: t + 0.002, deadline: t + 0.05)
        let segments = try XCTUnwrap(FrameSampler.report(try XCTUnwrap(sampler.recording), identity: [:])["segments"] as? [[String: Any]])
        XCTAssertEqual(segments.first?["phase"] as? String, "bounce")
        XCTAssertEqual(segments.first?["scroller"] as? String, "page", "a's bounce, not b's")
    }

    /// A new scroller at the address of one that was released is another
    /// scroller, with a name of its own. Whether the allocator reuses an
    /// address is not the test's to decide, so it builds the state that
    /// reuse leaves: an entry under the new scroller's identifier whose
    /// scroller is gone.
    func testANewScrollerWhereAReleasedOneWasGetsANewName() throws {
        let session = ExactApp.shared.makeSession()
        defer { _ = session.sampler?.endRecording(); session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let scroll = UIScrollView()
        sampler.recording?.names[ObjectIdentifier(scroll)] = .init(scroll: nil, name: "page")
        sampler.recording?.given = ["page"]
        FrameSampler.scrolled(scroll, in: session, person: true)
        XCTAssertEqual(sampler.recording?.scrollers.last?.name, "page (2)")
    }

    /// A recording and a held fling (`perf frames hold`) each read their own
    /// scroller: the agent's probe neither replaces the recording's, nor
    /// takes it away when the hold is released.
    func testARecordingAndAHeldFlingReadTheirOwnScrollers() throws {
        let session = ExactApp.shared.makeSession()
        defer { _ = session.sampler?.endRecording(); session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginRecording())
        let page = session.presenter.viewport
        page.contentSize = CGSize(width: 390, height: 3000)
        page.setContentOffset(CGPoint(x: 0, y: 40), animated: false)
        sampler.tracked = { (99, "t") }
        let p = 1.0 / 60
        var t = ExactEnv.t0 + 80
        sampler.observe(now: t, target: t + p)
        sampler.turnEnded(at: t + 0.001)
        t += p; sampler.observe(now: t, target: t + p)
        XCTAssertEqual(sampler.recording?.records.last?.scroll?.0, 40)
        XCTAssertEqual(((sampler.reply(all: true)["records"] as? [[String: Any]])?.last?["scroll"] as? [Any])?.first as? Double, 99, "the hold's own record")
        sampler.held = false
        sampler.turnEnded(at: t + 0.001)
        t += p; sampler.observe(now: t, target: t + p)
        XCTAssertEqual(sampler.recording?.records.last?.scroll?.0, 40, "released, the hold leaves the recording's reader")
    }

    /// A scroller that scrolls only sideways (a row of cards) is sampled
    /// along x: its offset, and past its edge, a bounce.
    func testASidewaysScrollerIsSampledAlongX() {
        let scroll = UIScrollView(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        scroll.contentSize = CGSize(width: 400, height: 100)
        scroll.contentOffset = CGPoint(x: 30, y: 0)
        XCTAssertEqual(scroll.scrollSample.0, 30)
        XCTAssertEqual(scroll.scrollSample.1, "-")
        scroll.contentOffset = CGPoint(x: -10, y: 0)
        XCTAssertEqual(scroll.scrollSample.1, "b")
    }

    /// A finger only touching a scroller is not a drag: the drag is what
    /// Apple's scroll-dragging metric times, once UIKit's pan has begun.
    func testATouchThatDoesNotDragIsNotADrag() {
        final class Touched: UIScrollView {
            var panBegan = false
            override var isTracking: Bool { true }
            override var isDragging: Bool { panBegan }
        }
        let scroll = Touched(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        scroll.contentSize = CGSize(width: 100, height: 400)
        XCTAssertEqual(scroll.scrollSample.1, "-")
        scroll.panBegan = true
        XCTAssertEqual(scroll.scrollSample.1, "t")
    }
    #endif

    /// A frame the link delivered on time is late all the same when the main
    /// thread's turn that commits it ends after the frame's target: the
    /// render server shows the last frame again.
    func testATurnPastItsFramesTargetIsAnOverrun() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        let p = 1.0 / 60, t = ExactEnv.t0 + 20
        sampler.observe(now: t, target: t + p)
        sampler.turnBegan(at: t + 0.001)
        sampler.turnEnded(at: t + 0.010)                           // within the frame
        sampler.observe(now: t + p, target: t + 2 * p)
        sampler.turnBegan(at: t + p + 0.002)
        sampler.turnEnded(at: t + 2 * p + 0.004)                   // 4 ms past its target
        sampler.observe(now: t + 2 * p, target: t + 3 * p)         // the link itself on time
        sampler.turnBegan(at: t + 2 * p + 0.017)                   // a turn that began after it
        sampler.turnEnded(at: t + 2 * p + 0.030)
        sampler.observe(now: t + 3 * p, target: t + 4 * p)
        let reply = sampler.reply()
        let lifetime = try XCTUnwrap(reply["lifetime"] as? [String: Int])
        XCTAssertEqual(lifetime["missed"], 0)
        XCTAssertEqual(lifetime["overruns"], 1)
        XCTAssertEqual(lifetime["late"], 1)
        let late = try XCTUnwrap((reply["late"] as? [[String: Any]])?.first)
        XCTAssertEqual(try XCTUnwrap(late["overrun"] as? Double), 4, accuracy: 0.05)
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("ms past the target"))

        // One turn through two callbacks, never sleeping between: the first
        // target is overrun when the second callback comes.
        let u = t + 10 * p
        sampler.observe(now: u, target: u + p, at: u + 0.0005)
        sampler.turnBegan(at: u + 0.001)
        sampler.observe(now: u + p, target: u + 2 * p, at: u + p + 0.006)
        sampler.turnEnded(at: u + p + 0.008)
        // A stop drops an overrun not yet sampled: the next segment's first
        // frames are on time.
        sampler.observe(now: u + 2 * p, target: u + 3 * p, at: u + 2 * p + 0.0005)
        sampler.turnBegan(at: u + 2 * p + 0.001)
        sampler.turnEnded(at: u + 3 * p + 0.005)
        sampler.stop()
        let v = u + 20 * p
        sampler.observe(now: v, target: v + p, at: v + 0.0005)
        sampler.observe(now: v + p, target: v + 2 * p, at: v + p + 0.0005)
        let after = try XCTUnwrap(sampler.reply()["lifetime"] as? [String: Int])
        XCTAssertEqual(after["overruns"], 2, "the turn through two callbacks, not the stopped segment's")
        let through = try XCTUnwrap((sampler.reply()["late"] as? [[String: Any]])?.last)
        XCTAssertEqual(try XCTUnwrap(through["overrun"] as? Double), 6, accuracy: 0.05)
    }

    /// With UIKit's commit deadline (iOS and tvOS 18, `committed`), a frame
    /// is late by as much as its UI update's commit was past that deadline:
    /// the commit is the frame's sampled in that update, and one in an
    /// update that sampled no frame goes to the next. A stop concludes a
    /// frame whose commit never came.
    func testACommitPastItsDeadlineIsItsOwnFramesLateness() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        sampler.byDeadline = true
        let p = 1.0 / 120, t = ExactEnv.t0 + 30
        sampler.observe(now: t, target: t + p)
        sampler.observe(now: t + p, target: t + 2 * p)
        sampler.committed(at: t + p + 0.004, deadline: t + p + 0.006)
        sampler.observe(now: t + 2 * p, target: t + 3 * p)
        sampler.committed(at: t + 2 * p + 0.009, deadline: t + 2 * p + 0.006)
        sampler.committed(at: t + 3 * p + 0.008, deadline: t + 3 * p + 0.007)
        sampler.observe(now: t + 4 * p, target: t + 5 * p)
        sampler.committed(at: t + 4 * p + 0.002, deadline: t + 4 * p + 0.006)
        sampler.observe(now: t + 5 * p, target: t + 6 * p)
        sampler.stop()
        let records = try XCTUnwrap(sampler.reply(all: true)["records"] as? [[String: Any]])
        let overruns = try records.map { try XCTUnwrap($0["overrun"] as? Double) }
        XCTAssertEqual(overruns, [0, 3, 1, 0])
        let lifetime = try XCTUnwrap(sampler.reply()["lifetime"] as? [String: Int])
        XCTAssertEqual(lifetime["overruns"], 2)
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("ms past its deadline"))
    }

    /// A frame keeps how long its UI update's commit took, and a late commit
    /// in an update that sampled no frame goes to the next frame with its
    /// time, unless that frame's own commit was later: `commit` belongs to
    /// the commit `overrun` measures.
    func testAFrameKeepsHowLongTheCommitThatMadeItLateTook() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        sampler.byDeadline = true
        let p = 1.0 / 120, t = ExactEnv.t0 + 30
        sampler.observe(now: t, target: t + p)
        // On time after a 1 ms commit
        sampler.observe(now: t + p, target: t + 2 * p)
        sampler.commitBegins(at: t + p + 0.003)
        sampler.committed(at: t + p + 0.004, deadline: t + p + 0.006)
        // 3 ms late after a 7 ms commit
        sampler.observe(now: t + 2 * p, target: t + 3 * p)
        sampler.commitBegins(at: t + 2 * p + 0.002)
        sampler.committed(at: t + 2 * p + 0.009, deadline: t + 2 * p + 0.006)
        // An update that sampled no frame commits 5 ms late after 6 ms; the next frame's own commit is on time
        sampler.commitBegins(at: t + 3 * p + 0.006)
        sampler.committed(at: t + 3 * p + 0.012, deadline: t + 3 * p + 0.007)
        sampler.observe(now: t + 4 * p, target: t + 5 * p)
        sampler.commitBegins(at: t + 4 * p + 0.001)
        sampler.committed(at: t + 4 * p + 0.002, deadline: t + 4 * p + 0.006)
        // A commit whose start went untimed
        sampler.observe(now: t + 5 * p, target: t + 6 * p)
        sampler.committed(at: t + 5 * p + 0.002, deadline: t + 5 * p + 0.006)
        sampler.observe(now: t + 6 * p, target: t + 7 * p)
        sampler.stop()
        let records = try XCTUnwrap(sampler.reply(all: true)["records"] as? [[String: Any]])
        XCTAssertEqual(records.map { $0["overrun"] as? Double }, [0, 3, 5, 0, 0])
        XCTAssertEqual(records.map { $0["commit"] as? Double }, [1, 7, 6, nil, nil])
    }

    /// A late commit is a frame shown a whole period late, or more for each
    /// further period it is late, and a frame both shown again and late in
    /// its commit counts once in the hitch estimate and in the late frames.
    func testALateCommitCountsOnceInTheHitchEstimate() throws {
        let recording = FrameSampler.Recording()
        let p = 1000.0 / 120
        func frame(_ t: Double, _ interval: Double, overrun: Double) {
            var r = FrameSampler.Record(t: t, interval: interval, missed: max(0, Int((interval / p).rounded()) - 1), seq: nil, batches: 0, apply: 0)
            r.period = p; r.overrun = overrun; r.main = 1
            recording.keep(r)
        }
        frame(8.33, p, overrun: 0)
        frame(16.67, p, overrun: 3)
        frame(33.33, p * 2, overrun: 2)
        frame(41.67, p, overrun: p + 1)
        let totals = try XCTUnwrap(FrameSampler.report(recording, identity: [:])["recording"] as? [String: Any])
        XCTAssertEqual(totals["lateFrames"] as? Int, 3)
        XCTAssertEqual(try XCTUnwrap(totals["hitchMs"] as? Double), 4 * p, accuracy: 0.01)
        XCTAssertEqual(FrameSampler.commitDelay(overrun: 2, period: p), p, accuracy: 0.001)
        XCTAssertEqual(FrameSampler.commitDelay(overrun: p + 1, period: p), 2 * p, accuracy: 0.001)

        // A frame's own commit 11 ms past a 10 ms period's deadline is two periods late, and the callback
        // gap its stall left in the next frame is the same stall
        func hitch(_ frames: [(t: Double, interval: Double, overrun: Double, own: Double)]) throws -> Double {
            let recording = FrameSampler.Recording()
            for f in frames {
                var r = FrameSampler.Record(t: f.t, interval: f.interval, missed: Int(f.interval / 10) - 1, seq: nil, batches: 0, apply: 0)
                r.period = 10; r.overrun = f.overrun; r.ownOverrun = f.own
                recording.keep(r)
            }
            let totals = try XCTUnwrap(FrameSampler.report(recording, identity: [:])["recording"] as? [String: Any])
            return try XCTUnwrap(totals["hitchMs"] as? Double)
        }
        XCTAssertEqual(try hitch([(10, 10, 11, 11), (30, 20, 0, 0)]), 20, accuracy: 0.01)
        // Without an update link, a turn's overrun is the next callback's, whose gap already holds it: two
        // separate stalls count in full
        XCTAssertEqual(try hitch([(20, 20, 5, 0), (50, 30, 0, 0)]), 30, accuracy: 0.01)
        // A gap after an interruption is not the stall before it
        XCTAssertEqual(try hitch([(10, 10, 11, 11), (60, 20, 0, 0)]), 30, accuracy: 0.01)
        // A frame that inherited an 11 ms commit from an update that sampled none, and whose own commit was
        // 1 ms late, carries only its own commit's period into the next gap: 20 ms, then 10
        XCTAssertEqual(try hitch([(30, 30, 11, 1), (60, 30, 0, 0)]), 30, accuracy: 0.01)
        // An update that sampled none, after the stall, missed its own deadline: its delay is not the
        // stall's, and counts in full in the next frame
        XCTAssertEqual(try hitch([(10, 10, 11, 11), (40, 30, 1, 0)]), 30, accuracy: 0.01)
    }

    /// Late frames carry their part of the hitch estimate, and the compact
    /// copy lists those late the longest by it first: a callback gap a late
    /// commit's stall left counts nothing of its own.
    func testACompactCopyOrdersLateFramesByTheirHitch() throws {
        let recording = FrameSampler.Recording()
        for (t, interval, overrun, own, screen) in [(10.0, 10.0, 21.0, 21.0, 0.0), (40.0, 30.0, 0.0, 0.0, 0.0), (50.0, 10.0, 5.0, 0.0, 0.0), (60.0, 10.0, 0.0, 0.0, 20.0)] {
            var r = FrameSampler.Record(t: t, interval: interval, missed: Int(interval / 10) - 1, seq: nil, batches: 0, apply: 0)
            r.period = 10; r.overrun = overrun; r.ownOverrun = own
            if screen > 0 { r.screenLate = screen; r.probed = true }
            recording.keep(r)
        }
        let report = FrameSampler.report(recording, identity: [:])
        let late = try XCTUnwrap(report["late"] as? [[String: Any]])
        XCTAssertEqual(late.map { $0["hitchMs"] as? Double }, [30, 0, 10, 20])
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 8000)) as? [String: Any])
        let columns = try XCTUnwrap(compact["lateColumns"] as? [String]), rows = try XCTUnwrap(compact["late"] as? [[Any]])
        let at = try XCTUnwrap(columns.firstIndex(of: "t"))
        XCTAssertEqual(rows.map { $0[at] as? Double }, [10, 60, 50, 40])
    }

    /// A late frame in a scroll carries its scroller's offset and its
    /// commit time, in the report and in its compact copy; the offset alone,
    /// as `phase` names the scroll's phase.
    func testALateFrameCarriesItsOffsetAndCommitTime() throws {
        let recording = FrameSampler.Recording()
        for (i, overrun) in [0.0, 4].enumerated() {
            var r = FrameSampler.Record(t: Double(i + 1) * 10, interval: 10, missed: 0, seq: nil, batches: 0, apply: 0)
            r.period = 10; r.overrun = overrun; r.ownOverrun = overrun; r.commit = 6
            r.scroll = (120.5 + Double(i), "d")
            recording.keep(r)
        }
        let report = FrameSampler.report(recording, identity: [:])
        let late = try XCTUnwrap(report["late"] as? [[String: Any]])
        XCTAssertEqual(late.count, 1)
        XCTAssertEqual(late.first?["offset"] as? Double, 121.5)
        XCTAssertEqual(late.first?["commit"] as? Double, 6)
        XCTAssertNil(late.first?["scroll"])
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 8000)) as? [String: Any])
        let columns = try XCTUnwrap(compact["lateColumns"] as? [String]), row = try XCTUnwrap((compact["late"] as? [[Any]])?.first)
        XCTAssertEqual(row[try XCTUnwrap(columns.firstIndex(of: "offset"))] as? Double, 121.5)
        XCTAssertEqual(row[try XCTUnwrap(columns.firstIndex(of: "commit"))] as? Double, 6)
    }

    /// A trace writes each number in its shortest form, the values unchanged.
    func testATraceWritesEachNumberInItsShortestForm() throws {
        let report: [String: Any] = ["periodMs": 8.33, "rows": [[1391.11, -0.67, 3, true]], "nested": ["ms": 58.31]]
        let data = try XCTUnwrap(FrameSampler.traceJSON(report))
        XCTAssertEqual(String(decoding: data, as: UTF8.self), #"{"nested":{"ms":58.31},"periodMs":8.33,"rows":[[1391.11,-0.67,3,true]]}"#)
    }

    /// The presentation probe's reports (`Recording.shown`): a frame shown
    /// within half a period of when UIKit expected it is probed and on time,
    /// one shown later is late on screen, one never shown is unshown and late
    /// by a period, and a frame late both in its commit and on screen counts
    /// once. A frame late only by the probe gets what ran in its interval
    /// then.
    func testTheProbesReportsMarkFramesLateOnScreen() throws {
        let recording = FrameSampler.Recording()
        let p = 1000.0 / 120
        for (i, overrun) in [0.0, 0, 3, 0].enumerated() {
            var r = FrameSampler.Record(t: Double(i + 1) * p, interval: p, missed: 0, seq: nil, batches: 0, apply: 0)
            r.period = p; r.overrun = overrun; r.main = 1
            recording.keep(r)
            recording.awaiting[i + 1] = (i, 100 + Double(i))
        }
        var asked: [(Double, Double)] = []
        let timings = { (from: CFTimeInterval, to: CFTimeInterval) -> (phases: [String: Any]?, hatches: [String: Any]?) in
            asked.append((from, to))
            return (["phases": [["phase": "apply", "ms": 2.0]]], nil)
        }
        recording.shown(drawable: 1, presented: 100.003, timings: timings)
        recording.shown(drawable: 2, presented: 101.010, timings: timings)
        recording.shown(drawable: 3, presented: 102.0085, timings: timings)
        recording.shown(drawable: 4, presented: 0, timings: timings)
        recording.shown(drawable: 5, presented: 104, timings: timings)
        let records = recording.records
        XCTAssertEqual(asked.count, 2, "the frames late on screen or unshown and late by nothing else")
        XCTAssertEqual(try XCTUnwrap(asked.first?.1), ExactEnv.t0 + 2 * p / 1000, accuracy: 1e-9)
        XCTAssertEqual(try XCTUnwrap(asked.first?.0), ExactEnv.t0 + p / 1000, accuracy: 1e-9)
        XCTAssertNotNil(records[1].phases)
        XCTAssertNil(records[2].phases, "a late commit's frame kept what it had when it was sampled")
        XCTAssertEqual(records.map(\.probed), [true, true, true, true])
        XCTAssertEqual(records.map(\.screenLate), [nil, 10, 8.5, nil])
        XCTAssertEqual(records.map(\.unshown), [false, false, false, true])
        XCTAssertTrue(recording.awaiting.isEmpty)
        let totals = try XCTUnwrap(FrameSampler.report(recording, identity: [:])["recording"] as? [String: Any])
        XCTAssertEqual(totals["probed"] as? Int, 4)
        XCTAssertEqual(totals["unshown"] as? Int, 1)
        XCTAssertEqual(totals["lateFrames"] as? Int, 3)
        // 10 ms late on screen, then 8.5 ms, which outweighs the late commit's one period, then a period unshown
        XCTAssertEqual(try XCTUnwrap(totals["hitchMs"] as? Double), 18.5 + p, accuracy: 0.01)
        // Two frames UIKit expects on screen at the same time each hear of their own drawable
        let same = FrameSampler.Recording()
        for i in 0..<2 {
            var r = FrameSampler.Record(t: Double(i + 1) * p, interval: p, missed: 0, seq: nil, batches: 0, apply: 0)
            r.period = p
            same.keep(r)
            same.awaiting[i + 1] = (i, 200)
        }
        same.shown(drawable: 2, presented: 200.020, timings: timings)
        same.shown(drawable: 1, presented: 200.001, timings: timings)
        XCTAssertEqual(same.records.map(\.screenLate), [nil, 20])
    }

    /// Each late frame of a passive recording names its cause: its update's
    /// own work past the deadline (`update`), a main thread busy before the
    /// update began or through a gap (`main`), a gap in which the scroller
    /// moved while the main thread was idle (`rate`), or a frame the probe saw
    /// reach the screen late or never (`render`). A quiet gap owed no frame:
    /// it is not late, and counts one period of the recording's time.
    func testALateFrameNamesItsCause() throws {
        let recording = FrameSampler.Recording()
        recording.scrollers = [.init(name: "list", from: 0)]
        let p = 1000.0 / 120
        var y = 0.0
        // Far apart, so no gap follows another frame's commit
        func frame(_ n: Int, interval: Double = p, overrun: Double = 0, start: Double = 0.5, work: Double = 1, main: Double = 1, unshown: Bool = false, quiet: Bool = false) {
            var r = FrameSampler.Record(t: Double(n) * 1000, interval: interval, missed: max(0, Int((interval / p).rounded()) - 1), seq: nil, batches: 0, apply: 0)
            r.period = p; r.overrun = overrun; r.ownOverrun = overrun; r.commit = 0.5; r.main = main; r.unshown = unshown; r.quiet = quiet; r.probed = true
            r.update = (start, 0.1, 0.2, work, 0.05)
            y += 10
            r.scroll = (y, "d"); r.taken = 0
            recording.keep(r)
        }
        frame(1)
        frame(2, overrun: 10, work: 11)
        frame(3, overrun: 3, start: 10)
        frame(4, interval: 2 * p, main: 15)
        frame(5, interval: 2 * p, main: 1)
        frame(6, unshown: true)
        frame(7, interval: 10 * p, main: 0.5, quiet: true)
        // A frame at 80 Hz with the main thread free for most of its gap: the system's rate
        frame(8, interval: 1.5 * p, main: 1.5)
        let report = FrameSampler.report(recording, identity: [:])
        let late = try XCTUnwrap(report["late"] as? [[String: Any]])
        XCTAssertEqual(late.map { $0["cause"] as? String }, ["update", "main", "main", "rate", "render", "rate"])
        let totals = try XCTUnwrap(report["recording"] as? [String: Any])
        XCTAssertEqual(totals["causes"] as? [String: Int], ["update": 1, "main": 2, "rate": 2, "render": 1])
        XCTAssertEqual(totals["lateFrames"] as? Int, 6)
        XCTAssertEqual(totals["missed"] as? Int, 3, "a quiet gap misses nothing")
        XCTAssertEqual(try XCTUnwrap(totals["ms"] as? Double), 10.5 * p, accuracy: 0.01, "and stands for one period")
        XCTAssertEqual(totals["hz"] as? Double, (8 / (10.5 * p / 1000)).rounded())
        XCTAssertEqual(totals["ownMeanMs"] as? Double, 0.05)
        XCTAssertEqual(totals["maxWorkMs"] as? Double, 11)
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 8000)) as? [String: Any])
        let columns = try XCTUnwrap(compact["lateColumns"] as? [String]), rows = try XCTUnwrap(compact["late"] as? [[Any]])
        let cause = try XCTUnwrap(columns.firstIndex(of: "cause")), work = try XCTUnwrap(columns.firstIndex(of: "work"))
        XCTAssertEqual(rows.first?[cause] as? String, "update", "the longest first")
        XCTAssertEqual(rows.first?[work] as? Double, 11)
    }

    func testACompactLateFrameKeepsItsQuietGapFlag() throws {
        let recording = FrameSampler.Recording()
        var frame = FrameSampler.Record(t: 1000, interval: 1000, missed: 119, seq: nil, batches: 1, apply: 0)
        frame.period = 1000.0 / 120; frame.quiet = true; frame.overrun = 1
        recording.keep(frame)
        let data = FrameSampler.compact(FrameSampler.report(recording, identity: [:]), file: nil, bytes: 1, maxBytes: 8000)
        let copy = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let columns = try XCTUnwrap(copy["lateColumns"] as? [String]), rows = try XCTUnwrap(copy["late"] as? [[Any]])
        let quiet = try XCTUnwrap(columns.firstIndex(of: "quiet"))
        XCTAssertEqual(rows.first?[quiet] as? Bool, true, "a late commit after rest must not make the idle interval look like missed display work")
    }

    /// A compact copy keeps its segments, then its worst late frames, then
    /// the render inventory's rows, then more late frames, as its budget allows.
    func testACompactCopyKeepsItsWorstLateFramesBeforeTheRenderInventory() throws {
        let recording = FrameSampler.Recording()
        let p = 1000.0 / 120
        for i in 0..<40 {
            var r = FrameSampler.Record(t: Double(i) * 1000, interval: 2 * p, missed: 1, seq: nil, batches: 0, apply: 0)
            r.period = p; r.main = 12
            recording.keep(r)
        }
        let rows: [[Any]] = (0..<200).map { ["backdrop", "content", "card-\($0)", Double($0 * 100), 80.0, 2] }
        let report = FrameSampler.report(recording, identity: [:], render: ["layers": 900, "rows": rows])
        let whole = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 100_000)) as? [String: Any])
        XCTAssertEqual((whole["render"] as? [String: Any])?["shown"] as? String, "200 of 200")
        XCTAssertEqual((whole["late"] as? [Any])?.count, 40)
        let tight = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 6000)) as? [String: Any])
        let late = try XCTUnwrap(tight["late"] as? [Any]), render = try XCTUnwrap(tight["render"] as? [String: Any])
        XCTAssertEqual(late.count, FrameSampler.compactLateFirst, "the worst late frames first")
        XCTAssertTrue((1..<200).contains(try XCTUnwrap(render["rows"] as? [Any]).count), "then as many rows as fit")
        XCTAssertLessThanOrEqual(FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 6000).count, 6000)
    }

    /// A trace carries data without explanations: neither copy has a legend
    /// or a timebase, and what each shows of all there was is counts alone.
    /// "Frame trace fields" in docs/contract-grammar.md says what each field
    /// means.
    func testATraceCarriesItsFieldsWithoutExplanations() throws {
        let recording = FrameSampler.Recording()
        for (i, interval) in [10.0, 20, 10].enumerated() {
            var r = FrameSampler.Record(t: [10.0, 30, 40][i], interval: interval, missed: Int(interval / 10) - 1, seq: nil, batches: 0, apply: 0)
            r.period = 10
            recording.keep(r)
        }
        let report = FrameSampler.report(recording, identity: [:])
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 8000)) as? [String: Any])
        for (name, copy) in [("the report", report), ("the compact copy", compact)] {
            XCTAssertNil(copy["legend"], name)
            XCTAssertNil(copy["timebase"], name)
        }
        XCTAssertEqual(report["lateShown"] as? String, "1 of 1")
        XCTAssertEqual(compact["shown"] as? String, "0 of 0 scroll segments, 1 of 1 late frames")
    }

    /// A recording with more late frames than the report keeps keeps those
    /// with the most `hitchMs`, in time order, and both copies count them
    /// against all the late frames: at a 10 ms period, 150 frames whose
    /// commit was 1 ms late (10 ms each) outrank 100 shown again for 5 ms.
    func testAReportKeepsTheLateFramesWithTheMostHitch() throws {
        let recording = FrameSampler.Recording()
        for i in 0..<250 {
            let shownAgain = i % 5 < 2
            // Far apart, so no frame's gap follows another's commit
            var r = FrameSampler.Record(t: Double(i) * 100, interval: shownAgain ? 15 : 10, missed: shownAgain ? 1 : 0, seq: nil, batches: 0, apply: 0)
            r.period = 10
            if !shownAgain { r.overrun = 1; r.ownOverrun = 1 }
            recording.keep(r)
        }
        let report = FrameSampler.report(recording, identity: [:])
        XCTAssertEqual(report["lateShown"] as? String, "200 of 250")
        let late = try XCTUnwrap(report["late"] as? [[String: Any]])
        let hitches = late.compactMap { $0["hitchMs"] as? Double }
        XCTAssertEqual(hitches.filter { $0 == 10 }.count, 150, "every frame late in its commit")
        XCTAssertEqual(hitches.filter { $0 == 5 }.count, 50)
        let times = late.compactMap { $0["t"] as? Double }
        XCTAssertEqual(times, times.sorted(), "in time order")
        let compact = try XCTUnwrap(try JSONSerialization.jsonObject(with: FrameSampler.compact(report, file: nil, bytes: 1, maxBytes: 4000)) as? [String: Any])
        let shown = try XCTUnwrap(compact["shown"] as? String)
        XCTAssertTrue(shown.hasSuffix(" of 250 late frames"), shown)
    }

    #if os(iOS) && !targetEnvironment(macCatalyst)
    /// On iOS 18 a passive update link judges each frame by UIKit's commit
    /// deadline, from the first activity of a segment once its view is in a
    /// window scene: an update whose commit finishes past it makes the frame
    /// sampled in that update late, not the next one, and a turn that stays
    /// awake past a target after an on-time commit makes none late.
    func testAnUpdateCommittedPastItsDeadlineMakesItsOwnFrameLate() throws {
        guard #available(iOS 18.0, *) else { throw XCTSkip("n/a: UIUpdateLink is iOS 18") }
        guard let scene = hostedWindowScene() else { throw missing("scene", "a UI update link observes a window scene's updates") }
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        let session = ExactApp.shared.makeSession()
        defer { session.sampler?.endObservation(); session.destroy(); window.isHidden = true }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        XCTAssertTrue(sampler.beginObservation())
        XCTAssertFalse(sampler.byDeadline, "a view in no window has no update link")
        session.presenter.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(session.presenter.viewport)
        sampler.activity()
        XCTAssertTrue(sampler.byDeadline, "the segment's next activity reads UIKit's deadlines once the view is in a window scene")
        // A link of the test's own updates every frame, and stalls one commit or one turn on request
        enum Stall { case none, commit, turn }
        var stall = Stall.none, stalled: Int?
        let driver = UIUpdateLink(view: session.presenter.viewport)
        driver.addAction(to: .beforeCATransactionCommit) { _, info in
            guard stall == .commit else { return }
            stall = .none; stalled = sampler.observedCount
            Thread.sleep(forTimeInterval: max(0, info.completionDeadlineTime - CACurrentMediaTime()) + 0.012)
        }
        driver.addAction(to: .afterUpdateComplete) { _, info in
            guard stall == .turn else { return }
            stall = .none; stalled = sampler.observedCount
            Thread.sleep(forTimeInterval: max(0, info.estimatedPresentationTime - CACurrentMediaTime()) + 0.012)
        }
        driver.requiresContinuousUpdates = true
        driver.isEnabled = true
        defer { driver.isEnabled = false }
        func overrun(after count: Int, _ index: Int) throws -> Double {
            let records = try XCTUnwrap(sampler.observation(from: 0, before: 0)["records"] as? [[String: Any]])
            return try XCTUnwrap(records[count + index]["overrun"] as? Double)
        }
        until("frames come") { sampler.observedCount > 3 }

        stall = .commit
        until("the stalled commit's frame and the next are kept") { stalled.map { sampler.observedCount > $0 + 2 } ?? false }
        let late = try XCTUnwrap(stalled)
        XCTAssertGreaterThan(try overrun(after: late, 0), 8, "the frame sampled in the stalled update is late")
        XCTAssertEqual(try overrun(after: late, 1), 0, "the next frame's commit was on time")

        stalled = nil
        stall = .turn
        until("the frame before the awake turn and the next are kept") { stalled.map { sampler.observedCount > $0 + 2 } ?? false }
        let awake = try XCTUnwrap(stalled)
        XCTAssertEqual(try overrun(after: awake, 0), 0, "its commit was on time, however long the turn ran after it")
    }

    /// Run the main run loop until `done`, for at most five seconds.
    private func until(_ what: String, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(5)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    #endif

    /// LLP 1075.003.000.001 §3.1: a late frame names the hatch calls that
    /// overlapped its interval, each charged its overlap; an on-time one names none.
    func testALateFrameNamesTheHatchesThatRanInIt() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler, "a development build samples")
        let p = 1.0 / 60
        var t = CACurrentMediaTime()
        sampler.observe(now: t, target: t + p)
        session.hatchDiagnostics.timed("element avatar", "built", counts: false) { Thread.sleep(forTimeInterval: 0.03) }
        session.hatchDiagnostics.timed("element avatar", "changed", counts: false) { Thread.sleep(forTimeInterval: 0.005) }
        t = CACurrentMediaTime()
        sampler.observe(now: t, target: t + p)                       // 35 ms and more since the last: late
        sampler.observe(now: t + p, target: t + 2 * p)               // on time, and no call in it
        let late = try XCTUnwrap(sampler.reply()["late"] as? [[String: Any]])
        XCTAssertEqual(late.count, 1)
        let ran = try XCTUnwrap(late[0]["hatches"] as? [[String: Any]])
        XCTAssertEqual(ran.count, 1)
        XCTAssertEqual(ran[0]["hatch"] as? String, "element avatar")
        XCTAssertEqual(ran[0]["calls"] as? Int, 2)
        XCTAssertGreaterThanOrEqual(try XCTUnwrap(ran[0]["ms"] as? Double), 35)
        XCTAssertNil(late[0]["coverage"], "every call in the window is still kept")
        // A window before any call names none; one that cuts a call charges the part inside it.
        XCTAssertNil(session.hatchDiagnostics.window(from: t - 100, to: t - 99))
        let half = try XCTUnwrap(session.hatchDiagnostics.window(from: t - 0.010, to: t)?["hatches"] as? [[String: Any]])
        XCTAssertLessThanOrEqual(try XCTUnwrap(half[0]["ms"] as? Double), 10.01)
    }

    func testALateFrameIsCountedAgainstTheTargetPeriodJournaledAndSaved() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler, "a development build samples")
        let p = 1.0 / 120, t = ExactEnv.t0 + 10
        sampler.observe(now: t, target: t + p)                     // the baseline, never a sample
        sampler.observe(now: t + p, target: t + 2 * p)             // on time
        sampler.batch((5, 6), ms: 2.5)
        sampler.observe(now: t + 5 * p, target: t + 6 * p)         // three frames missed
        let reply = sampler.reply()
        let lifetime = try XCTUnwrap(reply["lifetime"] as? [String: Int])
        XCTAssertEqual(lifetime["presented"], 2)
        XCTAssertEqual(lifetime["late"], 1)
        XCTAssertEqual(lifetime["missed"], 3)
        let late = try XCTUnwrap((reply["late"] as? [[String: Any]])?.first)
        XCTAssertEqual(late["missed"] as? Int, 3)
        XCTAssertEqual(late["seq"] as? [UInt64], [5, 6])
        XCTAssertEqual(late["apply"] as? Double, 2.5)
        XCTAssertEqual((reply["period"] as? [String: Any])?["source"] as? String, "target")
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("frame late at "), "a late frame is one journal line")

        // What a hatch recorded is in the trace's `hatches`, as `state` and `perf hatches` give it (LLP 1075.003.000.001 §3.3).
        _ = session.hatchDiagnostics.record(kind: 1, node: 0, scope: "module", name: "swipes", value: 3, text: Data())
        let url = try session.saveTrace().get()
        defer { try? FileManager.default.removeItem(at: url) }
        let trace = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        for key in ["identity", "proxies", "plan", "journal", "frames", "perf", "hatches"] { XCTAssertNotNil(trace[key], key) }
        let hatches = try XCTUnwrap(trace["hatches"] as? [String: [String: Any]])
        XCTAssertEqual(((hatches["perf"]?["counters"] as? [String: Any])?["module"] as? [String: Int])?["swipes"], 3)
        XCTAssertEqual((((hatches["state"]?["scopes"] as? [String: Any])?["module"] as? [String: Any])?["counters"] as? [String: Int])?["swipes"], 3)
        XCTAssertEqual(hatches["perf"]?["plan"] as? String, (trace["perf"] as? [String: Any])?["plan"] as? String, "the hatches' read names the plan `perf` does")
        XCTAssertEqual((trace["frames"] as? [String: Any])?["records"].map { ($0 as? [Any])?.count }, 2)
        XCTAssertNotNil((trace["perf"] as? [String: Any])?["sites"], "the runner measures in a development build")
        // The last one also under the name a phone's is copied off by
        // (`agent.mjs trace --phone` reads this literal name), replaced by
        // the next.
        let latest = FileManager.default.temporaryDirectory.appendingPathComponent("trace-latest.json")
        let first = try Data(contentsOf: url)
        XCTAssertEqual(try Data(contentsOf: latest), first)
        session.log("between the traces")
        let next = try session.saveTrace().get()
        defer { try? FileManager.default.removeItem(at: next) }
        XCTAssertEqual(try Data(contentsOf: latest), try Data(contentsOf: next))
        XCTAssertNotEqual(try Data(contentsOf: latest), first)
    }

    /// A release build's sessions count their sites' work from the first ask
    /// (LLP 1079 D1), and one made after it, a second window, counts from its
    /// boot: making it does not undo the ask.
    func testASessionMadeAfterTheFirstAskCountsItsWorkFromBoot() throws {
        let first = ExactApp.shared.makeSession()
        defer { first.runtime.measureWork(); first.destroy() }
        XCTAssertNil(FrameSampler.fromBoot(first, atBoot: false), "a release build samples nothing at boot")
        XCTAssertNil(first.boot(size: CGSize(width: 390, height: 844)).error)
        first.runtime.measureWork()
        let second = ExactApp.shared.makeSession()
        defer { second.destroy() }
        XCTAssertNil(FrameSampler.fromBoot(second, atBoot: false))
        XCTAssertNil(second.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertTrue(second.agent(#"{"op":"perf"}"#).contains("\"sites\""), "the second session counts from boot")
    }

    /// A release build measures nothing until something asks (LLP 1079):
    /// its first trace still has the journal and the sites' work, its frames
    /// and hatch timings empty and saying so, and starts the measuring that
    /// the next trace and the agent's `perf` read.
    func testAReleaseBuildsFirstTraceStartsItsMeasuring() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.runtime.measureWork(); session.destroy(); HatchDiagnostics.measuring = true }
        // As a release build boots: no site's work counted, no sampler, no hatch timing.
        exact_measure_work_later()
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertTrue(session.agent(#"{"op":"perf"}"#).contains("not measured until first asked"), "no site's work is counted")
        session.sampler?.stop()
        session.sampler = nil
        HatchDiagnostics.measuring = false
        let probes = { (session.hatchPerf()["calls"] as? [[String: Any]] ?? []).filter { $0["hatch"] as? String == "probe" }.count }
        session.natives.timedHatch("probe", "tick") {}
        XCTAssertEqual(probes(), 0, "no call is timed")
        session.log("before the trace")
        let url = try session.saveTrace().get()
        defer { try? FileManager.default.removeItem(at: url) }
        let trace = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        let frames = try XCTUnwrap(trace["frames"] as? [String: Any])
        XCTAssertEqual(frames["unmeasured"] as? String, FrameSampler.unmeasured)
        XCTAssertEqual((frames["records"] as? [Any])?.count, 0)
        XCTAssertEqual((trace["hatches"] as? [String: [String: Any]])?["perf"]?["measuring"] as? Bool, false)
        XCTAssertTrue("\(trace["journal"] ?? "")".contains("before the trace"), "the journal is kept")
        XCTAssertNotNil((trace["perf"] as? [String: Any])?["sites"], "the sites' work is kept")
        XCTAssertNotNil(session.sampler, "the trace started the frames")
        XCTAssertTrue(HatchDiagnostics.measuring, "and the hatch timing")
        session.natives.timedHatch("probe", "tick") {}
        XCTAssertEqual(probes(), 1)
    }
}

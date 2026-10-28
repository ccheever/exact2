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

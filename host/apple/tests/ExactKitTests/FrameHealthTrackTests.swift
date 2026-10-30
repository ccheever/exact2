import XCTest
import QuartzCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
@testable import ExactCore
@testable import ExactKit

/// `perf frames track` (exact.frame-health.track/1): an agent's bounded, leased scroll track, fed only from the
/// host's existing scroll callbacks, read in pages that fit #1141's 4000-byte envelope.
final class FrameHealthTrackTests: XCTestCase {
    private func pages(_ track: FrameHealthTrack, from cursor: String? = nil) throws -> [[String: Any]] {
        var out: [[String: Any]] = [], next = cursor
        for _ in 0..<200 {
            let raw = try XCTUnwrap(track.page(from: next.flatMap { UInt64($0) }, lease: track.began)["raw"] as? String)
            XCTAssertLessThanOrEqual(raw.utf8.count + 1, FrameHealthTrack.maximumBytes, "a page and its newline fit 4000 bytes")
            let page = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(raw.utf8)) as? [String: Any])
            out.append(page)
            next = page["cursor"] as? String
            if page["more"] as? Bool != true { break }
        }
        return out
    }

    #if canImport(UIKit)
    private func scroller() -> UIScrollView { UIScrollView(frame: CGRect(x: 0, y: 0, width: 100, height: 100)) }
    #else
    private func scroller() -> ChainingScrollView { ChainingScrollView(frame: CGRect(x: 0, y: 0, width: 100, height: 100)) }
    #endif

    func testTheTrackIsOffByDefaultAndAllocatesOnlyWhenLeased() {
        let session = ExactApp.shared.makeSession(label: "track-off")
        defer { session.destroy() }
        XCTAssertNil(session.frameHealth.track, "no track unless an agent leases one")
        let scroll = scroller()
        for _ in 0..<100 { session.frameHealth.trackScrolled(scroll) }
        XCTAssertNil(session.frameHealth.track, "the feed with no track is a nil check")
    }

    func testTheRingKeeps4096RowsAndPagesLoseAndRepeatNone() throws {
        let track = FrameHealthTrack(node: 7, scroller: scroller(), now: 100, durationMs: 30_000, maximumFramesPerSecond: 120)
        for i in 0..<5000 { track.record(Double(i), phase: i % 2 == 0 ? "t" : "d", at: 100 + Double(i) / 120) }
        let all = try pages(track)
        XCTAssertGreaterThan(all.count, 1, "paged")
        let rows = all.flatMap { $0["rows"] as? [[Any]] ?? [] }
        XCTAssertEqual(rows.count, FrameHealthTrack.capacity, "the retained rows, each once")
        let times = rows.compactMap { $0.first as? Int }
        XCTAssertEqual(times, times.sorted(), "oldest first")
        XCTAssertEqual(Set(times).count, times.count, "none repeated across pages")
        XCTAssertEqual(rows.first?[3] as? Int, 904 * 100, "the oldest retained offset, in centipoints")
        XCTAssertEqual(rows.last?[3] as? Int, 4999 * 100)
        XCTAssertEqual(all.first?["dropped"] as? Int, 904, "a read with no cursor is from row 0: the overwritten rows are counted")
        XCTAssertEqual(all.dropFirst().compactMap { $0["dropped"] as? Int }.reduce(0, +), 0, "and only once")
        let fromStart = try XCTUnwrap(try pages(track, from: "0").first)
        XCTAssertEqual(fromStart["dropped"] as? Int, 904, "the same as cursor 0")
        let retained = try XCTUnwrap(try pages(track, from: "904").first)
        XCTAssertEqual(retained["dropped"] as? Int, 0, "a cursor at the oldest retained row lost nothing")
        // A frame's interval at 120 Hz and its proxy for missed frames.
        let second = try XCTUnwrap(rows.dropFirst().first)
        XCTAssertEqual(Double(second[1] as? Int ?? 0), 1e6 / 120, accuracy: 2)
        XCTAssertEqual(second[2] as? Int, 0)
        XCTAssertEqual(rows.first?[1] as? Int, 0, "the oldest retained row has no retained predecessor")
    }

    func testMissedIsTheIntervalOverThePeriodAndAQuietGapStartsARun() throws {
        let track = FrameHealthTrack(node: 1, scroller: scroller(), now: 0, durationMs: 30_000, maximumFramesPerSecond: 60)
        for t in [1.0, 1.0 + 1 / 60, 1.0 + 4 / 60, 2.0] { track.record(t, phase: "d", at: t) }
        let rows = try XCTUnwrap(try pages(track).first?["rows"] as? [[Any]])
        XCTAssertEqual(rows.map { $0[2] as? Int }, [0, 0, 2, 0], "round(interval / period) - 1; a gap of 0.5 s or more is a new run")
        XCTAssertEqual(rows.last?[1] as? Int, 0)
        XCTAssertEqual(rows.map { $0[4] as? String }, ["d", "d", "d", "d"])
        // After a held finger or a scroller at rest, a pause is no frame lost; while decelerating, the same gap is.
        let paused = FrameHealthTrack(node: 1, scroller: scroller(), now: 0, durationMs: 30_000, maximumFramesPerSecond: 60)
        for (t, phase) in [(1.0, "t"), (1.2, "t"), (1.2 + 1 / 60, "d"), (1.4, "d"), (1.6, "-"), (1.9, "t")] as [(Double, Character)] {
            paused.record(t, phase: phase, at: t)
        }
        let pausedRows = try XCTUnwrap(try pages(paused).first?["rows"] as? [[Any]])
        XCTAssertEqual(pausedRows.map { $0[2] as? Int }, [0, 0, 0, 10, 11, 0], "t->t and -->t pauses are not missed frames; d->d and d->- gaps are")
    }

    func testOneTrackASessionReplacedByASecondStartLeasedAndRefusedWhereCaptureIs() throws {
        let health = FrameHealth()
        let a = scroller(), b = scroller()
        var reply = health.trackRequest("start", node: 1, scroller: a, durationMs: nil, cursor: nil, maximumFramesPerSecond: 60, now: 10)
        XCTAssertEqual(reply["track"] as? Bool, true, "\(reply)")
        XCTAssertEqual(reply["leaseMs"] as? Int, 30_000, "the default lease")
        XCTAssertEqual(reply["replaced"] as? Bool, false)
        reply = health.trackRequest("start", node: 2, scroller: b, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60, now: 11)
        XCTAssertEqual(reply["replaced"] as? Bool, true, "a second start replaces the first")
        XCTAssertTrue(try XCTUnwrap(health.track).tracks(b))
        XCTAssertFalse(try XCTUnwrap(health.track).tracks(a), "the first scroller is no longer read")
        // The lease ends at the first read after its deadline, with its rows; no timer.
        reply = health.trackRequest("read", node: nil, scroller: nil, durationMs: nil, cursor: nil, maximumFramesPerSecond: 60, now: 12.5)
        XCTAssertEqual(reply["reason"] as? String, "no-track")
        XCTAssertNil(health.track)
        for duration in [0.0, 120_001, 1.5, .nan] {
            XCTAssertNotNil(health.trackRequest("start", node: 1, scroller: a, durationMs: duration, cursor: nil, maximumFramesPerSecond: 60)["error"], "\(duration)")
        }
        XCTAssertEqual(health.trackRequest("start", node: 1, scroller: nil, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60)["error"] as? String, "no-scroller")
        // Stop answers the last page and releases the ring.
        _ = health.trackRequest("start", node: 1, scroller: a, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60)
        XCTAssertNotNil(health.trackRequest("stop", node: nil, scroller: nil, durationMs: nil, cursor: nil, maximumFramesPerSecond: 60)["raw"])
        XCTAssertNil(health.track)
        // Where the capture lease is refused, so is the track: collection off, or a virtual clock.
        let off = FrameHealth(enabled: false)
        XCTAssertEqual(off.trackRequest("start", node: 1, scroller: a, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60)["reason"] as? String,
                       off.setAgentCapture("start")["reason"] as? String)
        XCTAssertNil(off.track)
        health.clockChanged(virtual: true)
        XCTAssertEqual(health.trackRequest("start", node: 1, scroller: a, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60)["reason"] as? String, "virtual-clock")
        XCTAssertNil(health.track)
    }

    /// Any finite offset encodes: centipoints are clamped to JSON's safe integers, both signs.
    func testHugeFiniteOffsetsEncodeClampedBothSigns() throws {
        let track = FrameHealthTrack(node: 1, scroller: scroller(), now: 0, durationMs: 30_000, maximumFramesPerSecond: 60)
        for (i, offset) in [1e17, -1e17, Double(Float.greatestFiniteMagnitude), -Double(Float.greatestFiniteMagnitude), 1e14, -12.25].enumerated() {
            track.record(offset, phase: "d", at: Double(i) / 60)
        }
        let rows = try XCTUnwrap(try pages(track).first?["rows"] as? [[Any]])
        let limit = 9_007_199_254_740_991
        XCTAssertEqual(rows.map { $0[3] as? Int }, [limit, -limit, limit, -limit, limit, -1225])
    }

    /// The lease ends at the next FEED after its deadline too (no read needed), and the ring goes with it.
    func testALeaseEndsOnTheNextFeed() throws {
        let health = FrameHealth(), scroll = scroller()
        XCTAssertEqual(health.trackRequest("start", node: 1, scroller: scroll, durationMs: 1, cursor: nil, maximumFramesPerSecond: 60)["track"] as? Bool, true)
        let deadline = Date().addingTimeInterval(0.01)
        while Date() < deadline { RunLoop.main.run(mode: .default, before: deadline) }
        health.trackScrolled(scroll)
        XCTAssertNil(health.track, "the first feed past the lease releases it")
    }

    /// A reload's reset and a clock change each drop the track; so does tearing the session's health down, after which
    /// a feed appends nothing.
    func testResetClockChangeAndDestroyDropTheTrack() {
        let health = FrameHealth(), scroll = scroller()
        _ = health.trackRequest("start", node: 1, scroller: scroll, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60)
        health.reset()
        XCTAssertNil(health.track, "reload")
        _ = health.trackRequest("start", node: 1, scroller: scroll, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60)
        health.destroy()
        XCTAssertNil(health.track, "teardown")
        health.trackScrolled(scroll)
        XCTAssertNil(health.track)
        XCTAssertFalse(health.enabled)
    }

    /// One feed through the host's path: the scroller moves to `offset`, then its scroll callback runs.
    private func feed(_ health: FrameHealth, _ scroll: AnyObject, _ offset: Double) {
        #if canImport(UIKit)
        let sv = scroll as! UIScrollView
        sv.contentSize = CGSize(width: 100, height: 1_000_000)
        sv.contentOffset = CGPoint(x: 0, y: offset)
        health.trackScrolled(sv)
        #else
        let sv = scroll as! ChainingScrollView
        sv.contentView.setBoundsOrigin(NSPoint(x: 0, y: offset))
        health.trackScrolled(sv)
        #endif
    }

    private func read(_ health: FrameHealth, _ action: String, _ cursor: String?, now: Double = CACurrentMediaTime()) throws -> [String: Any] {
        let reply = health.trackRequest(action, node: nil, scroller: nil, durationMs: nil, cursor: cursor, maximumFramesPerSecond: 60, now: now)
        guard let raw = reply["raw"] as? String else { return reply }
        return try XCTUnwrap(JSONSerialization.jsonObject(with: Data(raw.utf8)) as? [String: Any])
    }

    /// `stop` freezes the track: no feed writes after it, its first page says `more`, the rest is read by cursor, and
    /// the read that answers `more: false` releases the ring. A tail of several pages comes back whole, `dropped` exact.
    func testAStopFreezesTheTrackAndItsTailIsReadWholeThenReleased() throws {
        for (fed, lost) in [(600, 0), (5000, 904)] {
            let health = FrameHealth(), scroll = scroller()
            _ = health.trackRequest("start", node: 1, scroller: scroll, durationMs: 60_000, cursor: nil, maximumFramesPerSecond: 60)
            for i in 1...fed { feed(health, scroll, Double(i)) }
            var page = try read(health, "stop", nil)
            XCTAssertEqual(page["more"] as? Bool, true, "a tail of several pages: \(fed)")
            XCTAssertEqual(page["stopped"] as? Bool, true)
            let written = try XCTUnwrap(health.track).written
            feed(health, scroll, 999_999)
            XCTAssertEqual(health.track?.written, written, "no feed writes after stop")
            var rows = page["rows"] as? [[Any]] ?? [], dropped = page["dropped"] as? Int ?? 0, pages = 1
            while page["more"] as? Bool == true {
                XCTAssertNotNil(health.track, "the frozen ring stays until a read answers more: false")
                page = try read(health, "read", page["cursor"] as? String)
                XCTAssertEqual(page["stopped"] as? Bool, true)
                rows += page["rows"] as? [[Any]] ?? []; dropped += page["dropped"] as? Int ?? 0; pages += 1
            }
            XCTAssertGreaterThan(pages, 2)
            XCTAssertEqual(rows.count, min(fed, FrameHealthTrack.capacity), "the whole tail")
            XCTAssertEqual(rows.last?[3] as? Int, fed * 100)
            XCTAssertEqual(dropped, lost, "dropped exact")
            XCTAssertNil(health.track, "released after more: false")
            XCTAssertEqual(try read(health, "read", nil)["reason"] as? String, "no-track")
        }
    }

    /// A frozen ring nobody reads to the end is released by its lease (on the next read, no timer) or a new start.
    func testAFrozenRingIsReleasedByItsLeaseOrANewStart() throws {
        let health = FrameHealth(), scroll = scroller(), t0 = CACurrentMediaTime()
        _ = health.trackRequest("start", node: 1, scroller: scroll, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60, now: t0)
        for i in 1...600 { health.track?.record(Double(i), phase: "d", at: t0 + Double(i) / 1000) }
        XCTAssertEqual(try read(health, "stop", nil, now: t0 + 0.7)["more"] as? Bool, true)
        XCTAssertTrue(try XCTUnwrap(health.track).frozen)
        XCTAssertEqual(try read(health, "read", "100", now: t0 + 1.5)["reason"] as? String, "no-track", "the lease ended")
        XCTAssertNil(health.track)
        _ = health.trackRequest("start", node: 1, scroller: scroll, durationMs: 60_000, cursor: nil, maximumFramesPerSecond: 60)
        for i in 1...600 { feed(health, scroll, Double(i)) }
        XCTAssertEqual(try read(health, "stop", nil)["more"] as? Bool, true)
        let reply = health.trackRequest("start", node: 2, scroller: scroll, durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60)
        XCTAssertEqual(reply["replaced"] as? Bool, true)
        XCTAssertEqual(health.track?.frozen, false, "a new start replaces the frozen ring")
        XCTAssertEqual(health.track?.written, 0)
    }

    func testAClockChangeDropsTheTrack() {
        let health = FrameHealth()
        _ = health.trackRequest("start", node: 1, scroller: scroller(), durationMs: 1000, cursor: nil, maximumFramesPerSecond: 60)
        XCTAssertNotNil(health.track)
        health.clockChanged(virtual: false)
        XCTAssertNil(health.track)
    }

    /// The agent's request: a pipe on the carrier, one JSON line back.
    private func ask(_ agent: Agent, _ request: [String: Any]) throws -> [String: Any] {
        let pipe = Pipe(), saved = Agent.out
        Agent.out = pipe.fileHandleForWriting
        defer { Agent.out = saved }
        let wire = try JSONSerialization.jsonObject(with: JSONSerialization.data(withJSONObject: request)) as? [String: Any] ?? [:]
        agent.handle(op: "perf", wire, line: "")
        return try XCTUnwrap(JSONSerialization.jsonObject(with: pipe.fileHandleForReading.availableData) as? [String: Any])
    }

    /// Where the last read stopped, as the driver keeps it, so consecutive reads don't repeat rows.
    private var cursor: String?

    private func readAll(_ agent: Agent) throws -> [[Any]] {
        var rows: [[Any]] = []
        for _ in 0..<100 {
            var request: [String: Any] = ["op": "perf", "frames": true, "track": "read"]
            if let cursor { request["cursor"] = cursor }
            let page = try ask(agent, request)
            XCTAssertEqual(page["format"] as? String, FrameHealthTrack.format, "\(page)")
            rows += page["rows"] as? [[Any]] ?? []
            cursor = page["cursor"] as? String
            if page["more"] as? Bool != true { break }
        }
        return rows
    }

    /// Through the carrier's dispatcher (`Agent.handle(_ line:)`), a track request is a bounded frame read: it never
    /// drains a slice in flight (`ExactSession.drainAll`), which would wait for the owner on main mid-glide.
    func testCarrierTrackRequestsDoNotDrainTheOwner() throws {
        let session = ExactApp.shared.makeSession(label: "track-carrier")
        defer { session.destroy() }
        session.clock = nil
        let pipe = Pipe(), saved = Agent.out, savedRoutes = Agent.routes
        Agent.out = pipe.fileHandleForWriting
        Agent.routes = [(session.label, session)]
        defer { Agent.out = saved; Agent.routes = savedRoutes }
        let entered = expectation(description: "the owner is held")
        let gate = DispatchSemaphore(value: 0)
        Owner.shared.post { entered.fulfill(); _ = gate.wait(timeout: .now() + 2) }
        wait(for: [entered], timeout: 5)
        session.tickInFlight = true
        defer { session.tickInFlight = false; gate.signal() }
        for line in ["{\"op\":\"perf\",\"frames\":true,\"track\":\"start\",\"node\":99}",
                     "{\"op\":\"perf\",\"frames\":true,\"track\":\"read\"}",
                     "{\"op\":\"perf\",\"frames\":true,\"track\":\"stop\"}"] {
            let began = CACurrentMediaTime()
            Agent.handle(line)
            XCTAssertLessThan(CACurrentMediaTime() - began, 0.5, "\(line) waited for the owner")
            XCTAssertNotNil(try? JSONSerialization.jsonObject(with: pipe.fileHandleForReading.availableData))
        }
    }

    private static let batch: [[String: Any]] = [
        ["op": "create", "id": 1, "kind": "view"],
        ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
        ["op": "create", "id": 3, "kind": "view"],
        ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
        ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
        ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 600.0],
        ["op": "content", "id": 2, "w": 400.0, "h": 4000.0],
        ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 4000.0],
    ]

    private func until(_ seconds: Double, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
    }

    #if os(iOS)
    /// Under platform timing, a scroller UIKit moves frame by frame (an animated offset, as a glide moves it) is read
    /// per committed offset through the agent: times rising, offsets moving one way, frame-spaced intervals, and no
    /// rows once it rests. A real finger's drag and momentum (`t` then `d`) needs touches the simulator's HID carries
    /// from the driver (`touch: platform`), which an XCTest host has no way to post: that is shown on a simulator with
    /// the driver instead (F1's A/B/R runs: B's slow drags ~520 `t` rows, then `d`).
    func testTheAgentReadsAScrollerFrameByFrameUnderPlatformTiming() throws {
        guard !ExactEnv.agentFreezes else { throw XCTSkip("platform timing") }
        let session = ExactApp.shared.makeSession(label: "track-ios")
        defer { session.destroy() }
        session.clock = nil
        // A development build samples from boot; the track must not start the sampler again (as #1141's capture).
        session.sampler?.stop(); session.sampler = nil
        let p = session.presenter
        guard let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first else { throw missing("scene", "a scroller animates frame by frame only in a scene's window") }
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch(Self.batch))
        p.viewport.layoutIfNeeded()
        let sv = try XCTUnwrap(p.views[2]?.scrollView)
        let agent = Agent(session: session)
        let started = try ask(agent, ["op": "perf", "frames": true, "track": "start", "node": 2, "durationMs": 10_000])
        XCTAssertEqual(started["track"] as? Bool, true, "\(started)")
        XCTAssertNil(session.sampler, "the legacy sampler stays off")
        XCTAssertEqual(try ask(agent, ["op": "perf", "frames": true, "track": "start", "node": 3])["error"] as? String, "no-scroller")
        XCTAssertTrue(session.frameHealth.track?.tracks(sv) == true, "a failed start leaves the leased track as it was")
        for node: Any in [-1, 4_294_967_296, 1e20, "2"] {
            XCTAssertEqual(try ask(agent, ["op": "perf", "frames": true, "track": "start", "node": node])["error"] as? String, "no-scroller", "\(node)")
        }
        XCTAssertTrue(session.frameHealth.track?.tracks(sv) == true, "malformed ids are refused and leave the track")
        _ = try ask(agent, ["op": "perf", "frames": true, "track": "start", "node": 2, "durationMs": 10_000])
        sv.setContentOffset(CGPoint(x: 0, y: 1500), animated: true)
        until(3) { sv.contentOffset.y >= 1499 && !sv.isDecelerating }
        until(0.3) { false }
        let rows = try readAll(agent)
        XCTAssertGreaterThanOrEqual(rows.count, 5, "frame by frame: \(rows.count) rows")
        until(0.3) { false }
        XCTAssertEqual(try readAll(agent).count, 0, "a scroller at rest writes nothing")
        let offsets = rows.compactMap { $0[3] as? Int }, times = rows.compactMap { $0[0] as? Int }
        XCTAssertEqual(times, times.sorted())
        XCTAssertEqual(offsets, offsets.sorted(), "moving one way")
        XCTAssertEqual(offsets.last, 150_000, "to where it ended, in centipoints")
        let intervals = rows.dropFirst().compactMap { $0[1] as? Int }.filter { $0 > 0 }
        XCTAssertFalse(intervals.isEmpty)
        XCTAssertLessThan(intervals.sorted()[intervals.count / 2], 50_000, "frame-spaced, not a timer's")
        XCTAssertNotNil(try ask(agent, ["op": "perf", "frames": true, "track": "stop"])["rows"])
        XCTAssertNil(session.frameHealth.track)
    }
    #endif

    #if os(macOS)
    /// A Mac scroller moved by the agent's plain wheel (phased trackpad replay is refused under platform timing) is
    /// read per committed clip origin, from the clip's existing bounds-change handler.
    func testTheAgentReadsAMacScrollerMovedByAPlainWheel() throws {
        guard !ExactEnv.agentFreezes else { throw XCTSkip("platform timing") }
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "track-mac")
        defer { session.destroy() }
        session.clock = nil
        // A development build samples from boot; the track must not start the sampler again (as #1141's capture).
        session.sampler?.stop(); session.sampler = nil
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 800)
        let window = NSWindow(contentRect: NSRect(x: 240, y: 160, width: 400, height: 800), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        window.orderFrontRegardless()
        defer { window.close() }
        p.apply(wireBatch(Self.batch))
        p.viewport.layoutSubtreeIfNeeded()
        let sv = try XCTUnwrap(p.views[2]?.scroll)
        let agent = Agent(session: session)
        let started = try ask(agent, ["op": "perf", "frames": true, "track": "start", "node": 2, "durationMs": 10_000])
        XCTAssertEqual(started["track"] as? Bool, true, "\(started)")
        XCTAssertNil(session.sampler, "the legacy sampler stays off")
        // Before any wheel: a bounds change at rest (the origin sampled at start) writes nothing.
        try XCTUnwrap(p.views[2]).clipScrolled()
        XCTAssertEqual(try readAll(agent).count, 0, "a first bounds change at rest writes no row")
        for _ in 0..<6 {
            XCTAssertNil(agent.tap(["id": 2, "wheel": [0.0, 100.0]])["error"])
            until(0.05) { false }
        }
        let rows = try readAll(agent)
        XCTAssertGreaterThanOrEqual(rows.count, 3, "\(rows)")
        let offsets = rows.compactMap { $0[3] as? Int }
        XCTAssertEqual(offsets.last, Int((sv.contentView.bounds.origin.y * 100).rounded()), "the clip's last committed origin")
        XCTAssertNotEqual(offsets.first, offsets.last, "it moved")
        XCTAssertEqual(offsets, offsets.sorted(), "one way")
        XCTAssertEqual(Set(rows.compactMap { $0[4] as? String }), ["-"], "a plain wheel has no phase")
        // A resize at rest posts a bounds change with the origin where it was: nothing is written.
        let origin = sv.contentView.bounds.origin
        sv.setFrameSize(NSSize(width: sv.frame.width - 50, height: sv.frame.height))
        // The clip's bounds-change handler, as AppKit calls it for a size change, with the origin unmoved.
        try XCTUnwrap(p.views[2]).clipScrolled()
        until(0.05) { false }
        XCTAssertEqual(sv.contentView.bounds.origin, origin, "the resize left the origin")
        XCTAssertEqual(try readAll(agent).count, 0, "a resize at rest writes no row")
    }

    /// The phase is the wheel handler's, from the event that moved the scroller: a finger's began/changed is `t`,
    /// momentum's began/changed `d`, an ended phase `-`, and a move outside a wheel event (a layout's) `-`.
    func testTheMacPhaseIsTheWheelEventsThatMovedIt() throws {
        guard !ExactEnv.agentFreezes else { throw XCTSkip("platform timing") }
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "track-mac-phase")
        defer { session.destroy() }
        session.clock = nil
        session.sampler?.stop(); session.sampler = nil
        let saved = ChainingScrollView.appliesAtOnce, savedLatch = ChainingScrollView.latch
        ChainingScrollView.appliesAtOnce = true // the handler applies each event itself, synchronously
        defer { ChainingScrollView.appliesAtOnce = saved; ChainingScrollView.latch = savedLatch }
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 800)
        let window = NSWindow(contentRect: NSRect(x: 240, y: 160, width: 400, height: 800), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        window.orderFrontRegardless()
        defer { window.close() }
        p.apply(wireBatch(Self.batch))
        p.viewport.layoutSubtreeIfNeeded()
        let sv = try XCTUnwrap(p.views[2]?.scroll)
        let agent = Agent(session: session)
        XCTAssertEqual(try ask(agent, ["op": "perf", "frames": true, "track": "start", "node": 2])["track"] as? Bool, true)
        // CGScrollPhase: 1 began, 2 changed, 4 ended; CGMomentumScrollPhase: 1 begin, 2 continue, 3 end.
        func wheel(_ dy: Int32, phase: Int64, momentum: Int64) throws -> NSEvent {
            let cg = try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: dy, wheel2: 0, wheel3: 0))
            cg.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
            cg.setIntegerValueField(.scrollWheelEventMomentumPhase, value: momentum)
            return try XCTUnwrap(NSEvent(cgEvent: cg))
        }
        for (phase, momentum) in [(1, 0), (2, 0), (0, 1), (0, 2), (0, 3)] as [(Int64, Int64)] {
            sv.scrollWheel(with: try wheel(-40, phase: phase, momentum: momentum))
        }
        sv.contentView.scroll(to: NSPoint(x: 0, y: sv.contentView.bounds.origin.y + 30)) // a layout's move
        sv.reflectScrolledClipView(sv.contentView)
        let rows = try readAll(agent)
        XCTAssertEqual(rows.compactMap { $0[4] as? String }, ["t", "t", "d", "d", "-", "-"], "\(rows)")
        XCTAssertEqual(sv.trackPhase, "-", "no phase outlives its handler")
    }

    /// A scroller that moves only sideways is read along x, as `scrollSample` reads it on iOS.
    func testAMacSidewaysScrollerIsReadAlongX() throws {
        guard !ExactEnv.agentFreezes else { throw XCTSkip("platform timing") }
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "track-mac-x")
        defer { session.destroy() }
        session.clock = nil
        session.sampler?.stop(); session.sampler = nil
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 800)
        let window = NSWindow(contentRect: NSRect(x: 240, y: 160, width: 400, height: 800), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        window.orderFrontRegardless()
        defer { window.close() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_x": "scroll"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            // Taller than the clip too: only the overflow flags say this scroller moves sideways alone.
            ["op": "content", "id": 2, "w": 4000.0, "h": 600.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 4000.0, "h": 600.0],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        let sv = try XCTUnwrap(p.views[2]?.scroll)
        XCTAssertTrue(sv.scrollsX && !sv.scrollsY)
        let agent = Agent(session: session)
        XCTAssertEqual(try ask(agent, ["op": "perf", "frames": true, "track": "start", "node": 2])["track"] as? Bool, true)
        for _ in 0..<4 {
            XCTAssertNil(agent.tap(["id": 2, "wheel": [100.0, 0.0]])["error"])
            until(0.05) { false }
        }
        let rows = try readAll(agent)
        let offsets = rows.compactMap { $0[3] as? Int }
        XCTAssertGreaterThan(sv.contentView.bounds.origin.x, 0, "the wheel moved it sideways")
        XCTAssertEqual(offsets.last, Int((sv.contentView.bounds.origin.x * 100).rounded()), "x, the axis it scrolls on: \(rows)")
        XCTAssertNotEqual(offsets.first, offsets.last)
    }
    #endif
}

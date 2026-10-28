import XCTest
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

/// A timer's (or a scroll event's) batch that changes nothing only moves the clock: it does not
/// reach `apply`, as fills and list feedback already skip theirs, and the
/// next timer is armed for its deadline. A batch with ops, an error, a
/// control's viewless contents (`controls`, LLP 1069.011 §9), new motion,
/// moved page facts or a wake lock still applies. The flag crosses the wire
/// as `"controls":true`.
final class IdleTickTests: XCTestCase {
    override class func setUp() {
        super.setUp()
        #if os(macOS)
        _ = NSApplication.shared
        #endif
    }

    func testATickThatChangesNothingOnlyMovesTheClock() throws {
        let session = ExactApp.shared.makeSession(label: "idle-tick")
        defer { session.destroy() }
        let before = session.appliedBatches
        let due = session.now() + 250
        var idle = Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due, pending: false)
        XCTAssertTrue(session.changesNothing(idle))
        session.applyUnlessEmpty(idle)
        XCTAssertEqual(session.appliedBatches, before, "an idle tick does not reach apply")
        XCTAssertEqual(session.timerDue, due, "its deadline is kept")
        if !ExactEnv.agentMode {
            XCTAssertEqual(session.clockDue, due, "and the next timer armed for it")
            XCTAssertEqual(session.clockTimer?.isValid, true)
        }
        // A later deadline re-arms; no deadline invalidates what was armed.
        let armed = session.clockTimer
        idle.timerDueMs = due + 250
        session.applyUnlessEmpty(idle)
        if !ExactEnv.agentMode {
            XCTAssertEqual(session.clockDue, due + 250)
            XCTAssertEqual(armed?.isValid, false, "the earlier timer is replaced")
        }
        let later = session.clockTimer
        if !ExactEnv.agentMode { XCTAssertEqual(later?.isValid, true, "the successor is armed") }
        idle.timerDueMs = nil
        session.applyUnlessEmpty(idle)
        XCTAssertEqual(session.appliedBatches, before)
        XCTAssertNil(session.clockTimer)
        XCTAssertNil(session.clockDue)
        XCTAssertNotEqual(later?.isValid, true, "and invalidated")
        // A scroll event's batch is no timer's (`timers` false): the same.
        let scrolled = Batch(ops: [], timers: false, motion: false, clock: nil, error: nil, timerDueMs: due)
        session.applyUnlessEmpty(scrolled)
        XCTAssertEqual(session.appliedBatches, before, "an empty scroll event does not reach apply")
        XCTAssertEqual(session.timerDue, due)
        // Any other event's empty batch still applies: the pass reconciles a
        // native control that changed itself first, and seeks native
        // animations to the agent's clock.
        session.apply(scrolled)
        XCTAssertEqual(session.appliedBatches, before + 1, "an ordinary empty batch reaches apply")
        // Each of these is news: the tick applies.
        var controls = Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due)
        controls.controls = true
        let failed = Batch(ops: [], timers: true, motion: false, clock: nil, error: "boom", timerDueMs: due)
        let moving = Batch(ops: [], timers: true, motion: true, clock: nil, error: nil, timerDueMs: due)
        let ops = wireBatch([["op": "props", "id": 1, "set": ["testId": "x"]]])
        // A timer's own `setMode` or `setRootFontSize` (page facts) or wake lock, with no node changed.
        var facts = Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due)
        facts.facts = true
        var wakelock = Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due)
        wakelock.wakelock = false
        for batch in [controls, failed, moving, ops, facts, wakelock] {
            XCTAssertFalse(session.changesNothing(batch))
            let count = session.appliedBatches
            session.applyUnlessEmpty(batch)
            XCTAssertEqual(session.appliedBatches, count + 1)
        }
    }

    /// A slice the owner published before an empty scroll batch lands
    /// first, and the scroll batch then still skips the pass: a fling's
    /// fills are often in flight.
    func testAnEmptyScrollBatchBehindAPublishedSliceLandsItAndSkipsThePass() throws {
        let session = ExactApp.shared.makeSession(label: "idle-behind-slice")
        defer { session.destroy() }
        let due = session.now() + 250
        session.publishTickForTests(wireBatch([["op": "props", "id": 1, "set": ["testId": "x"]]]))
        XCTAssertTrue(session.tickInFlight)
        let before = session.appliedBatches
        session.applyUnlessEmpty(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil, timerDueMs: due), scrolled: true)
        XCTAssertFalse(session.tickInFlight, "the slice landed")
        XCTAssertEqual(session.appliedBatches, before + 1, "the slice's pass, not the empty scroll batch's")
        XCTAssertEqual(session.timerDue, due, "the scroll batch's deadline, the owner's latest")
        // A report the landing owes waits for the batch, then is asked: not stranded by the skip.
        session.publishTickForTests(wireBatch([["op": "props", "id": 1, "set": ["testId": "y"]]]))
        session.oweColorsForTests()
        let held = session.appliedBatches
        session.applyUnlessEmpty(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil, timerDueMs: due), scrolled: true)
        XCTAssertEqual(session.appliedBatches, held + 2, "the slice, then the colours' report; the scroll batch skipped")
        XCTAssertNotEqual(session.timerDue, due, "the report applied after the skip, its clock the latest (an unbooted runtime's has none)")
    }

    func testWorkWaitingForAStaleSliceRunsAfterTheCommitAndDropsOldViewReports() {
        let session = ExactApp.shared.makeSession(label: "stale-slice")
        defer { session.destroy() }
        var calls: [String] = []
        session.holdFillForTests()
        session.whenIdle { calls.append("pump") }
        session.whenIdle(bound: true, { calls.append("old views") }, cancelled: { calls.append("cancelled") })
        let staleGeneration = session.generation
        session.commitForTests()
        session.whenIdle(bound: true) { calls.append("new views") }
        session.publishFillForTests(1, wireBatch([["op": "create", "id": 999, "kind": "view"]]), generation: staleGeneration)
        session.drainFill()
        XCTAssertTrue(calls.isEmpty, "the commit's tree must apply before its waiters")
        XCTAssertNil(session.presenter.views[999], "a stale slice applies nothing")
        let finished = expectation(description: "stale waiters released")
        DispatchQueue.main.async { finished.fulfill() }
        wait(for: [finished], timeout: 5)
        XCTAssertEqual(calls, ["pump", "cancelled", "new views"])
        session.holdFillForTests()
        session.whenIdle { calls.append("same generation") }
        session.publishFillForTests(1, Batch(ops: [], timers: false, motion: false, clock: nil, error: nil), generation: session.generation)
        session.drainFill()
        XCTAssertEqual(calls.last, "same generation")
    }

    func testNestedBatchesWaitForTheBatchHoldingALanding() {
        for emptyHeld in [false, true] {
            for (commandAfterApply, advance) in [(false, false), (true, false), (false, true), (true, true)] {
                let session = ExactApp.shared.makeSession(label: "nested-held-batch")
                defer { session.destroy() }
                session.clock = 0
                let before = session.appliedBatches
                var calls = 0
                let nested = { [unowned session] in
                    calls += 1
                    XCTAssertTrue(session.holdingReports)
                    XCTAssertEqual(session.applying, !commandAfterApply)
                    session.apply(wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30]]))
                    session.applyUnlessEmpty(wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 40]]))
                    session.applyUnlessEmpty(Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: 300), timer: true)
                    XCTAssertEqual(session.presenter.views[1]?.frame.height, 10, "nested commits wait, even after applying becomes false")
                    XCTAssertNotEqual(session.timerDue, 300, "the empty batch's clock waits too")
                }
                let delegate = IdleTickDelegate(nested)
                if commandAfterApply { session.delegate = delegate }
                else { session.presenter.onCommand = { _, _, _ in nested() } }
                let slice = wireBatch([
                    ["op": "create", "id": 1, "kind": "view", "handlers": ["focus"]],
                    ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
                    ["op": "command", "name": "nested", "args": []],
                ])
                if advance { session.advanceInFlight = true; session.publish(.advance, slice, session.generation) }
                else { session.publishFillForTests(1, slice, generation: session.generation) }
                var held = emptyHeld ? wireBatch([]) : wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]])
                held.timerDueMs = 200
                withExtendedLifetime(delegate) {
                    if emptyHeld { session.applyUnlessEmpty(held) } else { session.apply(held) }
                }
                XCTAssertEqual(calls, 1)
                XCTAssertEqual(session.presenter.views[1]?.frame.height, 40, "the newest layout survives the held batch")
                XCTAssertEqual(session.timerDue, 300)
                XCTAssertEqual(session.quietTimerAdvances, 1, "the queued empty batch keeps its timer flag")
                XCTAssertEqual(session.appliedBatches, before + (emptyHeld ? 3 : 4))
            }
        }
    }

    func testHeldWorkRunsInRequestOrderThroughAReentrantRelease() {
        // A stale slice's waiters run on a later turn (`landedFill`), outside this order.
        for emptyHeld in [false, true] {
            let session = ExactApp.shared.makeSession(label: "held-work-order")
            defer { session.destroy() }
            session.clock = 0
            var calls: [String] = []
            session.holdFillForTests()
            session.whenIdle { [unowned session] in
                calls.append("first")
                XCTAssertEqual(session.timerDue, 200, "the held batch applied before any waiter")
                session.whenIdle { calls.append("last") }
                session.publishTickForTests(wireBatch([["op": "command", "name": "reenter", "args": []]]))
                session.applyUnlessEmpty(wireBatch([]))
            }
            session.presenter.onCommand = { [unowned session] name, _, _ in
                // The tick lands as the batch released after it is applied, not under the hold.
                XCTAssertEqual(session.holdingReports, name == "slice")
                if name == "slice" {
                    session.whenIdle { calls.append("second") }
                    session.whenIdle { calls.append("third") }
                    XCTAssertTrue(calls.isEmpty)
                } else {
                    session.whenIdle { calls.append("reentrant") }
                }
            }
            session.publishFillForTests(1, wireBatch([["op": "command", "name": "slice", "args": []]]), generation: session.generation)
            let held = Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: 200)
            if emptyHeld { session.applyUnlessEmpty(held) } else { session.apply(held) }
            XCTAssertEqual(calls, ["first", "second", "third", "last", "reentrant"])
        }
    }

    func testAHeldBatchAppliesWhenReleasedThoughALaterSliceIsInFlight() {
        let session = ExactApp.shared.makeSession(label: "held-batch-in-turn")
        defer { session.destroy() }
        session.clock = 0
        session.presenter.onCommand = { [unowned session] _, _, _ in
            session.apply(wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30]]))
            // A slice the owner makes after that batch, still building when the landing ends.
            session.holdFillForTests()
        }
        session.publishFillForTests(1, wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
            ["op": "command", "name": "nested", "args": []],
        ]), generation: session.generation)
        session.apply(wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]]))
        XCTAssertEqual(session.presenter.views[1]?.frame.height, 30, "the held-back batch applies on release, not after the later slice")
    }

    func testABatchMadeAfterAHeldOneAppliesAfterIt() {
        let session = ExactApp.shared.makeSession(label: "held-batch-order")
        defer { session.destroy() }
        session.clock = 0
        session.presenter.onCommand = { [unowned session] name, _, _ in
            // The slice's handler makes C while B waits; B's handler makes D, after C.
            let h = name == "slice" ? 30 : 40
            session.apply(wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": h]]))
        }
        session.publishFillForTests(1, wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
            ["op": "command", "name": "slice", "args": []],
        ]), generation: session.generation)
        session.apply(wireBatch([
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20],
            ["op": "command", "name": "held", "args": []],
        ]))
        XCTAssertEqual(session.presenter.views[1]?.frame.height, 40, "C, then D: the owner's order")
    }

    func testABatchThatLandsASliceAppliesBeforeWhatTheSliceHeld() {
        for empty in [false, true] {
            let session = ExactApp.shared.makeSession(label: "held-behind-unless-empty")
            defer { session.destroy() }
            session.clock = 0
            session.presenter.onCommand = { [unowned session] _, _, _ in
                session.apply(wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30]]))
            }
            session.publishFillForTests(1, wireBatch([
                ["op": "create", "id": 1, "kind": "view"],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
                ["op": "command", "name": "slice", "args": []],
            ]), generation: session.generation)
            // B, made before the slice's handler made C: A, B, C.
            session.applyUnlessEmpty(empty ? wireBatch([]) : wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]]))
            XCTAssertEqual(session.presenter.views[1]?.frame.height, 30)
        }
    }

    func testQueuedBatchesExpireOnACommitOrDestroy() {
        for replacement in ["commit", "reset", "destroy"] {
            let session = ExactApp.shared.makeSession(label: "stale-held-batch")
            defer { session.destroy() }
            session.clock = 0
            let before = session.appliedBatches
            session.holdFillForTests()
            session.whenIdle { [unowned session] in
                if replacement == "destroy" { session.destroy() }
                else { session.commitForTests(resetViews: replacement == "reset") }
            }
            var queued = false
            session.presenter.onCommand = { [unowned session] _, _, _ in
                queued = true
                XCTAssertTrue(session.holdingReports)
                session.apply(wireBatch([["op": "create", "id": 999, "kind": "view"]]))
                session.applyUnlessEmpty(Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: 300), timer: true)
            }
            session.publishFillForTests(1, wireBatch([["op": "command", "name": "queue", "args": []]]), generation: session.generation)
            session.apply(Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: 200))
            XCTAssertTrue(queued)
            XCTAssertNil(session.presenter.views[999])
            XCTAssertEqual(session.appliedBatches, before + 2, "the slice and held batch, with no expired batch applied")
            XCTAssertEqual(session.timerDue, 200, "the expired empty batch cannot change the clock")
        }
    }

    func testAHeldBatchExpiresWhenItsLandingCommitsANewGeneration() {
        for emptyHeld in [false, true] {
            let session = ExactApp.shared.makeSession(label: "stale-held-landing")
            defer { session.destroy() }
            session.clock = 0
            session.presenter.onCommand = { [unowned session] _, _, _ in
                session.commitForTests()
                var replacement = wireBatch([["op": "create", "id": 998, "kind": "view", "handlers": ["focus"]]])
                replacement.timerDueMs = 300
                session.apply(replacement)
            }
            session.publishTickForTests(wireBatch([["op": "command", "name": "commit", "args": []]]))
            var held = emptyHeld ? wireBatch([]) : wireBatch([["op": "create", "id": 999, "kind": "view"]])
            held.timerDueMs = 200
            if emptyHeld { session.applyUnlessEmpty(held) } else { session.apply(held) }
            XCTAssertNil(session.presenter.views[999])
            XCTAssertNotNil(session.presenter.views[998], "abandoning the old batch still releases the replacement")
            XCTAssertEqual(session.timerDue, 300, "the held batch belongs to the generation the landing replaced")
        }
    }

    func testTheControlsFlagDecodes() throws {
        let data = Data(#"{"ops":[],"timers":false,"motion":false,"controls":true}"#.utf8)
        XCTAssertTrue(Batch.decode(data).controls)
        XCTAssertFalse(Batch.decode(Data(#"{"ops":[],"timers":false,"motion":false}"#.utf8)).controls)
    }
}

private final class IdleTickDelegate: ExactSessionDelegate {
    let onCommand: () -> Void
    init(_ onCommand: @escaping () -> Void) { self.onCommand = onCommand }
    func exactSession(_ session: ExactSession, command name: String, args: [Any]) { onCommand() }
}

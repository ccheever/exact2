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
        let ops = madeBatch([["op": "props", "id": 1, "set": ["testId": "x"]]])
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
        session.publishTickForTests(madeBatch([["op": "props", "id": 1, "set": ["testId": "x"]]]))
        XCTAssertTrue(session.tickInFlight)
        let before = session.appliedBatches
        session.applyUnlessEmpty(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil, timerDueMs: due), scrolled: true)
        XCTAssertFalse(session.tickInFlight, "the slice landed")
        XCTAssertEqual(session.appliedBatches, before + 1, "the slice's pass, not the empty scroll batch's")
        XCTAssertEqual(session.timerDue, due, "the scroll batch's deadline, the owner's latest")
        // A report the landing owes waits for the batch, then is asked: not stranded by the skip.
        session.publishTickForTests(madeBatch([["op": "props", "id": 1, "set": ["testId": "y"]]]))
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
        session.publishFillForTests(1, madeBatch([["op": "create", "id": 999, "kind": "view"]]), generation: staleGeneration)
        session.drainFill()
        session.whenIdle { calls.append("after landing") }
        XCTAssertTrue(calls.isEmpty, "the commit's tree must apply before its waiters")
        XCTAssertNil(session.presenter.views[999], "a stale slice applies nothing")
        let finished = expectation(description: "stale waiters released")
        DispatchQueue.main.async { finished.fulfill() }
        wait(for: [finished], timeout: 5)
        XCTAssertEqual(calls, ["pump", "cancelled", "new views", "after landing"])
        session.holdFillForTests()
        session.whenIdle { calls.append("same generation") }
        session.publishFillForTests(1, Batch(ops: [], timers: false, motion: false, clock: nil, error: nil), generation: session.generation)
        session.drainFill()
        XCTAssertEqual(calls.last, "same generation")
    }

    func testNestedBatchesApplyBeforeReturningWhenNothingOlderWaits() {
        for published in [false, true] {
            for commandAfterApply in [false, true] {
                let session = ExactApp.shared.makeSession(label: "nested-immediate-batch")
                defer { session.destroy() }
                session.clock = 0
                let before = session.appliedBatches
                var calls = 0
                let nested = { [unowned session] in
                    calls += 1
                    XCTAssertTrue(session.landing)
                    XCTAssertEqual(session.applying, !commandAfterApply)
                    var reported = false
                    session.whenIdle(bound: true) {
                        reported = true
                        let ops = madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30]]).ops
                        var layout = Batch(ops: ops, timers: false, motion: true, clock: nil, error: nil)
                        layout.seq = (2, 2)
                        session.apply(layout)
                        XCTAssertEqual(session.presenter.views[1]?.frame.height, 30, "a native callback reads the new layout")
                    }
                    XCTAssertTrue(reported, "an idle report runs before its native caller continues")
                    var empty = Batch(ops: [], timers: true, motion: true, clock: nil, error: nil, timerDueMs: 300)
                    empty.seq = (3, 3)
                    session.applyUnlessEmpty(empty, timer: true)
                    XCTAssertEqual(session.timerDue, 300)
                    XCTAssertEqual(session.appliedBatches, before + 2, "the nested empty batch skips presentation")
                }
                let delegate = IdleTickDelegate(nested)
                if commandAfterApply { session.delegate = delegate }
                else { session.presenter.onCommand = { _, _, _ in nested() } }
                var batch = madeBatch([
                    ["op": "create", "id": 1, "kind": "view"],
                    ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
                    ["op": "command", "name": "nested", "args": []],
                ])
                batch.timerDueMs = 200
                batch.seq = (1, 1)
                withExtendedLifetime(delegate) {
                    if published { session.publishTickForTests(batch); session.landFill() }
                    else { session.apply(batch) }
                }
                XCTAssertEqual(calls, 1)
                XCTAssertEqual(session.presenter.views[1]?.frame.height, 30)
                XCTAssertEqual(session.timerDue, 300, "the resumed outer batch cannot restore its clock")
                XCTAssertTrue(session.frames.motion, "the resumed outer batch cannot restore its motion flags")
                XCTAssertEqual(session.natives.hatchClock.seq, 3)
                XCTAssertEqual(session.quietTimerAdvances, 1)
            }
        }
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
                    XCTAssertTrue(session.landing)
                    XCTAssertEqual(session.applying, !commandAfterApply)
                    session.apply(madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30]]))
                    session.applyUnlessEmpty(madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 40]]))
                    session.applyUnlessEmpty(Owner.shared.sync { Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: 300) }, timer: true)
                    XCTAssertEqual(session.presenter.views[1]?.frame.height, 10, "nested commits wait, even after applying becomes false")
                    XCTAssertNotEqual(session.timerDue, 300, "the empty batch's clock waits too")
                }
                let delegate = IdleTickDelegate(nested)
                if commandAfterApply { session.delegate = delegate }
                else { session.presenter.onCommand = { _, _, _ in nested() } }
                let slice = madeBatch([
                    ["op": "create", "id": 1, "kind": "view", "handlers": ["focus"]],
                    ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
                    ["op": "command", "name": "nested", "args": []],
                ])
                if advance { session.advanceInFlight = true; session.publishForTests(.advance, slice, generation: session.generation) }
                else { session.publishFillForTests(1, slice, generation: session.generation) }
                var held = emptyHeld ? madeBatch([]) : madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]])
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
        // A stale slice releases its waiters on a later main turn.
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
                session.publishTickForTests(madeBatch([["op": "command", "name": "reenter", "args": []]]))
                session.applyUnlessEmpty(madeBatch([]))
            }
            session.presenter.onCommand = { [unowned session] name, _, _ in
                // Both callbacks run inside the same drain.
                XCTAssertTrue(session.landing)
                if name == "slice" {
                    session.whenIdle { calls.append("second") }
                    session.whenIdle { calls.append("third") }
                    XCTAssertTrue(calls.isEmpty)
                } else {
                    session.whenIdle { calls.append("reentrant") }
                }
            }
            session.publishFillForTests(1, madeBatch([["op": "command", "name": "slice", "args": []]]), generation: session.generation)
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
            session.apply(madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30]]))
            // A slice the owner makes after that batch, still building when the landing ends.
            session.holdFillForTests()
        }
        session.publishFillForTests(1, madeBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
            ["op": "command", "name": "nested", "args": []],
        ]), generation: session.generation)
        session.apply(madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]]))
        XCTAssertEqual(session.presenter.views[1]?.frame.height, 30, "the held-back batch applies on release, not after the later slice")
    }

    func testABatchMadeAfterAHeldOneAppliesAfterIt() {
        let session = ExactApp.shared.makeSession(label: "held-batch-order")
        defer { session.destroy() }
        session.clock = 0
        session.presenter.onCommand = { [unowned session] name, _, _ in
            // The slice's handler makes C while B waits; B's handler makes D, after C.
            let h = name == "slice" ? 30 : 40
            session.apply(madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": h]]))
        }
        session.publishFillForTests(1, madeBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
            ["op": "command", "name": "slice", "args": []],
        ]), generation: session.generation)
        session.apply(madeBatch([
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
                session.apply(madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30]]))
            }
            session.publishFillForTests(1, madeBatch([
                ["op": "create", "id": 1, "kind": "view"],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
                ["op": "command", "name": "slice", "args": []],
            ]), generation: session.generation)
            // B, made before the slice's handler made C: A, B, C.
            session.applyUnlessEmpty(empty ? madeBatch([]) : madeBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]]))
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
                XCTAssertTrue(session.landing)
                session.apply(madeBatch([["op": "create", "id": 999, "kind": "view"]]))
                session.applyUnlessEmpty(Owner.shared.sync { Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: 300) }, timer: true)
            }
            session.publishFillForTests(1, madeBatch([["op": "command", "name": "queue", "args": []]]), generation: session.generation)
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
                var replacement = madeBatch([["op": "create", "id": 998, "kind": "view", "handlers": ["focus"]]])
                replacement.timerDueMs = 300
                session.applyReplacement(replacement)
            }
            session.publishTickForTests(madeBatch([["op": "command", "name": "commit", "args": []]]))
            var held = emptyHeld ? madeBatch([]) : madeBatch([["op": "create", "id": 999, "kind": "view"]])
            held.timerDueMs = 200
            if emptyHeld { session.applyUnlessEmpty(held) } else { session.apply(held) }
            XCTAssertNil(session.presenter.views[999])
            XCTAssertNotNil(session.presenter.views[998], "abandoning the old batch still releases the replacement")
            XCTAssertEqual(session.timerDue, 300, "the held batch belongs to the generation the landing replaced")
        }
    }

    func testANestedDrainCannotOvertakeWaitingBatches() {
        for drain in [false, true] {
            let session = ExactApp.shared.makeSession(label: "nested-drain")
            defer { session.destroy() }
            session.clock = 0
            var order: [String] = []
            session.presenter.onCommand = { [unowned session] name, _, _ in
                order.append(name)
                if name == "A" { session.apply(madeBatch([["op": "command", "name": "C", "args": []]])) }
                if name == "B" {
                    session.publishTickForTests(madeBatch([["op": "command", "name": "D", "args": []]]))
                    if drain { session.drainFill() } else { session.landFill() }
                    XCTAssertEqual(order, ["A", "B"], "a nested drain only queues the publication")
                }
            }
            session.publishFillForTests(1, madeBatch([["op": "command", "name": "A", "args": []]]), generation: session.generation)
            session.apply(madeBatch([["op": "command", "name": "B", "args": []]]))
            XCTAssertEqual(order, ["A", "B", "C", "D"])
        }
    }

    func testNoticesFromOneOwnerJobStayAheadOfALaterBatch() {
        let session = ExactApp.shared.makeSession(label: "notice-order")
        defer { session.destroy() }
        session.clock = 0
        var order: [String] = []
        session.presenter.onCommand = { [unowned session] name, _, _ in
            order.append(name)
            if name == "first" {
                session.apply(madeBatch([["op": "command", "name": "nested", "args": []]]))
            }
        }
        session.tellSoon([
            { _ in wireBatch([["op": "command", "name": "first", "args": []]]) },
            { _ in wireBatch([["op": "command", "name": "second", "args": []]]) },
        ])
        session.apply(madeBatch([["op": "command", "name": "sync", "args": []]]))
        XCTAssertEqual(order, ["first", "second", "sync", "nested"])
        XCTAssertEqual(session.noticesInFlight, 0)
    }

    func testReleasedIdleWorkKeepsItsPlaceBesideBatchesAndNewFills() {
        let session = ExactApp.shared.makeSession(label: "released-idle-order")
        defer { session.destroy() }
        session.clock = 0
        var order: [String] = []
        session.holdFillForTests()
        session.whenIdle { [unowned session] in
            order.append("first")
            session.holdFillForTests()
            session.whenIdle { order.append("last") }
        }
        session.whenIdle { order.append("second") }
        session.presenter.onCommand = { [unowned session] name, _, _ in
            order.append(name)
            if name == "A" {
                session.whenIdle { order.append("third") }
                session.apply(madeBatch([["op": "command", "name": "C", "args": []]]))
            }
            if name == "B" { session.whenIdle { order.append("fourth") } }
        }
        session.publishFillForTests(1, madeBatch([["op": "command", "name": "A", "args": []]]), generation: session.generation)
        session.apply(madeBatch([["op": "command", "name": "B", "args": []]]))
        XCTAssertEqual(order, ["A", "B", "first", "second", "third", "C", "fourth"])
        session.publishFillForTests(1, madeBatch([]), generation: session.generation)
        session.drainFill()
        XCTAssertEqual(order.last, "last")
    }

    func testReplacementTreesApplyInsideALanding() throws {
        let plan = try fixturePlan("RESET_GATE")
        for boot in [false, true] {
            let session = ExactApp.shared.makeSession(label: "replacement-in-landing")
            defer { session.destroy() }
            session.clock = 0
            // Reload prepares at the presenter's viewport, even without an ExactView.
            session.presenter.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 600)
            XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 400, height: 600)).error)
            var replaced = false
            let delegate = IdleTickDelegate { [unowned session] in
                if boot { XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 400, height: 600)).error) }
                else { XCTAssertTrue(session.apply(plan, label: "replacement-test")) }
                replaced = true
                XCTAssertFalse(session.awaitingTree)
                XCTAssertGreaterThan(session.viewCount, 0, "treeIsIn follows a presented replacement")
                XCTAssertFalse(session.presenter.collections.entries.isEmpty, "the replacement's lists were applied")
            }
            session.delegate = delegate
            session.publishTickForTests(madeBatch([["op": "command", "name": "replace", "args": []]]))
            withExtendedLifetime(delegate) {
                session.apply(madeBatch([["op": "create", "id": 99999, "kind": "view"]]))
            }
            XCTAssertTrue(replaced)
            XCTAssertTrue(session.presenter.collections.entries.values.allSatisfy { $0.lastFacts != nil }, "the replacement still flushes its lists")
            XCTAssertNil(session.presenter.views[99999], "the waiting batch belonged to the replaced tree")
        }
    }

    func testCollectionReportsMeasureAfterWaitingBatches() throws {
        let session = ExactApp.shared.makeSession(label: "ordered-collection-report")
        defer { session.destroy() }
        session.clock = 0
        session.presenter.viewport.frame = CGRect(x: 0, y: 0, width: 300, height: 200)
        var heights: [Double] = []
        session.presenter.collections.onFeedback = { [unowned session] data in
            let count = data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 96, as: UInt32.self) }
            guard count > 0 else { return }
            let height = data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 112, as: Double.self) }
            heights.append(height)
            session.apply(madeBatch([["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": height]]))
        }
        session.publishFillForTests(1, madeBatch([
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 200],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 10],
            contentOp(1, 300, 200, padding: [0, 0, 0, 0]), ["op": "content", "id": 1, "w": 300, "h": 3000],
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0,
                "count": 100, "totalExtent": 3000, "rows": [["view": 2, "root": 2, "epoch": 1]], "correction": NSNull()]]],
        ]), generation: session.generation)
        session.apply(madeBatch([["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 40]]))
        XCTAssertFalse(heights.isEmpty, "a held report is retried after the queue drains")
        XCTAssertTrue(heights.allSatisfy { $0 == 40 }, "no report uses the slice's superseded row height")
        XCTAssertEqual(session.presenter.views[2]?.frame.height, 40)
    }

    func testASynchronousBatchKeepsItsSubmissionAcrossLaterCalls() {
        for nested in [false, true] {
            let session = ExactApp.shared.makeSession(label: "submission-number")
            defer { session.destroy() }
            session.clock = 0
            session.apply(madeBatch([
                ["op": "create", "id": 1, "kind": "view"],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
            ]))
            let submit = { [unowned session] in
                let early = session.runtime.on {
                    Owner.shared.callMain {
                        session.fillInFlight = true
                        let generation = session.generation, sequence = Owner.shared.enqueued + 1
                        Owner.shared.post {
                            session.publish(.fill(1), wireBatch([
                                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30],
                            ]), generation, sequence: sequence)
                        }
                    }
                    return wireBatch([["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20]])
                }
                // As in data activation, an agent read waits for the fill
                // before its caller presents the batch it already holds.
                _ = session.runtime.agent("{\"op\":\"tree\"}")
                session.apply(early)
            }
            if nested {
                session.presenter.onCommand = { _, _, _ in submit() }
                session.apply(madeBatch([["op": "command", "name": "submit", "args": []]]))
            } else { submit() }
            XCTAssertEqual(session.presenter.views[1]?.frame.height, 30, "the later fill survives the earlier batch")
            XCTAssertFalse(session.fillInFlight)
        }
    }

    func testQueuedFocusHandlerStopsBubblingBeforeItsBatchLands() {
        let session = ExactApp.shared.makeSession(label: "queued-focus-decision")
        defer { session.destroy() }
        session.clock = 0
        let p = session.presenter
        var heard: [UInt32] = []
        p.onFocusEvent = { id, _, _, _ in
            heard.append(id)
            session.apply(madeBatch([
                ["op": "command", "name": "stopPropagation", "args": []],
                ["op": "command", "name": "preventDefault", "args": []],
                ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 100, "h": 30],
            ]))
            XCTAssertEqual(p.views[2]?.frame.height, 10, "presentation still waits for B")
            XCTAssertTrue(p.defaultPrevented, "the dispatch can decide its native default now")
            p.defaultPrevented = false
        }
        p.onCommand = { _, _, _ in p.focus(2) }
        session.publishTickForTests(madeBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["focusin"]],
            ["op": "create", "id": 2, "kind": "input", "handlers": ["focusin"], "props": ["id": "field"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 100],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 100, "h": 10],
            ["op": "command", "name": "focus-test", "args": []],
        ]))
        session.apply(madeBatch([["op": "frame", "id": 2, "x": 0, "y": 0, "w": 100, "h": 20]]))
        XCTAssertEqual(heard, [2], "the parent never receives the stopped event")
        XCTAssertEqual(p.views[2]?.frame.height, 30)
        XCTAssertFalse(p.propagationStopped, "the queued batch cannot stop a later event")
        XCTAssertFalse(p.defaultPrevented, "the queued batch cannot cancel a later default")
    }

    func testReplacementCommandsWaitForCollectionReportsInsideALanding() throws {
        let plan = try fixturePlan("RESET_GATE")
        for nested in [false, true] {
            let session = ExactApp.shared.makeSession(label: "replacement-command-report")
            defer { session.destroy() }
            session.clock = 0
            session.presenter.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 600)
            XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 400, height: 600)).error)
            var commands = 0
            let delegate = IdleTickDelegate { [unowned session] in
                commands += 1
                XCTAssertFalse(session.awaitingTree)
                XCTAssertFalse(session.presenter.collections.entries.isEmpty)
                XCTAssertTrue(session.presenter.collections.entries.values.allSatisfy { $0.lastFacts != nil },
                              "the replacement command sees reported lists")
            }
            session.delegate = delegate
            let replace = { [unowned session] in
                guard let candidate = session.prepare(plan, resolver: session.app.resolver),
                      var batch = session.commit(candidate) else { XCTFail("prepare and commit"); return }
                batch.ops += wireBatch([["op": "command", "name": "reported", "args": []]]).ops
                session.presentCommitted(batch, label: "command-report")
            }
            withExtendedLifetime(delegate) {
                if nested {
                    let receive = session.presenter.onCommand
                    session.presenter.onCommand = { name, args, source in
                        if name == "replace" { replace() }
                        else { receive?(name, args, source) }
                    }
                    session.publishTickForTests(madeBatch([["op": "command", "name": "replace", "args": []]]))
                    session.apply(madeBatch([]))
                } else { replace() }
            }
            XCTAssertEqual(commands, 1)
        }
    }

    func testReleasedReportsContinueAfterTheirOwnLayout() {
        for laterTick in [false, true] {
            let session = ExactApp.shared.makeSession(label: "report-continuation")
            defer { session.destroy() }
            session.clock = 0
            session.apply(madeBatch([
                ["op": "create", "id": 1, "kind": "view"],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 10],
            ]))
            var order: [String] = []
            let report = { [unowned session] in
                if laterTick {
                    session.publishTickForTests(madeBatch([
                        ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 20],
                    ]))
                }
                session.applyUnlessEmpty(madeBatch([
                    ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 30],
                ]), scrolled: true) {
                    XCTAssertEqual(session.presenter.views[1]?.frame.height, 30)
                    order.append("presented")
                }
                order.append("reported")
                XCTAssertEqual(session.presenter.views[1]?.frame.height, 10)
            }
            if laterTick {
                session.presenter.onCommand = { _, _, _ in report() }
                session.apply(madeBatch([["op": "command", "name": "report", "args": []]]))
            } else {
                session.holdFillForTests()
                session.whenIdle(report)
                session.whenIdle { order.append("second") }
                session.publishFillForTests(1, madeBatch([]), generation: session.generation)
                session.drainFill()
            }
            XCTAssertEqual(order, laterTick ? ["reported", "presented"] : ["reported", "second", "presented"])
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

private func madeBatch(_ ops: [[String: Any]]) -> Batch { Owner.shared.sync { wireBatch(ops) } }

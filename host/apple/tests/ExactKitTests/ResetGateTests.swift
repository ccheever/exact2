#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
import ExactCore
@testable import ExactKit

/// A restart's reset reaches no runtime (ResetGate): what the old tree's teardown sets off is held at `Runtime.on`,
/// named, and once the committed batch is in, dropped or replayed, each replay once and in order. A real restart over
/// a field with an edit open, a virtualized list and a timer always due; then each path a review found, its call made
/// as the teardown makes it, with the runtime held.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class ResetGateTests: XCTestCase {
    #if os(iOS)
    private var window: UIWindow?
    #else
    private var window: NSWindow?
    #endif
    private var session: ExactSession?

    override func tearDown() {
        ResetGate.spy = nil
        session?.destroy()
        session = nil
        #if os(iOS)
        window?.isHidden = true
        #else
        window?.close()
        #endif
        window = nil
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    private func boot(_ fixture: String = "RESET_GATE") throws -> (ExactSession, Data) {
        let plan = try fixturePlan(fixture)
        let session = ExactApp.shared.makeSession(label: "reset-gate")
        self.session = session
        let size = CGSize(width: 400, height: 600)
        #if os(iOS)
        let scene = hostedWindowScene()
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(origin: .zero, size: size))
        window.frame = CGRect(origin: .zero, size: size)
        let host = UIViewController()
        window.rootViewController = host
        window.makeKeyAndVisible()
        let view = ExactView(session: session)
        view.frame = host.view.bounds
        host.view.addSubview(view)
        #else
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.makeKeyAndOrderFront(nil)
        #endif
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: size).error)
        if fixture == "RESET_GATE" { until("the list mounts its rows") { (try? nodes(session).filter { testId($0)?.hasPrefix("row-") == true }.count) ?? 0 > 4 } }
        return (session, plan)
    }

    private func nodes(_ session: ExactSession) throws -> [[String: Any]] {
        let tree = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"tree"}"#).utf8)) as? [String: Any])
        return try XCTUnwrap(tree["nodes"] as? [[String: Any]])
    }
    private func testId(_ node: [String: Any]) -> String? { ((node["props"] as? [String: Any])?["testId"] as? String) ?? node["testId"] as? String }
    private func id(_ session: ExactSession, _ test: String) throws -> UInt32 {
        UInt32(try XCTUnwrap(try nodes(session).first { testId($0) == test }?["id"] as? Int, "no \(test)"))
    }
    /// The words of the tree's texts, wherever the tree keeps them.
    private func words(_ session: ExactSession) throws -> [String] {
        func strings(_ value: Any) -> [String] {
            switch value {
            case let s as String: return [s]
            case let d as [String: Any]: return d.values.flatMap(strings)
            case let a as [Any]: return a.flatMap(strings)
            default: return []
            }
        }
        return try nodes(session).flatMap(strings)
    }
    private func ticks(_ session: ExactSession) throws -> Int {
        try words(session).compactMap { $0.hasPrefix("Ticks ") ? Int($0.dropFirst(6)) : nil }.max() ?? -1
    }

    /// The field's native editor, first responder: an edit open for the teardown to end.
    private func edit(_ session: ExactSession, _ field: UInt32) throws {
        let node = try XCTUnwrap(session.presenter.views[field])
        #if os(iOS)
        let text = try XCTUnwrap(node.firstInTree { $0 is UITextField || $0 is UITextView ? $0 : nil })
        XCTAssertTrue(text.becomeFirstResponder(), "the field takes focus")
        #else
        func editor(_ v: NSView) -> NSView? {
            if v is NSTextField || v is NSTextView { return v }
            for child in v.subviews { if let found = editor(child) { return found } }
            return nil
        }
        let text = try XCTUnwrap(editor(node))
        XCTAssertTrue(window?.makeFirstResponder(text) == true, "the field takes focus")
        #endif
        session.presenter.typed(field, "draft", input: true)
    }

    func testEachHeldCallIsDroppedOrReplayedForAReason() {
        XCTAssertTrue(Set(ResetGate.replayed.keys).isDisjoint(with: ResetGate.dropped.keys), "a call is dropped or replayed, never both")
        XCTAssertTrue(Set(ResetGate.knownLoss.keys).isDisjoint(with: Set(ResetGate.replayed.keys).union(ResetGate.dropped.keys)), "a known loss is in no other table")
        XCTAssertTrue(ResetGate.replayed.values.allSatisfy { !$0.1.isEmpty } && ResetGate.dropped.values.allSatisfy { !$0.isEmpty } && ResetGate.knownLoss.values.allSatisfy { !$0.isEmpty }, "each with its reason")
        XCTAssertEqual(Set(ResetGate.replayed.values.map(\.0)), Set(ResetGate.Replay.allCases), "each replay has a call that asks it")
        let held = ["collectionFeedback(_:now:)", "focus(_:now:)", "advance(now:untilRequest:)", "setPage(_:)", "advance(now:untilRequest:)", "covers(_:)", "tellSoon(_:)"]
        XCTAssertEqual(ResetGate.replays(held), [.advance, .covers, .page, .collections], "each replay once, in order; a focus dropped")
        XCTAssertEqual(ResetGate.replays(["focus(_:now:)", "change(_:_:selection:now:)", "unlisted(_:)"]), [], "dropped, and an unlisted call with them")
    }

    /// The spy test: over a real restart nothing reaches the runtime, every held call is listed, and what is replayed
    /// is replayed once, in order, after the committed batch, reaching the new runtime; the timer due over the reset
    /// fires after it, and the edit the teardown ended never reaches the runtime.
    func testARestartReachesNoRuntimeAndReplaysWhatItHeldOnceInOrder() throws {
        let (session, plan) = try boot()
        try edit(session, try id(session, "field"))
        until("the timer runs: \((try? words(session)) ?? [])") { (try? ticks(session)) ?? 0 > 2 }
        // A native switch, laid out at the size its control measures and reports (not the kernel's fallback).
        let toggle = try id(session, "switch")
        func measured() -> CGSize { session.presenter.views[toggle]?.frame.size ?? .zero }
        func own() -> CGSize { session.presenter.controls.controls[toggle]?.intrinsicContentSize ?? .zero }
        until("the switch is laid out at its control's own measure: \(measured()) vs \(own())") {
            own().width > 0 && abs(measured().width - own().width) < 1 && abs(measured().height - own().height) < 1
        }
        let natural = measured(), oldControl = session.presenter.controls.controls[toggle].map(ObjectIdentifier.init)
        var reportsAfter: [CGSize] = []
        let reported = session.presenter.onIntrinsic
        defer { session.presenter.onIntrinsic = reported }
        session.presenter.onIntrinsic = { [weak session] sizes in
            if let session, !session.runtime.held { reportsAfter += sizes.filter { $0.0 == toggle }.compactMap(\.1) }
            reported?(sizes)
        }
        var seen: [ResetGate.Seen] = []
        ResetGate.spy = { seen.append($0) }
        XCTAssertTrue(session.apply(plan, label: "generation"))
        ResetGate.spy = nil
        guard case let .reset(entered, held)? = seen.first else { return XCTFail("no reset seen: \(seen)") }
        XCTAssertEqual(entered, 0, "a call reached the runtime inside the reset")
        let listed = Set(ResetGate.replayed.keys).union(ResetGate.dropped.keys).union(ResetGate.knownLoss.keys)
        XCTAssertEqual(held.filter { !listed.contains($0) }, [], "a held call ResetGate doesn't list")
        let replays = ResetGate.replays(held)
        let replayed = seen.dropFirst().compactMap { if case let .replayed(r, entered) = $0 { return (r, entered) } else { return nil } }
        XCTAssertEqual(replayed.count, replays.isEmpty ? 0 : 1, "replayed once: \(seen)")
        if case let (r, entered)? = replayed.first {
            XCTAssertEqual(r, replays)
            XCTAssertEqual(r, Array(Set(r)).sorted { $0.rawValue < $1.rawValue }, "each once, in order")
            XCTAssertGreaterThan(entered, 0, "the replay reached the new runtime")
        }
        XCTAssertEqual(session.runtime.heldCalls, [], "nothing left held")
        XCTAssertFalse(session.runtime.held)
        // The old switch's intrinsics are dropped; the new switch, a new control, measures and reports its own.
        until("the new tree's switch reports its own measure") { !reportsAfter.isEmpty }
        XCTAssertNotEqual(session.presenter.controls.controls[toggle].map(ObjectIdentifier.init), oldControl, "a new control")
        let report = reportsAfter.last ?? .zero
        XCTAssertTrue(abs(report.width - natural.width) < 1 && abs(report.height - natural.height) < 1, "the new control's report: \(report) vs \(natural)")
        until("the new tree's switch is laid out at its own measure: \(measured()) vs \(natural)") { measured() == natural }
        let after = try ticks(session)
        until("the timer due over the reset fires after it") { (try? ticks(session)) ?? 0 > after }
        let saved = try words(session).filter { $0.hasPrefix("Saved") }
        XCTAssertEqual(saved, ["Saved "], "the edit the teardown ended reached the new runtime")
    }

    /// Each path a review found, made as the teardown makes it with the runtime held: held, named as listed, nothing
    /// reaching the runtime; then dropped, or replayed once after.
    private func held(_ session: ExactSession, _ call: () -> Void) -> (held: [String], replays: [ResetGate.Replay], entered: Int) {
        let before = session.runtime.entered
        session.runtime.held = true
        call()
        session.runtime.held = false
        XCTAssertEqual(session.runtime.entered, before, "a call reached the held runtime")
        let held = session.runtime.heldCalls
        let listed = Set(ResetGate.replayed.keys).union(ResetGate.dropped.keys).union(ResetGate.knownLoss.keys)
        XCTAssertEqual(held.filter { !listed.contains($0) }, [], "a held call ResetGate doesn't list")
        let replayed = session.runtime.entered
        let replays = session.replayHeld()
        return (held, replays, session.runtime.entered - replayed)
    }

    /// An edit the teardown ends (iOS: the field resigning; the Mac: an alert's prompt withdrawn): the old field's.
    func testAnEditTheResetEndsIsDropped() throws {
        let (session, _) = try boot()
        let field = try id(session, "field")
        session.presenter.typed(field, "draft", input: false)
        let (held, replays, _) = held(session) { session.presenter.commitEdit(field, "draft", change: true) }
        XCTAssertTrue(held.contains("change(_:_:selection:now:)"), "\(held)")
        XCTAssertEqual(replays, [])
        until("a turn") { true }
        XCTAssertEqual(try words(session).filter { $0.hasPrefix("Saved") }, ["Saved "], "the dropped change was told")
    }

    /// A sheet giving focus back as the teardown dismisses it, and the old field's blur: the old tree's.
    func testFocusTheResetMovesIsDropped() throws {
        let (session, _) = try boot()
        let field = try id(session, "field")
        let (held, replays, _) = held(session) {
            _ = session.runtime.focus(field, now: session.now())
            _ = session.runtime.focusEvent(field, entering: true, target: "field", related: nil, now: session.now())
            _ = session.runtime.blur(field, now: session.now())
        }
        XCTAssertEqual(held, ["focus(_:now:)", "focusEvent(_:entering:target:related:now:)", "blur(_:now:)"])
        XCTAssertEqual(replays, [])
    }

    /// A Markdown editor the teardown refocuses reports its selection outside `on`: held too.
    func testASelectionTheResetReportsIsDropped() throws {
        let (session, _) = try boot()
        let field = try id(session, "field")
        var answer: Batch? = Runtime.heldBatch
        let (held, replays, _) = held(session) {
            answer = session.runtime.selection(field, json: "{}", now: session.now())
            #if os(iOS)
            session.presenter.selectionChange(field, "Ticks", 0, 1)
            #endif
        }
        XCTAssertNil(answer)
        #if os(iOS)
        XCTAssertEqual(held.first, "selection(_:json:now:)")
        XCTAssertTrue(held.dropFirst().allSatisfy { $0 == "selectionChange(_:_:start:end:rects:now:)" }, "\(held)")
        #else
        XCTAssertEqual(held, ["selection(_:json:now:)"])
        #endif
        XCTAssertEqual(replays, [])
    }

    /// A key window the teardown moves (the Mac's key sheet closing), told at once or as a notice: the page is told
    /// again after, once; the notice is never counted in flight.
    func testAPageTheResetMovesIsToldAgainAfter() throws {
        let (session, _) = try boot()
        let inFlight = session.noticesInFlight
        let (held, replays, entered) = held(session) {
            session.tellPage()
            session.tellSoon([{ $0.setPage(0) }])
            session.tellPage()
        }
        XCTAssertEqual(held.filter { $0 == "setPage(_:)" || $0 == "tellSoon(_:)" }, ["setPage(_:)", "tellSoon(_:)", "setPage(_:)"], "\(held)")
        XCTAssertEqual(session.noticesInFlight, inFlight, "a held notice counted in flight: it never lands")
        XCTAssertEqual(replays, [.page])
        XCTAssertGreaterThanOrEqual(entered, 1, "told again")
    }

    /// A timer coming due while held, and the list's report and the covers: the report and the covers are the new
    /// runtime's, replayed once, in order. The 50 ms `every` is a task's native timer (#1144): its firing held is kept
    /// by its own queue and fires after the hold, not in the replay.
    func testWorkTheNewRuntimeMustSeeIsReplayedOnce() throws {
        let (session, _) = try boot()
        until("the timer runs") { (try? ticks(session)) ?? 0 > 1 }
        let before = try ticks(session)
        let (held, replays, entered) = held(session) {
            RunLoop.main.run(until: Date().addingTimeInterval(0.25)) // the 50 ms ticker comes due, held
            _ = session.runtime.collectionFeedback(Data(), now: session.now())
            _ = session.runtime.covers([])
        }
        XCTAssertTrue(held.contains { $0.hasPrefix("fireNativeTimer") }, "the due task's native firing was held: \(held)")
        until("the task's firing held runs after the hold") { (try? ticks(session)) ?? 0 > before }
        XCTAssertTrue(held.contains("collectionFeedback(_:now:)") && held.contains("covers(_:)"), "\(held)")
        // (Boot's own work may come due in the hold too, a fit or the data source's activation: owed, then replayed.)
        XCTAssertEqual(replays.filter { [.covers, .collections].contains($0) }, [.covers, .collections])
        XCTAssertEqual(replays, ResetGate.replays(held), "each once, in order")
        _ = entered
        if replays.contains(.activation) { XCTAssertNil(session.pendingActivation, "the held activation ran in the replay") }
    }

    /// A frame's tick sent to the owner while held: answered there, so the tick in flight ends; the advance replayed.
    func testATickSentWhileHeldEndsAndIsReplayed() throws {
        let (session, _) = try boot()
        until("no tick in flight") { !session.tickInFlight }
        let (held, replays, _) = held(session) { session.sendTick(now: session.now()) }
        XCTAssertEqual(held, ["tickAsync(now:frame:capture:measureFrame:done:)"])
        XCTAssertEqual(replays, [.advance])
        until("the held tick's answer lands, ending the tick in flight") { !session.tickInFlight }
    }

    /// The clock's other two calls, a timer's advance sent to the owner and a display frame, held: replayed as the
    /// tick is. (#1141 and #1144 renamed all three; each fell out of `replayed`, and was dropped.)
    func testAnAdvanceAndAFrameSentWhileHeldAreReplayed() throws {
        let (session, _) = try boot()
        let landed = expectation(description: "the held advance is answered on the owner")
        let (held, replays, entered) = held(session) {
            session.runtime.advanceAsync(now: session.now()) { batch in XCTAssertTrue(batch.held); landed.fulfill() }
            _ = session.runtime.frame(now: session.now())
        }
        XCTAssertEqual(held, ["advanceAsync(now:frame:admitted:capture:measureFrame:done:)", "frame(now:wall:admitted:)"])
        XCTAssertEqual(replays, [.advance])
        _ = entered
        wait(for: [landed], timeout: 5)
    }

    /// A task's native firing held is kept by its own queue, which delivers it after (no replay), on the synchronous
    /// path (the Mac's) and the one posted to the owner; a firing queued when a real restart commits is dropped with
    /// its generation, whose rows re-arm the task's timer (#1144; ResetGate.dropped). Each delivery is the queue's own.
    func testANativeFiringHeldIsDeliveredAfterAndARestartReArmsIt() throws {
        let wasAsync = ExactSession.asyncFills
        defer { ExactSession.asyncFills = wasAsync }
        for asynchronous in [false, true] {
            ExactSession.asyncFills = asynchronous
            let (session, plan) = try boot()
            until("the timer runs") { (try? ticks(session)) ?? 0 > 1 }
            let row = try XCTUnwrap(session.taskTimers.rows.first, "the ticker's native timer")
            let timer = try XCTUnwrap(session.taskTimers.timerForTests(row.index))
            while session.now() < row.due + 1 { Thread.sleep(forTimeInterval: 0.002) } // its beat is due
            let before = try ticks(session)
            let (firing, replays, _) = held(session) {
                timer.fire()
                // The queue's own ask, held; posted to the owner, its held answer lands and the firing is re-queued.
                let deadline = Date().addingTimeInterval(2)
                repeat { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.005)) }
                while !(session.taskTimers.hasReady && !session.advanceInFlight && session.runtime.heldCalls.contains { $0.hasPrefix("fireNativeTimer") }) && Date() < deadline
            }
            XCTAssertTrue(session.taskTimers.hasReady, "the held firing is retained at the front of its queue (that it then runs once is NativeTaskTimersTests')")
            let call = asynchronous ? "fireNativeTimerAsync(_:now:done:)" : "fireNativeTimer(_:now:)"
            XCTAssertTrue(firing.contains(call), "\(firing)")
            XCTAssertFalse(replays.contains(.advance), "not replayed: its queue keeps it")
            XCTAssertEqual(try ticks(session), before)
            until("delivered after the hold, by its queue") { (try? ticks(session)) ?? 0 > before }
            // A firing queued as a real restart commits: dropped with the old generation, the task re-armed.
            let old = try XCTUnwrap(session.taskTimers.timerForTests(row.index))
            old.fire() // queued unless a firing of it is pending already: either is the old generation's
            XCTAssertTrue(session.apply(plan, label: "generation"))
            XCTAssertFalse(session.taskTimers.hasReady, "the old generation's queued firing is dropped with it")
            let rearmed = try XCTUnwrap(session.taskTimers.timerForTests(row.index), "the new generation's rows re-arm the task")
            XCTAssertFalse(rearmed === old)
            let after = try ticks(session)
            until("the new generation's timer fires, by its queue") { (try? ticks(session)) ?? 0 > after }
            session.destroy(); self.session = nil
            #if os(iOS)
            window?.isHidden = true
            #else
            window?.close()
            #endif
            window = nil
        }
    }

    /// A timer admission check held (#1144's `wait=update`), on both paths, leaves nothing pending; after a real
    /// restart the new generation's next update check admits the firing (ResetGate.dropped). (An update-wait is
    /// admitted on an update by design: the check this test makes is the one the next update callback makes.)
    func testAnAdmissionCheckHeldLeavesNothingPendingAndARestartAdmits() throws {
        let wasAsync = ExactSession.asyncFills
        defer { ExactSession.asyncFills = wasAsync }
        for asynchronous in [false, true] {
            ExactSession.asyncFills = asynchronous
            let (session, plan) = try boot("TIMER_ADMISSION")
            func count() -> Int? { (try? words(session))?.compactMap { Int($0) }.max() }
            until("booted, and nothing in flight") { count() == 0 && !session.ownerInFlight && !session.applying && !session.advanceInFlight }
            let cutoff = session.now() + 200 // past the 100 ms timers' first due
            let (checked, replays, _) = held(session) {
                session.timerAdmissions.check(frame: false, update: true, at: cutoff)
                // A posted check runs, held, and its held answer lands.
                let deadline = Date().addingTimeInterval(2)
                repeat { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.005)) }
                while (session.timerAdmissions.hasPendingForTests || session.advanceInFlight) && Date() < deadline
            }
            XCTAssertTrue(checked.contains(asynchronous ? "timerWaitsAsync(kinds:cutoff:done:)" : "timerWaits(kinds:cutoff:mode:)"), "\(checked)")
            XCTAssertFalse(replays.contains(.advance), "not replayed")
            XCTAssertEqual(count(), 0, "nothing fired while held")
            XCTAssertFalse(session.timerAdmissions.hasPendingForTests, "the held check left nothing pending")
            XCTAssertTrue(session.apply(plan, label: "generation"))
            session.timerAdmissions.check(update: true, at: session.now() + 200) // what the next update callback calls
            until("the next update's check admits the firing") { count() == 1 }
            session.destroy(); self.session = nil
            #if os(iOS)
            window?.isHidden = true
            #else
            window?.close()
            #endif
            window = nil
        }
    }

    /// A resize while a restart holds the runtime (another session's reset, a keyboard it ends): the view's fit is owed,
    /// its last-told size untouched, and replayed after, so the runtime lays out at the new size.
    func testAResizeWhileHeldReachesTheRuntimeAfter() throws {
        let (session, _) = try boot()
        let root = try id(session, "root")
        func width() -> CGFloat { session.presenter.views[root]?.frame.width ?? 0 }
        until("laid out at 400") { width() == 400 }
        let (held, replays, _) = held(session) {
            #if os(iOS)
            session.view?.frame.size.width = 320
            session.view?.layoutIfNeeded()
            #else
            window?.setContentSize(CGSize(width: 320, height: 600))
            window?.contentView?.layoutSubtreeIfNeeded()
            #endif
            RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        }
        XCTAssertTrue(held.contains { ResetGate.replayed[$0]?.0 == .fit }, "the fit was owed: \(held)")
        XCTAssertEqual(replays.first, .fit)
        until("the runtime lays out at the new size after the replay") { width() == 320 }
    }
}
#endif

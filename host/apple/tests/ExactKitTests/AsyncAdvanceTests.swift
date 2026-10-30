#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
import ExactCore
@testable import ExactKit

/// LLP 1072.000: a timer's advance is posted to the owner, and main does not
/// wait for it. Its commit is held here between the owner making it and main
/// applying it, the window the ruling (§7.1 (a)) settles: an input on a row
/// the commit removed is refused and logged, one on a row it changed runs
/// against the current state, and nothing crashes. A delayed advance
/// observes the clock when it runs and keeps its repeat's phase.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class AsyncAdvanceTests: XCTestCase {
    private var sessions: [ExactSession] = []
    #if os(iOS)
    private var windows: [UIWindow] = []
    #else
    private var windows: [NSWindow] = []
    #endif
    private var wasAsync = false

    override func setUp() {
        super.setUp()
        wasAsync = ExactSession.asyncFills
        // Read when a session wires itself; the Mac keeps advances synchronous in the app.
        ExactSession.asyncFills = true
    }

    override func tearDown() {
        sessions.forEach { $0.destroy() }
        sessions = []
        #if os(iOS)
        windows.forEach { $0.isHidden = true }
        #else
        windows.forEach { $0.close() }
        #endif
        windows = []
        ExactSession.asyncFills = wasAsync
        super.tearDown()
    }

    private func booted() throws -> ExactSession {
        let plan = try fixturePlan("ASYNC_ADVANCE")
        let session = ExactApp.shared.makeSession(label: "async-advance")
        sessions.append(session)
        let size = CGSize(width: 400, height: 500)
        #if os(iOS)
        let window = UIWindow(frame: CGRect(origin: .zero, size: size))
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
        #endif
        windows.append(window)
        XCTAssertNil(session.boot(plan: plan, size: size).error)
        return session
    }

    private func id(_ session: ExactSession, _ testId: String) -> UInt32? {
        session.presenter.views.values.first { $0.props["testId"] as? String == testId }?.id
    }

    private func slot(_ session: ExactSession, _ name: String) -> Any? {
        let json = try? JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any]
        return (json?["slots"] as? [String: Any])?[name]
    }

    private func logs(_ session: ExactSession) -> String { session.agent(#"{"op":"logs"}"#) }

    /// A task's timer is Foundation's (#1144): once its first row is due, its
    /// firing, delivered as the run loop delivers it (`deliverNativeTimer`):
    /// posted to the owner. The row it fired.
    @discardableResult private func deliverTask(_ session: ExactSession, beforeDelivery: () -> Void = {}) throws -> NativeTimerWake {
        let row = try XCTUnwrap(session.taskTimers.rows.first, "the task's native timer")
        while session.now() < row.due + 1 { Thread.sleep(forTimeInterval: 0.002) }
        try XCTUnwrap(session.taskTimers.timerForTests(row.index)).fire()
        beforeDelivery()
        session.deliverNativeTimer()
        return row
    }

    /// The `tick` timer's commit, made on the owner and published, not yet
    /// applied: the screen still shows row `a` at the old price.
    private func holdTick(_ session: ExactSession) throws {
        session.presenter.onPress?(try XCTUnwrap(id(session, "arm")))
        try deliverTask(session) // `after(1, tick)` is due
        XCTAssertTrue(session.advanceInFlight, "posted, not run while main waits")
        Owner.shared.sync {} // the owner has made and published it; main has not landed it
        XCTAssertTrue(session.advanceInFlight)
        XCTAssertNotNil(id(session, "row-a"), "the removed row is still shown")
        XCTAssertEqual(slot(session, "price") as? Double, 2, "the owner is ahead of the screen")
    }

    /// Every input family the host delivers by view id, against a row the
    /// held commit removed: refused and logged, the commit stands.
    func testAnInputOnARowAHeldAdvanceRemovedIsRefusedAndLogged() throws {
        var entries: [(String, (ExactSession, UInt32, UInt32) -> Void)] = [
            ("press", { s, row, _ in s.presenter.onPress?(row) }),
            ("pointerdown", { s, row, _ in
                s.presenter.onPointer?(row, .down, PointerSample(x: 4, y: 4, buttons: 1, pressure: 0.5, type: "touch", id: 1, clientX: 4, clientY: 4))
            }),
            ("focus", { s, row, _ in s.presenter.onFocus?(row) }),
            ("contextmenu", { s, row, _ in s.presenter.onContextmenu?(row) }),
            ("input", { s, _, field in s.presenter.onInput?(field, "x") }),
            ("change", { s, _, field in s.presenter.onChange?(field, "x") }),
        ]
        #if os(iOS)
        // VoiceOver's activation reaches the press through the view UIKit still shows.
        entries.append(("accessibility activate", { s, row, _ in _ = (s.presenter.views[row] as UIView?)?.accessibilityActivate() }))
        #endif
        for (name, deliver) in entries {
            let session = try booted()
            let row = try XCTUnwrap(id(session, "row-a")), field = try XCTUnwrap(id(session, "field-a"))
            try holdTick(session)
            deliver(session, row, field)
            session.drainFill()
            XCTAssertFalse(session.advanceInFlight, name)
            XCTAssertNil(id(session, "row-a"), "\(name): the commit applied")
            XCTAssertEqual(slot(session, "log") as? String, "", "\(name): no action ran")
            XCTAssertEqual(slot(session, "price") as? Double, 2, "\(name): the commit stands")
            if name != "accessibility activate" {
                XCTAssertTrue(logs(session).contains("UnknownView(\(name == "input" || name == "change" ? field : row))"), "\(name): the refusal is logged")
            }
        }
    }

    /// A row the held commit changed but kept: the input runs with the
    /// owner's current values, not the ones shown (§7.1 (a)).
    func testAnInputOnARowAHeldAdvanceChangedRunsAgainstCurrentState() throws {
        let session = try booted()
        let row = try XCTUnwrap(id(session, "row-b")), field = try XCTUnwrap(id(session, "field-b"))
        try holdTick(session)
        session.presenter.onPress?(row)
        XCTAssertEqual(slot(session, "log") as? String, "b@2 ", "the current price, though the row showed 1")
        XCTAssertFalse(session.advanceInFlight, "the held commit applied before the press's batch")
        session.presenter.onInput?(field, "x")
        XCTAssertEqual(slot(session, "log") as? String, "b@2 b:x ")
        XCTAssertNotNil(id(session, "row-b"))
        XCTAssertNil(id(session, "row-a"))
    }

    /// On the live clock a delayed advance observes the clock when the owner
    /// runs it, as one main waited for does, and its repeat keeps the phase
    /// of its scheduled beat (the timers RFC D3).
    func testADelayedAdvanceObservesWhenItRunsAndKeepsItsPhase() throws {
        let session = try booted()
        session.presenter.onPress?(try XCTUnwrap(id(session, "beat")))
        // Anything the press left in flight lands first, so the firing this test delivers is the only one in flight,
        // and its beats count from there. (The task's own Foundation timer fires only when the run loop runs, which
        // this test never lets it.)
        for _ in 0..<10 where session.advanceInFlight { session.drainFill() }
        XCTAssertFalse(session.advanceInFlight)
        let before = try XCTUnwrap(slot(session, "beats") as? Double)
        let release = DispatchSemaphore(value: 0)
        var posted = 0.0
        let due = try deliverTask(session) { // returns while the owner is busy: main is free
            Owner.shared.post { _ = release.wait(timeout: .now() + 5) }
            posted = session.now()
        }.due
        Thread.sleep(forTimeInterval: 0.06)
        release.signal()
        // What the posted firing did, read through the owner before main lands it.
        XCTAssertEqual(slot(session, "beats") as? Double, before + 1, "missed beats fire once")
        let seen = try XCTUnwrap(slot(session, "seen") as? Double)
        XCTAssertGreaterThanOrEqual(seen, posted + 55, "`now` is when the owner ran it")
        session.drainFill()
        // (the landed advance's next beat: one posted on landing has not landed, and is in flight only if that beat is due)
        let next = try XCTUnwrap(session.taskTimers.nextDue)
        XCTAssert(!session.advanceInFlight || session.now() >= next, "nothing in flight but the next beat's, due already")
        XCTAssertGreaterThan(next, seen)
        XCTAssertLessThanOrEqual(next, seen + 40)
        XCTAssertEqual((next - due).truncatingRemainder(dividingBy: 40), 0, accuracy: 0.001, "on the phase of the beat it was scheduled for")
    }

    /// A task's native wake is posted to the owner; it does not run while main
    /// waits (`deliverNativeTimer`, as `advanceIfDue` posts a runtime timer's).
    func testATasksNativeWakeIsPosted() throws {
        let session = try booted()
        session.presenter.onPress?(try XCTUnwrap(id(session, "beat")))
        let due = try deliverTask(session).due
        XCTAssertTrue(session.advanceInFlight, "posted")
        XCTAssertEqual(slot(session, "beats") as? Double, 1, "the owner runs it before a later read")
        session.drainFill()
        // Landed: its next beat is scheduled past it, and one in flight now is only that beat's.
        let next = try XCTUnwrap(session.taskTimers.nextDue)
        XCTAssertGreaterThan(next, due, "landed")
        XCTAssert(!session.advanceInFlight || session.now() >= next, "nothing in flight but the next beat's, due already")
    }
}
#endif

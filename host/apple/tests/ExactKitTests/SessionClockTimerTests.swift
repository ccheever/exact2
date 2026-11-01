import CoreFoundation
import Foundation
import QuartzCore
import XCTest
#if os(macOS)
import AppKit
#endif
@testable import ExactKit
import ExactCore

// Exercise the production decoder even when a fixture is written as JSON values.
func batchFixture(ops: [[String: Any]], timers: Bool, motion: Bool, clock: Double?, error: String?, timerDueMs: Double? = nil, pending: Bool = false) -> Batch {
    let wire: [String: Any] = ["ops": ops, "timers": timers, "motion": motion,
        "clock": clock as Any? ?? NSNull(), "error": error as Any? ?? NSNull(),
        "timer_due_ms": timerDueMs as Any? ?? NSNull(), "pending": pending]
    return try! JSONDecoder().decode(Batch.self, from: JSONSerialization.data(withJSONObject: wire))
}

private final class ClockTicks: @unchecked Sendable {
    // The production timer and this oracle run only on the main run loop.
    var count = 0
    func tick() {
        precondition(Thread.isMainThread)
        count += 1
    }
}

final class SessionClockTimerTests: XCTestCase {
    func testAgentLaunchPlaceDefaultsAndOverrides() {
        let defaults = LaunchPlace(environment: ["EXACT_AGENT": "1"])
        XCTAssertEqual(defaults.locale, "en-US")
        XCTAssertEqual(defaults.timeZone, "UTC")
        XCTAssertEqual(defaults.seed, 1)
        XCTAssertEqual(defaults.epoch, 1_767_225_600_000)
        XCTAssertNil(LaunchPlace(environment: [:]).epoch)
        let custom = LaunchPlace(environment: ["EXACT_AGENT": "1",
            "EXACT_AGENT_LOCALE": "fr-CA", "EXACT_AGENT_TIME_ZONE": "America/Toronto",
            "EXACT_AGENT_SEED": "4294967295", "EXACT_AGENT_EPOCH": "1790000000000"])
        XCTAssertEqual(custom.locale, "fr-CA")
        XCTAssertEqual(custom.timeZone, "America/Toronto")
        XCTAssertEqual(custom.seed, (1 << 32) - 1)
        // The launch seed is below 2^32 on every host (LLP 1116.005 D5): a
        // drawn one, and a given one past it is 1, as the agent's stream takes it.
        for _ in 0..<64 { XCTAssertLessThan(LaunchPlace(environment: [:]).seed, 1 << 32) }
        XCTAssertEqual(LaunchPlace(environment: ["EXACT_AGENT": "1", "EXACT_AGENT_SEED": "4294967296"]).seed, 1)
        XCTAssertEqual(custom.epoch, 1_790_000_000_000)
        // The Region's format: the agent's own, else none (the runner answers
        // the locale); off the agent, `Locale.current`.
        XCTAssertEqual(defaults.formatLocale, "")
        XCTAssertEqual(LaunchPlace(environment: ["EXACT_AGENT": "1", "EXACT_AGENT_FORMAT_LOCALE": "en-KR"]).formatLocale, "en-KR")
        XCTAssertEqual(LaunchPlace(environment: [:]).formatLocale, SessionFacts.currentFormatLocale)
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let launch = session.launchPlace
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        session.tellTime()
        session.tellTime()
        XCTAssertEqual(session.launchPlace, launch)
    }

    /// The hour cycle the user forces (`AppleICUForce24HourTime`,
    /// `AppleICUForce12HourTime`) is the format locale's `hc` keyword, told
    /// again when Foundation says the locale changed. The argument domain
    /// stands in for the setting.
    func testTheFormatLocaleCarriesAForcedHourCycle() throws {
        let region = Locale(identifier: "en_US@rg=gbzzzz")
        XCTAssertEqual(SessionFacts.formatLocale(region, hourCycle: nil), "en-US-u-rg-gbzzzz")
        XCTAssertEqual(SessionFacts.formatLocale(region, hourCycle: .oneToTwelve), "en-US-u-hc-h12-rg-gbzzzz")
        XCTAssertEqual(SessionFacts.formatLocale(Locale(identifier: "en_KR"), hourCycle: .zeroToTwentyThree), "en-KR-u-hc-h23")
        XCTAssertEqual(SessionFacts.formatLocale(Locale(identifier: "en_US@hours=h11"), hourCycle: .zeroToTwentyThree), "en-US-u-hc-h23")
        let defaults = UserDefaults.standard, arguments = defaults.volatileDomain(forName: UserDefaults.argumentDomain)
        defer {
            defaults.setVolatileDomain(arguments, forName: UserDefaults.argumentDomain)
            NotificationCenter.default.post(name: NSLocale.currentLocaleDidChangeNotification, object: nil) // any live session reads it back
        }
        func force(_ hours: Int?) {
            var domain = arguments
            domain["AppleICUForce24HourTime"] = hours == 24
            domain["AppleICUForce12HourTime"] = hours == 12
            defaults.setVolatileDomain(domain, forName: UserDefaults.argumentDomain)
        }
        let h23 = SessionFacts.formatLocale(.current, hourCycle: .zeroToTwentyThree)
        force(nil)
        XCTAssertEqual(SessionFacts.currentFormatLocale, Locale.current.identifier(.bcp47))
        force(12)
        XCTAssertEqual(SessionFacts.currentFormatLocale, SessionFacts.formatLocale(.current, hourCycle: .oneToTwelve))
        force(24)
        XCTAssertEqual(SessionFacts.currentFormatLocale, h23)
        XCTAssertEqual(LaunchPlace(environment: ["EXACT_AGENT": "1"]).formatLocale, "")
        #if os(macOS)
        force(nil)
        let source = "shape Time\n  formatLocale: string\ncomponent App\n  resource time = exactTime() as shape Time\n  view\n    text time.formatLocale testId=\"format\"\n"
        let session = ExactSession(app: ExactApp.shared, label: "format-locale", launchPlace: LaunchPlace(environment: [:]))
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try TableFixture.plan(source), size: CGSize(width: 390, height: 844)).error)
        func shows(_ tag: String) -> Bool { session.agent("{\"op\":\"tree\"}").contains("\"\(tag)\"") }
        func wait(for tag: String) -> Bool {
            let deadline = Date().addingTimeInterval(10)
            while !shows(tag), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
            return shows(tag)
        }
        XCTAssertTrue(wait(for: Locale.current.identifier(.bcp47)))
        force(24)
        NotificationCenter.default.post(name: NSLocale.currentLocaleDidChangeNotification, object: nil)
        XCTAssertTrue(wait(for: h23), "the running app reads the forced cycle")
        XCTAssertEqual(session.launchPlace.formatLocale, h23)
        #endif
    }

    func testTargetedLayoutDigestStaysWithEachSessionsNode() throws {
        #if os(macOS)
        let a = ExactApp.shared.makeSession(), b = ExactApp.shared.makeSession()
        defer { a.destroy(); b.destroy() }
        XCTAssertNil(a.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertNil(b.boot(size: CGSize(width: 390, height: 844)).error)
        func inspect(_ session: ExactSession, mapped: Bool) throws -> [String: Any] {
            let bytes = try XCTUnwrap(session.agent("{\"op\":\"tree\"}").data(using: .utf8))
            let tree = try XCTUnwrap(JSONSerialization.jsonObject(with: bytes) as? [String: Any])
            let roots = try XCTUnwrap(tree["roots"] as? [Int])
            let id = try XCTUnwrap(roots.first)
            let reply = session.agentInstance.layout(["id": id, "plan": mapped])
            let node = try XCTUnwrap(reply["node"] as? [String: Any])
            XCTAssertEqual(node["id"] as? Int, id)
            return node
        }
        XCTAssertNil(try inspect(a, mapped: false)["planDigest"])
        let first = try inspect(a, mapped: true)
        let digest = try XCTUnwrap(first["planDigest"] as? String)
        XCTAssertEqual(digest.count, 64)
        XCTAssertNil(try inspect(b, mapped: false)["planDigest"])
        XCTAssertEqual(try inspect(b, mapped: true)["planDigest"] as? String, digest)
        XCTAssertEqual(try inspect(a, mapped: true)["planDigest"] as? String, digest)
        XCTAssertNil(first["plan"], "inspection must not carry the plan or its text")
        #endif
    }

    func testAgentPreferencesReachEveryLiveSession() throws {
        #if os(macOS)
        let before = DisplayPreferences.agent
        defer { DisplayPreferences.agent = before }
        DisplayPreferences.agent = (false, false)
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "shape Preferences\n  prefersReducedMotion: bool\n  prefersReducedTransparency: bool\ncomponent App\n  resource prefs = exactViewport() as shape Preferences\n  view\n    column\n      text (prefs.prefersReducedMotion ? \"reduce motion\" : \"allow motion\")\n      text (prefs.prefersReducedTransparency ? \"reduce transparency\" : \"allow transparency\")\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let plan = try Data(contentsOf: dir.appendingPathComponent("app.plan"))
        let a = ExactApp.shared.makeSession(), b = ExactApp.shared.makeSession()
        defer { a.destroy(); b.destroy() }
        for session in [a, b] {
            XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 390, height: 844)).error)
        }
        let reply = a.agentInstance.prefer(["media": ["prefers-reduced-motion": "reduce", "prefers-reduced-transparency": "reduce"]])
        XCTAssertNil(reply["error"])
        for session in [a, b] {
            let words = session.presenter.views.values.compactMap { $0.props["text"] }
            XCTAssertTrue(words.contains("reduce motion"), "\(session.label): \(words)")
            XCTAssertTrue(words.contains("reduce transparency"), "\(session.label): \(words)")
        }
        let c = ExactApp.shared.makeSession()
        defer { c.destroy() }
        XCTAssertNil(c.boot(plan: plan, size: CGSize(width: 390, height: 844)).error)
        XCTAssertTrue(c.presenter.views.values.contains { $0.props["text"] == "reduce motion" })
        #endif
    }

    func testScreenNotificationsRepublishDynamicRange() throws {
        #if os(macOS)
        let old = DisplayRange.pinned
        defer { DisplayRange.pinned = old }
        DisplayRange.pinned = 1
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "shape Display\n  dynamicRange: string\ncomponent App\n  resource display = exactViewport() as shape Display\n  view\n    text display.dynamicRange\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let view = ExactView(session: session)
        XCTAssertNil(session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 100, height: 100)).error)
        XCTAssertTrue(session.presenter.views.values.contains { $0.props["text"] == "standard" })
        DisplayRange.pinned = 4 // no preferences notification: only the screen change below
        NotificationCenter.default.post(name: NSApplication.didChangeScreenParametersNotification, object: NSApplication.shared)
        XCTAssertTrue(session.presenter.views.values.contains { $0.props["text"] == "high" })
        withExtendedLifetime(view) {}
        #endif
    }

    private func run(_ mode: RunLoop.Mode, seconds: TimeInterval) {
        let end = Date(timeIntervalSinceNow: seconds)
        while Date() < end && RunLoop.main.run(mode: mode, before: end) {}
    }

    /// Runs `mode` until `ticks` has ticked, for `seconds` at most. A timer
    /// firing does not end a run-loop pass, so each pass is 10 ms at most.
    private func run(_ mode: RunLoop.Mode, untilTicked ticks: ClockTicks, seconds: TimeInterval = 2) {
        let end = Date(timeIntervalSinceNow: seconds)
        while ticks.count == 0 && Date() < end {
            RunLoop.main.run(mode: mode, before: min(end, Date(timeIntervalSinceNow: 0.01)))
        }
    }

    private func trackingMode() -> RunLoop.Mode {
        // Model the existing AppKit tracking/common-mode membership without
        // a window, synthetic delegate notification or an agent clock.
        let mode = RunLoop.Mode("ExactSessionClockTrackingTest")
        CFRunLoopAddCommonMode(CFRunLoopGetMain(), CFRunLoopMode(rawValue: mode.rawValue as CFString))
        return mode
    }

    func testClockRunsInDefaultMode() {
        XCTAssertTrue(Thread.isMainThread)
        let ticks = ClockTicks()
        let timer = SessionClockTimer.schedule(after: 0.025) { _ in ticks.tick() }
        defer { timer.invalidate() }
        XCTAssertEqual(timer.timeInterval, 0)
        run(.default, untilTicked: ticks)
        XCTAssertGreaterThan(ticks.count, 0)
    }

    func testClockPreservesInitialDeadlineAndOnlyFiresOnce() {
        let before = Date(), ticks = ClockTicks()
        let timer = SessionClockTimer.schedule(after: 0.05) { _ in ticks.tick() }
        let after = Date()
        defer { timer.invalidate() }
        XCTAssertGreaterThanOrEqual(timer.fireDate, before.addingTimeInterval(0.05))
        XCTAssertLessThanOrEqual(timer.fireDate, after.addingTimeInterval(0.05))
        run(.default, seconds: 0.2)
        XCTAssertEqual(ticks.count, 1)
        XCTAssertFalse(timer.isValid)
    }

    func testEveryOrdinaryDeadlineUsesANativeWake() {
        XCTAssertEqual(SessionClockTimer.wake(due: 116, now: 100), .timeout(16))
        XCTAssertEqual(SessionClockTimer.wake(due: 0, now: 100), .timeout(0))
        XCTAssertEqual(SessionClockTimer.wake(due: 1100, now: 100), .timeout(1000))
        XCTAssertEqual(SessionClockTimer.wake(due: nil, now: 100), .none)
        XCTAssertEqual(SessionClockTimer.wake(due: 116, now: 100, agent: true), .none)
    }

    func testNearTimersNeverAddDisplayLinkDemandAndKeepAnUnchangedWake() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        session.clock = nil
        let due = session.now() + 40
        let empty = batchFixture(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due)
        session.applyUnlessEmpty(empty)
        XCTAssertFalse(session.frames.timerSoon)
        XCTAssertNil(session.frames.link)
        let timer = try XCTUnwrap(session.clockTimer)
        for _ in 0..<3 {
            session.applyUnlessEmpty(empty)
            XCTAssertTrue(session.clockTimer === timer)
            XCTAssertFalse(session.frames.timerSoon)
        }
        session.apply(empty)
        XCTAssertTrue(session.clockTimer === timer)
        XCTAssertNil(session.frames.link)
    }

    func testNestedTransformBatchPreservesTimerDeadline() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        session.clock = nil
        for delta in [16.0, 1000.0] {
            let due = session.now() + delta
            let data = try JSONSerialization.data(withJSONObject: ["accepted": true,
                "batch": ["ops": [], "timers": true, "timer_due_ms": due]])
            let reply = try XCTUnwrap(TransformDragReply(data))
            XCTAssertEqual(reply.batch.timerDueMs, due)
            session.apply(reply.batch)
            XCTAssertFalse(session.frames.timerSoon)
            XCTAssertNil(session.frames.link)
            XCTAssertNotNil(session.clockTimer)
        }
    }

    func testSlowTimerDoesNotRunDisplayLinkBetweenTicks() {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        session.clock = nil
        session.scheduleClock(due: session.now() + 16)
        XCTAssertFalse(session.frames.timerSoon)
        XCTAssertNil(session.frames.link)
        XCTAssertNotNil(session.clockTimer)
        session.scheduleClock(due: session.now() + 1000)
        XCTAssertFalse(session.frames.timerSoon)
        XCTAssertNil(session.frames.link)
        XCTAssertNotNil(session.clockTimer)
        session.scheduleClock(due: nil)
        XCTAssertNil(session.clockTimer)
    }

    func testClockRunsWhileTrackingModeIsActive() {
        XCTAssertTrue(Thread.isMainThread)
        let mode = trackingMode(), ticks = ClockTicks()
        let timer = SessionClockTimer.schedule(after: 0.025) { _ in ticks.tick() }
        defer { timer.invalidate() }
        run(mode, untilTicked: ticks)
        XCTAssertGreaterThan(ticks.count, 0, "tracking must not suspend the session clock")
    }

    func testClockInvalidationRemovesTrackingDelivery() {
        XCTAssertTrue(Thread.isMainThread)
        let mode = trackingMode(), ticks = ClockTicks()
        let timer = SessionClockTimer.schedule(after: 0.025) { _ in ticks.tick() }
        run(mode, untilTicked: ticks)
        XCTAssertGreaterThan(ticks.count, 0)
        timer.invalidate()
        let stopped = ticks.count
        run(mode, seconds: 0.1)
        XCTAssertFalse(timer.isValid)
        XCTAssertEqual(ticks.count, stopped)
    }
}


extension SessionClockTimerTests {
    func testTimerAdmissionObservationsResampleDemandAndEveryIdleTerm() {
        var observation = TimerAdmissions.Observation(now: 100, waits: [-1, -1, -1, -2])
        XCTAssertTrue(observation.idle)
        XCTAssertEqual(observation.kinds, 4 | 8)
        observation.frame = true
        XCTAssertEqual(observation.kinds, 2 | 4 | 8)
        observation.frame = false
        for term in 0..<5 {
            var blocked = observation
            switch term {
            case 0: blocked.scrolling = true
            case 1: blocked.motion = true
            case 2: blocked.applying = true
            case 3: blocked.waits[1] = -2
            default: blocked.next = blocked.now + 1000 / 60
            }
            XCTAssertFalse(blocked.idle, "Idle term \(term) blocks admission")
        }
        observation.next = (observation.now + 1000 / 60).nextUp
        XCTAssertTrue(observation.idle)
        observation.scrolling = true
        XCTAssertEqual(observation.kinds, 0)
        observation.hidden = true
        XCTAssertTrue(observation.idle)
        XCTAssertEqual(observation.kinds, 2 | 4 | 8)
        observation.hidden = false; observation.scrolling = false
        observation.tasks = true
        XCTAssertFalse(observation.idle)
        XCTAssertEqual(observation.kinds, 8)
        observation.tasks = false; observation.next = nil; observation.lastUpdate = observation.now
        XCTAssertFalse(observation.idle)
        observation.update = true
        XCTAssertEqual(observation.kinds, 4 | 8)
        observation.update = false
        observation.now += 1000 / 60
        XCTAssertFalse(observation.idle)
        observation.now = observation.now.nextUp; observation.next = nil
        XCTAssertTrue(observation.idle)
        XCTAssertEqual(observation.kinds, 4 | 8)
    }

    func testHeldFiringsWaitForEligibleObservationsWithoutRequestingOwnerWork() {
        for index in 0..<4 {
            var observation = TimerAdmissions.Observation(now: 100, lastUpdate: 100, scrolling: true,
                                                        waits: Array(repeating: -1, count: 4))
            observation.waits[index] = -2
            XCTAssertFalse(observation.eligible, "An ineligible held firing needs no owner turn")
            observation.frame = true
            XCTAssertEqual(observation.eligible, index == 0)
            observation.frame = false; observation.update = true
            XCTAssertEqual(observation.eligible, index == 1)
            observation.update = false; observation.scrolling = false
            XCTAssertEqual(observation.eligible, index == 2)
            observation.waits[index] = 101
            XCTAssertFalse(observation.eligible)
            observation.waits[index] = 100
            XCTAssertTrue(observation.eligible, "A nominal deadline still needs its first holding check")
        }
    }

    func testTolerantWakesUseNativeTimerTolerance() {
        XCTAssertEqual(SessionClockTimer.wake(due: 100, now: 0), .timeout(100))
        XCTAssertEqual(SessionClockTimer.wake(due: 100, now: 0), .timeout(100))
        let timer = SessionClockTimer.schedule(after: 100, tolerance: 0.05) { _ in XCTFail("The test invalidates its timer before it is due") }
        XCTAssertEqual(timer.tolerance, 0.05)
        timer.invalidate()
    }

    func testTimerAdmissionMetadataUsesTheProductionBatchDecoder() throws {
        let batch = try JSONDecoder().decode(Batch.self, from: Data("{\"ops\":[],\"timers\":true,\"motion\":false,\"waits\":[-2,50,-1,100],\"waitContinue\":true,\"nativeTimers\":[{\"index\":0,\"generation\":\"18446744073709551615\",\"due\":100,\"interval\":100,\"tolerance\":25,\"once\":true}]}".utf8))
        XCTAssertEqual(batch.timerWaits, [-2, 50, -1, 100])
        XCTAssertTrue(batch.waitContinue)
        XCTAssertEqual(batch.nativeTimers?.first?.tolerance, 25)
        XCTAssertEqual(batch.nativeTimers?.first?.generation, UInt64.max)
    }
}

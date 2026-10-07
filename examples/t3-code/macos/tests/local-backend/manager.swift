import Foundation
import XCTest

// Ported from T3 Code (MIT, see LICENSE-T3), reference 1e2ecbd975:
// apps/desktop/src/backend/DesktopBackendManager.test.ts and DesktopBackendConfiguration.test.ts.
// Change from the reference: the tests were bun/vitest over Effect; the logic is native here
// (T3LocalBackendManager.swift), so they are XCTest cases with the original names, and Effect's
// Deferred/Queue/TestClock become the doubles in harness.swift. Assertions on the WSL runtime
// pruning and the telemetry control source are dropped with those features (no WSL; telemetry
// excluded, issue X39).
final class LocalBackendManagerTests: XCTestCase {
    func test_retries_HTTP_readiness_before_reporting_the_backend_ready() {
        let t = TestInstance()
        t.prober.statuses = [503, 200]
        t.start()
        XCTAssertEqual(t.readyCount, 0)
        XCTAssertEqual(t.prober.urls, ["http://127.0.0.1:3773/.well-known/t3/environment"])
        t.clock.adjust(100)
        XCTAssertEqual(t.readyCount, 1)
        XCTAssertEqual(t.prober.urls, ["http://127.0.0.1:3773/.well-known/t3/environment", "http://127.0.0.1:3773/.well-known/t3/environment"])
        t.run { t.spawner.children[0].finish(T3LocalExit(code: 0)) }
        XCTAssertEqual(t.log.failures, ["pid=123 code=0"], "An exit nobody asked for is a failure, even with code 0")
    }

    func test_re_probes_readiness_after_the_first_budget_expires_while_the_backend_is_still_alive() {
        let t = TestInstance()
        t.prober.fallback = 503
        t.start()
        t.clock.adjust(T3LocalBackendManager.readinessTimeout)
        XCTAssertEqual(t.log.snapshots.count, 1, "A failed round persists a snapshot")
        XCTAssertEqual(t.readyCount, 0)
        t.prober.fallback = 200
        t.clock.adjust(T3LocalBackendManager.readinessInterval)
        XCTAssertEqual(t.readyCount, 1, "The probe keeps going while the process lives")
        XCTAssertEqual(t.spawner.starts, 1)
    }

    func test_restarts_an_unexpectedly_exited_backend_with_the_Effect_clock() {
        let t = TestInstance()
        t.prober.fallback = nil
        t.spawner.onSpawn = { $0.finish(T3LocalExit(code: 1)) }
        t.start()
        XCTAssertEqual(t.spawner.starts, 1)
        XCTAssertEqual(t.log.failures, ["pid=123 code=1"])
        t.clock.adjust(499)
        XCTAssertEqual(t.spawner.starts, 1)
        t.clock.adjust(1)
        XCTAssertEqual(t.spawner.starts, 2)
        t.clock.adjust(999)
        XCTAssertEqual(t.spawner.starts, 2)
        t.clock.adjust(1)
        XCTAssertEqual(t.spawner.starts, 3)
    }

    func test_starts_the_configured_backend_and_closes_the_scoped_process_on_stop() {
        let t = TestInstance()
        var teardownStarted = false
        t.spawner.onSpawn = { child in child.onClose = { _ in teardownStarted = true } }
        XCTAssertNil(t.manager.currentConfig)
        t.start()
        XCTAssertEqual(t.spawner.starts, 1)
        XCTAssertEqual(t.readyCount, 1)
        XCTAssertEqual(t.manager.currentConfig, baseConfig)
        XCTAssertEqual(t.snapshot.ready, true)
        XCTAssertEqual(t.snapshot.activePid, 123)

        var stopped = false
        t.run { t.manager.stop { stopped = true } }
        XCTAssertTrue(teardownStarted)
        XCTAssertEqual(t.shutdownCount, 1)
        XCTAssertFalse(stopped)
        t.run { t.spawner.children[0].finish(T3LocalExit(code: 0)) }
        XCTAssertTrue(stopped)
        XCTAssertEqual(t.spawner.starts, 1)
        XCTAssertEqual(t.spawner.children[0].closed, 1)
        XCTAssertEqual(t.log.failures.count, 0)
        XCTAssertEqual(t.log.discarded, 1)
        XCTAssertEqual(t.shutdownCount, 1)
        XCTAssertEqual(t.snapshot.desiredRunning, false)
        XCTAssertEqual(t.snapshot.ready, false)
        XCTAssertNil(t.snapshot.activePid)
    }

    func test_restarts_when_start_is_requested_during_stop_teardown() {
        let t = TestInstance()
        t.prober.fallback = nil
        var teardownStarted = false
        t.spawner.onSpawn = { child in if t.spawner.starts == 1 { child.onClose = { _ in teardownStarted = true } } }
        t.start()
        XCTAssertEqual(t.spawner.starts, 1)
        var stopped = false
        t.run { t.manager.stop { stopped = true } }
        XCTAssertTrue(teardownStarted)
        t.start()
        XCTAssertEqual(t.snapshot.desiredRunning, true)
        t.run { t.spawner.children[0].finish(T3LocalExit(code: 0)) }
        XCTAssertTrue(stopped)
        t.clock.adjust(500)
        XCTAssertEqual(t.spawner.starts, 2)
        XCTAssertEqual(t.snapshot.desiredRunning, true)
        XCTAssertEqual(t.snapshot.activePid, 123)
    }

    func test_keeps_a_timed_out_run_active_until_its_process_exits() {
        let t = TestInstance()
        t.prober.fallback = nil
        var teardownStarted = false
        t.spawner.onSpawn = { child in if t.spawner.starts == 1 { child.onClose = { _ in teardownStarted = true } } }
        t.start()
        var stopped = false
        t.run { t.manager.stop(timeoutMs: 100) { stopped = true } }
        XCTAssertTrue(teardownStarted)
        t.start()
        t.clock.adjust(100)
        XCTAssertTrue(stopped)
        XCTAssertEqual(t.spawner.starts, 1)
        XCTAssertEqual(t.snapshot.desiredRunning, true)
        XCTAssertEqual(t.snapshot.activePid, 123)
        t.run { t.spawner.children[0].finish(T3LocalExit(code: 0)) }
        t.clock.adjust(500)
        XCTAssertEqual(t.spawner.starts, 2)
    }

    func test_does_not_restart_after_stop_cancels_a_scheduled_restart() {
        let t = TestInstance()
        t.prober.fallback = nil
        t.spawner.onSpawn = { $0.finish(T3LocalExit(code: 1)) }
        t.start()
        XCTAssertEqual(t.spawner.starts, 1)
        XCTAssertTrue(t.snapshot.restartScheduled)
        t.run { t.manager.stop() }
        t.clock.adjust(500)
        XCTAssertEqual(t.spawner.starts, 1)
        XCTAssertEqual(t.snapshot.desiredRunning, false)
    }

    func test_cancels_a_scheduled_restart_when_start_is_requested_manually() {
        let t = TestInstance()
        t.prober.fallback = nil
        t.spawner.onSpawn = { child in if t.spawner.starts == 1 { child.finish(T3LocalExit(code: 1)) } }
        t.start()
        XCTAssertEqual(t.spawner.starts, 1)
        XCTAssertTrue(t.snapshot.restartScheduled)
        t.start()
        XCTAssertEqual(t.spawner.starts, 2)
        t.run { t.manager.stop() }
        t.clock.adjust(500)
        XCTAssertEqual(t.spawner.starts, 2)
    }

    func test_drains_trailing_child_output_before_persisting_an_unexpected_exit() {
        let t = TestInstance()
        t.prober.fallback = nil
        var persisted: [String]?
        t.log.onPersistFailure = { _ in persisted = t.log.output }
        t.spawner.onSpawn = { child in
            child.exit(T3LocalExit(code: 1))
            t.clock.after(1000) { child.emit(.stdout, "trailing output\n"); child.endOutput() }
        }
        t.start()
        XCTAssertNil(persisted)
        t.clock.adjust(1000)
        XCTAssertEqual(persisted, ["trailing output\n"])
    }

    func test_stopAllPoolInstances_bounds_the_quit_finalizer_when_backends_hang() {
        let executor = InlineExecutor(), clock = ManualClock(executor: executor)
        let one = TestInstance(executor: executor, clock: clock), two = TestInstance(executor: executor, clock: clock)
        var started: [String] = [], finished: [String] = []
        for (name, instance) in [("instance1", one), ("instance2", two)] {
            instance.prober.fallback = nil
            instance.spawner.onSpawn = { child in child.onClose = { _ in started.append(name) } }
            instance.start()
        }
        var quit = false
        executor.run { T3LocalBackendManager.stopAll([one.manager, two.manager]) { quit = true } }
        XCTAssertEqual(started.sorted(), ["instance1", "instance2"])
        clock.adjust(5_000)
        XCTAssertTrue(quit, "The quit finalizer returns after 5 s without the hung backends")
        XCTAssertEqual(finished.count, 0)
        for (name, instance) in [("instance1", one), ("instance2", two)] {
            executor.run { instance.spawner.children[0].finish(T3LocalExit(code: 0)); finished.append(name) }
        }
        XCTAssertEqual(finished.sorted(), ["instance1", "instance2"])
        XCTAssertNil(one.snapshot.activePid)
        XCTAssertNil(two.snapshot.activePid)
    }

    func test_resolvePrimary_produces_a_stable_scoped_bootstrap_token() {
        let first = T3LocalBackend.shared.currentBootstrapToken, second = T3LocalBackend.shared.currentBootstrapToken
        XCTAssertNotNil(first.range(of: "^[0-9a-f]{48}$", options: .regularExpression))
        XCTAssertEqual(first, second)
        let base = ["PATH": "/usr/bin", "T3CODE_PORT": "4888", "T3CODE_MODE": "web", "T3CODE_DESKTOP_LAN_HOST": "10.0.0.2", "T3CODE_HOME": "/elsewhere", "LANG": "en_US.UTF-8"]
        let env = T3LocalShellEnvironment.serverEnvironment(base: base) { _, _, _ in "" }
        let config = T3LocalBackend.startConfig(versionDir: URL(fileURLWithPath: "/home/.t3/runtime/versions/1.2.3"), port: 4888,
                                                home: URL(fileURLWithPath: "/home/.t3"), token: first, env: env, cwd: "/home")
        XCTAssertEqual(config.executablePath, "/home/.t3/runtime/versions/1.2.3/t3")
        XCTAssertEqual(config.args, ["--bootstrap-fd", "0"])
        XCTAssertEqual(config.cwd, "/home")
        XCTAssertNil(config.env["T3CODE_PORT"]); XCTAssertNil(config.env["T3CODE_MODE"]); XCTAssertNil(config.env["T3CODE_DESKTOP_LAN_HOST"])
        XCTAssertEqual(config.env["T3CODE_HOME"], "/elsewhere", "Other T3CODE_* variables pass through")
        XCTAssertEqual(config.env["T3CODE_TELEMETRY_ENABLED"], "false")
        XCTAssertFalse(config.args.joined().contains(first)); XCTAssertFalse(config.env.values.contains { $0.contains(first) })
        XCTAssertEqual(config.bootstrap.mode, "desktop")
        XCTAssertEqual(config.bootstrap.noBrowser, true)
        XCTAssertEqual(config.bootstrap.port, 4888)
        XCTAssertEqual(config.bootstrap.host, "127.0.0.1")
        XCTAssertEqual(config.bootstrap.t3Home, "/home/.t3")
        XCTAssertEqual(config.bootstrap.tailscaleServeEnabled, false)
        XCTAssertEqual(config.bootstrap.tailscaleServePort, 443)
        XCTAssertEqual(config.bootstrap.desktopBootstrapToken, first)
        XCTAssertNil(config.bootstrap.resourceMonitorPath)
        let line = String(decoding: config.bootstrap.jsonLine(), as: UTF8.self)
        XCTAssertTrue(line.hasSuffix("}\n"))
        let decoded = try? JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any]
        XCTAssertEqual(Set(decoded?.keys.map { $0 } ?? []), ["mode", "noBrowser", "port", "t3Home", "host", "desktopBootstrapToken", "tailscaleServeEnabled", "tailscaleServePort"])
    }

    /// Acceptance "Restart ladder": six exits before ready, then ready, then one kill.
    func testRestartLadder() {
        let t = TestInstance()
        t.prober.fallback = nil
        var timeline: [(attempt: Int, next: Int)] = []
        var scheduled = false
        t.manager.changed = {
            // One entry each time a restart becomes scheduled (the status topic fires more often).
            let now = t.manager.snapshot.restartScheduled
            if now, !scheduled, let next = t.manager.nextRestartMs { timeline.append((t.manager.restartAttempt, Int(next))) }
            scheduled = now
        }
        t.spawner.onSpawn = { child in if t.spawner.starts <= 6 { child.finish(T3LocalExit(code: 1)) } }
        t.start()
        for delay in [500.0, 1000, 2000, 4000, 8000, 10000] { t.clock.adjust(delay) }
        XCTAssertEqual(timeline.map { $0.next }, [500, 1000, 2000, 4000, 8000, 10000])
        XCTAssertEqual(timeline.map { $0.attempt }, [1, 2, 3, 4, 5, 6])
        XCTAssertEqual(t.spawner.starts, 7)
        t.prober.fallback = 200
        t.clock.adjust(T3LocalBackendManager.readinessProbeTimeout + T3LocalBackendManager.readinessInterval)
        XCTAssertTrue(t.snapshot.ready)
        XCTAssertEqual(t.snapshot.restartAttempt, 0, "Ready resets the ladder")
        timeline = []
        t.run { t.spawner.children[6].finish(T3LocalExit(signal: 9)) }
        XCTAssertEqual(timeline.map { $0.next }, [500], "After ready the next delay is 0.5 s, not 16 s")
        XCTAssertEqual(t.manager.lastExit, "signal=9")
        XCTAssertEqual(t.log.failures.last, "pid=123 signal=9")
    }

    func testCalculateRestartDelay() {
        XCTAssertEqual((0...7).map { T3LocalBackendManager.restartDelay($0) }, [500, 1000, 2000, 4000, 8000, 10000, 10000, 10000])
    }
}

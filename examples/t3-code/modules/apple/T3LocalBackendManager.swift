// The embedded server's supervisor (20261005-embedded-server-runtime): a port of T3 Code's
// desktop backend instance (MIT, see LICENSE-T3; reference 1e2ecbd975:
// apps/desktop/src/backend/DesktopBackendManager.ts `makeBackendInstance`, `runBackendProcess`,
// `calculateRestartDelay`; app/DesktopApp.ts `stopAllPoolInstances`).
//
// One instance owns one server process at a time: start, readiness rounds (a probe every 100 ms,
// 1 s each, 60 s rounds repeated while the process lives), a restart after an unexpected exit
// with min(500 ms × 2ⁿ, 10 s), n back to 0 once ready, and stop (SIGTERM, SIGKILL after 2 s).
// Effect's fibers become callbacks on one serial executor; its clock becomes `T3LocalClock`, so
// the ported tests (macos/tests/local-backend) drive time by hand. Every method runs on the
// executor. The WSL preflight states, telemetry fds and runtime pruning are not ported (no WSL,
// telemetry excluded: spec, issue X39).
import Foundation

protocol T3LocalExecutor: AnyObject { func run(_ block: @escaping () -> Void) }
protocol T3LocalCancellable: AnyObject { func cancel() }
protocol T3LocalClock: AnyObject {
    /// Milliseconds.
    func now() -> Double
    /// Runs `block` on the executor after `ms`, unless cancelled.
    @discardableResult func after(_ ms: Double, _ block: @escaping () -> Void) -> T3LocalCancellable
}

final class T3LocalQueueExecutor: T3LocalExecutor {
    let queue: DispatchQueue
    init(queue: DispatchQueue) { self.queue = queue }
    func run(_ block: @escaping () -> Void) { queue.async(execute: block) }
}

final class T3LocalWorkItem: T3LocalCancellable {
    let item: DispatchWorkItem
    init(_ item: DispatchWorkItem) { self.item = item }
    func cancel() { item.cancel() }
}

final class T3LocalQueueClock: T3LocalClock {
    let queue: DispatchQueue
    init(queue: DispatchQueue) { self.queue = queue }
    func now() -> Double { Double(DispatchTime.now().uptimeNanoseconds) / 1_000_000 }
    @discardableResult func after(_ ms: Double, _ block: @escaping () -> Void) -> T3LocalCancellable {
        let item = DispatchWorkItem(block: block)
        queue.asyncAfter(deadline: .now() + .microseconds(Int(ms * 1000)), execute: item)
        return T3LocalWorkItem(item)
    }
}

/// `DesktopBackendBootstrap` (packages/contracts/src/desktopBootstrap.ts), as this app sends it:
/// no OTLP endpoints and no telemetry fds.
struct T3LocalBootstrap: Codable, Equatable {
    var mode = "desktop"
    var noBrowser = true
    var port: Int
    var t3Home: String
    var host = "127.0.0.1"
    var desktopBootstrapToken: String
    var tailscaleServeEnabled = false
    var tailscaleServePort = 443
    var resourceMonitorPath: String?

    func jsonLine() -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.withoutEscapingSlashes]
        var line = (try? encoder.encode(self)) ?? Data()
        line.append(0x0A)
        return line
    }
}

/// `DesktopBackendStartConfig`, for the macOS SEA: `t3 --bootstrap-fd 0`, the envelope on stdin.
struct T3LocalStartConfig: Equatable {
    var executablePath: String
    var args: [String]
    var cwd: String
    var env: [String: String]
    var bootstrap: T3LocalBootstrap
    var httpBaseUrl: URL
}

struct T3LocalExit: Equatable {
    var code: Int32?
    var signal: Int32?
    /// `BackendProcessExit.reason`.
    var reason: String { code.map { "code=\($0)" } ?? "signal=\(signal ?? 0)" }
}

struct T3LocalChildEvents {
    let output: (T3LocalOutputStream, Data) -> Void
    let outputEnded: (T3LocalOutputStream) -> Void
    let exited: (T3LocalExit) -> Void
}

protocol T3LocalChild: AnyObject {
    var pid: Int32 { get }
    /// The run scope's close: SIGTERM, SIGKILL after 2 s (`forceKillAfter`). The exit arrives through the events.
    func close()
}

protocol T3LocalSpawner: AnyObject {
    func spawn(_ config: T3LocalStartConfig, events: T3LocalChildEvents) throws -> T3LocalChild
}

protocol T3LocalProber: AnyObject {
    /// GET `url`; `done(true)` for a 2xx answer. Any thread.
    func probe(_ url: URL, timeoutMs: Double, _ done: @escaping (Bool) -> Void)
}

/// `BackendInstanceSpec`.
struct T3LocalBackendSpec {
    var configResolve: () throws -> T3LocalStartConfig
    var entryExists: (T3LocalStartConfig) -> Bool = { FileManager.default.isExecutableFile(atPath: $0.executablePath) }
    var onReady: (URL) -> Void = { _ in }
    var onShutdown: () -> Void = {}
    var onStarted: (Int32, T3LocalStartConfig) -> Void = { _, _ in }
}

final class T3LocalBackendManager {
    static let initialRestartDelay = 500.0
    static let maxRestartDelay = 10_000.0
    static let readinessTimeout = 60_000.0
    static let readinessInterval = 100.0
    static let readinessProbeTimeout = 1_000.0
    static let outputDrainTimeout = 5_000.0
    static let readinessPath = "/.well-known/t3/environment"

    /// `calculateRestartDelay`.
    static func restartDelay(_ attempt: Int) -> Double { min(initialRestartDelay * pow(2, Double(attempt)), maxRestartDelay) }

    struct Snapshot: Equatable {
        var desiredRunning: Bool
        var ready: Bool
        var activePid: Int32?
        var restartAttempt: Int
        var restartScheduled: Bool
    }

    private final class Run {
        let id: Int
        let config: T3LocalStartConfig
        var pid: Int32?
        var child: T3LocalChild?
        var exitObserved = false
        var stopRequested = false
        var closing = false
        var finished = false
        var readyReported = false
        var exit: T3LocalExit?
        var openStreams = 2
        var round = 0
        var attempts = 0
        var timers: [T3LocalCancellable] = []
        var drainTimer: T3LocalCancellable?
        var waiters: [() -> Void] = []
        init(id: Int, config: T3LocalStartConfig) { self.id = id; self.config = config }
        func cancelTimers() { timers.forEach { $0.cancel() }; timers = [] }
    }

    let executor: T3LocalExecutor
    let clock: T3LocalClock
    let spawner: T3LocalSpawner
    let prober: T3LocalProber
    let log: T3LocalOutputLog
    var spec: T3LocalBackendSpec
    /// Each state change (on the executor).
    var changed: () -> Void = {}
    var logger: (String) -> Void = { _ in }

    private(set) var desiredRunning = false
    private(set) var ready = false
    private(set) var config: T3LocalStartConfig?
    private var active: Run?
    private(set) var restartAttempt = 0
    private var restartTimer: T3LocalCancellable?
    private(set) var restartDueAt: Double?
    private(set) var lastExit: String?
    private var nextRunId = 1

    init(executor: T3LocalExecutor, clock: T3LocalClock, spawner: T3LocalSpawner, prober: T3LocalProber, log: T3LocalOutputLog, spec: T3LocalBackendSpec) {
        self.executor = executor; self.clock = clock; self.spawner = spawner; self.prober = prober; self.log = log; self.spec = spec
    }

    var snapshot: Snapshot {
        Snapshot(desiredRunning: desiredRunning, ready: ready, activePid: active?.pid, restartAttempt: restartAttempt, restartScheduled: restartTimer != nil)
    }
    var currentConfig: T3LocalStartConfig? { config }
    /// The delay left before a scheduled restart, in ms.
    var nextRestartMs: Double? { restartDueAt.map { max(0, $0 - clock.now()) } }

    private func cancelRestart() {
        restartTimer?.cancel(); restartTimer = nil; restartDueAt = nil
    }

    func start() {
        if let active {
            _ = active
            if !desiredRunning { desiredRunning = true; changed() }
            return
        }
        let wasDesired = desiredRunning
        if ready {
            spec.onShutdown()
            ready = false
        }
        let resolved: T3LocalStartConfig
        do { resolved = try spec.configResolve() } catch {
            logger("failed to generate desktop backend configuration: \(error)")
            if wasDesired { scheduleRestart("failed to generate desktop backend configuration") }
            changed()
            return
        }
        let entryExists = spec.entryExists(resolved)
        cancelRestart()
        desiredRunning = true; ready = false; config = resolved
        if !entryExists {
            scheduleRestart("missing server entry at \(resolved.executablePath)")
            changed()
            return
        }
        let run = Run(id: nextRunId, config: resolved)
        nextRunId += 1
        active = run
        let events = T3LocalChildEvents(
            output: { [weak self] stream, data in self?.executor.run { self?.log.writeOutputChunk(stream, data) } },
            outputEnded: { [weak self] _ in self?.executor.run { self?.outputEnded(run) } },
            exited: { [weak self] exit in self?.executor.run { self?.exited(run, exit) } })
        do {
            let child = try spawner.spawn(resolved, events: events)
            run.child = child; run.pid = child.pid
            log.beginSession(details: "pid=\(child.pid) port=\(resolved.bootstrap.port) cwd=\(resolved.cwd)")
            spec.onStarted(child.pid, resolved)
            changed()
            startRound(run)
        } catch {
            finish(run, reason: "Failed to spawn desktop backend entry \(resolved.executablePath) with \(resolved.executablePath).")
        }
    }

    // MARK: readiness (`probeReadiness` repeated while the child lives)

    private func live(_ run: Run) -> Bool { active === run && !run.exitObserved && !run.readyReported }

    private func startRound(_ run: Run) {
        guard live(run) else { return }
        run.cancelTimers()
        run.round += 1; run.attempts = 0
        let round = run.round
        run.timers.append(clock.after(Self.readinessTimeout) { [weak self] in
            guard let self, run.round == round, self.live(run) else { return }
            self.readinessRoundFailed(run)
        })
        probe(run, round)
    }

    private func probe(_ run: Run, _ round: Int) {
        guard run.round == round, live(run) else { return }
        run.attempts += 1
        let url = URL(string: Self.readinessPath, relativeTo: run.config.httpBaseUrl)!.absoluteURL
        final class Answer { var given = false }
        let answer = Answer()
        let timeout = clock.after(Self.readinessProbeTimeout) { [weak self] in
            guard !answer.given else { return }
            answer.given = true
            self?.probeFailed(run, round)
        }
        run.timers.append(timeout)
        prober.probe(url, timeoutMs: Self.readinessProbeTimeout) { [weak self] ok in
            self?.executor.run {
                guard let self, !answer.given else { return }
                answer.given = true
                timeout.cancel()
                guard run.round == round, self.live(run) else { return }
                if ok { self.readinessSucceeded(run) } else { self.probeFailed(run, round) }
            }
        }
    }

    private func probeFailed(_ run: Run, _ round: Int) {
        guard run.round == round, live(run) else { return }
        // `Schedule.spaced(interval)` up to ceil(timeout / interval) retries, inside the round's timeout.
        if run.attempts > Int((Self.readinessTimeout / Self.readinessInterval).rounded(.up)) { return readinessRoundFailed(run) }
        run.timers.append(clock.after(Self.readinessInterval) { [weak self] in self?.probe(run, round) })
    }

    private func readinessRoundFailed(_ run: Run) {
        let url = URL(string: Self.readinessPath, relativeTo: run.config.httpBaseUrl)!.absoluteURL
        let message = "Timed out after \(Int(Self.readinessTimeout))ms waiting for desktop backend readiness at \(url.absoluteString)."
        logger("backend readiness check failed during bootstrap: \(message)")
        log.persistFailureSnapshot(details: message)
        startRound(run)
    }

    private func readinessSucceeded(_ run: Run) {
        run.readyReported = true
        run.cancelTimers()
        guard active === run else { return }
        restartAttempt = 0
        ready = true
        changed()
        spec.onReady(run.config.httpBaseUrl)
    }

    // MARK: exit, drain, finalize

    private func exited(_ run: Run, _ exit: T3LocalExit) {
        guard !run.exitObserved else { return }
        run.exitObserved = true
        run.exit = exit
        run.cancelTimers()
        if run.openStreams <= 0 { return finish(run, reason: exit.reason) }
        // Trailing output is drained for at most 5 s before the run is finalized.
        run.drainTimer = clock.after(Self.outputDrainTimeout) { [weak self] in self?.finish(run, reason: exit.reason) }
    }

    private func outputEnded(_ run: Run) {
        run.openStreams -= 1
        if run.exitObserved, run.openStreams <= 0, let exit = run.exit { finish(run, reason: exit.reason) }
    }

    private func finish(_ run: Run, reason: String) {
        guard !run.finished else { return }
        run.finished = true
        run.drainTimer?.cancel(); run.drainTimer = nil
        run.cancelTimers()
        finalizeRun(run, reason: reason)
        let waiters = run.waiters
        run.waiters = []
        waiters.forEach { $0() }
    }

    private func finalizeRun(_ run: Run, reason: String) {
        guard active === run else { return }
        let wasReady = ready
        active = nil
        ready = false
        if let pid = run.pid {
            if run.exitObserved && !run.stopRequested {
                log.persistFailure(details: "pid=\(pid) \(reason)")
            } else {
                log.discardSession()
            }
        }
        if run.exitObserved { lastExit = reason }
        if wasReady { spec.onShutdown() }
        if desiredRunning { scheduleRestart(reason) }
        changed()
    }

    private func scheduleRestart(_ reason: String) {
        guard desiredRunning, restartTimer == nil else { return }
        let delay = Self.restartDelay(restartAttempt)
        restartAttempt += 1
        logger("backend exited unexpectedly; restart scheduled reason=\(reason) delayMs=\(Int(delay))")
        restartDueAt = clock.now() + delay
        restartTimer = clock.after(delay) { [weak self] in
            guard let self else { return }
            self.restartTimer = nil; self.restartDueAt = nil
            if self.desiredRunning { self.start() } else { self.changed() }
        }
        changed()
    }

    // MARK: stop

    /// `stop`: no restart, SIGTERM (SIGKILL after 2 s), and, when the close finished, a start that
    /// was asked for during the teardown. With `timeoutMs`, `completion` runs after at most that long
    /// and the run stays active until its process exits.
    func stop(timeoutMs: Double? = nil, completion: @escaping () -> Void = {}) {
        let run = active
        if let run, !run.exitObserved { run.stopRequested = true }
        let notifyShutdown = ready
        desiredRunning = false
        ready = false
        cancelRestart()
        changed()
        if notifyShutdown { spec.onShutdown() }
        guard let run else { return completion() }
        closeRun(run, timeoutMs: timeoutMs) { [weak self] closed in
            guard let self, closed else { return completion() }
            let shouldStart: Bool
            if self.active === run {
                self.active = nil
                self.log.discardSession()
                shouldStart = self.desiredRunning
            } else {
                shouldStart = self.desiredRunning && self.active == nil && self.restartTimer == nil
            }
            if shouldStart { self.start() }
            completion()
        }
    }

    private func closeRun(_ run: Run, timeoutMs: Double?, _ done: @escaping (Bool) -> Void) {
        final class Once { var settled = false }
        let once = Once()
        let settle = { (closed: Bool) in
            if once.settled { return }
            once.settled = true
            done(closed)
        }
        if run.finished { return settle(true) }
        run.waiters.append { settle(true) }
        if !run.closing {
            run.closing = true
            run.child?.close()
        }
        if let timeoutMs { clock.after(timeoutMs) { settle(false) } }
    }

    /// `stopAllPoolInstances`: every instance stopped with a 5 s bound each, so a hung backend
    /// cannot hold the quit.
    static func stopAll(_ instances: [T3LocalBackendManager], timeoutMs: Double = 5_000, completion: @escaping () -> Void) {
        var remaining = instances.count
        if remaining == 0 { return completion() }
        for instance in instances {
            instance.executor.run {
                instance.stop(timeoutMs: timeoutMs) {
                    remaining -= 1
                    if remaining == 0 { completion() }
                }
            }
        }
    }
}

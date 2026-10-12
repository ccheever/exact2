import Foundation
import XCTest

// Test doubles for T3LocalBackendManager.swift, in the shape of the reference's
// DesktopBackendManager.test.ts helpers: a serial executor that runs inline, a clock the test
// moves by hand (Effect's TestClock), a spawner whose processes the test ends, an HTTP prober
// with scripted answers, and a recording output log.

/// A serial executor that runs work at once, queueing what arrives while it runs.
final class InlineExecutor: T3LocalExecutor {
    private var queue: [() -> Void] = []
    private var running = false
    func run(_ block: @escaping () -> Void) {
        queue.append(block)
        if running { return }
        running = true
        while !queue.isEmpty { queue.removeFirst()() }
        running = false
    }
}

/// `TestClock`: `adjust` runs every timer due by the new time, in order.
final class ManualClock: T3LocalClock {
    final class Timer: T3LocalCancellable {
        let due: Double, sequence: Int, block: () -> Void
        var cancelled = false
        init(due: Double, sequence: Int, block: @escaping () -> Void) { self.due = due; self.sequence = sequence; self.block = block }
        func cancel() { cancelled = true }
    }
    let executor: T3LocalExecutor
    private(set) var time = 0.0
    private var timers: [Timer] = []
    private var sequence = 0
    init(executor: T3LocalExecutor) { self.executor = executor }
    func now() -> Double { time }
    @discardableResult func after(_ ms: Double, _ block: @escaping () -> Void) -> T3LocalCancellable {
        sequence += 1
        let timer = Timer(due: time + ms, sequence: sequence, block: block)
        timers.append(timer)
        return timer
    }
    func adjust(_ ms: Double) {
        let target = time + ms
        while true {
            timers.removeAll { $0.cancelled }
            guard let next = timers.filter({ $0.due <= target }).min(by: { ($0.due, $0.sequence) < ($1.due, $1.sequence) }) else { break }
            timers.removeAll { $0 === next }
            time = next.due
            executor.run(next.block)
        }
        time = target
    }
    var pending: Int { timers.filter { !$0.cancelled }.count }
}

/// A child the test ends: `exit` reports the exit, `emit` writes output, `endOutput` closes the
/// streams. `close` (the run scope's close) calls `onClose`, which a test uses as its teardown.
final class FakeChild: T3LocalChild {
    let pid: Int32
    let events: T3LocalChildEvents
    var onClose: (FakeChild) -> Void = { $0.finish(T3LocalExit(code: 0)) }
    private(set) var closed = 0
    private(set) var exitedWith: T3LocalExit?
    private var outputOpen = true
    init(pid: Int32, events: T3LocalChildEvents) { self.pid = pid; self.events = events }
    func close() { closed += 1; onClose(self) }
    func emit(_ stream: T3LocalOutputStream, _ text: String) { events.output(stream, Data(text.utf8)) }
    func endOutput() {
        guard outputOpen else { return }
        outputOpen = false
        events.outputEnded(.stdout); events.outputEnded(.stderr)
    }
    func exit(_ exit: T3LocalExit) {
        guard exitedWith == nil else { return }
        exitedWith = exit
        events.exited(exit)
    }
    /// An exit whose output has already ended.
    func finish(_ exit: T3LocalExit) { endOutput(); self.exit(exit) }
}

final class FakeSpawner: T3LocalSpawner {
    var children: [FakeChild] = []
    var configs: [T3LocalStartConfig] = []
    /// Called with each new child, before `spawn` returns.
    var onSpawn: (FakeChild) -> Void = { _ in }
    var failure: Error?
    func spawn(_ config: T3LocalStartConfig, events: T3LocalChildEvents) throws -> T3LocalChild {
        if let failure { throw failure }
        let child = FakeChild(pid: 123, events: events)
        children.append(child); configs.append(config)
        onSpawn(child)
        return child
    }
    var starts: Int { children.count }
}

/// Answers each probe from `statuses` (nil: never answers).
final class FakeProber: T3LocalProber {
    var statuses: [Int?] = []
    var fallback: Int? = 200
    private(set) var urls: [String] = []
    func probe(_ url: URL, timeoutMs: Double, _ done: @escaping (Bool) -> Void) {
        urls.append(url.absoluteString)
        let status = statuses.isEmpty ? fallback : statuses.removeFirst()
        if let status { done((200..<300).contains(status)) }
    }
}

final class RecordingLog: T3LocalOutputLog {
    var output: [String] = []
    var failures: [String] = []
    var snapshots: [String] = []
    var begun: [String] = []
    var discarded = 0
    var onPersistFailure: (String) -> Void = { _ in }
    func beginSession(details: String) { begun.append(details) }
    func writeOutputChunk(_ stream: T3LocalOutputStream, _ chunk: Data) { output.append(String(decoding: chunk, as: UTF8.self)) }
    func persistFailureSnapshot(details: String) { snapshots.append(details) }
    func persistFailure(details: String) { failures.append(details); onPersistFailure(details) }
    func discardSession() { discarded += 1 }
}

let baseConfig = T3LocalStartConfig(
    executablePath: "/runtime/t3", args: ["--bootstrap-fd", "0"], cwd: "/home", env: ["T3CODE_TELEMETRY_ENABLED": "false"],
    bootstrap: T3LocalBootstrap(port: 3773, t3Home: "/tmp/t3", desktopBootstrapToken: "token"),
    httpBaseUrl: URL(string: "http://127.0.0.1:3773")!)

/// `makeTestInstance`: a manager over the doubles, every entry existing.
final class TestInstance {
    let executor: InlineExecutor
    let clock: ManualClock
    let spawner = FakeSpawner()
    let prober = FakeProber()
    let log = RecordingLog()
    let manager: T3LocalBackendManager
    var readyCount = 0
    var shutdownCount = 0
    init(config: @escaping () throws -> T3LocalStartConfig = { baseConfig }, executor: InlineExecutor = InlineExecutor(), clock: ManualClock? = nil) {
        self.executor = executor
        self.clock = clock ?? ManualClock(executor: executor)
        manager = T3LocalBackendManager(executor: executor, clock: self.clock, spawner: spawner, prober: prober, log: log,
                                        spec: T3LocalBackendSpec(configResolve: config, entryExists: { _ in true }))
        manager.spec.onReady = { [unowned self] _ in readyCount += 1 }
        manager.spec.onShutdown = { [unowned self] in shutdownCount += 1 }
    }
    func run(_ block: @escaping () -> Void) { executor.run(block) }
    func start() { run { self.manager.start() } }
    var snapshot: T3LocalBackendManager.Snapshot { manager.snapshot }
}

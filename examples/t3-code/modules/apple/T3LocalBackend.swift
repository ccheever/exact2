// The embedded T3 server (20261005-embedded-server-runtime): the app's one local backend, after
// T3 Code's desktop app (MIT, see LICENSE-T3; reference 1e2ecbd975: apps/desktop/src/app/DesktopApp.ts
// `resolveDesktopBackendPort` and the quit finalizer, backend/DesktopBackendConfiguration.ts
// `resolvePrimaryStartConfig` and the bootstrap token, backend/DesktopLocalEnvironmentAuth.ts).
//
// At the first session: unpack the runtime (T3LocalRuntime.swift), pick the port, start
// `<versionDir>/t3 --bootstrap-fd 0` with the desktop envelope on stdin, supervise it
// (T3LocalBackendManager.swift), and once it answers exchange the bootstrap token for a bearer
// kept in memory for the app run. When the last session goes (exact2 #105/#200: every session's
// module is destroyed at quit), stop it: SIGTERM, SIGKILL after 2 s, at most 5 s. A pid file in
// the app's data folder lets the next launch reap a server an app crash left behind.
//
// Real data and port 3773 are opt-in: only the packaged build (Contents/Resources/distribution.json
// `{"flavor":"packaged"}`, written by the packaging step) uses `~/.t3` and scans from 3773. Every
// other build starts nothing unless T3_LOCAL_HOME (an isolated folder) and T3_LOCAL_PORT
// (16000-16999) are set. No UI here: TypeScript reads `localBackendStatus` and watches `t3.local`.
import Foundation
import Darwin
import Security

/// Item 8 of the ticket: which home and port this build may use.
enum T3LocalPolicy: Equatable {
    case allowed(home: URL, configuredPort: Int?, scanStart: Int, runtimeDir: URL?)
    case refused(String)

    static let developmentMissing = "Development build: set T3_LOCAL_HOME and T3_LOCAL_PORT to start the local server."
    static let developmentRefused = "Refusing the real T3 home / port 3773 in a development build."
    static let defaultPort = 3773
    static let laneRange = 16000...16999

    /// `distribution.json` in the bundle's Resources marks the packaged build.
    static func packaged(resources: URL?) -> Bool {
        guard let resources, let data = try? Data(contentsOf: resources.appendingPathComponent("distribution.json")),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        return object["flavor"] as? String == "packaged"
    }

    /// Resolves `~`, `..` and symlinks (of the deepest part that exists).
    static func resolved(_ path: String, home: String) -> String {
        let expanded = path == "~" ? home : path.hasPrefix("~/") ? home + path.dropFirst(1) : path
        var url = URL(fileURLWithPath: expanded).standardizedFileURL
        var rest: [String] = []
        while !FileManager.default.fileExists(atPath: url.path), url.path != "/" {
            rest.insert(url.lastPathComponent, at: 0)
            url.deleteLastPathComponent()
        }
        var resolved = url.resolvingSymlinksInPath()
        for part in rest { resolved.appendPathComponent(part) }
        return resolved.standardizedFileURL.path
    }

    static func overlaps(_ a: String, _ b: String) -> Bool {
        a == b || a.hasPrefix(b.hasSuffix("/") ? b : b + "/") || b.hasPrefix(a.hasSuffix("/") ? a : a + "/")
    }

    static func resolve(env: [String: String], packaged: Bool, home: String, accountHome: String) -> T3LocalPolicy {
        if packaged {
            // `resolveDesktopBaseDir`: T3CODE_HOME, else ~/.t3; `configuredBackendPort`: T3CODE_PORT.
            let base = env["T3CODE_HOME"].flatMap { T3LocalShellEnvironment.trimmed($0) } ?? (home as NSString).appendingPathComponent(".t3")
            let port = env["T3CODE_PORT"].flatMap { Int($0) }.flatMap { (1...65535).contains($0) ? $0 : nil }
            return .allowed(home: URL(fileURLWithPath: base, isDirectory: true), configuredPort: port, scanStart: defaultPort, runtimeDir: nil)
        }
        guard let rawHome = T3LocalShellEnvironment.trimmed(env["T3_LOCAL_HOME"]), let rawPort = T3LocalShellEnvironment.trimmed(env["T3_LOCAL_PORT"]) else {
            return .refused(developmentMissing)
        }
        guard let port = Int(rawPort), port != defaultPort, laneRange.contains(port) else { return .refused(developmentRefused) }
        let target = resolved(rawHome, home: home)
        let realHomes = Set([home, accountHome].map { resolved(($0 as NSString).appendingPathComponent(".t3"), home: home) })
        if env["T3_LOCAL_ALLOW_REAL"] != "1", realHomes.contains(where: { overlaps(target, $0) }) { return .refused(developmentRefused) }
        let runtimeDir = T3LocalShellEnvironment.trimmed(env["T3_LOCAL_RUNTIME_DIR"]).map { URL(fileURLWithPath: resolved($0, home: home), isDirectory: true) }
        return .allowed(home: URL(fileURLWithPath: target, isDirectory: true), configuredPort: port, scanStart: defaultPort, runtimeDir: runtimeDir)
    }
}

/// `resolveDesktopBackendPort` and `NetService.canListenOnHost`.
enum T3LocalPorts {
    static let probeHosts = ["127.0.0.1", "0.0.0.0", "::"]

    static func canListen(_ port: Int, host: String) -> Bool {
        let v6 = host.contains(":")
        let fd = socket(v6 ? AF_INET6 : AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        defer { close(fd) }
        var yes: Int32 = 1, no: Int32 = 0
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, socklen_t(MemoryLayout<Int32>.size))
        var result: Int32
        if v6 {
            setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, &no, socklen_t(MemoryLayout<Int32>.size))
            var address = sockaddr_in6()
            address.sin6_len = UInt8(MemoryLayout<sockaddr_in6>.size); address.sin6_family = sa_family_t(AF_INET6)
            address.sin6_port = in_port_t(UInt16(port).bigEndian)
            _ = host.withCString { inet_pton(AF_INET6, $0, &address.sin6_addr) }
            result = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in6>.size)) } }
        } else {
            var address = sockaddr_in()
            address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET)
            address.sin_port = in_port_t(UInt16(port).bigEndian)
            _ = host.withCString { inet_pton(AF_INET, $0, &address.sin_addr) }
            result = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        }
        if result != 0 { return errno == EADDRNOTAVAIL }
        return listen(fd, 1) == 0
    }

    /// A configured port wins as it is; otherwise the first port free on all three hosts.
    static func resolve(configured: Int?, scanStart: Int, canListen: (Int, String) -> Bool = canListen) -> Result<Int, T3Failure> {
        if let configured { return .success(configured) }
        var port = scanStart
        while port <= 65535 {
            if probeHosts.allSatisfy({ canListen(port, $0) }) { return .success(port) }
            port += 1
        }
        return .failure(T3Failure(kind: "Port", message: "No desktop backend port is available on hosts \(probeHosts.joined(separator: ", ")) between \(scanStart) and 65535."))
    }
}

/// The record of the running server, so a launch after a crash can stop the one left behind.
enum T3LocalPidFile {
    struct Record: Codable, Equatable { let pid: Int32; let startTime: Int; let executable: String }

    static func startTime(_ pid: Int32) -> Int? {
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.size)
        guard proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size) == size else { return nil }
        return Int(info.pbi_start_tvsec)
    }

    static func executable(_ pid: Int32) -> String? {
        var buffer = [CChar](repeating: 0, count: 4 * Int(MAXPATHLEN))
        guard proc_pidpath(pid, &buffer, UInt32(buffer.count)) > 0 else { return nil }
        return String(cString: buffer)
    }

    static func write(_ url: URL, pid: Int32, executable: String) {
        guard let start = startTime(pid), let data = try? JSONEncoder().encode(Record(pid: pid, startTime: start, executable: executable)) else { return }
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? data.write(to: url, options: .atomic)
    }

    /// Stops the recorded server when it still runs as the same process (pid, start time and
    /// `proc_pidpath` all match): SIGTERM, then SIGKILL after 2 s. Returns the pid it stopped.
    @discardableResult static func reap(_ url: URL, wait: (TimeInterval) -> Void = { Thread.sleep(forTimeInterval: $0) }) -> Int32? {
        defer { try? FileManager.default.removeItem(at: url) }
        guard let data = try? Data(contentsOf: url), let record = try? JSONDecoder().decode(Record.self, from: data), record.pid > 1,
              startTime(record.pid) == record.startTime,
              let path = executable(record.pid), URL(fileURLWithPath: path).resolvingSymlinksInPath().path == URL(fileURLWithPath: record.executable).resolvingSymlinksInPath().path else { return nil }
        kill(record.pid, SIGTERM)
        for _ in 0..<20 where kill(record.pid, 0) == 0 { wait(0.1) }
        if kill(record.pid, 0) == 0 { kill(record.pid, SIGKILL) }
        return record.pid
    }
}

/// Foundation `Process` as the manager's spawner: stdin carries the envelope and closes at once
/// (the server waits 1 s for it, apps/server/src/bootstrap.ts); stdout and stderr are captured.
final class T3LocalProcessSpawner: T3LocalSpawner {
    final class Child: T3LocalChild {
        let process: Process
        var pid: Int32 { process.processIdentifier }
        init(_ process: Process) { self.process = process }
        func close() {
            guard process.isRunning else { return }
            kill(pid, SIGTERM)
            DispatchQueue.global().asyncAfter(deadline: .now() + 2) { [process] in
                if process.isRunning { kill(process.processIdentifier, SIGKILL) }
            }
        }
    }

    func spawn(_ config: T3LocalStartConfig, events: T3LocalChildEvents) throws -> T3LocalChild {
        let process = Process(), input = Pipe(), stdout = Pipe(), stderr = Pipe()
        process.executableURL = URL(fileURLWithPath: config.executablePath)
        process.arguments = config.args
        process.currentDirectoryURL = URL(fileURLWithPath: config.cwd, isDirectory: true)
        process.environment = config.env
        process.standardInput = input
        process.standardOutput = stdout
        process.standardError = stderr
        for (pipe, stream) in [(stdout, T3LocalOutputStream.stdout), (stderr, .stderr)] {
            pipe.fileHandleForReading.readabilityHandler = { handle in
                let data = handle.availableData
                if data.isEmpty {
                    handle.readabilityHandler = nil
                    events.outputEnded(stream)
                } else {
                    events.output(stream, data)
                }
            }
        }
        process.terminationHandler = { finished in
            events.exited(finished.terminationReason == .exit ? T3LocalExit(code: finished.terminationStatus) : T3LocalExit(signal: finished.terminationStatus))
        }
        try process.run()
        try? input.fileHandleForWriting.write(contentsOf: config.bootstrap.jsonLine())
        try? input.fileHandleForWriting.close()
        return Child(process)
    }
}

final class T3LocalHTTPProber: T3LocalProber {
    let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.connectionProxyDictionary = [:]
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        return URLSession(configuration: configuration)
    }()
    func probe(_ url: URL, timeoutMs: Double, _ done: @escaping (Bool) -> Void) {
        var request = URLRequest(url: url, timeoutInterval: timeoutMs / 1000)
        request.httpMethod = "GET"
        session.dataTask(with: request) { _, response, _ in
            done(((response as? HTTPURLResponse)?.statusCode).map { (200..<300).contains($0) } ?? false)
        }.resume()
    }
}

final class T3LocalBackend: @unchecked Sendable {
    static let shared = T3LocalBackend()
    static let topic = "t3.local"

    private let queue = DispatchQueue(label: "com.exact.t3code.local", qos: .userInitiated)
    private let installQueue = DispatchQueue(label: "com.exact.t3code.local.install", qos: .userInitiated)
    private let lock = NSLock()
    private var listeners: [ObjectIdentifier: (String) -> Void] = [:]
    private var begun = false
    private var manager: T3LocalBackendManager?
    private var status: [String: Any] = ["state": "stopped", "bearerReady": false, "restartAttempt": 0, "enabled": true]
    private var pidFile: URL?
    private var dataRoot: URL?
    private var policy: T3LocalPolicy?
    /// The packaged build (distribution.json); only a development build honors the unpack hold.
    private var packaged = false
    private let auth: T3LocalAuth
    private let session: URLSession
    /// The Local environment switch's waits for a start (`setEnabled(true)`): each gets nil once ready, else the reason.
    private var readyWaiters: [(String?) -> Void] = []
    private var waitEpoch = 0

    // Seams: the process environment, the bundle's Resources, the child's environment, the spawner and
    // readiness prober, and the fatal startup alert (tests replace them; T3LocalFatal.swift).
    var environment: () -> [String: String] = { ProcessInfo.processInfo.environment }
    var resources: () -> URL? = { Bundle.main.resourceURL }
    var serverEnvironment: ([String: String]) -> [String: String] = { T3LocalShellEnvironment.serverEnvironment(base: $0, run: T3LocalShellEnvironment.runCommand) }
    var makeSpawner: () -> T3LocalSpawner = { T3LocalProcessSpawner() }
    var makeProber: () -> T3LocalProber = { T3LocalHTTPProber() }
    var fatal: (String, String) -> Void = { T3LocalFatal.handle(stage: $0, message: $1) }
    var canListen: (Int, String) -> Bool = T3LocalPorts.canListen
    /// How long a switch-on waits for the server to answer.
    var startTimeout: TimeInterval = 60

    init(session: URLSession = URLSession(configuration: .ephemeral)) {
        self.session = session
        auth = T3LocalAuth(session: session)
    }

    /// The bootstrap token: 24 random bytes as hex, once per app run (`getOrCreateBootstrapToken`).
    private let bootstrapToken: String = {
        var bytes = [UInt8](repeating: 0, count: 24)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return bytes.map { String(format: "%02x", $0) }.joined()
    }()
    /// `makeDesktopRunId`: 12 hex characters, the failure log's runId.
    let runId = String(UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased().prefix(12))

    private func log(_ message: String) {
        FileHandle.standardError.write(Data("t3.local: \(String(format: "%.3f", Date().timeIntervalSince1970)) \(message)\n".utf8))
    }

    /// The status TypeScript reads (`localBackendStatus`). Any thread.
    func statusValue() -> [String: Any] { lock.lock(); defer { lock.unlock() }; return status }

    /// The bearer for the embedded server (memory only), once the exchange has run.
    func bearerToken() -> String? { auth.cached }

    private var lastLogged = ""

    private func publish(_ change: (inout [String: Any]) -> Void) {
        lock.lock()
        change(&status)
        // The status timeline on stderr: one line per change of state, attempt, pid, exit, bearer or
        // install phase (the status holds no secret).
        let install = status["install"] as? [String: Any]
        let key = ["state", "restartAttempt", "pid", "lastExit", "bearerReady", "enabled", "environmentId"].map { "\(status[$0] ?? "")" }.joined(separator: "|") + "|\(install?["phase"] ?? "")"
        let line = key == lastLogged ? nil : (try? JSONSerialization.data(withJSONObject: status, options: [.sortedKeys])).map { String(decoding: $0, as: UTF8.self) }
        lastLogged = key
        let notify = Array(listeners.values)
        lock.unlock()
        if let line { log("status \(line)") }
        notify.forEach { $0(Self.topic) }
    }

    /// A session's module arrived. The first one starts the backend.
    func attach(_ owner: AnyObject, dataRoot: URL, changed: @escaping (String) -> Void) {
        lock.lock()
        listeners[ObjectIdentifier(owner)] = changed
        let first = !begun
        begun = true
        lock.unlock()
        if first { installQueue.async { self.begin(dataRoot: dataRoot) } }
    }

    /// A session's module was destroyed. The last one stops the backend, waiting at most 5 s
    /// (`stopAllPoolInstances`); at quit this runs inside applicationWillTerminate (exact2 #200).
    func detach(_ owner: AnyObject) {
        lock.lock()
        listeners.removeValue(forKey: ObjectIdentifier(owner))
        let last = listeners.isEmpty
        lock.unlock()
        if last { stopAndWait() }
    }

    /// A process exit that skips the sessions' teardown (the agent driver's end of drive calls
    /// `exit(0)`, exact2 Agent.swift `exitAfterStorage`) still stops the server: the same bounded stop
    /// runs from `atexit`. It does nothing when the backend already stopped.
    private static let exitHook: Void = { atexit { T3LocalBackend.shared.stopAndWait() } }()

    private func stopAndWait() {
        lock.lock()
        let current = manager
        lock.unlock()
        guard let current else { return }
        let done = DispatchSemaphore(value: 0), pidFile = self.pidFile
        T3LocalBackendManager.stopAll([current], timeoutMs: 5_000) {
            // A server that outlived the bound keeps its record, so the next launch reaps it.
            if let pidFile, current.snapshot.activePid == nil { try? FileManager.default.removeItem(at: pidFile) }
            done.signal()
        }
        _ = done.wait(timeout: .now() + 5.5)
    }

    /// `localEnvironmentEnabled` in the app's own preference file (`t3-code.json`, decision U7: not the
    /// original app's desktop-settings.json); a missing file or key means on, as `DesktopAppSettings`.
    static func localEnvironmentEnabled(dataRoot: URL) -> Bool {
        guard let data = try? Data(contentsOf: dataRoot.appendingPathComponent("t3-code.json")),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return true }
        return object["localEnvironmentEnabled"] as? Bool != false
    }

    private func begin(dataRoot: URL) {
        let env = environment(), packaged = T3LocalPolicy.packaged(resources: resources())
        let policy = T3LocalPolicy.resolve(env: env, packaged: packaged,
                                           home: NSHomeDirectory(), accountHome: String(cString: getpwuid(getuid()).pointee.pw_dir))
        lock.lock(); self.dataRoot = dataRoot; self.policy = policy; self.packaged = packaged; lock.unlock()
        guard case .allowed = policy else {
            if case let .refused(reason) = policy { publish { $0["state"] = "refused"; $0["refused"] = reason } }
            return
        }
        // A server an app crash left behind goes before a new one starts.
        let pidFile = dataRoot.appendingPathComponent("embedded-server.pid")
        self.pidFile = pidFile
        if let reaped = T3LocalPidFile.reap(pidFile) { log("stopped the server pid \(reaped) a previous run left behind") }
        // The Local environment switch off: no server and nothing unpacked until it is turned on again.
        guard Self.localEnvironmentEnabled(dataRoot: dataRoot) else { return publish { $0["enabled"] = false; $0["state"] = "stopped" } }
        // `handleFatalStartupError`: a start that cannot pick a port is fatal (stage "bootstrap").
        if let failure = prepare(), failure.fatal { return fatal("bootstrap", failure.message) }
        lock.lock(); let manager = self.manager; let stillWanted = !listeners.isEmpty; lock.unlock()
        if stillWanted, let manager { queue.async { manager.start() } }
    }

    /// Unpacks the runtime, picks the port and builds the manager (once per app run). Returns why it
    /// could not; `fatal` marks a port failure, which `handleFatalStartupError` ends the app for.
    private func prepare() -> (message: String, fatal: Bool)? {
        lock.lock(); let existing = manager; let policy = self.policy; let pidFile = self.pidFile; lock.unlock()
        if existing != nil { return nil }
        guard case let .allowed(home, configuredPort, scanStart, runtimeDir) = policy, let pidFile else {
            if case let .refused(reason) = policy { return (reason, false) }
            return ("The local server is not available.", false)
        }
        let env = environment()
        // `resolveDesktopBackendPort` comes first, as in the reference's bootstrap.
        let port: Int
        switch T3LocalPorts.resolve(configured: configuredPort, scanStart: scanStart, canListen: canListen) {
        case let .success(value): port = value
        case let .failure(error):
            publish { $0["state"] = "failed"; $0["failure"] = error.message }
            return (error.message, true)
        }
        let versionDir: URL, version: String
        if let runtimeDir {
            versionDir = runtimeDir; version = (T3LocalRuntime.versionOutput(runtimeDir.appendingPathComponent("t3")) ?? "").replacingOccurrences(of: "t3 v", with: "")
        } else {
            let runtime = T3LocalRuntime(home: home, bundleDir: resources()?.appendingPathComponent("t3-runtime", isDirectory: true))
            lock.lock(); let packaged = self.packaged; lock.unlock()
            if !packaged, let hold = env["T3_LOCAL_UNPACK_DELAY_MS"].flatMap(Double.init), hold > 0 { runtime.stageHold = min(hold, 60_000) / 1000 }
            var lastPublished = -1.0
            runtime.progress = { [weak self] phase, fraction in
                guard fraction - lastPublished >= 0.02 || fraction == 0 || fraction == 1 else { return }
                lastPublished = fraction
                self?.publish { $0["state"] = "installing"; $0["install"] = ["phase": phase, "fraction": (fraction * 100).rounded() / 100] }
            }
            let started = Date()
            switch runtime.ensure() {
            case let .ready(dir, pinned, installed):
                versionDir = dir; version = pinned
                // The unpack's size and time (portable-app-download records them).
                if installed { log("installed runtime \(pinned) into \(dir.path) in \(String(format: "%.2f", Date().timeIntervalSince(started))) s") }
            case let .missing(reason):
                publish { $0["state"] = "runtime-missing"; $0["install"] = ["reason": reason] }
                return (reason, false)
            case let .failed(reason):
                log("runtime install failed: \(reason)")
                publish { $0["state"] = "failed"; $0["install"] = ["reason": reason] }
                return (reason, false)
            }
        }
        let config = Self.startConfig(versionDir: versionDir, port: port, home: home, token: bootstrapToken, env: serverEnvironment(env), cwd: NSHomeDirectory())
        let httpBaseUrl = config.httpBaseUrl
        let logDirectory = home.appendingPathComponent("userdata/logs", isDirectory: true)
        let manager = T3LocalBackendManager(
            executor: T3LocalQueueExecutor(queue: queue), clock: T3LocalQueueClock(queue: queue),
            spawner: makeSpawner(), prober: makeProber(),
            log: T3LocalFileOutputLog(logDirectory: logDirectory, runId: runId),
            spec: T3LocalBackendSpec(configResolve: { config }))
        manager.logger = { [weak self] in self?.log($0) }
        manager.spec.onStarted = { pid, config in T3LocalPidFile.write(pidFile, pid: pid, executable: config.executablePath) }
        manager.spec.onReady = { [weak self] url in self?.exchange(url) }
        manager.changed = { [weak self, weak manager] in
            guard let self, let manager else { return }
            self.publish { self.describe(manager, into: &$0) }
            self.settleWaiters(manager)
        }
        publish {
            $0["port"] = port; $0["httpBaseUrl"] = httpBaseUrl.absoluteString; $0["wsBaseUrl"] = "ws://127.0.0.1:\(port)"
            $0["version"] = version; $0["t3Home"] = home.path; $0["state"] = "starting"; $0["failure"] = ""
        }
        _ = Self.exitHook
        lock.lock(); self.manager = manager; lock.unlock()
        return nil
    }

    /// The Local environment switch (decision U4: the reference relaunches the app; exact2 cannot, issue
    /// X45 / #122, so this is the stopgap): off stops the server (SIGTERM, SIGKILL after 2 s, at most 5 s);
    /// on starts it and answers once it is ready. `done(nil)` on success, else the reason. TypeScript
    /// persists the setting first and reconnects after.
    func setEnabled(_ enabled: Bool, done: @escaping (String?) -> Void) {
        installQueue.async { [self] in
            publish { $0["enabled"] = enabled }
            lock.lock(); let current = manager; let pidFile = self.pidFile; lock.unlock()
            if !enabled {
                guard let current else { publish { $0["state"] = $0["state"] as? String == "refused" ? "refused" : "stopped" }; return done(nil) }
                return queue.async { [self] in
                    failWaiters("The local environment was turned off.")
                    current.stop(timeoutMs: 5_000) { [self] in
                        if let pidFile, current.snapshot.activePid == nil { try? FileManager.default.removeItem(at: pidFile) }
                        publish { describe(current, into: &$0) }
                        done(nil)
                    }
                }
            }
            if let failure = prepare() { return done(failure.message) }
            lock.lock(); let manager = self.manager; lock.unlock()
            guard let manager else { return done("The local server is not available.") }
            queue.async { [self] in
                if manager.snapshot.ready, auth.cached != nil { return done(nil) }
                readyWaiters.append(done)
                waitEpoch += 1
                let epoch = waitEpoch
                queue.asyncAfter(deadline: .now() + startTimeout) { [self] in
                    guard !readyWaiters.isEmpty, waitEpoch == epoch else { return }
                    failWaiters("The local server did not start in time.", stop: manager)
                }
                manager.start()
            }
        }
    }

    /// The first-launch view's Retry (20261005-portable-app-download): a runtime install that failed
    /// runs again (the lock and the `.staging-*` cleanup make it start clean), then the server starts.
    /// `done(nil)` once the install passed, else the reason; a server that already exists is left alone.
    func retryInstall(done: @escaping (String?) -> Void) {
        installQueue.async { [self] in
            lock.lock(); let existing = manager; let state = status["state"] as? String; lock.unlock()
            guard existing == nil, state == "failed" else { return done(nil) }
            publish { $0["state"] = "installing"; $0["install"] = ["phase": "verify", "fraction": 0.0] }
            if let failure = prepare() {
                if failure.fatal { fatal("bootstrap", failure.message) }
                return done(failure.message)
            }
            lock.lock(); let manager = self.manager; let wanted = !listeners.isEmpty; lock.unlock()
            if wanted, let manager { queue.async { manager.start() } }
            done(nil)
        }
    }

    /// On the manager's queue: ready (with its bearer) answers the waiters; an exit before ready fails
    /// them and stops the server, so the switch and the server agree.
    private func settleWaiters(_ manager: T3LocalBackendManager) {
        guard !readyWaiters.isEmpty else { return }
        let snapshot = manager.snapshot
        if snapshot.restartScheduled, !snapshot.ready {
            failWaiters("The local server stopped before it was ready (\(manager.lastExit ?? "no exit status")).", stop: manager)
        }
    }
    private func failWaiters(_ reason: String, stop: T3LocalBackendManager? = nil) {
        let waiters = readyWaiters
        readyWaiters = []
        if let stop { stop.stop(timeoutMs: 5_000) { [weak self] in self?.publish { self?.describe(stop, into: &$0) } } }
        waiters.forEach { $0(reason) }
    }
    private func answerWaiters() {
        queue.async { [self] in
            let waiters = readyWaiters
            readyWaiters = []
            waiters.forEach { $0(nil) }
        }
    }

    /// `resolvePrimaryStartConfig` for the SEA: `t3 --bootstrap-fd 0` from the version folder, cwd the
    /// home folder (`backendCwd` of a packaged build), local-only (`127.0.0.1`), no Tailscale Serve
    /// (this-machine-network-access adds exposure), the resource monitor when the tree has one.
    static func startConfig(versionDir: URL, port: Int, home: URL, token: String, env: [String: String], cwd: String) -> T3LocalStartConfig {
        let monitor = versionDir.appendingPathComponent("resource-monitor/darwin-arm64/t3-resource-monitor")
        return T3LocalStartConfig(
            executablePath: versionDir.appendingPathComponent("t3").path,
            args: ["--bootstrap-fd", "0"],
            cwd: cwd,
            env: env,
            bootstrap: T3LocalBootstrap(port: port, t3Home: home.path, desktopBootstrapToken: token,
                                        resourceMonitorPath: FileManager.default.isExecutableFile(atPath: monitor.path) ? monitor.path : nil),
            httpBaseUrl: URL(string: "http://127.0.0.1:\(port)")!)
    }

    /// The token this app run's servers are started with (tests read it; it is never logged).
    var currentBootstrapToken: String { bootstrapToken }

    private func describe(_ manager: T3LocalBackendManager, into status: inout [String: Any]) {
        let snapshot = manager.snapshot
        status["state"] = snapshot.ready ? "ready" : snapshot.restartScheduled ? "restarting" : snapshot.desiredRunning ? "starting" : "stopped"
        status["restartAttempt"] = snapshot.restartAttempt
        status["nextRestartMs"] = manager.nextRestartMs.map { Int($0.rounded()) } ?? NSNull()
        status["pid"] = snapshot.activePid.map { Int($0) } ?? NSNull()
        status["lastExit"] = manager.lastExit ?? NSNull()
        status["bearerReady"] = auth.cached != nil
    }

    /// At each ready: the bearer (exchanged once per app run) and the server's descriptor, whose
    /// environment id and label make the primary environment (`loadPrimaryConnectionRegistration`).
    private func exchange(_ base: URL) {
        let group = DispatchGroup()
        group.enter()
        auth.bearerToken(base: base, credential: bootstrapToken) { [weak self] token in
            if token == nil { self?.log("the bearer exchange failed") }
            self?.publish { $0["bearerReady"] = token != nil }
            group.leave()
        }
        group.enter()
        var request = URLRequest(url: URL(string: "/.well-known/t3/environment", relativeTo: base)!.absoluteURL, timeoutInterval: 10)
        request.httpMethod = "GET"
        session.dataTask(with: request) { [weak self] data, response, _ in
            defer { group.leave() }
            guard let self, ((response as? HTTPURLResponse)?.statusCode).map({ (200..<300).contains($0) }) == true,
                  let object = data.flatMap({ try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }),
                  let environmentId = object["environmentId"] as? String, !environmentId.isEmpty else { return }
            self.publish {
                $0["environmentId"] = environmentId; $0["label"] = object["label"] as? String ?? ""
                $0["serverVersion"] = object["serverVersion"] as? String ?? ""
            }
        }.resume()
        group.notify(queue: queue) { [weak self] in
            guard let self else { return }
            if self.auth.cached != nil { self.answerWaiters() }
            else { self.failWaiters("The local server did not issue a session for this app.") }
        }
    }
}

/// `DesktopLocalEnvironmentAuth`: the bootstrap token exchanged once for a bearer (no scope, so every
/// scope; "T3 Code Desktop", desktop), kept in memory for the app run and never logged or written.
/// Requests that arrive while the exchange runs wait for it (the reference's semaphore).
final class T3LocalAuth: @unchecked Sendable {
    private let lock = NSLock()
    private var token: String?
    private var waiting: [(String?) -> Void] = []
    let session: URLSession
    init(session: URLSession = URLSession(configuration: .ephemeral)) { self.session = session }

    var cached: String? { lock.lock(); defer { lock.unlock() }; return token }

    func bearerToken(base: URL, credential: String, _ done: @escaping (String?) -> Void) {
        lock.lock()
        if let token { lock.unlock(); return done(token) }
        waiting.append(done)
        let first = waiting.count == 1
        lock.unlock()
        guard first else { return }
        var request = URLRequest(url: URL(string: "/oauth/token", relativeTo: base)!.absoluteURL, timeoutInterval: 10)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "content-type")
        request.httpBody = T3RemoteAuth.exchangeForm(credential: credential, client: .localDesktop)
        session.dataTask(with: request) { [self] data, response, _ in
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            let object = (200..<300).contains(status) ? data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] } : nil
            let token = object?["access_token"] as? String
            lock.lock()
            if let token { self.token = token }
            let callbacks = waiting
            waiting = []
            lock.unlock()
            callbacks.forEach { $0(token) }
        }.resume()
    }
}

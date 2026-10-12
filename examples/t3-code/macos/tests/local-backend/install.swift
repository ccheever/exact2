import Foundation
import XCTest
import CryptoKit

// The first-launch unpack (T3LocalRuntime.swift), the development-build policy (item 8 of
// 20261005-embedded-server-runtime), port selection, the crash reaper, and a real child: the
// production spawner starting a fake `t3` script that records its envelope.

private func scratch(_ name: String) -> URL {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("t3-local-\(name)-\(UUID().uuidString)", isDirectory: true)
    try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url.resolvingSymlinksInPath()
}

private func sha256(_ url: URL) -> String { T3LocalRuntime.sha256(url)! }

private func run(_ tool: String, _ args: [String]) {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: tool)
    process.arguments = args
    try! process.run(); process.waitUntilExit()
    precondition(process.terminationStatus == 0, "\(tool) \(args)")
}

/// A release-shaped archive (`t3-<version>-darwin-arm64/…`), its pin and stage-runtime.mjs's manifest.
private struct Fixture {
    let bundleDir: URL
    let home: URL
    let version = "1.2.3"

    init() {
        let root = scratch("fixture")
        let tree = root.appendingPathComponent("t3-1.2.3-darwin-arm64", isDirectory: true)
        let files = FileManager.default
        try! files.createDirectory(at: tree.appendingPathComponent("node_modules/@scope/pkg"), withIntermediateDirectories: true)
        try! files.createDirectory(at: tree.appendingPathComponent("client"), withIntermediateDirectories: true)
        try! "#!/bin/sh\necho \"t3 v1.2.3\"\n".write(to: tree.appendingPathComponent("t3"), atomically: true, encoding: .utf8)
        try! files.setAttributes([.posixPermissions: 0o755], ofItemAtPath: tree.appendingPathComponent("t3").path)
        try! "module.exports = 1\n".write(to: tree.appendingPathComponent("node_modules/@scope/pkg/index.js"), atomically: true, encoding: .utf8)
        try! files.createSymbolicLink(atPath: tree.appendingPathComponent("node_modules/@scope/pkg/main.js").path, withDestinationPath: "index.js")
        try! "<html></html>\n".write(to: tree.appendingPathComponent("client/index.html"), atomically: true, encoding: .utf8)
        bundleDir = root.appendingPathComponent("t3-runtime", isDirectory: true)
        try! files.createDirectory(at: bundleDir, withIntermediateDirectories: true)
        let archive = bundleDir.appendingPathComponent("t3-1.2.3-darwin-arm64.tar.gz")
        run("/usr/bin/tar", ["-czf", archive.path, "-C", root.path, "t3-1.2.3-darwin-arm64"])
        var entries: [[String: Any]] = []
        let walker = files.enumerator(atPath: tree.path)!
        var paths: [String] = []
        for case let path as String in walker { paths.append(path) }
        var bytes = 0, count = 0
        for path in paths.sorted() {
            let url = tree.appendingPathComponent(path)
            var info = stat(); lstat(url.path, &info)
            let mode = Int(info.st_mode) & 0o7777
            switch info.st_mode & S_IFMT {
            case S_IFLNK: entries.append(["path": path, "type": "link", "link": try! files.destinationOfSymbolicLink(atPath: url.path)])
            case S_IFDIR: entries.append(["path": path, "type": "dir", "mode": mode])
            default:
                entries.append(["path": path, "type": "file", "mode": mode, "size": Int(info.st_size), "sha256": sha256(url)])
                bytes += Int(info.st_size); count += 1
            }
        }
        let size = (try! files.attributesOfItem(atPath: archive.path)[.size] as! NSNumber).intValue
        let hash = sha256(archive)
        let pin: [String: Any] = ["version": version, "asset": archive.lastPathComponent, "size": size, "sha256": hash]
        try! JSONSerialization.data(withJSONObject: pin).write(to: bundleDir.appendingPathComponent("runtime-pin.json"))
        let manifest: [String: Any] = ["version": version, "asset": archive.lastPathComponent, "sha256": hash, "size": size, "files": count, "bytes": bytes, "entries": entries]
        try! JSONSerialization.data(withJSONObject: manifest).write(to: bundleDir.appendingPathComponent("runtime-manifest.json"))
        home = scratch("home")
    }

    var archive: URL { bundleDir.appendingPathComponent("t3-1.2.3-darwin-arm64.tar.gz") }
    var versions: URL { T3LocalRuntime.versionsDir(home: home) }
    func runtime() -> T3LocalRuntime { T3LocalRuntime(home: home, bundleDir: bundleDir) }
}

final class LocalRuntimeInstallTests: XCTestCase {
    func testFirstLaunchUnpacksIntoTheVersionsFolder() {
        let fixture = Fixture()
        let runtime = fixture.runtime()
        var phases: [String] = [], last = 0.0
        runtime.progress = { phase, fraction in if phases.last != phase { phases.append(phase) }; last = fraction }
        let result = runtime.ensure()
        let paths = T3LocalRuntime.paths(home: fixture.home, version: "1.2.3")
        XCTAssertEqual(result, .ready(versionDir: paths.versionDir, version: "1.2.3", installed: true))
        XCTAssertEqual(phases, ["verify", "extract"])
        XCTAssertEqual(last, 1)
        XCTAssertEqual(try? String(contentsOf: paths.sentinel, encoding: .utf8), "1.2.3\n")
        XCTAssertEqual((try? FileManager.default.attributesOfItem(atPath: paths.entry.path)[.posixPermissions] as? NSNumber)?.intValue, 0o755)
        XCTAssertEqual(try? FileManager.default.destinationOfSymbolicLink(atPath: paths.versionDir.appendingPathComponent("node_modules/@scope/pkg/main.js").path), "index.js")
        XCTAssertEqual(paths.versionDir.path, fixture.home.appendingPathComponent("runtime/versions/1.2.3").path)
        XCTAssertFalse(((try? FileManager.default.contentsOfDirectory(atPath: fixture.versions.path)) ?? []).contains { $0.hasPrefix(".staging-") })
    }

    func testASecondLaunchUsesTheInstalledRuntime() {
        let fixture = Fixture()
        _ = fixture.runtime().ensure()
        let paths = T3LocalRuntime.paths(home: fixture.home, version: "1.2.3")
        XCTAssertEqual(fixture.runtime().ensure(), .ready(versionDir: paths.versionDir, version: "1.2.3", installed: false))
        // The same layout the `t3` installer writes: a runtime it installed is used as it is.
        try? FileManager.default.removeItem(at: fixture.archive)
        XCTAssertEqual(fixture.runtime().ensure(), .ready(versionDir: paths.versionDir, version: "1.2.3", installed: false))
    }

    func testATamperedArchiveIsRefusedBeforeItIsExpanded() {
        let fixture = Fixture()
        let handle = try! FileHandle(forUpdating: fixture.archive)
        try! handle.seek(toOffset: 20); try! handle.write(contentsOf: Data([0x00, 0x01, 0x02])); try! handle.close()
        guard case let .failed(reason) = fixture.runtime().ensure() else { return XCTFail("a tampered archive must fail") }
        XCTAssertTrue(reason.contains("SHA-256"), reason)
        XCTAssertFalse(FileManager.default.fileExists(atPath: fixture.versions.appendingPathComponent("1.2.3").path))
    }

    func testAFileThatDiffersFromTheManifestFailsAndLeavesNoStaging() {
        let fixture = Fixture()
        let url = fixture.bundleDir.appendingPathComponent("runtime-manifest.json")
        var manifest = try! JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [String: Any]
        manifest["entries"] = (manifest["entries"] as! [[String: Any]]).map { entry in
            var entry = entry
            if entry["path"] as? String == "client/index.html" { entry["sha256"] = String(repeating: "0", count: 64) }
            return entry
        }
        try! JSONSerialization.data(withJSONObject: manifest).write(to: url)
        guard case let .failed(reason) = fixture.runtime().ensure() else { return XCTFail("a manifest mismatch must fail") }
        XCTAssertTrue(reason.contains("client/index.html"), reason)
        XCTAssertEqual(((try? FileManager.default.contentsOfDirectory(atPath: fixture.versions.path)) ?? []).filter { !$0.hasPrefix(".install.lock") }, [])
    }

    func testAnInterruptedUnpackIsRemovedAtTheNextLaunch() {
        let fixture = Fixture()
        let leftover = fixture.versions.appendingPathComponent(".staging-abc123/node_modules")
        try! FileManager.default.createDirectory(at: leftover, withIntermediateDirectories: true)
        _ = fixture.runtime().ensure()
        XCTAssertFalse(FileManager.default.fileExists(atPath: fixture.versions.appendingPathComponent(".staging-abc123").path))
    }

    func testADevelopmentBuildWithoutAStagedRuntimeIsRuntimeMissing() {
        let fixture = Fixture()
        try! FileManager.default.removeItem(at: fixture.archive)
        guard case let .missing(reason) = fixture.runtime().ensure() else { return XCTFail("no archive must be runtime-missing") }
        XCTAssertTrue(reason.contains("stage-runtime.mjs"), reason)
        XCTAssertEqual(T3LocalRuntime(home: fixture.home, bundleDir: nil).ensure(), .missing("The app bundle has no runtime-pin.json."))
    }

    func testTwoLaunchesDoNotUnpackTogether() {
        let fixture = Fixture()
        var results: [T3LocalRuntimeResult] = []
        let lock = NSLock(), group = DispatchGroup()
        for _ in 0..<2 {
            DispatchQueue.global().async(group: group) {
                let result = fixture.runtime().ensure()
                lock.lock(); results.append(result); lock.unlock()
            }
        }
        group.wait()
        let installs = results.filter { if case .ready(_, _, true) = $0 { return true }; return false }
        XCTAssertEqual(results.count, 2)
        XCTAssertEqual(installs.count, 1, "\(results)")
    }
}

final class LocalPolicyTests: XCTestCase {
    func testADevelopmentBuildWithoutItsVariablesIsRefused() {
        let home = scratch("home").path
        XCTAssertEqual(T3LocalPolicy.resolve(env: [:], packaged: false, home: home, accountHome: home), .refused(T3LocalPolicy.developmentMissing))
        XCTAssertEqual(T3LocalPolicy.resolve(env: ["T3_LOCAL_HOME": "/tmp/lane"], packaged: false, home: home, accountHome: home), .refused(T3LocalPolicy.developmentMissing))
        XCTAssertEqual(T3LocalPolicy.resolve(env: ["T3_LOCAL_PORT": "16437"], packaged: false, home: home, accountHome: home), .refused(T3LocalPolicy.developmentMissing))
    }

    func testTheRealHomeAndPort3773AreRefused() {
        let home = scratch("home")
        try! FileManager.default.createDirectory(at: home.appendingPathComponent(".t3/userdata"), withIntermediateDirectories: true)
        try! FileManager.default.createSymbolicLink(at: home.appendingPathComponent("link-to-t3"), withDestinationURL: home.appendingPathComponent(".t3"))
        let refuse = { (env: [String: String]) in T3LocalPolicy.resolve(env: env, packaged: false, home: home.path, accountHome: home.path) }
        for value in ["~/.t3", "~/.t3/x", home.appendingPathComponent("link-to-t3").path, "~", home.path, "~/lanes/../.t3"] {
            XCTAssertEqual(refuse(["T3_LOCAL_HOME": value, "T3_LOCAL_PORT": "16437"]), .refused(T3LocalPolicy.developmentRefused), value)
        }
        for port in ["3773", "8080", "15999", "17000", "x"] {
            XCTAssertEqual(refuse(["T3_LOCAL_HOME": "/tmp/lane-t3", "T3_LOCAL_PORT": port]), .refused(T3LocalPolicy.developmentRefused), port)
        }
        // The one override lifts only the home refusal.
        XCTAssertEqual(refuse(["T3_LOCAL_HOME": "~/.t3", "T3_LOCAL_PORT": "16437", "T3_LOCAL_ALLOW_REAL": "1"]),
                       .allowed(home: home.appendingPathComponent(".t3", isDirectory: true), configuredPort: 16437, scanStart: 3773, runtimeDir: nil))
        XCTAssertEqual(refuse(["T3_LOCAL_HOME": "~/.t3", "T3_LOCAL_PORT": "3773", "T3_LOCAL_ALLOW_REAL": "1"]), .refused(T3LocalPolicy.developmentRefused))
        // The account's home counts too, when HOME points elsewhere.
        let other = scratch("other").path
        XCTAssertEqual(T3LocalPolicy.resolve(env: ["T3_LOCAL_HOME": home.appendingPathComponent(".t3").path, "T3_LOCAL_PORT": "16437"], packaged: false, home: other, accountHome: home.path),
                       .refused(T3LocalPolicy.developmentRefused))
    }

    func testALaneHomeAndPortAreAllowed() {
        let home = scratch("home").path, lane = scratch("lane")
        XCTAssertEqual(T3LocalPolicy.resolve(env: ["T3_LOCAL_HOME": lane.appendingPathComponent("t3-home").path, "T3_LOCAL_PORT": "16437"], packaged: false, home: home, accountHome: home),
                       .allowed(home: lane.appendingPathComponent("t3-home", isDirectory: true), configuredPort: 16437, scanStart: 3773, runtimeDir: nil))
    }

    func testThePackagedBuildDefaultsToTheSharedHomeAndTheScanFrom3773() {
        let resources = scratch("resources"), bare = scratch("bare"), home = scratch("home").path
        try! #"{"flavor":"packaged"}"#.write(to: resources.appendingPathComponent("distribution.json"), atomically: true, encoding: .utf8)
        XCTAssertTrue(T3LocalPolicy.packaged(resources: resources))
        XCTAssertFalse(T3LocalPolicy.packaged(resources: bare))
        XCTAssertEqual(T3LocalPolicy.resolve(env: [:], packaged: T3LocalPolicy.packaged(resources: resources), home: home, accountHome: home),
                       .allowed(home: URL(fileURLWithPath: home).appendingPathComponent(".t3", isDirectory: true), configuredPort: nil, scanStart: 3773, runtimeDir: nil))
        XCTAssertEqual(T3LocalPolicy.resolve(env: [:], packaged: T3LocalPolicy.packaged(resources: bare), home: home, accountHome: home),
                       .refused(T3LocalPolicy.developmentMissing))
        XCTAssertEqual(T3LocalPolicy.resolve(env: ["T3CODE_PORT": "4888", "T3CODE_HOME": "/srv/t3"], packaged: true, home: home, accountHome: home),
                       .allowed(home: URL(fileURLWithPath: "/srv/t3", isDirectory: true), configuredPort: 4888, scanStart: 3773, runtimeDir: nil))
    }

    func testPortSelectionScansFromTheStartAcrossAllThreeHosts() {
        XCTAssertEqual(try? T3LocalPorts.resolve(configured: 16437, scanStart: 3773) { _, _ in false }.get(), 16437)
        var asked: [String] = []
        let port = try? T3LocalPorts.resolve(configured: nil, scanStart: 3773) { port, host in asked.append("\(port)@\(host)"); return port >= 3775 || (port == 3774 && host != "::") }.get()
        XCTAssertEqual(port, 3775)
        XCTAssertEqual(asked, ["3773@127.0.0.1", "3774@127.0.0.1", "3774@0.0.0.0", "3774@::", "3775@127.0.0.1", "3775@0.0.0.0", "3775@::"])
        if case let .failure(error) = T3LocalPorts.resolve(configured: nil, scanStart: 65535, canListen: { _, _ in false }) {
            XCTAssertEqual(error.message, "No desktop backend port is available on hosts 127.0.0.1, 0.0.0.0, :: between 65535 and 65535.")
        } else { XCTFail("no port must fail") }
        // A real listener makes its port unavailable.
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        var address = sockaddr_in(); address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET); address.sin_port = 0
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        _ = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        Darwin.listen(fd, 1)
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) } }
        let bound = Int(UInt16(bigEndian: address.sin_port))
        XCTAssertFalse(T3LocalPorts.canListen(bound, host: "127.0.0.1"))
        Darwin.close(fd)
        XCTAssertTrue(T3LocalPorts.canListen(bound, host: "127.0.0.1"))
    }
}

final class LocalCrashReaperTests: XCTestCase {
    private func sleeper() -> Process {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sleep")
        process.arguments = ["30"]
        try! process.run()
        return process
    }

    func testALeftoverServerIsReapedOnlyWhenItIsTheSameProcess() {
        let directory = scratch("pid"), file = directory.appendingPathComponent("embedded-server.pid")
        let left = sleeper()
        T3LocalPidFile.write(file, pid: left.processIdentifier, executable: "/bin/sleep")
        XCTAssertEqual(T3LocalPidFile.reap(file), left.processIdentifier)
        left.waitUntilExit()
        XCTAssertFalse(left.isRunning)
        XCTAssertFalse(FileManager.default.fileExists(atPath: file.path))

        // Another program under a recorded pid (a reused pid) is left alone.
        let other = sleeper()
        T3LocalPidFile.write(file, pid: other.processIdentifier, executable: "/Users/nobody/.t3/runtime/versions/1.2.3/t3")
        XCTAssertNil(T3LocalPidFile.reap(file))
        XCTAssertTrue(other.isRunning)
        other.terminate()
        // So is a record whose start time is not the process's.
        let third = sleeper()
        let record = T3LocalPidFile.Record(pid: third.processIdentifier, startTime: 1, executable: "/bin/sleep")
        try! JSONEncoder().encode(record).write(to: file)
        XCTAssertNil(T3LocalPidFile.reap(file))
        XCTAssertTrue(third.isRunning)
        third.terminate()
    }
}

final class LocalProcessTests: XCTestCase {
    /// The production spawner with a fake `t3` that records its argv, environment and stdin, prints
    /// to both streams and exits 3: the envelope arrives on stdin and nowhere else, output reaches
    /// the log, and the unexpected exit restarts it after 0.5 s.
    func testTheFakeT3ReceivesTheEnvelopeOnStdinOnly() {
        let directory = scratch("fake-t3"), script = directory.appendingPathComponent("t3")
        try! """
        #!/bin/sh
        IFS= read -r line
        printf '%s' "$line" > "$FAKE_T3_OUT/envelope.json"
        printf '%s\\n' "$@" > "$FAKE_T3_OUT/argv.txt"
        env > "$FAKE_T3_OUT/env.txt"
        echo "fake server out"
        echo "fake server err" >&2
        exit 3
        """.write(to: script, atomically: true, encoding: .utf8)
        try! FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: script.path)
        let token = T3LocalBackend.shared.currentBootstrapToken
        let env = T3LocalShellEnvironment.serverEnvironment(base: ["PATH": "/usr/bin:/bin", "FAKE_T3_OUT": directory.path, "T3CODE_PORT": "1"]) { _, _, _ in "" }
        var config = T3LocalBackend.startConfig(versionDir: directory, port: 16999, home: directory.appendingPathComponent("home"), token: token, env: env, cwd: directory.path)
        config.executablePath = script.path
        let queue = DispatchQueue(label: "test.local")
        let log = RecordingLog(), prober = FakeProber()
        prober.fallback = nil
        let manager = T3LocalBackendManager(executor: T3LocalQueueExecutor(queue: queue), clock: T3LocalQueueClock(queue: queue),
                                            spawner: T3LocalProcessSpawner(), prober: prober, log: log, spec: T3LocalBackendSpec(configResolve: { config }))
        var pids: [Int32] = []
        manager.spec.onStarted = { pid, _ in pids.append(pid) }
        queue.async { manager.start() }
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline, queue.sync(execute: { pids.count < 2 }) { Thread.sleep(forTimeInterval: 0.05) }
        let done = DispatchSemaphore(value: 0)
        queue.async { manager.stop { done.signal() } }
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success)
        queue.sync {
            XCTAssertEqual(pids.count, 2, "exit 3 restarts after 0.5 s")
            XCTAssertEqual(log.failures.first.map { $0.hasSuffix("code=3") }, true, "\(log.failures)")
            XCTAssertTrue(log.output.joined().contains("fake server out"))
            XCTAssertTrue(log.output.joined().contains("fake server err"))
        }
        let envelope = try! JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("envelope.json"))) as! [String: Any]
        XCTAssertEqual(envelope["mode"] as? String, "desktop")
        XCTAssertEqual(envelope["port"] as? Int, 16999)
        XCTAssertEqual(envelope["desktopBootstrapToken"] as? String, token)
        XCTAssertEqual(envelope["host"] as? String, "127.0.0.1")
        XCTAssertEqual(envelope["t3Home"] as? String, directory.appendingPathComponent("home").path)
        let argv = try! String(contentsOf: directory.appendingPathComponent("argv.txt"), encoding: .utf8)
        XCTAssertEqual(argv, "--bootstrap-fd\n0\n")
        let childEnv = try! String(contentsOf: directory.appendingPathComponent("env.txt"), encoding: .utf8)
        XCTAssertFalse(childEnv.contains(token), "No token in the environment")
        XCTAssertTrue(childEnv.contains("T3CODE_TELEMETRY_ENABLED=false"))
        XCTAssertFalse(childEnv.contains("T3CODE_PORT="))
    }
}

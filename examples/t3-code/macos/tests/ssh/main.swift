import Foundation
import Darwin
import XCTest

// Add Environment → SSH (T3Ssh.swift). Compiles T3Link.swift (its T3Failure) and T3Ssh.swift
// directly. Discovery reads a temporary fixture home (never ~/.ssh). The live
// test runs only with T3_SSH_COMMAND naming an ssh test double whose "remote"
// is a local fixture T3 server (FAKE_SSH_REMOTE_HOME/.t3), and checks the whole
// flow: launch (reuse the external server), loopback tunnel, pairing token.
final class SshTests: XCTestCase {
    private func temporaryHome() throws -> String {
        let home = NSTemporaryDirectory() + "t3-ssh-\(UUID().uuidString)"
        let ssh = home + "/.ssh"
        try FileManager.default.createDirectory(atPath: ssh + "/conf.d", withIntermediateDirectories: true)
        try "Host devbox staging *.wild\n  HostName devbox.example\nInclude conf.d/*.conf\nHost !negated\n".write(toFile: ssh + "/config", atomically: true, encoding: .utf8)
        try "Host gpu\n  HostName 10.0.0.9\nInclude ~/.ssh/config\n".write(toFile: ssh + "/conf.d/extra.conf", atomically: true, encoding: .utf8)
        try "build.example,10.0.0.7 ssh-ed25519 AAAA\n[git.example]:2222 ssh-ed25519 AAAA\n|1|hashed= ssh-ed25519 AAAA\n@cert-authority *.corp ssh-ed25519 AAAA\ndevbox ssh-ed25519 AAAA\n"
            .write(toFile: ssh + "/known_hosts", atomically: true, encoding: .utf8)
        return home
    }

    func testDiscoveryReadsConfigIncludesAndKnownHostsWithoutPatterns() throws {
        let home = try temporaryHome()
        var visited = Set<String>()
        XCTAssertEqual(T3Ssh.configAliases(home + "/.ssh/config", home: home, visited: &visited), ["devbox", "gpu", "staging"])
        XCTAssertEqual(T3Ssh.knownHosts(try String(contentsOfFile: home + "/.ssh/known_hosts", encoding: .utf8)), ["10.0.0.7", "build.example", "devbox", "git.example"])
        setenv("T3_SSH_HOME", home, 1)
        defer { unsetenv("T3_SSH_HOME") }
        let ssh = T3Ssh(agent: true)
        let hosts = ssh.discover()
        XCTAssertEqual(hosts.map { $0["alias"] as? String ?? "" }, ["devbox", "gpu", "staging", "10.0.0.7", "build.example", "git.example"])
        XCTAssertEqual(hosts.first?["source"] as? String, "ssh-config")
        XCTAssertEqual(hosts.last?["source"] as? String, "known-hosts")
    }

    func testAgentRunsNeverReachRealSsh() {
        let saved = ProcessInfo.processInfo.environment["T3_SSH_COMMAND"]
        unsetenv("T3_SSH_HOME"); unsetenv("T3_SSH_COMMAND")
        defer { if let saved { setenv("T3_SSH_COMMAND", saved, 1) } }
        let ssh = T3Ssh(agent: true)
        XCTAssertNil(ssh.discoveryHome, "agent runs never read ~/.ssh")
        XCTAssertNil(ssh.sshCommand, "agent runs never run /usr/bin/ssh")
        XCTAssertEqual(ssh.discover().count, 0)
        XCTAssertThrowsError(try ssh.connect(T3Ssh.Target(alias: "devbox", hostname: "devbox", username: nil, port: nil), pair: false))
    }

    func testResolutionAndTargetValidation() throws {
        let resolved = T3Ssh.parseResolve(alias: "devbox", stdout: "hostname devbox.example\nuser me\nport 2222\nhostname ignored\n")
        XCTAssertEqual(resolved["hostname"] as? String, "devbox.example")
        XCTAssertEqual(resolved["username"] as? String, "me")
        XCTAssertEqual(resolved["port"] as? Int, 2222)
        XCTAssertEqual(T3Ssh.parseResolve(alias: "bare", stdout: "")["hostname"] as? String, "bare")
        XCTAssertThrowsError(try T3Ssh.target(["alias": "-oProxyCommand=evil"]))
        XCTAssertThrowsError(try T3Ssh.target(["alias": "a;b"]))
        XCTAssertThrowsError(try T3Ssh.target(["alias": "box", "port": 70000]))
        let target = try T3Ssh.target(["alias": "box", "username": "root", "port": 22])
        XCTAssertEqual(T3Ssh.hostSpec(target), "root@box")
        XCTAssertEqual(T3Ssh.stateKey(target).count, 16)
        XCTAssertEqual(T3Ssh.preferredPort(target), T3Ssh.preferredPort(try T3Ssh.target(["alias": "box", "username": "root", "port": 22])), "a target keeps its loopback port")
        XCTAssertTrue((41_000..<49_000).contains(T3Ssh.preferredPort(target)))
        XCTAssertEqual(T3Ssh.jsonObject("noise\n{\n  \"credential\": \"x\"\n}\n")?["credential"] as? String, "x")
        XCTAssertEqual(T3Ssh.sshError("Warning: Permanently added 'x'\nPermission denied (publickey).\n", fallback: "f"), "Permission denied (publickey).")
    }

    /// A listener on `address` (INADDR_ANY or loopback) and an ephemeral port that answers every
    /// connection with an empty HTTP 200. `stop` cancels its read source and waits for the descriptor to close.
    private func listener(any: Bool) throws -> (port: Int, stop: () -> Void) {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET); address.sin_port = 0
        address.sin_addr = in_addr(s_addr: any ? INADDR_ANY : inet_addr("127.0.0.1"))
        let bound = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        guard fd >= 0, bound == 0, listen(fd, 8) == 0 else { if fd >= 0 { close(fd) }; throw XCTSkip("no listener") }
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) } }
        let stopped = DispatchSemaphore(value: 0)
        _ = fcntl(fd, F_SETFL, O_NONBLOCK)
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: DispatchQueue(label: "ssh-test-listener"))
        source.setEventHandler {
            let client = accept(fd, nil, nil)
            guard client >= 0 else { return }
            defer { close(client) }
            _ = fcntl(client, F_SETFL, 0)
            var noSignal: Int32 = 1
            _ = setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &noSignal, socklen_t(MemoryLayout<Int32>.size))
            var buffer = [UInt8](repeating: 0, count: 1024)
            guard read(client, &buffer, buffer.count) > 0 else { return }
            let reply = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            _ = reply.withCString { write(client, $0, strlen($0)) }
        }
        source.setCancelHandler { close(fd); stopped.signal() }
        source.resume()
        var closed = false
        return (Int(UInt16(bigEndian: address.sin_port)), {
            guard !closed else { return }; closed = true
            source.cancel()
            XCTAssertEqual(stopped.wait(timeout: .now() + 2), .success)
        })
    }

    /// 9688e74112 (tunnel.test.ts / sshHelper.test.ts): a port another server answers on is not free,
    /// even where the loopback bind would succeed beside a wildcard listener.
    func testPortCheckSkipsAPortAnotherServerAnswersOn() throws {
        let wildcard = try listener(any: true)
        defer { wildcard.stop() }
        XCTAssertTrue(T3Ssh.answersOnLoopback(wildcard.port))
        XCTAssertFalse(T3Ssh.portFree(wildcard.port))
        let loopback = try listener(any: false)
        let port = loopback.port
        XCTAssertFalse(T3Ssh.portFree(port))
        loopback.stop()
        XCTAssertFalse(T3Ssh.answersOnLoopback(port), "a refused connection is free")
        XCTAssertTrue(T3Ssh.portFree(port))
    }

    /// 36f91c4568: reconnecting with no live default runtime finds the server it started still running
    /// and reuses it (left running) instead of starting a second one; no `t3` is needed for that.
    func testReconnectKeepsTheServerItStartedRunning() throws {
        let server = try listener(any: false)
        defer { server.stop() }
        let home = NSTemporaryDirectory() + "t3-ssh-launch-\(UUID().uuidString)"
        defer { try? FileManager.default.removeItem(atPath: home) }
        let state = home + "/.t3/ssh-launch/key"
        try FileManager.default.createDirectory(atPath: state, withIntermediateDirectories: true)
        let managed = Process()
        managed.executableURL = URL(fileURLWithPath: "/bin/sleep"); managed.arguments = ["30"]
        try managed.run()
        defer { managed.terminate() }
        try "\(managed.processIdentifier)\n".write(toFile: state + "/pid", atomically: true, encoding: .utf8)
        try "\(server.port)\n".write(toFile: state + "/port", atomically: true, encoding: .utf8)
        let ssh = T3Ssh(agent: true)
        defer { ssh.destroy() }
        let run = try ssh.run("/bin/sh", ["-s", "--", "key"], stdin: T3Ssh.launchScript, timeout: 30, environment: ["HOME": home, "PATH": "/usr/bin:/bin"])
        XCTAssertEqual(run.status, 0, run.stderr)
        XCTAssertEqual(T3Ssh.jsonObject(run.stdout)?["remotePort"] as? Int, server.port)
        XCTAssertEqual(T3Ssh.jsonObject(run.stdout)?["serverKind"] as? String, "managed")
        XCTAssertTrue(managed.isRunning, "the server is left running")

        // Alive but never ready: the script says so and still leaves it running.
        server.stop()
        let stalled = try ssh.run("/bin/sh", ["-s", "--", "key"], stdin: T3Ssh.launchScript, timeout: 60, environment: ["HOME": home, "PATH": "/usr/bin:/bin"])
        XCTAssertEqual(stalled.status, 1)
        XCTAssertTrue(stalled.stderr.contains("the server was left running"), stalled.stderr)
        XCTAssertTrue(managed.isRunning)
    }

    func testLiveTunnelThroughTheTestDouble() throws {
        guard let command = ProcessInfo.processInfo.environment["T3_SSH_COMMAND"], !command.isEmpty else { throw XCTSkip("T3_SSH_COMMAND names no ssh test double") }
        let ssh = T3Ssh(agent: true)
        defer { ssh.destroy() }
        let target = try T3Ssh.target(["alias": "devbox"])
        let bootstrap = try ssh.connect(target, pair: true)
        let origin = bootstrap["origin"] as? String ?? ""
        XCTAssertTrue(origin.hasPrefix("http://127.0.0.1:4"), origin)
        XCTAssertEqual(bootstrap["serverKind"] as? String, "external")
        XCTAssertFalse((bootstrap["credential"] as? String ?? "").isEmpty, "a pairing token came back")
        let port = Int(origin.split(separator: ":").last ?? "") ?? 0
        XCTAssertTrue(T3Ssh.httpReady(port, timeout: 2), "the tunnel answers")
        // A second connect reuses the open tunnel and its port.
        XCTAssertEqual(try ssh.connect(target, pair: false)["origin"] as? String, origin)
        let unreachable = try T3Ssh.target(["alias": "unreachable"])
        XCTAssertThrowsError(try ssh.connect(unreachable, pair: false)) { error in
            XCTAssertTrue((error as? T3Failure)?.message.contains("Connection refused") == true, "\(error)")
        }
    }
}

let suite = XCTestSuite(name: "T3 ssh")
suite.addTest(SshTests.defaultTestSuite); suite.addTest(SshAuthTests.defaultTestSuite) // auth.swift: password prompts, remote Open
suite.run()
let run = suite.testRun!
print("T3 ssh: \(run.executionCount) tests, \(run.skipCount) skipped, \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)

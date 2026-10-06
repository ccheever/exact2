import Foundation
import XCTest

// Add Environment → SSH (T3Ssh.swift). Compiles T3Protocol.swift and T3Ssh.swift
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

let suite = SshTests.defaultTestSuite
suite.run()
let run = suite.testRun!
print("T3 ssh: \(run.executionCount) tests, \(run.skipCount) skipped, \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)

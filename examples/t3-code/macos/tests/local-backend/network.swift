import Foundation
import XCTest

// 20261005-this-machine-network-access: the native facts behind Network access and Tailscale HTTPS
// (T3LocalNetwork.swift) and the restart with a new envelope (T3LocalBackend.restart, the U4 stopgap).
// The first tests keep the reference's names (MIT, see LICENSE-T3; reference 1e2ecbd975:
// apps/desktop/src/backend/DesktopNetworkInterfaces.test.ts and packages/tailscale/src/tailscale.test.ts,
// the process half); `tailscale serve` runs inside the server, so its tests stay there.
private final class AccessServer: URLProtocol, @unchecked Sendable {
    static var seen: [(method: String, path: String, authorization: String, body: String)] = []
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let path = request.url!.path
        var body = request.httpBody ?? Data()
        if body.isEmpty, let stream = request.httpBodyStream {
            stream.open(); defer { stream.close() }
            var buffer = [UInt8](repeating: 0, count: 4096)
            while stream.hasBytesAvailable { let count = stream.read(&buffer, maxLength: buffer.count); if count <= 0 { break }; body.append(buffer, count: count) }
        }
        if path.hasPrefix("/api/") { Self.seen.append((request.httpMethod ?? "", path, request.value(forHTTPHeaderField: "authorization") ?? "", String(decoding: body, as: UTF8.self))) }
        let status = path == "/api/auth/clients/revoke" ? 403 : 200
        let answer: Any = path == "/oauth/token" ? ["access_token": "local-bearer", "token_type": "Bearer"] as Any
            : path == "/api/auth/pairing-token" ? ["id": "link-1", "credential": "SECRET", "expiresAt": "2036-04-07T00:05:00.000Z"] as Any
            : path == "/api/auth/pairing-links" ? [[String: Any]]() as Any
            : ["environmentId": "local-env", "label": "Lane Mac", "serverVersion": "0.0.46-nightly.20261005.2667"] as Any
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: ["content-type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: answer))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

final class LocalNetworkTests: XCTestCase {
    private final class Owner {}
    private let owner = Owner()
    private func scratch(_ name: String) -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("t3-network-\(name)-\(UUID().uuidString)", isDirectory: true)
        try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.resolvingSymlinksInPath()
    }
    private func until(_ seconds: TimeInterval, _ condition: () -> Bool) -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end { if condition() { return true }; Thread.sleep(forTimeInterval: 0.02) }
        return condition()
    }
    private func status(_ runner: @escaping T3TailscaleCLI.Runner) -> Result<T3TailscaleCLI.Status, T3TailscaleCLI.Failure> {
        let done = DispatchSemaphore(value: 0)
        var result: Result<T3TailscaleCLI.Status, T3TailscaleCLI.Failure>!
        T3TailscaleCLI.readStatus(runner: runner) { result = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success)
        return result
    }
    private func output(_ stdout: String = "", stderr: String = "", code: Int32 = 0) -> T3TailscaleCLI.Runner {
        { _, _, done in done(.success(.init(code: code, stdout: Data(stdout.utf8), stderr: Data(stderr.utf8)))) }
    }

    // MARK: DesktopNetworkInterfaces

    func test_reads_network_interfaces_through_the_service() throws {
        let interfaces = ["en0": [T3NetworkAddress(address: "192.168.1.10", family: "IPv4", isInternal: false)]]
        XCTAssertEqual(try T3NetworkInterfaces.read { interfaces }, interfaces)
        // The real read: loopback is internal, every address has its family.
        let system = try T3NetworkInterfaces.read()
        XCTAssertTrue(system.values.contains { $0.contains { $0.address == "127.0.0.1" && $0.family == "IPv4" && $0.isInternal } }, "\(system)")
        XCTAssertTrue(system.values.allSatisfy { $0.allSatisfy { ["IPv4", "IPv6"].contains($0.family) } })
    }

    func test_preserves_network_interface_read_failures_as_structured_defects() {
        let cause = NSError(domain: "network interface probe failed", code: 1)
        XCTAssertThrowsError(try T3NetworkInterfaces.read(platform: "linux") { throw cause }) { error in
            guard let failure = error as? T3NetworkInterfaces.ReadError else { return XCTFail("\(error)") }
            XCTAssertEqual(failure.platform, "linux")
            XCTAssertEqual(failure.cause as NSError, cause)
            XCTAssertEqual(failure.message, "Failed to read desktop network interfaces on linux.")
        }
    }

    // MARK: readTailscaleStatus (the process half)

    func test_reads_tailscale_status_through_the_process_spawner_service() {
        var args: [String] = []
        let result = status { arguments, _, done in
            args = arguments
            done(.success(.init(code: 0, stdout: Data(#"{"Self":{"DNSName":"desktop.tail.ts.net.","TailscaleIPs":["100.90.1.2"]}}"#.utf8), stderr: Data())))
        }
        XCTAssertEqual(args, ["status", "--json"])
        XCTAssertEqual(try result.get(), T3TailscaleCLI.Status(magicDnsName: "desktop.tail.ts.net", tailnetIpv4Addresses: ["100.90.1.2"]))
    }

    func test_preserves_tailscale_spawn_failures_as_causes() {
        let result = status { _, _, done in done(.failure(.spawn)) }
        XCTAssertEqual(result, .failure(.spawn(subcommand: "status", argumentCount: 2)))
        if case let .failure(failure) = result { XCTAssertEqual(failure.message, "Failed to spawn tailscale status.") }
    }

    func test_keeps_nonzero_exit_diagnostics_structured() {
        let result = status(output(stderr: "not logged in tskey-auth-secret-token-value", code: 7))
        XCTAssertEqual(result, .failure(.exit(subcommand: "status", argumentCount: 2, exitCode: 7, stdoutLength: 0, stderrLength: 43, diagnostic: "not-logged-in")))
        if case let .failure(failure) = result {
            XCTAssertEqual(failure.message, "tailscale status exited with code 7.")
            XCTAssertFalse("\(failure)".contains("tskey-auth-secret-token-value"))
        }
    }

    func test_classifies_unrecognized_stderr_without_quoting_it() {
        let result = status(output(stderr: "something novel went wrong for node fluffy-badger tskey-auth-secret-token-value", code: 3))
        guard case let .failure(.exit(_, _, _, _, _, diagnostic)) = result else { return XCTFail("\(result)") }
        XCTAssertEqual(diagnostic, "unknown")
        XCTAssertFalse("\(result)".contains("fluffy-badger"))
        XCTAssertEqual(T3TailscaleCLI.diagnostic("serve permission denied"), "permission-denied")
        XCTAssertEqual(T3TailscaleCLI.diagnostic("error: handler does not exist"), "no-existing-handler")
        XCTAssertNil(T3TailscaleCLI.diagnostic("  "))
    }

    func test_times_out_tailscale_status_after_1500ms() {
        // The production runner over a `tailscale` on PATH that never answers.
        let bin = scratch("bin"), stub = bin.appendingPathComponent("tailscale")
        try! "#!/bin/sh\nsleep 30\n".write(to: stub, atomically: true, encoding: .utf8)
        try! FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: stub.path)
        let started = Date()
        let result = status(T3TailscaleCLI.processRunner(environment: { ["PATH": "\(bin.path):/usr/bin:/bin"] }))
        XCTAssertEqual(result, .failure(.timeout(subcommand: "status", argumentCount: 2, timeoutMs: 1500)))
        if case let .failure(failure) = result { XCTAssertEqual(failure.message, "tailscale status timed out after 1500ms.") }
        XCTAssertLessThan(Date().timeIntervalSince(started), 3)
        // No `tailscale` on PATH is a spawn failure; a stub that answers is read from stdout.
        XCTAssertEqual(status(T3TailscaleCLI.processRunner(environment: { ["PATH": "/usr/bin:/bin"] })), .failure(.spawn(subcommand: "status", argumentCount: 2)))
        try! "#!/bin/sh\n[ \"$1 $2\" = 'status --json' ] && echo '{\"Self\":{\"DNSName\":\"lane.tail.ts.net.\"}}'\n".write(to: stub, atomically: true, encoding: .utf8)
        XCTAssertEqual(try status(T3TailscaleCLI.processRunner(environment: { ["PATH": "\(bin.path):/usr/bin:/bin"] })).get().magicDnsName, "lane.tail.ts.net")
    }

    // MARK: The bind host at launch (configureFromSettings) and the cached facts

    func testTheLaunchBindHostFollowsTheSettingAndTheInterfaces() {
        let lan = ["en0": [T3NetworkAddress(address: "192.168.1.20", family: "IPv4", isInternal: false)]]
        let tailnet = ["utun4": [T3NetworkAddress(address: "100.90.1.2", family: "IPv4", isInternal: false)]]
        let loopback = ["lo0": [T3NetworkAddress(address: "127.0.0.1", family: "IPv4", isInternal: true)], "en5": [T3NetworkAddress(address: "169.254.3.4", family: "IPv4", isInternal: false)]]
        let network: [String: Any] = ["serverExposureMode": "network-accessible", "tailscaleServeEnabled": true, "tailscaleServePort": 8443]
        XCTAssertEqual(T3LocalExposure.atLaunch(settings: network, interfaces: lan, lanHostOverride: nil), T3LocalExposure(host: "0.0.0.0", tailscaleServeEnabled: true, tailscaleServePort: 8443))
        XCTAssertEqual(T3LocalExposure.atLaunch(settings: network, interfaces: tailnet, lanHostOverride: nil).host, "0.0.0.0", "Tailscale-only hosts stay network-accessible")
        XCTAssertEqual(T3LocalExposure.atLaunch(settings: network, interfaces: loopback, lanHostOverride: nil).host, "127.0.0.1", "no reachable address falls back to local-only")
        XCTAssertEqual(T3LocalExposure.atLaunch(settings: network, interfaces: loopback, lanHostOverride: "10.0.0.9").host, "0.0.0.0")
        XCTAssertEqual(T3LocalExposure.atLaunch(settings: [:], interfaces: lan, lanHostOverride: nil), T3LocalExposure())
        for port: Any in [0, 70_000, 44.5, "8443", true] { XCTAssertEqual(T3LocalExposure.normalizedPort(port), 443) }
    }

    func testFactsAnswerFromTheCacheAndReadTailscaleOnlyWhenAskedAndOnceAMinute() {
        let network = T3LocalNetwork()
        network.interfaces = { ["en0": [T3NetworkAddress(address: "192.168.1.20", family: "IPv4", isInternal: false)]] }
        network.environment = { ["T3CODE_DESKTOP_HTTPS_ENDPOINTS": " https://a.example , ,http://b.example:1 "] }
        var spawns = 0, announced = 0, clock = Date(timeIntervalSince1970: 1_000)
        network.now = { clock }
        network.runner = { _, _, done in spawns += 1; done(.success(.init(code: 0, stdout: Data(#"{"Self":{"DNSName":"lane.tail.ts.net."}}"#.utf8), stderr: Data()))) }
        let prober = FakeProber()
        network.prober = prober
        network.changed = { announced += 1 }
        let first = network.facts(tailscale: false, probe: "", refresh: false)
        XCTAssertEqual(spawns, 0, "local-only never spawns the CLI")
        XCTAssertNil(first["tailscale"])
        XCTAssertEqual(first["httpsEndpointUrls"] as? [String], ["https://a.example", "http://b.example:1"])
        XCTAssertNotNil((first["interfaces"] as? [String: Any])?["en0"])
        let pending = network.facts(tailscale: true, probe: "", refresh: false)
        XCTAssertEqual((pending["tailscale"] as? [String: Any])?["read"] as? Bool, false, "the first read answers at once and lands later")
        XCTAssertTrue(until(2) { announced == 1 })
        let read = network.facts(tailscale: true, probe: "https://lane.tail.ts.net/", refresh: false)
        XCTAssertEqual((read["tailscale"] as? [String: Any])?["magicDnsName"] as? String, "lane.tail.ts.net")
        XCTAssertEqual((read["probe"] as? [String: Any])?["read"] as? Bool, false)
        XCTAssertTrue(until(2) { (network.facts(tailscale: true, probe: "https://lane.tail.ts.net/", refresh: false)["probe"] as? [String: Any])?["reachable"] as? Bool == true })
        XCTAssertEqual(prober.urls, ["https://lane.tail.ts.net/.well-known/t3/environment"])
        clock = clock.addingTimeInterval(59)
        _ = network.facts(tailscale: true, probe: "", refresh: false)
        XCTAssertEqual(spawns, 1, "cached for 60 s")
        clock = clock.addingTimeInterval(2)
        _ = network.facts(tailscale: true, probe: "", refresh: false)
        XCTAssertTrue(until(2) { spawns == 2 })
        network.invalidate()
        XCTAssertEqual((network.facts(tailscale: true, probe: "", refresh: false)["tailscale"] as? [String: Any])?["read"] as? Bool, false, "a restart reads afresh")
    }

    // MARK: The restart with a new envelope and the access calls

    func testARestartStartsANewServerWithTheNewEnvelope() {
        let runtime = scratch("runtime"), script = runtime.appendingPathComponent("t3"), envelopes = scratch("envelopes")
        try! "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 't3 v0.0.46-nightly.20261005.2667'; exit 0; fi\nhead -n 1 > \(envelopes.path)/$$.json\nwhile :; do sleep 0.1; done\n"
            .write(to: script, atomically: true, encoding: .utf8)
        try! FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: script.path)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AccessServer.self]
        let backend = T3LocalBackend(session: URLSession(configuration: configuration)), home = scratch("home"), data = scratch("data")
        try! #"{"version":1,"serverExposureMode":"local-only"}"#.write(to: data.appendingPathComponent("t3-code.json"), atomically: true, encoding: .utf8)
        backend.environment = { ["T3_LOCAL_HOME": home.path, "T3_LOCAL_PORT": "16898", "T3_LOCAL_RUNTIME_DIR": runtime.path, "PATH": "/usr/bin:/bin"] }
        backend.resources = { nil }
        backend.serverEnvironment = { $0 }
        backend.makeProber = { FakeProber() }
        backend.fatal = { stage, message in XCTFail("unexpected fatal \(stage): \(message)") }
        backend.attach(owner, dataRoot: data, changed: { _ in })
        XCTAssertTrue(until(10) { backend.statusValue()["state"] as? String == "ready" && backend.statusValue()["bearerReady"] as? Bool == true }, "\(backend.statusValue())")
        XCTAssertEqual(backend.statusValue()["host"] as? String, "127.0.0.1")
        let pid = backend.statusValue()["pid"] as? Int ?? 0
        let done = DispatchSemaphore(value: 0)
        var failure: String? = "unanswered"
        backend.restart(exposure: T3LocalExposure(host: "0.0.0.0", tailscaleServeEnabled: true, tailscaleServePort: 8443)) { failure = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + 15), .success)
        XCTAssertNil(failure)
        let next = backend.statusValue()["pid"] as? Int ?? 0
        XCTAssertTrue(next > 1 && next != pid, "a new process: \(pid) → \(next)")
        XCTAssertEqual(backend.statusValue()["state"] as? String, "ready")
        XCTAssertEqual([backend.statusValue()["host"] as? String, "\(backend.statusValue()["tailscaleServePort"] ?? "")"], ["0.0.0.0", "8443"])
        XCTAssertEqual(backend.statusValue()["httpBaseUrl"] as? String, "http://127.0.0.1:16898", "the app still reaches it over loopback")
        var envelope: [String: Any]?
        XCTAssertTrue(until(5) {
            envelope = (try? Data(contentsOf: envelopes.appendingPathComponent("\(next).json"))).flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
            return envelope != nil
        }, "the new server read its envelope")
        XCTAssertEqual(envelope?["host"] as? String, "0.0.0.0")
        XCTAssertEqual(envelope?["tailscaleServeEnabled"] as? Bool, true)
        XCTAssertEqual(envelope?["tailscaleServePort"] as? Int, 8443)
        // localAccess: the six routes only, with the in-memory bearer; a refusal carries its status.
        AccessServer.seen = []
        func access(_ method: String, _ path: String, _ body: Any? = nil) -> Result<Any, T3Failure> {
            let answered = DispatchSemaphore(value: 0)
            var result: Result<Any, T3Failure>!
            backend.access(method: method, path: path, body: body) { result = $0; answered.signal() }
            XCTAssertEqual(answered.wait(timeout: .now() + 5), .success)
            return result
        }
        XCTAssertEqual((try? access("POST", "/api/auth/pairing-token", ["scopes": ["orchestration:read"]]).get()) as? [String: String], ["id": "link-1", "credential": "SECRET", "expiresAt": "2036-04-07T00:05:00.000Z"])
        if case let .failure(refused) = access("POST", "/api/auth/clients/revoke", ["sessionId": "s"]) { XCTAssertEqual([refused.kind, refused.detail], ["Http", "403"]) } else { XCTFail("revoke should fail") }
        if case let .failure(other) = access("GET", "/api/orchestration/shell") { XCTAssertEqual(other.kind, "Arguments") } else { XCTFail("not an access route") }
        XCTAssertEqual(AccessServer.seen.map { "\($0.method) \($0.path) \($0.authorization)" }, ["POST /api/auth/pairing-token Bearer local-bearer", "POST /api/auth/clients/revoke Bearer local-bearer"])
        XCTAssertEqual(AccessServer.seen.first?.body, #"{"scopes":["orchestration:read"]}"#)
        backend.detach(owner)
        XCTAssertTrue(until(6) { kill(pid_t(next), 0) != 0 })
    }
}

import Foundation
import Network
import XCTest

// Background environments (T3Fleet.swift) and the saved-environment catalog.
// Compiles T3Protocol.swift, T3Credentials.swift, T3Transport.swift and
// T3Fleet.swift directly; URLProtocol isolates HTTP from the network.
private final class FleetHTTP: URLProtocol, @unchecked Sendable {
    static let lock = NSLock()
    static var paths: [String] = []
    static var handler: (URLRequest) -> (Int, Any) = { _ in (500, [:]) }
    // The outdated-host socket goes to a real local WebSocket server (OutdatedSocketServer).
    override class func canInit(with request: URLRequest) -> Bool { !["ws", "wss"].contains(request.url?.scheme ?? "") }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        Self.lock.lock(); Self.paths.append(request.url!.absoluteString); let response = Self.handler(request); Self.lock.unlock()
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: response.0, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: response.1))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
    static func reset(_ handler: @escaping (URLRequest) -> (Int, Any)) { lock.lock(); paths = []; self.handler = handler; lock.unlock() }
    static var requested: [String] { lock.lock(); defer { lock.unlock() }; return paths }
}

private func fleetCall(_ fleet: T3Fleet, _ key: String, _ request: [String: Any]) -> [String: Any] {
    let ready = DispatchSemaphore(value: 0)
    var result: [String: Any] = [:]
    fleet.perform(key, request) { result = $0; ready.signal() }
    XCTAssertEqual(ready.wait(timeout: .now() + 3), .success)
    return result
}
private func transportCall(_ transport: T3Transport, _ request: [String: Any]) -> [String: Any] {
    let ready = DispatchSemaphore(value: 0)
    var result: [String: Any] = [:]
    transport.perform(request) { result = $0; ready.signal() }
    XCTAssertEqual(ready.wait(timeout: .now() + 3), .success)
    return result
}
private func waitFor(_ timeout: TimeInterval = 3, _ condition: () -> Bool) -> Bool {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline { if condition() { return true }; Thread.sleep(forTimeInterval: 0.02) }
    return condition()
}

final class FleetTests: XCTestCase {
    var configuration: URLSessionConfiguration {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [FleetHTTP.self]
        return config
    }

    func testSavedEnvironmentsKeepAddedOrderAndTheirSwitch() {
        let saved = T3SavedEnvironments(persistent: false)
        saved.remember(origin: "http://127.0.0.1:1", descriptor: ["environmentId": "A", "label": "First"])
        saved.remember(origin: "http://10.0.0.2:1", descriptor: ["environmentId": "B", "label": "Second"])
        XCTAssertTrue(saved.setEnabled(origin: "http://127.0.0.1:1", environment: "A", enabled: false))
        saved.remember(origin: "http://127.0.0.1:1", descriptor: ["environmentId": "A", "label": "First again"])
        XCTAssertEqual(saved.all.compactMap { $0["environmentId"] as? String }, ["A", "B"], "A reconnect must not reorder rows")
        XCTAssertEqual(saved.all.first?["enabled"] as? Bool, false, "A reconnect must not switch an environment back on")
        XCTAssertEqual(saved.all.first?["label"] as? String, "First again")
        XCTAssertEqual(saved.all.last?["enabled"] as? Bool, true)
        XCTAssertFalse(saved.setEnabled(origin: "http://127.0.0.1:9", environment: "Z", enabled: true))
    }

    func testTransportSwitchOpPersistsAndRefusesUnknownEnvironments() {
        FleetHTTP.reset { _ in XCTFail("The switch must not connect"); return (500, [:]) }
        let saved = T3SavedEnvironments(persistent: false)
        saved.remember(origin: "http://127.0.0.1:14819", descriptor: ["environmentId": "B", "label": "Remote"])
        let transport = T3Transport(persistent: false, configuration: configuration, savedEnvironments: saved, changed: { _ in })
        defer { transport.destroy() }
        let off = transportCall(transport, ["op": "setEnvironmentEnabled", "origin": "http://127.0.0.1:14819/", "environmentId": "B", "enabled": false])
        XCTAssertEqual(off["ok"] as? Bool, true)
        XCTAssertEqual(((off["value"] as? [String: Any])?["saved"] as? [[String: Any]])?.first?["enabled"] as? Bool, false)
        let missing = transportCall(transport, ["op": "setEnvironmentEnabled", "origin": "http://127.0.0.1:1", "environmentId": "Q", "enabled": true])
        XCTAssertEqual((missing["error"] as? [String: Any])?["kind"] as? String, "Missing")
        XCTAssertEqual(FleetHTTP.requested.count, 0)
    }

    func testFleetOpensOnlyOnConnectSharesCredentialsAndStops() throws {
        let credentials = T3Credentials(persistent: false), saved = T3SavedEnvironments(persistent: false)
        try credentials.save("bearer-b", origin: "http://127.0.0.1:14819", environment: "env-b")
        var changes: [String] = []
        let lock = NSLock()
        FleetHTTP.reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment": return (200, ["environmentId": "env-b", "label": "Remote", "orchestrationProtocolVersion": 2])
            case "/api/auth/session":
                XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer bearer-b", "The fleet must read the shared store")
                return (401, ["_tag": "EnvironmentAuthInvalidError", "message": "Session expired.", "traceId": "trace-123"])
            default: return (500, [:])
            }
        }
        let fleet = T3Fleet(persistent: false, credentials: credentials, saved: saved, configuration: configuration,
                            changed: { topic in lock.lock(); changes.append(topic); lock.unlock() })
        defer { fleet.destroy() }
        let key = "http://127.0.0.1:14819\nenv-b"
        let unknown = fleetCall(fleet, key, ["op": "status"])
        XCTAssertEqual((unknown["value"] as? [String: Any])?["state"] as? String, "disconnected")
        XCTAssertEqual((fleetCall(fleet, key, ["op": "request", "method": "server.getConfig"])["error"] as? [String: Any])?["kind"] as? String, "Disconnected")
        XCTAssertTrue(fleet.keys.isEmpty, "Only connect may open a background environment")
        XCTAssertEqual((fleetCall(fleet, "no-separator", ["op": "status"])["error"] as? [String: Any])?["kind"] as? String, "Arguments")
        let connected = fleetCall(fleet, key, ["op": "connect", "origin": "http://127.0.0.1:14819", "credential": ""])
        XCTAssertEqual(connected["ok"] as? Bool, false)
        XCTAssertEqual(fleet.keys, [key])
        XCTAssertTrue(waitFor { ((fleetCall(fleet, key, ["op": "status"])["value"] as? [String: Any])?["state"] as? String) == "error" })
        let status = fleetCall(fleet, key, ["op": "status"])["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(status["failureKind"] as? String, "Authentication")
        XCTAssertEqual(status["traceId"] as? String, "trace-123")
        XCTAssertNil(try credentials.read(origin: "http://127.0.0.1:14819", environment: "env-b"), "An expired session forgets the shared token")
        lock.lock(); let announced = changes; lock.unlock()
        XCTAssertTrue(!announced.isEmpty && announced.allSatisfy { $0 == "t3.fleet" }, "Background changes announce t3.fleet only")
        XCTAssertEqual((fleetCall(fleet, key, ["op": "fleetStop"])["value"] as? [String: Any])?["stopped"] as? Bool, true)
        XCTAssertTrue(fleet.keys.isEmpty)
    }

    func testFleetLimitsOpenEnvironments() {
        FleetHTTP.reset { _ in (500, ["message": "down"]) }
        let fleet = T3Fleet(persistent: false, credentials: T3Credentials(persistent: false), saved: T3SavedEnvironments(persistent: false),
                            configuration: configuration, changed: { _ in })
        defer { fleet.destroy() }
        for index in 0..<T3Fleet.limit { _ = fleetCall(fleet, "http://127.0.0.1:\(20000 + index)\nenv\(index)", ["op": "connect", "origin": "http://127.0.0.1:\(20000 + index)", "credential": ""]) }
        let refused = fleetCall(fleet, "http://127.0.0.1:29999\nmore", ["op": "connect", "origin": "http://127.0.0.1:29999", "credential": ""])
        XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "Limit")
    }
}

/// A local WebSocket endpoint for the outdated-host update socket: it records the
/// upgrade path and every frame, and answers each Request with `answer(frame, send)`.
private final class OutdatedSocketServer: @unchecked Sendable {
    let queue = DispatchQueue(label: "outdated-socket-server")
    let listener: NWListener
    let lock = NSLock()
    var frames: [[String: Any]] = []
    var paths: [String] = []
    var answer: (_ frame: [String: Any], _ send: @escaping ([String: Any]) -> Void, _ close: @escaping () -> Void) -> Void = { _, _, _ in }
    init() throws {
        let parameters = NWParameters.tcp
        let options = NWProtocolWebSocket.Options()
        options.autoReplyPing = true
        options.setClientRequestHandler(DispatchQueue(label: "outdated-socket-upgrade")) { _, _ in
            NWProtocolWebSocket.Response(status: .accept, subprotocol: nil)
        }
        parameters.defaultProtocolStack.applicationProtocols.insert(options, at: 0)
        listener = try NWListener(using: parameters, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in self?.accept(connection) }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 3)
    }
    var port: Int { Int(listener.port?.rawValue ?? 0) }
    private func accept(_ connection: NWConnection) {
        connection.start(queue: queue)
        let send = { (object: [String: Any]) in
            let data = try! JSONSerialization.data(withJSONObject: object)
            let context = NWConnection.ContentContext(identifier: "frame", metadata: [NWProtocolWebSocket.Metadata(opcode: .text)])
            connection.send(content: data, contentContext: context, isComplete: true, completion: .contentProcessed { _ in })
        }
        func loop() {
            connection.receiveMessage { [weak self] data, _, _, error in
                guard let self, error == nil, let data, let frame = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return }
                self.lock.lock(); self.frames.append(frame); let answer = self.answer; self.lock.unlock()
                answer(frame, send, { connection.cancel() })
                loop()
            }
        }
        loop()
    }
    var requests: [[String: Any]] { lock.lock(); defer { lock.unlock() }; return frames.filter { $0["_tag"] as? String == "Request" } }
    func stop() { listener.cancel() }
}

final class OutdatedHostTests: XCTestCase {
    var configuration: URLSessionConfiguration {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [FleetHTTP.self]
        return config
    }
    private func jobs(_ fleet: T3Fleet) -> [String: [String: Any]] {
        (fleetCall(fleet, "\n", ["op": "fleetOutdatedJobs"])["value"] as? [String: Any])?["jobs"] as? [String: [String: Any]] ?? [:]
    }

    func testCompatibilityByDirectionAndSelfUpdate() {
        XCTAssertNil(T3OutdatedHosts.compatibility(["orchestrationProtocolVersion": 2], label: "Studio"))
        let newer = T3OutdatedHosts.compatibility(["orchestrationProtocolVersion": 3, "capabilities": ["serverSelfUpdate": "respawn"]], label: "Studio")!
        XCTAssertEqual(newer.message, "This client is not supported by this server. Update your app or use a compatible release to connect to Studio.")
        XCTAssertFalse(newer.serverUpdateRequired)
        let older = T3OutdatedHosts.compatibility(["capabilities": ["serverSelfUpdate": "boot-service"]], label: "Studio")!
        XCTAssertEqual(older.message, "This client requires a newer server. Update T3 Code on Studio to connect.")
        XCTAssertTrue(older.serverUpdateRequired, "A missing protocol version is 1")
        XCTAssertFalse(T3OutdatedHosts.compatibility(["orchestrationProtocolVersion": 1, "capabilities": ["serverSelfUpdate": "desktop-managed"]], label: "S")!.serverUpdateRequired)
        XCTAssertTrue(T3OutdatedHosts.compatibility(["orchestrationProtocolVersion": 1, "capabilities": ["serverSelfUpdate": "desktop-managed", "desktopAppUpdate": true]], label: "S")!.serverUpdateRequired)
        XCTAssertFalse(T3OutdatedHosts.compatibility(["orchestrationProtocolVersion": 1, "capabilities": [:]], label: "S")!.serverUpdateRequired)
    }

    func testPairingSavesAnOutdatedSelfUpdatingHostSwitchedOff() throws {
        var capabilities: [String: Any] = ["serverSelfUpdate": "respawn"]
        var version = 1
        FleetHTTP.reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment":
                XCTAssertNil(request.value(forHTTPHeaderField: "x-t3-orchestration-protocol"), "The probe carries no protocol gate")
                return (200, ["environmentId": "old-env", "label": "Old box", "serverVersion": "0.0.30", "orchestrationProtocolVersion": version, "capabilities": capabilities])
            case "/oauth/token": return (200, ["token_type": "Bearer", "access_token": "bearer-old"])
            default: return (404, [:])
            }
        }
        let credentials = T3Credentials(persistent: false), saved = T3SavedEnvironments(persistent: false)
        let fleet = T3Fleet(persistent: false, credentials: credentials, saved: saved, configuration: configuration, changed: { _ in })
        defer { fleet.destroy() }
        let paired = fleetCall(fleet, "http://127.0.0.1:14901\n", ["op": "fleetOutdatedPair", "credential": "pair-code"])
        let value = paired["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(value["serverUpdateRequired"] as? Bool, true)
        XCTAssertEqual(value["message"] as? String, "This client requires a newer server. Update T3 Code on Old box to connect.")
        XCTAssertEqual(try credentials.read(origin: "http://127.0.0.1:14901", environment: "old-env"), "bearer-old")
        XCTAssertEqual(saved.all.first?["enabled"] as? Bool, false, "An outdated host is saved switched off")
        capabilities = [:]
        let manual = fleetCall(fleet, "http://127.0.0.1:14901\n", ["op": "fleetOutdatedPair", "credential": "pair-code"])
        XCTAssertEqual((manual["error"] as? [String: Any])?["message"] as? String, "This client requires a newer server. Update T3 Code on Old box to connect.")
        version = 3
        let newer = fleetCall(fleet, "http://127.0.0.1:14901\n", ["op": "fleetOutdatedPair", "credential": "pair-code"])
        XCTAssertEqual((newer["error"] as? [String: Any])?["message"] as? String, "This client is not supported by this server. Update your app or use a compatible release to connect to Old box.")
    }

    func testUpdateRunsOverABareSocketWithProgressThenWaitsForProtocolTwo() throws {
        let server = try OutdatedSocketServer()
        defer { server.stop() }
        let origin = "http://127.0.0.1:\(server.port)"
        let lock = NSLock()
        var updated = false, stages: [String] = []
        server.answer = { frame, send, _ in
            guard frame["_tag"] as? String == "Request", let id = frame["id"] else { return }
            XCTAssertEqual(frame["tag"] as? String, "server.updateServerWithProgress")
            XCTAssertEqual((frame["payload"] as? [String: Any])?["targetVersion"] as? String, "0.0.46")
            send(["_tag": "Chunk", "requestId": id, "values": [["type": "progress", "stage": "downloading"]]])
            send(["_tag": "Chunk", "requestId": id, "values": [["type": "progress", "stage": "installing"]]])
            send(["_tag": "Chunk", "requestId": id, "values": [["type": "complete", "result": ["targetVersion": "0.0.46", "method": "respawn"]]]])
            send(["_tag": "Exit", "requestId": id, "exit": ["_tag": "Success", "value": NSNull()]])
            lock.lock(); updated = true; lock.unlock()
        }
        FleetHTTP.reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment":
                lock.lock(); let done = updated; lock.unlock()
                return (200, ["environmentId": "old-env", "label": "Old box", "serverVersion": done ? "0.0.46" : "0.0.30", "orchestrationProtocolVersion": done ? 2 : 1,
                              "capabilities": ["serverSelfUpdate": "respawn", "serverSelfUpdateProgress": true]])
            case "/api/auth/websocket-ticket":
                XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer bearer-old")
                return (200, ["ticket": "ticket-1"])
            default: return (404, [:])
            }
        }
        let credentials = T3Credentials(persistent: false), saved = T3SavedEnvironments(persistent: false)
        try credentials.save("bearer-old", origin: origin, environment: "old-env")
        saved.remember(origin: origin, descriptor: ["environmentId": "old-env", "label": "Old box"])
        saved.setEnabled(origin: origin, environment: "old-env", enabled: false)
        let fleet = T3Fleet(persistent: false, credentials: credentials, saved: saved, configuration: configuration, changed: { _ in })
        defer { fleet.destroy() }
        fleet.outdated.pollInterval = 0.05
        let key = "\(origin)\nold-env"
        let started = fleetCall(fleet, key, ["op": "fleetOutdatedUpdate", "targetVersion": "0.0.46", "label": "Old box"])
        XCTAssertEqual((started["value"] as? [String: Any])?["started"] as? Bool, true)
        XCTAssertEqual((fleetCall(fleet, key, ["op": "fleetOutdatedUpdate", "targetVersion": "0.0.46"])["value"] as? [String: Any])?["started"] as? Bool, false, "One update per environment at a time")
        XCTAssertTrue(waitFor(5) {
            let job = self.jobs(fleet)[key] ?? [:]
            if let stage = job["stage"] as? String, stages.last != stage { stages.append(stage) }
            return job["status"] as? String == "done"
        }, "\(jobs(fleet))")
        let job = jobs(fleet)[key] ?? [:]
        XCTAssertEqual(job["resultVersion"] as? String, "0.0.46")
        XCTAssertEqual(job["fromVersion"] as? String, "0.0.30")
        XCTAssertTrue(stages.contains("resuming"))
        XCTAssertEqual(saved.all.first?["enabled"] as? Bool, true, "A host back on protocol 2 is switched on again")
        XCTAssertEqual(server.requests.count, 1)
        XCTAssertTrue(server.frames.contains { $0["_tag"] as? String == "Ack" }, "Progress chunks are acknowledged")
        XCTAssertEqual((server.requests.first?["headers"] as? [Any])?.count, 0, "The bare socket sends no protocol header")
        _ = fleetCall(fleet, key, ["op": "fleetOutdatedAck"])
        XCTAssertNil(jobs(fleet)[key])
    }

    func testDroppedLegacyUpdateSucceedsButANonUpdatableOrSilentHostFails() throws {
        let server = try OutdatedSocketServer()
        defer { server.stop() }
        let origin = "http://127.0.0.1:\(server.port)"
        var capabilities: [String: Any] = ["serverSelfUpdate": "boot-service"]
        server.answer = { frame, _, close in if frame["_tag"] as? String == "Request" { close() } }
        FleetHTTP.reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment": return (200, ["environmentId": "old-env", "label": "Old box", "orchestrationProtocolVersion": 1, "capabilities": capabilities])
            case "/api/auth/websocket-ticket": return (200, ["ticket": "ticket-1"])
            default: return (404, [:])
            }
        }
        let credentials = T3Credentials(persistent: false), saved = T3SavedEnvironments(persistent: false)
        try credentials.save("bearer-old", origin: origin, environment: "old-env")
        let fleet = T3Fleet(persistent: false, credentials: credentials, saved: saved, configuration: configuration, changed: { _ in })
        defer { fleet.destroy() }
        fleet.outdated.pollInterval = 0.05; fleet.outdated.restartTimeout = 0.4
        let key = "\(origin)\nold-env"
        _ = fleetCall(fleet, key, ["op": "fleetOutdatedUpdate", "targetVersion": "0.0.46"])
        XCTAssertTrue(waitFor(5) { self.jobs(fleet)[key]?["status"] as? String == "failed" })
        XCTAssertEqual(jobs(fleet)[key]?["message"] as? String, "Old box did not come back on a compatible T3 Code version.",
                       "A dropped boot-service socket counts as the restart; the descriptor never reached protocol 2")
        XCTAssertEqual(server.requests.first?["tag"] as? String, "server.updateServer")
        capabilities = [:]
        _ = fleetCall(fleet, key, ["op": "fleetOutdatedUpdate", "targetVersion": "0.0.46"])
        XCTAssertTrue(waitFor(3) { self.jobs(fleet)[key]?["message"] as? String == "Update T3 Code on Old box manually; it cannot update itself." })
    }

    /// Task server-update-banner: a connected server older than this client runs the same job
    /// (mode "connected"): its config's capabilities and continuation flag travel with the request,
    /// the restart ends when the descriptor reports the target version, and its switch stays on.
    func testConnectedServerUpdateWaitsForTargetVersionAndKeepsItsSwitch() throws {
        let server = try OutdatedSocketServer()
        defer { server.stop() }
        let origin = "http://127.0.0.1:\(server.port)"
        let lock = NSLock()
        var updated = false, fail = false, payloads: [[String: Any]] = []
        server.answer = { frame, send, _ in
            guard frame["_tag"] as? String == "Request", let id = frame["id"] else { return }
            XCTAssertEqual(frame["tag"] as? String, "server.updateServerWithProgress")
            lock.lock(); payloads.append(frame["payload"] as? [String: Any] ?? [:]); let failing = fail; lock.unlock()
            send(["_tag": "Chunk", "requestId": id, "values": [["type": "progress", "stage": "downloading"]]])
            if failing {
                send(["_tag": "Exit", "requestId": id, "exit": ["_tag": "Failure", "cause": [["_tag": "Fail", "error": ["_tag": "ServerSelfUpdateError", "reason": "The package could not be verified."]]]]])
                return
            }
            send(["_tag": "Chunk", "requestId": id, "values": [["type": "progress", "stage": "installing"]]])
            send(["_tag": "Chunk", "requestId": id, "values": [["type": "complete", "result": ["targetVersion": "0.0.46", "method": "respawn"]]]])
            send(["_tag": "Exit", "requestId": id, "exit": ["_tag": "Success", "value": NSNull()]])
            lock.lock(); updated = true; lock.unlock()
        }
        FleetHTTP.reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment":
                lock.lock(); let done = updated; lock.unlock()
                // A connected server already speaks protocol 2; only its version tells the restart apart.
                return (200, ["environmentId": "env", "label": "Studio", "serverVersion": done ? "0.0.46" : "0.0.45", "orchestrationProtocolVersion": 2, "capabilities": [:]])
            case "/api/auth/websocket-ticket": return (200, ["ticket": "ticket-1"])
            default: return (404, [:])
            }
        }
        let credentials = T3Credentials(persistent: false), saved = T3SavedEnvironments(persistent: false)
        try credentials.save("bearer", origin: origin, environment: "env")
        saved.remember(origin: origin, descriptor: ["environmentId": "env", "label": "Studio"])
        let fleet = T3Fleet(persistent: false, credentials: credentials, saved: saved, configuration: configuration, changed: { _ in })
        defer { fleet.destroy() }
        fleet.outdated.pollInterval = 0.05
        let key = "\(origin)\nenv"
        let request: [String: Any] = ["op": "fleetOutdatedUpdate", "mode": "connected", "targetVersion": "0.0.46", "fromVersion": "0.0.45", "label": "Studio",
                                      "capabilities": ["serverSelfUpdate": "respawn", "serverSelfUpdateProgress": true], "extras": ["continueRunningThreads": true, "ignored": 1]]
        lock.lock(); fail = true; lock.unlock()
        let first = fleetCall(fleet, key, request)["value"] as? [String: Any] ?? [:]
        XCTAssertTrue(waitFor(5) { self.jobs(fleet)[key]?["status"] as? String == "failed" })
        XCTAssertEqual(jobs(fleet)[key]?["message"] as? String, "Server update failed: The package could not be verified.")
        XCTAssertEqual(jobs(fleet)[key]?["stage"] as? String, "downloading", "A failure keeps the stage it reached")
        lock.lock(); fail = false; lock.unlock()
        let second = fleetCall(fleet, key, request)["value"] as? [String: Any] ?? [:]
        XCTAssertNotEqual(first["attempt"] as? String, second["attempt"] as? String, "Each attempt has its own identity")
        XCTAssertTrue(waitFor(5) { self.jobs(fleet)[key]?["status"] as? String == "done" }, "\(jobs(fleet))")
        let job = jobs(fleet)[key] ?? [:]
        XCTAssertEqual(job["mode"] as? String, "connected")
        XCTAssertEqual(job["resultVersion"] as? String, "0.0.46")
        XCTAssertEqual(job["fromVersion"] as? String, "0.0.45")
        lock.lock(); let sent = payloads.last ?? [:]; lock.unlock()
        XCTAssertEqual(sent["continueRunningThreads"] as? Bool, true)
        XCTAssertNil(sent["ignored"], "Only the continuation flag travels with the update")
        XCTAssertEqual(saved.all.first?["enabled"] as? Bool, true, "A connected server's switch is left alone")
    }
}

let suite = XCTestSuite(name: "T3 fleet")
suite.addTest(FleetTests.defaultTestSuite)
suite.addTest(OutdatedHostTests.defaultTestSuite)
suite.run()
guard let run = suite.testRun, run.executionCount == 9 else { exit(1) }
print("T3 fleet: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)

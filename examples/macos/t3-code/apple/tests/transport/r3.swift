import Foundation
import Network
import XCTest

/// HTTP for the fixture origin (descriptor, OAuth, session, socket ticket).
final class R3HTTP: URLProtocol, @unchecked Sendable {
    static let lock = NSLock()
    static var handler: (URLRequest) -> (Int, Any) = { _ in (500, [:]) }
    static var paths: [String] = []
    override class func canInit(with request: URLRequest) -> Bool { request.url?.scheme == "http" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        Self.lock.lock(); Self.paths.append(request.url!.path); let response = Self.handler(request); Self.lock.unlock()
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: response.0, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: response.1))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
    static func reset(_ handler: @escaping (URLRequest) -> (Int, Any)) { lock.lock(); paths = []; self.handler = handler; lock.unlock() }
    static func count(_ path: String) -> Int { lock.lock(); defer { lock.unlock() }; return paths.filter { $0 == path }.count }
    /// A healthy protocol-2 server; `descriptor` extends its descriptor.
    static func healthy(_ descriptor: [String: Any] = [:]) {
        reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment":
                return (200, ["environmentId": "fixture", "label": "Fixture", "orchestrationProtocolVersion": 2, "capabilities": ["connectionProbe": true]].merging(descriptor) { $1 })
            case "/oauth/token": return (200, ["access_token": "fixture-access", "token_type": "Bearer"])
            case "/api/auth/session": return (200, ["authenticated": true, "scopes": ["orchestration:read", "orchestration:operate"]])
            case "/api/auth/websocket-ticket": return (200, ["ticket": "fixture-ticket"])
            default: return (404, [:])
            }
        }
    }
}

/// A WebSocket peer speaking the Effect RPC frames the transport sends.
final class R3Socket: @unchecked Sendable {
    let listener: NWListener
    let queue = DispatchQueue(label: "r3.socket")
    private let lock = NSLock()
    private var connections: [NWConnection] = []
    private var frames: [[String: Any]] = []
    /// Answers one request frame; nil leaves it unanswered.
    var answer: ([String: Any]) -> [[String: Any]]? = { request in
        [["_tag": "Exit", "requestId": request["id"]!, "exit": ["_tag": "Success", "value": [:]]]]
    }
    var port: UInt16 { listener.port?.rawValue ?? 0 }

    init() throws {
        let parameters = NWParameters.tcp
        let options = NWProtocolWebSocket.Options()
        options.autoReplyPing = true
        let captured = self.captured
        options.setClientRequestHandler(DispatchQueue(label: "r3.socket.handshake")) { _, additional in
            captured.add(additional)
            return NWProtocolWebSocket.Response(status: .accept, subprotocol: nil)
        }
        parameters.defaultProtocolStack.applicationProtocols.insert(options, at: 0)
        listener = try NWListener(using: parameters, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in
            guard let self else { return }
            self.lock.lock(); self.connections.append(connection); self.lock.unlock()
            connection.start(queue: self.queue)
            self.receive(connection)
        }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
    }
    final class Box: @unchecked Sendable {
        private let lock = NSLock(); private var all: [[(name: String, value: String)]] = []
        func add(_ value: [(name: String, value: String)]) { lock.lock(); all.append(value); lock.unlock() }
        var handshakes: [[(name: String, value: String)]] { lock.lock(); defer { lock.unlock() }; return all }
    }
    private let captured = Box()
    private func receive(_ connection: NWConnection) {
        connection.receiveMessage { [weak self] data, _, _, error in
            guard let self, error == nil else { return }
            if let data, let object = try? JSONSerialization.jsonObject(with: data) {
                for frame in (object as? [[String: Any]]) ?? [(object as? [String: Any]) ?? [:]] {
                    self.lock.lock(); self.frames.append(frame); self.lock.unlock()
                    if frame["_tag"] as? String == "Request", let replies = self.answer(frame) { for reply in replies { self.send(reply, on: connection) } }
                }
            }
            self.receive(connection)
        }
    }
    func send(_ frame: [String: Any], on connection: NWConnection? = nil) {
        lock.lock(); let target = connection ?? connections.last; lock.unlock()
        guard let target, let data = try? JSONSerialization.data(withJSONObject: frame) else { return }
        let metadata = NWProtocolWebSocket.Metadata(opcode: .text)
        target.send(content: data, contentContext: NWConnection.ContentContext(identifier: "frame", metadata: [metadata]), isComplete: true, completion: .idempotent)
    }
    func requests(_ method: String) -> [[String: Any]] { lock.lock(); defer { lock.unlock() }; return frames.filter { $0["_tag"] as? String == "Request" && $0["tag"] as? String == method } }
    var connectionCount: Int { lock.lock(); defer { lock.unlock() }; return connections.count }
    /// A header of the `index`th WebSocket handshake, and the count of handshakes seen.
    func header(_ name: String, connection index: Int) -> String? {
        let all = captured.handshakes
        guard all.indices.contains(index) else { return nil }
        return all[index].first { $0.name.lowercased() == name.lowercased() }?.value
    }
    var handshakes: Int { captured.handshakes.count }
    func dropAll() { lock.lock(); let all = connections; lock.unlock(); for connection in all { connection.cancel() } }
    deinit { listener.cancel(); dropAll() }
}

// Lane r3-protocol: the read gate (T3ReadGate.swift), the reconnect policy and
// same-session stream retries (T3Transport.swift).
final class R3TransportTests: XCTestCase {
    private final class Topics: @unchecked Sendable {
        private let lock = NSLock(); private var list: [String] = []
        func add(_ topic: String) { lock.lock(); list.append(topic); lock.unlock() }
        var all: [String] { lock.lock(); defer { lock.unlock() }; return list }
        func clear() { lock.lock(); list.removeAll(); lock.unlock() }
    }
    private func wait(_ seconds: TimeInterval) { RunLoop.current.run(until: Date().addingTimeInterval(seconds)) }
    private func read(_ gate: T3ReadGate, _ op: String, reader: Int, session: String = "s") -> Bool {
        gate.began(["op": op, "reader": reader, "readerSession": session], answer: { _ in })
    }

    // MARK: Reconnect policy (9333509)

    private func transport(random: Double = 0, topics: Topics? = nil) -> T3Transport {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [R3HTTP.self]
        return T3Transport(persistent: false, configuration: config, random: { random }, signals: false, changed: { topics?.add($0) })
    }
    private func perform(_ transport: T3Transport, _ request: [String: Any], timeout: TimeInterval = 5) -> [String: Any] {
        let done = DispatchSemaphore(value: 0)
        var result: [String: Any] = [:]
        transport.perform(request) { result = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + timeout), .success, "\(request["op"] ?? "") did not answer")
        return result
    }
    private func status(_ transport: T3Transport) -> [String: Any] { perform(transport, ["op": "status"])["value"] as? [String: Any] ?? [:] }
    private func until(_ seconds: TimeInterval, _ condition: () -> Bool) -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end { if condition() { return true }; wait(0.02) }
        return condition()
    }
    private func connected(_ socket: R3Socket, random: Double = 0, descriptor: [String: Any] = [:]) -> T3Transport {
        R3HTTP.healthy(descriptor)
        let transport = transport(random: random)
        let opened = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(socket.port)", "credential": "pairing"])
        XCTAssertEqual((opened["value"] as? [String: Any])?["state"] as? String, "connected", "\(opened)")
        return transport
    }

    func testRetryDelayIsAJitteredUpperHalfCappedAtFiveMinutes() {
        XCTAssertEqual(T3Reconnect.delay(failureCount: 0, random: 0), 1)
        XCTAssertEqual(T3Reconnect.delay(failureCount: 0, random: 0.999_999), 2)
        XCTAssertEqual(T3Reconnect.delay(failureCount: 1, random: 0.5), 3)
        XCTAssertEqual(T3Reconnect.delay(failureCount: 2, random: 0), 4)
        XCTAssertEqual(T3Reconnect.delay(failureCount: 7, random: 0), 128)
        XCTAssertEqual(T3Reconnect.delay(failureCount: 8, random: 0), 150)
        XCTAssertEqual(T3Reconnect.delay(failureCount: 8, random: 0.999_999), 300)
        XCTAssertEqual(T3Reconnect.delay(failureCount: 40, random: 0.5), 225)
        XCTAssertEqual(T3Reconnect.streamDelay(retries: 0), 0.25)
        XCTAssertEqual(T3Reconnect.streamDelay(retries: 3), 2)
        XCTAssertEqual(T3Reconnect.streamDelay(retries: 9), 30)
        XCTAssertFalse(T3Reconnect.retriesStream(after: "EnvironmentAuthorizationError"))
        XCTAssertTrue(T3Reconnect.retriesStream(after: "OrchestrationV2GetThreadProjectionError"))
    }

    func testALostSocketBacksOffWithJitterAndAnExplicitRetryRunsNowFromTheFirstRung() throws {
        let socket = try R3Socket()
        let transport = connected(socket); defer { transport.destroy() }
        R3HTTP.reset { _ in (503, ["message": "Starting up."]) }
        socket.dropAll()
        XCTAssertTrue(until(3) { (status(transport)["message"] as? String ?? "").hasSuffix("Reconnecting in 1s…") }, "\(status(transport))")
        XCTAssertEqual(status(transport)["state"] as? String, "reconnecting")
        // The 1 s rung elapses, the descriptor fails again, and the next rung is 2–4 s.
        XCTAssertTrue(until(3) { status(transport)["message"] as? String == "Starting up. Reconnecting in 2s…" }, "\(status(transport))")
        let attempts = R3HTTP.count("/.well-known/t3/environment")
        let retried = perform(transport, ["op": "retry", "origin": "http://127.0.0.1:\(socket.port)"])
        XCTAssertEqual((retried["error"] as? [String: Any])?["kind"] as? String, "HTTP", "The waiting attempt ran at once and failed.")
        XCTAssertEqual(R3HTTP.count("/.well-known/t3/environment"), attempts + 1)
        XCTAssertEqual(status(transport)["message"] as? String, "Starting up. Reconnecting in 1s…", "An explicit retry restarts the ladder.")
    }

    func testAnExplicitRetryProbesTheLiveSocketInsteadOfReplacingIt() throws {
        let socket = try R3Socket()
        let transport = connected(socket); defer { transport.destroy() }
        let generation = perform(transport, ["op": "status"])["generation"] as? Int
        let retried = perform(transport, ["op": "retry", "origin": "http://127.0.0.1:\(socket.port)"])
        XCTAssertEqual((retried["value"] as? [String: Any])?["state"] as? String, "connected")
        XCTAssertEqual(retried["generation"] as? Int, generation, "A healthy socket is kept.")
        XCTAssertEqual(socket.requests("server.probe").count, 1)
        XCTAssertEqual(socket.connectionCount, 1)
        XCTAssertEqual(R3HTTP.count("/.well-known/t3/environment"), 1)
        // Before connectionProbe, the probe is server.getConfig.
        let older = try R3Socket()
        let legacy = connected(older, descriptor: ["capabilities": [:]]); defer { legacy.destroy() }
        _ = perform(legacy, ["op": "retry"])
        XCTAssertEqual(older.requests("server.getConfig").count, 1)
        XCTAssertEqual(older.requests("server.probe").count, 0)
    }

    func testAnUnansweredProbeReconnectsWithoutTheBackoffWait() throws {
        let socket = try R3Socket()
        socket.answer = { request in request["tag"] as? String == "server.probe" ? nil : [["_tag": "Exit", "requestId": request["id"]!, "exit": ["_tag": "Success", "value": [:]]]] }
        let transport = connected(socket); defer { transport.destroy() }
        let started = Date()
        let retried = perform(transport, ["op": "retry"], timeout: 6)
        XCTAssertGreaterThanOrEqual(Date().timeIntervalSince(started), 2.9, "An explicit retry waits 3 s for the probe.")
        XCTAssertEqual((retried["value"] as? [String: Any])?["message"] as? String, "Fixture did not respond to a connection health check. Reconnecting in 0s…")
        // No backoff rung: the replacement connects straight away.
        XCTAssertTrue(until(3) { status(transport)["state"] as? String == "connected" }, "\(status(transport))")
        XCTAssertEqual(socket.connectionCount, 2)
        XCTAssertEqual(R3HTTP.count("/.well-known/t3/environment"), 2)
    }

    func testForegroundAndNetworkSignalsProbeInsteadOfTearingDown() throws {
        let socket = try R3Socket()
        socket.answer = { request in request["tag"] as? String == "orchestration.subscribeShell" ? [] : [["_tag": "Exit", "requestId": request["id"]!, "exit": ["_tag": "Success", "value": [:]]]] }
        let transport = connected(socket); defer { transport.destroy() }
        let generation = perform(transport, ["op": "status"])["generation"] as! Int
        let shell = (perform(transport, ["op": "subscribe", "key": "shell", "method": "orchestration.subscribeShell", "payload": [:], "generation": generation])["value"] as? [String: Any])?["id"] as? String
        transport.applicationActive()
        XCTAssertTrue(until(2) { socket.requests("server.probe").count == 1 })
        // Returning to the app also resubscribes the shell and thread streams on this session.
        XCTAssertTrue(until(1) { self.events(transport, generation: generation).contains { $0["subscriptionId"] as? String == shell && ($0["value"] as? [String: Any])?["_retryDue"] as? Bool == true } })
        XCTAssertFalse(events(transport, generation: generation).contains { ($0["value"] as? [String: Any])?["_streamEnded"] != nil }, "The stream itself stayed open.")
        transport.networkChanged(online: false)
        XCTAssertTrue(until(2) { socket.requests("server.probe").count == 2 })
        wait(0.2)
        XCTAssertEqual(status(transport)["state"] as? String, "connected")
        XCTAssertEqual(socket.connectionCount, 1)
        // Back online during a backoff wait: the attempt runs now.
        R3HTTP.reset { _ in (503, ["message": "Starting up."]) }
        socket.dropAll()
        XCTAssertTrue(until(3) { status(transport)["state"] as? String == "reconnecting" })
        R3HTTP.healthy()
        let before = R3HTTP.count("/.well-known/t3/environment")
        transport.networkChanged(online: true)
        XCTAssertTrue(until(0.8) { R3HTTP.count("/.well-known/t3/environment") > before }, "Online wakes the waiting retry.")
        XCTAssertTrue(until(3) { status(transport)["state"] as? String == "connected" })
    }

    // MARK: Same-session stream retries (c5a929e)

    private func events(_ transport: T3Transport, generation: Int) -> [[String: Any]] {
        (perform(transport, ["op": "events", "after": 0, "generation": generation])["value"] as? [String: Any])?["events"] as? [[String: Any]] ?? []
    }

    func testAFailedStreamIsRetriedOnTheSameSessionWithADoublingDelay() throws {
        let socket = try R3Socket()
        let failure: [String: Any] = ["_tag": "Failure", "cause": [["_tag": "Fail", "error": ["_tag": "OrchestrationV2ShellStreamError", "message": "Shell stream failed."]]]]
        socket.answer = { request in request["tag"] as? String == "orchestration.subscribeShell" ? [] : [["_tag": "Exit", "requestId": request["id"]!, "exit": ["_tag": "Success", "value": [:]]]] }
        let transport = connected(socket); defer { transport.destroy() }
        let generation = perform(transport, ["op": "status"])["generation"] as! Int
        func subscribe() -> String {
            let reply = perform(transport, ["op": "subscribe", "key": "shell", "method": "orchestration.subscribeShell", "payload": ["afterSequence": 3], "generation": generation])
            return (reply["value"] as? [String: Any])?["id"] as? String ?? ""
        }
        func fail(_ id: String) { XCTAssertTrue(until(2) { socket.requests("orchestration.subscribeShell").contains { $0["id"] as? String == id } }); socket.send(["_tag": "Exit", "requestId": id, "exit": failure]) }
        func values(_ id: String) -> [[String: Any]] { events(transport, generation: generation).filter { $0["subscriptionId"] as? String == id }.compactMap { $0["value"] as? [String: Any] } }
        let first = subscribe(); fail(first)
        XCTAssertTrue(until(2) { values(first).first?["retryAfterMs"] as? Int == 250 }, "\(values(first))")
        XCTAssertEqual((values(first).first?["_transportError"] as? [String: Any])?["kind"] as? String, "OrchestrationV2ShellStreamError")
        XCTAssertTrue(until(1) { values(first).contains { $0["_retryDue"] as? Bool == true } }, "The retry comes due after 250 ms.")
        let second = subscribe(); fail(second)
        XCTAssertTrue(until(2) { values(second).first?["retryAfterMs"] as? Int == 500 }, "Consecutive failures double the delay.")
        // A resubscribe before the retry is due cancels it; its first value resets the ladder.
        let third = subscribe()
        XCTAssertTrue(until(2) { socket.requests("orchestration.subscribeShell").contains { $0["id"] as? String == third } })
        socket.send(["_tag": "Chunk", "requestId": third, "values": [["kind": "synchronized"]]])
        XCTAssertTrue(until(2) { values(third).contains { $0["kind"] as? String == "synchronized" } })
        wait(0.6)
        XCTAssertFalse(values(second).contains { $0["_retryDue"] as? Bool == true }, "A replaced subscription's retry never comes due.")
        fail(third)
        XCTAssertTrue(until(2) { values(third).last?["retryAfterMs"] as? Int == 250 }, "\(values(third))")
        // Authorization failures wait for the next session.
        let fourth = subscribe()
        XCTAssertTrue(until(2) { socket.requests("orchestration.subscribeShell").contains { $0["id"] as? String == fourth } })
        socket.send(["_tag": "Exit", "requestId": fourth, "exit": ["_tag": "Failure", "cause": [["_tag": "Fail", "error": ["_tag": "EnvironmentAuthorizationError", "message": "The authenticated token is missing required scope: orchestration:read."]]]]])
        XCTAssertTrue(until(2) { !values(fourth).isEmpty })
        wait(0.4)
        XCTAssertNil(values(fourth).first?["retryAfterMs"])
        XCTAssertFalse(values(fourth).contains { $0["_retryDue"] as? Bool == true })
    }

    // MARK: Outdated servers (22e9d35)

    private func outdated(_ capabilities: [String: Any], version: Int = 1, label: String = "Old box") -> [String: Any] {
        ["environmentId": "fixture", "label": label, "serverVersion": "0.0.30", "orchestrationProtocolVersion": version, "capabilities": capabilities]
    }

    func testPairingAndConnectingNameTheProtocolDirection() throws {
        let credentials = T3Credentials(persistent: false), saved = T3SavedEnvironments(persistent: false)
        let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [R3HTTP.self]
        let transport = T3Transport(persistent: false, configuration: config, credentials: credentials, savedEnvironments: saved, signals: false, changed: { _ in })
        defer { transport.destroy() }
        func pair(_ descriptor: [String: Any]) -> [String: Any] {
            R3HTTP.reset { request in request.url!.path == "/oauth/token" ? (200, ["access_token": "old-access", "token_type": "Bearer"]) : (200, descriptor) }
            return perform(transport, ["op": "pairEnvironment", "origin": "http://127.0.0.1:4400", "credential": "pairing"])
        }
        // An older host is refused here (kind Protocol); T3Fleet's fleetOutdatedPair saves one that can update itself.
        let older = pair(outdated(["serverSelfUpdate": "boot-service"]))
        XCTAssertEqual((older["error"] as? [String: Any])?["kind"] as? String, "Protocol")
        XCTAssertEqual((older["error"] as? [String: Any])?["message"] as? String, "This client requires a newer server. Update T3 Code on Old box to connect.")
        XCTAssertEqual(R3HTTP.count("/oauth/token"), 0, "An incompatible host never receives the credential.")
        let newer = pair(outdated([:], version: 3, label: "New box"))
        XCTAssertEqual((newer["error"] as? [String: Any])?["message"] as? String, "This client is not supported by this server. Update your app or use a compatible release to connect to New box.")
        XCTAssertTrue(saved.all.isEmpty)
        // A focused connection names the direction too, and keeps the descriptor for the row.
        R3HTTP.reset { _ in (200, self.outdated(["serverSelfUpdate": "respawn"])) }
        let refused = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:4400", "credential": "pairing"])
        XCTAssertEqual((refused["error"] as? [String: Any])?["message"] as? String, "This client requires a newer server. Update T3 Code on Old box to connect.")
        XCTAssertEqual(status(transport)["failureKind"] as? String, "Protocol")
        XCTAssertEqual((status(transport)["descriptor"] as? [String: Any])?["orchestrationProtocolVersion"] as? Int, 1)
        XCTAssertNil(T3Compatibility.problem(["orchestrationProtocolVersion": 2]))
        XCTAssertEqual(T3Compatibility.problem(["label": "X", "capabilities": ["serverSelfUpdate": "desktop-managed", "desktopAppUpdate": true]])?.serverUpdateRequired, true)
        XCTAssertEqual(T3Compatibility.problem(["label": "X", "capabilities": ["serverSelfUpdate": "desktop-managed"]])?.serverUpdateRequired, false)
    }

    func testANormalSocketCarriesTheProtocolGate() throws {
        let socket = try R3Socket()
        let transport = connected(socket); defer { transport.destroy() }
        XCTAssertEqual(socket.handshakes, 1)
        XCTAssertEqual(socket.header("x-t3-orchestration-protocol", connection: 0), "2", "\(socket.handshakes)")
    }

    /// Against a real server (R3_LIVE_PAIRING_FILE: {"credential": pairing link}; R3_LIVE_ORIGIN):
    /// Retry on a live socket is answered by server.probe and keeps the session.
    func testLiveRetryProbesAHeadServerAndKeepsTheSession() throws {
        guard let path = ProcessInfo.processInfo.environment["R3_LIVE_PAIRING_FILE"] else { throw XCTSkip("Set R3_LIVE_PAIRING_FILE to a disposable credential.") }
        let pairing = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: path))) as! [String: Any]
        let origin = ProcessInfo.processInfo.environment["R3_LIVE_ORIGIN"] ?? "http://127.0.0.1:14828"
        let transport = T3Transport(persistent: false, signals: false, changed: { _ in }); defer { transport.destroy() }
        let opened = perform(transport, ["op": "connect", "origin": origin, "credential": pairing["credential"] as? String ?? ""], timeout: 15)
        XCTAssertEqual((opened["value"] as? [String: Any])?["state"] as? String, "connected")
        let generation = opened["generation"] as? Int
        let started = Date()
        let retried = perform(transport, ["op": "retry", "origin": origin])
        XCTAssertEqual((retried["value"] as? [String: Any])?["state"] as? String, "connected")
        XCTAssertEqual(retried["generation"] as? Int, generation, "The live socket is kept.")
        XCTAssertLessThan(Date().timeIntervalSince(started), 1, "A HEAD server answers server.probe at once.")
        transport.applicationActive(); wait(0.3)
        XCTAssertEqual(perform(transport, ["op": "status"])["generation"] as? Int, generation)
    }

    func testGateHoldsTheReadsTopicsUntilItsLastReplyThenReplaysOnce() {
        let topics = Topics()
        let gate = T3ReadGate(changed: topics.add, idle: 5, settle: 0.01)
        XCTAssertFalse(read(gate, "status", reader: 1))
        XCTAssertTrue(gate.reading)
        let http: [String: Any] = ["op": "http", "reader": 1, "readerSession": "s"]
        gate.sent(http)
        gate.changed("t3.events"); gate.changed("t3.status"); gate.changed("t3.events"); gate.changed("t3.editor")
        XCTAssertEqual(topics.all, ["t3.editor"], "Only the snapshot read's topics are held.")
        var answered = false
        XCTAssertTrue(gate.began(["op": "readEnd", "reader": 1, "readerSession": "s", "generation": 4], answer: { reply in
            answered = reply["ok"] as? Bool == true && reply["generation"] as? Int == 4 }))
        XCTAssertTrue(answered)
        wait(0.05)
        XCTAssertEqual(topics.all, ["t3.editor"], "readEnd waits for the read's network replies.")
        gate.answered(http)
        wait(0.08)
        XCTAssertFalse(gate.reading)
        XCTAssertEqual(topics.all, ["t3.editor", "t3.events", "t3.status"], "Held topics replay once, after the last reply.")
        gate.changed("t3.status")
        XCTAssertEqual(topics.all.last, "t3.status", "An idle gate forwards at once.")
    }

    func testTheDrawAfterAReadIsCoalescedIntoOneReplay() {
        let topics = Topics()
        let gate = T3ReadGate(changed: topics.add, idle: 5, settle: 0.15, quietCap: 0.4)
        _ = read(gate, "status", reader: 1)
        _ = read(gate, "readEnd", reader: 1)
        // Hooks mounted by the drawn result report back during the quiet window.
        wait(0.05); gate.changed("t3.status")
        wait(0.1); gate.changed("t3.status"); gate.changed("t3.editor")
        XCTAssertEqual(topics.all, ["t3.editor"], "The quiet window holds only the read's topics.")
        wait(0.1)
        XCTAssertEqual(topics.all, ["t3.editor"], "Each held topic extends the window.")
        wait(0.35)
        XCTAssertEqual(topics.all, ["t3.editor", "t3.status"], "The burst replays once, within the cap.")
        gate.changed("t3.events")
        XCTAssertEqual(topics.all.last, "t3.events")
    }

    func testAnIdleBurstAsksOnceAtOnceAndOnceAfter() {
        let topics = Topics()
        let gate = T3ReadGate(changed: topics.add, idle: 5, settle: 0.1, quietCap: 0.3)
        gate.changed("t3.status"); gate.changed("t3.events"); gate.changed("t3.status")
        XCTAssertEqual(topics.all, ["t3.status"], "The first change asks at once; the rest of the burst waits.")
        wait(0.25)
        XCTAssertEqual(topics.all, ["t3.status", "t3.events", "t3.status"], "The burst replays once.")
        // A read that began meanwhile keeps the burst until it ends.
        topics.clear()
        gate.changed("t3.status"); _ = read(gate, "status", reader: 9); gate.changed("t3.events")
        wait(0.25)
        XCTAssertEqual(topics.all, ["t3.status"])
        _ = read(gate, "readEnd", reader: 9)
        wait(0.25)
        XCTAssertEqual(topics.all, ["t3.status", "t3.events"])
    }

    func testANewerReadSupersedesAndStragglersNeverRelease() {
        let topics = Topics()
        let gate = T3ReadGate(changed: topics.add, idle: 5, settle: 0.01)
        _ = read(gate, "status", reader: 1)
        _ = read(gate, "status", reader: 2)
        gate.changed("t3.events")
        _ = read(gate, "readEnd", reader: 1)
        _ = read(gate, "events", reader: 1)
        wait(0.05)
        XCTAssertTrue(gate.reading); XCTAssertEqual(topics.all, [])
        _ = read(gate, "readEnd", reader: 2)
        wait(0.05)
        XCTAssertEqual(topics.all, ["t3.events"])
        _ = read(gate, "status", reader: 2)
        XCTAssertFalse(gate.reading, "An ended read's late request opens nothing.")
        // A reloaded module restarts its numbering under a new session.
        _ = read(gate, "status", reader: 1, session: "reloaded")
        XCTAssertTrue(gate.reading)
        XCTAssertTrue(gate.began(["op": "readEnd"], answer: { _ in }), "An untagged readEnd is still answered.")
    }

    func testAReadExactLetGoIsReleasedAndAskedAgain() {
        let topics = Topics()
        let gate = T3ReadGate(changed: topics.add, idle: 0.3, settle: 0.01)
        _ = read(gate, "status", reader: 5)
        let pending: [String: Any] = ["op": "request", "reader": 5, "readerSession": "s"]
        gate.sent(pending)
        wait(0.6)
        XCTAssertTrue(gate.reading, "A read waiting on the network is not stalled.")
        gate.answered(pending)
        wait(0.8)
        XCTAssertFalse(gate.reading)
        XCTAssertEqual(topics.all, ["t3.status"], "A stalled read is asked again even when nothing was held.")
    }
}

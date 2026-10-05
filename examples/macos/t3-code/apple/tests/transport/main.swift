import Foundation
import XCTest

// Standalone tests compile these production files directly: T3Protocol.swift,
// T3Credentials.swift, T3Transport.swift. URLProtocol isolates HTTP from the network.
private final class HTTPFixture: URLProtocol, @unchecked Sendable {
    static let lock = NSLock()
    static var requests: [URLRequest] = []
    static var handler: (URLRequest) -> (Int, Any) = { _ in (500, [:]) }
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        Self.lock.lock(); Self.requests.append(request); let response = Self.handler(request); Self.lock.unlock()
        let url = request.url!
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: url, statusCode: response.0, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: response.1))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
    static func reset(_ handler: @escaping (URLRequest) -> (Int, Any)) {
        lock.lock(); requests = []; self.handler = handler; lock.unlock()
    }
    static var count: Int { lock.lock(); defer { lock.unlock() }; return requests.count }
}

private func call(_ transport: T3Transport, _ request: [String: Any], file: StaticString = #filePath, line: UInt = #line) -> [String: Any] {
    let ready = DispatchSemaphore(value: 0)
    var result: [String: Any] = [:]
    transport.perform(request) { result = $0; ready.signal() }
    XCTAssertEqual(ready.wait(timeout: .now() + 3), .success, file: file, line: line)
    return result
}

final class TransportTests: XCTestCase {
    func makeTransport() -> T3Transport {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [HTTPFixture.self]
        return T3Transport(persistent: false, configuration: config, changed: { _ in })
    }

    func testNoNetworkBeforeExplicitConnectAndUUIDsAreNative() {
        HTTPFixture.reset { _ in XCTFail("Status must not connect"); return (500, [:]) }
        let transport = makeTransport(); defer { transport.destroy() }
        let status = call(transport, ["op": "status"])
        XCTAssertEqual(status["ok"] as? Bool, true)
        XCTAssertEqual((status["value"] as? [String: Any])?["state"] as? String, "disconnected")
        let ids = call(transport, ["op": "ids", "count": 3])["value"] as? [String] ?? []
        XCTAssertEqual(ids.count, 3)
        XCTAssertEqual(Set(ids).count, 3)
        XCTAssertTrue(ids.allSatisfy { UUID(uuidString: $0) != nil })
        XCTAssertEqual(HTTPFixture.count, 0)
    }

    func testPreferencesAreLazyAtomicAndSurviveANewModule() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("exact-t3-preferences-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: directory) }
        let transport = T3Transport(persistent: false, dataDirectory: directory, changed: { _ in })
        defer { transport.destroy() }
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.path))
        let missing = call(transport, ["op": "readPreferences"])
        XCTAssertEqual(missing["ok"] as? Bool, true)
        XCTAssertEqual((missing["value"] as? [String: Any])?["text"] as? String, "")
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.path))
        let original = "{\"version\":1,\"drafts\":{\"thread\":\"한😀\\nSecond line\"}}"
        XCTAssertEqual(call(transport, ["op": "writePreferences", "text": original])["ok"] as? Bool, true)
        let file = directory.appendingPathComponent("t3-code.json")
        XCTAssertEqual(try String(contentsOf: file, encoding: .utf8), original)
        let replacement = T3Transport(persistent: false, dataDirectory: directory, changed: { _ in })
        defer { replacement.destroy() }
        let restored = call(replacement, ["op": "readPreferences"])
        XCTAssertEqual((restored["value"] as? [String: Any])?["text"] as? String, original)
        let oversized = call(transport, ["op": "writePreferences", "text": String(repeating: "x", count: T3Wire.maximumBytes + 1)])
        XCTAssertEqual((oversized["error"] as? [String: Any])?["kind"] as? String, "Limit")
        XCTAssertEqual(try String(contentsOf: file, encoding: .utf8), original)
        let impossible = T3Transport(persistent: false, dataDirectory: file, changed: { _ in })
        defer { impossible.destroy() }
        let refused = call(impossible, ["op": "writePreferences", "text": original])
        XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "Persistence")
    }

    func testOAuthIsFormEncodedAndTokensNeverReturnToTheApp() {
        let pairing = "pair+&=secret", access = "bearer-test-secret"
        HTTPFixture.reset { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "x-t3-orchestration-protocol"), "2")
            switch request.url!.path {
            case "/.well-known/t3/environment":
                XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"))
                return (200, ["environmentId": "fixture", "label": "Fixture", "orchestrationProtocolVersion": 2])
            case "/oauth/token":
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-www-form-urlencoded")
                var bytes = request.httpBody ?? Data()
                if let stream = request.httpBodyStream {
                    stream.open(); defer { stream.close() }
                    var chunk = [UInt8](repeating: 0, count: 1024)
                    while stream.hasBytesAvailable { let n = stream.read(&chunk, maxLength: chunk.count); if n <= 0 { break }; bytes.append(contentsOf: chunk.prefix(n)) }
                }
                let body = String(decoding: bytes, as: UTF8.self)
                let fields = URLComponents(string: "http://fixture/?" + body)!.queryItems!
                XCTAssertEqual(fields.first { $0.name == "subject_token" }?.value, pairing)
                XCTAssertEqual(fields.first { $0.name == "scope" }?.value, "orchestration:read orchestration:operate")
                return (200, ["access_token": access, "token_type": "Bearer", "expires_in": 3600])
            case "/api/auth/websocket-ticket":
                XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer " + access)
                return (401, ["message": "Rejected " + access])
            case "/api/auth/session":
                XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer " + access)
                return (200, ["authenticated": true, "scopes": ["orchestration:read"]])
            default: XCTFail("Unexpected endpoint"); return (404, [:])
            }
        }
        let transport = makeTransport(); defer { transport.destroy() }
        let response = call(transport, ["op": "connect", "origin": "http://127.0.0.1:4318", "credential": pairing])
        XCTAssertEqual(response["ok"] as? Bool, false)
        XCTAssertEqual((response["error"] as? [String: Any])?["kind"] as? String, "Authentication")
        let status = call(transport, ["op": "status"])
        XCTAssertEqual((status["value"] as? [String: Any])?["state"] as? String, "error")
        for value in [response, status] {
            let text = String(decoding: try! JSONSerialization.data(withJSONObject: value), as: UTF8.self)
            XCTAssertFalse(text.contains(access)); XCTAssertFalse(text.contains(pairing))
        }
        XCTAssertEqual(HTTPFixture.count, 4)
    }

    func testProtocolMismatchStopsBeforeExchangingCredential() {
        HTTPFixture.reset { _ in (200, ["environmentId": "old", "orchestrationProtocolVersion": 1]) }
        let transport = makeTransport(); defer { transport.destroy() }
        let response = call(transport, ["op": "connect", "origin": "http://localhost:4318", "credential": "pairing"])
        XCTAssertEqual((response["error"] as? [String: Any])?["kind"] as? String, "Protocol")
        XCTAssertEqual(HTTPFixture.count, 1)
    }

    func testDisconnectedWritesFailWithoutSendingAndDestroyClosesBridge() {
        HTTPFixture.reset { _ in XCTFail("No request is allowed"); return (500, [:]) }
        let transport = makeTransport()
        for op in ["request", "subscribe", "http"] {
            let response = call(transport, ["op": op, "method": "write", "path": "/api/test", "key": "feed"])
            XCTAssertEqual(response["ok"] as? Bool, false)
            XCTAssertEqual((response["error"] as? [String: Any])?["uncertain"] as? Bool, false)
        }
        let stale = call(transport, ["op": "request", "method": "write", "generation": 99])
        XCTAssertEqual((stale["error"] as? [String: Any])?["kind"] as? String, "stale")
        transport.destroy()
        let response = call(transport, ["op": "status"])
        XCTAssertEqual((response["error"] as? [String: Any])?["kind"] as? String, "Closed")
        XCTAssertEqual(HTTPFixture.count, 0)
    }

    func testInboxReadIsReplayableAndACKIsExplicit() throws {
        var inbox = T3Inbox()
        for i in 0..<100 { try inbox.append(generation: 4, key: "thread", value: ["text": String(i)]) }
        let first = inbox.read(after: 0), repeated = inbox.read(after: 0)
        XCTAssertEqual((first["events"] as? [Any])?.count, 100)
        XCTAssertEqual(try JSONSerialization.data(withJSONObject: first, options: [.sortedKeys]), try JSONSerialization.data(withJSONObject: repeated, options: [.sortedKeys]))
        inbox.acknowledge(through: 70)
        XCTAssertEqual((inbox.read(after: 70)["events"] as? [Any])?.count, 30)
        XCTAssertEqual(inbox.read(after: 70)["reset"] as? Bool, false)
        XCTAssertEqual(inbox.read(after: 69)["reset"] as? Bool, true)
        let events = inbox.read(after: 70)["events"] as! [[String: Any]]
        XCTAssertEqual(events.first?["seq"] as? Int, 71)
        XCTAssertTrue(events.allSatisfy { $0["generation"] as? Int == 4 })
    }

    func testInboxOverflowAndReplacementRequireResnapshot() throws {
        var inbox = T3Inbox(limit: 2, byteLimit: 256)
        for i in 0..<3 { try inbox.append(generation: 1, key: "a", value: ["n": i]) }
        XCTAssertEqual(inbox.read(after: 0)["reset"] as? Bool, true)
        XCTAssertEqual(inbox.latest, 3)
        XCTAssertLessThanOrEqual(inbox.entries.count, 2)
        XCTAssertEqual(inbox.read(after: 99)["reset"] as? Bool, true)
        inbox.reset()
        XCTAssertEqual(inbox.read(after: 2)["reset"] as? Bool, true)
        try inbox.append(generation: 2, key: "b", value: ["text": String(repeating: "x", count: 400)])
        XCTAssertEqual(inbox.entries.count, 0)
        XCTAssertEqual(inbox.read(after: 3)["reset"] as? Bool, true)
    }

    func testInboxNotifiesOnceUntilAllRetainedWorkIsAcknowledged() throws {
        var inbox = T3Inbox(limit: 2, byteLimit: 256)
        XCTAssertTrue(try inbox.append(generation: 1, key: "config", value: 1))
        XCTAssertFalse(try inbox.append(generation: 1, key: "config", value: 2))
        _ = inbox.read(after: 0)
        inbox.acknowledge(through: 1)
        XCTAssertFalse(try inbox.append(generation: 1, key: "shell", value: 3))
        XCTAssertFalse(try inbox.append(generation: 1, key: "shell", value: 4))
        XCTAssertEqual(inbox.read(after: 0)["reset"] as? Bool, true)
        inbox.acknowledge(through: inbox.latest)
        XCTAssertTrue(try inbox.append(generation: 1, key: "thread", value: 5))
        inbox.reset()
        XCTAssertTrue(try inbox.append(generation: 2, key: "config", value: 6))
    }

    func testPagesAndLargeValuesStayBelowNativeReplyLimitWithoutLosingUnicode() throws {
        var inbox = T3Inbox()
        for i in 0..<20 { try inbox.append(generation: 1, key: "thread", subscriptionId: "sub", value: ["i": i, "text": String(repeating: "x", count: 100_000)]) }
        let page = inbox.read(after: 0)
        let events = page["events"] as! [[String: Any]]
        XCTAssertTrue(events.count < 20)
        XCTAssertTrue(try JSONSerialization.data(withJSONObject: page).count < 1024 * 1024)
        XCTAssertEqual(page["latest"] as? Int, 20)
        var transfers = T3Transfers()
        let value = ["text": String(repeating: "한😀\\\"\n", count: 120_000)]
        let prepared = try transfers.prepare(value) as! [String: Any]
        let ref = prepared["_nativeTransfer"] as! [String: Any], id = ref["id"] as! String
        var joined = ""
        for index in 0..<(ref["parts"] as! Int) {
            let piece = try transfers.read(id: id, index: index)
            XCTAssertTrue(try JSONSerialization.data(withJSONObject: piece).count < 512 * 1024)
            joined += piece["text"] as! String
        }
        XCTAssertEqual(try JSONSerialization.jsonObject(with: Data(joined.utf8)) as? [String: String], value)
        transfers.release(id)
        XCTAssertEqual(transfers.bytes, 0)
        XCTAssertThrowsError(try transfers.read(id: id, index: 0))
    }

    func testEffectWireUsesTaggedFramesAndPreservesEveryChunkValue() throws {
        let request = T3Wire.request(id: "2-1", method: "orchestration.subscribe", payload: ["threadId": "t"])
        XCTAssertEqual(request["_tag"] as? String, "Request")
        XCTAssertNil(request["jsonrpc"])
        let encoded = try T3Wire.encode(request)
        XCTAssertEqual(try T3Wire.decode(Data(encoded.utf8)).count, 1)
        let frames = try T3Wire.decode(Data("[{\"_tag\":\"Chunk\",\"requestId\":1,\"values\":[1,2,3]},{\"_tag\":\"Exit\",\"requestId\":1,\"exit\":{\"_tag\":\"Success\",\"value\":{}}}]".utf8))
        XCTAssertEqual(frames.count, 2)
        XCTAssertEqual((frames[0]["values"] as? [Int]), [1, 2, 3])
        XCTAssertEqual(T3Wire.identifier(frames[0]["requestId"]), "1")
        XCTAssertThrowsError(try T3Wire.decode(Data("{}".utf8)))
        XCTAssertThrowsError(try T3Wire.decode(Data("{bad".utf8)))
        let failure = T3Wire.failure(["_tag": "Failure", "cause": [["_tag": "Fail", "error": ["_tag": "Forbidden", "message": "Read only"]]]])
        XCTAssertEqual(failure.kind, "Forbidden"); XCTAssertEqual(failure.message, "Read only")
    }

    func testEndpointBoundaryAndPairingURL() throws {
        let origin = try T3Endpoint.origin("https://EXAMPLE.test/pair?token=secret")
        XCTAssertEqual(origin.absoluteString, "https://example.test")
        XCTAssertEqual(try T3Endpoint.credential("https://example.test/pair?token=a%2Bb", at: origin), "a+b")
        XCTAssertEqual(try T3Endpoint.credential("https://example.test/pair#token=a%2Bb", at: origin), "a+b")
        XCTAssertThrowsError(try T3Endpoint.credential("https://other.test/pair?token=secret", at: origin))
        XCTAssertThrowsError(try T3Endpoint.origin("http://remote.example"))
        XCTAssertThrowsError(try T3Endpoint.origin("https://user:password@example.test"))
        XCTAssertThrowsError(try T3Endpoint.path("//other.test/private", at: origin))
        XCTAssertThrowsError(try T3Endpoint.path("https://other.test/private", at: origin))
        XCTAssertEqual(try T3Endpoint.path("/api/read?limit=20", at: origin).host, "example.test")
        XCTAssertNotEqual(T3Credentials.account(origin: "http://localhost:3773", environment: "one"),
                          T3Credentials.account(origin: "http://localhost:3773", environment: "two"))
    }

    func testLiveT3AuthSocketHTTPAndSubscriptions() throws {
        guard let path = ProcessInfo.processInfo.environment["T3_TRANSPORT_PAIRING_FILE"] else {
            throw XCTSkip("Set T3_TRANSPORT_PAIRING_FILE to a disposable isolated server credential.")
        }
        let pairing = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: path))) as! [String: Any]
        let credential = pairing["credential"] as! String
        let url = ProcessInfo.processInfo.environment["T3_TRANSPORT_ORIGIN"] ?? "http://127.0.0.1:13773"
        let changedLock = NSLock()
        var eventInvalidations = 0
        let transport = T3Transport(persistent: false, changed: { topic in
            changedLock.lock(); defer { changedLock.unlock() }
            if topic == "t3.events" { eventInvalidations += 1 }
        })
        defer { transport.destroy() }
        let connected = call(transport, ["op": "connect", "origin": url, "credential": credential])
        XCTAssertEqual(connected["ok"] as? Bool, true, "Connection failed: \(connected["error"] ?? "none")")
        guard connected["ok"] as? Bool == true else { return }
        let generation = connected["generation"] as! Int
        let probe = call(transport, ["op": "request", "method": "server.probe", "payload": [:], "generation": generation])
        XCTAssertEqual(probe["ok"] as? Bool, true, "Unary RPC failed: \(probe["error"] ?? "none")")
        let http = call(transport, ["op": "http", "path": "/api/orchestration/shell", "generation": generation])
        XCTAssertEqual(http["ok"] as? Bool, true, "HTTP snapshot failed: \(http["error"] ?? "none")")
        for (key, method) in [("config", "subscribeServerConfig"), ("shell", "orchestration.subscribeShell")] {
            let subscribed = call(transport, ["op": "subscribe", "key": key, "method": method, "payload": [:], "generation": generation])
            XCTAssertEqual(subscribed["ok"] as? Bool, true)
        }
        var events: [[String: Any]] = []
        for _ in 0..<60 {
            let response = call(transport, ["op": "events", "after": 0, "generation": generation])
            events = (response["value"] as? [String: Any])?["events"] as? [[String: Any]] ?? []
            if Set(events.compactMap { $0["key"] as? String }).count == 2 { break }
            Thread.sleep(forTimeInterval: 0.05)
        }
        XCTAssertEqual(Set(events.compactMap { $0["key"] as? String }), ["config", "shell"])
        XCTAssertTrue(events.allSatisfy { ($0["subscriptionId"] as? String)?.isEmpty == false })
        for event in events { XCTAssertNil((event["value"] as? [String: Any])?["_transportError"]) }
        changedLock.lock(); let invalidations = eventInvalidations; changedLock.unlock()
        XCTAssertEqual(invalidations, 1, "Retained subscription snapshots must coalesce until application ACK.")
        let through = events.compactMap { $0["seq"] as? Int }.max() ?? 0
        let acknowledged = call(transport, ["op": "ack", "through": through, "generation": generation])
        XCTAssertEqual(acknowledged["ok"] as? Bool, true)
        XCTAssertGreaterThanOrEqual((acknowledged["value"] as? [String: Any])?["latest"] as? Int ?? -1, through)
        for key in ["config", "shell"] {
            XCTAssertEqual(call(transport, ["op": "unsubscribe", "key": key, "generation": generation])["ok"] as? Bool, true)
        }
        XCTAssertEqual(call(transport, ["op": "disconnect"])["ok"] as? Bool, true)
    }
}

let suite = TransportTests.defaultTestSuite
suite.run()
guard let run = suite.testRun, run.executionCount == 12 else { exit(1) }
print("T3 transport: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)

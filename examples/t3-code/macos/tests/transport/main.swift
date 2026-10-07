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
    func testAgentStorageHonorsExplicitScratchWithoutChangingLiveRoots() {
        let original = URL(fileURLWithPath: "/system/tmp/exact-agent-123-2/data")
        let scratch = URL(fileURLWithPath: ProcessInfo.processInfo.environment["TMPDIR"]!)
        let expected = scratch.appendingPathComponent("exact-agent-123-2/data")
        XCTAssertEqual(T3Storage.dataRoot(agent: true, contextData: original).path, expected.path)
        XCTAssertEqual(T3Storage.dataRoot(agent: false, contextData: original), original)
        XCTAssertEqual(T3Storage.dataRoot(agent: true, contextData: original, environment: ["TMPDIR": "relative"]), original)
        XCTAssertEqual(T3Storage.dataRoot(agent: true, contextData: original, environment: [:]), original)
    }

    func makeTransport() -> T3Transport {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [HTTPFixture.self]
        return T3Transport(persistent: false, configuration: config, changed: { _ in })
    }

    func testAgentCredentialsSurviveDisconnectButNeverAnotherInstance() throws {
        let cache = T3Credentials(persistent: false)
        try cache.save("disposable-test-token", origin: "http://fixture", environment: "A")
        XCTAssertEqual(try cache.read(origin: "http://fixture", environment: "A"), "disposable-test-token")
        XCTAssertNil(try cache.read(origin: "http://other", environment: "A"))
        XCTAssertNil(try cache.read(origin: "http://fixture", environment: "B"))
        XCTAssertNil(try T3Credentials(persistent: false).read(origin: "http://fixture", environment: "A"))
        try cache.forget(origin: "http://fixture", environment: "A")
        XCTAssertNil(try cache.read(origin: "http://fixture", environment: "A"))
    }

    func testSavedEnvironmentsRememberForgetAndStayPerInstance() throws {
        let saved = T3SavedEnvironments(persistent: false)
        saved.remember(origin: "http://127.0.0.1:1", descriptor: ["environmentId": "A", "label": "First", "platform": ["machine": "laptop"]])
        saved.remember(origin: "http://127.0.0.1:2", descriptor: ["environmentId": "B", "label": "Second"])
        saved.remember(origin: "http://127.0.0.1:1", descriptor: ["environmentId": "A", "label": "First again", "platform": ["machine": "desktop"]])
        XCTAssertEqual(saved.all.compactMap { $0["environmentId"] as? String }, ["A", "B"])
        XCTAssertEqual(saved.all.first?["label"] as? String, "First again")
        XCTAssertEqual(saved.all.first?["machine"] as? String, "desktop")
        XCTAssertNil(saved.all.first?["token"])
        saved.forget(origin: "http://127.0.0.1:2", environment: "B")
        XCTAssertEqual(saved.all.count, 1)
        XCTAssertEqual(T3SavedEnvironments(persistent: false).all.count, 0)
        HTTPFixture.reset { _ in XCTFail("Listing and forgetting saved environments must not connect"); return (500, [:]) }
        let transport = makeTransport(); defer { transport.destroy() }
        XCTAssertEqual(((call(transport, ["op": "environments"])["value"] as? [String: Any])?["saved"] as? [Any])?.count, 0)
        XCTAssertEqual(((call(transport, ["op": "forgetEnvironment", "origin": "ftp://x", "environmentId": "A"])["error"] as? [String: Any])?["kind"]) as? String, "Address")
        let forgot = call(transport, ["op": "forgetEnvironment", "origin": "http://127.0.0.1:9", "environmentId": "A"])
        XCTAssertEqual((forgot["value"] as? [String: Any])?["state"] as? String, "disconnected")
        XCTAssertEqual(HTTPFixture.count, 0)
    }

    func testPairingAnotherEnvironmentKeepsTheActiveConnectionAndSavesItsToken() throws {
        var exchanged = 0
        HTTPFixture.reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment":
                return (200, ["environmentId": "env-b", "label": "Fixture B", "orchestrationProtocolVersion": 2, "platform": ["machine": "laptop", "os": "darwin"]])
            case "/oauth/token":
                exchanged += 1
                return exchanged == 1 ? (401, ["_tag": "EnvironmentAuthInvalidError", "code": "auth_invalid", "reason": "invalid_credential"])
                    : (200, ["access_token": "bearer-b", "token_type": "Bearer"])
            default: XCTFail("Pairing must not open a socket or session: \(request.url!.path)"); return (500, [:])
            }
        }
        let transport = makeTransport(); defer { transport.destroy() }
        let refused = call(transport, ["op": "pairEnvironment", "origin": "http://127.0.0.1:14806", "credential": "invalid-fixture-code"])
        XCTAssertEqual((refused["error"] as? [String: Any])?["message"] as? String, "The environment credential is invalid.")
        XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "Authentication")
        XCTAssertFalse("\(refused)".contains("invalid-fixture-code"))
        let empty = call(transport, ["op": "pairEnvironment", "origin": "http://127.0.0.1:14806", "credential": ""])
        XCTAssertEqual((empty["error"] as? [String: Any])?["message"] as? String, "Enter a pairing code.")
        let paired = call(transport, ["op": "pairEnvironment", "origin": "http://127.0.0.1:14806/pair#token=PAIRCODE", "credential": "http://127.0.0.1:14806/pair#token=PAIRCODE"])
        XCTAssertEqual((paired["value"] as? [String: Any])?["environmentId"] as? String, "env-b")
        XCTAssertNil((paired["value"] as? [String: Any])?["access_token"])
        let saved = (call(transport, ["op": "environments"])["value"] as? [String: Any])?["saved"] as? [[String: Any]] ?? []
        XCTAssertEqual(saved.map { $0["label"] as? String }, ["Fixture B"])
        XCTAssertEqual((call(transport, ["op": "status"])["value"] as? [String: Any])?["state"] as? String, "disconnected")
        _ = call(transport, ["op": "forgetEnvironment", "origin": "http://127.0.0.1:14806", "environmentId": "env-b"])
        XCTAssertEqual(((call(transport, ["op": "environments"])["value"] as? [String: Any])?["saved"] as? [Any])?.count, 0)
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
                XCTAssertEqual(fields.first { $0.name == "scope" }?.value, "orchestration:read orchestration:operate terminal:operate review:write relay:read")
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
        let response = call(transport, ["op": "connect", "origin": "http://127.0.0.1:4318", "credential": pairing, "scope": "orchestration:read orchestration:operate terminal:operate review:write relay:read"])
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

    // Wire shapes captured from a HEAD (f90b77d) server: a typed failure keeps its own
    // tag and message (Cause.squash), and PullRequestOperationError's reason/detail survive.
    func testTypedFailuresKeepTheirTagMessageReasonAndDetail() throws {
        let thread = T3Wire.failure(["_tag": "Failure", "cause": [["_tag": "Fail", "error": [
            "_tag": "OrchestrationV2GetThreadProjectionError", "threadId": "t", "message": "Failed to load orchestration V2 thread t",
            "cause": ["name": "OrchestratorProjectionError", "message": "Failed to load orchestration projection for thread t.",
                      "cause": ["name": "ProjectionStoreThreadNotFoundError", "message": "No orchestration projection exists for thread t."]]]]]])
        XCTAssertEqual(thread.kind, "OrchestrationV2GetThreadProjectionError")
        XCTAssertEqual(thread.message, "Failed to load orchestration V2 thread t")
        let pr = T3Wire.failure(["_tag": "Failure", "cause": [["_tag": "Fail", "error": [
            "_tag": "PullRequestOperationError", "operation": "getDetail", "detail": "GitHub could not find pull request #7.",
            "reason": "not-found", "cause": ["name": "Error", "message": "HTTP 404"]]]]])
        XCTAssertEqual(pr.kind, "PullRequestOperationError")
        XCTAssertEqual(pr.message, "Pull request operation getDetail failed: GitHub could not find pull request #7.")
        XCTAssertEqual(pr.reason, "not-found"); XCTAssertEqual(pr.detail, "GitHub could not find pull request #7.")
        XCTAssertEqual(pr.json["reason"] as? String, "not-found")
        XCTAssertEqual(pr.json["detail"] as? String, "GitHub could not find pull request #7.")
        let die = T3Wire.failure(["_tag": "Failure", "cause": [["_tag": "Die", "defect": "Unknown request tag: x.y"]]])
        XCTAssertEqual(die.kind, "RPC"); XCTAssertEqual(die.message, "Unknown request tag: x.y")
        let mixed = T3Wire.failure([["_tag": "Interrupt"], ["_tag": "Fail", "error": ["_tag": "EnvironmentAuthorizationError", "message": "The authenticated token is missing required scope: orchestration:operate."]]])
        XCTAssertEqual(mixed.kind, "EnvironmentAuthorizationError")
        XCTAssertEqual(T3Wire.failure(["_tag": "Interrupt"]).kind, "Interrupt")
        XCTAssertEqual(T3Wire.failure(["cause": ["_tag": "Fail", "error": ["_tag": "Legacy", "message": "Old wire"]]]).message, "Old wire")
        XCTAssertEqual(T3Wire.failure(["_tag": "SilentError"]).message, "The server refused the request (SilentError).")
        XCTAssertEqual(T3Wire.failure(["_tag": "Failure", "cause": [["_tag": "Fail", "error": ["_tag": "Wrapped", "cause": ["name": "Error", "message": "Inner text"]]]]]).message, "Inner text")
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
        // Signed upload is a fixture-only asset lifecycle, never provider dispatch.
        let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl6QXkAAAAASUVORK5CYII=")!
        let created = call(transport, ["op": "request", "method": "attachments.createUploadUrl", "payload": ["type": "image", "name": "native-isolated-snapshot.png", "mimeType": "image/png", "sizeBytes": png.count], "generation": generation])
        XCTAssertEqual(created["ok"] as? Bool, true)
        if let upload = created["value"] as? [String: Any], let attachmentId = upload["attachmentId"] as? String, let route = upload["relativeUrl"] as? String {
            defer { XCTAssertEqual(call(transport, ["op": "request", "method": "attachments.delete", "payload": ["attachmentId": attachmentId], "generation": generation])["ok"] as? Bool, true) }
            XCTAssertTrue(route.hasPrefix("/api/attachments/upload/"))
            XCTAssertFalse(route.contains("?") || route.contains("%"), "Reference signs base64url path components")
            let rejected = call(transport, ["op": "uploadAttachment", "path": "/api/attachments/upload/invalid", "base64": png.base64EncodedString(), "generation": generation])
            XCTAssertEqual(rejected["ok"] as? Bool, false)
            XCTAssertEqual(call(transport, ["op": "uploadAttachment", "path": route, "base64": png.base64EncodedString(), "generation": generation])["ok"] as? Bool, true)
        } else { XCTFail("Upload metadata unavailable") }
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

// Lane r13-store (F5): an environment removed on this device leaves nothing that names it. Two loopback servers
// (R3Socket, R3HTTP) are paired, one is focused, then removed; credentials and the saved list are in memory and the
// remembered origin lives in a UserDefaults suite of this test's own, so no real Keychain item and no real preferences
// domain is read or written.
final class ForgetOriginTests: XCTestCase {
    private let originKey = "t3.server.origin"
    private struct Rig {
        let a: R3Socket, b: R3Socket
        let credentials: T3Credentials, saved: T3SavedEnvironments
        let defaults: UserDefaults, suite: String
        var originA: String { "http://127.0.0.1:\(a.port)" }
        var originB: String { "http://127.0.0.1:\(b.port)" }
    }
    private func makeRig() throws -> Rig {
        let a = try R3Socket(), b = try R3Socket()
        let suite = "exact-t3-transport-forget-\(UUID().uuidString)"
        let rig = Rig(a: a, b: b, credentials: T3Credentials(persistent: false), saved: T3SavedEnvironments(persistent: false),
                      defaults: UserDefaults(suiteName: suite)!, suite: suite)
        R3HTTP.reset { request in
            let environment = request.url?.port == Int(a.port) ? "env-a" : "env-b"
            switch request.url!.path {
            case "/.well-known/t3/environment":
                return (200, ["environmentId": environment, "label": "Fixture \(environment)", "orchestrationProtocolVersion": 2, "capabilities": ["connectionProbe": true]])
            case "/oauth/token": return (200, ["access_token": "bearer-\(environment)", "token_type": "Bearer"])
            case "/api/auth/session": return (200, ["authenticated": true, "scopes": ["orchestration:read", "orchestration:operate"]])
            case "/api/auth/websocket-ticket": return (200, ["ticket": "ticket-\(environment)"])
            default: return (404, [:])
            }
        }
        return rig
    }
    private func makeTransport(_ rig: Rig, persistent: Bool = true) -> T3Transport {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [R3HTTP.self]
        return T3Transport(persistent: persistent, configuration: config, credentials: rig.credentials, savedEnvironments: rig.saved,
                           defaults: rig.defaults, signals: false, changed: { _ in })
    }
    private func perform(_ transport: T3Transport, _ request: [String: Any]) -> [String: Any] {
        let done = DispatchSemaphore(value: 0)
        var result: [String: Any] = [:]
        transport.perform(request) { result = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success, "\(request["op"] ?? "") did not answer")
        return result
    }
    private func status(_ transport: T3Transport) -> [String: Any] { perform(transport, ["op": "status"])["value"] as? [String: Any] ?? [:] }
    private func pair(_ transport: T3Transport, _ origin: String) {
        let paired = perform(transport, ["op": "pairEnvironment", "origin": origin, "credential": "pairing"])
        XCTAssertEqual(paired["ok"] as? Bool, true, "\(paired)")
    }
    /// Pairs A and B, then focuses `focus` (its socket opens, which remembers the origin).
    private func pairedAndFocused(_ rig: Rig, on focus: String) -> T3Transport {
        let transport = makeTransport(rig)
        pair(transport, rig.originA); pair(transport, rig.originB)
        let opened = perform(transport, ["op": "connect", "origin": focus, "credential": ""])
        XCTAssertEqual((opened["value"] as? [String: Any])?["state"] as? String, "connected", "\(opened)")
        return transport
    }
    private func environmentIds(_ rig: Rig) -> [String] { rig.saved.all.compactMap { $0["environmentId"] as? String } }

    func testPairedBothThenFocusedBRemembersBAndRemovingItLeavesNothingNamingB() throws {
        let rig = try makeRig(); defer { UserDefaults.standard.removePersistentDomain(forName: rig.suite) }
        let transport = pairedAndFocused(rig, on: rig.originB); defer { transport.destroy() }
        // The starting point: B is focused, remembered, saved and has its credential.
        XCTAssertEqual(status(transport)["origin"] as? String, rig.originB)
        XCTAssertEqual(rig.defaults.string(forKey: originKey), rig.originB)
        XCTAssertEqual(try rig.credentials.read(origin: rig.originB, environment: "env-b"), "bearer-env-b")
        XCTAssertEqual(environmentIds(rig), ["env-a", "env-b"])

        let removed = perform(transport, ["op": "forgetEnvironment", "origin": rig.originB, "environmentId": "env-b"])
        let after = removed["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(after["state"] as? String, "disconnected")
        XCTAssertNotEqual(after["origin"] as? String, rig.originB)
        XCTAssertEqual(after["origin"] as? String, "")
        XCTAssertEqual(after["environmentId"] as? String, "")
        XCTAssertNotEqual(status(transport)["origin"] as? String, rig.originB, "A later status read does not restore B either")
        XCTAssertNil(rig.defaults.string(forKey: originKey), "the remembered origin is cleared")
        XCTAssertNil(try rig.credentials.read(origin: rig.originB, environment: "env-b"), "B's credential item is gone")
        XCTAssertEqual(try rig.credentials.read(origin: rig.originA, environment: "env-a"), "bearer-env-a", "A keeps its credential")
        XCTAssertEqual(environmentIds(rig), ["env-a"])

        // A relaunch: a new transport on the same stores. Its first status names neither B nor anything stale.
        let relaunched = makeTransport(rig); defer { relaunched.destroy() }
        XCTAssertEqual(status(relaunched)["origin"] as? String, "")
        XCTAssertEqual(status(relaunched)["state"] as? String, "disconnected")
    }

    func testRemovingTheUnfocusedBKeepsTheFocusAndItsRememberedOrigin() throws {
        let rig = try makeRig(); defer { UserDefaults.standard.removePersistentDomain(forName: rig.suite) }
        let transport = pairedAndFocused(rig, on: rig.originA); defer { transport.destroy() }
        XCTAssertEqual(rig.defaults.string(forKey: originKey), rig.originA)
        let removed = perform(transport, ["op": "forgetEnvironment", "origin": rig.originB, "environmentId": "env-b"])
        XCTAssertEqual((removed["value"] as? [String: Any])?["state"] as? String, "connected")
        XCTAssertEqual((removed["value"] as? [String: Any])?["origin"] as? String, rig.originA)
        XCTAssertEqual(rig.defaults.string(forKey: originKey), rig.originA)
        XCTAssertNil(try rig.credentials.read(origin: rig.originB, environment: "env-b"))
        XCTAssertEqual(environmentIds(rig), ["env-a"])
        // The relaunch finds A: the key names a saved environment.
        let relaunched = makeTransport(rig); defer { relaunched.destroy() }
        XCTAssertEqual(status(relaunched)["origin"] as? String, rig.originA)
    }

    func testDisconnectWithForgetOnTheFocusedBClearsTheSameThings() throws {
        let rig = try makeRig(); defer { UserDefaults.standard.removePersistentDomain(forName: rig.suite) }
        let transport = pairedAndFocused(rig, on: rig.originB); defer { transport.destroy() }
        let forgot = perform(transport, ["op": "disconnect", "forget": true])
        let after = forgot["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(after["state"] as? String, "disconnected")
        XCTAssertEqual(after["origin"] as? String, "")
        XCTAssertNil(rig.defaults.string(forKey: originKey))
        XCTAssertNil(try rig.credentials.read(origin: rig.originB, environment: "env-b"))
        XCTAssertEqual(environmentIds(rig), ["env-a"])
        // Without forget the focus stays remembered (Switch off / Disconnect are the reversible paths).
        let kept = try makeRig(); defer { UserDefaults.standard.removePersistentDomain(forName: kept.suite) }
        let other = pairedAndFocused(kept, on: kept.originB); defer { other.destroy() }
        let disconnected = perform(other, ["op": "disconnect", "forget": false])
        XCTAssertEqual((disconnected["value"] as? [String: Any])?["origin"] as? String, kept.originB)
        XCTAssertEqual(kept.defaults.string(forKey: originKey), kept.originB)
        XCTAssertEqual(environmentIds(kept), ["env-a", "env-b"])
    }

    func testAKeyLeftByAnEarlierBuildForARemovedEnvironmentIsDroppedNotRestored() throws {
        let rig = try makeRig(); defer { UserDefaults.standard.removePersistentDomain(forName: rig.suite) }
        rig.saved.remember(origin: rig.originA, descriptor: ["environmentId": "env-a", "label": "A"])
        rig.defaults.set(rig.originB, forKey: originKey) // B is not saved: the old build forgot it and left the key
        let first = makeTransport(rig); defer { first.destroy() }
        XCTAssertEqual(status(first)["origin"] as? String, "")
        XCTAssertNil(rig.defaults.string(forKey: originKey))
        rig.defaults.set("not an address", forKey: originKey)
        let second = makeTransport(rig); defer { second.destroy() }
        XCTAssertEqual(status(second)["origin"] as? String, "")
        XCTAssertNil(rig.defaults.string(forKey: originKey))
        // A saved environment's origin is restored as before.
        rig.defaults.set(rig.originA, forKey: originKey)
        let third = makeTransport(rig); defer { third.destroy() }
        XCTAssertEqual(status(third)["origin"] as? String, rig.originA)
    }

    func testAnAgentRunKeepsNoOriginButStillNamesNothingAfterRemoval() throws {
        let rig = try makeRig(); defer { UserDefaults.standard.removePersistentDomain(forName: rig.suite) }
        let transport = makeTransport(rig, persistent: false); defer { transport.destroy() }
        pair(transport, rig.originA); pair(transport, rig.originB)
        _ = perform(transport, ["op": "connect", "origin": rig.originB, "credential": ""])
        XCTAssertNil(rig.defaults.string(forKey: originKey), "a non-persistent transport never writes the key")
        let removed = perform(transport, ["op": "forgetEnvironment", "origin": rig.originB, "environmentId": "env-b"])
        XCTAssertEqual((removed["value"] as? [String: Any])?["origin"] as? String, "")
        XCTAssertEqual(status(transport)["origin"] as? String, "")
        XCTAssertNil(try rig.credentials.read(origin: rig.originB, environment: "env-b"))
    }
}

// R3TransportTests (r3.swift): reconnect policy, read gate, stream retries. ForgetOriginTests: F5, removal leaves no origin.
// RemoteScopeTests (remote-scopes.swift): the exchange sends the scope TS names; a narrow link is refused.
// RouteTests (routes.swift): one environment, several routes.
// TerminalStreamTests (terminal-streams.swift): the drawer's attach streams beside the app's 16.
let suites = [TransportTests.defaultTestSuite, R3TransportTests.defaultTestSuite, ForgetOriginTests.defaultTestSuite, RemoteScopeTests.defaultTestSuite, RouteTests.defaultTestSuite, TerminalStreamTests.defaultTestSuite]
var executed = 0, failures = 0, succeeded = true
for suite in suites {
    suite.run()
    guard let run = suite.testRun else { exit(1) }
    executed += run.executionCount; failures += run.totalFailureCount; succeeded = succeeded && run.hasSucceeded
}
print("T3 transport: \(executed) tests, \(failures) failures")
guard executed >= 17 else { exit(1) }
exit(succeeded ? 0 : 1)

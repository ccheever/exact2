import Foundation
import XCTest

/// HTTP for the remote-scope tests: records each `/oauth/token` form and answers from `handler`.
final class ScopeHTTP: URLProtocol, @unchecked Sendable {
    static let lock = NSLock()
    static var handler: (URLRequest) -> (Int, Any) = { _ in (500, [:]) }
    static var forms: [[String: String]] = []
    override class func canInit(with request: URLRequest) -> Bool { request.url?.scheme == "http" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        var bytes = request.httpBody ?? Data()
        if let stream = request.httpBodyStream {
            stream.open(); defer { stream.close() }
            var chunk = [UInt8](repeating: 0, count: 1024)
            while stream.hasBytesAvailable { let n = stream.read(&chunk, maxLength: chunk.count); if n <= 0 { break }; bytes.append(contentsOf: chunk.prefix(n)) }
        }
        Self.lock.lock()
        if request.url!.path == "/oauth/token" {
            let items = URLComponents(string: "http://fixture/?" + String(decoding: bytes, as: UTF8.self))?.queryItems ?? []
            Self.forms.append(Dictionary(items.map { ($0.name, $0.value ?? "") }, uniquingKeysWith: { $1 }))
        }
        let response = Self.handler(request); Self.lock.unlock()
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: response.0, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: response.1))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
    static func reset(_ handler: @escaping (URLRequest) -> (Int, Any)) { lock.lock(); forms = []; self.handler = handler; lock.unlock() }
    static var lastForm: [String: String]? { lock.lock(); defer { lock.unlock() }; return forms.last }
}

/// remote-scopes.ts: TypeScript names the scopes; the transport sends exactly the `scope` it is given.
final class RemoteScopeTests: XCTestCase {
    /// A test fixture mirroring remote-scopes.ts AUTH_STANDARD_CLIENT_SCOPES (the production names live in TS).
    let standard = "orchestration:read orchestration:operate terminal:operate review:write relay:read"

    private func makeTransport() -> T3Transport {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [ScopeHTTP.self]
        return T3Transport(persistent: false, configuration: config, changed: { _ in })
    }
    private func perform(_ transport: T3Transport, _ request: [String: Any]) -> [String: Any] {
        let done = DispatchSemaphore(value: 0)
        var result: [String: Any] = [:]
        transport.perform(request) { result = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success, "\(request["op"] ?? "") did not answer")
        return result
    }
    private func server(token: @escaping () -> (Int, Any)) {
        ScopeHTTP.reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment": return (200, ["environmentId": "env-s", "label": "Scoped", "orchestrationProtocolVersion": 2])
            case "/oauth/token": return token()
            case "/api/auth/session": return (200, ["authenticated": true, "scopes": ["orchestration:read"]])
            default: return (404, [:])
            }
        }
    }

    func testPairingSendsTheScopeItIsGiven() {
        server { (200, ["access_token": "bearer-s", "token_type": "Bearer"]) }
        let transport = makeTransport(); defer { transport.destroy() }
        let paired = perform(transport, ["op": "pairEnvironment", "origin": "http://127.0.0.1:16141", "credential": "code-1", "scope": standard])
        XCTAssertEqual(paired["ok"] as? Bool, true)
        XCTAssertEqual(ScopeHTTP.lastForm?["scope"], standard)
        XCTAssertEqual(ScopeHTTP.lastForm?["subject_token"], "code-1")
        _ = perform(transport, ["op": "forgetEnvironment", "origin": "http://127.0.0.1:16141", "environmentId": "env-s"])
    }

    func testConnectSendsTheScopeItIsGivenAndNoneWhenAbsent() {
        server { (200, ["access_token": "bearer-s", "token_type": "Bearer"]) }
        let first = makeTransport()
        _ = perform(first, ["op": "connect", "origin": "http://127.0.0.1:16142", "credential": "code-2", "scope": standard])
        XCTAssertEqual(ScopeHTTP.lastForm?["scope"], standard)
        first.destroy()
        // The embedded primary's exchange omits `scope` and receives everything its credential carries.
        let second = makeTransport(); defer { second.destroy() }
        _ = perform(second, ["op": "connect", "origin": "http://127.0.0.1:16142", "credential": "code-3"])
        XCTAssertEqual(ScopeHTTP.lastForm?["subject_token"], "code-3")
        XCTAssertNil(ScopeHTTP.lastForm?["scope"])
    }

    func testANarrowLinkIsRefusedWithTheReferenceTextAndNothingIsSaved() {
        // EnvironmentAuth.ts: a requested scope the link does not carry answers scope_not_granted (HTTP 400).
        server { (400, ["_tag": "EnvironmentRequestInvalidError", "code": "invalid_request", "reason": "scope_not_granted", "traceId": "trace-1"]) }
        let transport = makeTransport(); defer { transport.destroy() }
        let refused = perform(transport, ["op": "pairEnvironment", "origin": "http://127.0.0.1:16143", "credential": "read-only-code", "scope": standard])
        XCTAssertEqual(refused["ok"] as? Bool, false)
        XCTAssertEqual((refused["error"] as? [String: Any])?["message"] as? String, "The environment rejected the authentication request.")
        XCTAssertFalse("\(refused)".contains("read-only-code"))
        let saved = (perform(transport, ["op": "environments"])["value"] as? [String: Any])?["saved"] as? [Any] ?? []
        XCTAssertEqual(saved.count, 0)
        let connected = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:16143", "credential": "read-only-code", "scope": standard])
        XCTAssertEqual((connected["error"] as? [String: Any])?["message"] as? String, "The environment rejected the authentication request.")
    }

    func testFailureTextKeepsTheServerMessageAndTheCredentialRule() {
        XCTAssertEqual(T3RemoteAuth.failureMessage(["message": "Exact text"], status: 400), "Exact text")
        XCTAssertEqual(T3RemoteAuth.failureMessage(["reason": "invalid_credential"], status: 401), "The environment credential is invalid.")
        XCTAssertEqual(T3RemoteAuth.failureMessage(["reason": "invalid_scope"], status: 400), "The environment rejected the authentication request.")
        XCTAssertEqual(T3RemoteAuth.failureMessage(["reason": "invalid_history_cursor"], status: 400), "The server returned HTTP 400.")
        XCTAssertEqual(T3RemoteAuth.failureMessage(nil, status: 502), "The server returned HTTP 502.")
    }
}

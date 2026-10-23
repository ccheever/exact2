import Foundation
import XCTest

// Ported from T3 Code (MIT, see LICENSE-T3), reference 9c161462c9:
// apps/desktop/src/backend/DesktopLocalEnvironmentAuth.test.ts. Change from the reference: an
// XCTest case over T3LocalAuth (T3LocalBackend.swift), a URLProtocol in place of Effect's client.
private final class TokenEndpoint: URLProtocol, @unchecked Sendable {
    static let lock = NSLock()
    static var bodies: [String] = []
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        var body = request.httpBody ?? Data()
        if body.isEmpty, let stream = request.httpBodyStream {
            stream.open(); defer { stream.close() }
            var buffer = [UInt8](repeating: 0, count: 4096)
            while stream.hasBytesAvailable { let n = stream.read(&buffer, maxLength: buffer.count); if n <= 0 { break }; body.append(buffer, count: n) }
        }
        Self.lock.lock(); Self.bodies.append(String(decoding: body, as: UTF8.self)); Self.lock.unlock()
        // The answer comes a little later, so the second request finds the first in flight.
        Thread.sleep(forTimeInterval: 0.1)
        let json = #"{"access_token":"desktop-bearer-token","issued_token_type":"urn:ietf:params:oauth:token-type:access_token","token_type":"Bearer","expires_in":3600,"scope":"orchestration:read"}"#
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: ["content-type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(json.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

/// Answers the first `failures` exchanges with `failureStatus` (0: a refused connection), then with a token.
private final class FlakyTokenEndpoint: URLProtocol, @unchecked Sendable {
    static let lock = NSLock()
    static var failures = 0, failureStatus = 503, requests = 0
    static func reset(failures: Int, status: Int) { lock.lock(); self.failures = failures; failureStatus = status; requests = 0; lock.unlock() }
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        Self.lock.lock(); Self.requests += 1; let failing = Self.requests <= Self.failures, status = Self.failureStatus; Self.lock.unlock()
        if failing && status <= 0 { client?.urlProtocol(self, didFailWithError: URLError(status == 0 ? .cannotConnectToHost : status == -1 ? .timedOut : .networkConnectionLost)); return }
        let body = failing ? (status == 401 ? #"{"_tag":"EnvironmentAuthInvalidError","code":"auth_invalid","reason":"invalid_credential","traceId":"trace-1"}"# : "")
            : #"{"access_token":"desktop-bearer-token","token_type":"Bearer","expires_in":3600}"#
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: failing ? status : 200, httpVersion: "HTTP/1.1", headerFields: ["content-type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

final class LocalAuthTests: XCTestCase {
    private func flakyExchange(failures: Int, status: Int) -> (String?, Int) {
        FlakyTokenEndpoint.reset(failures: failures, status: status)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [FlakyTokenEndpoint.self]
        let auth = T3LocalAuth(session: URLSession(configuration: configuration), retryTimeout: 2, retryInterval: 0.05)
        let done = DispatchSemaphore(value: 0)
        var token: String?
        auth.bearerToken(base: URL(string: "http://127.0.0.1:3773")!, credential: "desktop-bootstrap-token") { token = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success)
        FlakyTokenEndpoint.lock.lock(); defer { FlakyTokenEndpoint.lock.unlock() }
        return (token, FlakyTokenEndpoint.requests)
    }

    func test_retries_a_backend_that_is_still_starting() {
        for status in [502, 503, 504, -1, -2] {
            let (token, requests) = flakyExchange(failures: 2, status: status)
            XCTAssertEqual(token, "desktop-bearer-token", "status \(status)")
            XCTAssertEqual(requests, 3, "status \(status)")
        }
    }

    func test_retries_a_refused_connection() {
        let (token, requests) = flakyExchange(failures: 1, status: 0)
        XCTAssertEqual(token, "desktop-bearer-token")
        XCTAssertEqual(requests, 2)
    }

    func test_does_not_retry_a_rejected_bootstrap_credential() {
        let (token, requests) = flakyExchange(failures: 1, status: 401)
        XCTAssertNil(token)
        XCTAssertEqual(requests, 1)
    }

    func test_gives_up_on_a_backend_that_stays_unavailable_at_the_deadline() {
        FlakyTokenEndpoint.reset(failures: .max, status: 504)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [FlakyTokenEndpoint.self]
        let auth = T3LocalAuth(session: URLSession(configuration: configuration), retryTimeout: 0.3, retryInterval: 0.05)
        let done = DispatchSemaphore(value: 0), started = Date()
        var token: String? = "unset"
        auth.bearerToken(base: URL(string: "http://127.0.0.1:3773")!, credential: "desktop-bootstrap-token") { token = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success)
        XCTAssertNil(token)
        XCTAssertLessThan(Date().timeIntervalSince(started), 1)
        FlakyTokenEndpoint.lock.lock(); XCTAssertGreaterThan(FlakyTokenEndpoint.requests, 2); FlakyTokenEndpoint.lock.unlock()
    }

    func test_exchanges_the_desktop_bootstrap_credential_only_once() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [TokenEndpoint.self]
        let auth = T3LocalAuth(session: URLSession(configuration: configuration))
        let base = URL(string: "http://127.0.0.1:3773")!
        let group = DispatchGroup(), lock = NSLock()
        var tokens: [String?] = []
        for _ in 0..<2 {
            group.enter()
            auth.bearerToken(base: base, credential: "desktop-bootstrap-token") { token in lock.lock(); tokens.append(token); lock.unlock(); group.leave() }
        }
        XCTAssertEqual(group.wait(timeout: .now() + 5), .success)
        XCTAssertEqual(tokens, ["desktop-bearer-token", "desktop-bearer-token"])
        TokenEndpoint.lock.lock(); let bodies = TokenEndpoint.bodies; TokenEndpoint.lock.unlock()
        XCTAssertEqual(bodies.count, 1)
        // The request is DesktopLocalEnvironmentAuth's: no scope, its label and device type, no OS.
        let fields = Dictionary(uniqueKeysWithValues: (URLComponents(string: "?" + (bodies.first ?? ""))?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
        XCTAssertEqual(fields, ["grant_type": "urn:ietf:params:oauth:grant-type:token-exchange", "subject_token": "desktop-bootstrap-token",
                                "subject_token_type": "urn:t3:params:oauth:token-type:environment-bootstrap",
                                "requested_token_type": "urn:ietf:params:oauth:token-type:access_token",
                                "client_label": "T3 Code Desktop", "client_device_type": "desktop"])
        XCTAssertEqual(auth.cached, "desktop-bearer-token")
    }

}

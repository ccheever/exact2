import Foundation
import XCTest

// Ported from T3 Code (MIT, see LICENSE-T3), reference 1e2ecbd975:
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

final class LocalAuthTests: XCTestCase {
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

    func testARemoteExchangeKeepsItsOwnLabel() {
        let body = String(decoding: T3RemoteAuth.exchangeForm(credential: "c", scope: "orchestration:read"), as: UTF8.self)
        XCTAssertTrue(body.contains("client_label=Exact%20T3%20for%20Mac"), body)
        XCTAssertTrue(body.contains("client_os=macos"))
        XCTAssertTrue(body.contains("scope=orchestration:read") || body.contains("scope=orchestration%3Aread"))
    }
}

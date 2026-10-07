import Foundation
import XCTest

// The ChatGPT sign-in's loopback receiver (T3CodexAuth.swift), ported from T3 Code 1e2ecbd975
// apps/desktop/src/app/CodexAuthCallback.test.ts (its four cases; the hosted-web 303 case is not
// ported, the receiver has no destination) and packages/shared/src/codexAuthHandoff.test.ts
// ("rejects duplicated authorization parameters and non-loopback callback addresses", the OpenAI
// host case of "rejects other handlers…"), plus the expiry, port-in-use, already-open and browser
// failure messages. Build with the README's module-test recipe; no OpenAI account is involved.
final class CodexAuthTests: XCTestCase {
    private func freePort() -> UInt16 {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        var address = sockaddr_in(); address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1"); address.sin_port = 0
        _ = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        var bound = sockaddr_in(); var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &bound) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) } }
        close(fd)
        return UInt16(bigEndian: bound.sin_port)
    }
    private func request(_ port: UInt16, state: String = String(repeating: "a", count: 43)) -> String {
        var url = URLComponents(string: "https://auth.openai.com/api/accounts/authorize")!
        url.queryItems = [URLQueryItem(name: "client_id", value: "dynamic_agent_client"), URLQueryItem(name: "response_type", value: "code"),
                          URLQueryItem(name: "redirect_uri", value: "http://127.0.0.1:\(port)/auth/callback"), URLQueryItem(name: "state", value: state),
                          URLQueryItem(name: "code_challenge_method", value: "S256"), URLQueryItem(name: "code_challenge", value: String(repeating: "b", count: 43))]
        return url.url!.absoluteString
    }
    private func callback(_ authorizationUrl: String, state: String? = nil) -> String {
        let items = URLComponents(string: authorizationUrl)!.queryItems!
        var url = URLComponents(string: items.first { $0.name == "redirect_uri" }!.value!)!
        url.queryItems = [URLQueryItem(name: "state", value: state ?? items.first { $0.name == "state" }!.value!),
                          URLQueryItem(name: "code", value: "test-code"), URLQueryItem(name: "client_id", value: "oaiapp_test")]
        return url.url!.absoluteString
    }
    /// A blocking GET (the fake browser).
    private func fetch(_ url: String, method: String = "GET") -> (status: Int, headers: [AnyHashable: Any], body: String) {
        var request = URLRequest(url: URL(string: url)!); request.httpMethod = method; request.timeoutInterval = 5
        let done = DispatchSemaphore(value: 0); var result: (Int, [AnyHashable: Any], String) = (0, [:], "")
        URLSession(configuration: .ephemeral).dataTask(with: request) { data, response, _ in
            let http = response as? HTTPURLResponse
            result = (http?.statusCode ?? 0, http?.allHeaderFields ?? [:], data.map { String(decoding: $0, as: UTF8.self) } ?? "")
            done.signal()
        }.resume()
        done.wait()
        return result
    }
    private func header(_ headers: [AnyHashable: Any], _ name: String) -> String? {
        headers.first { ($0.key as? String)?.lowercased() == name }?.value as? String
    }

    func testBindsBeforeOpeningSignInIgnoresAForeignResponseAndReturnsOnlyTheCodeCallback() {
        let auth = T3CodexAuth(), authorizationUrl = request(freePort()), expected = callback(authorizationUrl)
        var opened: [String] = []
        let failure = auth.start(authorizationUrl) { url in
            opened.append(url)
            XCTAssertEqual(self.fetch(self.callback(authorizationUrl, state: "foreign")).status, 400, "a foreign state is refused")
            XCTAssertEqual(self.fetch(expected, method: "POST").status, 400, "only GET")
            XCTAssertEqual(self.fetch(expected.replacingOccurrences(of: "/auth/callback", with: "/other")).status, 400, "only the callback path")
            let response = self.fetch(expected)
            XCTAssertEqual(response.status, 200)
            XCTAssertEqual(self.header(response.headers, "cache-control"), "no-store")
            XCTAssertEqual(self.header(response.headers, "referrer-policy"), "no-referrer")
            XCTAssertEqual(self.header(response.headers, "x-content-type-options"), "nosniff")
            XCTAssertEqual(self.header(response.headers, "content-security-policy"), "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'")
            XCTAssertTrue(response.body.contains("Return to T3 Code"))
            XCTAssertFalse(response.body.contains("test-code"), "the code never appears in the page")
            return true
        }
        XCTAssertNil(failure)
        XCTAssertEqual(opened, [authorizationUrl], "the browser opens once, after the bind")
        XCTAssertEqual(auth.take(authorizationUrl)["callbackUrl"] as? String, expected)
        XCTAssertEqual(auth.take(authorizationUrl)["phase"] as? String, "none", "the outcome is taken once")
    }

    func testCancelsAndReleasesItsListenerSoExactPortReauthorizationCanRunAgain() {
        let auth = T3CodexAuth(), authorizationUrl = request(freePort())
        XCTAssertNil(auth.start(authorizationUrl) { _ in true })
        XCTAssertEqual(auth.take(authorizationUrl)["phase"] as? String, "waiting")
        auth.cancel(authorizationUrl)
        let cancelled = auth.take(authorizationUrl)
        XCTAssertEqual(cancelled["phase"] as? String, "failed")
        XCTAssertEqual(cancelled["message"] as? String, "Sign-in cancelled on this computer.")
        XCTAssertFalse(auth.listening(authorizationUrl))
        XCTAssertNil(auth.start(authorizationUrl) { _ in self.fetch(self.callback(authorizationUrl)).status == 200 }, "the same port binds again")
        XCTAssertEqual(auth.take(authorizationUrl)["callbackUrl"] as? String, callback(authorizationUrl))
    }

    func testAllowsTwoAccountsToCompleteIndependently() {
        let auth = T3CodexAuth()
        let a = request(freePort(), state: String(repeating: "a", count: 43)), b = request(freePort(), state: String(repeating: "c", count: 43))
        XCTAssertNil(auth.start(a) { _ in true })
        XCTAssertNil(auth.start(b) { _ in true })
        XCTAssertEqual(fetch(callback(b)).status, 200)
        XCTAssertEqual(fetch(callback(a)).status, 200)
        XCTAssertEqual(auth.take(a)["callbackUrl"] as? String, callback(a))
        XCTAssertEqual(auth.take(b)["callbackUrl"] as? String, callback(b))
    }

    func testExpiresAfterTheTimeoutAndFreesThePort() {
        let auth = T3CodexAuth(), authorizationUrl = request(freePort())
        XCTAssertNil(auth.start(authorizationUrl, timeout: 0.3) { _ in true })
        Thread.sleep(forTimeInterval: 0.6)
        let expired = auth.take(authorizationUrl)
        XCTAssertEqual(expired["message"] as? String, "Sign-in expired. Try again.")
        XCTAssertNil(auth.start(authorizationUrl) { _ in true }, "the port is free again")
        auth.cancel(authorizationUrl)
    }

    func testRefusesAPortInUseASecondListenerAndABrowserThatDidNotOpen() {
        let auth = T3CodexAuth(), port = freePort(), authorizationUrl = request(port)
        // Another listener holds the redirect's port.
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        var address = sockaddr_in(); address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1"); address.sin_port = port.bigEndian
        _ = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        listen(fd, 1)
        var opened = false
        XCTAssertEqual(auth.start(authorizationUrl) { _ in opened = true; return true }, T3CodexAuth.portInUse)
        XCTAssertFalse(opened, "the browser never opens without the port")
        close(fd)
        XCTAssertNil(auth.start(authorizationUrl) { _ in true })
        XCTAssertEqual(auth.start(authorizationUrl) { _ in true }, "This sign-in is already open on this computer.")
        auth.cancel(authorizationUrl)
        _ = auth.take(authorizationUrl)
        XCTAssertEqual(auth.start(authorizationUrl) { _ in false }, "Could not open your sign-in browser.")
        XCTAssertFalse(auth.listening(authorizationUrl), "a failed open releases the port")
    }

    func testRejectsDuplicatedAuthorizationParametersAndNonLoopbackCallbackAddresses() {
        let valid = request(54213)
        XCTAssertNoThrow(try T3CodexAuthRules.authorizationRequest(valid))
        func refuse(_ value: String) { XCTAssertThrowsError(try T3CodexAuthRules.authorizationRequest(value), value) }
        refuse(valid + "&redirect_uri=http://localhost:1/auth/callback")
        refuse(valid.replacingOccurrences(of: "127.0.0.1", with: "localhost"))
        refuse(valid.replacingOccurrences(of: "http%3A//127.0.0.1%3A54213", with: "https%3A//attacker.example").replacingOccurrences(of: "http://127.0.0.1:54213", with: "https://attacker.example"))
        refuse(valid.replacingOccurrences(of: "auth.openai.com", with: "attacker.example"))
        refuse(valid.replacingOccurrences(of: String(repeating: "a", count: 43), with: "short"))
        refuse(valid.replacingOccurrences(of: String(repeating: "b", count: 43), with: String(repeating: "b", count: 42)))
        refuse(valid.replacingOccurrences(of: "dynamic_agent_client", with: "other_client"))
        refuse(valid + "#fragment")
        let state = String(repeating: "a", count: 43), redirect = "http://127.0.0.1:54213/auth/callback"
        let base = "\(redirect)?state=\(state)&code=one-time-code&client_id=oaiapp_test"
        XCTAssertNoThrow(try T3CodexAuthRules.callbackUrl(base, redirectUri: redirect, state: state))
        XCTAssertNoThrow(try T3CodexAuthRules.callbackUrl("\(redirect)?state=\(state)&error=access_denied", redirectUri: redirect, state: state))
        for bad in [base + "&state=another", base + "&code=second", base + "&error=both", "\(redirect)?state=\(state)", base.replacingOccurrences(of: "oaiapp_test", with: "other"),
                    base.replacingOccurrences(of: "54213", with: "54214"), base.replacingOccurrences(of: "/auth/callback", with: "/auth/other")] {
            XCTAssertThrowsError(try T3CodexAuthRules.callbackUrl(bad, redirectUri: redirect, state: state), bad)
        }
    }
}

let suite = XCTestSuite(name: "T3 codex auth")
suite.addTest(CodexAuthTests.defaultTestSuite)
suite.run()
guard let run = suite.testRun, run.executionCount == 6 else { print("T3 codex auth: \(suite.testRun?.executionCount ?? 0) tests ran, expected 6"); exit(1) }
print("T3 codex auth: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)

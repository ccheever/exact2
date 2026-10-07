import Foundation
import XCTest

// 20261005-local-primary-environment: the primary environment over T3Transport. Its bearer is the
// embedded server's (memory, `primaryBearer`), it walks no routes, it is never saved and never
// remembered by origin (its port can change every launch); a focus token `primary` beside
// `t3.server.origin` names it for the next launch. A saved duplicate (same environment id) can be
// forgotten without touching the primary's connection (decision U6).
final class PrimaryTransportTests: XCTestCase {
    private let originKey = "t3.server.origin", focusKey = "t3.server.focus"
    private final class Headers: @unchecked Sendable {
        private let lock = NSLock(); private var list: [String] = []
        func add(_ value: String) { lock.lock(); list.append(value); lock.unlock() }
        var all: [String] { lock.lock(); defer { lock.unlock() }; return list }
    }
    /// The embedded server: environment "local-env"; the session answers only the memory bearer.
    private func serve(_ headers: Headers) {
        R3HTTP.reset { (request: URLRequest) -> (Int, Any) in
            let descriptor: [String: Any] = ["environmentId": "local-env", "label": "Daehyeon's MacBook Pro", "orchestrationProtocolVersion": 2,
                                             "capabilities": ["connectionProbe": true] as [String: Any]]
            let session: [String: Any] = ["authenticated": true, "scopes": ["orchestration:read", "orchestration:operate", "access:write"]]
            switch request.url!.path {
            case "/.well-known/t3/environment": return (200, descriptor)
            case "/oauth/token": return (500, ["message": "The primary never exchanges a pairing code."] as [String: Any])
            case "/api/auth/session", "/api/auth/websocket-ticket":
                let bearer = request.value(forHTTPHeaderField: "Authorization") ?? ""
                headers.add(bearer)
                if bearer != "Bearer memory-bearer" { return (401, ["message": "Unauthorized."] as [String: Any]) }
                if request.url!.path == "/api/auth/session" { return (200, session) }
                return (200, ["ticket": "local-ticket"] as [String: Any])
            default: return (404, [:] as [String: Any])
            }
        }
    }
    private func makeTransport(persistent: Bool = false, defaults: UserDefaults = .standard, saved: T3SavedEnvironments = T3SavedEnvironments(persistent: false),
                               credentials: T3Credentials = T3Credentials(persistent: false), bearer: @escaping () -> String? = { "memory-bearer" }) -> T3Transport {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [R3HTTP.self]
        let transport = T3Transport(persistent: persistent, configuration: config, credentials: credentials, savedEnvironments: saved,
                                    defaults: defaults, signals: false, changed: { _ in })
        transport.primaryBearer = bearer
        return transport
    }
    private func perform(_ transport: T3Transport, _ request: [String: Any]) -> [String: Any] {
        let done = DispatchSemaphore(value: 0)
        var result: [String: Any] = [:]
        transport.perform(request) { result = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success, "\(request["op"] ?? "") did not answer")
        return result
    }
    private func status(_ transport: T3Transport) -> [String: Any] { perform(transport, ["op": "status"])["value"] as? [String: Any] ?? [:] }
    private func until(_ seconds: TimeInterval, _ condition: () -> Bool) -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end { if condition() { return true }; RunLoop.current.run(until: Date().addingTimeInterval(0.02)) }
        return condition()
    }

    func testThePrimaryConnectsWithTheMemoryBearerAndIsNeverSaved() throws {
        let socket = try R3Socket(), headers = Headers(), saved = T3SavedEnvironments(persistent: false), credentials = T3Credentials(persistent: false)
        serve(headers)
        let transport = makeTransport(saved: saved, credentials: credentials); defer { transport.destroy() }
        let origin = "http://127.0.0.1:\(socket.port)"
        let opened = perform(transport, ["op": "connect", "origin": origin, "primary": true, "credential": "ignored"])
        let value = opened["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(value["state"] as? String, "connected", "\(opened)")
        XCTAssertEqual(value["primary"] as? Bool, true)
        XCTAssertEqual(value["focus"] as? String, "primary")
        XCTAssertEqual(value["environmentId"] as? String, "local-env")
        XCTAssertEqual(Set(headers.all), ["Bearer memory-bearer"], "every authorized request carries the memory bearer")
        XCTAssertEqual(R3HTTP.count("/oauth/token"), 0, "no pairing exchange")
        XCTAssertTrue(saved.all.isEmpty, "the primary is never a saved environment")
        XCTAssertNil(try credentials.read(origin: origin, environment: "local-env"), "nor a stored credential")
        XCTAssertEqual(socket.header("x-t3-orchestration-protocol", connection: 0), "2")
    }

    func testThePrimaryWaitsForItsBearerOnItsLadderInsteadOfFailing() throws {
        let socket = try R3Socket(), headers = Headers()
        serve(headers)
        final class Bearer: @unchecked Sendable { var value: String? }
        let bearer = Bearer()
        let transport = makeTransport(bearer: { bearer.value }); defer { transport.destroy() }
        _ = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(socket.port)", "primary": true])
        XCTAssertTrue(until(3) { self.status(transport)["state"] as? String == "reconnecting" }, "\(status(transport))")
        XCTAssertTrue((status(transport)["message"] as? String ?? "").hasPrefix("The local server is starting."), "\(status(transport))")
        bearer.value = "memory-bearer"
        let retried = perform(transport, ["op": "retry"])
        XCTAssertEqual((retried["value"] as? [String: Any])?["state"] as? String, "connected", "\(retried)")
    }

    func testAFocusOnThePrimaryNeverRewritesTheRememberedOrigin() throws {
        let first = try R3Socket(), second = try R3Socket(), headers = Headers()
        serve(headers)
        let suite = "exact-t3-transport-primary-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        // A saved remote environment was the last focus by origin.
        let saved = T3SavedEnvironments(persistent: false)
        saved.remember(origin: "https://devbox.example.com", descriptor: ["environmentId": "remote-env", "label": "Remote"])
        defaults.set("https://devbox.example.com", forKey: originKey)
        // Launch 1: the primary on one port.
        let launch1 = makeTransport(persistent: true, defaults: defaults, saved: saved)
        XCTAssertEqual(status(launch1)["focus"] as? String, "", "no token yet: the remembered origin decides")
        XCTAssertEqual(status(launch1)["origin"] as? String, "https://devbox.example.com")
        let opened = perform(launch1, ["op": "connect", "origin": "http://127.0.0.1:\(first.port)", "primary": true])
        XCTAssertEqual((opened["value"] as? [String: Any])?["state"] as? String, "connected", "\(opened)")
        XCTAssertEqual(defaults.string(forKey: originKey), "https://devbox.example.com", "t3.server.origin is not rewritten")
        XCTAssertEqual(defaults.string(forKey: focusKey), "primary")
        launch1.destroy()
        // Launch 2: the token brings the focus back to the primary, now on another port.
        let launch2 = makeTransport(persistent: true, defaults: defaults, saved: saved); defer { launch2.destroy() }
        let restored = status(launch2)
        XCTAssertEqual(restored["focus"] as? String, "primary")
        XCTAssertEqual(restored["origin"] as? String, "", "no origin is restored for the primary")
        let reopened = perform(launch2, ["op": "connect", "origin": "http://127.0.0.1:\(second.port)", "primary": true])
        XCTAssertEqual((reopened["value"] as? [String: Any])?["state"] as? String, "connected", "\(reopened)")
        XCTAssertEqual(defaults.string(forKey: originKey), "https://devbox.example.com")
        XCTAssertEqual(defaults.string(forKey: focusKey), "primary")
    }

    func testForgettingASavedDuplicateKeepsThePrimaryConnected() throws {
        let socket = try R3Socket(), headers = Headers(), saved = T3SavedEnvironments(persistent: false), credentials = T3Credentials(persistent: false)
        serve(headers)
        let transport = makeTransport(saved: saved, credentials: credentials); defer { transport.destroy() }
        _ = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(socket.port)", "primary": true])
        XCTAssertEqual(status(transport)["state"] as? String, "connected")
        // A pairing of the same machine made earlier (same ~/.t3, so the same environment id).
        saved.remember(origin: "http://127.0.0.1:16999", descriptor: ["environmentId": "local-env", "label": "Old pairing"])
        try credentials.save("old-token", origin: "http://127.0.0.1:16999", environment: "local-env")
        let forgotten = perform(transport, ["op": "forgetEnvironment", "origin": "http://127.0.0.1:16999", "environmentId": "local-env"])
        XCTAssertEqual(forgotten["ok"] as? Bool, true, "\(forgotten)")
        XCTAssertTrue(saved.all.isEmpty)
        XCTAssertNil(try credentials.read(origin: "http://127.0.0.1:16999", environment: "local-env"), "its credential is forgotten")
        XCTAssertEqual(status(transport)["state"] as? String, "connected", "the primary stays connected")
        // A disconnect asking to forget never forgets the primary (there is nothing saved to forget).
        _ = perform(transport, ["op": "disconnect", "forget": true])
        XCTAssertTrue(saved.all.isEmpty)
    }
}

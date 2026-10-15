import Foundation
import XCTest

// One saved environment, several routes (T3Credentials.swift
// T3SavedEnvironments, T3Routes.swift, T3Transport.swift). The fixture origin is one
// loopback WebSocket peer (R3Socket) reached as 127.0.0.1 and as localhost, so the two
// addresses are two routes to the same server; R3HTTP answers each host differently.
final class RouteTests: XCTestCase {
    private final class Log: @unchecked Sendable {
        private let lock = NSLock(); private var list: [String] = []
        func add(_ line: String) { lock.lock(); list.append(line); lock.unlock() }
        var all: [String] { lock.lock(); defer { lock.unlock() }; return list }
    }
    private let credentials = T3Credentials(persistent: false)
    private let saved = T3SavedEnvironments(persistent: false)

    private func transport() -> T3Transport {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [R3HTTP.self]
        return T3Transport(persistent: false, configuration: config, credentials: credentials, savedEnvironments: saved, random: { 0 }, signals: false, changed: { _ in })
    }
    private func perform(_ transport: T3Transport, _ request: [String: Any], timeout: TimeInterval = 8) -> [String: Any] {
        let done = DispatchSemaphore(value: 0)
        var result: [String: Any] = [:]
        transport.perform(request) { result = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + timeout), .success, "\(request["op"] ?? "") did not answer")
        return result
    }
    private func status(_ transport: T3Transport) -> [String: Any] { perform(transport, ["op": "status"])["value"] as? [String: Any] ?? [:] }
    private func until(_ seconds: TimeInterval, _ condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline { if condition() { return true }; RunLoop.current.run(until: Date().addingTimeInterval(0.05)) }
        return condition()
    }
    /// The fixture server as `host`: healthy unless `down` names it; every request is logged with its bearer.
    private func serve(down: @escaping () -> Set<String>, log: Log, environment: [String: String] = [:]) {
        R3HTTP.reset { request in
            let host = request.url!.host ?? "", path = request.url!.path
            log.add("\(host) \(path) \(request.value(forHTTPHeaderField: "Authorization") ?? "-")")
            if down().contains(host) { return (503, ["message": "\(host) is starting up."]) }
            switch path {
            case "/.well-known/t3/environment":
                return (200, ["environmentId": environment[host] ?? "fixture", "label": "Fixture", "orchestrationProtocolVersion": 2, "capabilities": ["connectionProbe": true]])
            case "/api/auth/session": return (200, ["authenticated": true, "scopes": ["orchestration:read"]])
            case "/api/auth/websocket-ticket": return (200, ["ticket": "fixture-ticket"])
            default: return (404, [:])
            }
        }
    }
    /// This Mac's first private IPv4 (10/8, 172.16/12, 192.168/16, not loopback): what the server reports as its LAN
    /// address (DirectEndpoints.ts). The fixture socket listens on every interface, so the address reaches it.
    private func privateIPv4() -> String? {
        let addresses = (try? T3NetworkInterfaces.system())?.values.flatMap { $0 } ?? []
        return addresses.first(where: { address in
            let parts = address.address.split(separator: ".").compactMap { Int($0) }
            guard address.family == "IPv4", !address.isInternal, parts.count == 4 else { return false }
            return parts[0] == 10 || (parts[0] == 172 && (16...31).contains(parts[1])) || (parts[0] == 192 && parts[1] == 168)
        })?.address
    }
    private func route(_ origin: String, credential: String? = nil, learned: Bool = false) -> [String: Any] {
        ["id": learned ? "learned:fixture:\(origin)@\(credential ?? origin)" : origin, "origin": origin, "kind": "", "learned": learned, "credential": credential ?? origin]
    }

    func testEntriesSavedBeforeRoutesJoinOneEnvironmentWithOneRouteEach() {
        let old: [[String: Any]] = [["origin": "http://127.0.0.1:1", "environmentId": "A", "label": "Desk", "enabled": true],
                                    ["origin": "http://10.0.0.2:1", "environmentId": "B", "label": "Other"],
                                    ["origin": "https://desk.tail1.ts.net", "environmentId": "A", "label": "Desk"]]
        let migrated = T3SavedEnvironments.migrated(old)
        XCTAssertTrue(migrated.changed)
        XCTAssertEqual(migrated.list.map { $0["environmentId"] as? String }, ["A", "B"])
        XCTAssertEqual((migrated.list[0]["routes"] as? [[String: Any]])?.map { $0["id"] as? String }, ["http://127.0.0.1:1", "https://desk.tail1.ts.net"])
        XCTAssertEqual((migrated.list[0]["routes"] as? [[String: Any]])?.map { $0["credential"] as? String }, ["http://127.0.0.1:1", "https://desk.tail1.ts.net"],
                       "Each migrated route keeps reading its own Keychain item")
        XCTAssertFalse(T3SavedEnvironments.migrated(migrated.list).changed)
    }

    func testPairingTheSameMachineAtASecondAddressAddsARouteNotARow() throws {
        saved.remember(origin: "http://127.0.0.1:2", descriptor: ["environmentId": "A", "label": "Desk"])
        saved.remember(origin: "http://localhost:2", descriptor: ["environmentId": "A", "label": "Desk"])
        saved.remember(origin: "http://localhost:2", descriptor: ["environmentId": "A", "label": "Desk again"])
        XCTAssertEqual(saved.all.count, 1)
        XCTAssertEqual(saved.all[0]["origin"] as? String, "http://127.0.0.1:2", "The home origin stays")
        XCTAssertEqual((saved.all[0]["routes"] as? [[String: Any]])?.map { $0["origin"] as? String }, ["http://127.0.0.1:2", "http://localhost:2"])
        XCTAssertEqual(saved.entry(origin: "http://localhost:2/")?["environmentId"] as? String, "A")
    }

    func testRouteEditsKeepTheHomeAndForgetARemovedRoutesTokenUnlessBorrowed() throws {
        let transport = transport(); defer { transport.destroy() }
        saved.remember(origin: "http://127.0.0.1:3", descriptor: ["environmentId": "fixture", "label": "Desk"])
        saved.remember(origin: "http://localhost:3", descriptor: ["environmentId": "fixture", "label": "Desk"])
        try credentials.save("one", origin: "http://127.0.0.1:3", environment: "fixture")
        try credentials.save("two", origin: "http://localhost:3", environment: "fixture")
        let refused = perform(transport, ["op": "setRoutes", "environmentId": "fixture", "routes": [route("http://localhost:3"), route("http://localhost:3")]])
        XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "Arguments")
        // Reorder, and a learned route that borrows the first route's token.
        _ = perform(transport, ["op": "setRoutes", "environmentId": "fixture", "routes": [route("http://localhost:3"), route("http://127.0.0.1:3"),
                                                                                      route("https://desk.tail1.ts.net", credential: "http://localhost:3", learned: true)]])
        XCTAssertEqual(saved.all[0]["origin"] as? String, "http://127.0.0.1:3", "Reordering keeps the home origin")
        // Removing the home route moves the home to the preferred paired route and forgets its token.
        _ = perform(transport, ["op": "setRoutes", "environmentId": "fixture", "routes": [route("http://localhost:3"), route("https://desk.tail1.ts.net", credential: "http://localhost:3", learned: true)]])
        XCTAssertEqual(saved.all[0]["origin"] as? String, "http://localhost:3")
        XCTAssertNil(try credentials.read(origin: "http://127.0.0.1:3", environment: "fixture"))
        XCTAssertEqual(try credentials.read(origin: "http://localhost:3", environment: "fixture"), "two", "A borrowed token stays")
    }

    func testTheWalkSkipsAnotherMachineAtASavedAddressAndNeverSendsItACredential() throws {
        let socket = try R3Socket(), port = socket.port, log = Log()
        serve(down: { [] }, log: log, environment: ["localhost": "someone-else"])
        saved.remember(origin: "http://localhost:\(port)", descriptor: ["environmentId": "fixture", "label": "Desk"])
        saved.remember(origin: "http://127.0.0.1:\(port)", descriptor: ["environmentId": "fixture", "label": "Desk"])
        try credentials.save("token-localhost", origin: "http://localhost:\(port)", environment: "fixture")
        try credentials.save("token-loopback", origin: "http://127.0.0.1:\(port)", environment: "fixture")
        let transport = transport(); defer { transport.destroy() }
        let opened = perform(transport, ["op": "connect", "origin": "http://localhost:\(port)", "credential": ""])
        let value = opened["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(value["state"] as? String, "connected", "\(opened)")
        XCTAssertEqual(value["activeRouteId"] as? String, "http://127.0.0.1:\(port)")
        XCTAssertEqual(value["origin"] as? String, "http://127.0.0.1:\(port)", "The route in use")
        XCTAssertEqual(value["homeOrigin"] as? String, "http://localhost:\(port)", "The saved environment's key stays")
        let leaked = log.all.filter { $0.hasPrefix("localhost ") && !$0.hasSuffix(" -") }
        XCTAssertEqual(leaked, [], "No credential reaches a route before its descriptor matched")
        XCTAssertEqual(saved.all.count, 1)
    }

    /// A learned plain-HTTP LAN route, as the server reports it (directEndpoints: a
    /// numeric private IPv4 over http), is walked like any direct route and connects with the token it borrows from the
    /// route it was learned over (routes.ts credentialConnectionId, resolver.ts), as the reference desktop does; the
    /// token goes out only after the address answered as this environment. The fixture socket listens on every
    /// interface, so this Mac's own private IPv4 reaches it; without one the test skips.
    func testALearnedPlainHttpLanRouteConnectsWithTheTokenItBorrows() throws {
        guard let lan = privateIPv4() else { throw XCTSkip("This Mac has no private IPv4 address.") }
        let socket = try R3Socket(), port = socket.port, log = Log()
        let home = "http://127.0.0.1:\(port)", learned = "http://\(lan):\(port)", learnedId = "learned:fixture:\(learned)@\(home)"
        // The paired loopback route is down (a stopped proxy); the learned LAN address answers.
        serve(down: { ["127.0.0.1"] }, log: log)
        saved.remember(origin: home, descriptor: ["environmentId": "fixture", "label": "Desk"])
        try credentials.save("token-home", origin: home, environment: "fixture")
        let transport = transport(); defer { transport.destroy() }
        _ = perform(transport, ["op": "setRoutes", "environmentId": "fixture", "routes": [route(home), route(learned, credential: home, learned: true)]])
        let opened = perform(transport, ["op": "connect", "origin": home, "credential": ""])
        let value = opened["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(value["state"] as? String, "connected", "\(opened) \(log.all)")
        XCTAssertEqual(value["activeRouteId"] as? String, learnedId)
        XCTAssertEqual(value["origin"] as? String, learned, "The route in use")
        XCTAssertEqual(value["homeOrigin"] as? String, home)
        let lanLines = log.all.filter { $0.hasPrefix("\(lan) ") }
        XCTAssertEqual(lanLines.first, "\(lan) /.well-known/t3/environment -", "The descriptor first, with no credential")
        XCTAssertTrue(lanLines.contains("\(lan) /api/auth/session Bearer token-home"), "The borrowed token: \(lanLines)")
        XCTAssertTrue(lanLines.contains("\(lan) /api/auth/websocket-ticket Bearer token-home"), "\(lanLines)")
        XCTAssertNil(try credentials.read(origin: learned, environment: "fixture"), "A learned route owns no Keychain item")
    }

    /// Learning a route that ranks above the one in use (a LAN address learned over a tailnet or public route)
    /// checks it at once with the borrowed token and moves the session to it, as supervisor.ts learnRoutesFrom
    /// requests the better-route check; the 60 s timer is not what moves it.
    func testALearnedRouteAboveTheOneInUseTakesOverAtOnce() throws {
        guard ProcessInfo.processInfo.environment["T3_ROUTE_CHECK_INTERVAL_MS"] == nil else { throw XCTSkip("Needs the 60 s default interval.") }
        guard let lan = privateIPv4() else { throw XCTSkip("This Mac has no private IPv4 address.") }
        let socket = try R3Socket(), port = socket.port, log = Log()
        // The route in use stands for a tailnet or public address; `localhost` reaches the same fixture socket.
        let home = "http://localhost:\(port)", learned = "http://\(lan):\(port)", learnedId = "learned:fixture:\(learned)@\(home)"
        serve(down: { [] }, log: log)
        saved.remember(origin: home, descriptor: ["environmentId": "fixture", "label": "Desk"])
        try credentials.save("token-home", origin: home, environment: "fixture")
        let transport = transport(); defer { transport.destroy() }
        let opened = perform(transport, ["op": "connect", "origin": home, "credential": ""])
        XCTAssertEqual((opened["value"] as? [String: Any])?["activeRouteId"] as? String, home, "\(opened)")
        // learnRoutes: the server reported its LAN address; insertRoute puts it ahead of the route in use.
        _ = perform(transport, ["op": "setRoutes", "environmentId": "fixture", "routes": [route(learned, credential: home, learned: true), route(home)]])
        XCTAssertTrue(until(5) { status(transport)["activeRouteId"] as? String == learnedId && status(transport)["state"] as? String == "connected" }, "\(status(transport)) \(log.all)")
        XCTAssertEqual(status(transport)["origin"] as? String, learned)
        XCTAssertTrue(log.all.contains("\(lan) /api/auth/session Bearer token-home"), "The preflight and the session use the borrowed token")
        XCTAssertEqual(log.all.first { $0.hasPrefix("\(lan) ") }, "\(lan) /.well-known/t3/environment -", "The descriptor first, with no credential")
    }

    func testAFallbackRouteConnectsAndTheBetterRouteTakesOverOnceItAnswers() throws {
        let socket = try R3Socket(), port = socket.port, log = Log()
        final class Down: @unchecked Sendable { var hosts: Set<String> = ["127.0.0.1"] }
        let down = Down()
        serve(down: { down.hosts }, log: log)
        saved.remember(origin: "http://127.0.0.1:\(port)", descriptor: ["environmentId": "fixture", "label": "Desk"])
        saved.remember(origin: "http://localhost:\(port)", descriptor: ["environmentId": "fixture", "label": "Desk"])
        try credentials.save("token-a", origin: "http://127.0.0.1:\(port)", environment: "fixture")
        try credentials.save("token-b", origin: "http://localhost:\(port)", environment: "fixture")
        let transport = transport(); defer { transport.destroy() }
        let opened = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(port)", "credential": ""])
        XCTAssertEqual((opened["value"] as? [String: Any])?["activeRouteId"] as? String, "http://localhost:\(port)", "\(opened)")
        // The preferred route answers again; returning to the app checks it (the 60 s timer does too).
        down.hosts = []
        transport.applicationActive()
        XCTAssertTrue(until(6) { status(transport)["activeRouteId"] as? String == "http://127.0.0.1:\(port)" && status(transport)["state"] as? String == "connected" }, "\(status(transport))")
        XCTAssertTrue(log.all.contains("127.0.0.1 /api/auth/session Bearer token-a"), "The preflight proves the credential before switching")
    }

    func testATransientFailureIsReportedOverABlockedRoute() throws {
        let socket = try R3Socket(), port = socket.port, log = Log()
        serve(down: { ["localhost"] }, log: log)
        saved.remember(origin: "http://127.0.0.1:\(port)", descriptor: ["environmentId": "fixture", "label": "Desk"])
        saved.remember(origin: "http://localhost:\(port)", descriptor: ["environmentId": "fixture", "label": "Desk"])
        // No token for the first route: blocked. The second is starting up: transient.
        let transport = transport(); defer { transport.destroy() }
        _ = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(port)", "credential": ""])
        XCTAssertTrue(until(4) { status(transport)["state"] as? String == "reconnecting" }, "\(status(transport))")
        XCTAssertTrue((status(transport)["message"] as? String ?? "").hasPrefix("localhost is starting up."), "\(status(transport))")
    }

    func testAddingARouteToAnotherMachineIsRefusedBeforeTheCodeIsSpent() throws {
        let log = Log()
        R3HTTP.reset { request in
            log.add(request.url!.path)
            let descriptor: [String: Any] = ["environmentId": "other", "label": "Studio", "orchestrationProtocolVersion": 2]
            let grant: [String: Any] = ["access_token": "x", "token_type": "Bearer"]
            return (200, request.url!.path == "/.well-known/t3/environment" ? descriptor : grant)
        }
        let transport = transport(); defer { transport.destroy() }
        let refused = perform(transport, ["op": "pairEnvironment", "origin": "http://127.0.0.1:14907", "credential": "PAIRCODE", "expectedEnvironmentId": "fixture"])
        XCTAssertEqual((refused["error"] as? [String: Any])?["message"] as? String, "That address reaches Studio, a different machine. Add it as its own environment instead.")
        let ssh = perform(transport, ["op": "pairEnvironment", "origin": "http://127.0.0.1:14907", "credential": "PAIRCODE", "expectedEnvironmentId": "fixture", "ssh": true])
        XCTAssertEqual((ssh["error"] as? [String: Any])?["message"] as? String, "That host reaches Studio, a different machine. Add it as its own environment instead.")
        XCTAssertFalse(log.all.contains("/oauth/token"), "The one-time code is not spent")
        XCTAssertEqual(saved.all.count, 0)
    }
}

// One environment, several routes: the route walk and the better-route check
// (T3 Code 1e2ecbd975 packages/client-runtime/src/connection/driver.ts
// connectOverRoutes and checkRoute, supervisor.ts checkBetterRoutes; commits
// 979ca66ced and 745c225f92; MIT, see LICENSE-T3). The app (connection-routes.ts)
// owns the route model and writes each environment's ordered routes to
// T3SavedEnvironments; one T3Transport walks them through a T3RouteState, beside
// the reconnect ladder (T3Reconnect in T3Protocol.swift).
//
// Port changes: the Effect services become one object the transport drives on its
// own serial queue, with native timers (issue X19: data sources have no clock).
// An SSH route's origin is its tunnel's loopback address, so its kind comes from the
// stored `kind`. A route this client's address policy refuses (plain HTTP to a
// non-loopback host, T3Endpoint) counts as silent: no credential ever reaches it.
// T3_ROUTE_CHECK_INTERVAL_MS (agent seam, like T3_SSH_COMMAND) shortens the 60 s check.
import Foundation

/// One saved route as the transport walks it.
struct T3Route {
    let id: String
    let origin: String
    let kind: String
    let learned: Bool
    /// The origin whose Keychain account holds this route's token.
    let credential: String
    /// Direct routes have a cheap unauthenticated check; SSH and T3 Connect do not.
    var direct: Bool { kind != "ssh" && kind != "relay" }

    init?(_ raw: [String: Any]) {
        guard let id = raw["id"] as? String, !id.isEmpty, let origin = raw["origin"] as? String, !origin.isEmpty else { return nil }
        self.id = id; self.origin = origin; kind = raw["kind"] as? String ?? ""; learned = raw["learned"] as? Bool == true
        credential = (raw["credential"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? origin
    }
    init(origin: String) {
        let text = T3SavedEnvironments.trimmed(origin)
        id = text; self.origin = text; kind = ""; learned = false; credential = text
    }
}

/// The result of an unauthenticated reachability check (driver.ts RouteCheck).
enum T3RouteCheck { case answered, silent, unchecked }

/// The routes of the environment one transport connects to: which to try next,
/// which one is in use, and when to look for a better one. Used only on `queue`.
final class T3RouteState {
    /// How long a direct route has to answer before it counts as unreachable from here.
    static let checkTimeout: TimeInterval = 2.5
    /// A better route that answered but then failed to connect is not tried again for this long.
    static let cooldown: TimeInterval = 300
    /// While connected over a fallback route, how often to look for a better one.
    static var checkInterval: TimeInterval {
        if let raw = ProcessInfo.processInfo.environment["T3_ROUTE_CHECK_INTERVAL_MS"], let ms = Double(raw), ms >= 100 { return ms / 1000 }
        return 60
    }
    /// Failures that say this route will not work as configured; any other failure may pass.
    static func blocks(_ failure: T3Failure) -> Bool { ["Authentication", "Credential", "Keychain", "Address", "Limit"].contains(failure.kind) }

    private let queue: DispatchQueue
    private let session: () -> URLSession
    private(set) var environmentId = ""
    private(set) var label = ""
    private(set) var home = ""
    private(set) var routes: [T3Route] = []
    // The walk in progress (connectOverRoutes).
    private(set) var walking = false
    private var walkEpoch = 0
    /// The walk's order: the saved order, with a better route being switched to first.
    private var order: [T3Route] = []
    private var results: [T3RouteCheck?] = []
    private var cursor = 0
    private var silent: [Int] = []
    private var silentCursor = 0
    private var waiter: ((T3Route?) -> Void)?
    private(set) var current: T3Route?
    /// The credential owner whose token the transport holds.
    var tokenOwner = ""
    private var transient: T3Failure?
    private var blocked: T3Failure?
    // The supervisor's side: the route in use, a better route being switched to, and cooldowns.
    private(set) var activeId = ""
    private var preferred: String?
    private var switchingTo: String?
    private var cooldowns: [String: Date] = [:]
    private var timer: DispatchSourceTimer?
    private var checking = false

    init(queue: DispatchQueue, session: @escaping () -> URLSession) { self.queue = queue; self.session = session }

    /// The saved environment one of whose routes is `origin`; an unsaved origin (a pairing) is its own one route.
    func load(origin: String, saved: T3SavedEnvironments) {
        if let entry = saved.entry(origin: origin) { adopt(entry) }
        else { environmentId = ""; label = ""; home = T3SavedEnvironments.trimmed(origin); routes = [T3Route(origin: origin)] }
        activeId = ""; preferred = nil; switchingTo = nil; current = nil
        walking = false; waiter = nil; walkEpoch += 1; timer?.cancel(); timer = nil
    }
    /// Re-reads the saved routes (learned routes and reorders arrive while connected).
    func reload(saved: T3SavedEnvironments) {
        guard !environmentId.isEmpty, let entry = saved.entry(environment: environmentId) else { return }
        adopt(entry)
    }
    private func adopt(_ entry: [String: Any]) {
        environmentId = entry["environmentId"] as? String ?? ""
        label = (entry["label"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? URL(string: entry["origin"] as? String ?? "")?.host ?? "The environment"
        home = T3SavedEnvironments.trimmed(entry["origin"] as? String ?? "")
        let parsed = (entry["routes"] as? [[String: Any]] ?? []).compactMap(T3Route.init)
        routes = parsed.isEmpty ? [T3Route(origin: home)] : parsed
    }
    var activeIndex: Int { routes.firstIndex { $0.id == activeId } ?? -1 }

    // MARK: The walk

    /// Starts a walk: every direct route is checked at once, a route waits only for its own check.
    func beginWalk(saved: T3SavedEnvironments) {
        reload(saved: saved)
        walkEpoch += 1; walking = true; cursor = 0; silent = []; silentCursor = 0; waiter = nil
        current = nil; transient = nil; blocked = nil
        order = routes
        if let wanted = preferred, let index = order.firstIndex(where: { $0.id == wanted }) { order.insert(order.remove(at: index), at: 0) }
        switchingTo = preferred; preferred = nil
        results = Array(repeating: nil, count: order.count)
        guard order.count > 1 else { results = [.unchecked]; return }
        let epoch = walkEpoch
        for (index, route) in order.enumerated() {
            guard route.direct else { results[index] = .unchecked; continue }
            check(route) { [weak self] outcome in
                guard let self, self.walkEpoch == epoch else { return }
                self.results[index] = outcome; self.pump()
            }
        }
    }
    /// The next route worth trying, in order; silent routes go last, since a check is not proof. nil when none is left.
    func next(_ completion: @escaping (T3Route?) -> Void) { waiter = completion; pump() }
    private func pump() {
        guard let waiting = waiter else { return }
        while cursor < order.count {
            guard let outcome = results[cursor] else { return }
            let index = cursor; cursor += 1
            if outcome == .silent { silent.append(index); continue }
            waiter = nil; current = order[index]; waiting(order[index]); return
        }
        if silentCursor < silent.count {
            let index = silent[silentCursor]; silentCursor += 1
            waiter = nil; current = order[index]; waiting(order[index]); return
        }
        waiter = nil; current = nil; waiting(nil)
    }
    /// A route failed: a transient error is reported over a blocked one.
    func record(_ failure: T3Failure) {
        if Self.blocks(failure) { if blocked == nil { blocked = failure } } else if transient == nil { transient = failure }
    }
    /// Whether this walk has more than its one route, so a failure moves on instead of ending it.
    var walksSeveral: Bool { walking && order.count > 1 }
    /// What the walk reports when no route connected.
    func exhausted() -> T3Failure {
        walking = false
        return transient ?? blocked ?? T3Failure(kind: "Network", message: "\(label.isEmpty ? "The environment" : label) did not answer on any saved route.")
    }
    /// The socket opened over `current`. A better route that did not land cools down.
    func landed(origin: String, environment: String, saved: T3SavedEnvironments) {
        walking = false; waiter = nil
        // A pairing's environment is saved only now; adopt its routes.
        if environmentId.isEmpty || environmentId != environment, let entry = saved.entry(environment: environment) { adopt(entry) } else { reload(saved: saved) }
        let address = T3SavedEnvironments.trimmed(origin)
        activeId = current?.id ?? routes.first(where: { $0.origin == address && !$0.learned })?.id ?? routes.first(where: { $0.origin == address })?.id ?? address
        if let wanted = switchingTo, wanted != activeId { cooldowns[wanted] = Date().addingTimeInterval(Self.cooldown) }
        switchingTo = nil
    }
    func stop() { walking = false; waiter = nil; walkEpoch += 1; activeId = ""; timer?.cancel(); timer = nil }

    // MARK: Checks

    /// checkRoute: the public descriptor answers as this environment; no credential is sent.
    func check(_ route: T3Route, completion: @escaping (T3RouteCheck) -> Void) {
        guard route.direct else { return completion(.unchecked) }
        guard let base = try? T3Endpoint.origin(route.origin), let url = try? T3Endpoint.path("/.well-known/t3/environment", at: base) else { return completion(.silent) }
        var request = URLRequest(url: url, timeoutInterval: Self.checkTimeout)
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        let expected = environmentId, queue = self.queue
        session().dataTask(with: request) { data, response, _ in
            let object = data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]
            let ok = ((response as? HTTPURLResponse)?.statusCode).map { (200..<300).contains($0) } ?? false
            let answered = ok && (object?["environmentId"] as? String).map { expected.isEmpty || $0 == expected } == true
            queue.async { completion(answered ? .answered : .silent) }
        }.resume()
    }

    /// preflight: the route answers as this environment and accepts this client's credential, without a socket.
    /// The credential is read only after the descriptor matched.
    func preflight(_ route: T3Route, credentials: T3Credentials, completion: @escaping (Bool) -> Void) {
        check(route) { [weak self] outcome in
            guard let self, outcome == .answered, let base = try? T3Endpoint.origin(route.origin),
                  let url = try? T3Endpoint.path("/api/auth/session", at: base),
                  let token = try? credentials.read(origin: route.credential, environment: self.environmentId), !token.isEmpty else { return completion(false) }
            var request = URLRequest(url: url, timeoutInterval: Self.checkTimeout * 2)
            request.setValue("application/json", forHTTPHeaderField: "Accept")
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            let queue = self.queue
            self.session().dataTask(with: request) { data, response, _ in
                let object = data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]
                let ok = ((response as? HTTPURLResponse)?.statusCode).map { (200..<300).contains($0) } ?? false
                queue.async { completion(ok && object?["authenticated"] as? Bool == true) }
            }.resume()
        }
    }

    /// While connected over a fallback route: check every interval (and when asked) for a better one.
    func schedule(_ fire: @escaping () -> Void) {
        timer?.cancel(); timer = nil
        guard activeIndex > 0 else { return }
        let interval = Self.checkInterval
        let source = DispatchSource.makeTimerSource(queue: queue)
        source.schedule(deadline: .now() + interval, repeating: interval)
        source.setEventHandler(handler: fire)
        timer = source; source.resume()
    }

    /// checkBetterRoutes: preflight the routes ranked above the one in use (not cooling down, direct only)
    /// and name the best that would connect. One check at a time; a trigger during a check is dropped.
    func checkBetter(credentials: T3Credentials, saved: T3SavedEnvironments, found: @escaping (String) -> Void) {
        reload(saved: saved)
        let current = activeIndex
        guard !checking, current > 0 else { return }
        let now = Date()
        let better = routes[0..<current].filter { $0.direct && (cooldowns[$0.id] ?? .distantPast) <= now }
        guard !better.isEmpty else { return }
        checking = true
        var passed = Array(repeating: false, count: better.count), left = better.count
        let active = activeId
        for (index, route) in better.enumerated() {
            preflight(route, credentials: credentials) { [weak self] ok in
                guard let self else { return }
                passed[index] = ok; left -= 1
                guard left == 0 else { return }
                self.checking = false
                // The check may outlive the session it was asked about.
                guard self.activeId == active, let best = passed.firstIndex(of: true) else { return }
                self.preferred = better[best].id
                found(better[best].id)
            }
        }
    }
}

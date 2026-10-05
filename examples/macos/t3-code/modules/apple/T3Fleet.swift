// Background environments (Settings → Connections, several switched on at once).
// The focused environment keeps T3Module's transport; every other switched-on
// saved environment gets its own T3Transport here, sharing the app's credential
// store and saved list. A request carrying `fleet: <origin\nenvironmentId>` is
// answered by that environment's transport; its changes announce "t3.fleet".
import Foundation

final class T3Fleet: @unchecked Sendable {
    private let lock = NSLock()
    private var transports: [String: T3Transport] = [:]
    private let credentials: T3Credentials
    private let saved: T3SavedEnvironments
    private let persistent: Bool
    private let configuration: URLSessionConfiguration
    private let changed: (String) -> Void
    private var alive = true
    let outdated: T3OutdatedHosts
    static let limit = 8

    init(persistent: Bool, credentials: T3Credentials, saved: T3SavedEnvironments,
         configuration: URLSessionConfiguration = .ephemeral, changed: @escaping (String) -> Void) {
        self.persistent = persistent; self.credentials = credentials; self.saved = saved
        self.configuration = configuration; self.changed = changed
        outdated = T3OutdatedHosts(credentials: credentials, saved: saved, configuration: configuration, changed: changed)
    }

    /// The fleet's own operations, then any transport operation for one key.
    func perform(_ key: String, _ request: [String: Any], completion: @escaping ([String: Any]) -> Void) {
        let op = request["op"] as? String ?? ""
        let failure = { (kind: String, message: String) in
            completion(["ok": false, "generation": 0, "error": ["kind": kind, "message": message, "uncertain": false]])
        }
        guard key.contains("\n"), key.count <= 2048 else { return failure("Arguments", "fleet requires an origin and environment key.") }
        // Hosts too old for this client (upstream 22e9d35613): probe, pair and update them without a session.
        if op.hasPrefix("fleetOutdated") { return outdated.perform(op, key, request, completion: completion) }
        if op == "fleetStop" {
            lock.lock(); let transport = transports.removeValue(forKey: key); lock.unlock()
            transport?.destroy()
            if transport != nil { changed("t3.fleet") }
            return completion(["ok": true, "generation": 0, "value": ["stopped": transport != nil]])
        }
        lock.lock()
        guard alive else { lock.unlock(); return failure("Closed", "The window was closed.") }
        var transport = transports[key]
        if transport == nil {
            // Only a connect may open a background environment; reads of an
            // unknown key report it disconnected instead of creating it.
            guard op == "connect" else {
                lock.unlock()
                if op == "status" {
                    return completion(["ok": true, "generation": 0, "value": ["state": "disconnected", "origin": "", "environmentId": "",
                                                                               "message": "", "descriptor": [:], "failureKind": "", "traceId": ""]])
                }
                return failure("Disconnected", "That environment is not connected.")
            }
            guard transports.count < Self.limit else { lock.unlock(); return failure("Limit", "Too many environments are switched on.") }
            let changed = self.changed
            let created = T3Transport(persistent: persistent, configuration: configuration.copy() as! URLSessionConfiguration,
                                      credentials: credentials, savedEnvironments: saved, remembersOrigin: false,
                                      changed: { _ in changed("t3.fleet") })
            transports[key] = created
            transport = created
        }
        lock.unlock()
        var forwarded = request
        forwarded.removeValue(forKey: "fleet")
        transport!.perform(forwarded, completion: completion)
    }

    /// Keys with an open background transport.
    var keys: [String] { lock.lock(); defer { lock.unlock() }; return Array(transports.keys) }

    func destroy() {
        lock.lock(); alive = false; let all = Array(transports.values); transports.removeAll(); lock.unlock()
        for transport in all { transport.destroy() }
        outdated.destroy()
    }
}

// MARK: - Outdated hosts (upstream 22e9d35613 compatibility.ts, onboarding.ts, outdatedHostUpdate.ts)
//
// A host whose orchestration protocol is older than this client's cannot open a
// session, but one that can update itself is still paired (saved switched off)
// and can be updated from here: a bare socket without the protocol gate carries
// only the self-update RPCs, whose wire shape has not changed across protocol
// versions. Then the descriptor is polled until the host answers protocol 2,
// and the environment is switched back on. Jobs run on this object's queue;
// TypeScript reads them with fleetOutdatedJobs whenever "t3.fleet" changes.
//   fleetOutdatedProbe   (key origin\\nid)  the unauthenticated descriptor
//   fleetOutdatedPair    (key origin\\n)    pair a self-updatable outdated host
//   fleetOutdatedUpdate  (key origin\\nid)  start an update job { targetVersion, fromVersion }
//   fleetOutdatedJobs    (any key)          every job { status, stage, fromVersion, targetVersion, message, resultVersion, label }
//   fleetOutdatedAck     (key origin\\nid)  forget a finished job
final class T3OutdatedHosts: @unchecked Sendable {
    static let protocolVersion = 2
    static let restartTimeout: TimeInterval = 240
    static let socketOpenTimeout: TimeInterval = 15
    private let queue = DispatchQueue(label: "t3.fleet.outdated")
    private let credentials: T3Credentials
    private let saved: T3SavedEnvironments
    private let session: URLSession
    private let changed: (String) -> Void
    private var jobs: [String: [String: Any]] = [:]
    private var sockets: [String: URLSessionWebSocketTask] = [:]
    private var alive = true
    /// Test seam: how long the descriptor poll waits between attempts.
    var pollInterval: TimeInterval = 1
    var restartTimeout: TimeInterval = T3OutdatedHosts.restartTimeout

    init(credentials: T3Credentials, saved: T3SavedEnvironments, configuration: URLSessionConfiguration, changed: @escaping (String) -> Void) {
        self.credentials = credentials; self.saved = saved; self.changed = changed
        session = URLSession(configuration: configuration.copy() as! URLSessionConfiguration)
    }

    typealias Completion = ([String: Any]) -> Void
    private func reply(_ completion: Completion, _ value: Any) { completion(["ok": true, "generation": 0, "value": value]) }
    private func reply(_ completion: Completion, failure: T3Failure) { completion(["ok": false, "generation": 0, "error": failure.json]) }

    /// compatibility.ts canSelfUpdate: a self-update capability, and a desktop-managed one only with desktop app updates.
    static func canSelfUpdate(_ descriptor: [String: Any]) -> Bool {
        let capabilities = descriptor["capabilities"] as? [String: Any] ?? [:]
        guard let selfUpdate = capabilities["serverSelfUpdate"] as? String, !selfUpdate.isEmpty else { return false }
        return selfUpdate != "desktop-managed" || capabilities["desktopAppUpdate"] as? Bool == true
    }
    static func protocolOf(_ descriptor: [String: Any]) -> Int { descriptor["orchestrationProtocolVersion"] as? Int ?? 1 }
    /// orchestrationProtocolCompatibilityError's detail, by direction; nil when compatible.
    static func compatibility(_ descriptor: [String: Any], label: String) -> (message: String, serverUpdateRequired: Bool)? {
        let version = protocolOf(descriptor)
        if version == protocolVersion { return nil }
        if version > protocolVersion {
            return ("This client is not supported by this server. Update your app or use a compatible release to connect to \(label).", false)
        }
        return ("This client requires a newer server. Update T3 Code on \(label) to connect.", canSelfUpdate(descriptor))
    }

    func perform(_ op: String, _ key: String, _ request: [String: Any], completion: @escaping Completion) {
        queue.async { [self] in
            guard alive else { return reply(completion, failure: T3Failure(kind: "Closed", message: "The window was closed.")) }
            if op == "fleetOutdatedJobs" { return reply(completion, ["jobs": jobs]) }
            let parts = key.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false).map(String.init)
            let environment = parts.count > 1 ? parts[1] : ""
            let origin: URL
            do { origin = try T3Endpoint.origin(parts[0]) } catch { return reply(completion, failure: error as? T3Failure ?? T3Failure(kind: "Address", message: "The server address is invalid.")) }
            switch op {
            case "fleetOutdatedProbe":
                descriptor(origin) { [self] result in
                    switch result { case .success(let value): reply(completion, value); case .failure(let failure): reply(completion, failure: failure) }
                }
            case "fleetOutdatedPair": pair(origin, request["credential"] as? String ?? "", completion: completion)
            case "fleetOutdatedUpdate":
                guard !environment.isEmpty else { return reply(completion, failure: T3Failure(kind: "Arguments", message: "Choose a saved environment to update.")) }
                guard let target = (request["targetVersion"] as? String)?.trimmingCharacters(in: .whitespaces), !target.isEmpty, target.count <= 128 else {
                    return reply(completion, failure: T3Failure(kind: "Arguments", message: "Choose a version to update to."))
                }
                let jobKey = "\(origin.absoluteString)\n\(environment)"
                // Single-flight per environment (createOutdatedServerUpdateCommand concurrency).
                if jobs[jobKey]?["status"] as? String == "running" { return reply(completion, ["started": false]) }
                jobs[jobKey] = ["status": "running", "stage": "downloading", "fromVersion": request["fromVersion"] as? String ?? target,
                                "targetVersion": target, "message": "", "resultVersion": "", "label": request["label"] as? String ?? ""]
                changed("t3.fleet")
                reply(completion, ["started": true])
                update(jobKey, origin: origin, environment: environment, target: target)
            case "fleetOutdatedAck":
                let jobKey = "\(origin.absoluteString)\n\(environment)"
                if jobs[jobKey]?["status"] as? String != "running" { jobs.removeValue(forKey: jobKey) }
                reply(completion, ["jobs": jobs])
            default: reply(completion, failure: T3Failure(kind: "Arguments", message: "Unsupported outdated-host operation."))
            }
        }
    }

    func destroy() {
        queue.async { [self] in
            alive = false
            for socket in sockets.values { socket.cancel(with: .goingAway, reason: nil) }
            sockets.removeAll()
            session.invalidateAndCancel()
        }
    }

    // MARK: HTTP

    private func http(_ path: String, at origin: URL, method: String = "GET", body: Data? = nil, bearer: String = "",
                      completion: @escaping (Result<[String: Any], T3Failure>) -> Void) {
        do {
            var request = URLRequest(url: try T3Endpoint.path(path, at: origin))
            request.httpMethod = method; request.httpBody = body; request.timeoutInterval = 15
            request.setValue("application/json", forHTTPHeaderField: "Accept")
            if body != nil { request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type") }
            if !bearer.isEmpty { request.setValue("Bearer \(bearer)", forHTTPHeaderField: "Authorization") }
            session.dataTask(with: request) { [weak self] data, response, error in
                guard let self else { return }
                self.queue.async {
                    if let error { return completion(.failure(T3Failure(kind: "Network", message: String(error.localizedDescription.prefix(300))))) }
                    let status = (response as? HTTPURLResponse)?.statusCode ?? 0, bytes = data ?? Data()
                    let decoded = bytes.count <= T3Wire.maximumBytes ? (try? JSONSerialization.jsonObject(with: bytes)) as? [String: Any] : nil
                    guard (200..<300).contains(status), let decoded else {
                        let reason = decoded?["message"] as? String ?? (decoded?["reason"] as? String == "invalid_credential" ? "The environment credential is invalid." : "The server returned HTTP \(status).")
                        return completion(.failure(T3Failure(kind: [401, 403].contains(status) ? "Authentication" : "HTTP", message: reason)))
                    }
                    completion(.success(decoded))
                }
            }.resume()
        } catch { queue.async { completion(.failure(error as? T3Failure ?? T3Failure(kind: "Address", message: "The server address is invalid."))) } }
    }
    private func descriptor(_ origin: URL, completion: @escaping (Result<[String: Any], T3Failure>) -> Void) {
        http("/.well-known/t3/environment", at: origin) { result in
            if case .success(let value) = result, (value["environmentId"] as? String ?? "").isEmpty {
                return completion(.failure(T3Failure(kind: "Protocol", message: "The server did not describe its environment.")))
            }
            completion(result)
        }
    }

    // MARK: Pairing (onboarding.ts preparePairingRegistration)

    private func pair(_ origin: URL, _ input: String, completion: @escaping Completion) {
        let credential: String
        do { credential = try T3Endpoint.credential(input, at: origin) } catch { return reply(completion, failure: error as? T3Failure ?? T3Failure(kind: "Credential", message: "Enter a pairing code.")) }
        guard !credential.isEmpty else { return reply(completion, failure: T3Failure(kind: "Credential", message: "Enter a pairing code.")) }
        descriptor(origin) { [self] result in
            guard case .success(let descriptor) = result else {
                if case .failure(let failure) = result { reply(completion, failure: failure) }
                return
            }
            let environment = descriptor["environmentId"] as! String
            let label = descriptor["label"] as? String ?? origin.host ?? "this environment"
            guard let blocked = Self.compatibility(descriptor, label: label) else {
                return reply(completion, failure: T3Failure(kind: "Compatible", message: "\(label) can pair normally."))
            }
            // Only an outdated host that can update itself is saved; anything else stays refused.
            guard blocked.serverUpdateRequired else { return reply(completion, failure: T3Failure(kind: "Protocol", message: blocked.message)) }
            let body = T3Endpoint.form([
                "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange", "subject_token": credential,
                "subject_token_type": "urn:t3:params:oauth:token-type:environment-bootstrap",
                "requested_token_type": "urn:ietf:params:oauth:token-type:access_token",
                "scope": "orchestration:read orchestration:operate review:write",
                "client_label": "Exact T3 for Mac", "client_device_type": "desktop", "client_os": "macos",
            ])
            http("/oauth/token", at: origin, method: "POST", body: body) { [self] grant in
                switch grant {
                case .failure(let failure):
                    reply(completion, failure: T3Failure(kind: failure.kind, message: failure.message.replacingOccurrences(of: credential, with: "[redacted]")))
                case .success(let grant):
                    guard grant["token_type"] as? String == "Bearer", let access = grant["access_token"] as? String, !access.isEmpty else {
                        return reply(completion, failure: T3Failure(kind: "Protocol", message: "The server did not issue a bearer access token."))
                    }
                    do { try credentials.save(access, origin: origin.absoluteString, environment: environment) }
                    catch { return reply(completion, failure: error as? T3Failure ?? T3Failure(kind: "Credential", message: "The access token could not be saved.")) }
                    saved.remember(origin: origin.absoluteString, descriptor: descriptor)
                    saved.setEnabled(origin: origin.absoluteString, environment: environment, enabled: false)
                    changed("t3.fleet")
                    reply(completion, ["origin": origin.absoluteString, "environmentId": environment, "label": label,
                                       "serverVersion": descriptor["serverVersion"] as? String ?? "", "message": blocked.message, "serverUpdateRequired": true])
                }
            }
        }
    }

    // MARK: Update (outdatedHostUpdate.ts updateOutdatedHost)

    private func setJob(_ key: String, _ fields: [String: Any]) {
        guard var job = jobs[key] else { return }
        for (name, value) in fields { job[name] = value }
        jobs[key] = job
        changed("t3.fleet")
    }
    private func fail(_ key: String, _ message: String) {
        sockets.removeValue(forKey: key)?.cancel(with: .goingAway, reason: nil)
        setJob(key, ["status": "failed", "message": message])
    }

    private func update(_ key: String, origin: URL, environment: String, target: String) {
        descriptor(origin) { [self] result in
            guard alive else { return }
            guard case .success(let descriptor) = result else {
                if case .failure(let failure) = result { fail(key, failure.message) }
                return
            }
            let label = descriptor["label"] as? String ?? origin.host ?? "This environment"
            if (jobs[key]?["label"] as? String ?? "").isEmpty { setJob(key, ["label": label]) }
            guard descriptor["environmentId"] as? String == environment else { return fail(key, "\(label) is a different environment now. Pair with it again.") }
            guard Self.canSelfUpdate(descriptor) else { return fail(key, "Update T3 Code on \(label) manually; it cannot update itself.") }
            let capabilities = descriptor["capabilities"] as? [String: Any] ?? [:]
            if let from = descriptor["serverVersion"] as? String, !from.isEmpty, jobs[key]?["fromVersion"] as? String == target { setJob(key, ["fromVersion": from]) }
            let token: String
            do { token = try credentials.read(origin: origin.absoluteString, environment: environment) ?? "" } catch { token = "" }
            guard !token.isEmpty else { return fail(key, "Pair with \(label) again to update it.") }
            http("/api/auth/websocket-ticket", at: origin, method: "POST", body: Data(), bearer: token) { [self] ticket in
                guard alive else { return }
                switch ticket {
                case .failure(let failure): fail(key, failure.message)
                case .success(let value):
                    guard let ticket = value["ticket"] as? String, !ticket.isEmpty, var url = URLComponents(url: origin, resolvingAgainstBaseURL: false) else {
                        return fail(key, "The server did not issue a socket ticket.")
                    }
                    // The bare socket: no orchestrationProtocol query or header (resolver.ts prepareForUpdate).
                    url.scheme = url.scheme == "https" ? "wss" : "ws"; url.path = "/ws"
                    url.queryItems = [URLQueryItem(name: "wsTicket", value: ticket), URLQueryItem(name: "clientSurface", value: "desktop"),
                                      URLQueryItem(name: "clientOs", value: "macos"), URLQueryItem(name: "clientDeviceType", value: "desktop")]
                    var request = URLRequest(url: url.url!)
                    request.timeoutInterval = Self.socketOpenTimeout
                    let socket = session.webSocketTask(with: request)
                    socket.maximumMessageSize = T3Wire.maximumBytes
                    sockets[key] = socket
                    socket.resume()
                    runUpdate(key, socket: socket, origin: origin, label: label, target: target, capabilities: capabilities)
                }
            }
        }
    }

    /// One RPC over the bare socket: progress chunks call `progress`; the result arrives as `.success(exit value)`,
    /// a server failure as `.failure(message)`, and a dropped socket as `.dropped`.
    enum Outcome { case success(Any), failure(String), dropped(String) }
    private func call(_ socket: URLSessionWebSocketTask, id: String, method: String, payload: [String: Any], key: String,
                      progress: @escaping ([String: Any]) -> Void, done: @escaping (Outcome) -> Void) {
        var opened = false
        let request: [String: Any] = ["_tag": "Request", "id": id, "tag": method, "payload": payload, "headers": [] as [Any]]
        guard let text = try? T3Wire.encode(request) else { return done(.failure("The update request could not be encoded.")) }
        queue.asyncAfter(deadline: .now() + Self.socketOpenTimeout + 1) { [weak self] in
            guard let self, !opened, self.sockets[key] === socket else { return }
            done(.failure("The server WebSocket did not open."))
        }
        socket.send(.string(text)) { [weak self] error in
            guard let self else { return }
            self.queue.async {
                opened = true
                if let error { return done(.dropped(error.localizedDescription)) }
                self.receive(socket, id: id, key: key, progress: progress, done: done)
            }
        }
    }
    private func receive(_ socket: URLSessionWebSocketTask, id: String, key: String, progress: @escaping ([String: Any]) -> Void, done: @escaping (Outcome) -> Void) {
        socket.receive { [weak self] result in
            guard let self else { return }
            self.queue.async {
                guard self.alive, self.sockets[key] === socket else { return }
                let data: Data
                switch result {
                case .failure(let error): return done(.dropped(error.localizedDescription))
                case .success(.string(let text)): data = Data(text.utf8)
                case .success(.data(let bytes)): data = bytes
                case .success: return done(.failure("Unknown WebSocket message type."))
                }
                guard let frames = try? T3Wire.decode(data) else { return done(.failure("The server sent an invalid RPC frame.")) }
                for frame in frames {
                    let tag = frame["_tag"] as? String ?? ""
                    if tag == "Ping" { socket.send(.string(#"{"_tag":"Pong"}"#)) { _ in }; continue }
                    if tag == "Pong" { continue }
                    if tag == "Defect" || tag == "ClientProtocolError" { return done(.failure(T3Wire.failure(frame).message)) }
                    guard T3Wire.identifier(frame["requestId"]) == id else { continue }
                    if tag == "Chunk" {
                        for value in frame["values"] as? [Any] ?? [] { if let event = value as? [String: Any] { progress(event) } }
                        if let ack = try? T3Wire.encode(["_tag": "Ack", "requestId": frame["requestId"]!]) { socket.send(.string(ack)) { _ in } }
                        continue
                    }
                    if tag == "Exit", let exit = frame["exit"] as? [String: Any] {
                        if exit["_tag"] as? String == "Success" { return done(.success(exit["value"] ?? NSNull())) }
                        return done(.failure(Self.updateFailure(exit)))
                    }
                }
                self.receive(socket, id: id, key: key, progress: progress, done: done)
            }
        }
    }
    /// ServerSelfUpdateError's message is a getter (`Server update failed: ${reason}`), not a wire field.
    static func updateFailure(_ exit: [String: Any]) -> String {
        let failure = T3Wire.failure(exit)
        if failure.kind == "ServerSelfUpdateError", !failure.reason.isEmpty, failure.message.hasPrefix("The server refused") { return "Server update failed: \(failure.reason)" }
        return failure.message
    }

    private func runUpdate(_ key: String, socket: URLSessionWebSocketTask, origin: URL, label: String, target: String, capabilities: [String: Any]) {
        let selfUpdate = capabilities["serverSelfUpdate"] as? String ?? ""
        let finish = { [self] (result: [String: Any]) in
            let commitToken = result["desktopUpdateToken"] as? String ?? ""
            guard result["method"] as? String == "desktop-app", !commitToken.isEmpty else { return resume(key, origin: origin, label: label) }
            // The commit relaunches the desktop app, so a dropped socket is success.
            call(socket, id: "2", method: "server.commitDesktopUpdate", payload: ["requestId": commitToken], key: key, progress: { _ in }) { [self] outcome in
                if case .failure(let message) = outcome { return fail(key, message) }
                resume(key, origin: origin, label: label)
            }
        }
        if capabilities["serverSelfUpdateProgress"] as? Bool == true {
            var terminal: [String: Any]?
            call(socket, id: "1", method: "server.updateServerWithProgress", payload: ["targetVersion": target], key: key, progress: { [self] event in
                if event["type"] as? String == "complete", let result = event["result"] as? [String: Any] { terminal = result }
                else if event["type"] as? String == "progress", let stage = event["stage"] as? String, ["downloading", "installing"].contains(stage) { setJob(key, ["stage": stage]) }
            }) { [self] outcome in
                // resolveServerUpdateProgressResult: the terminal event wins over a lost handoff.
                switch outcome {
                case .success, .dropped:
                    if let terminal { return finish(terminal) }
                    if case .dropped(let message) = outcome { return fail(key, message) }
                    fail(key, "The t3@\(target) update ended before the server accepted the restart.")
                case .failure(let message): fail(key, message)
                }
            }
        } else {
            call(socket, id: "1", method: "server.updateServer", payload: ["targetVersion": target], key: key, progress: { _ in }) { [self] outcome in
                switch outcome {
                case .success(let value): finish(value as? [String: Any] ?? ["targetVersion": target, "method": selfUpdate])
                // Older servers can drop the socket before acknowledging the restart.
                case .dropped(let message):
                    if selfUpdate == "boot-service" || selfUpdate == "respawn" { return finish(["targetVersion": target, "method": selfUpdate]) }
                    fail(key, message)
                case .failure(let message): fail(key, message)
                }
            }
        }
    }

    private func resume(_ key: String, origin: URL, label: String) {
        sockets.removeValue(forKey: key)?.cancel(with: .goingAway, reason: nil)
        setJob(key, ["stage": "resuming"])
        let deadline = Date().addingTimeInterval(restartTimeout)
        func poll() {
            descriptor(origin) { [self] result in
                guard alive, jobs[key]?["status"] as? String == "running" else { return }
                if case .success(let descriptor) = result, Self.protocolOf(descriptor) == Self.protocolVersion {
                    let environment = key.split(separator: "\n", maxSplits: 1).last.map(String.init) ?? ""
                    saved.remember(origin: origin.absoluteString, descriptor: descriptor)
                    saved.setEnabled(origin: origin.absoluteString, environment: environment, enabled: true)
                    return setJob(key, ["status": "done", "resultVersion": descriptor["serverVersion"] as? String ?? ""])
                }
                guard Date() < deadline else { return fail(key, "\(label) did not come back on a compatible T3 Code version.") }
                queue.asyncAfter(deadline: .now() + pollInterval) { poll() }
            }
        }
        poll()
    }
}

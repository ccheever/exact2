import Foundation

/// A serial owner keeps wire delivery lossless. Topic invalidations may coalesce;
/// the underlying journal cannot. All completion closures run on this queue.
final class T3Transport: NSObject, URLSessionWebSocketDelegate, @unchecked Sendable {
    typealias Completion = ([String: Any]) -> Void
    private struct Pending {
        let completion: Completion
        let deadline: Date
        /// The app's trace id: a status read lists the traces still pending (r3-protocol-reader.ts).
        var trace: Int? = nil
    }
    private let queue = DispatchQueue(label: "com.exact.t3code.transport")
    private let changed: (String) -> Void
    private let credentials: T3Credentials
    private let savedEnvironments: T3SavedEnvironments
    private let persistent: Bool
    /// The focused transport restores its origin at launch; background
    /// environment transports (T3Fleet) never replace that saved origin.
    private let remembersOrigin: Bool
    /// Where the focused origin is remembered (`originKey`). The app's own domain; a test passes a suite of its own.
    private let defaults: UserDefaults
    private static let originKey = "t3.server.origin"
    private let preferencesURL: URL?
    private var session: URLSession!
    private var socket: URLSessionWebSocketTask?
    private var httpTasks: [Int: URLSessionDataTask] = [:]
    private var pending: [String: Pending] = [:]
    private var streams: [String: String] = [:] // request id -> app subscription key
    /// Same-session resubscribes after a stream failure (rpc/client.ts): consecutive
    /// failures per key, and the failed subscription whose `_retryDue` is scheduled.
    private var streamRetries: [String: Int] = [:]
    private var failedStreams: [String: String] = [:]
    private var inbox = T3Inbox()
    private var transfers = T3Transfers()
    private var outbox: [String] = []
    private var outboxBytes = 0
    private var sending = false
    private var generation = 0
    private var serial = 0
    private var origin: URL?
    private var token = ""
    private var privateValues: [String] = []
    private var descriptor: [String: Any] = [:]
    private var state = "disconnected"
    private var message = "Connect to your T3 server."
    private var alive = true
    private var wantsConnection = false
    /// Consecutive failed attempts (T3Reconnect.delay's ladder); reset only by a stable connection or a retry.
    private var failures = 0
    private var connectedAt: Date?
    private var everConnected = false
    /// A health check of the live socket (explicit retry, foreground, offline report): its RPC id,
    /// deadline and who waits. Unanswered when the connection then fails, the reconnect skips the wait.
    private var probing: (id: String, deadline: Date, waiters: [Completion])?
    private var probeUnanswered = false
    private let random: () -> Double
    private var signals: T3Signals?
    private var reconnect: DispatchWorkItem?
    private var tick: DispatchSourceTimer?
    private var lastPong = Date()
    private var nextPing = Date()
    private var opening: Completion?
    // The last connection failure, for the Environments row: its kind decides
    // "Client not supported", and a server trace ID offers "Copy trace ID".
    private var failureKind = ""
    private var failureTrace = ""
    private var lastHTTPTrace = ""

    init(persistent: Bool, dataDirectory: URL? = nil, configuration: URLSessionConfiguration = .ephemeral,
         credentials: T3Credentials? = nil, savedEnvironments: T3SavedEnvironments? = nil, remembersOrigin: Bool = true,
         defaults: UserDefaults = .standard, random: @escaping () -> Double = { Double.random(in: 0..<1) }, signals: Bool = true,
         changed: @escaping (String) -> Void) {
        self.persistent = persistent; self.changed = changed; self.remembersOrigin = remembersOrigin; self.defaults = defaults; self.random = random
        preferencesURL = dataDirectory?.appendingPathComponent("t3-code.json", isDirectory: false)
        self.credentials = credentials ?? T3Credentials(persistent: persistent)
        self.savedEnvironments = savedEnvironments ?? T3SavedEnvironments(persistent: persistent)
        super.init()
        // No file/network work during init. UserDefaults is read on first status.
        let config = configuration
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.urlCache = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.timeoutIntervalForRequest = 15
        config.timeoutIntervalForResource = 30
        session = URLSession(configuration: config, delegate: self, delegateQueue: nil)
        if signals {
            self.signals = T3Signals(queue: queue, active: { [weak self] in self?.applicationActive() },
                                     network: { [weak self] online in self?.networkChanged(online: online) })
        }
    }

    func perform(_ request: [String: Any], completion: @escaping Completion) {
        queue.async { [self] in
            guard alive else { return finish(completion, failure: T3Failure(kind: "Closed", message: "The window was closed.")) }
            do {
                if ["http", "request", "subscribe", "unsubscribe", "events", "ack", "readChunk", "releaseChunk"].contains(request["op"] as? String ?? ""),
                   let expected = request["generation"] as? Int, expected != generation {
                    throw T3Failure(kind: "stale", message: "The connection changed before this operation was sent.")
                }
                switch request["op"] as? String {
                case "status":
                    restoreOrigin(); finish(completion, value: status())
                case "ids":
                    let count = request["count"] as? Int ?? 1
                    guard (1...16).contains(count) else { throw T3Failure(kind: "Arguments", message: "ids requires a count from 1 to 16.") }
                    finish(completion, value: (0..<count).map { _ in UUID().uuidString.lowercased() })
                case "readPreferences": finish(completion, value: ["text": try readPreferences()])
                case "writePreferences":
                    guard let text = request["text"] as? String else { throw arguments("writePreferences requires text.") }
                    try writePreferences(text); finish(completion, value: [:])
                case "connect": try connect(request, completion: completion)
                case "retry": try retryNow(request, completion: completion)
                case "disconnect":
                    wantsConnection = false; reconnect?.cancel(); reconnect = nil
                    retire(T3Failure(kind: "Disconnected", message: "Disconnected from the server.", uncertain: true))
                    generation += 1; inbox.reset(); token = ""
                    if request["forget"] as? Bool == true, let origin, let environment = descriptor["environmentId"] as? String {
                        try credentials.forget(origin: origin.absoluteString, environment: environment)
                        savedEnvironments.forget(origin: origin.absoluteString, environment: environment)
                        forgotten(origin, wasFocused: true)
                    }
                    // r10-connect: a pairing that failed with nothing to go back to leaves no origin behind.
                    if request["abandon"] as? Bool == true { origin = nil; descriptor = [:] }
                    setStatus("disconnected", "Disconnected.")
                    finish(completion, value: status())
                case "environments": finish(completion, value: ["saved": savedEnvironments.all])
                case "connectionPreferences": finish(completion, value: ["text": savedEnvironments.preferences])
                case "setConnectionPreferences":
                    guard let text = request["text"] as? String else { throw arguments("setConnectionPreferences requires text.") }
                    try savedEnvironments.setPreferences(text); finish(completion, value: ["text": text])
                case "setEnvironmentEnabled":
                    guard let raw = request["origin"] as? String, let environment = request["environmentId"] as? String, !environment.isEmpty,
                          let enabled = request["enabled"] as? Bool else { throw arguments("setEnvironmentEnabled requires origin, environmentId and enabled.") }
                    let saved = try T3Endpoint.origin(raw)
                    guard savedEnvironments.setEnabled(origin: saved.absoluteString, environment: environment, enabled: enabled) else {
                        throw T3Failure(kind: "Missing", message: "That environment is no longer saved on this device.")
                    }
                    finish(completion, value: ["saved": savedEnvironments.all])
                case "forgetEnvironment":
                    // Forget one saved environment's pairing on this device; the
                    // active connection closes only when it is that environment.
                    guard let raw = request["origin"] as? String, let environment = request["environmentId"] as? String, !environment.isEmpty else {
                        throw arguments("forgetEnvironment requires origin and environmentId.")
                    }
                    let saved = try T3Endpoint.origin(raw)
                    try credentials.forget(origin: saved.absoluteString, environment: environment)
                    savedEnvironments.forget(origin: saved.absoluteString, environment: environment)
                    let focusedId = descriptor["environmentId"] as? String ?? ""
                    let focused = origin == saved && (focusedId == environment || focusedId.isEmpty)
                    if focused {
                        wantsConnection = false; reconnect?.cancel(); reconnect = nil
                        retire(T3Failure(kind: "Disconnected", message: "Disconnected from the server.", uncertain: true))
                        generation += 1; inbox.reset(); token = ""
                        setStatus("disconnected", "Disconnected.")
                    }
                    forgotten(saved, wasFocused: focused)
                    finish(completion, value: status())
                case "pairEnvironment": try pairEnvironment(request, completion: completion)
                case "http": try readHTTP(request, completion: completion)
                case "uploadAttachment": try uploadAttachment(request, completion: completion)
                case "request": try rpc(request, completion: completion)
                case "subscribe": try subscribe(request, completion: completion)
                case "unsubscribe":
                    guard let key = request["key"] as? String else { throw arguments("unsubscribe requires a key.") }
                    unsubscribe(key); finish(completion, value: [:])
                case "events": finish(completion, value: inbox.read(after: max(0, request["after"] as? Int ?? 0)))
                case "ack":
                    inbox.acknowledge(through: request["through"] as? Int ?? 0)
                    // The reader must also drain events that arrived after its
                    // last page but before this ACK, while notification was armed.
                    finish(completion, value: ["latest": inbox.latest])
                case "readChunk":
                    guard let id = request["id"] as? String, let index = request["index"] as? Int else { throw arguments("readChunk requires id and index.") }
                    finish(completion, value: try transfers.read(id: id, index: index))
                case "releaseChunk":
                    if let id = request["id"] as? String { transfers.release(id) }
                    finish(completion, value: [:])
                default: throw arguments("Unknown native operation.")
                }
            } catch { finish(completion, failure: failure(error)) }
        }
    }

    func destroy() {
        queue.async { [self] in
            guard alive else { return }
            alive = false; wantsConnection = false
            reconnect?.cancel(); reconnect = nil
            signals?.cancel(); signals = nil
            retire(T3Failure(kind: "Closed", message: "The window was closed.", uncertain: true))
            token = ""; privateValues.removeAll(); inbox.reset()
            session.invalidateAndCancel()
        }
    }

    private func arguments(_ text: String) -> T3Failure { T3Failure(kind: "Arguments", message: text) }
    private func readPreferences() throws -> String {
        guard let preferencesURL else { throw T3Failure(kind: "Persistence", message: "The app data directory is unavailable.") }
        do {
            let attributes = try FileManager.default.attributesOfItem(atPath: preferencesURL.path)
            guard (attributes[.size] as? NSNumber)?.intValue ?? 0 <= T3Wire.maximumBytes else {
                throw T3Failure(kind: "Limit", message: "Saved drafts and preferences are too large.")
            }
            let data = try Data(contentsOf: preferencesURL)
            guard let text = String(data: data, encoding: .utf8) else {
                throw T3Failure(kind: "Persistence", message: "Saved drafts and preferences are not valid UTF-8.")
            }
            return text
        } catch let error as CocoaError where error.code == .fileReadNoSuchFile { return "" }
        catch let error as T3Failure { throw error }
        catch { throw T3Failure(kind: "Persistence", message: "Could not read saved drafts and preferences: \(error.localizedDescription)") }
    }
    private func writePreferences(_ text: String) throws {
        guard let preferencesURL else { throw T3Failure(kind: "Persistence", message: "The app data directory is unavailable.") }
        let data = Data(text.utf8)
        guard data.count <= T3Wire.maximumBytes else { throw T3Failure(kind: "Limit", message: "Drafts and preferences are too large to save.") }
        do {
            try FileManager.default.createDirectory(at: preferencesURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: preferencesURL, options: .atomic)
        } catch { throw T3Failure(kind: "Persistence", message: "Could not save drafts and preferences: \(error.localizedDescription)") }
    }
    private func restoreOrigin() {
        guard origin == nil, persistent, remembersOrigin, let saved = defaults.string(forKey: Self.originKey) else { return }
        // The key only ever names a saved environment (it is written once a socket to a saved one opens). One that no
        // saved environment has — written by a build that left it behind a removal — is dropped, not restored.
        guard let url = try? T3Endpoint.origin(saved), isSaved(url) else { defaults.removeObject(forKey: Self.originKey); return }
        origin = url
    }
    private func isSaved(_ url: URL) -> Bool {
        savedEnvironments.all.contains { entry in (entry["origin"] as? String).flatMap { try? T3Endpoint.origin($0) } == url }
    }
    /// An environment was forgotten on this device (Remove in Settings → Connections, or `disconnect` with `forget`).
    /// Nothing may name it afterwards: not this transport's origin when it was the focused connection, and not the
    /// remembered origin `t3.server.origin`.
    ///
    /// The key is CLEARED, not repointed. The reference's Remove (ConnectionsSettings `removeSavedBackend` →
    /// client-runtime `EnvironmentRegistry.removeLocked`) deletes the registration, its token and its cached data, and
    /// keeps no record of a "current" environment that it could move to another: it picks no replacement. The clone's
    /// relaunch already chooses without the key when it is empty (`relaunchTarget`: this machine, then the first
    /// saved one, never a switched-off one), and the next socket that opens writes the key anew. A second saved
    /// environment at the same origin keeps the key, since the origin still names a saved environment.
    private func forgotten(_ gone: URL, wasFocused: Bool) {
        if wasFocused { origin = nil; descriptor = [:]; failureKind = ""; failureTrace = ""; lastHTTPTrace = "" }
        guard persistent, remembersOrigin, !isSaved(gone),
              let saved = defaults.string(forKey: Self.originKey), (try? T3Endpoint.origin(saved)) == gone else { return }
        defaults.removeObject(forKey: Self.originKey)
    }
    private func status() -> [String: Any] {
        ["state": state, "origin": origin?.absoluteString ?? "", "environmentId": descriptor["environmentId"] as? String ?? "",
         "message": message, "descriptor": descriptor, "failureKind": failureKind, "traceId": failureTrace,
         "traces": pending.values.compactMap { $0.trace }]
    }
    private func setStatus(_ next: String, _ text: String) {
        let cleaned = clean(text)
        guard next != state || cleaned != message else { return }
        state = next; message = cleaned
        if alive { changed("t3.status") }
    }
    private func clean(_ input: String) -> String {
        var result = input
        for secret in privateValues + (token.isEmpty ? [] : [token]) where !secret.isEmpty {
            result = result.replacingOccurrences(of: secret, with: "[redacted]")
        }
        // URLSession error descriptions can include the ticket URL.
        result = result.replacingOccurrences(of: "(?i)(wsTicket|token|access_token)=([^&\\s]+)", with: "$1=[redacted]", options: .regularExpression)
        return String(result.prefix(700))
    }
    private func failure(_ error: Error) -> T3Failure {
        if let error = error as? T3Failure { return T3Failure(kind: error.kind, message: clean(error.message), uncertain: error.uncertain, reason: error.reason, detail: clean(error.detail)) }
        return T3Failure(kind: "Network", message: clean(error.localizedDescription))
    }
    private func finish(_ completion: Completion, value: Any = NSNull()) {
        do { completion(["ok": true, "generation": generation, "value": try transfers.prepare(value)]) }
        catch { finish(completion, failure: failure(error)) }
    }
    private func finish(_ completion: Completion, failure: T3Failure) {
        completion(["ok": false, "generation": generation, "error": self.failure(failure).json])
    }

    private func connect(_ request: [String: Any], completion: @escaping Completion) throws {
        let newOrigin = try T3Endpoint.origin(request["origin"] as? String ?? "")
        let credential = try T3Endpoint.credential(request["credential"] as? String ?? "", at: newOrigin)
        wantsConnection = false; reconnect?.cancel(); reconnect = nil
        retire(T3Failure(kind: "Replaced", message: "The connection was replaced.", uncertain: true))
        origin = newOrigin; descriptor = [:]; token = ""
        failureKind = ""; failureTrace = ""; lastHTTPTrace = ""
        privateValues = credential.isEmpty ? [] : [credential]
        wantsConnection = true; failures = 0; everConnected = false; opening = completion
        // r9-connect: the origin is remembered once its socket opens; a failed pairing leaves nothing behind.
        start(credential: credential)
    }

    /// Pair another environment without touching the active connection: read its
    /// descriptor, exchange the one-time credential, and keep the token in Keychain.
    private func pairEnvironment(_ request: [String: Any], completion: @escaping Completion) throws {
        let target = try T3Endpoint.origin(request["origin"] as? String ?? "")
        let credential = try T3Endpoint.credential(request["credential"] as? String ?? "", at: target)
        guard !credential.isEmpty else { throw T3Failure(kind: "Credential", message: "Enter a pairing code.") }
        let secrets = [credential]
        let scrub = { (text: String) -> String in secrets.reduce(self.clean(text)) { $0.replacingOccurrences(of: $1, with: "[redacted]") } }
        func send(_ path: String, body: Data?, then: @escaping ([String: Any]) -> Void) {
            do {
                var req = URLRequest(url: try T3Endpoint.path(path, at: target))
                req.httpMethod = body == nil ? "GET" : "POST"; req.httpBody = body
                req.setValue("2", forHTTPHeaderField: "x-t3-orchestration-protocol")
                req.setValue("application/json", forHTTPHeaderField: "Accept")
                if body != nil { req.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type") }
                session.dataTask(with: req) { [weak self] data, response, error in
                    guard let self else { return }
                    self.queue.async {
                        guard self.alive else { return self.finish(completion, failure: T3Failure(kind: "Closed", message: "The window was closed.")) }
                        if let error { return self.finish(completion, failure: T3Failure(kind: "Network", message: scrub(error.localizedDescription))) }
                        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
                        let bytes = data ?? Data()
                        let decoded = bytes.count <= T3Wire.maximumBytes ? (try? JSONSerialization.jsonObject(with: bytes)) as? [String: Any] : nil
                        guard (200..<300).contains(status), let decoded else {
                            let reason = decoded?["message"] as? String ?? (decoded?["reason"] as? String == "invalid_credential" ? "The environment credential is invalid." : "The server returned HTTP \(status).")
                            return self.finish(completion, failure: T3Failure(kind: [401, 403].contains(status) ? "Authentication" : "HTTP", message: scrub(reason)))
                        }
                        then(decoded)
                    }
                }.resume()
            } catch { finish(completion, failure: failure(error)) }
        }
        send("/.well-known/t3/environment", body: nil) { [self] descriptor in
            guard let environment = descriptor["environmentId"] as? String, !environment.isEmpty else {
                return finish(completion, failure: T3Failure(kind: "Protocol", message: "The server did not identify its environment."))
            }
            // The message names the direction (compatibility.ts); T3Fleet's fleetOutdatedPair saves an
            // outdated host that can update itself, so this one never exchanges the credential.
            if let problem = T3Compatibility.problem(descriptor) { return finish(completion, failure: T3Failure(kind: "Protocol", message: problem.message)) }
            let body = T3Endpoint.form([
                "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange", "subject_token": credential,
                "subject_token_type": "urn:t3:params:oauth:token-type:environment-bootstrap",
                "requested_token_type": "urn:ietf:params:oauth:token-type:access_token",
                "scope": "orchestration:read orchestration:operate review:write",
                "client_label": "Exact T3 for Mac", "client_device_type": "desktop", "client_os": "macos",
            ])
            send("/oauth/token", body: body) { [self] grant in
                guard grant["token_type"] as? String == "Bearer", let access = grant["access_token"] as? String, !access.isEmpty else {
                    return finish(completion, failure: T3Failure(kind: "Protocol", message: "The server did not issue a bearer access token."))
                }
                do { try credentials.save(access, origin: target.absoluteString, environment: environment) }
                catch { return finish(completion, failure: failure(error)) }
                savedEnvironments.remember(origin: target.absoluteString, descriptor: descriptor)
                finish(completion, value: ["origin": target.absoluteString, "environmentId": environment, "label": descriptor["label"] as? String ?? ""])
            }
        }
    }

    private func start(credential: String = "") {
        guard wantsConnection, alive, let origin else { return }
        generation += 1; inbox.reset()
        let epoch = generation
        setStatus(failures == 0 && !everConnected ? "connecting" : "reconnecting", "Connecting to \(origin.host ?? "the server")…")
        http(path: "/.well-known/t3/environment", epoch: epoch, authorized: false) { [self] result in
            switch result {
            case .failure(let error): connectionFailed(error, epoch: epoch)
            case .success(let value):
                guard let object = value as? [String: Any], object["environmentId"] is String else {
                    return connectionFailed(T3Failure(kind: "Protocol", message: "The server did not identify its environment."), epoch: epoch)
                }
                if let problem = T3Compatibility.problem(object) {
                    // Keep the descriptor: the Environments row offers Update when the server can update itself.
                    descriptor = object
                    return connectionFailed(T3Failure(kind: "Protocol", message: problem.message), epoch: epoch)
                }
                let previousEnvironment = descriptor["environmentId"] as? String
                descriptor = object
                if credential.isEmpty {
                    do {
                        let environment = object["environmentId"] as! String
                        if token.isEmpty || previousEnvironment != environment {
                            token = try credentials.read(origin: origin.absoluteString, environment: environment) ?? ""
                        }
                        guard !token.isEmpty else { throw T3Failure(kind: "Credential", message: "Paste a pairing link or token from this T3 server.") }
                        validateSession(epoch: epoch)
                    } catch { connectionFailed(failure(error), epoch: epoch) }
                }
                else { exchange(credential, epoch: epoch) }
            }
        }
    }

    private func exchange(_ credential: String, epoch: Int) {
        let body = T3Endpoint.form([
            "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange",
            "subject_token": credential,
            "subject_token_type": "urn:t3:params:oauth:token-type:environment-bootstrap",
            "requested_token_type": "urn:ietf:params:oauth:token-type:access_token",
            "scope": "orchestration:read orchestration:operate review:write",
            "client_label": "Exact T3 for Mac", "client_device_type": "desktop", "client_os": "macos",
        ])
        http(path: "/oauth/token", method: "POST", raw: body, contentType: "application/x-www-form-urlencoded", epoch: epoch, authorized: false) { [self] result in
            switch result {
            case .failure(let error): connectionFailed(error, epoch: epoch)
            case .success(let value):
                guard let object = value as? [String: Any], object["token_type"] as? String == "Bearer",
                      let access = object["access_token"] as? String, !access.isEmpty else {
                    return connectionFailed(T3Failure(kind: "Protocol", message: "The server did not issue a bearer access token."), epoch: epoch)
                }
                token = access
                do { try credentials.save(access, origin: origin!.absoluteString, environment: descriptor["environmentId"] as! String) }
                catch { return connectionFailed(failure(error), epoch: epoch) }
                privateValues.removeAll()
                validateSession(epoch: epoch)
            }
        }
    }

    private func validateSession(epoch: Int) {
        http(path: "/api/auth/session", epoch: epoch) { [self] result in
            switch result {
            case .failure(let error): connectionFailed(error, epoch: epoch)
            case .success(let value):
                guard let auth = value as? [String: Any], auth["authenticated"] as? Bool == true else {
                    return connectionFailed(T3Failure(kind: "Authentication", message: "The saved session expired. Pair with the server again."), epoch: epoch)
                }
                openSocket(epoch: epoch)
            }
        }
    }

    private func openSocket(epoch: Int) {
        http(path: "/api/auth/websocket-ticket", method: "POST", epoch: epoch) { [self] result in
            switch result {
            case .failure(let error): connectionFailed(error, epoch: epoch)
            case .success(let value):
                guard let ticket = (value as? [String: Any])?["ticket"] as? String, !ticket.isEmpty,
                      var url = URLComponents(url: origin!, resolvingAgainstBaseURL: false) else {
                    return connectionFailed(T3Failure(kind: "Protocol", message: "The server did not issue a socket ticket."), epoch: epoch)
                }
                privateValues = [ticket]
                url.scheme = url.scheme == "https" ? "wss" : "ws"; url.path = "/ws"
                url.queryItems = [URLQueryItem(name: "wsTicket", value: ticket), URLQueryItem(name: "orchestrationProtocol", value: "2"),
                                  URLQueryItem(name: "clientSurface", value: "desktop"), URLQueryItem(name: "clientOs", value: "macos"),
                                  URLQueryItem(name: "clientDeviceType", value: "desktop")]
                var request = URLRequest(url: url.url!)
                request.timeoutInterval = 15
                request.setValue("2", forHTTPHeaderField: "x-t3-orchestration-protocol")
                let next = session.webSocketTask(with: request)
                next.maximumMessageSize = T3Wire.maximumBytes
                socket = next; next.resume()
                receive(next, epoch: epoch)
                queue.asyncAfter(deadline: .now() + 16) { [weak self, weak next] in
                    guard let self, let next, self.socket === next, self.generation == epoch,
                          self.state != "connected" else { return }
                    self.connectionFailed(T3Failure(kind: "Network", message: "The server WebSocket did not open."), epoch: epoch)
                }
            }
        }
    }

    private func http(path: String, method: String = "GET", raw: Data? = nil, contentType: String = "application/json",
                      epoch: Int, authorized: Bool = true, completion: @escaping (Result<Any, T3Failure>) -> Void) {
        do {
            guard let origin else { throw T3Failure(kind: "Disconnected", message: "Connect to a server first.") }
            var request = URLRequest(url: try T3Endpoint.path(path, at: origin))
            request.httpMethod = method; request.httpBody = raw
            request.setValue("2", forHTTPHeaderField: "x-t3-orchestration-protocol")
            request.setValue("application/json", forHTTPHeaderField: "Accept")
            if raw != nil { request.setValue(contentType, forHTTPHeaderField: "Content-Type") }
            if authorized { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
            let task = session.dataTask(with: request) { [weak self] data, response, error in
                guard let self else { return }
                self.queue.async {
                    self.httpTasks = self.httpTasks.filter { $0.value.state != .completed }
                    guard self.alive, self.generation == epoch else {
                        return completion(.failure(T3Failure(kind: "Replaced", message: "The connection changed during the request.")))
                    }
                    if let error { return completion(.failure(self.failure(error))) }
                    guard let response = response as? HTTPURLResponse else {
                        return completion(.failure(T3Failure(kind: "Network", message: "The server returned no HTTP response.")))
                    }
                    let bytes = data ?? Data()
                    guard bytes.count <= T3Wire.maximumBytes else {
                        return completion(.failure(T3Failure(kind: "Limit", message: "The server response is too large.")))
                    }
                    let decoded = bytes.isEmpty ? [:] : (try? JSONSerialization.jsonObject(with: bytes, options: [.fragmentsAllowed]))
                    guard (200..<300).contains(response.statusCode) else {
                        if let trace = (decoded as? [String: Any])?["traceId"] as? String { self.lastHTTPTrace = String(trace.prefix(200)) }
                        let reason = (decoded as? [String: Any])?["message"] as? String
                            ?? ((decoded as? [String: Any])?["reason"] as? String == "invalid_credential" ? "The environment credential is invalid." : nil)
                        let kind = [401, 403].contains(response.statusCode) ? "Authentication" : response.statusCode == 426 ? "Protocol" : "HTTP"
                        return completion(.failure(T3Failure(kind: kind, message: self.clean(reason ?? "The server returned HTTP \(response.statusCode)."))))
                    }
                    guard let decoded else { return completion(.failure(T3Failure(kind: "Protocol", message: "The server returned invalid JSON."))) }
                    completion(.success(decoded))
                }
            }
            httpTasks[task.taskIdentifier] = task
            task.resume()
        } catch { completion(.failure(failure(error))) }
    }

    private func uploadAttachment(_ request: [String: Any], completion: @escaping Completion) throws {
        guard state == "connected" else { throw T3Failure(kind: "Disconnected", message: "The server is not connected.") }
        // A folded paste uploads as UTF-8 text and an attached file under its own type, up to 50MB
        // (composer-editor-files.ts); anything without a type is a captured PNG.
        let declared = request["contentType"] as? String ?? ""
        let typed = !declared.isEmpty && declared.range(of: #"^[A-Za-z0-9!#$&^_.+-]+/[A-Za-z0-9!#$&^_.+-]+(;[ A-Za-z0-9=._-]*)?$"#, options: .regularExpression) != nil
        guard let path = request["path"] as? String, path.hasPrefix("/api/attachments/upload/"),
              let encoded = request["base64"] as? String, let data = Data(base64Encoded: encoded),
              data.count > 0, data.count <= (typed ? 50 : 10) * 1024 * 1024 else { throw arguments(typed ? "Choose a file up to 50MB." : "Choose a valid captured PNG up to10MB.") }
        guard typed || data.starts(with: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) else { throw arguments("The captured image is not a PNG.") }
        http(path: path, method: "POST", raw: data, contentType: typed ? declared : "image/png", epoch: generation, authorized: false) { [self] result in
            switch result {
            case .success(let value): finish(completion, value: value)
            case .failure(let error): finish(completion, failure: error)
            }
        }
    }

    private func readHTTP(_ request: [String: Any], completion: @escaping Completion) throws {
        guard state == "connected" else { throw T3Failure(kind: "Disconnected", message: "The server is not connected.") }
        guard let path = request["path"] as? String, (request["method"] as? String ?? "GET") == "GET", request["body"] == nil else {
            throw arguments("The client HTTP interface is read-only.")
        }
        let epoch = generation
        http(path: path, epoch: epoch) { [self] result in
            switch result {
            case .success(let value): finish(completion, value: value)
            case .failure(let error): finish(completion, failure: error)
            }
        }
    }

    /// Lane r6-media: the device hub's media credentials (resolveDeviceHubAccess): the server origin
    /// and a short-lived WebSocket ticket for requests that cannot carry the bearer header
    /// (R6DeviceStream.swift). nil when not connected or the server issued no ticket.
    func deviceHubAccess(_ completion: @escaping (URL?, String?) -> Void) {
        queue.async { [self] in
            guard state == "connected", let origin else { return completion(nil, nil) }
            http(path: "/api/auth/websocket-ticket", method: "POST", epoch: generation) { result in
                guard case .success(let value) = result, let ticket = (value as? [String: Any])?["ticket"] as? String, !ticket.isEmpty else { return completion(nil, nil) }
                completion(origin, ticket)
            }
        }
    }

    private func nextID() -> String { serial += 1; return "\(generation)-\(serial)" }
    private func rpc(_ request: [String: Any], completion: @escaping Completion) throws {
        guard state == "connected" else { throw T3Failure(kind: "Disconnected", message: "The server is not connected.") }
        guard let method = request["method"] as? String, !method.isEmpty else { throw arguments("request requires a method.") }
        guard pending.count < 64 else { throw T3Failure(kind: "Busy", message: "Too many server requests are already pending.") }
        let id = nextID(), wire = T3Wire.request(id: id, method: method, payload: request["payload"] ?? [:])
        let text = try T3Wire.encode(wire)
        pending[id] = Pending(completion: completion, deadline: Date().addingTimeInterval(30), trace: request["trace"] as? Int)
        send(text, epoch: generation)
    }
    private func subscribe(_ request: [String: Any], completion: @escaping Completion) throws {
        guard state == "connected" else { throw T3Failure(kind: "Disconnected", message: "The server is not connected.") }
        guard let method = request["method"] as? String, !method.isEmpty,
              let key = request["key"] as? String, !key.isEmpty else { throw arguments("subscribe requires a key and method.") }
        guard streams.count < 16 || streams.values.contains(key) else { throw T3Failure(kind: "Busy", message: "Too many subscriptions are open.") }
        let id = nextID(), text = try T3Wire.encode(T3Wire.request(id: id, method: method, payload: request["payload"] ?? [:]))
        unsubscribe(key)
        failedStreams[key] = nil
        streams[id] = key
        send(text, epoch: generation)
        finish(completion, value: ["id": id])
    }
    private func unsubscribe(_ key: String) {
        for (id, value) in streams where value == key {
            streams.removeValue(forKey: id)
            send(["_tag": "Interrupt", "requestId": id], epoch: generation)
        }
    }
    private func send(_ object: [String: Any], epoch: Int) {
        do { send(try T3Wire.encode(object), epoch: epoch) }
        catch { connectionFailed(failure(error), epoch: epoch) }
    }
    private func send(_ text: String, epoch: Int) {
        guard socket != nil, generation == epoch else { return }
        guard outbox.count < 4096, outboxBytes + text.utf8.count <= T3Wire.maximumBytes else {
            return connectionFailed(T3Failure(kind: "Limit", message: "The server is not consuming outgoing messages."), epoch: epoch)
        }
        outbox.append(text); outboxBytes += text.utf8.count
        flush(epoch: epoch)
    }
    private func flush(epoch: Int) {
        guard !sending, let socket, generation == epoch, let text = outbox.first else { return }
        sending = true
        Task { [weak self, weak socket] in
            guard let socket else { return }
            do {
                try await socket.send(.string(text))
                self?.queue.async { [weak self] in
                    guard let self, self.socket === socket, self.generation == epoch else { return }
                    self.sending = false
                    if !self.outbox.isEmpty { self.outboxBytes -= self.outbox.removeFirst().utf8.count }
                    self.flush(epoch: epoch)
                }
            }
            catch {
                self?.queue.async { [weak self] in
                    guard let self, self.socket === socket else { return }
                    self.connectionFailed(self.socketFailure(error, task: socket), epoch: epoch)
                }
            }
        }
    }
    private func receive(_ task: URLSessionWebSocketTask, epoch: Int) {
        Task { [weak self, weak task] in
            guard let task else { return }
            do {
                let message = try await task.receive()
                self?.queue.async { [weak self] in
                    guard let self, self.alive, self.socket === task, self.generation == epoch else { return }
                    do {
                        let data: Data
                        switch message {
                        case .string(let text): data = Data(text.utf8)
                        case .data(let bytes): data = bytes
                        @unknown default: throw T3Failure(kind: "Protocol", message: "Unknown WebSocket message type.")
                        }
                        for frame in try T3Wire.decode(data) { try self.consume(frame, epoch: epoch) }
                        self.receive(task, epoch: epoch)
                    } catch { self.connectionFailed(self.failure(error), epoch: epoch) }
                }
            } catch {
                self?.queue.async { [weak self] in
                    guard let self, self.socket === task else { return }
                    self.connectionFailed(self.socketFailure(error, task: task), epoch: epoch)
                }
            }
        }
    }

    private func consume(_ frame: [String: Any], epoch: Int) throws {
        guard generation == epoch else { return }
        let tag = frame["_tag"] as? String ?? ""
        if tag == "Pong" { lastPong = Date(); return }
        if tag == "Ping" { send(["_tag": "Pong"], epoch: epoch); return }
        if tag == "Defect" || tag == "ClientProtocolError" { throw T3Wire.failure(frame) }
        guard let id = T3Wire.identifier(frame["requestId"]) else { throw T3Failure(kind: "Protocol", message: "The server RPC response has no request ID.") }
        if tag == "Chunk" {
            guard let values = frame["values"] as? [Any] else { throw T3Failure(kind: "Protocol", message: "An RPC chunk has no values.") }
            if let key = streams[id] {
                streamRetries[key] = nil // the first value resets the retry ladder
                var notify = false
                for value in values { notify = try inbox.append(generation: epoch, key: key, subscriptionId: id, value: value) || notify }
                // ACK only once every value is retained or overflow is explicitly marked.
                send(["_tag": "Ack", "requestId": frame["requestId"]!], epoch: epoch)
                // One invalidation advertises all retained work until the app ACKs
                // it. Repeated initial snapshots must not cancel every bootstrap.
                if notify { changed("t3.events") }
            } else {
                // A late cancelled subscription must not hold the server's producer.
                send(["_tag": "Ack", "requestId": frame["requestId"]!], epoch: epoch)
            }
            return
        }
        guard tag == "Exit", let exit = frame["exit"] as? [String: Any] else {
            throw T3Failure(kind: "Protocol", message: "The server sent an unsupported RPC response.")
        }
        let success = exit["_tag"] as? String == "Success"
        if let call = pending.removeValue(forKey: id) {
            if success { finish(call.completion, value: exit["value"] ?? NSNull()) }
            else { finish(call.completion, failure: T3Wire.failure(exit)) }
        } else if let key = streams.removeValue(forKey: id) {
            let problem = success ? nil : failure(T3Wire.failure(exit))
            var value: [String: Any] = problem.map { ["_transportError": $0.json] } ?? ["_streamEnded": true]
            // An expected failure resubscribes on this session after 250 ms doubling to 30 s;
            // an authorization failure waits for the next session.
            if problem.map({ T3Reconnect.retriesStream(after: $0.kind) }) ?? true {
                let retries = streamRetries[key] ?? 0, delay = T3Reconnect.streamDelay(retries: retries)
                streamRetries[key] = retries + 1; failedStreams[key] = id
                value["retryAfterMs"] = Int((delay * 1000).rounded())
                queue.asyncAfter(deadline: .now() + delay) { [weak self] in
                    guard let self, self.alive, self.generation == epoch, self.failedStreams[key] == id else { return }
                    self.failedStreams[key] = nil
                    if (try? self.inbox.append(generation: epoch, key: key, subscriptionId: id, value: ["_retryDue": true])) == true { self.changed("t3.events") }
                }
            }
            if try inbox.append(generation: epoch, key: key, subscriptionId: id, value: value) { changed("t3.events") }
        }
    }

    private func runTimer() {
        tick?.cancel()
        lastPong = Date(); nextPing = Date().addingTimeInterval(10)
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + 1, repeating: 1)
        timer.setEventHandler { [weak self] in
            guard let self, self.state == "connected" else { return }
            let now = Date()
            self.transfers.expire()
            for (id, call) in self.pending where call.deadline <= now {
                self.pending.removeValue(forKey: id)
                self.send(["_tag": "Interrupt", "requestId": id], epoch: self.generation)
                self.finish(call.completion, failure: T3Failure(kind: "Timeout", message: "The server did not confirm the request. Refresh before trying it again.", uncertain: true))
            }
            if let probe = self.probing, now >= probe.deadline {
                let label = self.descriptor["label"] as? String ?? self.origin?.host ?? "The server"
                self.connectionFailed(T3Failure(kind: "Network", message: "\(label) did not respond to a connection health check."), epoch: self.generation)
            } else if now.timeIntervalSince(self.lastPong) > 35 {
                self.connectionFailed(T3Failure(kind: "Network", message: "The server stopped responding."), epoch: self.generation)
            } else if now >= self.nextPing {
                self.nextPing = now.addingTimeInterval(10)
                self.send(["_tag": "Ping"], epoch: self.generation)
            }
        }
        tick = timer; timer.resume()
    }

    private func retire(_ reason: T3Failure) {
        tick?.cancel(); tick = nil
        connectedAt = nil
        let waiters = probing?.waiters ?? []; probing = nil
        for waiter in waiters { finish(waiter, failure: reason) }
        let old = socket; socket = nil
        outbox.removeAll(); outboxBytes = 0; sending = false; transfers.reset()
        old?.cancel(with: .goingAway, reason: nil)
        for task in httpTasks.values { task.cancel() }; httpTasks.removeAll()
        let calls = Array(pending.values); pending.removeAll()
        for call in calls { finish(call.completion, failure: reason) }
        streams.removeAll(); streamRetries.removeAll(); failedStreams.removeAll()
        if let waiting = opening { opening = nil; finish(waiting, failure: reason) }
    }

    private func connectionFailed(_ error: T3Failure, epoch: Int) {
        guard alive, generation == epoch, wantsConnection else { return }
        // Cancelled operations from a retiring socket cannot schedule a second retry.
        guard reconnect == nil else { return }
        let problem = failure(error)
        failureKind = problem.kind; failureTrace = lastHTTPTrace; lastHTTPTrace = ""
        // The ladder restarts only after the lost connection stayed up 30 s.
        if let since = connectedAt, Date().timeIntervalSince(since) >= T3Reconnect.stableAfter { failures = 0 }
        // A health check asked and the connection failed before answering: reconnect now.
        let failedProbe = probeUnanswered; probeUnanswered = false
        let waiters = probing?.waiters ?? []; probing = nil
        retire(T3Failure(kind: problem.kind, message: problem.message, uncertain: true))
        let terminal = ["Authentication", "Credential", "Protocol", "Keychain", "Address", "Limit"].contains(problem.kind) || token.isEmpty
        if terminal {
            wantsConnection = false
            if problem.kind == "Authentication", let origin, let environment = descriptor["environmentId"] as? String {
                try? credentials.forget(origin: origin.absoluteString, environment: environment); token = ""
            }
            setStatus("error", problem.message)
            for waiter in waiters { finish(waiter, value: status()) }
            return
        }
        let delay: TimeInterval
        if failedProbe {
            // Only this first attempt skips the ladder; if it fails too, backoff resumes from the start.
            failures = 0; delay = 0
            setStatus("reconnecting", "\(problem.message) Reconnecting in 0s…")
        } else {
            failures += 1
            delay = T3Reconnect.delay(failureCount: failures - 1, random: random())
            setStatus("reconnecting", "\(problem.message) Reconnecting in \(Int(delay.rounded()))s…")
        }
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.alive, self.wantsConnection else { return }
            self.reconnect = nil; self.start()
        }
        reconnect = work; queue.asyncAfter(deadline: .now() + delay, execute: work)
        for waiter in waiters { finish(waiter, value: status()) }
    }

    /// The connection buttons' Retry (supervisor retryNow): a live socket is probed, not
    /// replaced; a waiting retry runs now with the ladder reset; anything else connects.
    private func retryNow(_ request: [String: Any], completion: @escaping Completion) throws {
        if let raw = request["origin"] as? String, !raw.isEmpty, let current = origin, try T3Endpoint.origin(raw) != current {
            return try connect(request, completion: completion)
        }
        guard origin != nil else { throw T3Failure(kind: "Disconnected", message: "Connect to a server first.") }
        if wantsConnection, state == "connected", socket != nil { return probe(timeout: T3Reconnect.quickProbe, waiter: completion) }
        if wantsConnection, reconnect != nil {
            reconnect?.cancel(); reconnect = nil
            failures = 0; opening = completion; start(); return
        }
        // An attempt in flight restarts; a stopped connection starts again from saved credentials.
        retire(T3Failure(kind: "Replaced", message: "The connection was retried.", uncertain: true))
        failureKind = ""; failureTrace = ""
        wantsConnection = true; failures = 0; opening = completion
        start()
    }

    /// Returning to the app (application-active): probe a live socket within 15 s, or wake a waiting retry.
    /// The shell and thread streams also resubscribe on the session (foregroundResubscriptions), since a
    /// suspended app may have missed events; the app reads `_retryDue` as for a failed stream.
    func applicationActive() {
        queue.async { [self] in
            guard alive, wantsConnection else { return }
            if state == "connected", socket != nil {
                probe(timeout: T3Reconnect.foregroundProbe, waiter: nil)
                var notify = false
                for (id, key) in streams where key == "shell" || key == "thread" {
                    notify = ((try? inbox.append(generation: generation, key: key, subscriptionId: id, value: ["_retryDue": true])) ?? false) || notify
                }
                if notify { changed("t3.events") }
            }
            else if let pending = reconnect { pending.cancel(); reconnect = nil; failures = 0; start() }
        }
    }

    /// A reachability change: offline questions a live socket (3 s); back online wakes a waiting retry.
    func networkChanged(online: Bool) {
        queue.async { [self] in
            guard alive, wantsConnection else { return }
            if !online { if state == "connected", socket != nil { probe(timeout: T3Reconnect.quickProbe, waiter: nil) } }
            else if let pending = reconnect { pending.cancel(); reconnect = nil; start() }
        }
    }

    /// server.probe (or server.getConfig before connectionProbe): a later quicker
    /// signal shortens a running probe; only a failed or timed-out probe reconnects.
    private func probe(timeout: TimeInterval, waiter: Completion?) {
        let deadline = Date().addingTimeInterval(timeout)
        if var current = probing {
            if deadline < current.deadline { current.deadline = deadline }
            if let waiter { current.waiters.append(waiter) }
            probing = current
            return
        }
        let capable = (descriptor["capabilities"] as? [String: Any])?["connectionProbe"] as? Bool == true
        let id = nextID(), epoch = generation
        probing = (id, deadline, waiter.map { [$0] } ?? [])
        probeUnanswered = true
        pending[id] = Pending(completion: { [weak self] response in
            guard let self, let current = self.probing, current.id == id, self.generation == epoch else { return }
            if response["ok"] as? Bool == true {
                self.probing = nil; self.probeUnanswered = false; self.lastPong = Date()
                for waiter in current.waiters { self.finish(waiter, value: self.status()) }
            } else {
                let error = response["error"] as? [String: Any] ?? [:]
                self.connectionFailed(T3Failure(kind: "Network", message: error["message"] as? String ?? "The connection health check failed."), epoch: epoch)
            }
        }, deadline: deadline.addingTimeInterval(60))
        do { send(try T3Wire.encode(T3Wire.request(id: id, method: capable ? "server.probe" : "server.getConfig", payload: [:])), epoch: epoch) }
        catch { connectionFailed(failure(error), epoch: epoch) }
    }

    private func socketFailure(_ error: Error, task: URLSessionWebSocketTask) -> T3Failure {
        if let response = task.response as? HTTPURLResponse {
            if [401, 403].contains(response.statusCode) {
                return T3Failure(kind: "Authentication", message: "The server refused the socket credential. Pair again.")
            }
            if response.statusCode == 426 {
                return T3Failure(kind: "Protocol", message: "The server refused orchestration protocol 2.")
            }
        }
        return failure(error)
    }

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        queue.async { [weak self] in
            guard let self, self.alive, self.socket === webSocketTask else { return }
            self.connectedAt = Date(); self.everConnected = true; self.failureKind = ""; self.failureTrace = ""
            self.setStatus("connected", "Connected to \(self.descriptor["label"] as? String ?? self.origin?.host ?? "T3").")
            if let origin = self.origin {
                self.savedEnvironments.remember(origin: origin.absoluteString, descriptor: self.descriptor)
                if self.persistent && self.remembersOrigin { self.defaults.set(origin.absoluteString, forKey: Self.originKey) }
            }
            self.runTimer()
            if let completion = self.opening { self.opening = nil; self.finish(completion, value: self.status()) }
        }
    }
    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
        queue.async { [weak self] in
            guard let self, self.socket === webSocketTask else { return }
            self.connectionFailed(T3Failure(kind: "Network", message: "The server connection closed (\(closeCode.rawValue))."), epoch: self.generation)
        }
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        // Never forward a bearer or one-time credential to a redirected origin.
        completionHandler(nil)
    }
}

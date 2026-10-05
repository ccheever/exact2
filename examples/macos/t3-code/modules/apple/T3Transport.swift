import Foundation

/// A serial owner keeps wire delivery lossless. Topic invalidations may coalesce;
/// the underlying journal cannot. All completion closures run on this queue.
final class T3Transport: NSObject, URLSessionWebSocketDelegate, @unchecked Sendable {
    typealias Completion = ([String: Any]) -> Void
    private struct Pending {
        let completion: Completion
        let deadline: Date
    }
    private let queue = DispatchQueue(label: "com.exact.t3code.transport")
    private let changed: (String) -> Void
    private let credentials: T3Credentials
    private let persistent: Bool
    private let preferencesURL: URL?
    private var session: URLSession!
    private var socket: URLSessionWebSocketTask?
    private var httpTasks: [Int: URLSessionDataTask] = [:]
    private var pending: [String: Pending] = [:]
    private var streams: [String: String] = [:] // request id -> app subscription key
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
    private var retry = 0
    private var reconnect: DispatchWorkItem?
    private var tick: DispatchSourceTimer?
    private var lastPong = Date()
    private var nextPing = Date()
    private var opening: Completion?

    init(persistent: Bool, dataDirectory: URL? = nil, configuration: URLSessionConfiguration = .ephemeral, changed: @escaping (String) -> Void) {
        self.persistent = persistent; self.changed = changed
        preferencesURL = dataDirectory?.appendingPathComponent("t3-code.json", isDirectory: false)
        credentials = T3Credentials(persistent: persistent)
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
                case "disconnect":
                    wantsConnection = false; reconnect?.cancel(); reconnect = nil
                    retire(T3Failure(kind: "Disconnected", message: "Disconnected from the server.", uncertain: true))
                    generation += 1; inbox.reset(); token = ""
                    if request["forget"] as? Bool == true, let origin, let environment = descriptor["environmentId"] as? String {
                        try credentials.forget(origin: origin.absoluteString, environment: environment)
                    }
                    setStatus("disconnected", "Disconnected.")
                    finish(completion, value: status())
                case "http": try readHTTP(request, completion: completion)
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
        guard origin == nil, persistent,
              let saved = UserDefaults.standard.string(forKey: "t3.server.origin") else { return }
        origin = try? T3Endpoint.origin(saved)
    }
    private func status() -> [String: Any] {
        ["state": state, "origin": origin?.absoluteString ?? "", "environmentId": descriptor["environmentId"] as? String ?? "",
         "message": message, "descriptor": descriptor]
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
        if let error = error as? T3Failure { return T3Failure(kind: error.kind, message: clean(error.message), uncertain: error.uncertain) }
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
        privateValues = credential.isEmpty ? [] : [credential]
        wantsConnection = true; retry = 0; opening = completion
        if persistent { UserDefaults.standard.set(newOrigin.absoluteString, forKey: "t3.server.origin") }
        start(credential: credential)
    }

    private func start(credential: String = "") {
        guard wantsConnection, alive, let origin else { return }
        generation += 1; inbox.reset()
        let epoch = generation
        setStatus(retry == 0 ? "connecting" : "reconnecting", "Connecting to \(origin.host ?? "the server")…")
        http(path: "/.well-known/t3/environment", epoch: epoch, authorized: false) { [self] result in
            switch result {
            case .failure(let error): connectionFailed(error, epoch: epoch)
            case .success(let value):
                guard let object = value as? [String: Any], object["environmentId"] is String,
                      object["orchestrationProtocolVersion"] as? Int == 2 else {
                    return connectionFailed(T3Failure(kind: "Protocol", message: "This app requires T3 orchestration protocol 2."), epoch: epoch)
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
            "scope": "orchestration:read orchestration:operate",
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
                        let reason = (decoded as? [String: Any])?["message"] as? String
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

    private func nextID() -> String { serial += 1; return "\(generation)-\(serial)" }
    private func rpc(_ request: [String: Any], completion: @escaping Completion) throws {
        guard state == "connected" else { throw T3Failure(kind: "Disconnected", message: "The server is not connected.") }
        guard let method = request["method"] as? String, !method.isEmpty else { throw arguments("request requires a method.") }
        guard pending.count < 64 else { throw T3Failure(kind: "Busy", message: "Too many server requests are already pending.") }
        let id = nextID(), wire = T3Wire.request(id: id, method: method, payload: request["payload"] ?? [:])
        let text = try T3Wire.encode(wire)
        pending[id] = Pending(completion: completion, deadline: Date().addingTimeInterval(30))
        send(text, epoch: generation)
    }
    private func subscribe(_ request: [String: Any], completion: @escaping Completion) throws {
        guard state == "connected" else { throw T3Failure(kind: "Disconnected", message: "The server is not connected.") }
        guard let method = request["method"] as? String, !method.isEmpty,
              let key = request["key"] as? String, !key.isEmpty else { throw arguments("subscribe requires a key and method.") }
        guard streams.count < 16 || streams.values.contains(key) else { throw T3Failure(kind: "Busy", message: "Too many subscriptions are open.") }
        let id = nextID(), text = try T3Wire.encode(T3Wire.request(id: id, method: method, payload: request["payload"] ?? [:]))
        unsubscribe(key)
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
            let value: [String: Any] = success ? ["_streamEnded": true] : ["_transportError": failure(T3Wire.failure(exit)).json]
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
            if now.timeIntervalSince(self.lastPong) > 35 {
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
        let old = socket; socket = nil
        outbox.removeAll(); outboxBytes = 0; sending = false; transfers.reset()
        old?.cancel(with: .goingAway, reason: nil)
        for task in httpTasks.values { task.cancel() }; httpTasks.removeAll()
        let calls = Array(pending.values); pending.removeAll()
        for call in calls { finish(call.completion, failure: reason) }
        streams.removeAll()
        if let waiting = opening { opening = nil; finish(waiting, failure: reason) }
    }

    private func connectionFailed(_ error: T3Failure, epoch: Int) {
        guard alive, generation == epoch, wantsConnection else { return }
        // Cancelled operations from a retiring socket cannot schedule a second retry.
        guard reconnect == nil else { return }
        let problem = failure(error)
        retire(T3Failure(kind: problem.kind, message: problem.message, uncertain: true))
        let terminal = ["Authentication", "Credential", "Protocol", "Keychain", "Address", "Limit"].contains(problem.kind) || token.isEmpty
        if terminal {
            wantsConnection = false
            if problem.kind == "Authentication", let origin, let environment = descriptor["environmentId"] as? String {
                try? credentials.forget(origin: origin.absoluteString, environment: environment); token = ""
            }
            setStatus("error", problem.message)
            return
        }
        let delay = min(30.0, pow(2.0, Double(min(retry, 5))))
        retry += 1
        setStatus("reconnecting", "\(problem.message) Reconnecting in \(Int(delay))s…")
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.alive, self.wantsConnection else { return }
            self.reconnect = nil; self.start()
        }
        reconnect = work; queue.asyncAfter(deadline: .now() + delay, execute: work)
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
            self.retry = 0
            self.setStatus("connected", "Connected to \(self.descriptor["label"] as? String ?? self.origin?.host ?? "T3").")
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

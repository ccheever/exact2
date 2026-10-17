// The module's own connections to the environments the data module has connected. The server
// connections are the data module's (src/transport/transport.ts); what stays native here is what
// feeds native views: the terminal's attach streams (T3TerminalSessions.swift; their bytes go
// straight into the terminal's buffer), the Browser's automation replies and recording uploads
// (T3BrowserAutomation.swift) and the device hub's tickets (T3DeviceStream.swift). The data module
// hands over each connected environment's origin and bearer (`links`, with its connection's
// generation); a link opens its own socket, with a ticket of its own, only while something here
// needs one, and lets it go when the data module's connection goes. Its wire is the subset of T3
// Code's Effect RPC frames those views use (T3 Code 1e2ecbd975, MIT, see LICENSE-T3).
import Foundation

struct T3Failure: Error {
    let kind: String
    let message: String
    var uncertain = false
    /// A typed server error's own fields (PullRequestOperationError.reason "not-found", .detail).
    var reason = ""
    var detail = ""
    var json: [String: Any] { ["kind": kind, "message": message, "uncertain": uncertain, "reason": reason, "detail": detail] }
}

enum T3Wire {
    static let maximumBytes = 16 * 1024 * 1024

    static func encode(_ message: [String: Any]) throws -> String {
        let data = try JSONSerialization.data(withJSONObject: message, options: [.fragmentsAllowed])
        guard data.count <= maximumBytes else { throw T3Failure(kind: "Limit", message: "The request is too large.") }
        return String(decoding: data, as: UTF8.self)
    }

    static func decode(_ data: Data) throws -> [[String: Any]] {
        guard data.count <= maximumBytes else { throw T3Failure(kind: "Limit", message: "The server frame is too large.") }
        let object = try JSONSerialization.jsonObject(with: data)
        let frames: [[String: Any]]
        if let array = object as? [[String: Any]] { frames = array }
        else if let one = object as? [String: Any] { frames = [one] }
        else { throw T3Failure(kind: "Protocol", message: "The server sent an invalid RPC frame.") }
        guard frames.allSatisfy({ $0["_tag"] is String }) else { throw T3Failure(kind: "Protocol", message: "The server sent an untagged RPC frame.") }
        return frames
    }

    static func identifier(_ value: Any?) -> String? {
        if let value = value as? String { return value }
        if let value = value as? NSNumber { return value.stringValue }
        return nil
    }

    static func request(id: String, method: String, payload: Any) -> [String: Any] {
        ["_tag": "Request", "id": id, "tag": method, "payload": payload, "headers": [["x-t3-orchestration-protocol", "2"]]]
    }

    /// An Exit failure as the reference squashes it (Cause.squash): the first typed `Fail`, else a `Die`, else an interrupt.
    static func failure(_ value: Any) -> T3Failure {
        if let array = value as? [Any] {
            let reasons = array.compactMap { $0 as? [String: Any] }
            if let fail = reasons.first(where: { $0["_tag"] as? String == "Fail" }) ?? reasons.first(where: { $0["_tag"] as? String == "Die" }) { return failure(fail) }
            if let first = array.first { return failure(first) }
            return T3Failure(kind: "RPC", message: "The server request failed.")
        }
        guard let object = value as? [String: Any] else {
            return T3Failure(kind: "RPC", message: (value as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "The server request failed.")
        }
        switch object["_tag"] as? String {
        case "Exit"?: if let exit = object["exit"] { return failure(exit) }
        case "Failure"?: if let cause = object["cause"] { return failure(cause) }
        case "Fail"?: if let error = object["error"] { return typed(error) }
        case "Die"?, "Defect"?: if let defect = object["defect"] { return failure(defect) }
        case "ClientProtocolError"?: if let error = object["error"] { return failure(error) }
        case "Interrupt"?: return T3Failure(kind: "Interrupt", message: "The server interrupted the request.")
        case nil:
            if object["message"] == nil {
                if let error = object["error"] { return failure(error) }
                if let cause = object["cause"] { return failure(cause) }
                if let defect = object["defect"] { return failure(defect) }
            }
        default: break
        }
        return typed(object)
    }

    private static func typed(_ value: Any) -> T3Failure {
        guard let error = value as? [String: Any] else { return failure(value) }
        let tag = error["_tag"] as? String ?? error["name"] as? String ?? "RPC"
        let text = { (key: String) -> String in (error[key] as? String).map { $0.trimmingCharacters(in: .whitespacesAndNewlines) } ?? "" }
        var message = text("message")
        if message.isEmpty, !text("detail").isEmpty { message = text("detail") }
        if message.isEmpty, let cause = error["cause"] { message = failure(cause).message }
        if message.isEmpty || message == "The server request failed." { message = "The server refused the request (\(tag))." }
        return T3Failure(kind: tag, message: message, reason: text("reason"), detail: text("detail"))
    }
}

/// One `terminal.attach` stream on a link. Its chunks go to the native session, never to the data module, and each
/// is acknowledged only after the session's buffer holds it: the server sends at most 8 chunks or 64 KiB ahead of
/// the acknowledgements (T3 Code apps/server/src/terminal/OutputProtocol.ts), so a busy main thread slows the shell
/// instead of growing a queue.
final class T3TerminalStream {
    let id: String
    let epoch: Int
    /// On the link's queue: the chunk's values and the acknowledgement to call once they are held.
    let receive: ([Any], @escaping () -> Void) -> Void
    /// On the link's queue: the stream ended (nil: a normal end) or the socket went away (`lost`).
    let ended: (_ failure: T3Failure?, _ lost: Bool) -> Void
    init(id: String, epoch: Int, receive: @escaping ([Any], @escaping () -> Void) -> Void, ended: @escaping (T3Failure?, Bool) -> Void) {
        self.id = id; self.epoch = epoch; self.receive = receive; self.ended = ended
    }
}

/// One environment's link: its origin and bearer as the data module has them, and a socket opened on demand.
final class T3Link: NSObject, URLSessionWebSocketDelegate, @unchecked Sendable {
    typealias Done = (T3Failure?, Any?) -> Void
    let queue = DispatchQueue(label: "com.exact.t3code.link")
    let environment: String
    let primary: Bool
    private(set) var origin: URL
    private var token: String
    /// The data module's connection generation this link was handed (a reconnect hands a new one).
    private(set) var generation: Int
    private var session: URLSession!
    private var socket: URLSessionWebSocketTask?
    private var open = false, opening = false
    private var epoch = 0, serial = 0
    private var pending: [String: Done] = [:]
    private var waiting: [() -> Void] = []
    var terminalStreams: [String: T3TerminalStream] = [:]
    private var lastPong = Date()
    private var tick: DispatchSourceTimer?
    private var alive = true
    /// The reference's worst case: 11 mounted threads × 4 terminals.
    static let maximumTerminalStreams = 44
    /// The bearer of the embedded server (memory only), for the primary's link.
    var primaryBearer: () -> String? = { T3LocalBackend.shared.bearerToken() }

    init(environment: String, origin: URL, token: String, primary: Bool, generation: Int, configuration: URLSessionConfiguration = .ephemeral) {
        self.environment = environment; self.origin = origin; self.token = token; self.primary = primary; self.generation = generation
        super.init()
        let config = configuration
        config.httpCookieStorage = nil; config.httpShouldSetCookies = false; config.urlCache = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.timeoutIntervalForRequest = 15; config.timeoutIntervalForResource = 30
        session = URLSession(configuration: config, delegate: self, delegateQueue: nil)
    }

    private var bearer: String { primary ? (primaryBearer() ?? "") : token }
    var isOpen: Bool { queue.sync { open } }

    /// The data module's latest facts for this environment; a new origin or bearer reopens on the next use.
    func update(origin: URL, token: String, generation: Int) {
        queue.async { [self] in
            guard origin != self.origin || token != self.token || generation != self.generation else { return }
            self.origin = origin; self.token = token; self.generation = generation
            if socket != nil { lost(T3Failure(kind: "Replaced", message: "The connection moved.", uncertain: true)) }
        }
    }

    func close() {
        queue.async { [self] in
            guard alive else { return }
            alive = false
            lost(T3Failure(kind: "Closed", message: "The server is not connected.", uncertain: true))
            session.invalidateAndCancel()
        }
    }

    // MARK: Requests

    /// One RPC; `done` runs on the link's queue with the failure or the Exit value.
    func request(_ method: String, payload: Any, done: @escaping Done) {
        withSocket(done: { failure in done(failure, nil) }) { [self] in
            let id = nextID()
            do { try send(T3Wire.encode(T3Wire.request(id: id, method: method, payload: payload))) } catch { return done(fail(error), nil) }
            pending[id] = done
        }
    }

    /// Opens an attach stream (`terminal.attach`, which also starts a shell when none runs).
    func terminalAttach(payload: [String: Any], receive: @escaping ([Any], @escaping () -> Void) -> Void,
                        ended: @escaping (T3Failure?, Bool) -> Void, opened: @escaping (String?, T3Failure?) -> Void) {
        withSocket(done: { failure in opened(nil, failure) }) { [self] in
            guard terminalStreams.count < Self.maximumTerminalStreams else { return opened(nil, T3Failure(kind: "Busy", message: "Too many terminals are attached.")) }
            let id = nextID()
            do { try send(T3Wire.encode(T3Wire.request(id: id, method: "terminal.attach", payload: payload))) } catch { return opened(nil, fail(error)) }
            terminalStreams[id] = T3TerminalStream(id: id, epoch: epoch, receive: receive, ended: ended)
            opened(id, nil)
        }
    }

    /// Stops an attach stream (an Interrupt). A late chunk is still acknowledged.
    func terminalDetach(_ id: String) {
        queue.async { [self] in
            guard terminalStreams.removeValue(forKey: id) != nil else { return }
            try? send(T3Wire.encode(["_tag": "Interrupt", "requestId": id]))
            closeIfIdle()
        }
    }

    /// A one-shot terminal RPC (`terminal.write`, `terminal.resize`, `provider.auth.respond`).
    func terminalCall(_ method: String, payload: [String: Any], done: @escaping (T3Failure?) -> Void) {
        request(method, payload: payload) { failure, _ in done(failure) }
    }

    /// A short-lived socket ticket for requests that cannot carry the bearer (the device hub's media).
    func ticket(_ done: @escaping (URL?, String?) -> Void) {
        queue.async { [self] in
            http("/api/auth/websocket-ticket", method: "POST", body: nil, contentType: nil, authorized: true) { result in
                guard case .success(let value) = result, let ticket = (value as? [String: Any])?["ticket"] as? String, !ticket.isEmpty else { return done(nil, nil) }
                done(self.origin, ticket)
            }
        }
    }

    /// An attachment's bytes to the server's upload URL (unauthenticated, as the reference's upload is).
    func upload(path: String, data: Data, contentType: String, done: @escaping (T3Failure?, Any?) -> Void) {
        queue.async { [self] in
            guard path.hasPrefix("/api/attachments/upload/") else { return done(T3Failure(kind: "Arguments", message: "That is not an upload URL."), nil) }
            http(path, method: "POST", body: data, contentType: contentType, authorized: false) { result in
                switch result { case .success(let value): done(nil, value); case .failure(let failure): done(failure, nil) }
            }
        }
    }

    // MARK: The socket

    private func nextID() -> String { serial += 1; return "link-\(epoch)-\(serial)" }
    private func fail(_ error: Error) -> T3Failure { error as? T3Failure ?? T3Failure(kind: "Network", message: error.localizedDescription) }

    /// Runs `work` on the queue once the socket is open, opening it first.
    private func withSocket(done: @escaping (T3Failure) -> Void, _ work: @escaping () -> Void) {
        queue.async { [self] in
            guard alive else { return done(T3Failure(kind: "Disconnected", message: "The server is not connected.")) }
            if open { return work() }
            waiting.append(work)
            guard !opening else { return }
            opening = true
            let current = epoch
            http("/api/auth/websocket-ticket", method: "POST", body: nil, contentType: nil, authorized: true) { [self] result in
                guard current == epoch, alive else { return }
                guard case .success(let value) = result, let ticket = (value as? [String: Any])?["ticket"] as? String, !ticket.isEmpty,
                      var url = URLComponents(url: origin, resolvingAgainstBaseURL: false) else {
                    let failure: T3Failure
                    if case .failure(let error) = result { failure = error } else { failure = T3Failure(kind: "Protocol", message: "The server did not issue a socket ticket.") }
                    return lost(failure)
                }
                url.scheme = url.scheme == "https" ? "wss" : "ws"; url.path = "/ws"
                url.queryItems = [URLQueryItem(name: "wsTicket", value: ticket), URLQueryItem(name: "orchestrationProtocol", value: "2"),
                                  URLQueryItem(name: "clientSurface", value: "desktop"), URLQueryItem(name: "clientOs", value: "macos"),
                                  URLQueryItem(name: "clientDeviceType", value: "desktop")]
                var request = URLRequest(url: url.url!)
                request.timeoutInterval = 15
                request.setValue("2", forHTTPHeaderField: "x-t3-orchestration-protocol")
                let task = session.webSocketTask(with: request)
                task.maximumMessageSize = T3Wire.maximumBytes
                socket = task; task.resume()
                receive(task, epoch: current)
                queue.asyncAfter(deadline: .now() + 16) { [weak self, weak task] in
                    guard let self, let task, self.socket === task, !self.open else { return }
                    self.lost(T3Failure(kind: "Network", message: "The server WebSocket did not open."))
                }
            }
        }
    }

    private func send(_ text: String) throws {
        guard let socket, open else { throw T3Failure(kind: "Disconnected", message: "The server is not connected.") }
        let current = epoch
        socket.send(.string(text)) { [weak self] error in
            guard let self, let error else { return }
            self.queue.async { if self.epoch == current { self.lost(self.fail(error)) } }
        }
    }

    private func receive(_ task: URLSessionWebSocketTask, epoch current: Int) {
        task.receive { [weak self, weak task] result in
            guard let self, let task else { return }
            self.queue.async {
                guard self.alive, self.socket === task, self.epoch == current else { return }
                switch result {
                case .failure(let error): self.lost(self.fail(error))
                case .success(let message):
                    let data: Data
                    switch message {
                    case .string(let text): data = Data(text.utf8)
                    case .data(let bytes): data = bytes
                    @unknown default: return self.lost(T3Failure(kind: "Protocol", message: "Unknown WebSocket message type."))
                    }
                    do { for frame in try T3Wire.decode(data) { try self.consume(frame) } } catch { return self.lost(self.fail(error)) }
                    self.receive(task, epoch: current)
                }
            }
        }
    }

    private func consume(_ frame: [String: Any]) throws {
        let tag = frame["_tag"] as? String ?? ""
        if tag == "Pong" { lastPong = Date(); return }
        if tag == "Ping" { try send(T3Wire.encode(["_tag": "Pong"])); return }
        if tag == "Defect" || tag == "ClientProtocolError" { throw T3Wire.failure(frame) }
        guard let id = T3Wire.identifier(frame["requestId"]), let requestId = frame["requestId"] else { throw T3Failure(kind: "Protocol", message: "The server RPC response has no request ID.") }
        if tag == "Chunk" {
            let values = frame["values"] as? [Any] ?? []
            let current = epoch
            guard let stream = terminalStreams[id] else { return try send(T3Wire.encode(["_tag": "Ack", "requestId": requestId])) }
            stream.receive(values) { [weak self] in
                self?.queue.async { [weak self] in
                    guard let self, self.epoch == current else { return }
                    try? self.send(T3Wire.encode(["_tag": "Ack", "requestId": requestId]))
                }
            }
            return
        }
        guard tag == "Exit", let exit = frame["exit"] as? [String: Any] else { throw T3Failure(kind: "Protocol", message: "The server sent an unsupported RPC response.") }
        let success = exit["_tag"] as? String == "Success"
        if let stream = terminalStreams.removeValue(forKey: id) {
            stream.ended(success ? nil : T3Wire.failure(exit), false)
            return closeIfIdle()
        }
        if let done = pending.removeValue(forKey: id) {
            if success { done(nil, exit["value"] ?? NSNull()) } else { done(T3Wire.failure(exit), nil) }
            closeIfIdle()
        }
    }

    /// The socket is gone: requests fail, attach streams end lost, and the next use opens a new one.
    private func lost(_ failure: T3Failure) {
        epoch += 1; open = false; opening = false
        tick?.cancel(); tick = nil
        let old = socket; socket = nil
        old?.cancel(with: .goingAway, reason: nil)
        let calls = pending; pending = [:]
        for done in calls.values { done(failure, nil) }
        let streams = terminalStreams; terminalStreams = [:]
        for stream in streams.values { stream.ended(nil, true) }
        let works = waiting; waiting = []
        if !works.isEmpty, alive, failure.kind == "Replaced" {
            // The connection moved: the work waiting for it opens the new one.
            for work in works { withSocket(done: { _ in }, work) }
        }
    }

    /// Nothing here needs the socket any more: let it go after a minute.
    private func closeIfIdle() {
        guard pending.isEmpty, terminalStreams.isEmpty else { return }
        let current = epoch
        queue.asyncAfter(deadline: .now() + 60) { [weak self] in
            guard let self, self.epoch == current, self.pending.isEmpty, self.terminalStreams.isEmpty, self.socket != nil else { return }
            self.lost(T3Failure(kind: "Idle", message: "The link was idle."))
        }
    }

    private func http(_ path: String, method: String, body: Data?, contentType: String?, authorized: Bool, completion: @escaping (Result<Any, T3Failure>) -> Void) {
        guard path.hasPrefix("/"), let url = URL(string: path, relativeTo: origin)?.absoluteURL else { return completion(.failure(T3Failure(kind: "Address", message: "The request must stay on the connected server."))) }
        var request = URLRequest(url: url)
        request.httpMethod = method; request.httpBody = body
        request.setValue("2", forHTTPHeaderField: "x-t3-orchestration-protocol")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let contentType { request.setValue(contentType, forHTTPHeaderField: "Content-Type") }
        if authorized { request.setValue("Bearer \(bearer)", forHTTPHeaderField: "Authorization") }
        session.dataTask(with: request) { [weak self] data, response, error in
            guard let self else { return }
            self.queue.async {
                if let error { return completion(.failure(self.fail(error))) }
                let status = (response as? HTTPURLResponse)?.statusCode ?? 0
                let decoded = (data ?? Data()).isEmpty ? [String: Any]() : (try? JSONSerialization.jsonObject(with: data ?? Data(), options: [.fragmentsAllowed]))
                guard (200..<300).contains(status), let decoded else {
                    let message = ((decoded as? [String: Any])?["message"] as? String) ?? "The server returned HTTP \(status)."
                    return completion(.failure(T3Failure(kind: [401, 403].contains(status) ? "Authentication" : "HTTP", message: message)))
                }
                completion(.success(decoded))
            }
        }.resume()
    }

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        queue.async { [weak self] in
            guard let self, self.alive, self.socket === webSocketTask else { return }
            self.open = true; self.opening = false; self.lastPong = Date()
            let timer = DispatchSource.makeTimerSource(queue: self.queue)
            timer.schedule(deadline: .now() + 10, repeating: 10)
            timer.setEventHandler { [weak self] in
                guard let self, self.open else { return }
                if Date().timeIntervalSince(self.lastPong) > 35 { return self.lost(T3Failure(kind: "Network", message: "The server stopped responding.")) }
                try? self.send(T3Wire.encode(["_tag": "Ping"]))
            }
            self.tick = timer; timer.resume()
            let works = self.waiting; self.waiting = []
            for work in works { work() }
        }
    }
    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
        queue.async { [weak self] in
            guard let self, self.socket === webSocketTask else { return }
            self.lost(T3Failure(kind: "Network", message: "The server connection closed (\(closeCode.rawValue))."))
        }
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil) // never forward a bearer or a ticket to a redirected origin
    }
}

/// The module's links, by environment: the data module's `links` op keeps them to its connected sessions.
final class T3Links {
    private var byEnvironment: [String: T3Link] = [:]
    private var byFleet: [String: String] = [:]
    private(set) var focused = ""
    /// On the main queue: an environment's link came (a connection, or a reconnect's new generation) or went (nil).
    var changed: [(String, T3Link?) -> Void] = []

    /// `links` { focused, routes: [{ fleet, environmentId, origin, token, primary, generation }] }.
    func update(_ request: [String: Any]) {
        let routes = request["routes"] as? [[String: Any]] ?? []
        focused = request["focused"] as? String ?? ""
        var seen = Set<String>(), fleet: [String: String] = [:], notices: [(String, T3Link?)] = []
        for route in routes {
            guard let environment = route["environmentId"] as? String, !environment.isEmpty, !seen.contains(environment),
                  let origin = (route["origin"] as? String).flatMap(URL.init(string:)) else { continue }
            seen.insert(environment)
            let token = route["token"] as? String ?? "", primary = route["primary"] as? Bool == true
            let generation = (route["generation"] as? NSNumber)?.intValue ?? 0
            if let key = route["fleet"] as? String, !key.isEmpty { fleet[key] = environment }
            if let link = byEnvironment[environment], link.primary == primary {
                if link.origin != origin || link.generation != generation { notices.append((environment, link)) }
                link.update(origin: origin, token: token, generation: generation)
            } else {
                byEnvironment[environment]?.close()
                let link = T3Link(environment: environment, origin: origin, token: token, primary: primary, generation: generation)
                byEnvironment[environment] = link
                notices.append((environment, link))
            }
        }
        for (environment, link) in byEnvironment where !seen.contains(environment) {
            link.close(); byEnvironment[environment] = nil; notices.append((environment, nil))
        }
        byFleet = fleet
        for (environment, link) in notices { for observer in changed { observer(environment, link) } }
    }

    func link(environment: String) -> T3Link? { byEnvironment[environment] }
    /// A background environment's link by its fleet key, else the focused environment's.
    func link(fleet key: String?) -> T3Link? {
        if let key, !key.isEmpty { return byFleet[key].flatMap { byEnvironment[$0] } }
        return byEnvironment[focused]
    }
    var focusedLink: T3Link? { byEnvironment[focused] }

    /// The device hub's media credentials (resolveDeviceHubAccess): the focused server's origin and a socket ticket.
    func deviceHubAccess(_ done: @escaping (URL?, String?) -> Void) {
        DispatchQueue.main.async { [weak self] in
            guard let link = self?.focusedLink else { return done(nil, nil) }
            link.ticket(done)
        }
    }

    func destroy() { for link in byEnvironment.values { link.close() }; byEnvironment = [:]; byFleet = [:] }
}

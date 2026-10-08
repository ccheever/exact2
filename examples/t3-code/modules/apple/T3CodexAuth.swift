// The ChatGPT sign-in's loopback receiver (MIT reference, see LICENSE-T3, at 1e2ecbd975):
// packages/shared/src/codexAuthCallback.ts:16-100 (receiveCodexAuthCallback,
// cancelCodexAuthCallback), its rules in packages/shared/src/codexAuthHandoff.ts:27-80
// (codexAuthorizationRequest, codexCallbackUrl; codex-auth-request.ts is the TypeScript copy),
// and apps/desktop/src/ipc/methods/providerAuth.ts:16-49 (reveal the window once the callback
// arrived). Credentials and PKCE stay on the environment: this only binds the redirect's
// 127.0.0.1 port before the browser opens, answers the one GET on the callback path with the
// right `state` and exactly one `code` or `error`, and hands that URL back.
// Changes: the desktop's one long IPC call is split (X14): `codexAuthStart` answers after the
// bind and the browser open, the callback is announced on the `t3.codexAuth` topic and read once
// with `codexAuthTake`; `codexAuthCancel` is cancelCodexAuthCallback. The 303 redirect for hosted
// web (the `destination` argument) is not ported: this app is the only receiver. An agent run
// records the browser open (T3RemoteEditors) and does not raise the window; the journal
// (`logs`) gets one line per bind, refusal, callback and release, never a code or a URL.
import Foundation
import AppKit

enum T3CodexAuthRules {
    struct Request { let authorizationUrl: String; let redirectUri: String; let state: String; let port: UInt16 }
    struct Invalid: Error { let message: String }

    private static func matches(_ pattern: String, _ value: String) -> Bool {
        value.range(of: pattern, options: .regularExpression) != nil
    }
    /// Every value of one query key, decoded as URLSearchParams decodes it (`+` is a space).
    private static func values(_ components: URLComponents, _ key: String) -> [String] {
        let query = components.percentEncodedQuery ?? ""
        guard !query.isEmpty else { return [] }
        return query.split(separator: "&", omittingEmptySubsequences: true).compactMap { pair -> String? in
            let parts = pair.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let name = String(parts[0]).replacingOccurrences(of: "+", with: " ").removingPercentEncoding ?? String(parts[0])
            guard name == key else { return nil }
            let raw = parts.count > 1 ? String(parts[1]) : ""
            return raw.replacingOccurrences(of: "+", with: " ").removingPercentEncoding ?? raw
        }
    }

    /// codexAuthorizationRequest: only OpenAI's authorize endpoint with a 127.0.0.1 callback.
    static func authorizationRequest(_ value: String) throws -> Request {
        let invalid = Invalid(message: "Invalid ChatGPT sign-in request.")
        guard value.count <= 16_384, let url = URLComponents(string: value), url.scheme?.lowercased() == "https",
              url.host?.lowercased() == "auth.openai.com", url.port == nil || url.port == 443, url.path == "/api/accounts/authorize",
              url.user == nil, url.password == nil, url.fragment == nil else { throw invalid }
        func single(_ key: String) throws -> String {
            let found = values(url, key)
            guard found.count == 1, let first = found.first, !first.isEmpty else { throw invalid }
            return first
        }
        let redirectUri = try single("redirect_uri"), state = try single("state")
        guard matches(#"^http://127\.0\.0\.1:[1-9][0-9]{0,4}/auth/callback$"#, redirectUri),
              let port = URLComponents(string: redirectUri)?.port, port <= 65_535,
              matches(#"^[A-Za-z0-9_-]{16,128}$"#, state),
              try single("response_type") == "code", try single("code_challenge_method") == "S256",
              matches(#"^[A-Za-z0-9_-]{43}$"#, try single("code_challenge")),
              matches(#"^(dynamic_agent_client|oaiapp_[A-Za-z0-9_-]+)$"#, try single("client_id")) else { throw invalid }
        return Request(authorizationUrl: value, redirectUri: redirectUri, state: state, port: UInt16(port))
    }

    /// codexCallbackUrl: the redirect belongs to this sign-in, with exactly one code or one error.
    static func callbackUrl(_ value: String, redirectUri: String, state: String) throws -> String {
        let invalid = Invalid(message: "This redirect URL does not belong to the current sign-in.")
        guard value.count <= 16_384, let callback = URLComponents(string: value), let expected = URLComponents(string: redirectUri),
              callback.scheme?.lowercased() == expected.scheme?.lowercased(), callback.host?.lowercased() == expected.host?.lowercased(),
              callback.port == expected.port, callback.path == expected.path,
              callback.user == nil, callback.password == nil, callback.fragment == nil else { throw invalid }
        let states = values(callback, "state"), codes = values(callback, "code"), errors = values(callback, "error"), clients = values(callback, "client_id")
        guard states.count == 1, states[0] == state, clients.count <= 1,
              clients.isEmpty || matches(#"^oaiapp_[A-Za-z0-9_-]+$"#, clients[0]),
              (codes.count == 1 && !codes[0].isEmpty && errors.isEmpty) || (errors.count == 1 && !errors[0].isEmpty && codes.isEmpty) else { throw invalid }
        return value
    }
}

/// The listeners by `state`, and the outcome each one left for `take`.
final class T3CodexAuth {
    static let shared = T3CodexAuth()
    static let portInUse = "The ChatGPT callback port is in use on this computer. Close the other sign-in and try again, or paste the redirect URL in T3 Code."
    static let page = "<!doctype html><html><head><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"light dark\"><title>T3 Code</title><style>body{font-family:system-ui;display:grid;place-items:center;min-height:90vh;margin:0}main{max-width:360px;padding:32px}h1{font-size:24px}p{line-height:1.6;opacity:.7}</style></head><body><main><h1>Return to T3 Code</h1><p>Your sign-in response has been received. T3 Code is finishing the connection. You can close this tab.</p></main></body></html>"

    private final class Listener {
        let request: T3CodexAuthRules.Request
        var fd: Int32 = -1
        var source: DispatchSourceRead?
        var timer: DispatchWorkItem?
        var done = false
        init(request: T3CodexAuthRules.Request) { self.request = request }
    }
    private let queue = DispatchQueue(label: "t3.codex-auth")
    private var listeners: [String: Listener] = [:]
    private var outcomes: [String: [String: Any]] = [:]
    /// The topic `codexAuthTake` readers watch, the window reveal and the journal; the module sets them.
    var changed: () -> Void = {}
    var reveal: () -> Void = {}
    var log: (String) -> Void = { _ in }

    /// receiveCodexAuthCallback up to the wait: validate, refuse a second listener for the same
    /// sign-in, bind 127.0.0.1:<redirect port>, then open the browser. nil, or the failure text.
    func start(_ authorizationUrl: String, timeout: TimeInterval = 300, open: (String) -> Bool) -> String? {
        let request: T3CodexAuthRules.Request
        do { request = try T3CodexAuthRules.authorizationRequest(authorizationUrl) } catch { return (error as? T3CodexAuthRules.Invalid)?.message ?? "Invalid ChatGPT sign-in request." }
        let listener = Listener(request: request)
        let refused: String? = queue.sync {
            if listeners[request.state] != nil { return "This sign-in is already open on this computer." }
            outcomes[request.state] = nil
            listeners[request.state] = listener
            return nil
        }
        if let refused { return refused }
        guard bind(listener) else { finish(listener, outcome: nil, reason: "port in use"); return Self.portInUse }
        log("codex-auth: listening on 127.0.0.1:\(request.port)")
        let timer = DispatchWorkItem { [weak self] in self?.finish(listener, outcome: ["phase": "failed", "message": "Sign-in expired. Try again."], reason: "expired") }
        listener.timer = timer
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout, execute: timer)
        if queue.sync(execute: { listener.done }) { return "Sign-in cancelled on this computer." }
        guard open(request.authorizationUrl) else { finish(listener, outcome: nil, reason: "browser"); return "Could not open your sign-in browser." }
        return nil
    }

    /// cancelCodexAuthCallback: the listener for this request's `state` stops and frees its port.
    func cancel(_ authorizationUrl: String) {
        guard let request = try? T3CodexAuthRules.authorizationRequest(authorizationUrl),
              let listener = queue.sync(execute: { listeners[request.state] }) else { return }
        finish(listener, outcome: ["phase": "failed", "message": "Sign-in cancelled on this computer."], reason: "cancelled")
    }

    /// The outcome once: `received` with the callback URL, `failed` with the reason, else `waiting` or `none`.
    func take(_ authorizationUrl: String) -> [String: Any] {
        guard let request = try? T3CodexAuthRules.authorizationRequest(authorizationUrl) else { return ["phase": "none"] }
        return queue.sync {
            if let outcome = outcomes.removeValue(forKey: request.state) { return outcome }
            return listeners[request.state] != nil ? ["phase": "waiting"] : ["phase": "none"]
        }
    }

    /// Whether a listener holds this request's state (tests).
    func listening(_ authorizationUrl: String) -> Bool {
        guard let request = try? T3CodexAuthRules.authorizationRequest(authorizationUrl) else { return false }
        return queue.sync { listeners[request.state] != nil }
    }

    private func bind(_ listener: Listener) -> Bool {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        var yes: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, socklen_t(MemoryLayout<Int32>.size))
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = listener.request.port.bigEndian
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        let bound = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        guard bound == 0, listen(fd, 16) == 0 else { close(fd); return false }
        _ = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK)
        listener.fd = fd
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: queue)
        source.setEventHandler { [weak self, weak listener] in
            guard let self, let listener, !listener.done else { return }
            while true {
                let client = accept(fd, nil, nil)
                if client < 0 { break }
                DispatchQueue.global(qos: .userInitiated).async { self.serve(client, listener) }
            }
        }
        source.setCancelHandler { close(fd) }
        listener.source = source
        source.resume()
        return true
    }

    /// One connection: GET on the callback path with the right state answers the page and finishes; anything else is a 400.
    private func serve(_ client: Int32, _ listener: Listener) {
        _ = fcntl(client, F_SETFL, fcntl(client, F_GETFL) & ~O_NONBLOCK) // an accepted socket inherits the listener's O_NONBLOCK
        var timeout = timeval(tv_sec: 5, tv_usec: 0)
        setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        var yes: Int32 = 1
        setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &yes, socklen_t(MemoryLayout<Int32>.size))
        var data = Data(), buffer = [UInt8](repeating: 0, count: 4096)
        while data.count < 32_768, data.range(of: Data("\r\n\r\n".utf8)) == nil {
            let count = read(client, &buffer, buffer.count)
            if count <= 0 { break }
            data.append(buffer, count: count)
        }
        let line = String(decoding: data.prefix(while: { $0 != 13 && $0 != 10 }), as: UTF8.self).split(separator: " ")
        var callback: String?
        if line.count >= 2, line[0] == "GET", let target = line.count >= 2 ? String(line[1]) : nil, target.hasPrefix("/") {
            let value = "http://127.0.0.1:\(listener.request.port)\(target)"
            callback = try? T3CodexAuthRules.callbackUrl(value, redirectUri: listener.request.redirectUri, state: listener.request.state)
        }
        var accepted = false
        if callback != nil { accepted = queue.sync { () -> Bool in if listener.done { return false }; listener.done = true; return true } }
        if accepted, let callback {
            let body = Data(Self.page.utf8)
            let head = "HTTP/1.1 200 OK\r\ncache-control: no-store\r\nreferrer-policy: no-referrer\r\nx-content-type-options: nosniff\r\ncontent-security-policy: default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: \(body.count)\r\nconnection: close\r\n\r\n"
            write(client, Data(head.utf8) + body)
            close(client)
            log("codex-auth: received the callback on 127.0.0.1:\(listener.request.port)")
            finish(listener, outcome: ["phase": "received", "callbackUrl": callback], reason: "received", already: true)
            reveal()
        } else {
            let body = Data("This response does not belong to the active sign-in.".utf8)
            write(client, Data("HTTP/1.1 400 Bad Request\r\ncontent-length: \(body.count)\r\nconnection: close\r\n\r\n".utf8) + body)
            close(client)
            log("codex-auth: refused a request on 127.0.0.1:\(listener.request.port) (400)")
        }
    }

    private func write(_ client: Int32, _ data: Data) {
        data.withUnsafeBytes { raw in
            var offset = 0
            while offset < raw.count {
                let sent = Darwin.write(client, raw.baseAddress!.advanced(by: offset), raw.count - offset)
                if sent <= 0 { break }
                offset += sent
            }
        }
    }

    /// Stop listening, free the port, keep the outcome for `take`, announce it.
    private func finish(_ listener: Listener, outcome: [String: Any]?, reason: String, already: Bool = false) {
        let finished: Bool = queue.sync {
            if listener.done && !already { return false }
            listener.done = true
            listener.timer?.cancel(); listener.timer = nil
            listener.source?.cancel(); listener.source = nil
            listener.fd = -1
            if listeners[listener.request.state] === listener { listeners[listener.request.state] = nil }
            if let outcome { outcomes[listener.request.state] = outcome }
            return true
        }
        guard finished else { return }
        log("codex-auth: released 127.0.0.1:\(listener.request.port) (\(reason))")
        if outcome != nil { changed() }
    }
}

extension T3Module {
    /// codexAuthStart / codexAuthCancel / codexAuthTake (codex-setup-ops.ts).
    func codexAuthOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        let op = request["op"] as? String ?? ""
        guard op.hasPrefix("codexAuth") else { return next() }
        let generation = request["generation"] as? Int ?? 0, url = request["authorizationUrl"] as? String ?? ""
        let auth = T3CodexAuth.shared, agent = context.agent, changed = context.changed, diagnostics = context.diagnostics
        auth.changed = { changed("t3.codexAuth") }
        auth.log = { line in diagnostics.log(line) }
        auth.reveal = {
            DispatchQueue.main.async {
                // DesktopWindow.reveal: show, restore and focus the main window. An agent run leaves the user's focus alone.
                if agent { diagnostics.log("codex-auth: reveal the window (agent run: recorded)"); return }
                NSApp.activate(ignoringOtherApps: true)
                let window = NSApp.mainWindow ?? NSApp.windows.first { $0.isVisible || $0.isMiniaturized }
                if window?.isMiniaturized == true { window?.deminiaturize(nil) }
                window?.makeKeyAndOrderFront(nil)
            }
        }
        let success = { (value: [String: Any]) in reply.send(["ok": true, "generation": generation, "value": value]) }
        DispatchQueue.global(qos: .userInitiated).async {
            switch op {
            case "codexAuthStart":
                let timeout = agent ? (Double(ProcessInfo.processInfo.environment["T3_CODEX_AUTH_TIMEOUT_MS"] ?? "") ?? 300_000) / 1000 : 300
                if let failure = auth.start(url, timeout: timeout, open: { T3RemoteEditors.openOrRecord($0, agent: agent) }) {
                    reply.send(["ok": false, "generation": generation, "error": ["kind": "CodexAuth", "message": failure, "uncertain": false]])
                } else { success(["listening": true]) }
            case "codexAuthCancel": auth.cancel(url); success([:])
            case "codexAuthTake": success(auth.take(url))
            default: reply.send(["ok": false, "generation": generation, "error": ["kind": "Arguments", "message": "Unknown op \(op).", "uncertain": false]])
            }
        }
    }
}

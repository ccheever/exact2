// The network form of the dev plan (LLP 1023 D1–D3, Stage 1): given the app
// URL — the same one a browser opens — resolve the envelope (negotiated
// directly, or through index.html's rel=alternate link), fetch and verify
// the plan, hand the bytes to the app (which applies them to every session
// through the restart-with-carry path), and subscribe to the server's
// events so an edit re-fetches. Transactional: a candidate replaces the
// running app only after its bytes hash clean and boot runs; every failure
// keeps the last good app on screen. `{rebuilt}` is session-terminal — the
// wasm-side program changed, so a new plan against this binary's data crate
// would be a poisoned boot; the menu's Reload starts over after the host is
// rebuilt. One connection per app (LLP 1031 D11), never per session.
import CryptoKit
import Foundation

/// One bounded HTTP rung. URLSession's completion-handler API buffers an
/// entire response before the caller can inspect it; the loader instead owns
/// a data delegate so every chunk is stopped at the rung's limit. Redirects
/// stay on the app origin before another request is issued.
private final class BoundedFetch: NSObject, URLSessionDataDelegate, URLSessionTaskDelegate {
    struct Failure: Error { let message: String }
    typealias Completion = (Result<(Data, HTTPURLResponse), Failure>) -> Void

    let origin: URL
    let limit: Int
    let completion: Completion
    private var bytes = Data()
    private var response: HTTPURLResponse?
    private var finished = false
    private var task: URLSessionDataTask?
    private lazy var session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.timeoutIntervalForRequest = 10
        config.timeoutIntervalForResource = 30
        return URLSession(configuration: config, delegate: self, delegateQueue: .main)
    }()

    init(origin: URL, limit: Int, completion: @escaping Completion) {
        self.origin = origin
        self.limit = limit
        self.completion = completion
    }

    func start(_ request: URLRequest) {
        let task = session.dataTask(with: request)
        self.task = task
        task.resume()
    }

    func cancel() {
        finish(.failure(Failure(message: "cancelled")))
    }

    private func finish(_ result: Result<(Data, HTTPURLResponse), Failure>) {
        guard !finished else { return }
        finished = true
        task?.cancel()
        session.invalidateAndCancel()
        completion(result)
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        guard let next = request.url, PlanURL.sameOrigin(next, origin) else {
            finish(.failure(Failure(message: "refused cross-origin redirect to \(request.url?.absoluteString ?? "an invalid URL")")))
            completionHandler(nil)
            return
        }
        completionHandler(request)
    }

    func urlSession(
        _ session: URLSession,
        dataTask: URLSessionDataTask,
        didReceive response: URLResponse,
        completionHandler: @escaping (URLSession.ResponseDisposition) -> Void
    ) {
        guard let http = response as? HTTPURLResponse,
              let final = http.url,
              PlanURL.sameOrigin(final, origin) else {
            finish(.failure(Failure(message: "refused a response outside the app origin")))
            completionHandler(.cancel)
            return
        }
        if response.expectedContentLength > Int64(limit) {
            finish(.failure(Failure(message: "response exceeded the \(limit)-byte limit")))
            completionHandler(.cancel)
            return
        }
        self.response = http
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard data.count <= limit - bytes.count else {
            finish(.failure(Failure(message: "response exceeded the \(limit)-byte limit")))
            return
        }
        bytes.append(data)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error {
            finish(.failure(Failure(message: error.localizedDescription)))
        } else if let response {
            finish(.success((bytes, response)))
        } else {
            finish(.failure(Failure(message: "response had no HTTP status")))
        }
    }
}

final class PlanURL: NSObject, URLSessionDataDelegate {
    static let envelopeType = "application/vnd.exact.envelope+json"
    static let pageLimit = 64 * 1024
    static let envelopeLimit = 64 * 1024
    static let planLimit = 64 * 1024 * 1024
    static let eventLimit = 64 * 1024

    /// The env locator accepts a path or a URL; this decides which.
    static func isURL(_ value: String) -> Bool {
        value.hasPrefix("http://") || value.hasPrefix("https://")
    }

    /// URL.origin for the two schemes the loader admits. Foundation's
    /// `host` omits the port, so compare all three origin fields explicitly.
    static func sameOrigin(_ a: URL, _ b: URL) -> Bool {
        guard let aScheme = a.scheme?.lowercased(),
              let bScheme = b.scheme?.lowercased(),
              let aHost = a.host?.lowercased(),
              let bHost = b.host?.lowercased(),
              let aDefault = defaultPort(aScheme),
              let bDefault = defaultPort(bScheme) else { return false }
        return aScheme == bScheme && aHost == bHost
            && (a.port ?? aDefault) == (b.port ?? bDefault)
    }

    private static func defaultPort(_ scheme: String) -> Int? {
        switch scheme {
        case "http": return 80
        case "https": return 443
        default: return nil
        }
    }

    /// Connect to `page` — the app URL. `apply` boots verified plan bytes
    /// into the app's sessions and returns true only when the candidate
    /// became the running app; the digest is committed with that success,
    /// never merely with a good hash.
    static func open(_ page: String, apply: @escaping (Data, String) -> Bool) -> PlanURL? {
        guard let u = URL(string: page), u.host != nil else {
            print("exact url: not a URL: \(page)")
            return nil
        }
        let s = PlanURL(u, apply: apply)
        s.resolveAndBoot(thenSubscribe: true)
        return s
    }

    let page: URL
    private let applyPlan: (Data, String) -> Bool
    /// "rebuild the native host" once `{rebuilt}` arrives; Reload clears it.
    var terminal: String?
    private var bootedDigest = ""
    private var eventsURL: URL?
    private var stream: URLSessionDataTask?
    private var buffer = Data()
    private var retrySeconds = 1.0
    private var closed = false
    private var resolution: UInt64 = 0
    private var fetches: [UUID: BoundedFetch] = [:]
    private lazy var session: URLSession = {
        let c = URLSessionConfiguration.ephemeral
        c.requestCachePolicy = .reloadIgnoringLocalCacheData
        // The first local-network request can stall while iOS resolves the
        // permission prompt; wait rather than fail it (LLP 1023 §6).
        c.waitsForConnectivity = true
        c.timeoutIntervalForResource = 3600 * 24 // the SSE stream is long-lived
        return URLSession(configuration: c, delegate: self, delegateQueue: .main)
    }()

    private init(_ u: URL, apply: @escaping (Data, String) -> Bool) {
        page = u
        applyPlan = apply
    }

    func close() {
        closed = true
        resolution &+= 1
        Array(fetches.values).forEach { $0.cancel() }
        fetches.removeAll()
        stream?.cancel()
        session.invalidateAndCancel()
    }

    /// The menu's Reload: start over — clears a `{rebuilt}` stop.
    func reload() {
        terminal = nil
        stream?.cancel()
        stream = nil
        resolveAndBoot(thenSubscribe: true)
    }

    // MARK: the two rungs (D1)

    private func resolveAndBoot(thenSubscribe: Bool) {
        resolution &+= 1
        let generation = resolution
        Array(fetches.values).forEach { $0.cancel() }
        fetches.removeAll()
        var req = URLRequest(url: page)
        req.setValue("\(PlanURL.envelopeType), text/html;q=0.9", forHTTPHeaderField: "Accept")
        req.setValue("no-cache", forHTTPHeaderField: "Cache-Control")
        fetch(req, limit: PlanURL.pageLimit, generation: generation) { [weak self] result in
            guard let self else { return }
            guard case let .success((data, http)) = result, http.statusCode == 200 else {
                let reason = if case let .failure(error) = result { error.message } else { "HTTP response refused" }
                self.status("cannot reach \(self.page): \(reason)")
                return
            }
            let base = http.url ?? self.page // final URL after redirects: the resolution base (D2)
            let type = (http.value(forHTTPHeaderField: "Content-Type") ?? "").lowercased()
            if type.contains("json") || data.first == UInt8(ascii: "{") {
                self.takeEnvelope(data, base: base, thenSubscribe: thenSubscribe, generation: generation)
            } else if let href = PlanURL.envelopeLink(in: data), let u = URL(string: href, relativeTo: base) {
                guard PlanURL.sameOrigin(u.absoluteURL, self.page) else {
                    self.status("refused: \(u.absoluteURL) is not same-origin with \(self.page)")
                    return
                }
                self.fetchEnvelope(u.absoluteURL, thenSubscribe: thenSubscribe, generation: generation)
            } else {
                self.status("\(base) is not an Exact app: no envelope and no rel=alternate link in the page")
            }
        }
    }

    private func fetchEnvelope(_ u: URL, thenSubscribe: Bool, generation: UInt64) {
        var req = URLRequest(url: u)
        req.setValue(PlanURL.envelopeType, forHTTPHeaderField: "Accept")
        fetch(req, limit: PlanURL.envelopeLimit, generation: generation) { [weak self] result in
            guard let self else { return }
            guard case let .success((data, response)) = result, response.statusCode == 200 else {
                self.status("cannot fetch the envelope at \(u)")
                return
            }
            self.takeEnvelope(data, base: response.url ?? u, thenSubscribe: thenSubscribe, generation: generation)
        }
    }

    private func fetch(
        _ request: URLRequest,
        limit: Int,
        generation: UInt64,
        completion: @escaping (Result<(Data, HTTPURLResponse), BoundedFetch.Failure>) -> Void
    ) {
        let id = UUID()
        let fetch = BoundedFetch(origin: page, limit: limit) { [weak self] result in
            guard let self else { return }
            self.fetches.removeValue(forKey: id)
            guard !self.closed, generation == self.resolution else { return }
            completion(result)
        }
        fetches[id] = fetch
        fetch.start(request)
    }

    /// The link rung: a bounded, parser-free scan of the page's first 16 KB
    /// (exact1 LLP 0268's rule — native never runs an HTML parser, and
    /// never executes the page).
    static func envelopeLink(in html: Data) -> String? {
        let text = String(decoding: html.prefix(16384), as: UTF8.self)
        guard let type = text.range(of: envelopeType) else { return nil }
        guard let open = text.range(of: "<link", options: .backwards, range: text.startIndex..<type.lowerBound),
              let close = text.range(of: ">", range: type.upperBound..<text.endIndex) else { return nil }
        let tag = text[open.lowerBound..<close.upperBound]
        for quote in ["\"", "'"] {
            if let h = tag.range(of: "href=" + quote), let end = tag.range(of: quote, range: h.upperBound..<tag.endIndex) {
                return String(tag[h.upperBound..<end.lowerBound]).replacingOccurrences(of: "&amp;", with: "&")
            }
        }
        return nil
    }

    // MARK: the envelope and the plan (D2)

    private func takeEnvelope(_ data: Data, base: URL, thenSubscribe: Bool, generation: UInt64) {
        guard data.count <= PlanURL.envelopeLimit, PlanURL.sameOrigin(base, page) else {
            status("refused an oversized or cross-origin envelope at \(base)")
            return
        }
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            self.status("the envelope at \(base) is not JSON")
            return
        }
        guard let major = json["exact"] as? Int else {
            self.status("\(base): no \"exact\" version — not an envelope")
            return
        }
        guard major == 1 else {
            self.status("envelope version \(major) is newer than this host — rebuild the native host")
            return
        }
        guard let plan = json["plan"] as? [String: Any],
              let path = plan["url"] as? String,
              let sha = plan["sha256"] as? String,
              let count = plan["bytes"] as? Int,
              let planURL = URL(string: path, relativeTo: base)?.absoluteURL else {
            self.status("\(base): the envelope names no plan")
            return
        }
        guard count >= 0, count <= PlanURL.planLimit else {
            self.status("refused: plan byte count \(count) exceeds the \(PlanURL.planLimit)-byte limit")
            return
        }
        // Same-origin only in v1 (D2): everything lives beside the page.
        guard PlanURL.sameOrigin(planURL, page) else {
            self.status("refused: \(planURL) is not same-origin with \(page)")
            return
        }
        let events = (json["events"] as? String).flatMap { URL(string: $0, relativeTo: base)?.absoluteURL }
        if let events, !PlanURL.sameOrigin(events, page) {
            self.status("refused: \(events) is not same-origin with \(page)")
            return
        }
        fetchPlan(planURL, sha256: sha.lowercased(), count: count, generation: generation)
        if thenSubscribe, let events {
            eventsURL = events
            subscribe(events)
        }
    }

    private func fetchPlan(_ u: URL, sha256: String, count: Int, generation: UInt64) {
        var request = URLRequest(url: u)
        request.setValue("application/vnd.exact.plan", forHTTPHeaderField: "Accept")
        fetch(request, limit: count, generation: generation) { [weak self] result in
            guard let self, self.terminal == nil else { return }
            guard case let .success((data, response)) = result, response.statusCode == 200 else {
                self.status("cannot fetch the plan at \(u); keeping the running app")
                return
            }
            guard data.count == count else {
                self.status("plan is \(data.count) bytes, envelope said \(count); keeping the running app")
                return
            }
            let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
            guard digest == sha256 else {
                self.status("plan sha256 mismatch at \(u); keeping the running app")
                return
            }
            guard digest != self.bootedDigest else { return }
            if self.applyPlan(data, u.lastPathComponent + " ← " + (self.page.host ?? "")) {
                self.bootedDigest = digest
            }
        }
    }

    // MARK: the events stream (D3)

    private func subscribe(_ u: URL) {
        buffer.removeAll()
        var req = URLRequest(url: u)
        req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        let t = session.dataTask(with: req)
        stream = t
        t.resume()
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard dataTask === stream else { return }
        guard data.count <= PlanURL.eventLimit - buffer.count else {
            status("events response exceeded the \(PlanURL.eventLimit)-byte frame limit")
            buffer.removeAll()
            stream?.cancel()
            return
        }
        buffer.append(data)
        while let frame = nextFrame() {
            handle(frame)
        }
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        guard task === stream else { completionHandler(nil); return }
        guard let next = request.url, PlanURL.sameOrigin(next, page) else {
            status("refused cross-origin events redirect to \(request.url?.absoluteString ?? "an invalid URL")")
            completionHandler(nil)
            return
        }
        completionHandler(request)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard task === stream, !closed, terminal == nil else { return }
        if (error as? URLError)?.code == .cancelled { return }
        // Reconnect; the server's hello re-syncs whatever was missed.
        let delay = retrySeconds
        retrySeconds = min(retrySeconds * 2, 5)
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            guard let self, !self.closed, self.terminal == nil else { return }
            self.resolveAndBoot(thenSubscribe: true)
        }
    }

    private func nextFrame() -> [String: Any]? {
        let sep = Data("\n\n".utf8)
        guard let r = buffer.range(of: sep) else { return nil }
        let chunk = buffer.subdata(in: buffer.startIndex..<r.lowerBound)
        buffer.removeSubrange(buffer.startIndex..<r.upperBound)
        guard let text = String(data: chunk, encoding: .utf8) else { return [:] }
        for line in text.split(separator: "\n") where line.hasPrefix("data: ") {
            if let json = try? JSONSerialization.jsonObject(with: Data(line.dropFirst(6).utf8)) as? [String: Any] {
                return json
            }
        }
        return [:] // a comment frame (the stream's opening ":")
    }

    private func handle(_ frame: [String: Any]) {
        retrySeconds = 1
        if frame["rebuilt"] != nil {
            // Session-terminal (D3): the program changed; a new plan against
            // this binary's data crate cannot be trusted to boot.
            terminal = "the dev server rebuilt the app's native code — rebuild and relaunch this host (the last good plan is still running)"
            status(terminal!)
            stream?.cancel()
            return
        }
        if let e = frame["error"] as? String {
            status("dev: \(e)")
            return
        }
        // hello and {seq}: both name the current revision; boot on mismatch.
        if let digest = frame["digest"] as? String, digest != bootedDigest, terminal == nil {
            resolveAndBoot(thenSubscribe: false)
        }
    }

    private func status(_ message: String) {
        print("exact url: \(message)")
    }
}

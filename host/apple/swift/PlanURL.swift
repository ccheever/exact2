// The network form of the dev plan (LLP 1023 D1–D3, Stage 1): given the app
// URL — the same one a browser opens — resolve the envelope (negotiated
// directly, or through index.html's rel=alternate link), fetch and verify
// the plan, boot it through the host's own dev-reload path, and subscribe
// to the server's events so an edit re-fetches. Transactional: a candidate
// replaces the running app only after its bytes hash clean and boot runs;
// every failure keeps the last good app on screen. `{rebuilt}` is
// session-terminal — the wasm-side program changed, so a new plan against
// this binary's data crate would be a poisoned boot; the menu's Reload
// starts over after the host is rebuilt.
import CryptoKit
import Foundation

final class PlanURL: NSObject, URLSessionDataDelegate {
    static let envelopeType = "application/vnd.exact.envelope+json"
    /// Wired by the host at startup: boot these verified plan bytes — the
    /// presenter reset + bootPlan + apply the file watcher already does.
    nonisolated(unsafe) static var boot: ((Data, String) -> Void)?
    nonisolated(unsafe) static var current: PlanURL?

    /// The env locator accepts a path or a URL; this decides which.
    static func isURL(_ value: String) -> Bool {
        value.hasPrefix("http://") || value.hasPrefix("https://")
    }

    /// Connect to `page` — the app URL — replacing any current session.
    static func open(_ page: String) {
        current?.close()
        current = nil
        guard let u = URL(string: page), u.host != nil else {
            print("exact url: not a URL: \(page)")
            return
        }
        let s = PlanURL(u)
        current = s
        s.resolveAndBoot(thenSubscribe: true)
    }

    let page: URL
    /// "rebuild the native host" once `{rebuilt}` arrives; Reload clears it.
    var terminal: String?
    private var bootedDigest = ""
    private var eventsURL: URL?
    private var stream: URLSessionDataTask?
    private var buffer = Data()
    private var retrySeconds = 1.0
    private var closed = false
    private lazy var session: URLSession = {
        let c = URLSessionConfiguration.ephemeral
        c.requestCachePolicy = .reloadIgnoringLocalCacheData
        // The first local-network request can stall while iOS resolves the
        // permission prompt; wait rather than fail it (LLP 1023 §6).
        c.waitsForConnectivity = true
        c.timeoutIntervalForResource = 3600 * 24 // the SSE stream is long-lived
        return URLSession(configuration: c, delegate: self, delegateQueue: .main)
    }()

    private init(_ u: URL) { page = u }

    func close() {
        closed = true
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
        var req = URLRequest(url: page)
        req.setValue("\(PlanURL.envelopeType), text/html;q=0.9", forHTTPHeaderField: "Accept")
        req.setValue("no-cache", forHTTPHeaderField: "Cache-Control")
        session.dataTask(with: req) { [weak self] data, response, error in
            guard let self else { return }
            guard let data, let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                self.status("cannot reach \(self.page): \(error?.localizedDescription ?? "HTTP \((response as? HTTPURLResponse)?.statusCode ?? 0))")")
                return
            }
            let base = http.url ?? self.page // final URL after redirects: the resolution base (D2)
            let type = (http.value(forHTTPHeaderField: "Content-Type") ?? "").lowercased()
            if type.contains("json") || data.first == UInt8(ascii: "{") {
                self.takeEnvelope(data, base: base, thenSubscribe: thenSubscribe)
            } else if let href = PlanURL.envelopeLink(in: data), let u = URL(string: href, relativeTo: base) {
                self.fetchEnvelope(u.absoluteURL, thenSubscribe: thenSubscribe)
            } else {
                self.status("\(base) is not an Exact app: no envelope and no rel=alternate link in the page")
            }
        }.resume()
    }

    private func fetchEnvelope(_ u: URL, thenSubscribe: Bool) {
        var req = URLRequest(url: u)
        req.setValue(PlanURL.envelopeType, forHTTPHeaderField: "Accept")
        session.dataTask(with: req) { [weak self] data, response, _ in
            guard let self else { return }
            guard let data, (response as? HTTPURLResponse)?.statusCode == 200 else {
                self.status("cannot fetch the envelope at \(u)")
                return
            }
            self.takeEnvelope(data, base: (response?.url ?? u), thenSubscribe: thenSubscribe)
        }.resume()
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

    private func takeEnvelope(_ data: Data, base: URL, thenSubscribe: Bool) {
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
        // Same-origin only in v1 (D2): everything lives beside the page.
        guard planURL.host == page.host else {
            self.status("refused: \(planURL) is not on \(page.host ?? "?")")
            return
        }
        let events = (json["events"] as? String).flatMap { URL(string: $0, relativeTo: base)?.absoluteURL }
        fetchPlan(planURL, sha256: sha.lowercased(), count: count)
        if thenSubscribe, let events, events.host == page.host {
            eventsURL = events
            subscribe(events)
        }
    }

    private func fetchPlan(_ u: URL, sha256: String, count: Int) {
        session.dataTask(with: URLRequest(url: u)) { [weak self] data, response, _ in
            guard let self, self.terminal == nil else { return }
            guard let data, (response as? HTTPURLResponse)?.statusCode == 200 else {
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
            self.bootedDigest = digest
            PlanURL.boot?(data, u.lastPathComponent + " ← " + (self.page.host ?? ""))
        }.resume()
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
        buffer.append(data)
        while let frame = nextFrame() {
            handle(frame)
        }
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

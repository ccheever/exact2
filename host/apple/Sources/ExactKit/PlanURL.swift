// The network form of the dev plan (LLP 1023 D1–D3, Stage 1): given the app
// URL — the same one a browser opens — resolve the envelope (negotiated
// directly, or through index.html's rel=alternate link), fetch and verify
// the complete plan and asset manifest, hand the verified generation to
// the app for all-session acceptance, and subscribe to the server's events
// so an edit re-fetches. Transactional: a candidate replaces the
// running app only after its bytes hash clean and boot runs; every failure
// keeps the last good app on screen. `{rebuilt}` is session-terminal — the
// wasm-side program changed, so a new plan against this binary's data crate
// would be a poisoned boot; the menu's Reload starts over after the host is
// rebuilt. One connection per app (LLP 1031 D11), never per session.
import CryptoKit
import Foundation

/// A fixed JavaScript/Rust generation crosses the data seam as one candidate.
private enum DevelopmentModules {
    static func combine(javascript: ExactModule?, rust: ExactModule?) throws -> ExactModule? {
        guard let javascript else { return rust }
        guard let rust else { return javascript }
        guard let jsReceipt = try JSONSerialization.jsonObject(with: javascript.receipt) as? [String: Any],
              let rustReceipt = try JSONSerialization.jsonObject(with: rust.receipt) as? [String: Any] else {
            throw CocoaError(.fileReadCorruptFile)
        }
        let receipt = try JSONSerialization.data(withJSONObject: [
            "version": 1, "kind": "mixed", "javascript": jsReceipt, "rust": rustReceipt,
            "javascriptBytes": javascript.bytecode.count,
        ])
        guard receipt.count <= 1 << 20, javascript.bytecode.count <= 32 << 20,
              rust.bytecode.count <= (32 << 20) - javascript.bytecode.count else { throw CocoaError(.fileReadTooLarge) }
        var bytes = javascript.bytecode
        bytes.append(rust.bytecode)
        return ExactModule(receipt: receipt, bytecode: bytes)
    }
}

/// A local development bake is a plan plus its optional, inseparable module
/// pair (LLP 1027 D7). Watch all three files: a writer can replace them in
/// either order, and a refused partial bake must be retried when it finishes.
public struct ExactDevelopmentPlan {
    private let baseFiles: [URL]
    private var files: [URL] {
        let root = baseFiles[0].deletingLastPathComponent().appendingPathComponent("rust")
        let found = (FileManager.default.enumerator(at: root, includingPropertiesForKeys: nil)?.allObjects as? [URL] ?? [])
            .filter { !$0.hasDirectoryPath }.sorted { $0.path < $1.path }
        return baseFiles + found
    }
    public init(_ path: String) {
        let plan = URL(fileURLWithPath: path)
        let directory = plan.deletingLastPathComponent()
        baseFiles = [plan, directory.appendingPathComponent("app.module.json"), directory.appendingPathComponent("app.hbc")]
    }

    public var hasModule: Bool { files.dropFirst().contains { FileManager.default.fileExists(atPath: $0.path) } }

    /// Identity and subsecond modification time, including missing files.
    public var revision: [Double] {
        files.flatMap { url -> [Double] in
            let a = try? FileManager.default.attributesOfItem(atPath: url.path)
            return [(a?[.modificationDate] as? Date)?.timeIntervalSinceReferenceDate ?? -1,
                    (a?[.size] as? NSNumber)?.doubleValue ?? -1,
                    (a?[.systemFileNumber] as? NSNumber)?.doubleValue ?? -1]
        }
    }

    @discardableResult
    public func apply(to app: ExactApp) -> Bool {
        let files = self.files
        do {
            func read(_ index: Int, limit: Int) throws -> Data {
                let url = files[index]
                let a = try FileManager.default.attributesOfItem(atPath: url.path)
                guard let size = a[.size] as? NSNumber, size.intValue <= limit else {
                    throw NSError(domain: "ExactDevelopmentPlan", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(url.lastPathComponent) exceeds the development artifact limit"])
                }
                let bytes = try Data(contentsOf: url)
                guard bytes.count <= limit else { throw CocoaError(.fileReadTooLarge) }
                return bytes
            }
            let plan = try read(0, limit: 32 << 20)
            let label = files[0].lastPathComponent
            if hasModule {
                var javascript: ExactModule?, rust: ExactModule?
                if files[1...2].contains(where: { FileManager.default.fileExists(atPath: $0.path) }) {
                    javascript = ExactModule(receipt: try read(1, limit: 1 << 20), bytecode: try read(2, limit: 32 << 20))
                }
                if files.count > baseFiles.count && !(app.rustPolicy.mode == "off" && javascript != nil) {
                    let policy = app.rustPolicy
                    let prefix = policy.mode == "wasm" ? "rust/wasm" : "rust/" + policy.mode + "/" + policy.target
                    guard policy.mode != "off",
                          let index = files.firstIndex(where: { $0.path.hasSuffix(prefix + "/app.module.json") }) else {
                        throw NSError(domain: "ExactDevelopmentPlan", code: 1, userInfo: [NSLocalizedDescriptionKey: "no Rust replacement matching this binary's executor and target"])
                    }
                    let receiptIndex = index
                    let json = try JSONSerialization.jsonObject(with: read(index, limit: 1 << 20)) as? [String: Any]
                    guard let row = json?["module"] as? [String: Any], let name = row["file"] as? String,
                          ["app.module.wasm", "app.module.dylib", "app.module.so", "app.module.dll", "app.module.bin"].contains(name),
                          let index = files.firstIndex(where: { $0.path.hasSuffix(prefix + "/" + name) }) else { throw CocoaError(.fileNoSuchFile) }
                    rust = ExactModule(receipt: try read(receiptIndex, limit: 1 << 20), bytecode: try read(index, limit: 32 << 20))
                }
                guard let module = try DevelopmentModules.combine(javascript: javascript, rust: rust) else { throw CocoaError(.fileNoSuchFile) }
                return app.applyGeneration(ExactGeneration(plan: plan, assets: app.resolver, module: module), label: label, commit: { true })
            }
            return app.apply(plan, label: label)
        } catch {
            fputs("exact: local generation refused: \(error.localizedDescription)\n", stderr)
            return false
        }
    }
}

/// Explicit development-only opening action (LLP 1030.000 §7). The locator
/// stays an HTTP(S) URL; the outer app-specific scheme only selects the client.
public enum ExactDevelopmentLink {
    public static func page(_ link: URL, scheme: String) -> URL? {
        guard link.absoluteString.utf8.count <= 8192,
              let c = URLComponents(url: link, resolvingAgainstBaseURL: false),
              c.scheme?.lowercased() == scheme, c.host == "open",
              c.user == nil, c.password == nil, c.port == nil, c.fragment == nil,
              c.path.isEmpty || c.path == "/",
              let items = c.queryItems, items.count == 1, items[0].name == "url",
              let value = items[0].value, let page = URL(string: value),
              ["http", "https"].contains(page.scheme?.lowercased() ?? ""),
              let host = page.host, !host.isEmpty, page.user == nil, page.password == nil
        else { return nil }
        return page
    }

    public static func page(_ link: URL) -> URL? {
        guard let scheme = Bundle.main.object(forInfoDictionaryKey: "ExactDevelopmentURLScheme") as? String else { return nil }
        return page(link, scheme: scheme)
    }

    @discardableResult
    public static func open(_ link: URL) -> Bool {
        guard let page = page(link) else { return false }
        // The existing loader still checks app identity, pairing, grants and
        // runtime compatibility before changing any session. A scheme is no trust.
        ExactApp.shared.connect(page.absoluteString)
        return true
    }
}

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
    static let assetLimit = 64 * 1024 * 1024
    static let eventLimit = 64 * 1024
    static let generationLimit = 256 * 1024 * 1024

    struct Generation {
        let identity: String
        let plan: Data
        let assets: [String: Data]
        let module: ExactModule?
    }
    private struct Revision: Equatable {
        let epoch: String
        let seq: Int
        let identity: String
        let program: String?
    }
    private struct FileCard {
        let name: String
        let url: URL
        let sha: String
        let count: Int
        var canonical: String {
            let named = name.isEmpty ? "" : ",\"name\":" + PlanURL.quote(name)
            return "{\"bytes\":\(count)\(named),\"sha256\":\(PlanURL.quote(sha))}"
        }
    }

    static func isURL(_ value: String) -> Bool {
        value.hasPrefix("http://") || value.hasPrefix("https://")
    }

    /// URL.origin, including the scheme's default port.
    static func sameOrigin(_ a: URL, _ b: URL) -> Bool {
        guard let aScheme = a.scheme?.lowercased(), let bScheme = b.scheme?.lowercased(),
              let aHost = a.host?.lowercased(), let bHost = b.host?.lowercased(),
              let aDefault = defaultPort(aScheme), let bDefault = defaultPort(bScheme) else { return false }
        return aScheme == bScheme && aHost == bHost && (a.port ?? aDefault) == (b.port ?? bDefault)
    }
    private static func defaultPort(_ scheme: String) -> Int? {
        switch scheme { case "http": return 80; case "https": return 443; default: return nil }
    }

    /// The app owns the identity: replacing a connection must not restart an
    /// already current app, while another kind of apply invalidates the identity.
    static func open(_ page: String, acceptProgram: @escaping (String?) -> Bool, current: @escaping () -> String?, waiting: @escaping () -> Bool = { false }, apply: @escaping (Generation, String) -> Bool) -> PlanURL? {
        guard let u = URL(string: page), u.host != nil, sameOrigin(u, u) else {
            fputs("exact url: not an HTTP app URL: \(page)\n", stderr)
            return nil
        }
        let connection = PlanURL(u, acceptProgram: acceptProgram, current: current, waiting: waiting, apply: apply)
        connection.resolveAndBoot()
        return connection
    }

    let page: URL
    private let currentGeneration: () -> String?
    private let applyGeneration: (Generation, String) -> Bool
    private let preparationPending: () -> Bool
    private let acceptProgram: (String?) -> Bool
    var terminal: String?
    private var observed: Revision?
    private var retiredEpochs = Set<String>()
    private var pending: Revision?
    private var stream: URLSessionDataTask?
    private var buffer = Data()
    private var retrySeconds = 1.0
    private var closed = false
    private var resolution: UInt64 = 0
    private var resolutionStarted = ProcessInfo.processInfo.systemUptime
    private var fetches: [UUID: BoundedFetch] = [:]
    private lazy var session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.waitsForConnectivity = true
        config.timeoutIntervalForResource = 3600 * 24
        return URLSession(configuration: config, delegate: self, delegateQueue: .main)
    }()

    private init(_ page: URL, acceptProgram: @escaping (String?) -> Bool, current: @escaping () -> String?, waiting: @escaping () -> Bool = { false }, apply: @escaping (Generation, String) -> Bool) {
        self.page = page
        self.acceptProgram = acceptProgram
        currentGeneration = current
        applyGeneration = apply
        preparationPending = waiting
    }

    @discardableResult
    private func beginResolution() -> UInt64 {
        resolution &+= 1
        resolutionStarted = ProcessInfo.processInfo.systemUptime
        pending = nil
        Array(fetches.values).forEach { $0.cancel() }
        fetches.removeAll()
        return resolution
    }

    func close() {
        closed = true
        beginResolution()
        stream?.cancel()
        stream = nil
        session.invalidateAndCancel()
    }

    func reload() {
        terminal = nil
        resolveAndBoot()
    }

    // MARK: discovery and immutable generation fetches (LLP 1023 D1–D3)

    private func resolveAndBoot() {
        guard !closed, terminal == nil else { return }
        stream?.cancel()
        stream = nil
        let generation = beginResolution()
        var request = URLRequest(url: page)
        request.setValue("\(PlanURL.envelopeType), text/html;q=0.9", forHTTPHeaderField: "Accept")
        request.setValue("no-cache", forHTTPHeaderField: "Cache-Control")
        fetch(request, limit: PlanURL.pageLimit, generation: generation) { [weak self] result in
            guard let self else { return }
            guard case let .success((data, http)) = result, http.statusCode == 200 else {
                self.failed("cannot reach \(self.page)", generation: generation)
                return
            }
            let base = http.url ?? self.page
            let type = (http.value(forHTTPHeaderField: "Content-Type") ?? "").lowercased()
            if type.contains("json") || data.first == UInt8(ascii: "{") {
                self.takeEnvelope(data, base: base, subscribe: true, generation: generation, expected: nil)
            } else if let href = PlanURL.envelopeLink(in: data),
                      let url = URL(string: href, relativeTo: base)?.absoluteURL,
                      PlanURL.sameOrigin(url, self.page) {
                self.fetchEnvelope(url, subscribe: true, generation: generation, expected: nil)
            } else {
                self.failed("\(base) has no same-origin Exact envelope", generation: generation)
            }
        }
    }

    private func fetchEnvelope(_ url: URL, subscribe: Bool, generation: UInt64, expected: Revision?) {
        var request = URLRequest(url: url)
        request.setValue(PlanURL.envelopeType, forHTTPHeaderField: "Accept")
        fetch(request, limit: PlanURL.envelopeLimit, generation: generation) { [weak self] result in
            guard let self else { return }
            guard case let .success((data, response)) = result, response.statusCode == 200 else {
                // Immutable snapshots are bounded on the server. Discovery
                // supplies the latest snapshot if this URL has expired.
                self.failed("cannot fetch the envelope at \(url)", generation: generation)
                return
            }
            self.takeEnvelope(data, base: response.url ?? url, subscribe: subscribe, generation: generation, expected: expected)
        }
    }

    private func fetch(_ request: URLRequest, limit: Int, generation: UInt64,
                       completion: @escaping (Result<(Data, HTTPURLResponse), BoundedFetch.Failure>) -> Void) {
        let id = UUID()
        let fetch = BoundedFetch(origin: page, limit: limit) { [weak self] result in
            guard let self else { return }
            self.fetches.removeValue(forKey: id)
            guard !self.closed, self.terminal == nil, generation == self.resolution else { return }
            completion(result)
        }
        fetches[id] = fetch
        fetch.start(request)
    }

    static func envelopeLink(in html: Data) -> String? {
        let text = String(decoding: html.prefix(16384), as: UTF8.self)
        guard let type = text.range(of: envelopeType) else { return nil }
        guard let open = text.range(of: "<link", options: .backwards, range: text.startIndex..<type.lowerBound),
              let close = text.range(of: ">", range: type.upperBound..<text.endIndex) else { return nil }
        let tag = text[open.lowerBound..<close.upperBound]
        for quote in ["\"", "'"] {
            if let start = tag.range(of: "href=" + quote), let end = tag.range(of: quote, range: start.upperBound..<tag.endIndex) {
                return String(tag[start.upperBound..<end.lowerBound]).replacingOccurrences(of: "&amp;", with: "&")
            }
        }
        return nil
    }

    private static func count(_ value: Any?) -> Int? {
        guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID() else { return nil }
        let n = number.doubleValue
        guard n >= 0, n <= 9_007_199_254_740_991, n.rounded(.down) == n else { return nil }
        return Int(n)
    }
    private static func digest(_ value: Any?) -> String? {
        guard let text = value as? String, text.utf8.count == 64,
              text.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else { return nil }
        return text
    }
    private static func safeName(_ name: String) -> Bool {
        !name.isEmpty && !name.contains(":") && !name.contains("\\") && !name.contains("\0") &&
            name.split(separator: "/", omittingEmptySubsequences: false).allSatisfy { !$0.isEmpty && $0 != "." && $0 != ".." }
    }
    private func card(_ row: [String: Any], name: String, base: URL) -> FileCard? {
        guard let path = row["url"] as? String, let url = URL(string: path, relativeTo: base)?.absoluteURL,
              PlanURL.sameOrigin(url, page), let sha = PlanURL.digest(row["sha256"]),
              let count = PlanURL.count(row["bytes"]), count <= (name.isEmpty ? PlanURL.planLimit : PlanURL.assetLimit) else { return nil }
        return FileCard(name: name, url: url, sha: sha, count: count)
    }
    private static func revision(_ row: [String: Any]) -> Revision? {
        guard let epoch = row["epoch"] as? String, !epoch.isEmpty, epoch.utf8.count <= 128,
              let seq = count(row["seq"]), let identity = digest(row["generation"]) else { return nil }
        return Revision(epoch: epoch, seq: seq, identity: identity, program: digest(row["program"]))
    }

    /// A new epoch resets ordering; an already retired epoch cannot return.
    /// All fetch completions also carry a local resolution, so a response
    /// started before a newer announcement cannot commit afterward.
    private func observe(_ revision: Revision) -> Bool {
        guard acceptProgram(revision.program) else {
            terminal = "the dev server rebuilt the app's native code — rebuild and relaunch this host (the last good plan is still running)"
            status(terminal!)
            beginResolution()
            stream?.cancel()
            return false
        }
        if let old = observed {
            if old.epoch == revision.epoch {
                guard revision.seq >= old.seq, revision.seq != old.seq || revision.identity == old.identity else { return false }
            } else {
                guard !retiredEpochs.contains(revision.epoch) else { return false }
                retiredEpochs.insert(old.epoch)
            }
        }
        observed = revision
        return true
    }

    private func takeEnvelope(_ data: Data, base: URL, subscribe shouldSubscribe: Bool, generation: UInt64, expected: Revision?) {
        guard data.count <= PlanURL.envelopeLimit, PlanURL.sameOrigin(base, page),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any], PlanURL.count(json["exact"]) == 1,
              let planRow = json["plan"] as? [String: Any], let plan = card(planRow, name: "", base: base),
              let assetRows = json["assets"] as? [[String: Any]] else {
            failed("invalid or unsupported envelope at \(base)", generation: generation)
            return
        }
        var assets: [FileCard] = []
        var names = Set<String>()
        var total = plan.count
        for row in assetRows {
            guard let name = row["name"] as? String, PlanURL.safeName(name), names.insert(name).inserted,
                  let asset = card(row, name: name, base: base), asset.count <= PlanURL.generationLimit - total else {
                failed("invalid, duplicate, or oversized asset manifest at \(base)", generation: generation)
                return
            }
            total += asset.count
            assets.append(asset)
        }
        assets.sort { $0.name.utf8.lexicographicallyPrecedes($1.name.utf8) }
        var module: [String: FileCard] = [:]
        if let row = json["module"] {
            guard let rows = row as? [String: Any], Set(rows.keys) == Set(["native", "receipt", "web"]) else {
                failed("invalid module manifest", generation: generation); return
            }
            for key in ["native", "receipt", "web"] {
                guard let row = rows[key] as? [String: Any], let file = card(row, name: "", base: base),
                      file.count <= (key == "receipt" ? 1024 * 1024 : 32 * 1024 * 1024),
                      file.count <= PlanURL.generationLimit - total else {
                    failed("invalid or oversized module card", generation: generation); return
                }
                module[key] = file; total += file.count
            }
        }
        let moduleCanonical = module.isEmpty ? "" : ",\"module\":{" + ["native", "receipt", "web"].map { PlanURL.quote($0) + ":" + module[$0]!.canonical }.joined(separator: ",") + "}"
        var rustCanonical = ""
        var rustModule: [String: FileCard] = [:]
        if let raw = json["rust"] {
            guard let variants = raw as? [String: Any], !variants.isEmpty,
                  Set(variants.keys).isSubset(of: Set(["native", "tiered", "wasm"])) else {
                failed("invalid Rust module manifest", generation: generation); return
            }
            let policy = ExactApp.shared.rustPolicy
            var canonicalVariants: [String] = []
            for key in variants.keys.sorted() {
                guard let variant = variants[key] as? [String: Any],
                      Set(variant.keys) == Set(key != "wasm" ? ["module", "receipt", "target"] : ["module", "receipt"]) else {
                    failed("invalid Rust executor manifest", generation: generation); return
                }
                var cards: [String: FileCard] = [:]
                for name in ["module", "receipt"] {
                    guard let row = variant[name] as? [String: Any], let file = card(row, name: "", base: base),
                          file.count <= (name == "receipt" ? 1 << 20 : 32 << 20), file.count <= PlanURL.generationLimit - total else {
                        failed("invalid or oversized Rust module card", generation: generation); return
                    }
                    cards[name] = file; total += file.count
                }
                let target = variant["target"] as? String
                if key != "wasm" && (target == nil || target!.isEmpty) {
                    failed("missing Rust native target", generation: generation); return
                }
                let targetCanonical = target.map { ",\"target\":" + PlanURL.quote($0) } ?? ""
                canonicalVariants.append(PlanURL.quote(key) + ":{\"module\":" + cards["module"]!.canonical + ",\"receipt\":" + cards["receipt"]!.canonical + targetCanonical + "}")
                if key == policy.mode && (key == "wasm" || target == policy.target) {
                    rustModule = ["receipt": cards["receipt"]!, "native": cards["module"]!]
                }
            }
            guard !rustModule.isEmpty || (policy.mode == "off" && !module.isEmpty) else {
                failed("no Rust replacement matching this binary's executor and target", generation: generation); return
            }
            rustCanonical = ",\"rust\":{" + canonicalVariants.joined(separator: ",") + "}"
        }
        let canonical = "{\"assets\":[" + assets.map(\.canonical).joined(separator: ",") + "]" + moduleCanonical + ",\"plan\":" + plan.canonical + rustCanonical + "}"
        let identity = PlanURL.hash(Data(canonical.utf8))
        if let dev = json["dev"] as? [String: Any] {
            guard let revision = PlanURL.revision(dev), revision.identity == identity,
                  expected == nil || revision == expected,
                  let path = dev["events"] as? String, let events = URL(string: path, relativeTo: base)?.absoluteURL,
                  PlanURL.sameOrigin(events, page), observe(revision) else {
                failed("invalid or obsolete dev generation at \(base)", generation: generation)
                return
            }
            pending = revision
            if shouldSubscribe { subscribe(events) }
        } else if json["dev"] != nil || expected != nil {
            failed("missing dev generation identity at \(base)", generation: generation)
            return
        }
        // Subscribe before payloads finish: hello repairs an edit between
        // discovery and the stream opening, including an asset-only edit.
        guard currentGeneration() != identity else { pending = nil; return }
        fetchGeneration(plan: plan, assets: assets, module: module, rust: rustModule, identity: identity, generation: generation)
    }

    /// At most four payloads are in flight. Every card still has its own byte
    /// bound, and all plan, assets and module bytes verify before one apply.
    private func fetchGeneration(plan: FileCard, assets: [FileCard], module: [String: FileCard], rust: [String: FileCard], identity: String, generation: UInt64) {
        let payloadStarted = ProcessInfo.processInfo.systemUptime
        let envelopeMs = (payloadStarted - resolutionStarted) * 1000
        let cards = [plan] + assets + [module, rust].flatMap { files in ["receipt", "native"].compactMap { files[$0] } }
        var payloads = [Data](repeating: Data(), count: cards.count)
        var nextIndex = 0, active = 0, completed = 0
        var stopped = false
        func refuse(_ message: String) {
            guard !stopped else { return }
            stopped = true
            Array(fetches.values).forEach { $0.cancel() }
            failed(message, generation: generation)
        }
        func pump() {
            guard !stopped, !closed, terminal == nil, generation == resolution else { return }
            if completed == cards.count {
                stopped = true
                pending = nil
                if currentGeneration() != identity {
                    let javascriptIndex = 1 + assets.count
                    let rustIndex = javascriptIndex + (module.isEmpty ? 0 : 2)
                    let javascript = module.isEmpty ? nil : ExactModule(receipt: payloads[javascriptIndex], bytecode: payloads[javascriptIndex + 1])
                    let rustModule = rust.isEmpty ? nil : ExactModule(receipt: payloads[rustIndex], bytecode: payloads[rustIndex + 1])
                    do {
                        let logic = try DevelopmentModules.combine(javascript: javascript, rust: rustModule)
                        let assetBytes = Dictionary(uniqueKeysWithValues: assets.enumerated().map { ($0.element.name, payloads[$0.offset + 1]) })
                        let candidate = Generation(identity: identity, plan: payloads[0], assets: assetBytes, module: logic)
                        let fetched = ProcessInfo.processInfo.systemUptime
                        func commitCandidate() {
                            guard !self.closed, self.terminal == nil, generation == self.resolution else { return }
                            if applyGeneration(candidate, "generation ← " + (page.host ?? "")) {
                                status(String(format: "generation %@: envelope %.1f ms, payloads %.1f ms, preparation %.1f ms", String(identity.prefix(12)), envelopeMs, (fetched - payloadStarted) * 1000, (ProcessInfo.processInfo.systemUptime - fetched) * 1000))
                            } else if preparationPending() {
                                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: commitCandidate)
                            } else {
                                status("the host refused the generation; keeping the running app")
                            }
                        }
                        commitCandidate()
                    } catch {
                        failed("cannot combine JavaScript and Rust modules: \(error.localizedDescription); keeping the running app", generation: generation)
                    }
                }
                return
            }
            while !stopped && active < 4 && nextIndex < cards.count {
                let index = nextIndex, card = cards[index]
                nextIndex += 1
                active += 1
                var request = URLRequest(url: card.url)
                request.setValue("no-cache", forHTTPHeaderField: "Cache-Control")
                fetch(request, limit: card.count, generation: generation) { result in
                    guard !stopped else { return }
                    active -= 1
                    guard case let .success((data, response)) = result, response.statusCode == 200,
                          data.count == card.count, PlanURL.hash(data) == card.sha else {
                        refuse("cannot verify \(card.url); keeping the running app")
                        return
                    }
                    payloads[index] = data
                    completed += 1
                    pump()
                }
            }
        }
        pump()
    }

    private static func hash(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
    /// JSON.stringify escaping, without Foundation's optional slash escaping.
    /// Names sort by UTF-8 bytes, matching the server's canonical manifest.
    private static func quote(_ value: String) -> String {
        var result = "\""
        for scalar in value.unicodeScalars {
            switch scalar.value {
            case 34: result += "\\\""
            case 92: result += "\\\\"
            case 8: result += "\\b"
            case 9: result += "\\t"
            case 10: result += "\\n"
            case 12: result += "\\f"
            case 13: result += "\\r"
            case 0..<32: result += String(format: "\\u%04x", scalar.value)
            default: result.unicodeScalars.append(scalar)
            }
        }
        return result + "\""
    }

    // MARK: reconnect and ordering (LLP 1023 D3)

    private func subscribe(_ url: URL) {
        stream?.cancel()
        buffer.removeAll()
        var request = URLRequest(url: url)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        let task = session.dataTask(with: request)
        stream = task
        task.resume()
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard dataTask === stream, let http = response as? HTTPURLResponse, http.statusCode == 200,
              let url = http.url, PlanURL.sameOrigin(url, page),
              (http.value(forHTTPHeaderField: "Content-Type") ?? "").lowercased().hasPrefix("text/event-stream") else {
            completionHandler(.cancel)
            failed("invalid events response", generation: resolution)
            return
        }
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard dataTask === stream else { return }
        guard data.count <= PlanURL.eventLimit - buffer.count else {
            stream?.cancel()
            failed("events exceeded the \(PlanURL.eventLimit)-byte frame limit", generation: resolution)
            return
        }
        buffer.append(data)
        while let frame = nextFrame() { handle(frame) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        guard task === stream, let next = request.url, PlanURL.sameOrigin(next, page) else {
            completionHandler(nil)
            return
        }
        completionHandler(request)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard task === stream, !closed, terminal == nil else { return }
        if (error as? URLError)?.code == .cancelled { return }
        failed("events disconnected; resolving the current generation", generation: resolution)
    }

    private func failed(_ message: String, generation: UInt64) {
        status(message)
        pending = nil
        let delay = retrySeconds
        retrySeconds = min(retrySeconds * 2, 5)
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            guard let self, !self.closed, self.terminal == nil, self.resolution == generation else { return }
            self.resolveAndBoot()
        }
    }

    private func nextFrame() -> [String: Any]? {
        guard let range = buffer.range(of: Data("\n\n".utf8)) else { return nil }
        let chunk = buffer.subdata(in: buffer.startIndex..<range.lowerBound)
        buffer.removeSubrange(buffer.startIndex..<range.upperBound)
        guard let text = String(data: chunk, encoding: .utf8) else { return [:] }
        for line in text.split(separator: "\n") where line.hasPrefix("data: ") {
            if let json = try? JSONSerialization.jsonObject(with: Data(line.dropFirst(6).utf8)) as? [String: Any] { return json }
        }
        return [:]
    }

    private func handle(_ frame: [String: Any]) {
        retrySeconds = 1
        if frame["rebuilt"] != nil {
            terminal = "the dev server rebuilt the app's native code — rebuild and relaunch this host (the last good plan is still running)"
            status(terminal!)
            beginResolution()
            stream?.cancel()
            return
        }
        if let error = frame["error"] as? String { status("dev: \(error)"); return }
        if frame["ready"] as? Bool == false { return }
        guard let revision = PlanURL.revision(frame), terminal == nil, observe(revision) else { return }
        if currentGeneration() == revision.identity { beginResolution(); return }
        guard pending != revision,
              let path = frame["envelope"] as? String, let url = URL(string: path, relativeTo: page)?.absoluteURL,
              PlanURL.sameOrigin(url, page) else { return }
        let generation = beginResolution()
        pending = revision
        fetchEnvelope(url, subscribe: false, generation: generation, expected: revision)
    }

    private func status(_ message: String) { fputs("exact url: \(message)\n", stderr) }
}

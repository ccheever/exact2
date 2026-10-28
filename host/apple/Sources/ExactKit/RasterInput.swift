import CommonCrypto
import CoreGraphics
import Foundation

/// A source-interest lifetime, not a Runtime callback. Cancelling detaches the
/// task handler under lock and invokes it outside every backend/core lock.
final class RasterCancellation: @unchecked Sendable {
    private let lock = NSLock()
    private var stopped = false
    private var handler: (() -> Void)?
    var isCancelled: Bool { lock.lock(); defer { lock.unlock() }; return stopped }
    func install(_ handler: @escaping () -> Void) {
        lock.lock(); let cancel = stopped
        if !cancel { self.handler = handler }; lock.unlock()
        if cancel { handler() }
    }
    func clear() { lock.lock(); handler = nil; lock.unlock() }
    func cancel() {
        lock.lock(); stopped = true; let callback = handler; handler = nil; lock.unlock()
        callback?()
    }
}

/// Workers keep a descriptor, never a queued encoded Data closure. Complete
/// resolvers still verify through their existing bytes/url path. Remote inputs
/// spool to a bounded temporary file and are reused for metadata and decode.
final class RasterInput: @unchecked Sendable {
    static var httpCacheUsage: [String: Int] { RasterDownload.cacheUsage }
    /// A `data:` source's bound, in bytes of URL text, on every host (LLP 1011
    /// §2; `exact_raster::MAX_DATA_URL_BYTES`, the web hosts' `DATA_LIMIT`).
    static let dataLimit = 1024 * 1024
    let url: URL
    let encodedBytes: Int
    private let temporary: Bool
    /// The asset catalog's name for this file, when it is the launch image
    /// the catalog was built from (`ExactLaunchImageSource` and its bytes'
    /// `ExactLaunchImageSHA256`, host/apple/assets.mjs) and a plain PNG:
    /// decoded from the catalog at the size the launch screen shows it. An
    /// update's new bytes under the name are decoded from the file.
    private(set) var catalog: String?
    /// A package's picture, already decoded (PackagePictures.swift): no
    /// bytes, its size the image's own, drawn into the plan's storage.
    private(set) var decoded: CGImage?
    private init(url: URL, encodedBytes: Int, temporary: Bool, catalog: String? = nil) {
        self.url = url; self.encodedBytes = encodedBytes; self.temporary = temporary; self.catalog = catalog
    }
    static func picture(_ image: CGImage) -> RasterInput {
        let bytes = min(RasterMetadata.encodedLimit, max(1, image.bytesPerRow * image.height))
        let input = RasterInput(url: URL(fileURLWithPath: "/dev/null"), encodedBytes: bytes, temporary: false)
        input.decoded = image
        return input
    }
    private static let launchSource = Bundle.main.object(forInfoDictionaryKey: "ExactLaunchImageSource") as? String
    private static let launchDigest = Bundle.main.object(forInfoDictionaryKey: "ExactLaunchImageSHA256") as? String
    private static let launchImage = (Bundle.main.object(forInfoDictionaryKey: "UILaunchScreen") as? [String: Any])?["UIImageName"] as? String
    deinit { if temporary { try? FileManager.default.removeItem(at: url) } }
    /// CommonCrypto's, not CryptoKit's: launching an app that touches no
    /// other CryptoKit then dirties none of its pages (40 KB in euxy on iOS).
    private static func sha256(_ data: Data) -> String {
        var hash = [UInt8](repeating: 0, count: Int(CC_SHA256_DIGEST_LENGTH))
        _ = data.withUnsafeBytes { CC_SHA256($0.baseAddress, CC_LONG(data.count), &hash) }
        return hash.map { String(format: "%02x", $0) }.joined()
    }
    static func open(_ name: String, resolver: AssetResolver, cancellation: RasterCancellation = RasterCancellation()) throws -> RasterInput {
        guard !cancellation.isCancelled else { throw URLError(.cancelled) }
        // The app's own file (LLP 1069.002 D7): a picked photo's preview.
        if name.hasPrefix("app:/") {
            guard let url = AppFiles.url(name) else { throw RasterFailure.unresolved }
            let values = try url.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey])
            guard values.isRegularFile == true, let count = values.fileSize,
                  count > 0, count <= RasterMetadata.encodedLimit else { throw RasterFailure.encodedLimit }
            return RasterInput(url: url, encodedBytes: count, temporary: false)
        }
        // A `data:` URL (RFC 2397), as a page's `<img>` takes one: its bytes,
        // spooled like a download's, so metadata and decode read a file.
        if name.hasPrefix("data:") {
            guard name.utf8.count <= dataLimit, let bytes = dataURL(name), !bytes.isEmpty else { throw RasterFailure.decode }
            let file = try RasterSpool.file()
            try bytes.write(to: file, options: .atomic)
            return RasterInput(url: file, encodedBytes: bytes.count, temporary: true)
        }
        if let url = URL(string: name), let scheme = url.scheme {
            guard scheme == "https" || scheme == "http" else { throw RasterFailure.unresolved }
            let download = try RasterDownload(url: url)
            let (file, count) = try download.run(cancellation)
            return RasterInput(url: file, encodedBytes: count, temporary: true)
        }
        guard let url = resolver.url(name, maximumBytes: RasterMetadata.encodedLimit) else { throw RasterFailure.unresolved }
        let values = try url.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey])
        guard values.isRegularFile == true, let count = values.fileSize,
              count > 0, count <= RasterMetadata.encodedLimit else { throw RasterFailure.encodedLimit }
        return RasterInput(url: url, encodedBytes: count, temporary: false,
                           catalog: name == launchSource && launchDigest != nil ? launchImage : nil)
    }
    /// The URL of an `http:` or `https:` source, which `fetch` downloads.
    static func remote(_ name: String) -> URL? {
        guard let url = URL(string: name), url.scheme == "https" || url.scheme == "http" else { return nil }
        return url
    }
    /// Downloads a remote source without waiting for it: `done` gets its
    /// spooled input, or why not, on the download's queue (or at once, for
    /// a response already stored). Many download at a time over the
    /// process's shared connections, as a browser's images do.
    /// An `urgent` one (a picture that fills the screen, asked as it starts)
    /// goes first, with the longer whole-load bound (`RasterRetry`).
    static func fetch(_ url: URL, urgent: (() -> Bool)? = nil, cancellation: RasterCancellation, done: @escaping (Result<RasterInput, Error>) -> Void) {
        do {
            try RasterDownload(url: url, urgency: urgent).start(cancellation) { result in
                done(result.map { RasterInput(url: $0.0, encodedBytes: $0.1, temporary: true) })
            }
        } catch { done(.failure(error)) }
    }
    /// Every download of `url` under way now fills the screen: it goes first,
    /// with the longer whole-load bound from its ask (one about to start asks
    /// its `urgent` as it does).
    static func promote(_ url: URL) { RasterDownloads.shared.promote(url) }
    /// A `data:` URL's bytes: base64 after `;base64`, else percent-decoded;
    /// whitespace and percent escapes in base64 forgiven, as browsers do.
    static func dataURL(_ name: String) -> Data? {
        let utf8 = Array(name.utf8.dropFirst(5))
        guard let comma = utf8.firstIndex(of: UInt8(ascii: ",")) else { return nil }
        let meta = String(decoding: utf8[..<comma], as: UTF8.self).lowercased()
        var body: [UInt8] = []
        var i = comma + 1
        while i < utf8.count {
            if utf8[i] == UInt8(ascii: "%"), i + 2 < utf8.count, let byte = UInt8(String(decoding: utf8[i + 1...i + 2], as: UTF8.self), radix: 16) {
                body.append(byte); i += 3
            } else { body.append(utf8[i]); i += 1 }
        }
        guard meta.hasSuffix(";base64") else { return Data(body) }
        var text = String(decoding: body.filter { ![9, 10, 12, 13, 32].contains($0) }, as: UTF8.self)
        while text.count % 4 != 0 { text += "=" }
        return Data(base64Encoded: text)
    }
    func metadata() throws -> RasterMetadata {
        if let decoded {
            return try RasterMetadata.validated(width: decoded.width, height: decoded.height, orientation: 1,
                                                encodedBytes: encodedBytes, headerBytes: 1, deep: decoded.bitsPerComponent > 8)
        }
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        // The launch image's name: the whole file is read once (only under
        // that name, on the worker inspecting it), and the catalog stands in
        // only for the bytes it was made from, whose header says its size.
        if catalog != nil {
            let whole = encodedBytes <= RasterMetadata.headerLimit ? try file.read(upToCount: encodedBytes + 1) ?? Data() : Data()
            if whole.count == encodedBytes,
               Self.sha256(whole) == Self.launchDigest,
               let plain = RasterMetadata.plainPNG(prefix: whole, encodedBytes: encodedBytes) { return plain }
            catalog = nil
            try file.seek(toOffset: 0)
        }
        let prefix = try file.read(upToCount: RasterMetadata.headerLimit) ?? Data()
        return try RasterMetadata.read(prefix: prefix, encodedBytes: encodedBytes)
    }
    func bytes() throws -> Data {
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        // A changed file cannot make readToEnd allocate an unbounded buffer.
        guard try file.seekToEnd() == encodedBytes else { throw RasterFailure.encodedLimit }
        try file.seek(toOffset: 0)
        let bytes = try file.read(upToCount: encodedBytes) ?? Data()
        guard bytes.count == encodedBytes else { throw RasterFailure.decode }
        return bytes
    }
}

/// A response that is not 2xx: its status is the image's `error` (LLP 1011 §4).
/// `retryAfter`: not asked again before this, a 429's or 503's `Retry-After`.
struct RasterHTTPStatus: Error, CustomStringConvertible {
    let code: Int
    var retryAfter: Date? = nil
    var description: String { "HTTP \(code)" }
}

private final class RasterDownload: NSObject, @unchecked Sendable {
    // Encoded HTTP responses are separate from the decoded-raster ledger.
    // Keep them across process restarts without another in-memory image cache.
    static let responseCache = URLCache(memoryCapacity: 0, diskCapacity: 64 * 1024 * 1024,
        directory: FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first?
            .appendingPathComponent("exact-raster-http", isDirectory: true))
    static var cacheUsage: [String: Int] {
        ["memoryBytes": responseCache.currentMemoryUsage, "memoryCapacity": responseCache.memoryCapacity,
         "diskBytes": responseCache.currentDiskUsage, "diskCapacity": responseCache.diskCapacity]
    }
    fileprivate let url: URL
    private let destination: URL
    private let file: FileHandle
    private var count = 0
    private var failure: Error?
    private var cancellation: RasterCancellation?
    private var done: ((Result<(URL, Int), Error>) -> Void)?
    private let policy: URLRequest.CachePolicy
    /// A picture that fills the screen: its task goes first and its whole
    /// load has the longer bound, from when it was asked for (`promote`).
    /// Set before the download is published, then only on the sessions' queue.
    fileprivate var urgent = false
    private let urgency: (() -> Bool)?
    /// When it was asked for, on the monotonic clock; the bound's timer and
    /// its generation, so a superseded timer's cancel does nothing.
    fileprivate var asked = DispatchTime.now()
    fileprivate var task: URLSessionDataTask?
    private var bound: DispatchSourceTimer?
    private var armed = 0
    init(url: URL, policy: URLRequest.CachePolicy = .useProtocolCachePolicy, urgency: (() -> Bool)? = nil) throws {
        self.url = url
        self.policy = policy
        self.urgency = urgency
        destination = try RasterSpool.file()
        // (The spool, which asking again may mend: not a raster refusal.)
        guard FileManager.default.createFile(atPath: destination.path, contents: nil, attributes: [.posixPermissions: 0o600]) else { throw CocoaError(.fileWriteUnknown) }
        file = try FileHandle(forWritingTo: destination)
        super.init()
    }
    /// Waits for `start`'s answer.
    func run(_ cancellation: RasterCancellation) throws -> (URL, Int) {
        let finished = DispatchSemaphore(value: 0)
        var outcome: Result<(URL, Int), Error> = .failure(RasterFailure.decode)
        start(cancellation) { outcome = $0; finished.signal() }
        finished.wait() // bounded by URLSession's resource timeout
        return try outcome.get()
    }
    /// Downloads into the spool file and answers once, on the sessions'
    /// queue, or at once for a stored response it may show.
    func start(_ cancellation: RasterCancellation, done: @escaping (Result<(URL, Int), Error>) -> Void) {
        // Show a stored response that is stale but within its
        // `stale-while-revalidate` window now and refresh it behind, as a
        // browser does (RFC 5861): URLCache knows only freshness, so it would
        // wait on the network for an image it already holds.
        if policy == .useProtocolCachePolicy, let data = Self.staleWithinWindow(url) {
            do { try file.write(contentsOf: data); try file.close() } catch {
                try? FileManager.default.removeItem(at: destination); return done(.failure(error))
            }
            count = data.count
            Self.revalidate(url)
            return done(.success((destination, count)))
        }
        // A server that said when to ask again is not asked sooner, by any
        // source of the URL (LLP 1011 §4).
        if let cooling = RasterDownloads.shared.cooling(url) {
            try? file.close(); try? FileManager.default.removeItem(at: destination)
            return done(.failure(cooling))
        }
        self.done = done; self.cancellation = cancellation
        requested = Date()
        // Published with its task, its ask and its urgency set (`RasterDownloads.task`).
        let task = RasterDownloads.shared.task(url, policy: policy, urgent: urgency?() == true, for: self)
        cancellation.install { task.cancel() }
        RasterDownloads.shared.queue.addOperation { [self] in
            // One whose picture came to fill the screen since it was asked.
            if !urgent, urgency?() == true { urgent = true; task.priority = URLSessionTask.highPriority }
            arm()
        }
        task.resume()
    }
    /// The whole load's bound, from when it was asked for: past it the task
    /// fails as timed out. On the sessions' queue.
    private func arm() {
        bound?.cancel(); bound = nil
        guard done != nil else { return }
        armed += 1
        let generation = armed
        let timer = DispatchSource.makeTimerSource(queue: .global(qos: .utility))
        timer.schedule(deadline: asked + (urgent ? RasterRetry.urgentWhole : RasterRetry.whole))
        timer.setEventHandler { [weak self] in
            RasterDownloads.shared.queue.addOperation {
                guard let self, self.done != nil, self.armed == generation else { return }
                self.failure = self.failure ?? URLError(.timedOut); self.task?.cancel()
            }
        }
        bound = timer; timer.resume()
    }
    /// Its picture now fills the screen: its task goes first, and its whole
    /// load has the longer bound from when it was asked for. On the sessions' queue.
    fileprivate func promote() {
        guard !urgent, done != nil else { return }
        urgent = true; task?.priority = URLSessionTask.highPriority; arm()
    }
    private func finish() {
        cancellation?.clear()
        bound?.cancel(); bound = nil; armed += 1
        try? file.close()
        let answer = done; done = nil
        if let failure {
            if let status = failure as? RasterHTTPStatus, let at = status.retryAfter { RasterDownloads.shared.cool(url, until: at, status) }
            try? FileManager.default.removeItem(at: destination); answer?(.failure(failure)); return
        }
        guard count > 0 else { try? FileManager.default.removeItem(at: destination); answer?(.failure(RasterEmptyResponse())); return }
        answer?(.success((destination, count)))
    }
    /// The stored body for `url` when it is stale but may still be shown:
    /// its age past `max-age` and within `stale-while-revalidate`, no
    /// directive that forbids serving it stale (RFC 9111 §4.2.4), and no
    /// `Vary` this lookup, which has no request headers, cannot match. Its
    /// age runs from when its origin generated it, which the download stored
    /// with it; one stored without that is not reused.
    private static func staleWithinWindow(_ url: URL) -> Data? {
        guard let stored = responseCache.cachedResponse(for: URLRequest(url: url)),
              let http = stored.response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
              !stored.data.isEmpty, stored.data.count <= RasterMetadata.encodedLimit,
              (http.value(forHTTPHeaderField: "Vary") ?? "").lowercased().split(separator: ",")
                .allSatisfy({ $0.trimmingCharacters(in: .whitespaces) == "accept-encoding" }) else { return nil }
        let directives = (http.value(forHTTPHeaderField: "Cache-Control") ?? "").lowercased()
            .split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
        func seconds(_ name: String) -> Double? {
            directives.first { $0.hasPrefix(name + "=") }.flatMap { Double($0.dropFirst(name.count + 1).trimmingCharacters(in: ["\""])) }
        }
        guard directives.allSatisfy({ !["no-store", "no-cache", "must-revalidate"].contains($0) }),
              let maxAge = seconds("max-age"), let window = seconds("stale-while-revalidate"),
              let generated = stored.userInfo?[generatedKey] as? Double else { return nil }
        let age = Date().timeIntervalSinceReferenceDate - generated
        return age > maxAge && age <= maxAge + window ? stored.data : nil
    }
    private static let httpDate = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(identifier: "GMT")
        formatter.dateFormat = "EEE, dd MMM yyyy HH:mm:ss zzz"
        return formatter
    }()

    /// Refreshes the stored response for `url` behind the image already
    /// shown, through the same bounded download, one at a time for each URL
    /// and two at once.
    private static func revalidate(_ url: URL) {
        guard revalidatingLock.withLock({ revalidating.insert(url).inserted }) else { return }
        refreshes.addOperation {
            defer { revalidatingLock.withLock { _ = revalidating.remove(url) } }
            guard let (file, _) = try? RasterDownload(url: url, policy: .reloadRevalidatingCacheData).run(RasterCancellation()) else { return }
            try? FileManager.default.removeItem(at: file)
        }
    }
    private static let revalidatingLock = NSLock()
    nonisolated(unsafe) private static var revalidating: Set<URL> = []
    private static let refreshes = {
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 2; queue.qualityOfService = .utility
        return queue
    }()

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            let code = (response as? HTTPURLResponse)?.statusCode ?? 0
            // Discord's CDN, and the person's own account: a 429's or 503's
            // wait is kept, an unreadable one an hour (`RasterRetry`).
            let after = [429, 503].contains(code) ? (response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Retry-After") : nil
            failure = RasterHTTPStatus(code: code, retryAfter: after.map {
                Date().addingTimeInterval(RasterRetry.retryAfter($0, now: Date()) ?? RasterRetry.unreadable)
            })
            completionHandler(.cancel); return
        }
        guard response.expectedContentLength <= RasterMetadata.encodedLimit else {
            failure = RasterFailure.encodedLimit; completionHandler(.cancel); return
        }
        // RFC 9111 §4.2.3's corrected initial age: by its `Date` (none is now),
        // or its `Age` plus the time the request took, whichever is larger. A
        // `Date` in a form this does not read (RFC 850, asctime) leaves it
        // unknown, so the response is never shown stale.
        let now = Date(), header = http.value(forHTTPHeaderField: "Date")
        if let date = header.map(Self.httpDate.date(from:)) ?? now {
            let delay = requested.map { now.timeIntervalSince($0) } ?? 0
            generated = now.addingTimeInterval(-max(now.timeIntervalSince(date), (http.value(forHTTPHeaderField: "Age").flatMap(Double.init) ?? 0) + delay, 0))
        }
        completionHandler(.allow)
    }
    /// Stores when the response was generated with it, which `URLCache` does
    /// not keep and the stale check needs
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, willCacheResponse proposed: CachedURLResponse,
                    completionHandler: @escaping (CachedURLResponse?) -> Void) {
        guard let generated else { return completionHandler(proposed) }
        completionHandler(CachedURLResponse(response: proposed.response, data: proposed.data,
            userInfo: [Self.generatedKey: generated.timeIntervalSinceReferenceDate], storagePolicy: proposed.storagePolicy))
    }
    private var requested: Date?
    /// When the origin generated the response, by this device's clock: its
    /// arrival less its corrected initial age
    private var generated: Date?
    private static let generatedKey = "exact.generated"
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        // (Bytes a cancel has not yet stopped keep the failure it had.)
        guard failure == nil else { return }
        guard data.count <= RasterMetadata.encodedLimit - count else {
            failure = RasterFailure.encodedLimit; dataTask.cancel(); return
        }
        do { try file.write(contentsOf: data); count += data.count }
        catch { failure = error; dataTask.cancel() }
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        failure = failure ?? error; finish()
    }
}

/// The process's image downloads: one session for each cache policy, made
/// on first use and kept, so images share connections (HTTP/2 streams on
/// one) and run at once. A session per download, waited out on a raster
/// worker, fetched two images at a time, each on its own new connection.
private final class RasterDownloads: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    static let shared = RasterDownloads()
    private let lock = NSLock()
    private var sessions: [UInt: URLSession] = [:]
    /// By task, not `taskIdentifier`, which is unique only within one session.
    private var downloads: [ObjectIdentifier: RasterDownload] = [:]
    /// Callbacks for every download run one at a time, so each download's
    /// state is touched by one thread.
    fileprivate let queue: OperationQueue = { let queue = OperationQueue(); queue.maxConcurrentOperationCount = 1; return queue }()
    /// URLs whose server said when to ask again (`Retry-After`), until then,
    /// with its answer: at most 256, the others ending soonest first to go.
    private var coolingURLs: [URL: (until: Date, status: RasterHTTPStatus)] = [:]
    func task(_ url: URL, policy: URLRequest.CachePolicy, urgent: Bool, for download: RasterDownload) -> URLSessionDataTask {
        lock.lock(); defer { lock.unlock() }
        let session = sessions[policy.rawValue] ?? URLSession(configuration: Self.configuration(policy), delegate: self, delegateQueue: queue)
        sessions[policy.rawValue] = session
        let task = session.dataTask(with: url)
        // A hint: URLSession serves the others as well.
        if urgent { task.priority = URLSessionTask.highPriority }
        download.urgent = urgent; download.task = task; download.asked = .now()
        downloads[ObjectIdentifier(task)] = download
        return task
    }
    /// Every download of `url` under way now fills the screen (`RasterDownload.promote`).
    func promote(_ url: URL) {
        lock.lock(); let matching = downloads.values.filter { $0.url == url }; lock.unlock()
        queue.addOperation { matching.forEach { $0.promote() } }
    }
    func cooling(_ url: URL) -> RasterHTTPStatus? {
        lock.lock(); defer { lock.unlock() }
        guard let entry = coolingURLs[url], entry.until > Date() else { return nil }
        return entry.status
    }
    func cool(_ url: URL, until: Date, _ status: RasterHTTPStatus) {
        lock.lock(); defer { lock.unlock() }
        let now = Date()
        coolingURLs = coolingURLs.filter { $0.value.until > now }
        coolingURLs[url] = (until, status)
        while coolingURLs.count > 256, let soonest = coolingURLs.filter({ $0.key != url || until <= now }).min(by: { $0.value.until < $1.value.until }) {
            coolingURLs.removeValue(forKey: soonest.key)
        }
    }
    private func download(_ task: URLSessionTask, finished: Bool = false) -> RasterDownload? {
        lock.lock(); defer { lock.unlock() }
        return finished ? downloads.removeValue(forKey: ObjectIdentifier(task)) : downloads[ObjectIdentifier(task)]
    }
    private static func configuration(_ policy: URLRequest.CachePolicy) -> URLSessionConfiguration {
        let configuration = URLSessionConfiguration.default
        configuration.urlCache = RasterDownload.responseCache; configuration.requestCachePolicy = policy
        // Switching cache policy must not add ambient cookies or credentials.
        configuration.httpCookieStorage = nil; configuration.httpShouldSetCookies = false
        configuration.urlCredentialStorage = nil
        // Each download's whole load has its own bound, from its ask
        // (`RasterRetry.whole`, longer for a picture that fills the screen).
        configuration.timeoutIntervalForRequest = 15; configuration.timeoutIntervalForResource = 130
        return configuration
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard let download = download(dataTask) else { return completionHandler(.cancel) }
        download.urlSession(session, dataTask: dataTask, didReceive: response, completionHandler: completionHandler)
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, willCacheResponse proposed: CachedURLResponse,
                    completionHandler: @escaping (CachedURLResponse?) -> Void) {
        guard let download = download(dataTask) else { return completionHandler(proposed) }
        download.urlSession(session, dataTask: dataTask, willCacheResponse: proposed, completionHandler: completionHandler)
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        download(dataTask)?.urlSession(session, dataTask: dataTask, didReceive: data)
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        download(task, finished: true)?.urlSession(session, task: task, didCompleteWithError: error)
    }
}

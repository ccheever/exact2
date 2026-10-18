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
    /// (`ExactLaunchImageSource`, host/apple/assets.mjs) and a plain PNG:
    /// decoded from the catalog at the size the launch screen shows it.
    private(set) var catalog: String?
    private init(url: URL, encodedBytes: Int, temporary: Bool, catalog: String? = nil) {
        self.url = url; self.encodedBytes = encodedBytes; self.temporary = temporary; self.catalog = catalog
    }
    private static let launchSource = Bundle.main.object(forInfoDictionaryKey: "ExactLaunchImageSource") as? String
    deinit { if temporary { try? FileManager.default.removeItem(at: url) } }
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
                           catalog: name == launchSource ? "ExactLaunchImage" : nil)
    }
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
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        let prefix = try file.read(upToCount: RasterMetadata.headerLimit) ?? Data()
        if catalog != nil {
            if let plain = RasterMetadata.plainPNG(prefix: prefix, encodedBytes: encodedBytes) { return plain }
            catalog = nil
        }
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
struct RasterHTTPStatus: Error, CustomStringConvertible {
    let code: Int
    var description: String { "HTTP \(code)" }
}

private final class RasterDownload: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    // Encoded HTTP responses are separate from the decoded-raster ledger.
    // Keep them across process restarts without another in-memory image cache.
    private static let responseCache = URLCache(memoryCapacity: 0, diskCapacity: 64 * 1024 * 1024,
        directory: FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first?
            .appendingPathComponent("exact-raster-http", isDirectory: true))
    static var cacheUsage: [String: Int] {
        ["memoryBytes": responseCache.currentMemoryUsage, "memoryCapacity": responseCache.memoryCapacity,
         "diskBytes": responseCache.currentDiskUsage, "diskCapacity": responseCache.diskCapacity]
    }
    private let url: URL
    private let destination: URL
    private let file: FileHandle
    private let done = DispatchSemaphore(value: 0)
    private var count = 0
    private var failure: Error?
    private let policy: URLRequest.CachePolicy
    init(url: URL, policy: URLRequest.CachePolicy = .useProtocolCachePolicy) throws {
        self.url = url
        self.policy = policy
        destination = try RasterSpool.file()
        guard FileManager.default.createFile(atPath: destination.path, contents: nil, attributes: [.posixPermissions: 0o600]) else { throw RasterFailure.decode }
        file = try FileHandle(forWritingTo: destination)
        super.init()
    }
    func run(_ cancellation: RasterCancellation) throws -> (URL, Int) {
        // Show a stored response that is stale but within its
        // `stale-while-revalidate` window now and refresh it behind, as a
        // browser does (RFC 5861): URLCache knows only freshness, so it would
        // wait on the network for an image it already holds.
        if policy == .useProtocolCachePolicy, let data = Self.staleWithinWindow(url) {
            do { try file.write(contentsOf: data); try file.close() } catch { try? FileManager.default.removeItem(at: destination); throw error }
            count = data.count
            Self.revalidate(url)
            return (destination, count)
        }
        let queue = OperationQueue(); queue.maxConcurrentOperationCount = 1
        let session = URLSession(configuration: Self.configuration(policy), delegate: self, delegateQueue: queue)
        let task = session.dataTask(with: url)
        cancellation.install { task.cancel() }
        requested = Date()
        task.resume()
        done.wait() // bounded URLSession resource timeout, on a process worker
        cancellation.clear()
        session.finishTasksAndInvalidate()
        try? file.close()
        if let failure { try? FileManager.default.removeItem(at: destination); throw failure }
        guard count > 0 else { try? FileManager.default.removeItem(at: destination); throw RasterFailure.decode }
        return (destination, count)
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

    private static func configuration(_ policy: URLRequest.CachePolicy) -> URLSessionConfiguration {
        let configuration = URLSessionConfiguration.default
        configuration.urlCache = responseCache; configuration.requestCachePolicy = policy
        // Switching cache policy must not add ambient cookies or credentials.
        configuration.httpCookieStorage = nil; configuration.httpShouldSetCookies = false
        configuration.urlCredentialStorage = nil
        configuration.timeoutIntervalForRequest = 15; configuration.timeoutIntervalForResource = 30
        return configuration
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            failure = RasterHTTPStatus(code: (response as? HTTPURLResponse)?.statusCode ?? 0); completionHandler(.cancel); return
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
        guard failure == nil, data.count <= RasterMetadata.encodedLimit - count else {
            failure = RasterFailure.encodedLimit; dataTask.cancel(); return
        }
        do { try file.write(contentsOf: data); count += data.count }
        catch { failure = error; dataTask.cancel() }
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        failure = failure ?? error; done.signal()
    }
}

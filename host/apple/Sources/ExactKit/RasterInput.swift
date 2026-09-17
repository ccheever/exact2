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
    let url: URL
    let encodedBytes: Int
    private let temporary: Bool
    private init(url: URL, encodedBytes: Int, temporary: Bool) {
        self.url = url; self.encodedBytes = encodedBytes; self.temporary = temporary
    }
    deinit { if temporary { try? FileManager.default.removeItem(at: url) } }
    static func open(_ name: String, resolver: AssetResolver, cancellation: RasterCancellation = RasterCancellation()) throws -> RasterInput {
        guard !cancellation.isCancelled else { throw URLError(.cancelled) }
        if let url = URL(string: name), let scheme = url.scheme {
            guard scheme == "https" || scheme == "http" else { throw RasterFailure.decode }
            let download = try RasterDownload(url: url)
            let (file, count) = try download.run(cancellation)
            return RasterInput(url: file, encodedBytes: count, temporary: true)
        }
        guard let url = resolver.url(name, maximumBytes: RasterMetadata.encodedLimit) else { throw RasterFailure.decode }
        let values = try url.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey])
        guard values.isRegularFile == true, let count = values.fileSize,
              count > 0, count <= RasterMetadata.encodedLimit else { throw RasterFailure.encodedLimit }
        return RasterInput(url: url, encodedBytes: count, temporary: false)
    }
    func metadata() throws -> RasterMetadata {
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
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

private final class RasterDownload: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    private let url: URL
    private let destination: URL
    private let file: FileHandle
    private let done = DispatchSemaphore(value: 0)
    private var count = 0
    private var failure: Error?
    init(url: URL) throws {
        self.url = url
        destination = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-raster-\(UUID().uuidString)")
        guard FileManager.default.createFile(atPath: destination.path, contents: nil, attributes: [.posixPermissions: 0o600]) else { throw RasterFailure.decode }
        file = try FileHandle(forWritingTo: destination)
        super.init()
    }
    func run(_ cancellation: RasterCancellation) throws -> (URL, Int) {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.urlCache = nil; configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.timeoutIntervalForRequest = 15; configuration.timeoutIntervalForResource = 30
        let queue = OperationQueue(); queue.maxConcurrentOperationCount = 1
        let session = URLSession(configuration: configuration, delegate: self, delegateQueue: queue)
        let task = session.dataTask(with: url)
        cancellation.install { task.cancel() }
        task.resume()
        done.wait() // bounded URLSession resource timeout, on a process worker
        cancellation.clear()
        session.finishTasksAndInvalidate()
        try? file.close()
        if let failure { try? FileManager.default.removeItem(at: destination); throw failure }
        guard count > 0 else { try? FileManager.default.removeItem(at: destination); throw RasterFailure.decode }
        return (destination, count)
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard response.expectedContentLength <= RasterMetadata.encodedLimit,
              (response as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) ?? false else {
            failure = RasterFailure.encodedLimit; completionHandler(.cancel); return
        }
        completionHandler(.allow)
    }
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

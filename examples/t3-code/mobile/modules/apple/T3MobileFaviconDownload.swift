import Foundation

// The image response carries its own signed URL. Never use the environment transport's headers.
// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
final class T3MobileFaviconDownload: @unchecked Sendable {
    enum Failure: Error, LocalizedError, Equatable {
        case invalidURL, response(Int?), tooLarge, network
        var errorDescription: String? {
            switch self {
            case .invalidURL: return "Project icon URL is invalid."
            case .response(let status): return status.map { "Project icon request failed with \($0)." } ?? "Project icon request failed."
            case .tooLarge: return "Project icon is too large to decode."
            case .network: return "Project icon could not be loaded."
            }
        }
    }
    private final class Read: @unchecked Sendable {
        let id: String, url: URL, completion: (Result<String, Error>) -> Void
        var task: URLSessionDataTask!
        var bytes = Data(), contentType: String?
        var result: Result<String, Error>?, delivered = false, decoding = false
        init(_ id: String, _ url: URL, _ completion: @escaping (Result<String, Error>) -> Void) {
            self.id = id; self.url = url; self.completion = completion
        }
    }
    private final class Delegate: NSObject, URLSessionDataDelegate, @unchecked Sendable {
        weak var owner: T3MobileFaviconDownload?
        func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                        completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
            completionHandler(owner?.receive(response, task: dataTask) == true ? .allow : .cancel)
        }
        func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
            owner?.receive(data, task: dataTask)
        }
        func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
            owner?.complete(task, error: error)
        }
        func urlSession(_ session: URLSession, task: URLSessionTask, didReceive challenge: URLAuthenticationChallenge,
                        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
            // Preserve normal TLS trust evaluation, but never satisfy HTTP authentication.
            completionHandler(challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust
                ? .performDefaultHandling : .cancelAuthenticationChallenge, nil)
        }
    }
    private let lock = NSLock(), callbackQueue: DispatchQueue
    private let decodeQueue: DispatchQueue
    private var reads = [String: Read](), tasks = [Int: Read](), alive = true
    private var session: URLSession!

    // Protocol classes are injectable for executable URLSession tests; production uses platform networking.
    init(callbackQueue: DispatchQueue = .main, protocolClasses: [AnyClass]? = nil,
         decodeQueue: DispatchQueue = DispatchQueue(label: "t3.favicon.decode", qos: .utility)) {
        self.callbackQueue = callbackQueue; self.decodeQueue = decodeQueue
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpCookieStorage = nil; configuration.httpShouldSetCookies = false
        configuration.urlCredentialStorage = nil; configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.timeoutIntervalForRequest = 30; configuration.timeoutIntervalForResource = 60
        if let protocolClasses { configuration.protocolClasses = protocolClasses }
        let delegate = Delegate(); delegate.owner = self
        let queue = OperationQueue(); queue.maxConcurrentOperationCount = 1
        session = URLSession(configuration: configuration, delegate: delegate, delegateQueue: queue)
    }
    deinit { shutdown() }

    var activeRequestCount: Int { lock.lock(); defer { lock.unlock() }; return reads.count }

    var activeDecodeCount: Int {
        lock.lock(); defer { lock.unlock() }
        return reads.values.filter { $0.decoding && $0.result == nil }.count
    }

    func load(requestId: String, url: URL, completion: @escaping (Result<String, Error>) -> Void) {
        lock.lock(); defer { lock.unlock() }
        if let previous = reads[requestId] { cancelLocked(previous) }
        let read = Read(requestId, url, completion)
        reads[requestId] = read
        guard alive else { finishLocked(read, .failure(CancellationError())); return }
        guard ["http", "https"].contains(url.scheme?.lowercased() ?? ""), url.host != nil,
              url.user?.isEmpty != false, url.password?.isEmpty != false else {
            finishLocked(read, .failure(Failure.invalidURL)); return
        }
        var request = URLRequest(url: url); request.httpShouldHandleCookies = false
        read.task = session.dataTask(with: request); tasks[read.task.taskIdentifier] = read
        read.task.resume()
    }
    func cancel(requestId: String) {
        lock.lock(); defer { lock.unlock() }
        if let read = reads[requestId] { cancelLocked(read) }
    }
    func shutdown() {
        lock.lock()
        guard alive else { lock.unlock(); return }
        alive = false
        for read in reads.values { cancelLocked(read) }
        lock.unlock()
        session?.invalidateAndCancel()
    }
    private func cancelLocked(_ read: Read) {
        guard !read.delivered else { return }
        if read.result == nil { finishLocked(read, .failure(CancellationError())) }
        else { read.result = .failure(CancellationError()) }
        read.task?.cancel(); read.bytes.removeAll(keepingCapacity: false)
    }
    private func finishLocked(_ read: Read, _ result: Result<String, Error>) {
        guard read.result == nil, !read.delivered else { return }
        read.result = result
        // Keep the terminal result cancellable until delivery. An old result cannot win
        // merely because it was queued before a clear, replacement or teardown.
        callbackQueue.async { [weak self] in
            guard let self else { read.completion(.failure(CancellationError())); return }
            self.lock.lock()
            guard !read.delivered, let result = read.result else { self.lock.unlock(); return }
            read.delivered = true
            if self.reads[read.id] === read { self.reads.removeValue(forKey: read.id) }
            if let task = read.task { self.tasks.removeValue(forKey: task.taskIdentifier) }
            read.bytes.removeAll(keepingCapacity: false)
            self.lock.unlock()
            read.completion(result)
        }
    }
    private func receive(_ response: URLResponse, task: URLSessionDataTask) -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard let read = tasks[task.taskIdentifier], read.result == nil, alive else { return false }
        guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode) else {
            finishLocked(read, .failure(Failure.response((response as? HTTPURLResponse)?.statusCode))); return false
        }
        let declared = response.value(forHTTPHeaderField: "Content-Length").flatMap(Double.init)
        guard response.expectedContentLength <= T3MobileFaviconImage.maxSourceBytes,
              !(declared.map { $0 > Double(T3MobileFaviconImage.maxSourceBytes) } ?? false) else {
            finishLocked(read, .failure(Failure.tooLarge)); return false
        }
        read.contentType = response.value(forHTTPHeaderField: "Content-Type")
        return true
    }
    private func receive(_ data: Data, task: URLSessionDataTask) {
        lock.lock(); defer { lock.unlock() }
        guard let read = tasks[task.taskIdentifier], read.result == nil, !read.decoding, alive else { return }
        guard data.count <= T3MobileFaviconImage.maxSourceBytes - read.bytes.count else {
            read.bytes.removeAll(keepingCapacity: false)
            finishLocked(read, .failure(Failure.tooLarge)); task.cancel(); return
        }
        read.bytes.append(data)
    }
    private func complete(_ task: URLSessionTask, error: Error?) {
        lock.lock(); defer { lock.unlock() }
        guard let read = tasks[task.taskIdentifier], read.result == nil, !read.decoding, alive else { return }
        if let error {
            let cancelled = (error as NSError).code == NSURLErrorCancelled
            finishLocked(read, .failure(cancelled ? CancellationError() : Failure.network)); return
        }
        read.decoding = true
        let bytes = read.bytes; read.bytes = Data()
        decodeQueue.async { [weak self] in
            let result = Result {
                try T3MobileFaviconImage.dataURL(bytes: bytes, contentType: read.contentType,
                    url: read.url.absoluteString, cancelled: { [weak self] in
                        guard let self else { return true }
                        self.lock.lock(); defer { self.lock.unlock() }
                        return !self.alive || read.result != nil || self.reads[read.id] !== read
                    })
            }
            guard let self else { return }
            self.lock.lock(); defer { self.lock.unlock() }
            self.finishLocked(read, result)
        }
    }
}

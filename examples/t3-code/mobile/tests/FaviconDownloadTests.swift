import Foundation
import Network

private final class FaviconProtocol: URLProtocol, @unchecked Sendable {
    struct Fixture {
        var status = 200, headers = [String: String](), chunks = [Data](), delay: Double = 0
        var fail = false, chunkDelay: Double = 0
        var firstChunk: DispatchSemaphore?
    }
    static let lock = NSLock()
    static var fixtures = [String: Fixture](), requests = [URLRequest](), stopped = Set<String>()
    private let stateLock = NSLock()
    private var cancelled = false
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "favicon.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        Self.lock.lock()
        Self.requests.append(request)
        let fixture = Self.fixtures[request.url!.path] ?? Fixture(status: 404)
        Self.lock.unlock()
        DispatchQueue.global().asyncAfter(deadline: .now() + fixture.delay) { [self] in
            stateLock.lock(); defer { stateLock.unlock() }
            guard !cancelled else { return }
            if fixture.fail { client?.urlProtocol(self, didFailWithError: URLError(.cannotConnectToHost)); return }
            let response = HTTPURLResponse(url: request.url!, statusCode: fixture.status, httpVersion: "HTTP/1.1", headerFields: fixture.headers)!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            if fixture.chunkDelay > 0 { emit(fixture, index: 0) }
            else {
                for chunk in fixture.chunks { client?.urlProtocol(self, didLoad: chunk) }
                client?.urlProtocolDidFinishLoading(self)
            }
        }
    }
    private func emit(_ fixture: Fixture, index: Int) {
        DispatchQueue.global().asyncAfter(deadline: .now() + (index == 0 ? 0 : fixture.chunkDelay)) { [self] in
            stateLock.lock(); defer { stateLock.unlock() }
            guard !cancelled else { return }
            if index == fixture.chunks.count { client?.urlProtocolDidFinishLoading(self); return }
            client?.urlProtocol(self, didLoad: fixture.chunks[index])
            if index == 0 { fixture.firstChunk?.signal() }
            emit(fixture, index: index + 1)
        }
    }
    override func stopLoading() {
        stateLock.lock(); cancelled = true; stateLock.unlock()
        Self.lock.lock(); Self.stopped.insert(request.url!.path); Self.lock.unlock()
    }
}

// Real sockets exercise URLSession's redirect and header behavior as well as URLProtocol streams.
private final class FaviconServer {
    let listener: NWListener
    let ready = DispatchSemaphore(value: 0), lock = NSLock()
    var requests = [String](), connections = [NWConnection]()
    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        listener.stateUpdateHandler = { [weak self] state in if case .ready = state { self?.ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in
            guard let self else { connection.cancel(); return }
            self.lock.lock(); self.connections.append(connection); self.lock.unlock()
            connection.start(queue: .global())
            connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [weak self] data, _, _, _ in
                guard let self, let data else { connection.cancel(); return }
                let request = String(decoding: data, as: UTF8.self)
                self.lock.lock(); self.requests.append(request); self.lock.unlock()
                let redirect = request.hasPrefix("GET /redirect.png ")
                let body = redirect ? Data() : FaviconDownloadTests.png
                let header = redirect
                    ? "HTTP/1.1 302 Found\r\nLocation: /no-extension\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    : "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: \(body.count)\r\nConnection: close\r\n\r\n"
                connection.send(content: Data(header.utf8) + body, completion: .contentProcessed { _ in connection.cancel() })
            }
        }
        listener.start(queue: .global())
        guard ready.wait(timeout: .now() + 5) == .success else { throw URLError(.timedOut) }
    }
    var url: URL { URL(string: "http://127.0.0.1:\(listener.port!.rawValue)/redirect.png")! }
    func stop() {
        listener.cancel(); lock.lock(); let all = connections; lock.unlock()
        for connection in all { connection.cancel() }
    }
    deinit { stop() }
}

@main struct FaviconDownloadTests {
    static let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a0ZkAAAAASUVORK5CYII=")!
    static var checks = 0, failures = [String]()
    static func check(_ condition: Bool, _ message: String) { checks += 1; if !condition { failures.append(message) } }
    final class Reply {
        let semaphore = DispatchSemaphore(value: 0), lock = NSLock()
        var results = [Result<String, Error>]()
        func receive(_ result: Result<String, Error>) { lock.lock(); results.append(result); lock.unlock(); semaphore.signal() }
        func wait() -> Result<String, Error>? {
            guard semaphore.wait(timeout: .now() + 8) == .success else { return nil }
            lock.lock(); defer { lock.unlock() }; return results.last
        }
        var count: Int { lock.lock(); defer { lock.unlock() }; return results.count }
    }
    static func value(_ result: Result<String, Error>?) -> String? { if case .success(let value) = result { return value }; return nil }
    static func error(_ result: Result<String, Error>?) -> Error? { if case .failure(let error) = result { return error }; return nil }
    static func owner(queue: DispatchQueue = .global()) -> T3MobileFaviconDownload {
        T3MobileFaviconDownload(callbackQueue: queue, protocolClasses: [FaviconProtocol.self])
    }
    fileprivate static func load(_ owner: T3MobileFaviconDownload, _ name: String, _ fixture: FaviconProtocol.Fixture, id: String? = nil) -> Reply {
        FaviconProtocol.lock.lock(); FaviconProtocol.fixtures["/" + name] = fixture; FaviconProtocol.lock.unlock()
        let reply = Reply(); owner.load(requestId: id ?? name, url: URL(string: "https://favicon.invalid/" + name)!, completion: reply.receive)
        return reply
    }
    static func main() {
        let sourceLimit = 4 * 1024 * 1024
        let download = owner()
        for (name, mime, bytes) in [("small.png", "IMAGE/PNG; charset=binary", png),
                                   ("small.svg", "image/svg+xml", Data("<svg/>".utf8)),
                                   ("small.gif", "image/gif", Data("GIF89a".utf8)),
                                   ("small.ico", "image/x-icon", Data([0, 0, 1, 0]))] {
            let reply = load(download, name, .init(headers: ["Content-Type": mime], chunks: [bytes]))
            let result = value(reply.wait())
            check(result?.hasSuffix(bytes.base64EncodedString()) == true, "\(name) keeps exact served bytes")
            check(result?.count ?? Int.max <= 32 * 1024, "\(name) bridge output bounded")
            check(reply.count == 1, "\(name) exactly one callback")
        }
        for name in ["missing-length.png", "lying-length.png"] {
            let headers = name.hasPrefix("lying") ? ["Content-Length": "1"] : [:]
            let bytes = png
            let result = value(load(download, name, .init(headers: headers, chunks: [bytes.prefix(20), bytes.dropFirst(20)])).wait())
            check(result == "data:image/png;base64," + png.base64EncodedString(), "\(name) counts streamed bytes independently")
        }
        let tooBigDeclared = load(download, "declared.png", .init(headers: ["Content-Length": String(sourceLimit + 1)], chunks: [png]))
        check(error(tooBigDeclared.wait()) as? T3MobileFaviconDownload.Failure == .tooLarge, "declared limit rejects before decode")
        for (name, headers) in [("overflow.png", [String: String]()), ("lying-overflow.png", ["Content-Length": "1"])] {
            let reply = load(download, name, .init(headers: headers, chunks: [Data(repeating: 0, count: sourceLimit), Data([0])]))
            check(error(reply.wait()) as? T3MobileFaviconDownload.Failure == .tooLarge, "\(name) actual byte overflow rejects")
        }
        var exact = png; exact.append(Data(repeating: 0, count: sourceLimit - png.count))
        let exactResult = load(download, "exact.png", .init(headers: ["Content-Length": String(sourceLimit)], chunks: [exact])).wait()
        check(value(exactResult) != nil, "exact 4MiB stream reaches codec and downsizes")
        check(value(exactResult)?.count ?? Int.max <= 32 * 1024, "exact 4MiB stream never crosses bridge raw")
        for status in [199, 301, 404, 500] {
            let result = load(download, "status-\(status).png", .init(status: status, chunks: [png])).wait()
            check(error(result) as? T3MobileFaviconDownload.Failure == .response(status), "HTTP \(status) refuses decode")
        }
        check(value(load(download, "status-201.png", .init(status: 201, chunks: [png])).wait()) != nil, "all 2xx accepted")
        check(error(load(download, "failure.png", .init(fail: true)).wait()) as? T3MobileFaviconDownload.Failure == .network, "network error has sanitized message")
        check(error(load(download, "empty.png", .init()).wait()) != nil, "empty body decode fails")
        check(error(load(download, "large.svg", .init(chunks: [Data(repeating: 32, count: 32768)])).wait()) != nil, "large SVG refuses rasterization")
        let invalid = Reply(); download.load(requestId: "invalid", url: URL(string: "file:///tmp/private.png")!, completion: invalid.receive)
        check(error(invalid.wait()) as? T3MobileFaviconDownload.Failure == .invalidURL, "non-network URL refused")
        for authority in ["user:password@", "user@", ":password@", "%75ser@"] {
            let rejected = Reply()
            download.load(requestId: "userinfo", url: URL(string: "https://" + authority + "favicon.invalid/private.png")!, completion: rejected.receive)
            check(error(rejected.wait()) as? T3MobileFaviconDownload.Failure == .invalidURL, "nonempty URL credential is refused before networking")
        }
        let emptyUser = Reply()
        download.load(requestId: "empty-userinfo", url: URL(string: "https://@favicon.invalid/small.png")!, completion: emptyUser.receive)
        check(value(emptyUser.wait()) != nil, "empty URL userinfo matches Fetch acceptance")
        check(download.activeRequestCount == 0, "finished and failed tasks leave owner")

        let cancel = load(download, "cancel.png", .init(chunks: [png], delay: 0.3))
        download.cancel(requestId: "cancel.png")
        check(error(cancel.wait()) is CancellationError, "request-ID cancellation completes awaiting caller")
        check(download.activeRequestCount == 0, "cancelled request released")
        let old = load(download, "old.png", .init(chunks: [png], delay: 0.3), id: "same")
        let new = load(download, "new.png", .init(chunks: [png]), id: "same")
        check(error(old.wait()) is CancellationError, "reused request ID cancels old owner")
        check(value(new.wait()) != nil, "old completion cannot remove replacement owner")

        let firstChunk = DispatchSemaphore(value: 0)
        let streaming = load(download, "streaming.png", .init(chunks: [png.prefix(20), png.dropFirst(20)], chunkDelay: 0.3, firstChunk: firstChunk))
        check(firstChunk.wait(timeout: .now() + 3) == .success, "stream starts before cancellation")
        download.cancel(requestId: "streaming.png")
        check(error(streaming.wait()) is CancellationError, "mid-stream cancellation refuses partial decode")

        let replaceQueue = DispatchQueue(label: "favicon.test.replacement"); replaceQueue.suspend()
        let replaceOwner = owner(queue: replaceQueue)
        let pendingOld = load(replaceOwner, "pending-old.png", .init(chunks: [png]), id: "pending")
        let pendingNew = load(replaceOwner, "pending-new.png", .init(chunks: [png]), id: "pending")
        replaceQueue.resume()
        check(error(pendingOld.wait()) is CancellationError, "replacement cancels old callback queued on suspended queue")
        check(value(pendingNew.wait()) != nil && replaceOwner.activeRequestCount == 0, "old pending callback cannot erase replacement")
        replaceOwner.shutdown()

        let suspended = DispatchQueue(label: "favicon.test.suspended"); suspended.suspend()
        let delayedOwner = owner(queue: suspended)
        let pending = load(delayedOwner, "queued.png", .init(chunks: [png]))
        // A deterministic queue barrier gates callback delivery; cancellation must win even if decoding already ended.
        Thread.sleep(forTimeInterval: 0.1)
        delayedOwner.cancel(requestId: "queued.png"); suspended.resume()
        check(error(pending.wait()) is CancellationError, "cancellation defeats queued success")
        delayedOwner.shutdown()
        let teardownQueue = DispatchQueue(label: "favicon.test.teardown"); teardownQueue.suspend()
        let teardownOwner = owner(queue: teardownQueue)
        let queuedTeardown = load(teardownOwner, "teardown-queued.png", .init(chunks: [exact]))
        teardownOwner.shutdown(); teardownQueue.resume()
        check(error(queuedTeardown.wait()) is CancellationError, "shutdown refuses queued decode or success completion")
        let shutdownA = load(download, "shutdown-a.png", .init(chunks: [png], delay: 0.3))
        let shutdownB = load(download, "shutdown-b.png", .init(chunks: [png], delay: 0.3))
        download.shutdown(); download.shutdown()
        check(error(shutdownA.wait()) is CancellationError && error(shutdownB.wait()) is CancellationError, "shutdown cancels every pending request")
        check(download.activeRequestCount == 0, "shutdown drains task ownership")
        check(error(load(download, "after.png", .init(chunks: [png])).wait()) is CancellationError, "shutdown owner cannot restart")
        var released: T3MobileFaviconDownload? = owner()
        weak let weakOwner = released
        let releaseReply = load(released!, "release.png", .init(chunks: [png], delay: 0.3)); released = nil
        check(weakOwner == nil, "session delegate does not retain owner")
        check(error(releaseReply.wait()) is CancellationError, "deinit cancels and completes request")
        let decodeQueue = DispatchQueue(label: "favicon.test.decode"); decodeQueue.suspend()
        var decodeOwner: T3MobileFaviconDownload? = T3MobileFaviconDownload(callbackQueue: .global(), protocolClasses: [FaviconProtocol.self], decodeQueue: decodeQueue)
        weak let weakDecodeOwner = decodeOwner
        let decodeReply = load(decodeOwner!, "release-decode.png", .init(chunks: [exact]))
        let decodeDeadline = Date().addingTimeInterval(3)
        while decodeOwner!.activeDecodeCount == 0 && Date() < decodeDeadline { Thread.sleep(forTimeInterval: 0.001) }
        check(decodeOwner!.activeDecodeCount == 1, "real URLSession stream reaches suspended decode queue")
        decodeOwner = nil
        check(weakDecodeOwner == nil, "queued codec work does not retain the download owner")
        check(error(decodeReply.wait()) is CancellationError, "deinit completes cancellation while decode queue is suspended")
        decodeQueue.resume()
        Thread.sleep(forTimeInterval: 0.4)
        check(decodeReply.count == 1, "abandoned queued decode never delivers another result")
        check(cancel.count == 1 && old.count == 1 && shutdownA.count == 1 && shutdownB.count == 1 && releaseReply.count == 1,
              "late network work never delivers a second callback")
        FaviconProtocol.lock.lock(); let requests = FaviconProtocol.requests, stopped = FaviconProtocol.stopped; FaviconProtocol.lock.unlock()
        check(stopped.contains("/streaming.png"), "mid-stream task reaches URLProtocol cancellation cleanup")
        check(streaming.count == 1 && pendingOld.count == 1 && pendingNew.count == 1, "stream and reused-ID callbacks remain exactly once")
        check(requests.allSatisfy { $0.value(forHTTPHeaderField: "Authorization") == nil && $0.value(forHTTPHeaderField: "Cookie") == nil },
              "image requests contain no transport bearer or cookie")
        do {
            let server = try FaviconServer(); defer { server.stop() }
            let real = T3MobileFaviconDownload(callbackQueue: .global()); defer { real.shutdown() }
            let reply = Reply(); real.load(requestId: "redirect", url: server.url, completion: reply.receive)
            check(value(reply.wait()) == "data:image/png;base64," + png.base64EncodedString(), "real HTTP redirect uses original URL MIME fallback")
            server.lock.lock(); let seen = server.requests; server.lock.unlock()
            check(seen.count == 2 && seen.last?.hasPrefix("GET /no-extension ") == true, "URLSession follows ordinary redirect")
            check(seen.allSatisfy { !$0.lowercased().contains("authorization:") && !$0.lowercased().contains("cookie:") }, "real redirect adds no credentials")
            check(real.activeRequestCount == 0, "real network task released")
        } catch { check(false, "real HTTP fixture: \(error.localizedDescription)") }
        for failure in failures { print("FAIL: \(failure)") }
        print("Favicon download: \(checks - failures.count)/\(checks) checks passed")
        if !failures.isEmpty { exit(1) }
    }
}

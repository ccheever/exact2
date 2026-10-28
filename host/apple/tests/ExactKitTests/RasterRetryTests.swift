#if os(macOS)
import AppKit
import ImageIO
import XCTest
import Darwin
@testable import ExactKit

/// A remote picture whose download failed in a way asking again may mend is
/// asked again while it is on screen (LLP 1011 §4, the Linux host's rules):
/// the ladder in milliseconds here (`RasterRetry.scale`).
final class RasterRetryTests: XCTestCase {
    override func setUp() { RasterRetry.scale = 0.01; RasterRetry.wholeScale = 1 }
    override func tearDown() { RasterRetry.scale = 1; RasterRetry.wholeScale = 1 }

    func testAFailedDownloadOnScreenIsAskedAgainAndShows() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        let server = try ScriptedRasterHTTP { _, n in n < 2 ? .status(503) : .picture }
        defer { server.stop() }
        node.loadGeneration = 1
        loader.load(node, source: "\(server.origin)/flaky.png", resolver: resolver)
        settle { node.raster != nil }
        XCTAssertEqual(server.times.count, 3, "busy twice, then the picture")
        XCTAssertEqual(loader.testAnnounced.map(\.failure), [nil], "its load, and no error before it")
        XCTAssertEqual(images(loader).first?["failure"] as? String, "")
        XCTAssertEqual(images(loader).first?["retries"] as? Int, 2)
    }

    func testANon2xxThatAskingAgainCannotMendFailsAtOnce() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        let server = try ScriptedRasterHTTP { _, _ in .status(404) }
        defer { server.stop() }
        node.loadGeneration = 1
        loader.load(node, source: "\(server.origin)/gone.png", resolver: resolver)
        settle { images(loader).first?["failure"] as? String == "HTTP 404" }
        XCTAssertEqual(images(loader).first?["held"] as? Bool, false)
        pump(0.3)
        XCTAssertEqual(server.times.count, 1)
    }

    func testAServersRetryAfterIsNeverAskedSooner() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        let server = try ScriptedRasterHTTP { _, n in n == 0 ? .status(429, retryAfter: "1") : .picture }
        defer { server.stop() }
        node.loadGeneration = 1
        loader.load(node, source: "\(server.origin)/later.png", resolver: resolver)
        settle { node.raster != nil }
        let times = server.times
        XCTAssertEqual(times.count, 2)
        XCTAssertGreaterThanOrEqual(times[1].timeIntervalSince(times[0]), 1, "not before the server's second, though the ladder's step is 10 ms")
    }

    /// A wait past what a picture waits ends the showing's retries: its
    /// `error` goes out, and a new view of the URL asks nothing meanwhile.
    func testARetryAfterPastWhatAPictureWaitsFailsAndARemountAsksNothing() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        let server = try ScriptedRasterHTTP { _, _ in .status(503, retryAfter: "600") }
        defer { server.stop() }
        node.loadGeneration = 1
        let url = "\(server.origin)/wait.png"
        loader.load(node, source: url, resolver: resolver)
        settle { images(loader).first?["failure"] as? String == "HTTP 503" && images(loader).first?["held"] as? Bool == false }
        loader.cancel(node.id)
        node.loadGeneration = 2
        loader.load(node, source: url, resolver: resolver)
        settle { images(loader).first?["failure"] as? String == "HTTP 503" && images(loader).first?["held"] as? Bool == false }
        XCTAssertEqual(server.times.count, 1, "the server's wait is kept by URL")
    }

    /// Fails on screen, leaves it, comes back: the ladder starts from
    /// nothing, and the `error` comes once a showing's retries are spent.
    func testAPictureThatLeavesTheScreenStartsItsRetriesAgainWhenItShows() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        RasterRetry.scale = 0.05
        let server = try ScriptedRasterHTTP { _, _ in .status(503) }
        defer { server.stop() }
        node.loadGeneration = 1
        loader.load(node, source: "\(server.origin)/dead.png", resolver: resolver)
        settle { (images(loader).first?["retries"] as? Int ?? 0) >= 2 }
        let shown = node.frame
        // Laid out off screen, as a layout pass tells the loader.
        node.frame = shown.offsetBy(dx: 5000, dy: 0); loader.resized(node)
        // (A retry under way lands first, off screen.)
        settle { images(loader).first?["held"] as? Bool == true && images(loader).first?["retries"] as? Int == 0 }
        let asked = server.times.count
        pump(1)
        XCTAssertEqual(server.times.count, asked, "nothing is asked while it is off screen")
        XCTAssertEqual(images(loader).first?["held"] as? Bool, true, "and no error yet")
        node.frame = shown; loader.resized(node)
        // (Between retries it is neither held nor failed: a download is under way.)
        settle { images(loader).first?["held"] as? Bool == false && images(loader).first?["failure"] as? String == "HTTP 503" }
        XCTAssertEqual(server.times.count, asked + RasterRetry.steps.count, "a whole ladder after it showed again")
        XCTAssertEqual(loader.testAnnounced.map(\.failure), ["HTTP 503"], "one error, after the retries, and no load")
    }

    /// Leaving and coming back while a retry downloads starts the count over too.
    func testLeavingWhileARetryDownloadsStartsTheCountOver() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        RasterRetry.scale = 0.02
        let server = try ScriptedRasterHTTP(delay: 0.4) { _, _ in .status(503) }
        defer { server.stop() }
        node.loadGeneration = 1
        loader.load(node, source: "\(server.origin)/slow.png", resolver: resolver)
        // A retry's request at the server, its answer not yet back.
        settle {
            let retries = images(loader).first?["retries"] as? Int ?? 0
            return images(loader).first?["retrying"] as? Bool == true && retries >= 2 && server.times.count == retries + 1
        }
        let asked = server.times.count, shown = node.frame
        node.frame = shown.offsetBy(dx: 5000, dy: 0); loader.resized(node); pump(0.05)
        node.frame = shown; loader.resized(node)
        settle { images(loader).first?["held"] as? Bool == false && images(loader).first?["failure"] as? String == "HTTP 503" }
        XCTAssertEqual(server.times.count, asked + RasterRetry.steps.count, "the retry under way, then a whole ladder")
        XCTAssertEqual(loader.testAnnounced.map(\.failure), ["HTTP 503"])
    }

    /// Two views of one URL: one retry downloads for both, the other joining it.
    func testViewsOfOneURLShareTheirRetries() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        let second = NodeView(id: 2, kind: "image", presenter: presenter)
        presenter.views[2] = second; presenter.viewport.addSubview(second); second.frame = node.frame
        defer { loader.shutdown(); node.raster = nil; second.raster = nil; window.close(); try? FileManager.default.removeItem(at: root) }
        let server = try ScriptedRasterHTTP { _, n in n < 2 ? .status(502) : .picture }
        defer { server.stop() }
        node.loadGeneration = 1; second.loadGeneration = 1
        let url = "\(server.origin)/shared.png"
        loader.load(node, source: url, resolver: resolver); loader.load(second, source: url, resolver: resolver)
        settle { node.raster != nil && second.raster != nil }
        XCTAssertEqual(server.times.count, 3, "two failures, then the picture, for both")
        XCTAssertEqual(loader.testAnnounced.filter { $0.failure != nil }.count, 0)
    }

    /// A picture that fills the screen has the longer whole-load bound: a
    /// body that takes longer than the ordinary one lands.
    func testAPictureThatFillsTheScreenHasTheLongerBound() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); presenter.views.values.forEach { $0.raster = nil }; window.close(); try? FileManager.default.removeItem(at: root) }
        RasterRetry.wholeScale = 0.01 // 0.3 s, and 1.2 s for one that fills the screen
        let server = try ScriptedRasterHTTP(delay: 0.7) { _, _ in .picture }
        defer { server.stop() }
        node.frame = NSRect(x: 0, y: 0, width: 320, height: 200); node.loadGeneration = 1
        loader.resized(node)
        loader.load(node, source: "\(server.origin)/viewer.png", resolver: resolver)
        loader.resized(node)
        settle { node.raster != nil }
        XCTAssertEqual(server.times.count, 1, "within the longer bound, at its first ask")

        let small = NodeView(id: 2, kind: "image", presenter: presenter)
        presenter.views[2] = small; presenter.viewport.addSubview(small)
        small.frame = NSRect(x: 0, y: 0, width: 40, height: 40); small.loadGeneration = 1
        loader.load(small, source: "\(server.origin)/thumb.png", resolver: resolver)
        settle { (images(loader).first { $0["view"] as? UInt32 == 2 }?["retries"] as? Int ?? 0) >= 1 }
        XCTAssertNil(small.raster, "an ordinary picture's whole load times out at its bound")
    }

    /// A first download that could fill the screen, scrolled on screen while
    /// it downloads: hurried to the longer bound at its first ask.
    func testAPictureScrolledOnToFillTheScreenIsHurriedWhileItDownloads() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root) }
        RasterRetry.wholeScale = 0.01 // 0.3 s, and 1.2 s for one that fills the screen
        let server = try ScriptedRasterHTTP(delay: 0.7) { _, _ in .picture }
        defer { server.stop() }
        let scroller = NSScrollView(frame: NSRect(x: 0, y: 0, width: 320, height: 200))
        let document = NSView(frame: NSRect(x: 0, y: 0, width: 320, height: 2000))
        scroller.documentView = document; scroller.contentView.postsBoundsChangedNotifications = true
        presenter.viewport.addSubview(scroller)
        node.removeFromSuperview(); document.addSubview(node)
        node.frame = NSRect(x: 0, y: 1500, width: 320, height: 200); node.loadGeneration = 1
        loader.load(node, source: "\(server.origin)/below.png", resolver: resolver)
        settle { server.times.count == 1 }
        scroller.contentView.scroll(to: NSPoint(x: 0, y: 1500)); scroller.reflectScrolledClipView(scroller.contentView)
        settle { node.raster != nil }
        XCTAssertEqual(server.times.count, 1, "within the longer bound, at its first ask")
    }

    /// The same, when its box grows to fill the screen while it downloads off screen.
    func testAPictureThatGrowsOffScreenThenScrollsOnIsHurried() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root) }
        RasterRetry.wholeScale = 0.01
        let server = try ScriptedRasterHTTP(delay: 0.7) { _, _ in .picture }
        defer { server.stop() }
        let scroller = NSScrollView(frame: NSRect(x: 0, y: 0, width: 320, height: 200))
        let document = NSView(frame: NSRect(x: 0, y: 0, width: 320, height: 2000))
        scroller.documentView = document; scroller.contentView.postsBoundsChangedNotifications = true
        presenter.viewport.addSubview(scroller)
        node.removeFromSuperview(); document.addSubview(node)
        node.frame = NSRect(x: 0, y: 1500, width: 20, height: 20); node.loadGeneration = 1
        loader.load(node, source: "\(server.origin)/grows.png", resolver: resolver)
        settle { server.times.count == 1 }
        node.frame = NSRect(x: 0, y: 1500, width: 320, height: 200); loader.resized(node); pump(0.02)
        scroller.contentView.scroll(to: NSPoint(x: 0, y: 1500)); scroller.reflectScrolledClipView(scroller.contentView)
        settle { node.raster != nil }
        XCTAssertEqual(server.times.count, 1, "within the longer bound, at its first ask")
    }

    func testARetryAfterIsReadAsSecondsOrADate() {
        let now = Date(timeIntervalSince1970: 784_111_777) // Sun, 06 Nov 1994 08:49:37 GMT
        XCTAssertEqual(RasterRetry.retryAfter(" 7 ", now: now), 7)
        XCTAssertEqual(RasterRetry.retryAfter("Sun, 06 Nov 1994 08:50:07 GMT", now: now), 30)
        XCTAssertEqual(RasterRetry.retryAfter("Sun, 06 Nov 1994 08:49:00 GMT", now: now), 0)
        XCTAssertEqual(RasterRetry.retryAfter("18446744073709551615", now: now), 10 * 365 * 86_400)
        for unread in ["", "-1", "1.5", "soon", "99999999999999999999999"] {
            XCTAssertNil(RasterRetry.retryAfter(unread, now: now), unread)
        }
        XCTAssertTrue(RasterRetry.transient(URLError(.timedOut)))
        XCTAssertTrue(RasterRetry.transient(RasterHTTPStatus(code: 502)))
        XCTAssertTrue(RasterRetry.transient(RasterHTTPStatus(code: 429)))
        XCTAssertTrue(RasterRetry.transient(RasterEmptyResponse()))
        XCTAssertFalse(RasterRetry.transient(RasterHTTPStatus(code: 404)))
        XCTAssertFalse(RasterRetry.transient(URLError(.cancelled)))
        XCTAssertFalse(RasterRetry.transient(RasterFailure.encodedLimit))
    }

    private func images(_ loader: RasterLoader) -> [[String: Any]] { loader.diagnostics["images"] as? [[String: Any]] ?? [] }
    private func pump(_ seconds: TimeInterval) {
        let end = Date(timeIntervalSinceNow: seconds)
        while Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
    }
    private func settle(file: StaticString = #file, line: UInt = #line, _ predicate: () -> Bool) {
        let hang = Date(timeIntervalSinceNow: 60)
        while !predicate() && Date() < hang { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
        XCTAssertTrue(predicate(), file: file, line: line)
    }
    private func fixture() throws -> (URL, AssetResolver, Presenter, NodeView, RasterLoader, NSWindow) {
        _ = NSApplication.shared
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-raster-retry-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        let presenter = Presenter()
        let node = NodeView(id: 1, kind: "image", presenter: presenter)
        presenter.views[1] = node
        node.frame = NSRect(x: 0, y: 0, width: 100, height: 50)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport; presenter.viewport.addSubview(node)
        return (root, AssetResolver(root: root), presenter, node, RasterLoader(budget: 32 * 1024 * 1024), window)
    }
}

/// Loopback HTTP/1.1 answering each request as `answer(path, n)` says, `n`
/// the requests for that path before it; `times` when each came.
private final class ScriptedRasterHTTP: @unchecked Sendable {
    enum Answer { case status(Int, retryAfter: String? = nil), picture }
    private let listener: Int32
    private let lock = NSLock()
    private var seen: [(String, Date)] = []
    private let answer: (String, Int) -> Answer
    private let delay: TimeInterval
    let origin: String
    var times: [Date] { lock.lock(); defer { lock.unlock() }; return seen.map(\.1) }
    private static let picture: Data = {
        let context = CGContext(data: nil, width: 6, height: 4, bitsPerComponent: 8, bytesPerRow: 24,
                                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(red: 0.2, green: 0.4, blue: 0.6, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: 6, height: 4))
        let data = NSMutableData()
        let destination = CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil)!
        CGImageDestinationAddImage(destination, context.makeImage()!, nil); CGImageDestinationFinalize(destination)
        return data as Data
    }()
    init(delay: TimeInterval = 0, answer: @escaping (String, Int) -> Answer) throws {
        self.answer = answer; self.delay = delay
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        listener = fd
        guard fd >= 0 else { throw NSError(domain: "socket", code: 1) }
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        let bound = withUnsafePointer(to: &address) { p in p.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        guard bound == 0, listen(fd, 16) == 0 else { close(fd); throw NSError(domain: "listen", code: 1) }
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &address) { p in p.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) } }
        // A fresh path prefix: the response cache and the kept waits are the process's.
        origin = "http://127.0.0.1:\(UInt16(bigEndian: address.sin_port))/\(UUID().uuidString)"
        Thread.detachNewThread { [self] in
            while true {
                let client = accept(listener, nil, nil)
                if client < 0 { break }
                Thread.detachNewThread { [self] in respond(client); close(client) }
            }
        }
    }
    private func respond(_ client: Int32) {
        var noPipe: Int32 = 1
        setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &noPipe, socklen_t(MemoryLayout<Int32>.size))
        var buffer = [UInt8](repeating: 0, count: 4096), request = Data()
        while request.range(of: Data("\r\n\r\n".utf8)) == nil {
            let count = recv(client, &buffer, buffer.count, 0)
            if count <= 0 { return }
            request.append(contentsOf: buffer.prefix(count))
        }
        let path = String(decoding: request, as: UTF8.self).split(separator: " ").dropFirst().first.map(String.init) ?? "/"
        lock.lock(); let n = seen.filter { $0.0 == path }.count; seen.append((path, Date())); lock.unlock()
        if delay > 0 { Thread.sleep(forTimeInterval: delay) }
        let response: Data
        switch answer(path, n) {
        case .picture:
            let body = Self.picture
            response = Data("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nCache-Control: no-store\r\nContent-Length: \(body.count)\r\nConnection: close\r\n\r\n".utf8) + body
        case .status(let code, let retryAfter):
            let after = retryAfter.map { "Retry-After: \($0)\r\n" } ?? ""
            response = Data("HTTP/1.1 \(code) Nope\r\n\(after)Cache-Control: no-store\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".utf8)
        }
        response.withUnsafeBytes { bytes in
            var offset = 0
            while offset < bytes.count {
                let sent = send(client, bytes.baseAddress!.advanced(by: offset), bytes.count - offset, 0)
                if sent <= 0 { break }; offset += sent
            }
        }
    }
    func stop() { shutdown(listener, SHUT_RDWR); close(listener) }
}
#endif

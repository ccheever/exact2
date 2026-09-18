#if os(macOS)
import AppKit
import ImageIO
import XCTest
import Darwin
import CExact
@testable import ExactKit

final class RasterLoaderTests: XCTestCase {
    private func png(_ root: URL, _ name: String, width: Int, height: Int, identity: Int) throws {
        let context = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
            bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(CGColor(colorSpace: CGColorSpace(name: CGColorSpace.sRGB)!,
            components: [CGFloat(identity & 255) / 255, CGFloat((identity >> 8) & 255) / 255, 0.5, 1])!)
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let image = try XCTUnwrap(context.makeImage())
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(root.appendingPathComponent(name) as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
    }
    private func settle(file: StaticString = #file, line: UInt = #line, _ predicate: () -> Bool) {
        let end = Date(timeIntervalSinceNow: 5)
        while !predicate() && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
        XCTAssertTrue(predicate(), file: file, line: line)
    }
    private func fixture() throws -> (URL, AssetResolver, Presenter, NodeView, RasterLoader, NSWindow) {
        _ = NSApplication.shared
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-raster-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        let presenter = Presenter()
        let node = NodeView(id: 1, kind: "image", presenter: presenter)
        presenter.views[1] = node
        node.frame = NSRect(x: 0, y: 0, width: 100, height: 50)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport; presenter.viewport.addSubview(node)
        return (root, AssetResolver(root: root), presenter, node, RasterLoader(), window)
    }
    func testCreationStartsBeforePresenterRegistersView() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root) }
        try png(root, "created.png", width: 120, height: 80, identity: 7)
        presenter.views.removeValue(forKey: node.id)
        node.loadGeneration = 1
        XCTAssertTrue(loader.load(node, source: "created.png", resolver: resolver))
        // Presenter applies initial props inside create, then registers the view.
        presenter.views[node.id] = node
        settle { node.raster != nil }
        XCTAssertEqual(node.raster?.image.naturalSize, CGSize(width: 120, height: 80))
    }
    func testReplacementKeepsBackingAndNaturalGeometryUntilMatchingAcceptance() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root) }
        try png(root, "a.png", width: 4000, height: 2000, identity: 1)
        try png(root, "b.png", width: 64, height: 64, identity: 2)
        try png(root, "c.png", width: 120, height: 80, identity: 3)
        var intrinsic = CGSize.zero
        presenter.onIntrinsic = { _, size in intrinsic = size ?? .zero }
        node.loadGeneration = 1
        loader.load(node, source: "a.png", resolver: resolver)
        settle { node.raster != nil }
        let a = try XCTUnwrap(node.raster)
        XCTAssertEqual(intrinsic, CGSize(width: 4000, height: 2000))
        node.loadGeneration = 2
        loader.load(node, source: "b.png", resolver: resolver)
        node.loadGeneration = 3
        loader.load(node, source: "c.png", resolver: resolver)
        XCTAssertTrue(node.raster === a)
        XCTAssertEqual(intrinsic, a.image.naturalSize)
        settle { node.raster?.image.naturalSize == CGSize(width: 120, height: 80) }
        XCTAssertEqual(intrinsic, CGSize(width: 120, height: 80))
        // Old A remains a charged alias while C is accepted; reset cannot debit A.
        loader.reset()
        XCTAssertTrue((loader.diagnostics["residentBytes"] as? UInt64 ?? 0) >= UInt64(a.image.residentBytes))
    }
    func testTwentyGroupsOfDistinctReplacementsBoundSourceMaps() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        // 240 distinct sources, produced on demand: never allocate a 25k-image fixture.
        var seen = Set<UInt8>()
        var first: NativeRasterLease?
        for traversal in 0..<20 {
            for row in 0..<12 {
                let identity = traversal * 12 + row
                let name = "row-\(identity).png"
                try png(root, name, width: 120, height: 60, identity: identity)
                node.loadGeneration += 1
                let before = node.raster
                loader.load(node, source: name, resolver: resolver)
                settle { node.raster != nil && node.raster !== before }
                if identity == 0 { first = node.raster }
                let bitmap = try XCTUnwrap(node.raster?.image.image)
                let bytes = try XCTUnwrap(bitmap.dataProvider?.data)
                seen.insert(CFDataGetBytePtr(bytes)[0])
                let s = loader.diagnostics
                XCTAssertTrue((s["peakBytes"] as? UInt64 ?? UInt64.max) <= 32 * 1024 * 1024)
                XCTAssertTrue((s["running"] as? UInt64 ?? UInt64.max) <= 2)
                XCTAssertTrue((s["deliveryCells"] as? UInt64 ?? UInt64.max) <= 2)
                XCTAssertTrue((s["pending"] as? UInt64 ?? UInt64.max) <= 64)
                XCTAssertTrue((s["subscribers"] as? UInt64 ?? UInt64.max) <= 1024)
                XCTAssertTrue((s["coldEntries"] as? UInt64 ?? UInt64.max) <= 64)
                XCTAssertTrue((s["sources"] as? Int ?? Int.max) <= 130)
                try FileManager.default.removeItem(at: root.appendingPathComponent(name))
            }
        }
        XCTAssertEqual(seen.count, 240)
        XCTAssertEqual(loader.diagnostics["decoded"] as? Int, 240)
        // A stays pinned while >64 cold source identities come and go. Its
        // original file is already deleted: a lost identity cannot pass by
        // silently decoding it a second time.
        let again = NodeView(id: 2, kind: "image", presenter: presenter)
        presenter.views[2] = again; presenter.viewport.addSubview(again); again.frame = node.frame
        loader.load(again, source: "row-0.png", resolver: resolver)
        settle { again.raster != nil }
        XCTAssertTrue(again.raster?.image.image === first?.image.image)
        XCTAssertEqual(loader.diagnostics["decoded"] as? Int, 240)

        // Same name, explicitly new immutable generation must not alias A.
        try png(root, "row-0.png", width: 120, height: 60, identity: 250)
        loader.invalidate("row-0.png"); again.loadGeneration += 1
        loader.load(again, source: "row-0.png", resolver: resolver)
        settle { again.raster != nil && again.raster?.image.image !== first?.image.image }
        XCTAssertEqual(loader.diagnostics["decoded"] as? Int, 241)
        let providerAlias = try XCTUnwrap(first?.image.image)
        first = nil; node.raster = nil; again.raster = nil; loader.reset()
        XCTAssertEqual(loader.diagnostics["sources"] as? Int, 0)
        XCTAssertEqual(loader.diagnostics["subscribers"] as? UInt64, 0)
        // Exact cannot claim eviction freed a painter/framework alias. The
        // standalone provider-drop test checks the eventual last-owner release.
        withExtendedLifetime(providerAlias) {
            XCTAssertTrue((loader.diagnostics["residentBytes"] as? UInt64 ?? 0) >= UInt64(providerAlias.bytesPerRow * providerAlias.height))
        }
    }
    func testDownsizedAdmissionDoesNotRestartOnUnchangedLayout() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); presenter.views.values.forEach { $0.raster = nil }; window.close(); try? FileManager.default.removeItem(at: root) }
        var nodes = [node]
        for i in 0..<30 {
            let item = i == 0 ? node : NodeView(id: UInt32(i + 1), kind: "image", presenter: presenter)
            if i > 0 { nodes.append(item); presenter.views[item.id] = item; presenter.viewport.addSubview(item) }
            item.frame = CGRect(x: 0, y: 0, width: 1200, height: 600)
            try autoreleasepool { try png(root, "pressure-\(i).png", width: 1200, height: 600, identity: i) }
            item.loadGeneration = 1
            loader.load(item, source: "pressure-\(i).png", resolver: resolver)
        }
        settle { nodes.allSatisfy { $0.raster != nil } }
        let reduced = try XCTUnwrap(nodes.first { ($0.raster?.image.image.width ?? 1200) < 1200 })
        let image = reduced.raster
        let cancellations = loader.diagnostics["cancelled"] as? UInt64
        for _ in 0..<100 { loader.resized(reduced) }
        XCTAssertEqual(loader.diagnostics["cancelled"] as? UInt64, cancellations)
        XCTAssertTrue(reduced.raster === image)
    }
    func testMetadataBootstrapUpgradesOnceRealGeometryIsKnown() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root) }
        try png(root, "bootstrap.png", width: 1200, height: 600, identity: 50)
        node.frame = .zero; node.loadGeneration += 1
        presenter.onIntrinsic = { _, size in
            node.frame = CGRect(origin: .zero, size: size ?? .zero); loader.resized(node)
        }
        loader.load(node, source: "bootstrap.png", resolver: resolver)
        settle { node.raster?.image.image.width == 1200 }
        let decoded = loader.diagnostics["decoded"] as? Int
        for _ in 0..<100 { loader.resized(node) }
        XCTAssertEqual(loader.diagnostics["decoded"] as? Int, decoded)
    }
    func testRetiringTwoHangingHTTPInspectionsFreesWorkersForAnotherSession() throws {
        for operation in ["cancel", "reset", "pause", "shutdown"] {
            let server = try HangingRasterHTTP()
            defer { server.stop() }
            let (root, resolver, presenter, first, loader, window) = try fixture()
            let other = RasterLoader()
            defer { loader.shutdown(); other.shutdown(); presenter.views.values.forEach { $0.raster = nil }; window.close(); try? FileManager.default.removeItem(at: root) }
            let second = NodeView(id: 2, kind: "image", presenter: presenter)
            let local = NodeView(id: 3, kind: "image", presenter: presenter)
            presenter.views[2] = second; presenter.views[3] = local
            presenter.viewport.addSubview(second); presenter.viewport.addSubview(local)
            second.frame = first.frame; local.frame = first.frame
            loader.load(first, source: "http://127.0.0.1:\(server.port)/a", resolver: resolver)
            loader.load(second, source: "http://127.0.0.1:\(server.port)/b", resolver: resolver)
            settle { server.accepted == 2 }
            switch operation {
            case "cancel": loader.cancel(first.id); loader.cancel(second.id)
            case "reset": loader.reset()
            case "pause": loader.setPaused(true)
            default: loader.shutdown()
            }
            try png(root, "local.png", width: 120, height: 60, identity: 90)
            other.load(local, source: "local.png", resolver: resolver)
            // No main-runloop progress: cancelled HTTP must free the workers
            // so another session can inspect local metadata independently.
            let deadline = Date(timeIntervalSinceNow: 2)
            while (other.diagnostics["metadataReads"] as? Int ?? 0) == 0 && Date() < deadline { Thread.sleep(forTimeInterval: 0.005) }
            XCTAssertEqual(other.diagnostics["metadataReads"] as? Int, 1)
            settle { local.raster != nil }
            XCTAssertEqual(server.accepted, 2) // server still never replied
        }
    }
    func testExpiredResolverAndReplacementRootDoNotReuseOldPixels() throws {
        let (root, _, presenter, node, loader, window) = try fixture()
        let replacementRoot = root.appendingPathComponent("replacement")
        try FileManager.default.createDirectory(at: replacementRoot, withIntermediateDirectories: false)
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        try png(root, "same.png", width: 120, height: 60, identity: 10)
        try png(replacementRoot, "same.png", width: 90, height: 45, identity: 20)
        var old: AssetResolver? = AssetResolver(root: root)
        weak var expired = old
        loader.load(node, source: "same.png", resolver: old!)
        settle { node.raster != nil }
        let oldBacking = node.raster
        old = nil
        settle { expired == nil }
        let replacement = AssetResolver(root: replacementRoot)
        node.loadGeneration += 1
        loader.load(node, source: "same.png", resolver: replacement)
        settle { node.raster != nil && node.raster !== oldBacking }
        XCTAssertEqual(node.raster?.image.naturalSize, CGSize(width: 90, height: 45))
        XCTAssertEqual(oldBacking?.image.naturalSize, CGSize(width: 120, height: 60))
        expired = nil
    }
    func testMetadataProgressWhileDecodeAdmissionIsContinuouslyReplenished() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        let flood = RasterAdmissionFlood()
        defer { flood.stop() }
        try png(root, "local.png", width: 120, height: 60, identity: 70)
        loader.load(node, source: "local.png", resolver: resolver)
        settle { node.raster != nil }
        XCTAssertTrue(flood.admitted > 32)
        XCTAssertEqual(loader.diagnostics["metadataReads"] as? Int, 1)
    }
}

/// Continuously replenished metadata-ready demands compete at the real worker
/// admission boundary. They have no native backend and fail after admission;
/// this tests metadata/decode turn fairness, not decoder throughput.
private final class RasterAdmissionFlood: @unchecked Sendable {
    private let session = exact_raster_session_create()
    private let lock = NSLock()
    private var running = true
    private var count = 0
    private let done = DispatchSemaphore(value: 0)
    var admitted: Int { lock.lock(); defer { lock.unlock() }; return count }
    init() {
        DispatchQueue.global().async { [self] in
            var requests = [UInt64](repeating: 0, count: 32)
            var serial: UInt64 = 0
            while true {
                lock.lock(); let keepGoing = running; lock.unlock()
                if !keepGoing { break }
                for i in requests.indices {
                    let status = exact_raster_status(session, requests[i])
                    if requests[i] == 0 || status >= 100 {
                        exact_raster_cancel(session, requests[i]); serial += 1
                        var d = ExactRasterDemand()
                        d.view = UInt64(i); d.view_generation = 1; d.source = serial; d.generation = 1
                        d.width = 1; d.height = 1; d.natural_width = 1; d.natural_height = 1
                        d.encoded_bytes = 4; d.header_bytes = 4; d.stride = 4
                        requests[i] = exact_raster_request(session, d)
                        if requests[i] != 0 { lock.lock(); count += 1; lock.unlock() }
                    }
                }
            }
            exact_raster_session_control(session, 3); done.signal()
        }
    }
    func stop() { lock.lock(); running = false; lock.unlock(); done.wait() }
}

/// Loopback-only fixture. It accepts two HTTP connections and sends no bytes;
/// the native source cancellation, not a server response, must unblock workers.
private final class HangingRasterHTTP: @unchecked Sendable {
    private let listener: Int32
    private let lock = NSLock()
    private var clients: [Int32] = []
    let port: UInt16
    var accepted: Int { lock.lock(); defer { lock.unlock() }; return clients.count }
    init() throws {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        listener = fd
        guard fd >= 0 else { throw NSError(domain: "socket", code: 1) }
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        let bound = withUnsafePointer(to: &address) { p in p.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        guard bound == 0, listen(listener, 2) == 0 else { close(listener); throw NSError(domain: "listen", code: 1) }
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &address) { p in p.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) } }
        port = UInt16(bigEndian: address.sin_port)
        DispatchQueue.global().async { [self] in
            for _ in 0..<2 {
                let client = accept(listener, nil, nil)
                if client >= 0 { lock.lock(); clients.append(client); lock.unlock() }
            }
        }
    }
    func stop() {
        shutdown(listener, SHUT_RDWR); close(listener)
        lock.lock(); let sockets = clients; clients.removeAll(); lock.unlock()
        for socket in sockets { shutdown(socket, SHUT_RDWR); close(socket) }
    }
}
#endif

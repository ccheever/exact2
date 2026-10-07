#if os(macOS)
import AppKit
import ImageIO
import XCTest
import Darwin
import CExact
@testable import ExactKit

final class RasterLoaderTests: XCTestCase {
    func testQueueFullRequestAdmissionRemainsRetryable() {
        XCTAssertTrue(RasterLoader.transientRequestRefusal(8))
        for permanent in [1, 2, 6, 7, 9, 12, 15] {
            XCTAssertFalse(RasterLoader.transientRequestRefusal(UInt64(permanent)))
        }
    }
    func testQueueFullInterestSurvivesAndPaintsOnNextWorkerWake() throws {
        let (root, resolver, presenter, seed, loader, window) = try fixture()
        let node = NodeView(id: 2, kind: "image", presenter: presenter)
        defer {
            loader.testRequest = nil; loader.shutdown(); seed.raster = nil; node.raster = nil; window.close()
            try? FileManager.default.removeItem(at: root)
        }
        try png(root, "queue.png", width: 256, height: 256, identity: 4)
        seed.frame = CGRect(x: 0, y: 0, width: 32, height: 32); seed.loadGeneration = 1
        XCTAssertTrue(loader.load(seed, source: "queue.png", resolver: resolver))
        settle { seed.raster != nil }

        node.frame = CGRect(x: 0, y: 0, width: 64, height: 64); node.loadGeneration = 1
        presenter.views[node.id] = node; presenter.viewport.addSubview(node)
        var refused = false
        loader.testRequest = { [unowned loader] demand in
            if !refused { refused = true; return (0, 8) }
            return (exact_raster_request(loader.id, demand), nil)
        }
        XCTAssertTrue(loader.load(node, source: "queue.png", resolver: resolver))
        settle { refused }
        XCTAssertNil(node.raster)
        XCTAssertEqual(loader.loadingOnScreen, 1)
        XCTAssertEqual(loader.diagnostics["deferred"] as? Int, 1)

        // A real worker completion calls the same pass. The original interest
        // must submit again and paint without a prop, resize or reload event.
        loader.reconcile()
        settle { node.raster != nil }
        XCTAssertEqual(loader.loadingOnScreen, 0)
        XCTAssertEqual(loader.diagnostics["deferred"] as? Int, 0)
    }
    /// A decode ImageIO declines is asked for again after a delay, loading
    /// all the while, and paints with no prop change, reload or new source.
    func testDeclinedDecodeAsksAgainAndPaints() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        try png(root, "busy.png", width: 120, height: 80, identity: 9)
        loader.testDecline(next: 2)
        node.loadGeneration = 1
        let start = Date()
        XCTAssertTrue(loader.load(node, source: "busy.png", resolver: resolver))
        settle { declines(loader) == 1 }
        XCTAssertNil(node.raster)
        XCTAssertEqual(loader.loadingOnScreen, 1, "a declined decode is still loading")
        settle { node.raster != nil }
        XCTAssertGreaterThanOrEqual(Date().timeIntervalSince(start), 0.75, "after 250 ms, then 500 ms")
        XCTAssertEqual(node.raster?.image.naturalSize, CGSize(width: 120, height: 80))
        XCTAssertEqual(declines(loader), 0)
        XCTAssertEqual(images(loader).first?["failure"] as? String, "")
        XCTAssertEqual(loader.loadingOnScreen, 0)
    }
    /// Past the last delay a declined decode is the image's failure.
    func testDecodeDeclinedPastTheLastDelayFails() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        try png(root, "busy.png", width: 120, height: 80, identity: 10)
        loader.testDecline(next: RasterLoader.declineDelays.count + 1)
        node.loadGeneration = 1
        XCTAssertTrue(loader.load(node, source: "busy.png", resolver: resolver))
        settle { images(loader).first?["failure"] as? String == "decode failed" }
        XCTAssertEqual(declines(loader), RasterLoader.declineDelays.count)
        XCTAssertNil(node.raster)
        XCTAssertEqual(loader.loadingOnScreen, 0)
    }
    /// A decode that failed for good is let go of: a second view of the same
    /// source and size decodes it again rather than taking the cached failure.
    func testAViewAfterAFailedDeclineDecodesAgain() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        let second = NodeView(id: 2, kind: "image", presenter: presenter)
        presenter.views[2] = second; presenter.viewport.addSubview(second)
        second.frame = node.frame
        defer { loader.shutdown(); node.raster = nil; second.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        try png(root, "busy.png", width: 120, height: 80, identity: 11)
        loader.testDecline(next: RasterLoader.declineDelays.count + 1)
        node.loadGeneration = 1
        XCTAssertTrue(loader.load(node, source: "busy.png", resolver: resolver))
        settle { images(loader).first { $0["view"] as? UInt32 == 1 }?["failure"] as? String == "decode failed" }
        second.loadGeneration = 1
        XCTAssertTrue(loader.load(second, source: "busy.png", resolver: resolver))
        settle { second.raster != nil }
        let view2 = images(loader).first { $0["view"] as? UInt32 == 2 }
        XCTAssertEqual(view2?["declines"] as? Int, 0)
        XCTAssertNil(node.raster, "the first view keeps its error")
    }
    private func images(_ loader: RasterLoader) -> [[String: Any]] { loader.diagnostics["images"] as? [[String: Any]] ?? [] }
    private func declines(_ loader: RasterLoader) -> Int? { images(loader).first?["declines"] as? Int }
    /// One decoder while any owner's list travels fast; both once none does.
    func testDecodesOneAtATimeWhileAnyListTravelsFast() {
        let workers = RasterWorkers.shared, a = NSObject(), b = NSObject()
        XCTAssertFalse(workers.single)
        workers.travelling(a, true); workers.travelling(b, true)
        XCTAssertTrue(workers.single)
        workers.travelling(a, false)
        XCTAssertTrue(workers.single, "another list still travels")
        workers.travelling(b, false)
        XCTAssertFalse(workers.single)
    }
    /// A large WebP's size is read from its prefix (ImageIO reads WebP only
    /// whole): the Bluesky CDN's `VP8X`, and synthetic `VP8L` and `VP8 `.
    func testWebPSizeFromItsHeader() throws {
        let vp8x: [UInt8] = [82, 73, 70, 70, 92, 196, 4, 0, 87, 69, 66, 80, 86, 80, 56, 88, 10, 0, 0, 0, 8, 0, 0, 0, 237, 2, 0, 231, 3, 0, 86, 80]
        XCTAssertTrue(webpSize(Data(vp8x))! == (750, 1000))
        var vp8l: [UInt8] = [82, 73, 70, 70, 0, 0, 0, 0, 87, 69, 66, 80, 86, 80, 56, 76, 0, 0, 0, 0, 0x2f]
        let bits = UInt32(99) | UInt32(49) << 14
        vp8l += [UInt8(bits & 0xff), UInt8(bits >> 8 & 0xff), UInt8(bits >> 16 & 0xff), UInt8(bits >> 24 & 0xff)] + [UInt8](repeating: 0, count: 7)
        XCTAssertTrue(webpSize(Data(vp8l))! == (100, 50))
        let vp8: [UInt8] = [82, 73, 70, 70, 0, 0, 0, 0, 87, 69, 66, 80, 86, 80, 56, 32, 0, 0, 0, 0, 0, 0, 0, 0x9d, 0x01, 0x2a, 0x80, 0x02, 0xe0, 0x01, 0, 0]
        XCTAssertTrue(webpSize(Data(vp8))! == (640, 480))
        XCTAssertNil(webpSize(Data([UInt8](repeating: 0, count: 32))))
        let metadata = try RasterMetadata.read(prefix: Data(vp8x), encodedBytes: 312_420)
        XCTAssertEqual(metadata.naturalSize, CGSize(width: 750, height: 1000))
    }

    func testHTTPImagesReuseFreshBytesRespectNoStoreAndRevalidate() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        try png(root, "image.png", width: 24, height: 32, identity: 4)
        let bytes = try Data(contentsOf: root.appendingPathComponent("image.png"))
        let server = try CachingRasterHTTP(body: bytes)
        defer { server.stop() }
        let resolver = AssetResolver(root: root)
        for path in ["fresh", "no-store", "revalidate"] {
            // Each open creates a new URLSession and drops the prior input.
            for _ in 0..<2 {
                let input = try RasterInput.open("http://127.0.0.1:\(server.port)/\(path)", resolver: resolver)
                XCTAssertEqual(try input.bytes(), bytes)
                XCTAssertEqual(try input.metadata().naturalSize, CGSize(width: 24, height: 32))
            }
        }
        XCTAssertEqual(server.requests.filter { $0.hasPrefix("GET /fresh ") }.count, 1)
        XCTAssertEqual(server.requests.filter { $0.hasPrefix("GET /no-store ") }.count, 2)
        let revalidated = server.requests.filter { $0.hasPrefix("GET /revalidate ") }
        XCTAssertEqual(revalidated.count, 2)
        XCTAssertTrue(revalidated.last?.lowercased().contains("if-none-match: \"raster-v1\"") == true)
    }
    /// A `data:` source opens as a page's `<img>` takes it, to its bound
    /// (LLP 1011 §2): base64, forgiving whitespace and escapes, or percent-encoded.
    func testDataURLsOpenWithinTheirBound() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        try png(root, "image.png", width: 24, height: 32, identity: 5)
        let bytes = try Data(contentsOf: root.appendingPathComponent("image.png"))
        let resolver = AssetResolver(root: root)
        let input = try RasterInput.open("data:image/png;base64,\(bytes.base64EncodedString(options: .lineLength64Characters))", resolver: resolver)
        XCTAssertEqual(try input.bytes(), bytes)
        XCTAssertEqual(try input.metadata().naturalSize, CGSize(width: 24, height: 32))
        XCTAssertEqual(RasterInput.dataURL("data:;base64,aGk%3D"), Data("hi".utf8))
        XCTAssertEqual(RasterInput.dataURL("data:image/svg+xml,%3Csvg%3E <"), Data("<svg> <".utf8))
        XCTAssertNil(RasterInput.dataURL("data:image/png;base64"))
        let over = "data:," + String(repeating: "a", count: RasterInput.dataLimit)
        XCTAssertThrowsError(try RasterInput.open(over, resolver: resolver))
    }
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
    /// Runs the main run loop until the event `predicate` names has happened,
    /// however long a loaded machine takes to decode; `hang` only stops a
    /// test whose event can never come.
    private func settle(file: StaticString = #file, line: UInt = #line, _ predicate: () -> Bool) {
        let hang = Date(timeIntervalSinceNow: 300)
        while !predicate() && Date() < hang { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
        XCTAssertTrue(predicate(), file: file, line: line)
    }
    private func fixture(sourceLimit: Int = 1152, metadataLimit: Int = 64) throws -> (URL, AssetResolver, Presenter, NodeView, RasterLoader, NSWindow) {
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
        // The core's minimum budget, which these pressures are sized for.
        return (root, AssetResolver(root: root), presenter, node,
                RasterLoader(budget: 32 * 1024 * 1024, sourceLimit: sourceLimit, metadataLimit: metadataLimit), window)
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
    func testMetadataAdmissionQueuesLateImagesAndEventuallyPaintsEveryOne() throws {
        let (root, resolver, presenter, first, loader, window) = try fixture()
        var nodes = [first]
        defer {
            loader.shutdown(); nodes.forEach { $0.raster = nil }; window.close()
            try? FileManager.default.removeItem(at: root)
        }
        // Hold inspection still so this deterministically crosses the 64-source
        // metadata cap. The remaining images must become bounded interests,
        // rather than one-shot refusals that need an unrelated prop update.
        loader.setPaused(true)
        for index in 0..<70 { try png(root, "burst-\(index).png", width: 32, height: 24, identity: index) }
        for index in 0..<70 {
            let node: NodeView
            if index == 0 { node = first }
            else {
                node = NodeView(id: UInt32(index + 1), kind: "image", presenter: presenter)
                nodes.append(node); presenter.views[node.id] = node; presenter.viewport.addSubview(node)
            }
            node.frame = NSRect(x: 0, y: 0, width: 32, height: 24)
            node.loadGeneration = 1
            XCTAssertTrue(loader.load(node, source: "burst-\(index).png", resolver: resolver))
        }
        XCTAssertEqual(loader.diagnostics["viewInterests"] as? Int, 64)
        XCTAssertEqual(loader.diagnostics["deferredAdmission"] as? Int, 6)
        XCTAssertEqual(loader.loadingOnScreen, 70)
        loader.setPaused(false)
        settle { nodes.allSatisfy { $0.raster != nil } }
        XCTAssertEqual(loader.diagnostics["deferredAdmission"] as? Int, 0)
        XCTAssertEqual(loader.diagnostics["decoded"] as? Int, 70)
    }
    func testReplacementAtSourceCapReclaimsColdIdentityWithoutLosingOldPixels() throws {
        let (root, resolver, presenter, first, loader, window) = try fixture(sourceLimit: 3, metadataLimit: 3)
        let second = NodeView(id: 2, kind: "image", presenter: presenter)
        presenter.views[2] = second; presenter.viewport.addSubview(second)
        second.frame = first.frame
        defer {
            loader.shutdown(); first.raster = nil; second.raster = nil; window.close()
            try? FileManager.default.removeItem(at: root)
        }
        for (index, name) in ["cold.png", "a.png", "b.png", "replacement.png"].enumerated() {
            try png(root, name, width: 32, height: 24, identity: index)
        }
        first.loadGeneration = 1
        loader.load(first, source: "cold.png", resolver: resolver)
        settle { first.raster != nil }
        let coldPixels = first.raster
        loader.cancel(first.id)
        first.loadGeneration += 1
        loader.load(first, source: "a.png", resolver: resolver)
        second.loadGeneration = 1
        loader.load(second, source: "b.png", resolver: resolver)
        settle { loader.diagnostics["decoded"] as? Int == 3 }
        let old = first.raster
        first.loadGeneration += 1
        XCTAssertTrue(loader.load(first, source: "replacement.png", resolver: resolver))
        XCTAssertTrue(first.raster === old, "replacement retains the painted lease while admission runs")
        settle { first.raster !== old }
        XCTAssertEqual(loader.diagnostics["deferredAdmission"] as? Int, 0)
        XCTAssertLessThanOrEqual(loader.diagnostics["sources"] as? Int ?? .max, 3)
        withExtendedLifetime(coldPixels) {}
    }
    func testDeferredAdmissionContinuesPastBusyAndSharesNewlyAdmittedSource() throws {
        let (root, resolver, presenter, first, loader, window) = try fixture(sourceLimit: 1, metadataLimit: 1)
        var nodes = [first]
        for id in 2...4 {
            let node = NodeView(id: UInt32(id), kind: "image", presenter: presenter)
            node.frame = first.frame; node.loadGeneration = 1
            presenter.views[node.id] = node; presenter.viewport.addSubview(node); nodes.append(node)
        }
        defer {
            loader.shutdown(); window.close(); try? FileManager.default.removeItem(at: root)
        }
        first.loadGeneration = 1
        loader.load(first, source: "occupied", resolver: resolver)
        loader.load(nodes[1], source: "shared", resolver: resolver)
        loader.load(nodes[2], source: "blocked", resolver: resolver)
        loader.load(nodes[3], source: "shared", resolver: resolver)
        XCTAssertEqual(loader.diagnostics["deferredAdmission"] as? Int, 3)
        loader.cancel(first.id)
        settle {
            loader.diagnostics["viewInterests"] as? Int == 2
                && loader.diagnostics["deferredAdmission"] as? Int == 1
        }
        loader.cancel(nodes[1].id); loader.cancel(nodes[3].id)
        settle { loader.diagnostics["deferredAdmission"] as? Int == 0 }
    }
    #if DEBUG
    func testTrimWakesADeferredAdmissionPass() throws {
        let (root, _, _, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root) }
        let before = loader.testReconciliations
        loader.trimCold()
        settle { loader.testReconciliations > before }
    }
    #endif
    func testReplacementKeepsBackingAndNaturalGeometryUntilMatchingAcceptance() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root) }
        try png(root, "a.png", width: 4000, height: 2000, identity: 1)
        try png(root, "b.png", width: 64, height: 64, identity: 2)
        try png(root, "c.png", width: 120, height: 80, identity: 3)
        var intrinsic = CGSize.zero
        presenter.onIntrinsic = { sizes in intrinsic = sizes.last?.1 ?? .zero }
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
        // A decoded pixel, whatever byte order the decoder chose (BGRA here):
        // each source's red differs, so each first pixel does.
        var seen = Set<UInt32>()
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
                seen.insert(UnsafeRawPointer(CFDataGetBytePtr(bytes)).loadUnaligned(as: UInt32.self))
                let s = loader.diagnostics
                XCTAssertTrue((s["peakBytes"] as? UInt64 ?? UInt64.max) <= 32 * 1024 * 1024)
                XCTAssertTrue((s["running"] as? UInt64 ?? UInt64.max) <= 2)
                XCTAssertTrue((s["deliveryCells"] as? UInt64 ?? UInt64.max) <= 2)
                XCTAssertTrue((s["pending"] as? UInt64 ?? UInt64.max) <= 64)
                XCTAssertTrue((s["subscribers"] as? UInt64 ?? UInt64.max) <= 1024)
                XCTAssertTrue((s["coldEntries"] as? UInt64 ?? UInt64.max) <= 256)
                XCTAssertTrue((s["sources"] as? Int ?? Int.max) <= RasterLoader.coldSources + 66)
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
        presenter.onIntrinsic = { sizes in
            node.frame = CGRect(origin: .zero, size: sizes.last?.1 ?? .zero); loader.resized(node)
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
            let hang = Date(timeIntervalSinceNow: 300)
            while (other.diagnostics["metadataReads"] as? Int ?? 0) == 0 && Date() < hang { Thread.sleep(forTimeInterval: 0.005) }
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
        // The image layer takes the new pixels at once (LLP 1100 D9).
        if let layer = node.imageLayer { XCTAssertTrue((layer.contents as AnyObject?) === node.raster?.image.image) }
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
    /// `cover` scales the image until both axes fill the box, so the decode
    /// is as long as the overflowing axis needs (a browser decodes for the
    /// size it paints), not the box's longest side.
    func testACoverImageDecodesForTheAxisThatOverflows() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        try png(root, "wide.png", width: 800, height: 100, identity: 11)
        node.frame = NSRect(x: 0, y: 0, width: 100, height: 50)
        node.style = ["object_fit": .string("cover")]
        node.loadGeneration = 1
        loader.load(node, source: "wide.png", resolver: resolver)
        settle { node.raster != nil }
        let scale = window.backingScaleFactor
        // Height fills at half the source's scale: 800 × 0.5, at the screen's scale.
        XCTAssertEqual(node.raster?.image.image.width, min(800, Int(400 * scale)))
    }
    /// Cold pixels are evicted before a decode waits, so pixels no view
    /// holds never lower the resolution a visible image decodes at.
    func testColdPixelsDoNotLowerAResolution() throws {
        let (root, resolver, presenter, node, loader, window) = try fixture()
        defer { loader.shutdown(); node.raster = nil; window.close(); try? FileManager.default.removeItem(at: root); withExtendedLifetime(presenter) {} }
        let scale = window.backingScaleFactor
        node.frame = NSRect(x: 0, y: 0, width: 1000 / scale, height: 1000 / scale)
        for i in 0..<6 {
            try autoreleasepool { try png(root, "cold-\(i).png", width: 1000, height: 1000, identity: 20 + i) }
            let before = node.raster
            node.loadGeneration += 1
            loader.load(node, source: "cold-\(i).png", resolver: resolver)
            settle { node.raster != nil && node.raster !== before }
            XCTAssertEqual(node.raster?.image.image.width, 1000, "image \(i)")
        }
        XCTAssertTrue((loader.diagnostics["coldBytes"] as? UInt64 ?? 0) > 0)
    }
}

/// Continuously replenished metadata-ready demands compete at the real worker
/// admission boundary. They have no native backend and fail after admission;
/// this tests metadata/decode turn fairness, not decoder throughput.
private final class RasterAdmissionFlood: @unchecked Sendable {
    private let session = exact_raster_session_create(0)
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
                        d.encoded_bytes = 4; d.header_bytes = 4; d.stride = 4; d.variant = RasterVariant.own8
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

/// Real Foundation cache behavior across separate sessions, using only loopback.
private final class CachingRasterHTTP: @unchecked Sendable {
    private let listener: Int32
    private let lock = NSLock()
    private var seen: [String] = []
    private let body: Data
    let port: UInt16
    var requests: [String] { lock.lock(); defer { lock.unlock() }; return seen }
    init(body: Data) throws {
        self.body = body
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        listener = fd
        guard fd >= 0 else { throw NSError(domain: "socket", code: 1) }
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        let bound = withUnsafePointer(to: &address) { p in
            p.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
        }
        guard bound == 0, listen(fd, 8) == 0 else { close(fd); throw NSError(domain: "listen", code: 1) }
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &address) { p in
            p.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) }
        }
        port = UInt16(bigEndian: address.sin_port)
        DispatchQueue.global().async { [self] in
            while true {
                let client = accept(listener, nil, nil)
                if client < 0 { break }
                respond(client); close(client)
            }
        }
    }
    private func respond(_ client: Int32) {
        var timeout = timeval(tv_sec: 2, tv_usec: 0), noPipe: Int32 = 1
        setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &noPipe, socklen_t(MemoryLayout<Int32>.size))
        var buffer = [UInt8](repeating: 0, count: 4096), request = Data()
        while request.count < 8192 {
            let count = recv(client, &buffer, buffer.count, 0)
            if count <= 0 { return }
            request.append(contentsOf: buffer.prefix(count))
            if request.range(of: Data("\r\n\r\n".utf8)) != nil { break }
        }
        let text = String(decoding: request, as: UTF8.self)
        lock.lock(); seen.append(text); lock.unlock()
        let validate = text.hasPrefix("GET /revalidate ")
        let notModified = validate && text.lowercased().contains("if-none-match: \"raster-v1\"")
        let policy = text.hasPrefix("GET /no-store ") ? "no-store" : validate ? "max-age=0, must-revalidate" : "public, max-age=3600"
        let date = DateFormatter(); date.locale = Locale(identifier: "en_US_POSIX")
        date.timeZone = TimeZone(secondsFromGMT: 0); date.dateFormat = "EEE, dd MMM yyyy HH:mm:ss 'GMT'"
        let payload = notModified ? Data() : body
        let header = "HTTP/1.1 \(notModified ? "304 Not Modified" : "200 OK")\r\nDate: \(date.string(from: Date()))\r\nCache-Control: \(policy)\r\nETag: \"raster-v1\"\r\nContent-Type: image/png\r\nContent-Length: \(payload.count)\r\nConnection: close\r\n\r\n"
        let response = Data(header.utf8) + payload
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

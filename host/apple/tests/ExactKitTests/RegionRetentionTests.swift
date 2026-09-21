import Foundation
import CoreGraphics
#if os(macOS)
import IOSurface
#endif
import XCTest
@testable import ExactKit

@MainActor final class RegionRetentionTests: XCTestCase {
    private func request(size: CGSize = CGSize(width: 160,height: 120), scroll: CGFloat = 80.375,
                         scale: Int = 1, generation: Int = 2, publication: UInt64 = 7,
                         background: [CGFloat] = [1,1,1,1], selected: Bool = false,
                         format: UInt32 = CGImageAlphaInfo.premultipliedLast.rawValue,
                         profile: NativeProfile? = nil) -> RegionRasterRequest {
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let p = profile ?? NativeProfile.capture(original: space,data: space.copyICCData(),account: NativeProfileAccount()).owner!
        var row = RegionPaintRow(artifact: 19,box: CGRect(x: 0,y: 0,width: 160,height: 1000))
        if selected { row.selection = NSRange(location: 1,length: 3) }
        return RegionRasterRequest(serial: 1,publication: publication,generation: generation,rows: [row],
            scroll: CGPoint(x: 0,y: scroll),size: size,scale: scale,profile: p,
            format: format,background: background,selectionColor: [0,0,1,0.45])
    }
    private func mapping(_ a: RegionRasterRequest, _ b: RegionRasterRequest,
                         actual: CGPoint? = nil, extent: CGSize = CGSize(width: 160,height: 1000),
                         clip: CGRect? = nil) -> RegionRetainedWitness? {
        RegionRetainedWitness(accepted: a,current: b,actualScroll: actual ?? b.scroll,extent: extent,
            clip: clip ?? CGRect(origin: .zero,size: b.size))
    }
    func testIntegerScrollRetainsOriginalFractionalPixelsAtBothScales() throws {
        for scale in [1,2] {
            let a = request(scale: scale)
            for delta: CGFloat in [-40,40] {
                let b = request(size: CGSize(width: 200,height: 140),scroll: a.scroll.y + delta,scale: scale,profile: a.profile)
                let retained = try XCTUnwrap(mapping(a,b))
                XCTAssertEqual(retained.publication,a.publication)
                XCTAssertEqual(retained.imageFrame,CGRect(x: 0,y: -delta,width: 160,height: 120))
                XCTAssertEqual(retained.coverage,CGRect(x: 0,y: delta > 0 ? 0 : 40,width: 160,height: delta > 0 ? 80 : 100))
                XCTAssertFalse(retained.coversViewport)
                XCTAssertFalse(a.samePixels(as: b),"retained display must not weaken exact current acceptance")
                XCTAssertFalse(a.sameOutput(as: b))
                let point = CGPoint(x: 12,y: retained.coverage.midY)
                XCTAssertEqual(point.y - retained.imageFrame.minY + a.scroll.y,point.y + b.scroll.y)
            }
        }
    }
    func testShrinkClipEmptyIntersectionAndReverseRecovery() throws {
        let a = request()
        let small = request(size: CGSize(width: 80,height: 60),profile: a.profile)
        XCTAssertTrue(try XCTUnwrap(mapping(a,small)).coversViewport)
        XCTAssertEqual(try XCTUnwrap(mapping(a,small,clip: CGRect(x: 10,y: 10,width: 50,height: 40))).coverage,
                       CGRect(x: 10,y: 10,width: 50,height: 40))
        XCTAssertFalse(try XCTUnwrap(mapping(a,small,clip: CGRect(x: 10,y: 10,width: 50,height: 40))).coversViewport)
        let away = request(scroll: 280.375,profile: a.profile)
        XCTAssertNil(mapping(a,away))
        let back = request(scroll: 120.375,profile: a.profile)
        XCTAssertEqual(try XCTUnwrap(mapping(a,back)).coverage.height,80)
        XCTAssertTrue(try XCTUnwrap(mapping(a,a)).coversViewport,"same A can cover again without history")
    }
    func testNoEpsilonNoScrollGuessAndOnlyAcceptedExtent() {
        for scale in [1,2] {
            let a = request(scale: scale)
            let mismatch = request(scroll: a.scroll.y + 0.25,scale: scale,profile: a.profile)
            XCTAssertNil(mapping(a,mismatch))
            let almost = request(scroll: (a.scroll.y + 40).nextUp,scale: scale,profile: a.profile)
            XCTAssertNil(mapping(a,almost),"must not snap an almost integral displacement")
            XCTAssertNil(mapping(a,a,actual: CGPoint(x: 0,y: a.scroll.y + 1)))
            XCTAssertNil(mapping(a,a,extent: CGSize(width: 160,height: 130)),"A extent cannot be replaced by B's larger extent")
            XCTAssertNil(mapping(a,a,clip: CGRect(x: 0.125,y: 0,width: 100,height: 100)))
            let negative = request(scroll: -0.625,scale: scale,profile: a.profile)
            XCTAssertNil(mapping(a,negative),"rubber-band phase is not admitted")
            let overlarge = request(size: CGSize(width: 16385,height: 120),scale: scale,profile: a.profile)
            XCTAssertNil(mapping(a,overlarge))
            let zero = request(size: .zero,scale: scale,profile: a.profile)
            XCTAssertNil(mapping(a,zero))
        }
        let a = request(scroll: 880), end = request(size: CGSize(width: 160,height: 160),scroll: 840,profile: a.profile)
        XCTAssertNotNil(mapping(a,end),"actual end clamp preserves integral phase")
        XCTAssertNil(mapping(a,end,actual: a.scroll),"unresolved old scroll is not the clamped current offset")
    }
    func testPaletteProfileGenerationSourceAndSelectionMustStillMatch() {
        let a = request()
        let p3 = CGColorSpace(name: CGColorSpace.displayP3)!
        let other = NativeProfile.capture(original: p3,data: p3.copyICCData(),account: NativeProfileAccount()).owner!
        for b in [request(generation: 3,profile: a.profile), request(publication: 8,profile: a.profile),
                  request(background: [0,0,0,1],profile: a.profile), request(selected: true,profile: a.profile),
                  request(profile: other), request(scale: 2,profile: a.profile)] {
            XCTAssertNil(mapping(a,b))
        }
        let selection = request(selected: true,profile: a.profile)
        XCTAssertTrue(a.sameInkAndGeometry(as: selection),"ordinary selection-only anchor contract stays unchanged")
        XCTAssertFalse(a.samePixels(as: selection))
    }
    func testReplacementCannotUseOldVisibleWitnessOrRelaxExactPixelIdentity() {
        let a = request()
        let b = request(size: CGSize(width: 200,height: 140),publication: 8,profile: a.profile)
        let old = RegionVisibleWitness(publication: a.publication,size: a.size,scroll: a.scroll,
            profile: a.profile.bytes,scale: a.scale)
        XCTAssertFalse(old.matches(publication: b.publication,size: b.size,scroll: b.scroll,
            profile: b.profile.bytes,scale: b.scale))
        XCTAssertNil(mapping(a,b),"a replacement publication is not retained-A geometry")
        XCTAssertTrue(b.samePixels(as: b),"already accepted B can qualify without a new raster request")
        for stale in [a, request(size: b.size,publication: 8,selected: true,profile: b.profile),
                      request(size: b.size,generation: 3,publication: 8,profile: b.profile),
                      request(size: b.size,publication: 8,background: [0,0,0,1],profile: b.profile)] {
            XCTAssertFalse(b.samePixels(as: stale))
        }
        let fresh = RegionVisibleWitness(publication: b.publication,size: b.size,scroll: b.scroll,
            profile: b.profile.bytes,scale: b.scale)
        XCTAssertTrue(fresh.matches(publication: b.publication,size: b.size,scroll: b.scroll,
            profile: b.profile.bytes,scale: b.scale))
    }
    #if os(macOS)
    func testOrdinaryBatchesStillAllowRegionRegistrationRefusalAndRetirement() {
        let session = ExactApp.shared.makeSession(label: "region-registration")
        defer { session.destroy() }
        let controller = session.regions
        func prepare(_ ops: [[String: Any]], error: String? = nil) {
            controller.prepare(Batch(ops: ops, timers: false, motion: false, clock: nil, error: error))
        }
        func registration(_ incarnation: String) -> [String: Any] {
            ["op": "region", "incarnation": incarnation, "owner": UInt32(1),
             "content": UInt32(2), "pending": UInt32(3), "request": "0", "source": "1",
             "publication": "0", "current": true, "frames": [[String: Any]](),
             "members": [UInt32(1), UInt32(2), UInt32(3)]]
        }
        let ordinary: [[String: Any]] = [["op": "props", "id": UInt32(99), "set": ["text": "outside"]]]
        let disabled: [String: Any] = ["op": "region", "disabled": "ordinary layout"]
        for error: String? in [nil, "unrelated refusal"] {
            prepare(ordinary, error: error)
            XCTAssertEqual(controller.diagnostics["registered"] as? Bool, false)
            XCTAssertNil(controller.failure)
        }
        prepare(ordinary + [["op": "region"]])
        XCTAssertEqual(controller.failure, "invalid region wire", "the first malformed region must be inspected")
        controller.reset()
        prepare(ordinary + [registration("1")])
        XCTAssertEqual(controller.diagnostics["registered"] as? Bool, true)
        XCTAssertEqual(controller.currentSourcePublication, 0)
        XCTAssertEqual(controller.currentGeneration, session.generation)
        prepare(ordinary)
        XCTAssertEqual(controller.currentSourcePublication, 0, "ordinary batches retain the active region")
        prepare([registration("2"), disabled])
        XCTAssertEqual(controller.diagnostics["registered"] as? Bool, false)
        XCTAssertNil(controller.currentSourcePublication)
        XCTAssertEqual(controller.currentGeneration, -1)
        prepare(ordinary)
        prepare([disabled] + ordinary + [registration("3")])
        XCTAssertEqual(controller.diagnostics["registered"] as? Bool, true, "region operations retain their order")
        XCTAssertEqual(controller.currentGeneration, session.generation)
        XCTAssertNil(controller.failure)
        prepare([["op": "region"]])
        XCTAssertEqual(controller.failure, "invalid region wire", "active-region refusals still run")
        prepare([disabled])
        XCTAssertEqual(controller.diagnostics["registered"] as? Bool, false)
        XCTAssertNil(controller.failure)
    }

    func testPreApplyProtectsOldNewMembersAndAncestorsButAllowsGeometryAndSiblingTyping() {
        let protected: Set<UInt32> = [1,2,3,4,5]
        let neutral: [[String: Any]] = [
            ["op":"frame","id":2,"w":200,"h":100], ["op":"content","id":3,"h":900],
            ["op":"props","id":UInt32(99),"set":["text":"outside typing"]],
            ["op":"children","id":UInt32(99),"ids":[UInt32(100)]]]
        XCTAssertFalse(regionInvalidationFixture(ops: neutral,protected: protected))
        for kind in ["props","style","destroy","create","present","surface"] {
            for id: UInt32 in [1,3,5] {
                XCTAssertTrue(regionInvalidationFixture(ops: [["op":kind,"id":id]],protected: protected))
            }
        }
        XCTAssertTrue(regionInvalidationFixture(ops: [["op":"children","id":UInt32(99),"ids":[UInt32(3)]]],protected: protected),"reparent into a previously unrelated ancestor")
        XCTAssertTrue(regionInvalidationFixture(ops: [["op":"roots","ids":[UInt32(1)]]],protected: protected))
        XCTAssertTrue(regionInvalidationFixture(ops: [["op":"unknown"]],protected: protected))
        XCTAssertTrue(regionInvalidationFixture(ops: [["op":"style"]],protected: protected))
    }
    func testSourceOrContextABACannotRearmWithoutFreshPixels() {
        var state = RegionRetentionValidity()
        state.acceptedPixels()
        XCTAssertTrue(state.valid)
        state.invalidate()
        XCTAssertFalse(state.valid)
        // Returning to an identical source/context or receiving neutral geometry
        // does not call acceptedPixels. Invalidation is sticky until publication.
        let protected: Set<UInt32> = [1]
        let aba: [[String: Any]] = [["op":"props","id":UInt32(1),"set":["text":"B"]],
                                   ["op":"props","id":UInt32(1),"set":["text":"A"]]]
        if regionInvalidationFixture(ops: aba,protected: protected) { state.invalidate() }
        XCTAssertFalse(state.valid)
        XCTAssertFalse(regionInvalidationFixture(ops: [["op":"frame","id":UInt32(1)]],protected: protected))
        XCTAssertFalse(state.valid)
        state.acceptedPixels()
        XCTAssertTrue(state.valid)
    }
    #endif
    func testTranslatedImageCropBytesAndUncoveredBackgroundAreExact() throws {
        for scale in [1,2] {
            let a = request(size: CGSize(width: 8,height: 8),scroll: 40.375,scale: scale)
            let width = a.width, height = a.height, space = a.profile.makeSpace()!
            // Asymmetric opaque pixels discriminate flip, stretch and direction.
            var bytes = [UInt8](repeating: 0,count: width*height*4)
            for y in 0..<height { for x in 0..<width {
                let i = (y*width+x)*4
                bytes[i] = UInt8(x*7+3); bytes[i+1] = UInt8(y*5+1); bytes[i+2] = 91; bytes[i+3] = 255
            } }
            let provider = CGDataProvider(data: Data(bytes) as CFData)!
            let image = CGImage(width: width,height: height,bitsPerComponent: 8,bitsPerPixel: 32,
                bytesPerRow: width*4,space: space,bitmapInfo: CGBitmapInfo(rawValue: a.format),
                provider: provider,decode: nil,shouldInterpolate: false,intent: .defaultIntent)!
            for delta: CGFloat in [-2,2] {
                let b = request(size: CGSize(width: 10,height: 10),scroll: a.scroll.y+delta,scale: scale,profile: a.profile)
                let placement = try XCTUnwrap(mapping(a,b))
                let ctx = CGContext(data: nil,width: b.width,height: b.height,bitsPerComponent: 8,
                    bytesPerRow: b.width*4,space: space,bitmapInfo: a.format)!
                ctx.setFillColor(CGColor(colorSpace: space,components: b.background)!)
                ctx.fill(CGRect(x: 0,y: 0,width: b.width,height: b.height))
                // Quartz bottom-up destination; image rows and witness are top-down.
                let f = placement.imageFrame, q = CGFloat(scale)
                ctx.interpolationQuality = .none
                ctx.draw(image,in: CGRect(x: f.minX*q,y: CGFloat(b.height)-f.maxY*q,width: f.width*q,height: f.height*q))
                ctx.flush()
                let actual = Array(UnsafeBufferPointer(start: ctx.data!.assumingMemoryBound(to: UInt8.self),count: b.bytes!))
                var expected = [UInt8](repeating: 255,count: b.bytes!)
                let dx = Int(f.minX*q), dy = Int(f.minY*q)
                for y in 0..<b.height { for x in 0..<b.width {
                    let sx = x-dx, sy = y-dy
                    if sx >= 0 && sx < width && sy >= 0 && sy < height {
                        for c in 0..<4 { expected[(y*b.width+x)*4+c] = bytes[(sy*width+sx)*4+c] }
                    }
                } }
                XCTAssertEqual(actual,expected,"exact original pixels plus background, scale=\(scale), delta=\(delta)")
                XCTAssertTrue(image.dataProvider === provider)
            }
        }
    }
    #if os(macOS)
    func testCompositedSurfaceKeepsItsChargeAfterTheReceiptAndProviderDie() throws {
        let a = request(size: CGSize(width: 397, height: 83), format: RegionRasterRequest.compositedFormat)
        let account = RegionPixelAccount(), box = RetainedPixelsBox(), done = DispatchSemaphore(value: 0)
        DispatchQueue(label: "region-surface-owner-test").async {
            do {
                let charge = account.reserve(a.bytes!)!
                box.pixels = try RegionPixels(count: a.bytes!, charge: charge, profile: a.profile, request: a) { p in
                    p.initializeMemory(as: UInt8.self, repeating: 42, count: a.bytes!)
                    return true
                }
            } catch { box.error = String(describing: error) }
            done.signal()
        }
        done.wait()
        XCTAssertEqual(box.error, "")
        let observed = RetainedPixelsWeak(box.pixels)
        var surface: IOSurface?
        try autoreleasepool {
            let pixels = try XCTUnwrap(box.pixels)
            surface = try XCTUnwrap(pixels.surface)
            let provider = try XCTUnwrap(pixels.provider())
            XCTAssertEqual(surface?.bytesPerRow, a.bytesPerRow)
            XCTAssertEqual(account.stats.bytes, surface?.allocationSize)
            XCTAssertEqual((provider.data! as Data).first, 42)
            box.pixels = nil
        }
        XCTAssertNil(observed.value, "the layer's surface outlives the pixel receipt and image provider")
        XCTAssertEqual(account.stats.owners, 1)
        XCTAssertEqual(account.stats.bytes, a.bytes)
        surface = nil
        XCTAssertEqual(account.stats.owners, 0)
        XCTAssertEqual(account.stats.bytes, 0)
        withExtendedLifetime(surface) {}
    }
    #endif
    func testProviderAliasesKeepAChargeWhileTranslationsAllocateNoNewPixels() throws {
        let a = request(size: CGSize(width: 8,height: 8),scroll: 40.375)
        let account = RegionPixelAccount(), box = RetainedPixelsBox(), done = DispatchSemaphore(value: 0)
        DispatchQueue(label: "retained-pixel-owner-test").async {
            do {
                let charge = account.reserve(a.bytes!)!
                box.pixels = try RegionPixels(count: a.bytes!,charge: charge,profile: a.profile) { p in
                    let bytes = p.assumingMemoryBound(to: UInt8.self)
                    for i in 0..<a.bytes! { bytes[i] = UInt8(i % 251) }
                    return true
                }
            } catch { box.error = String(describing: error) }
            done.signal()
        }
        done.wait()
        XCTAssertEqual(box.error,"")
        let observedPixels = RetainedPixelsWeak(box.pixels)
        var second: RegionPixelCharge?
        try autoreleasepool {
            var provider: CGDataProvider? = try XCTUnwrap(box.pixels).provider()
            let original = try XCTUnwrap(provider?.data) as Data
            var alias = provider
            box.pixels = nil
            let b = request(size: CGSize(width: 10,height: 10),scroll: 42.375,profile: a.profile)
            for _ in 0..<20 { XCTAssertNotNil(mapping(a,b)) }
            XCTAssertEqual(account.stats.owners,1)
            XCTAssertEqual(try XCTUnwrap(alias?.data) as Data,original)
            second = account.reserve(a.bytes!)
            XCTAssertNotNil(second)
            XCTAssertNil(account.reserve(a.bytes!),"A+B only, not a retained-image history")
            provider = nil
            XCTAssertEqual(account.stats.owners,2,"remaining provider/data aliases still own A")
            XCTAssertNotNil(observedPixels.value)
            alias = nil
            // CGDataProvider.data and its bridged Data may themselves keep A
            // alive. The actual last alias dies at this autoreleasepool boundary.
            withExtendedLifetime([provider,alias]) {}
        }
        XCTAssertNil(observedPixels.value,"actual RegionPixels owner, not just ledger counters")
        XCTAssertEqual(account.stats.owners,1)
        XCTAssertEqual(account.stats.drops,1)
        second = nil
        XCTAssertEqual(account.stats.owners,0)
        XCTAssertEqual(account.stats.drops,2)
        withExtendedLifetime(second) {}
    }
}
private final class RetainedPixelsBox: @unchecked Sendable {
    var pixels: RegionPixels?
    var error = ""
}

private final class RetainedPixelsWeak {
    weak var value: RegionPixels?
    init(_ value: RegionPixels?) { self.value = value }
}

import Foundation
import CoreGraphics
import XCTest
@testable import ExactKit

@MainActor final class RegionReuseTests: XCTestCase {
    func testDifferentWidthsPrepareOnceWhileLayoutsAndHeightRequestIDsStayFresh() {
        let h = RegionReuseHarness(), s = source()
        defer { h.service.close() }
        #if REGION_PREPARATION_SENTINEL
        let before = RegionPreparationSentinel.count
        #endif
        let a = h.shape(1, source: s, width: 320, height: -2)
        let b = h.shape(2, source: s, width: 270, height: -1)
        let c = h.shape(3, source: s, width: 270, height: 400)
        XCTAssertEqual(h.admissions.count, 3)
        XCTAssertEqual(h.constructions.count, 2)
        XCTAssertEqual([a.id,b.id,c.id], [1,2,3])
        XCTAssertFalse(a.metadata === b.metadata)
        XCTAssertTrue(b.metadata === c.metadata)
        XCTAssertEqual([a.metadata.offeredWidth,b.metadata.offeredWidth,c.metadata.offeredWidth], [320,270,270])
        XCTAssertEqual(b.metadata.copy(NSRange(location: 0,length: s.utf16Count)), s.text)
        #if REGION_PREPARATION_SENTINEL
        XCTAssertEqual(RegionPreparationSentinel.count - before, 1, "actual attributed/typesetter construction, not admission or equal metrics")
        #endif
        withExtendedLifetime([a,b,c]) {}
    }

    func testPreparationUsesExactSourceIdentityAndExistingLiveBindingsOnly() {
        let h = RegionReuseHarness(), s = source(), equal = source()
        defer { h.service.close() }
        #if REGION_PREPARATION_SENTINEL
        let before = RegionPreparationSentinel.count
        #endif
        let a = h.shape(1, source: s)
        let b = h.shape(2, source: s, width: 270)
        let otherObject = h.shape(3, source: equal, width: 270)
        let otherID = h.shape(4, source: s, sourceID: 8, width: 270)
        let otherGeneration = h.shape(5, source: s, width: 270, generation: 2)
        let otherPalette = h.shape(6, source: source(color: [80,20,110,255]), width: 270)
        let otherFont = h.shape(7, source: source(size: 23), width: 270)
        XCTAssertEqual(s.sourceSHA256, equal.sourceSHA256)
        XCTAssertEqual(h.constructions.count, 7)
        XCTAssertFalse(a.metadata === b.metadata)
        for value in [otherObject,otherID,otherGeneration,otherPalette,otherFont] {
            XCTAssertFalse(value.metadata === b.metadata)
        }
        #if REGION_PREPARATION_SENTINEL
        XCTAssertEqual(RegionPreparationSentinel.count - before, 6)
        #endif
        withExtendedLifetime([a,b,otherObject,otherID,otherGeneration,otherPalette,otherFont]) {}
    }

    func testPreparedOwnerDropsWithLastLayoutAndNeverCrossesTheWorker() {
        // This default-suite test checks the real owner without sentinel flags.
        let s = source(), done = DispatchSemaphore(value: 0), result = PreparedLifetimeEvidence()
        DispatchQueue(label: "prepared-lifetime-test").async {
            autoreleasepool {
                var a: RegionWorkerLayout? = RegionWorkerLayout.shape(s,width: 320)
                weak var owner = a?.preparation
                defer { owner = nil }
                var b: RegionWorkerLayout? = RegionWorkerLayout.shape(s,width: 270,preparation: a!.preparation)
                result.shared = a!.preparation === b!.preparation
                result.sourceMatched = b!.preparation.source === s
                a = nil
                result.survivesFirstDrop = owner != nil
                b = nil
                result.dropsLast = owner == nil
                result.worker = !Thread.isMainThread
            }
            done.signal()
        }
        done.wait()
        XCTAssertTrue(result.shared)
        XCTAssertTrue(result.sourceMatched)
        XCTAssertTrue(result.survivesFirstDrop)
        XCTAssertTrue(result.dropsLast)
        XCTAssertTrue(result.worker)
    }
    func testSharedPreparationMatchesFreshWidthFontClampAndSelectionPixels() {
        for (size, clamp) in [(CGFloat(16),0),(CGFloat(23),2)] {
            autoreleasepool {
                let s = source(size: size, clamp: clamp), h = RegionReuseHarness(), fresh = RegionReuseHarness()
                defer { h.service.close(); fresh.service.close() }
                let a = h.shape(1,source: s,width: 320)
                let b = h.shape(2,source: s,width: 270)
                let reference = fresh.shape(3,source: s,width: 270)
                let x = b.metadata, y = reference.metadata
                XCTAssertEqual(x.width,y.width); XCTAssertEqual(x.height,y.height)
                XCTAssertEqual(x.baselines,y.baselines); XCTAssertEqual(x.lineBottoms,y.lineBottoms)
                XCTAssertEqual(x.lines.count,y.lines.count)
                for (l,r) in zip(x.lines,y.lines) {
                    XCTAssertEqual(l.range,r.range); XCTAssertEqual(l.ink,r.ink)
                    XCTAssertEqual(l.ascent,r.ascent); XCTAssertEqual(l.descent,r.descent)
                    XCTAssertEqual(l.leading,r.leading); XCTAssertEqual(l.typographicWidth,r.typographicWidth)
                    XCTAssertEqual(l.flushOffset,r.flushOffset)
                }
                let profile = h.profile(), selection = NSRange(location: 2,length: 15)
                let pixels = h.raster(1,publication: 1,artifact: b,profile: profile,selection: selection)
                let original = fresh.raster(1,publication: 1,artifact: reference,profile: profile,selection: selection)
                XCTAssertEqual(pixels.image()!.dataProvider!.data! as Data,original.image()!.dataProvider!.data! as Data)
                let box = CGRect(x: 0,y: 0,width: 270,height: x.height)
                for yy in [CGFloat(0),x.firstBaseline,x.height] {
                    for xx in [CGFloat(-2),0,17.25,269,290] {
                        let point = CGPoint(x: xx,y: yy)
                        let li = x.cachedIndex(at: point,in: box,artifact: b.id,hits: pixels.hits)
                        let ri = y.cachedIndex(at: point,in: box,artifact: reference.id,hits: original.hits)
                        XCTAssertEqual(li,ri)
                        if let li, let ri { XCTAssertEqual(x.link(at: point,in: box,exactIndex: li),y.link(at: point,in: box,exactIndex: ri)) }
                    }
                }
                XCTAssertEqual(x.copy(NSRange(location: 0,length: s.utf16Count)),s.text)
                withExtendedLifetime(a) {}
            }
        }
    }

    func testCloseDropsPreparedSourceAndFontsWhilePixelAliasSurvives() {
        let h = RegionReuseHarness()
        weak var captured: RegionTextSource?
        weak var font: RegionFontOwner?
        defer { captured = nil; font = nil; h.service.close() }
        var pixels: RegionRaster?
        autoreleasepool {
            let s = source()
            captured = s; font = s.runs[0].font.owner
            let a = h.shape(1,source: s,width: 320)
            let b = h.shape(2,source: s,width: 270)
            pixels = h.raster(1,publication: 1,artifact: b,profile: h.profile())
            h.service.close()
            withExtendedLifetime([a,b]) {}
        }
        let deadline = Date().addingTimeInterval(3)
        while captured != nil && Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.001))
        }
        XCTAssertNil(captured)
        XCTAssertNil(font)
        autoreleasepool {
            XCTAssertNotNil(pixels?.image(), "pixel aliases must not own source/font/layout preparation")
        }
        pixels = nil
        XCTAssertEqual(h.service.pixels.stats.owners,0)
    }

    private func source(_ text: String = "ffi e\u{301} אבג 👩🏽‍🚀 wrapped link words ", color: [Double] = [20,40,60,255], size: CGFloat = 16, clamp: Int = 0) -> RegionTextSource {
        let run = Run(text: String(repeating: text, count: 4), size: size, weight: 400, family: 0,
                      italic: true, lineHeight: 26.25, letterSpacing: 0,
                      decoration: "underline", href: "https://example.invalid/reuse")
        return RegionTextSource.capture(Spec(runs: [run], align: 0, lineClamp: clamp, color: color, strut: run),
                                        engine: TextEngine(resolve: { _ in nil }))
    }
    func testThreeExactHeightRequestsShareOneConstructionButKeepFreshIdentities() {
        let h = RegionReuseHarness(), s = source()
        defer { h.service.close() }
        let a = h.shape(1, source: s, height: -2)
        let b = h.shape(2, source: s, height: -1)
        let c = h.shape(3, source: s, height: 400)
        XCTAssertEqual(h.admissions.count, 3, "beforeShape remains an admission/barrier hook")
        XCTAssertEqual(h.constructions.count, 1, "actual RegionWorkerLayout.shape calls, not just equal answers")
        XCTAssertEqual([a.id,b.id,c.id], [1,2,3])
        XCTAssertEqual([a.sourceID,b.sourceID,c.sourceID], [7,7,7])
        XCTAssertEqual([a.generation,b.generation,c.generation], [1,1,1])
        XCTAssertTrue(a !== b && b !== c)
        XCTAssertTrue(a.metadata === b.metadata && b.metadata === c.metadata)
        XCTAssertTrue(a.metadata.source === s)
        XCTAssertEqual(a.metadata.copy(NSRange(location: 0,length: s.utf16Count)), s.text)
        XCTAssertFalse(a.metadata.shapedOnMainThread)
        XCTAssertEqual(a.metadata.offeredWidth, 320)
        XCTAssertEqual(a.metadata.baselines, c.metadata.baselines)
        XCTAssertEqual(a.metadata.lineBottoms, c.metadata.lineBottoms)
        withExtendedLifetime([a,b,c]) {}
    }
    func testSourceObjectSourceIDGenerationAndExactWidthAreIndependentMissKeys() {
        let h = RegionReuseHarness(), s = source(), equalBytes = source()
        defer { h.service.close() }
        let a = h.shape(1, source: s)
        let differentObject = h.shape(2, source: equalBytes)
        let differentID = h.shape(3, source: s, sourceID: 8)
        let differentGeneration = h.shape(4, source: s, generation: 2)
        let differentWidth = h.shape(5, source: s, width: 320.0001)
        XCTAssertEqual(s.sourceSHA256, equalBytes.sourceSHA256)
        XCTAssertEqual(h.constructions.count, 5)
        for other in [differentObject,differentID,differentGeneration,differentWidth] {
            XCTAssertFalse(other.metadata === a.metadata)
        }
        let changedPaint = h.shape(6, source: source(color: [90,80,70,255]))
        XCTAssertFalse(changedPaint.metadata === a.metadata)
        XCTAssertEqual(h.constructions.count, 6)
        withExtendedLifetime([a,differentObject,differentID,differentGeneration,differentWidth,changedPaint]) {}
    }
    func testIntrinsicOffersNeverReuseButDefiniteZeroDoes() {
        #if REGION_PREPARATION_SENTINEL
        let preparationStart = RegionPreparationSentinel.count
        #endif
        let h = RegionReuseHarness(), s = source()
        defer { h.service.close() }
        var retained: [RegionArtifact] = []
        for (i,w) in [CGFloat(-2),-2,-1,-1].enumerated() {
            retained.append(h.shape(UInt64(i+1), source: s, width: w))
        }
        XCTAssertEqual(h.constructions.count, 4)
        XCTAssertTrue(retained.allSatisfy { $0.metadata.lines.isEmpty })
        retained.append(h.shape(5, source: s, width: 0, height: -2))
        retained.append(h.shape(6, source: s, width: 0, height: -1))
        XCTAssertEqual(h.constructions.count, 5)
        XCTAssertTrue(retained[4].metadata === retained[5].metadata)
        XCTAssertEqual(retained[5].metadata.offeredWidth, 0)
        XCTAssertFalse(retained[5].metadata.lines.isEmpty)
        withExtendedLifetime(retained) {}
        #if REGION_PREPARATION_SENTINEL
        XCTAssertEqual(RegionPreparationSentinel.count - preparationStart, 5)
        #endif
    }
    func testRetiringOldAliasStillRastersFreshIDWithExactPixelsAndHits() {
        let h = RegionReuseHarness(), s = source()
        defer { h.service.close() }
        var a: RegionArtifact? = h.shape(1, source: s, height: -2)
        let b = h.shape(2, source: s, height: -1)
        weak var metadata = a?.metadata
        defer { metadata = nil }
        XCTAssertTrue(metadata === b.metadata)
        let profile = h.profile()
        let before = h.raster(1, publication: 1, artifact: a!, profile: profile)
        let beforeImage = before.image()!
        let expected = beforeImage.dataProvider!.data! as Data
        a = nil // actual artifact deinit enqueues retirement of ID1
        let after = h.raster(2, publication: 2, artifact: b, profile: profile)
        let afterImage = after.image()!
        XCTAssertEqual(after.request.rows.first?.artifact, 2)
        XCTAssertEqual(afterImage.dataProvider!.data! as Data, expected)
        XCTAssertNotNil(metadata)
        XCTAssertEqual(h.constructions.count, 1)
        let box = CGRect(x: 0,y: 0,width: 320,height: b.metadata.height)
        let point = CGPoint(x: 2,y: b.metadata.firstBaseline)
        XCTAssertEqual(b.metadata.copy(NSRange(location: 0,length: s.utf16Count)), s.text)
        let cached = b.metadata.cachedIndex(at: point,in: box,artifact: b.id,hits: after.hits)
        XCTAssertNotNil(cached)
        XCTAssertEqual(b.metadata.link(at: point,in: box,exactIndex: cached!),s.link(at: cached!))
        XCTAssertEqual(after.request.rows.first?.selection,NSRange(location: 0,length: 0))
        let reference = ReuseSelectionReference(), completed = DispatchSemaphore(value: 0)
        DispatchQueue(label: "reuse-selection-reference").async {
            let layout = RegionWorkerLayout.shape(s,width: 320), p = layout.metadata
            let dense = RegionParagraph(source: s,lines: layout.lines,baselines: p.baselines,
                width: p.width,height: p.height,lineBottoms: p.lineBottoms,offeredWidth: p.offeredWidth)
            reference.rectangles = dense.selectionRects(NSRange(location: 0,length: 12),in: box,dirty: box)
            completed.signal()
        }
        completed.wait()
        XCTAssertFalse(reference.rectangles.isEmpty, "original exact worker selection-rectangle oracle remains")
        XCTAssertEqual(h.service.pixels.stats.owners, 2)
        withExtendedLifetime([before,after]) {}
    }
    func testAliasBindingsStillCountTowardLive64CapAndRetirementReopensOneSlot() {
        #if REGION_PREPARATION_SENTINEL
        let preparationStart = RegionPreparationSentinel.count
        #endif
        let h = RegionReuseHarness(), s = source("small cap source ")
        defer { h.service.close() }
        var retained: [RegionArtifact?] = []
        for n in 1...64 { retained.append(h.shape(UInt64(n), source: s, height: CGFloat(n))) }
        XCTAssertEqual(h.constructions.count, 1)
        let refused = h.perform(.shape(RegionShapeRequest(id: 65,sourceID: 7,source: s,width: 320,height: -1,generation: 1)))
        if case .refused(.shape(let request),let reason) = refused {
            XCTAssertEqual(request.id, 65); XCTAssertEqual(reason, "region live artifact cap")
        } else { XCTFail("65th live binding must refuse even when backing is shared") }
        XCTAssertEqual(h.admissions.count, 65)
        XCTAssertEqual(h.constructions.count, 1)
        retained[0] = nil
        let next = h.shape(66, source: s)
        XCTAssertTrue(next.metadata === retained[1]?.metadata)
        XCTAssertEqual(h.constructions.count, 1)
        withExtendedLifetime(retained) {}
        #if REGION_PREPARATION_SENTINEL
        XCTAssertEqual(RegionPreparationSentinel.count - preparationStart, 1)
        #endif
    }
    func testDroppingLastDictionaryBindingDoesNotCreateRetainedLookupHistory() {
        #if REGION_PREPARATION_SENTINEL
        let preparationStart = RegionPreparationSentinel.count
        #endif
        let h = RegionReuseHarness(), s = source()
        defer { h.service.close() }
        var a: RegionArtifact? = h.shape(1, source: s)
        weak var oldMetadata = a?.metadata
        defer { oldMetadata = nil }
        XCTAssertNotNil(oldMetadata)
        a = nil
        let b = h.shape(2, source: s)
        XCTAssertEqual(h.constructions.count, 2, "retired entries are not a history cache")
        XCTAssertNil(oldMetadata)
        XCTAssertEqual(b.id, 2)
        #if REGION_PREPARATION_SENTINEL
        XCTAssertEqual(RegionPreparationSentinel.count - preparationStart, 2)
        #endif
    }
    func testResetClearsReuseEvenWhenOldMetadataIsExternallyRetained() {
        #if REGION_PREPARATION_SENTINEL
        let preparationStart = RegionPreparationSentinel.count
        #endif
        let h = RegionReuseHarness(), s = source()
        defer { h.service.close() }
        let old = h.shape(1, source: s)
        h.service.reset()
        let next = h.shape(2, source: s, generation: 2)
        XCTAssertEqual(h.constructions.count, 2)
        XCTAssertFalse(old.metadata === next.metadata)
        XCTAssertEqual(next.generation, 2)
        XCTAssertEqual(old.metadata.copy(NSRange(location: 0,length: s.utf16Count)), s.text)
        withExtendedLifetime(old) {}
        #if REGION_PREPARATION_SENTINEL
        XCTAssertEqual(RegionPreparationSentinel.count - preparationStart, 2)
        #endif
    }
    func testPaintOnlyOwnerIsNotAHistoryEntryAndCloseDropsIt() {
        #if REGION_PREPARATION_SENTINEL
        let preparationStart = RegionPreparationSentinel.count
        #endif
        let h = RegionReuseHarness(), s = source()
        defer { h.service.close() }
        var a: RegionArtifact? = h.shape(1, source: s)
        weak var oldMetadata = a?.metadata
        defer { oldMetadata = nil }
        let raster = h.raster(1, publication: 1, artifact: a!, profile: h.profile())
        a = nil
        let b = h.shape(2, source: s)
        XCTAssertEqual(h.constructions.count, 2, "lookup must not rediscover old paint-only backing")
        XCTAssertNotNil(oldMetadata, "old paint legitimately retains the old layout")
        XCTAssertFalse(oldMetadata === b.metadata)
        h.service.close()
        let deadline = Date().addingTimeInterval(3)
        while oldMetadata != nil && Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.001))
        }
        XCTAssertNil(oldMetadata, "close releases queue-owned paint/layout references")
        XCTAssertNotNil(raster.image(), "pixel payload can outlive service/cache without CTLine ownership")
        withExtendedLifetime(b) {}
        #if REGION_PREPARATION_SENTINEL
        XCTAssertEqual(RegionPreparationSentinel.count - preparationStart, 2)
        #endif
    }
    func testAdmissionBarrierStillRunsOnAReuseHitAndResetKeepsOneWorker() {
        #if REGION_PREPARATION_SENTINEL
        let preparationStart = RegionPreparationSentinel.count
        #endif
        let gate = RegionReuseGate(blockAdmission: 2)
        let h = RegionReuseHarness(admission: { gate.enter() }), s = source()
        defer { h.service.close() }
        let a = h.shape(1, source: s)
        h.service.submit(.shape(RegionShapeRequest(id: 2,sourceID: 7,source: s,width: 320,height: -1,generation: 1)))
        XCTAssertEqual(gate.started.wait(timeout: .now()+2), .success)
        XCTAssertEqual(h.constructions.count, 1)
        for n in 3...40 {
            h.service.reset()
            h.service.submit(.shape(RegionShapeRequest(id: UInt64(n),sourceID: 7,source: s,width: 320,height: -1,generation: n)))
        }
        XCTAssertEqual(gate.count, 2, "blocked old admission still occupies sole queue")
        gate.release.signal()
        let answer = h.receive()
        if case .shape(let newest) = answer { XCTAssertEqual(newest.id, 40); XCTAssertEqual(newest.generation, 40) }
        else { XCTFail("latest generation must recover") }
        XCTAssertEqual(gate.count, 3)
        XCTAssertEqual(h.constructions.count, 2, "old same-source admission reuses before epoch rejection; new generation constructs")
        XCTAssertTrue(h.inbox.answers.isEmpty, "no old answer delivered")
        withExtendedLifetime(a) {}
        #if REGION_PREPARATION_SENTINEL
        XCTAssertEqual(RegionPreparationSentinel.count - preparationStart, 2)
        #endif
    }
}

@MainActor private final class RegionReuseInbox { var answers: [RegionAnswer] = [] }
private final class RegionReuseCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var value = 0
    var count: Int { lock.lock(); defer { lock.unlock() }; return value }
    func increment() { lock.lock(); value += 1; lock.unlock() }
}
private final class RegionReuseGate: @unchecked Sendable {
    let started = DispatchSemaphore(value: 0), release = DispatchSemaphore(value: 0)
    private let lock = NSLock()
    private var value = 0
    private let blockAdmission: Int
    init(blockAdmission: Int) { self.blockAdmission = blockAdmission }
    var count: Int { lock.lock(); defer { lock.unlock() }; return value }
    func enter() {
        lock.lock(); value += 1; let block = value == blockAdmission; lock.unlock()
        if block { started.signal(); release.wait() }
    }
}
@MainActor private final class RegionReuseHarness {
    let inbox = RegionReuseInbox()
    let admissions = RegionReuseCounter(), constructions = RegionReuseCounter()
    let service: RegionService
    init(admission: @escaping @Sendable () -> Void = {}) {
        let inbox = self.inbox, admissions = self.admissions, constructions = self.constructions
        service = RegionService(beforeShape: { admissions.increment(); admission() },
                                beforeLayoutConstruction: { constructions.increment() }) { answer in
            inbox.answers.append(answer)
        }
    }
    func receive() -> RegionAnswer {
        let deadline = Date().addingTimeInterval(5)
        while inbox.answers.isEmpty && Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.001))
        }
        precondition(!inbox.answers.isEmpty, "actual worker delivery deadline")
        return inbox.answers.removeFirst()
    }
    func perform(_ job: RegionJob) -> RegionAnswer { service.submit(job); return receive() }
    func shape(_ id: UInt64, source: RegionTextSource, sourceID: UInt64 = 7,
               width: CGFloat = 320, height: CGFloat = -1, generation: Int = 1) -> RegionArtifact {
        let answer = perform(.shape(RegionShapeRequest(id: id,sourceID: sourceID,source: source,
                                                      width: width,height: height,generation: generation)))
        guard case .shape(let value) = answer else { preconditionFailure("expected shape, got \(answer)") }
        return value
    }
    func profile() -> NativeProfile {
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        return NativeProfile.capture(original: space,data: space.copyICCData(),account: NativeProfileAccount()).owner!
    }
    func raster(_ serial: UInt64, publication: UInt64, artifact: RegionArtifact, profile: NativeProfile,
                selection: NSRange = NSRange(location: 0,length: 0)) -> RegionRaster {
        let request = RegionRasterRequest(serial: serial,publication: publication,generation: artifact.generation,
            rows: [RegionPaintRow(artifact: artifact.id,box: CGRect(x: 0,y: 0,width: artifact.metadata.offeredWidth,height: artifact.metadata.height),selection: selection)],
            scroll: CGPoint(x: 0,y: 0.375),size: CGSize(width: 340,height: 100),scale: 1,
            profile: profile,format: CGImageAlphaInfo.premultipliedLast.rawValue,
            background: [1,1,1,1],selectionColor: [0.2,0.4,0.8,0.45])
        guard case .raster(let value) = perform(.raster(request)) else { preconditionFailure("expected raster") }
        return value
    }
}

private final class ReuseSelectionReference: @unchecked Sendable { var rectangles: [CGRect] = [] }
private final class PreparedLifetimeEvidence: @unchecked Sendable {
    var shared = false, sourceMatched = false, survivesFirstDrop = false, dropsLast = false, worker = false
}

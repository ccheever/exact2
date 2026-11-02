import Foundation
import CoreText
import CoreGraphics
import XCTest
@testable import ExactKit

@MainActor final class RegionViewportHitsTests: XCTestCase {
    func testInPlaceUniqueMatchesFiniteReferenceThroughMaximumScratchCapacity() {
        // Actual helper: only the reference/oracle owns Swift sorting arrays.
        // Each caller pointer has sentinels proving writes stay in its range.
        let maximumOffsets = 6 * RegionViewportHits.maximumCaretUnits + 18
        let maximumEdges = 2 * maximumOffsets - 1
        let inputs: [[CGFloat]] = [[],[7],[7,7,7],[-0.0,0.0,-3,4,-3,2,-0.0],
            Array((-100...100).reversed()).map { CGFloat($0) },
            (0..<maximumEdges).reversed().map { CGFloat($0 / 3 - maximumEdges / 6) }]
        for input in inputs {
            let storage = UnsafeMutablePointer<CGFloat>.allocate(capacity: input.count + 2)
            storage.initialize(repeating: 123456789,count: input.count + 2)
            defer { storage.deinitialize(count: input.count + 2); storage.deallocate() }
            let values = storage.advanced(by: 1)
            for i in input.indices { values[i] = input[i] }
            let sorted = input.sorted()
            var expected: [CGFloat] = []
            for value in sorted where expected.last != value { expected.append(value) }
            let count = RegionViewportHits.sortedUnique(values,count: input.count)
            XCTAssertEqual(count,expected.count)
            XCTAssertEqual(Array(UnsafeBufferPointer(start: values,count: count)),expected)
            XCTAssertEqual(storage[0],123456789)
            XCTAssertEqual(storage[input.count + 1],123456789)
        }
        XCTAssertEqual(maximumEdges,98339)
    }
    private func source(_ text: String, height: CGFloat = 26) -> RegionTextSource {
        let run = Run(text: text, size: 16, weight: 400, family: 0, italic: false,
            lineHeight: height, letterSpacing: 0, href: "https://example.invalid/hit")
        return RegionTextSource.capture(Spec(runs: [run], align: 0, lineClamp: 0,
            color: [0,0,0,255], overflowWrap: 0, strut: run), engine: TextEngine(resolve: { _ in nil }))
    }
    private func request(_ source: RegionTextSource, height: CGFloat = 160) -> RegionRasterRequest {
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let profile = NativeProfile.capture(original: space, data: space.copyICCData(), account: NativeProfileAccount()).owner!
        return RegionRasterRequest(serial: 1, publication: 2, generation: 3,
            rows: [RegionPaintRow(artifact: 4, box: CGRect(x: 0,y: 0,width: 320,height: height))],
            scroll: CGPoint(x: 0,y: 0.375),size: CGSize(width: 320,height: 160),scale: 1,
            profile: profile,format: CGImageAlphaInfo.premultipliedLast.rawValue,
            background: [1,1,1,1],selectionColor: [0.2,0.4,0.8,0.45])
    }
    func testQuickDownDragUpRetainsBothExactPointsUntilAnchorAnswer() {
        var contact = RegionContact(gesture: 1, artifact: 4, point: CGPoint(x: 10,y: 20))
        let down = contact.query!
        XCTAssertTrue(contact.update(point: CGPoint(x: 30,y: 40), dragged: true, terminal: false))
        XCTAssertTrue(contact.update(point: CGPoint(x: 70,y: 800), dragged: true, terminal: true))
        XCTAssertEqual(contact.query!.begin, down.begin)
        XCTAssertEqual(contact.query!.point, CGPoint(x: 70,y: 800))
        XCTAssertTrue(contact.query!.terminal)
        XCTAssertFalse(contact.accept(RegionPointReply(query: down, anchor: 2,index: 2,link: nil,selection: nil)))
        let final = contact.query!
        XCTAssertTrue(contact.accept(RegionPointReply(query: final,anchor: 2,index: 90,link: nil,
            selection: NSRange(location: 2,length: 88))))
        XCTAssertTrue(contact.finished)
        XCTAssertNil(contact.query)
        XCTAssertFalse(contact.accept(RegionPointReply(query: final,anchor: 2,index: 90,link: nil,selection: nil)))
    }
    func testResolvedAnchorNeedsOnlyOneLatestPointAndWrongGestureIsInert() {
        var contact = RegionContact(gesture: 7, artifact: 4, point: CGPoint(x: 1,y: 2))
        let first = contact.query!
        XCTAssertTrue(contact.accept(RegionPointReply(query: first,anchor: 9,index: 9,link: nil,selection: nil)))
        XCTAssertTrue(contact.update(point: CGPoint(x: 100,y: 4000), dragged: true, terminal: false))
        XCTAssertNil(contact.query!.begin)
        XCTAssertEqual(contact.query!.anchor, 9)
        let other = RegionContact(gesture: 8, artifact: 4, point: .zero).query!
        XCTAssertFalse(contact.accept(RegionPointReply(query: other,anchor: 2,index: 2,link: nil,selection: nil)))
        XCTAssertEqual(contact.query!.point, CGPoint(x: 100,y: 4000))
    }
    func testDelayedFinalPointPublishesSelectionAndPixelsTogetherOnActualWorker() {
        let s = source(String(repeating: "ffi אבג selection words ",count: 10))
        let phase = request(s,height: 20000)
        let gate = ViewportShapeGate(), inbox = ViewportInbox()
        let service = RegionService(beforeShape: { gate.enter() }) { inbox.answers.append($0) }
        defer { service.close() }
        func drain() -> RegionAnswer {
            let deadline = Date().addingTimeInterval(5)
            while inbox.answers.isEmpty && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.001)) }
            precondition(!inbox.answers.isEmpty, "actual queue delivery")
            return inbox.answers.removeFirst()
        }
        service.submit(.shape(RegionShapeRequest(id: 4,sourceID: 7,source: s,width: 320,height: 400,generation: 3)))
        guard case .shape(let a) = drain() else { XCTFail("first shape"); return }
        // Block another admission on the actual same service queue, then enqueue
        // down and terminal intents. The first down must remain in the latest.
        service.submit(.shape(RegionShapeRequest(id: 5,sourceID: 7,source: s,width: 320,height: -1,generation: 3)))
        XCTAssertEqual(gate.started.wait(timeout: .now()+2),.success)
        var contact = RegionContact(gesture: 1,artifact: 4,point: CGPoint(x: 5,y: 4))
        var down = phase; down.interaction = contact.query
        service.submit(.raster(down))
        XCTAssertTrue(contact.update(point: CGPoint(x: 100,y: 45),dragged: true,terminal: false))
        XCTAssertTrue(contact.update(point: CGPoint(x: 80,y: 90),dragged: true,terminal: true))
        var final = phase; final.interaction = contact.query
        service.submit(.raster(final))
        gate.release.signal()
        guard case .shape(let alias) = drain() else { XCTFail("held admission result"); return }
        guard case .raster(let result) = drain(), let answer = result.interaction else { XCTFail("one final raster answer"); return }
        XCTAssertEqual(result.intent.interaction,final.interaction)
        XCTAssertTrue(result.intent.sameOutput(as: final))
        XCTAssertEqual(result.request.rows[0].selection,answer.selection)
        XCTAssertTrue(contact.accept(answer))
        XCTAssertTrue(contact.finished)
        XCTAssertFalse(contact.accept(answer),"terminal selection applies once")
        XCTAssertNotNil(result.image())
        XCTAssertTrue(inbox.answers.isEmpty,"no independent stale down output")
        // Independent ordinary renderer supplies the exact endpoints and selected pixels.
        let run = Run(text: s.text,size: 16,weight: 400,family: 0,italic: false,lineHeight: 26,
            letterSpacing: 0,href: "https://example.invalid/hit")
        let engine = TextEngine(resolve: { _ in nil })
        let spec = Spec(runs: [run],align: 0,lineClamp: 0,color: [0,0,0,255],strut: run)
        let ordinary = engine.paragraph(spec,width: 320)
        func ordinaryIndex(_ point: CGPoint) -> Int {
            let line = ordinary.lineBottoms.firstIndex(where: { point.y < $0 }) ?? (ordinary.lines.count - 1)
            let row = ordinary.lines[line]
            let x = point.x - CGFloat(CTLineGetPenOffsetForFlush(row,0,320))
            let value = CTLineGetStringIndexForPosition(row,CGPoint(x: x,y: 0))
            return value == kCFNotFound ? s.utf16Count : min(max(0,value),s.utf16Count)
        }
        let expectedAnchor = ordinaryIndex(CGPoint(x: 5,y: 4))
        let expectedEnd = ordinaryIndex(CGPoint(x: 80,y: 90))
        XCTAssertEqual(answer.anchor,expectedAnchor)
        XCTAssertEqual(answer.index,expectedEnd)
        let selected = NSRange(location: min(expectedAnchor,expectedEnd),length: abs(expectedEnd-expectedAnchor))
        XCTAssertEqual(answer.selection,selected)
        let space = phase.profile.makeSpace()!
        let context = CGContext(data: nil,width: 320,height: 160,bitsPerComponent: 8,bytesPerRow: 1280,
            space: space,bitmapInfo: phase.format)!
        context.setFillColor(CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(),components: phase.background)!)
        context.fill(CGRect(x: 0,y: 0,width: 320,height: 160))
        context.translateBy(x: 0,y: 160); context.scaleBy(x: 1,y: -1)
        context.setFillColor(CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(),components: phase.selectionColor)!)
        for (i,line) in ordinary.lines.enumerated() {
            let range = CTLineGetStringRange(line)
            let lo = max(selected.location,range.location), hi = min(NSMaxRange(selected),range.location+range.length)
            guard hi > lo else { continue }
            var ascent: CGFloat = 0, descent: CGFloat = 0
            _ = CTLineGetTypographicBounds(line,&ascent,&descent,nil)
            let x0 = CTLineGetOffsetForStringIndex(line,lo,nil), x1 = CTLineGetOffsetForStringIndex(line,hi,nil)
            let x = CGFloat(CTLineGetPenOffsetForFlush(line,0,320))
            let y = ordinary.baselines[i].rounded() - phase.scroll.y
            context.fill(CGRect(x: x+min(x0,x1),y: y-ascent,width: max(1,abs(x1-x0)),height: ascent+descent))
        }
        TextEngine.draw(ordinary,spec: spec,in: CGRect(x: 0,y: -phase.scroll.y,width: 320,height: ordinary.height),context: context)
        context.flush()
        XCTAssertEqual(result.image()!.dataProvider!.data! as Data,Data(bytes: context.data!,count: 320*160*4))
        withExtendedLifetime([a,alias]) {}
    }
    func testCompleteOutputCertificateRejectsABAAndPhaseChanges() {
        let s = source("point certificate"), a = request(s)
        var b = a; b.interaction = RegionContact(gesture: 1,artifact: 4,point: .zero).query
        XCTAssertFalse(a.sameOutput(as: b))
        var newer = b
        newer.interaction = RegionContact(gesture: 2,artifact: 4,point: .zero).query
        XCTAssertFalse(newer.sameOutput(as: b))
        let moved = RegionRasterRequest(serial: 3,publication: a.publication,generation: a.generation,
            rows: a.rows,scroll: CGPoint(x: 0,y: 1.375),size: a.size,scale: a.scale,
            profile: a.profile,format: a.format,background: a.background,selectionColor: a.selectionColor,interaction: b.interaction)
        XCTAssertFalse(moved.sameOutput(as: b))
        XCTAssertFalse(moved.sameInkAndGeometry(as: b))
    }
    func testSamePixelsDoesNotMeanSameInteractionOutput() {
        let s = source("ffi words")
        let a = request(s)
        var b = a
        b.interaction = RegionContact(gesture: 1,artifact: 4,point: .zero).query
        XCTAssertTrue(a.samePixels(as: b))
        XCTAssertFalse(a.sameOutput(as: b))
    }
    func testHitChargeHasTwoActualOwnersAndLastAliasRetires() {
        let account = RegionHitAccount()
        var a: RegionHitCharge? = account.reserve()
        var alias = a
        var b: RegionHitCharge? = account.reserve()
        XCTAssertEqual(account.stats.bytes, 8 * 1024 * 1024)
        XCTAssertNil(account.reserve())
        a = nil
        XCTAssertEqual(account.stats.owners, 2)
        alias = nil
        XCTAssertEqual(account.stats.owners, 1)
        b = nil
        XCTAssertEqual(account.stats.bytes, 0)
        withExtendedLifetime([a,alias,b]) {}
    }
    func testActualSlabAliasesKeepChargeAndThirdCacheFallsBackWithoutRefusing() {
        let s = source("actual retained metadata owner"), r = request(s)
        let box = ViewportHitOwners(), account = RegionHitAccount(), done = DispatchSemaphore(value: 0)
        DispatchQueue(label: "hit-owner-lifetime-test").async {
            let layout = RegionWorkerLayout.shape(s,width: 320)
            box.first = RegionViewportHits(request: r,lookup: { _ in layout },account: account)
            box.second = RegionViewportHits(request: r,lookup: { _ in layout },account: account)
            box.third = RegionViewportHits(request: r,lookup: { _ in layout },account: account)
            done.signal()
        }
        done.wait()
        XCTAssertGreaterThan(box.first!.cachedLines,0)
        XCTAssertGreaterThan(box.second!.cachedLines,0)
        XCTAssertEqual(box.third!.cachedLines,0)
        XCTAssertEqual(account.stats.owners,2)
        var alias = box.first
        box.first = nil
        withExtendedLifetime(alias) {
            XCTAssertEqual(account.stats.bytes,8*1024*1024)
        }
        alias = nil
        XCTAssertEqual(account.stats.bytes,4*1024*1024)
        box.second = nil
        XCTAssertEqual(account.stats.bytes,0)
        XCTAssertEqual(account.stats.drops,2)
    }
    func testSingleMiBUnwrappedLineRendersAndFallsBackExactlyWithoutDenseExpansion() {
        let s = source(String(repeating: "W",count: 1024 * 1024))
        let r = request(s), done = DispatchSemaphore(value: 0), box = ViewportHitBox()
        DispatchQueue(label: "long-line-viewport-test").async {
            let layout = RegionWorkerLayout.shape(s,width: 320)
            box.lean = layout.metadata.lines.allSatisfy { $0.carets.isEmpty }
            box.lineCount = layout.lines.count
            let account = RegionHitAccount()
            let hits = RegionViewportHits(request: r,lookup: { $0 == 4 ? layout : nil },account: account)
            box.cached = hits.index(artifact: 4,line: 0,x: 100)
            box.units = hits.caretUnits
            box.direct = layout.exactIndex(at: CGPoint(x: 100,y: 5),in: r.rows[0].box)
            box.expected = CTLineGetStringIndexForPosition(layout.lines[0],CGPoint(x: 100,y: 0))
            do {
                let paint = try RegionPaintIndex(request: r,lookup: { $0 == 4 ? layout : nil },account: InkAccount())
                box.raster = try paint.render(r,account: RegionPixelAccount(),hits: account)
            } catch { box.error = String(describing: error) }
            box.copy = layout.metadata.copy(NSRange(location: 0,length: s.utf16Count))
            done.signal()
        }
        done.wait()
        XCTAssertEqual(box.error, "")
        XCTAssertEqual(box.lineCount, 1)
        XCTAssertTrue(box.lean)
        XCTAssertNil(box.cached)
        XCTAssertEqual(box.units, 0, "oversize is detected before any per-UTF16 expansion")
        XCTAssertEqual(box.direct, box.expected)
        XCTAssertEqual(box.copy, s.text)
        XCTAssertNotNil(box.raster?.image(), "missing dense coverage must not refuse pixels")
    }
    func testZeroHeightOverlapIsBoundedAndUnavailableUsesExactWorkerPoint() {
        let s = source(String(repeating: "small words\n",count: 2000),height: 0)
        let r = request(s), done = DispatchSemaphore(value: 0), box = ViewportHitBox()
        DispatchQueue(label: "overlap-hit-test").async {
            let layout = RegionWorkerLayout.shape(s,width: 320)
            let hits = RegionViewportHits(request: r,lookup: { _ in layout },account: RegionHitAccount())
            box.units = hits.caretUnits; box.lineCount = hits.cachedLines
            box.direct = layout.exactIndex(at: CGPoint(x: 2,y: 0),in: r.rows[0].box)
            box.expected = layout.metadata.source.utf16Count
            done.signal()
        }
        done.wait()
        XCTAssertLessThanOrEqual(box.units, RegionViewportHits.maximumCaretUnits)
        XCTAssertLessThanOrEqual(box.lineCount, RegionViewportHits.maximumLines)
        XCTAssertGreaterThanOrEqual(box.direct, 0)
        XCTAssertLessThanOrEqual(box.direct, box.expected)
    }
    func testVisibleCacheMatchesDirectCoreTextEdgesAndOffViewportPoint() {
        let s = source(String(repeating: "ffi e\u{301} אבג 🧑🏿‍🚀 link words ",count: 80))
        let r = request(s,height: 20000), done = DispatchSemaphore(value: 0), box = ViewportHitBox()
        DispatchQueue(label: "exact-viewport-hit-test").async {
            let layout = RegionWorkerLayout.shape(s,width: 320)
            let hits = RegionViewportHits(request: r,lookup: { _ in layout },account: RegionHitAccount())
            for y: CGFloat in [0,0.375,25.999,26,52,159.999,160.375] {
                let line = layout.metadata.lineIndex(at: y)!
                for x: CGFloat in [-1,0,1,17,100,319,321] {
                    let direct = layout.exactIndex(at: CGPoint(x: x,y: y),in: r.rows[0].box)
                    let cached = layout.metadata.cachedIndex(at: CGPoint(x: x,y: y),in: r.rows[0].box,artifact: 4,hits: hits)
                    box.answers.append((direct,cached)); box.lineCount = max(box.lineCount,line)
                }
            }
            box.cached = layout.metadata.cachedIndex(at: CGPoint(x: 10,y: 900),in: r.rows[0].box,artifact: 4,hits: hits)
            done.signal()
        }
        done.wait()
        XCTAssertEqual(box.answers.count,49)
        for (expected,actual) in box.answers { XCTAssertEqual(actual,expected) }
        XCTAssertNil(box.cached,"out of coverage cannot clamp to last cached line")
    }
}
private final class ViewportHitBox: @unchecked Sendable {
    var lean = false, lineCount = 0, units = 0, cached: Int?, direct = 0, expected = 0
    var raster: RegionRaster?, copy = "", error = ""
    var answers: [(Int,Int?)] = []
}

@MainActor private final class ViewportInbox { var answers: [RegionAnswer] = [] }
private final class ViewportShapeGate: @unchecked Sendable {
    let started = DispatchSemaphore(value: 0), release = DispatchSemaphore(value: 0)
    private let lock = NSLock()
    private var admissions = 0
    func enter() {
        lock.lock(); admissions += 1; let block = admissions == 2; lock.unlock()
        if block { started.signal(); release.wait() }
    }
}

private final class ViewportHitOwners: @unchecked Sendable {
    var first: RegionViewportHits?, second: RegionViewportHits?, third: RegionViewportHits?
}

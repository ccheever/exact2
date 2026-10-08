// Stationary viewport pixels (opaque surfaces or transparent reader ink).
// Fractional content/scroll phase stays inside
// worker drawing; output edges and the final copy are integral device pixels.
import Foundation
import CoreGraphics
import CoreText
#if os(macOS)
import IOSurface
import ObjectiveC
#endif

struct RegionPaintRow: Sendable, Equatable {
    let artifact: UInt64
    let box: CGRect
    var selection = NSRange(location: 0, length: 0)
}
struct RegionRasterRequest: Sendable {
    let serial: UInt64
    let publication: UInt64
    let generation: Int
    let rows: [RegionPaintRow]
    let scroll: CGPoint
    let size: CGSize
    let scale: Int
    let profile: NativeProfile
    let format: UInt32
    let background: [CGFloat]
    let selectionColor: [CGFloat]
    var interaction: RegionPointRequest? = nil
    // The registered region surface keeps its 8 MiB contract. A reader can
    // admit a larger visible viewport, never a document-sized bitmap.
    var pixelLimit: Int = 8 * 1024 * 1024
    static let maximumPixelLimit = 32 * 1024 * 1024
    static let compositedFormat = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
    static func stride(width: Int, format: UInt32) -> Int {
        #if os(macOS)
        if format == compositedFormat { return IOSurfaceAlignProperty(kIOSurfaceBytesPerRow, width * 4) }
        #endif
        return width * 4
    }
    var bytesPerRow: Int { Self.stride(width: width, format: format) }
    // A continuing selection gesture may outlive replacement highlight pixels,
    // but never a source/geometry/palette change. New hits still require all pixels.
    func sameInkAndGeometry(as other: RegionRasterRequest) -> Bool {
        publication == other.publication && generation == other.generation &&
        rows.count == other.rows.count && zip(rows, other.rows).allSatisfy({
            $0.artifact == $1.artifact && $0.box == $1.box
        }) && scroll == other.scroll && size == other.size && scale == other.scale && profile == other.profile &&
        format == other.format && background == other.background && selectionColor == other.selectionColor
    }
    // Serial is delivery identity; the remaining fields describe exact pixels.
    func samePixels(as other: RegionRasterRequest) -> Bool {
        sameInkAndGeometry(as: other) && rows == other.rows
    }
    func sameOutput(as other: RegionRasterRequest) -> Bool {
        samePixels(as: other) && interaction == other.interaction
    }
    func selecting(_ range: NSRange?, artifact: UInt64) -> RegionRasterRequest {
        var rows = self.rows
        if let range, let i = rows.firstIndex(where: { $0.artifact == artifact }) { rows[i].selection = range }
        return RegionRasterRequest(serial: serial,publication: publication,generation: generation,rows: rows,
            scroll: scroll,size: size,scale: scale,profile: profile,format: format,
            background: background,selectionColor: selectionColor,interaction: interaction,pixelLimit: pixelLimit)
    }
    var width: Int { Int(size.width * CGFloat(scale)) }
    var height: Int { Int(size.height * CGFloat(scale)) }
    var bytes: Int? {
        let w = size.width * CGFloat(scale), h = size.height * CGFloat(scale)
        guard (scale == 1 || scale == 2), w.isFinite, h.isFinite,
              w > 0, h > 0, w <= 16384, h <= 16384,
              w.rounded() == w, h.rounded() == h,
              background.count == 4, (background[3] == 1 || background[3] == 0),
              background.allSatisfy({ $0.isFinite && $0 >= 0 && $0 <= 1 }),
              selectionColor.count == 4,
              selectionColor.allSatisfy({ $0.isFinite && $0 >= 0 && $0 <= 1 }),
              scroll.x.isFinite, scroll.y.isFinite, rows.count <= 64,
              pixelLimit > 0, pixelLimit <= Self.maximumPixelLimit,
              format == CGImageAlphaInfo.premultipliedLast.rawValue || format == Self.compositedFormat else { return nil }
        var bytes = bytesPerRow * Int(h)
        #if os(macOS)
        if format == Self.compositedFormat { bytes = IOSurfaceAlignProperty(kIOSurfaceAllocSize, bytes) }
        #endif
        return bytes > pixelLimit ? nil : bytes
    }
}
enum RegionRasterRefusal: Error { case capacity, invalid, missingArtifact, profile, context }
struct RegionPixelStats { var bytes = 0; var owners = 0; var peak = 0; var drops = 0 }
final class RegionPixelAccount: @unchecked Sendable {
    private let lock = NSLock()
    private var value = RegionPixelStats()
    var stats: RegionPixelStats { lock.lock(); defer { lock.unlock() }; return value }
    func reserve(_ bytes: Int, limit: Int = 8 * 1024 * 1024) -> RegionPixelCharge? {
        lock.lock(); defer { lock.unlock() }
        guard bytes > 0, bytes <= limit, limit <= RegionRasterRequest.maximumPixelLimit, value.owners < 2,
              bytes <= 2 * RegionRasterRequest.maximumPixelLimit - value.bytes else { return nil }
        value.bytes += bytes; value.owners += 1; value.peak = max(value.peak, value.bytes)
        return RegionPixelCharge(self, bytes)
    }
    fileprivate func release(_ bytes: Int) {
        lock.lock(); value.bytes -= bytes; value.owners -= 1; value.drops += 1; lock.unlock()
    }
}
final class RegionPixelCharge: Sendable {
    private let account: RegionPixelAccount
    let bytes: Int
    init(_ account: RegionPixelAccount, _ bytes: Int) { self.account = account; self.bytes = bytes }
    deinit { account.release(bytes) }
}
/// Mutation is confined to construction on worker. The CGContext is destroyed
/// before publication. Provider aliases retain both pixel and profile charges.
final class RegionPixels: @unchecked Sendable {
    private let storage: UnsafeMutableRawPointer
    let count: Int
    private let charge: RegionPixelCharge
    private let profile: NativeProfile
    #if os(macOS)
    let surface: IOSurface?
    private static var chargeKey: UInt8 = 0
    #endif
    init(count: Int, charge: RegionPixelCharge, profile: NativeProfile, request: RegionRasterRequest? = nil,
         fill: (UnsafeMutableRawPointer) -> Bool) throws {
        precondition(!Thread.isMainThread)
        #if os(macOS)
        if let request, request.format == RegionRasterRequest.compositedFormat {
            guard let surface = IOSurface(properties: [.width: request.width, .height: request.height,
                    .bytesPerElement: 4, .bytesPerRow: request.bytesPerRow, .allocSize: count,
                    .pixelFormat: UInt32(0x42475241)]), surface.allocationSize <= count,
                  surface.bytesPerRow == request.bytesPerRow, let space = profile.makeSpace() else {
                throw RegionRasterRefusal.context
            }
            // CALayer and CGImage providers can outlive the raster receipt.
            // Charge the surface itself until its final local owner releases it.
            objc_setAssociatedObject(surface, &Self.chargeKey, charge, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
            surface.lock(options: [], seed: nil)
            surface.baseAddress.initializeMemory(as: UInt8.self, repeating: 0, count: count)
            let filled = fill(surface.baseAddress)
            surface.unlock(options: [], seed: nil)
            guard filled else { throw RegionRasterRefusal.context }
            if let color = space.copyPropertyList() { IOSurfaceSetValue(surface, kIOSurfaceColorSpace, color) }
            self.surface = surface; storage = surface.baseAddress
            self.count = count; self.charge = charge; self.profile = profile
            return
        }
        surface = nil
        #endif
        let memory = UnsafeMutableRawPointer.allocate(byteCount: count, alignment: 64)
        memory.initializeMemory(as: UInt8.self, repeating: 0, count: count)
        guard fill(memory) else { memory.deallocate(); throw RegionRasterRefusal.context }
        storage = memory; self.count = count; self.charge = charge; self.profile = profile
    }
    deinit {
        #if os(macOS)
        if surface != nil { return }
        #endif
        storage.deallocate()
    }
    @MainActor func provider() -> CGDataProvider? {
        let retained = Unmanaged.passRetained(self)
        guard let p = CGDataProvider(dataInfo: retained.toOpaque(), data: storage, size: count,
            releaseData: { info, _, _ in if let info { Unmanaged<RegionPixels>.fromOpaque(info).release() } }) else {
            retained.release(); return nil
        }
        return p
    }
}
final class RegionRaster: Sendable {
    let request: RegionRasterRequest
    let pixels: RegionPixels
    let hits: RegionViewportHits
    let intent: RegionRasterRequest
    let interaction: RegionPointReply?
    init(_ request: RegionRasterRequest, _ pixels: RegionPixels, hits: RegionViewportHits,
         intent: RegionRasterRequest, interaction: RegionPointReply?) {
        self.request = request; self.pixels = pixels; self.hits = hits
        self.intent = intent; self.interaction = interaction
    }
    /// Certifies the supplied payload, never an unknown destination CGContext.
    @MainActor func accepts(_ image: CGImage, size: CGSize, scale: Int, profile: NativeProfile) -> Bool {
        guard request.size == size, request.scale == scale, request.profile == profile,
              image.width == request.width, image.height == request.height,
              image.bitsPerComponent == 8, image.bitsPerPixel == 32,
              image.bytesPerRow == request.bytesPerRow, image.bitmapInfo.rawValue == request.format,
              let space = image.colorSpace, let expected = profile.makeSpace() else { return false }
        return CFEqual(space, expected)
    }
    @MainActor func image() -> CGImage? {
        guard let space = request.profile.makeSpace(), let provider = pixels.provider() else { return nil }
        return CGImage(width: request.width, height: request.height, bitsPerComponent: 8, bitsPerPixel: 32,
            bytesPerRow: request.bytesPerRow, space: space, bitmapInfo: CGBitmapInfo(rawValue: request.format),
            provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    }
}
/// One aggregate index per accepted publication, including all paragraph ink.
/// At most 64 paragraph boundaries, no second per-line mapping allocation.
final class RegionPaintIndex {
    let publication: UInt64
    private let rows: [RegionPaintRow]
    private let layouts: [RegionWorkerLayout]
    private let starts: [Int]
    private let index: WorkerInkIndex
    init(request: RegionRasterRequest, lookup: (UInt64) -> RegionWorkerLayout?, account: InkAccount) throws {
        precondition(!Thread.isMainThread)
        publication = request.publication
        let rows = request.rows; self.rows = rows
        var layouts: [RegionWorkerLayout] = [], starts: [Int] = [], count = 0
        for row in rows {
            guard let layout = lookup(row.artifact), row.box.width == layout.metadata.offeredWidth else {
                throw RegionRasterRefusal.missingArtifact
            }
            layouts.append(layout); starts.append(count); count += layout.lineCount
        }
        self.layouts = layouts; self.starts = starts
        index = try WorkerInkIndex(count: count, account: account) { slot in
            let p = Self.paragraph(slot, starts: starts), i = slot - starts[p]
            let line = layouts[p].metadata.lines[i]
            let ink = line.ink
            let a = line.ascent, d = line.descent, l = line.leading
            let above = max(a + max(l, 0), ink.isNull ? 0 : ink.maxY)
            let below = max(d + max(l, 0), ink.isNull ? 0 : -ink.minY)
            let y = rows[p].box.minY + layouts[p].baselines[i].rounded()
            return InkSpan(top: Double(y - above - 2), bottom: Double(y + below + 2))
        }
    }
    private static func paragraph(_ slot: Int, starts: [Int]) -> Int {
        var lo = 0, hi = starts.count
        while lo < hi {
            let mid = lo + (hi - lo) / 2
            if starts[mid] <= slot { lo = mid + 1 } else { hi = mid }
        }
        return max(0, lo - 1)
    }
    func render(_ intent: RegionRasterRequest, account: RegionPixelAccount,
                hits accountHits: RegionHitAccount) throws -> RegionRaster {
        precondition(!Thread.isMainThread)
        guard intent.publication == publication, intent.rows.count == rows.count,
              zip(intent.rows,rows).allSatisfy({ $0.artifact == $1.artifact && $0.box == $1.box }) else {
            throw RegionRasterRefusal.missingArtifact
        }
        for layout in layouts { layout.beginViewport() }
        let interaction: RegionPointReply?
        if let query = intent.interaction {
            guard let i = intent.rows.firstIndex(where: { $0.artifact == query.artifact }),
                  query.point.x.isFinite, query.point.y.isFinite,
                  query.begin.map({ $0.x.isFinite && $0.y.isFinite }) != false,
                  query.anchor != nil || query.begin != nil else { throw RegionRasterRefusal.invalid }
            let layout = layouts[i], box = intent.rows[i].box
            let anchor = query.anchor ?? layout.exactIndex(at: query.begin!,in: box)
            guard anchor >= 0, anchor <= layout.source.utf16Count else { throw RegionRasterRefusal.invalid }
            let index = layout.exactIndex(at: query.point,in: box)
            let selection: NSRange? = query.selectAll ? NSRange(location: 0,length: layout.source.utf16Count)
                : query.dragged ? NSRange(location: min(anchor,index),length: abs(index-anchor)) : nil
            interaction = RegionPointReply(query: query,anchor: anchor,index: index,
                link: layout.metadata.link(at: query.point,in: box,exactIndex: index),selection: selection)
        } else { interaction = nil }
        let request = interaction.map { intent.selecting($0.selection,artifact: $0.query.artifact) } ?? intent
        guard request.publication == publication, request.rows.count == rows.count,
              let count = request.bytes else { throw RegionRasterRefusal.invalid }
        guard let charge = account.reserve(count, limit: request.pixelLimit) else { throw RegionRasterRefusal.capacity }
        guard let space = request.profile.makeSpace() else { throw RegionRasterRefusal.profile }
        let pixels = try RegionPixels(count: count, charge: charge, profile: request.profile, request: request) { pointer in
            guard let ctx = CGContext(data: pointer, width: request.width, height: request.height,
                    bitsPerComponent: 8, bytesPerRow: request.bytesPerRow, space: space,
                    bitmapInfo: request.format) else { return false }
            ctx.setFillColor(CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(), components: request.background)!)
            ctx.setBlendMode(.copy)
            ctx.fill(CGRect(x: 0, y: 0, width: request.width, height: request.height))
            ctx.setBlendMode(.normal)
            ctx.translateBy(x: 0, y: CGFloat(request.height))
            ctx.scaleBy(x: CGFloat(request.scale), y: -CGFloat(request.scale))
            index.query(top: Double(request.scroll.y), bottom: Double(request.scroll.y + request.size.height)) { ids, _ in
                // Ordinary renderer paints all selection before all ink.
                for selected in [true, false] {
                    if selected { ctx.setFillColor(CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(), components: request.selectionColor)!) }
                    for ordinal in ids {
                        let p = Self.paragraph(Int(ordinal), starts: starts), i = Int(ordinal) - starts[p]
                        let layout = layouts[p], row = request.rows[p], line = layout.line(at: i)
                        let flush: CGFloat = layout.source.align == 1 ? 0.5 : layout.source.align == 2 ? 1 : 0
                        let x = row.box.minX - request.scroll.x + TextEngine.lineOffset(line, flush: flush, width: row.box.width, rtl: layout.source.direction == 1)
                        let y = row.box.minY - request.scroll.y + layout.baselines[i].rounded()
                        if selected {
                            let range = CTLineGetStringRange(line)
                            let lo = max(row.selection.location, range.location)
                            let hi = min(NSMaxRange(row.selection), range.location + range.length)
                            if hi > lo {
                                var a: CGFloat = 0, d: CGFloat = 0
                                _ = CTLineGetTypographicBounds(line, &a, &d, nil)
                                let x0 = CTLineGetOffsetForStringIndex(line, lo, nil), x1 = CTLineGetOffsetForStringIndex(line, hi, nil)
                                ctx.fill(CGRect(x: x + min(x0, x1), y: y - a, width: max(1, abs(x1 - x0)), height: a + d))
                            }
                        } else { TextLinePaint.draw(line, at: CGPoint(x: x, y: y), in: ctx) }
                    }
                }
            }
            ctx.flush(); return true
        }
        let hits = RegionViewportHits(request: request,lookup: { id in
            guard let i = request.rows.firstIndex(where: { $0.artifact == id }) else { return nil }
            return self.layouts[i]
        },account: accountHits)
        return RegionRaster(request,pixels,hits: hits,intent: intent,interaction: interaction)
    }
}

struct RegionVisibleWitness {
    let publication: UInt64
    let size: CGSize
    let scroll: CGPoint
    let profile: Data
    let scale: Int
    func matches(publication: UInt64, size: CGSize, scroll: CGPoint, profile: Data, scale: Int) -> Bool {
        self.publication == publication && self.size == size && self.scroll == scroll && self.profile == profile && self.scale == scale
    }
}

/// Display-only placement of one immutable accepted image. This is deliberately
/// not a RegionVisibleWitness and cannot certify fresh hits or raster acceptance.
struct RegionRetainedWitness: Equatable {
    let publication: UInt64
    let imageFrame: CGRect
    let coverage: CGRect
    let viewport: CGRect
    var coversViewport: Bool { coverage == viewport }
    init?(accepted a: RegionRasterRequest, current b: RegionRasterRequest,
          actualScroll: CGPoint, extent: CGSize, clip: CGRect) {
        guard a.bytes != nil, b.bytes != nil,
              a.publication == b.publication, a.generation == b.generation, a.rows == b.rows,
              a.scale == b.scale, a.profile == b.profile, a.format == b.format,
              a.background == b.background, a.selectionColor == b.selectionColor,
              actualScroll == b.scroll,
              extent.width.isFinite, extent.height.isFinite, extent.width >= 0, extent.height >= 0,
              b.scroll.x >= 0, b.scroll.y >= 0,
              b.scroll.x <= max(0,extent.width - b.size.width),
              b.scroll.y <= max(0,extent.height - b.size.height),
              !clip.isNull, !clip.isInfinite, clip.width > 0, clip.height > 0 else { return nil }
        let q = CGFloat(a.scale)
        let t = CGPoint(x: a.scroll.x - b.scroll.x,y: a.scroll.y - b.scroll.y)
        let edges = [t.x*q,t.y*q,clip.minX*q,clip.minY*q,clip.maxX*q,clip.maxY*q]
        guard edges.allSatisfy({ $0.isFinite && $0.rounded() == $0 }),
              (a.scroll.x*q).truncatingRemainder(dividingBy: 1) == (b.scroll.x*q).truncatingRemainder(dividingBy: 1),
              (a.scroll.y*q).truncatingRemainder(dividingBy: 1) == (b.scroll.y*q).truncatingRemainder(dividingBy: 1) else { return nil }
        let frame = CGRect(origin: t,size: a.size)
        let viewport = CGRect(origin: .zero,size: b.size)
        let coverage = frame.intersection(viewport).intersection(clip)
        guard !coverage.isNull, coverage.width > 0, coverage.height > 0 else { return nil }
        publication = a.publication; imageFrame = frame; self.coverage = coverage; self.viewport = viewport
    }
}
/// Returning to an old context is not publication. Only accepted fresh pixels
/// rearm a source/context invalidation; geometry-only pending keeps its owner.
struct RegionRetentionValidity {
    private(set) var valid = false
    mutating func invalidate() { valid = false }
    mutating func acceptedPixels() { valid = true }
}

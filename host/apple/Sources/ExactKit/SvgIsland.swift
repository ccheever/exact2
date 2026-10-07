// @ref LLP 1055.000 D2, D7, D10, §8 ruling 4 — SVG islands on Apple: what
// Core Animation cannot composite is rendered to pixels. The source is the
// same layers the scene already builds (a sub-scene, rendered by Core
// Animation's own `render(in:)`), so text, gradients and clips come along and
// antialias as the rest of Apple's SVG does. The pixel work after that (a
// mask's luminance, a filter's primitives) is `exact-svg-raster`, a separate
// module (`libexact_svg.dylib`) opened the first time an island needs it and
// never linked into ExactKit. A pattern needs no pixel work: Core Graphics
// tiles its rendered tile, so a pattern never loads the module.
import CoreGraphics
import Foundation
import QuartzCore

/// The island module (LLP 1047 D1's loaded tier). A plan that can show an
/// island (`exact_svg_islands`) has it opened off the main thread at boot
/// (`prewarm`); otherwise it opens synchronously at first use, so the frame
/// that first shows an island is late by the load (measured and logged) and
/// never shows the element without its effect.
final class SvgRasterModule {
    typealias Abi = @convention(c) () -> UInt32
    typealias Mask = @convention(c) (UnsafeMutablePointer<UInt8>?, Int, UInt8) -> Void
    typealias Filter = @convention(c) (UnsafePointer<Float>?, Int, UnsafeMutablePointer<UInt8>?, Int, Int, Float, Float, Float, Float) -> Int32
    typealias DocumentSize = @convention(c) (UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<UInt32>?, UnsafeMutablePointer<UInt32>?) -> Int32
    typealias DocumentRender = @convention(c) (UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<UInt8>?, UInt32, UInt32, Int, UInt32, UInt32) -> Int32
    /// The ABI this host speaks (`exact_svg_raster_abi`).
    static let abi: UInt32 = 2
    let mask: Mask
    let filter: Filter
    let documentSize: DocumentSize
    let documentRender: DocumentRender
    /// Milliseconds the load took.
    let loadMs: Double

    private init(_ library: UnsafeMutableRawPointer, ms: Double) {
        mask = unsafeBitCast(dlsym(library, "exact_svg_raster_mask")!, to: Mask.self)
        filter = unsafeBitCast(dlsym(library, "exact_svg_raster_filter")!, to: Filter.self)
        documentSize = unsafeBitCast(dlsym(library, "exact_svg_document_size")!, to: DocumentSize.self)
        documentRender = unsafeBitCast(dlsym(library, "exact_svg_document_render")!, to: DocumentRender.self)
        loadMs = ms
    }

    private static let path: String = {
        #if os(macOS)
        let standard = Bundle.main.executableURL!.deletingLastPathComponent().path + "/libexact_svg.dylib"
        #else
        let standard = embeddedModule(framework: "ExactSvg", dylib: "libexact_svg.dylib")
        #endif
        return ProcessInfo.processInfo.environment["EXACT_SVG_DYLIB"] ?? standard
    }()

    /// Main thread only: whether `prewarm` ran, and whether its load is done.
    private static var prewarming = false
    private static let prewarmed = DispatchSemaphore(value: 0)
    private static var waited = false

    /// Open the module on a background queue, once. The first open of a
    /// freshly installed file waits for the system's one-time check of it
    /// (110–480 ms measured on a Mac), and `dlopen` holds dyld's lock while
    /// it waits, so any other thread's `dlsym` or `dlopen` would wait too.
    /// An executable mapping of the file asks for that check first, outside
    /// the lock: the mapping is refused (the signature is not registered
    /// yet, as `dlopen` does before it maps), the verdict is kept, and the
    /// `dlopen` after it holds the lock 2–5 ms.
    static func prewarm() {
        guard !prewarming else { return }
        prewarming = true
        DispatchQueue.global(qos: .userInitiated).async {
            _ = shared
            prewarmed.signal()
        }
    }

    /// The module for an island, on the main thread: a prewarm still
    /// running is waited for (the wait is logged once).
    static var ready: SvgRasterModule? {
        if prewarming, !waited {
            waited = true
            let t0 = CFAbsoluteTimeGetCurrent()
            prewarmed.wait()
            let ms = (CFAbsoluteTimeGetCurrent() - t0) * 1000
            FileHandle.standardError.write(Data(String(format: "exact svg: the first island waited %.2f ms for the module\n", ms).utf8))
        }
        return shared
    }

    /// The raster workers open it on first SVG image use, never on the UI thread.
    static var imageDecoder: SvgRasterModule? { shared }

    /// The module, or `nil` (reported once, by name) when it cannot load.
    private static let shared: SvgRasterModule? = {
        // Images can be the first consumer without an island prewarm. Keep
        // the signature check outside dyld's lock on that worker path too.
        let checkStart = CFAbsoluteTimeGetCurrent()
        let fd = open(path, O_RDONLY)
        if fd >= 0 {
            if let p = mmap(nil, 16384, PROT_READ | PROT_EXEC, MAP_PRIVATE, fd, 0), p != MAP_FAILED { munmap(p, 16384) }
            close(fd)
        }
        let checkMs = (CFAbsoluteTimeGetCurrent() - checkStart) * 1000
        let t0 = CFAbsoluteTimeGetCurrent()
        let off = !Thread.isMainThread
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            FileHandle.standardError.write(Data("exact svg: the raster module is not loaded (\(String(cString: dlerror()))); SVG images, masks and filters cannot render\n".utf8))
            return nil
        }
        guard let abi = dlsym(library, "exact_svg_raster_abi"), dlsym(library, "exact_svg_raster_mask") != nil, dlsym(library, "exact_svg_raster_filter") != nil,
              dlsym(library, "exact_svg_document_size") != nil, dlsym(library, "exact_svg_document_render") != nil,
              unsafeBitCast(abi, to: Abi.self)() == SvgRasterModule.abi else {
            FileHandle.standardError.write(Data("exact svg: \(path) is not an exact SVG island module of ABI \(SvgRasterModule.abi)\n".utf8))
            dlclose(library)
            return nil
        }
        let ms = (CFAbsoluteTimeGetCurrent() - t0) * 1000
        FileHandle.standardError.write(Data(String(format: "exact svg: island module loaded in %.2f ms%@\n", ms, off ? String(format: " off the main thread, after a %.2f ms check", checkMs) : "").utf8))
        return SvgRasterModule(library, ms: ms)
    }()
}

private func num(_ v: Any?) -> Double { (v as? NSNumber)?.doubleValue ?? 0 }
private func nums(_ v: Any?) -> [Double] { (v as? [Any])?.map(num) ?? [] }
private func affine(_ v: Any?) -> CGAffineTransform {
    let t = nums(v)
    return t.count == 6 ? CGAffineTransform(a: t[0], b: t[1], c: t[2], d: t[3], tx: t[4], ty: t[5]) : .identity
}

/// A pattern's rendered tile, for Core Graphics' pattern callback.
private final class TileCell {
    let image: CGImage, rect: CGRect
    init(image: CGImage, rect: CGRect) { self.image = image; self.rect = rect }
}

enum SvgIsland {
    /// The most pixels one island's bitmap holds (64 MB at 4 bytes each).
    static let cap = 16_777_216
    /// The most bytes a filter island's work may hold at once: its bitmaps
    /// and the chain's working images (F1's map was 747 MB in the module's
    /// `f32` images for one drop shadow).
    static let budget = 128 << 20

    /// The part of `rect` (an island's space) an island renders, its size
    /// in pixels and its pixels per unit, at `k` pixels per unit when that
    /// is at most `cap` pixels, as before. Over `cap` (a huge filter region
    /// or mask at device scale), `rect` is first cut to `seen`, what can
    /// show of it (in the same space), on the full bitmap's own pixel grid,
    /// so the pixels kept are the full bitmap's; then, when that is still
    /// over `cap`, it is drawn at fewer pixels per unit and the layer scales
    /// it up, as a browser draws a huge effect at a lower resolution rather
    /// than not at all. `nil` when nothing of it can show.
    static func extent(_ rect: CGRect, k: CGFloat, seen: CGRect?, limit: Int = cap) -> (rect: CGRect, w: Int, h: Int, k: CGFloat)? {
        let cap = min(limit, SvgIsland.cap)
        let fw = (rect.width * k).rounded(.up), fh = (rect.height * k).rounded(.up)
        guard rect.width > 0, rect.height > 0, fw >= 1, fh >= 1, fw.isFinite, fh.isFinite else { return nil }
        if fw * fh <= CGFloat(cap) { return (rect, Int(fw), Int(fh), k) }
        var (r, w, h) = (rect, fw, fh)
        if let seen {
            let (sx, sy) = (fw / rect.width, fh / rect.height)
            let i0 = max(0, ((seen.minX - rect.minX) * sx).rounded(.down)), i1 = min(fw, ((seen.maxX - rect.minX) * sx).rounded(.up))
            let j0 = max(0, ((seen.minY - rect.minY) * sy).rounded(.down)), j1 = min(fh, ((seen.maxY - rect.minY) * sy).rounded(.up))
            guard i1 > i0, j1 > j0 else { return nil }
            r = CGRect(x: rect.minX + i0 / sx, y: rect.minY + j0 / sy, width: (i1 - i0) / sx, height: (j1 - j0) / sy)
            (w, h) = (i1 - i0, j1 - j0)
        }
        var scaled = k
        if w * h > CGFloat(cap) {
            let f = (CGFloat(cap) / (w * h)).squareRoot()
            (w, h) = (max(1, (w * f).rounded(.down)), max(1, (h * f).rounded(.down)))
            scaled = k * f
        }
        return (r, Int(w), Int(h), scaled)
    }

    /// A premultiplied sRGB bitmap of `els` (scene elements in a space `t`
    /// maps to the island's), covering `rect` of the island's space at
    /// `w` × `h` pixels, `k` per unit. `flip` puts the rect's top in the bitmap's first
    /// row, as a layer's contents; unflipped suits Core Graphics drawing.
    static func render(_ els: [Any], rect: CGRect, w: Int, h: Int, k: CGFloat, transform t: CGAffineTransform, flip: Bool,
                       dark: Bool, fonts: SvgText.Fonts?) -> CGContext? {
        guard rect.width > 0, rect.height > 0, w > 0, h > 0, w * h <= cap,
              let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        let sub = SvgScene()
        #if os(macOS)
        // Off screen, a Mac layer draws its contents images bottom up
        // unless its geometry is flipped as the on-screen tree's is.
        sub.root.isGeometryFlipped = true
        #endif
        sub.scale = k
        sub.fonts = fonts
        let m = t.concatenating(CGAffineTransform(translationX: -rect.minX, y: -rect.minY))
        sub.apply(["box": [0, 0, rect.width, rect.height], "t": [m.a, m.b, m.c, m.d, m.tx, m.ty], "els": els], dark: dark, clock: nil)
        if flip { ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: 1, y: -1) }
        ctx.scaleBy(x: CGFloat(w) / rect.width, y: CGFloat(h) / rect.height)
        sub.root.render(in: ctx)
        sub.reset()
        return ctx
    }

    /// A `mask` (LLP 1055.000 D10) as a layer to set as a mask: its content
    /// rendered over the region, turned into coverage by the module. `k` is
    /// device pixels per unit of the masked layer. Without the module the
    /// layer is empty, and masks everything away.
    static func mask(_ spec: [String: Any], k: CGFloat, seen: CGRect?, dark: Bool, fonts: SvgText.Fonts?) -> CALayer {
        // One gradient rectangle (a vignette, a fade) is Core Animation's
        // own gradient layer: no pixels of ours.
        if let gradient = SvgGradientMask.layer(spec, dark: dark) { return gradient }
        let layer = CALayer()
        layer.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull()]
        let r = nums(spec["r"])
        guard r.count == 4 else { return layer }
        let t = affine(spec["t"])
        guard let (rect, w, h, k) = extent(CGRect(x: r[0], y: r[1], width: r[2], height: r[3]).applying(t), k: k, seen: seen?.applying(t)) else {
            return layer
        }
        layer.anchorPoint = .zero
        layer.bounds = CGRect(origin: .zero, size: rect.size)
        layer.position = rect.origin
        guard let module = SvgRasterModule.ready,
              let ctx = render(spec["c"] as? [Any] ?? [], rect: rect, w: w, h: h, k: k, transform: t, flip: true, dark: dark, fonts: fonts),
              let data = ctx.data else { return layer }
        module.mask(data.assumingMemoryBound(to: UInt8.self), ctx.bytesPerRow * ctx.height, num(spec["l"]) != 0 ? 1 : 0)
        layer.contents = ctx.makeImage()
        return layer
    }

    /// Where a filter island goes and at what size (`extent`, bounded by
    /// `budget` at what its chain costs a pixel), its chain, and whether
    /// the GPU runs it.
    static func filterExtent(_ spec: [String: Any], k: CGFloat, seen: CGRect?)
        -> (rect: CGRect, w: Int, h: Int, k: CGFloat, program: [Float], gpu: Bool)? {
        let r = nums(spec["r"])
        guard r.count == 4 else { return nil }
        // What can show, widened by how far the chain reads (`rc`: units,
        // then pixels), so the pixels that show are the whole region's; a
        // chain that can read anywhere is never cut.
        let reach = nums(spec["rc"])
        let pad = reach.count == 2 ? reach[0] + (reach[1] + 2) / max(k, 1e-6) : 0
        let wide = reach.count == 2 ? seen?.insetBy(dx: -pad, dy: -pad) : nil
        let program = nums(spec["p"]).map(Float.init)
        // Bytes a pixel costs: the source and the result, and on the GPU
        // about two half-float intermediates; in the module a premultiplied
        // `f32` image per input, working copy and result of each primitive.
        let gpu = SvgFilterGPU.runs(program)
        let steps = program.count > 4 ? Int(max(1, program[4])) : 1
        let perPixel = gpu ? 24 : 8 + 16 * (3 + 3 * steps)
        guard let e = extent(CGRect(x: r[0], y: r[1], width: r[2], height: r[3]), k: k, seen: wide, limit: budget / perPixel) else { return nil }
        return (e.rect, e.w, e.h, e.k, program, gpu)
    }

    /// A filtered element's picture (LLP 1055.000 D14): the element without
    /// its effects rendered over the filter region at `k` pixels per user
    /// unit, run through the chain on the GPU (`SvgFilterGPU`) or else by
    /// the module, as a layer placed on the region. Without either the
    /// element draws nothing.
    static func filter(_ spec: [String: Any], k: CGFloat, seen: CGRect?, dark: Bool, fonts: SvgText.Fonts?) -> CALayer {
        let layer = CALayer()
        layer.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull()]
        guard let (rect, w, h, k, program, gpu) = filterExtent(spec, k: k, seen: seen) else { return layer }
        layer.anchorPoint = .zero
        layer.bounds = CGRect(origin: .zero, size: rect.size)
        layer.position = rect.origin
        guard let ctx = render(spec["c"] as? [Any] ?? [], rect: rect, w: w, h: h, k: k, transform: .identity, flip: true, dark: dark, fonts: fonts) else { return layer }
        if gpu, let source = ctx.makeImage(),
           let out = SvgFilterGPU.run(program, source: source, origin: rect.origin,
                                      scale: CGSize(width: CGFloat(w) / rect.width, height: CGFloat(h) / rect.height)) {
            layer.contents = out
            return layer
        }
        guard let module = SvgRasterModule.ready, let data = ctx.data else { return layer }
        let ok = program.withUnsafeBufferPointer { p in
            module.filter(p.baseAddress, p.count, data.assumingMemoryBound(to: UInt8.self), w, h,
                          Float(rect.minX), Float(rect.minY), Float(CGFloat(w) / rect.width), Float(CGFloat(h) / rect.height))
        }
        guard ok == 0 else { return layer }
        layer.contents = ctx.makeImage()
        return layer
    }

    /// Core Animation's names for `mix-blend-mode`, in CSS's order.
    private static let blendFilters = ["", "multiplyBlendMode", "screenBlendMode", "overlayBlendMode", "darkenBlendMode",
                                       "lightenBlendMode", "colorDodgeBlendMode", "colorBurnBlendMode", "hardLightBlendMode",
                                       "softLightBlendMode", "differenceBlendMode", "exclusionBlendMode", "hueBlendMode",
                                       "saturationBlendMode", "colorBlendMode", "luminosityBlendMode"]
    private static var saidBlend = false

    /// `mix-blend-mode` and `isolation` on an element's placed layer (LLP
    /// 1055.000 D19): a compositing filter on macOS. iOS has no public
    /// blend on a layer and declares it unsupported (§8 ruling 6): the
    /// element draws unblended and the host says so once.
    static func blend(_ layer: CALayer, mode: Int, isolate: Bool, scale: CGFloat) {
        #if os(macOS)
        let name = (1..<blendFilters.count).contains(mode) ? blendFilters[mode] : nil
        if (layer.compositingFilter as? String) != name { layer.compositingFilter = name }
        #else
        if mode != 0, !saidBlend {
            saidBlend = true
            FileHandle.standardError.write(Data("exact svg: `mix-blend-mode` is not supported on iOS (LLP 1055.000 §8 ruling 6); the element draws unblended\n".utf8))
        }
        #endif
        // An isolated group composites its content alone first.
        if layer.shouldRasterize != isolate {
            layer.shouldRasterize = isolate
            layer.rasterizationScale = scale
        }
    }

    /// A pattern paint (LLP 1055.000 D7) drawn over `rect` (the shape's
    /// user units) at `scale` pixels per unit: its tile rendered once at
    /// the scale it shows at, then tiled by Core Graphics under the
    /// pattern's transform.
    static func pattern(_ g: [String: Any], rect: CGRect, scale: CGFloat, dark: Bool, fonts: SvgText.Fonts?) -> CALayer? {
        let tile = nums(g["pt"])
        guard tile.count == 4 else { return nil }
        let t = affine(g["t"])
        let unit = sqrt(abs(t.a * t.d - t.b * t.c))
        let tileRect = CGRect(x: tile[0], y: tile[1], width: tile[2], height: tile[3])
        let area = rect.integral.insetBy(dx: -1, dy: -1)
        guard let (_, w, h, shown) = extent(area, k: scale, seen: nil),
              let (_, tw, th, tk) = extent(tileRect, k: max(shown * unit, 0.01), seen: nil),
              let tileCtx = render(g["c"] as? [Any] ?? [], rect: tileRect, w: tw, h: th, k: tk, transform: .identity, flip: false, dark: dark, fonts: fonts),
              let image = tileCtx.makeImage() else { return nil }
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: 1, y: -1)
        ctx.scaleBy(x: CGFloat(w) / area.width, y: CGFloat(h) / area.height)
        ctx.translateBy(x: -area.minX, y: -area.minY)
        // A Core Graphics pattern whose matrix is pattern space to the
        // bitmap's device space: its phase is the tile's origin in pattern
        // space, and one rendered cell repeats without seams.
        let cell = TileCell(image: image, rect: tileRect)
        var callbacks = CGPatternCallbacks(version: 0, drawPattern: { info, c in
            guard let info else { return }
            let cell = Unmanaged<TileCell>.fromOpaque(info).takeUnretainedValue()
            c.draw(cell.image, in: cell.rect)
        }, releaseInfo: nil)
        let made = withExtendedLifetime(cell) { () -> Bool in
            guard let pattern = CGPattern(info: Unmanaged.passUnretained(cell).toOpaque(), bounds: tileRect,
                                          matrix: t.concatenating(ctx.ctm), xStep: tileRect.width, yStep: tileRect.height,
                                          tiling: .constantSpacing, isColored: true, callbacks: &callbacks),
                  let space = CGColorSpace(patternBaseSpace: nil) else { return false }
            ctx.setFillColorSpace(space)
            var alpha = CGFloat(num(g["o"] ?? 1))
            ctx.setFillPattern(pattern, colorComponents: &alpha)
            ctx.fill(area)
            return true
        }
        guard made else { return nil }
        guard let out = ctx.makeImage() else { return nil }
        let layer = CALayer()
        layer.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull()]
        layer.contents = out
        layer.anchorPoint = .zero
        layer.bounds = area
        layer.position = area.origin
        return layer
    }
}

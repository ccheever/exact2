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

/// The island module (LLP 1047 D1's loaded tier): opened synchronously at
/// first use, so the frame that first shows an island is late by the load
/// (measured and logged) and never shows the element without its effect.
final class SvgRasterModule {
    typealias Abi = @convention(c) () -> UInt32
    typealias Mask = @convention(c) (UnsafeMutablePointer<UInt8>?, Int, UInt8) -> Void
    typealias Filter = @convention(c) (UnsafePointer<Float>?, Int, UnsafeMutablePointer<UInt8>?, Int, Int, Float, Float, Float, Float) -> Int32
    /// The ABI this host speaks (`exact_svg_raster_abi`).
    static let abi: UInt32 = 1
    let mask: Mask
    let filter: Filter
    /// Milliseconds the load took.
    let loadMs: Double

    private init(_ library: UnsafeMutableRawPointer, ms: Double) {
        mask = unsafeBitCast(dlsym(library, "exact_svg_raster_mask")!, to: Mask.self)
        filter = unsafeBitCast(dlsym(library, "exact_svg_raster_filter")!, to: Filter.self)
        loadMs = ms
    }

    /// The module, or `nil` (reported once, by name) when it cannot load.
    static let shared: SvgRasterModule? = {
        let t0 = CFAbsoluteTimeGetCurrent()
        #if os(macOS)
        let directory = Bundle.main.executableURL!.deletingLastPathComponent().path
        #else
        let directory = Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath
        #endif
        let path = ProcessInfo.processInfo.environment["EXACT_SVG_DYLIB"] ?? directory + "/libexact_svg.dylib"
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            FileHandle.standardError.write(Data("exact svg: the island module is not loaded (\(String(cString: dlerror()))); masks and filters draw nothing\n".utf8))
            return nil
        }
        guard let abi = dlsym(library, "exact_svg_raster_abi"), dlsym(library, "exact_svg_raster_mask") != nil, dlsym(library, "exact_svg_raster_filter") != nil,
              unsafeBitCast(abi, to: Abi.self)() == SvgRasterModule.abi else {
            FileHandle.standardError.write(Data("exact svg: \(path) is not an exact SVG island module of ABI \(SvgRasterModule.abi)\n".utf8))
            dlclose(library)
            return nil
        }
        let ms = (CFAbsoluteTimeGetCurrent() - t0) * 1000
        FileHandle.standardError.write(Data(String(format: "exact svg: island module loaded in %.2f ms\n", ms).utf8))
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
    /// A premultiplied sRGB bitmap of `els` (scene elements in a space `t`
    /// maps to the island's), covering `rect` of the island's space at `k`
    /// pixels per unit. `flip` puts the rect's top in the bitmap's first
    /// row, as a layer's contents; unflipped suits Core Graphics drawing.
    static func render(_ els: [Any], rect: CGRect, transform t: CGAffineTransform, k: CGFloat, flip: Bool,
                       dark: Bool, fonts: SvgText.Fonts?) -> CGContext? {
        let w = Int((rect.width * k).rounded(.up)), h = Int((rect.height * k).rounded(.up))
        guard rect.width > 0, rect.height > 0, w > 0, h > 0, w * h <= 16_777_216,
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
    static func mask(_ spec: [String: Any], k: CGFloat, dark: Bool, fonts: SvgText.Fonts?) -> CALayer {
        let layer = CALayer()
        layer.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull()]
        let r = nums(spec["r"])
        guard r.count == 4 else { return layer }
        let t = affine(spec["t"])
        let rect = CGRect(x: r[0], y: r[1], width: r[2], height: r[3]).applying(t)
        layer.anchorPoint = .zero
        layer.bounds = CGRect(origin: .zero, size: rect.size)
        layer.position = rect.origin
        guard let module = SvgRasterModule.shared,
              let ctx = render(spec["c"] as? [Any] ?? [], rect: rect, transform: t, k: k, flip: true, dark: dark, fonts: fonts),
              let data = ctx.data else { return layer }
        module.mask(data.assumingMemoryBound(to: UInt8.self), ctx.bytesPerRow * ctx.height, num(spec["l"]) != 0 ? 1 : 0)
        layer.contents = ctx.makeImage()
        return layer
    }

    /// A filtered element's picture (LLP 1055.000 D14): the element without
    /// its effects rendered over the filter region at `k` pixels per user
    /// unit, run through the chain by the module, as a layer placed on the
    /// region. Without the module the element draws nothing.
    static func filter(_ spec: [String: Any], k: CGFloat, dark: Bool, fonts: SvgText.Fonts?) -> CALayer {
        let layer = CALayer()
        layer.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull()]
        let r = nums(spec["r"])
        guard r.count == 4 else { return layer }
        let rect = CGRect(x: r[0], y: r[1], width: r[2], height: r[3])
        layer.anchorPoint = .zero
        layer.bounds = CGRect(origin: .zero, size: rect.size)
        layer.position = rect.origin
        let program = nums(spec["p"]).map(Float.init)
        guard let module = SvgRasterModule.shared,
              let ctx = render(spec["c"] as? [Any] ?? [], rect: rect, transform: .identity, k: k, flip: true, dark: dark, fonts: fonts),
              let data = ctx.data else { return layer }
        let (w, h) = (ctx.width, ctx.height)
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
        let k = max(scale * unit, 0.01)
        let tileRect = CGRect(x: tile[0], y: tile[1], width: tile[2], height: tile[3])
        guard let tileCtx = render(g["c"] as? [Any] ?? [], rect: tileRect, transform: .identity, k: k, flip: false, dark: dark, fonts: fonts),
              let image = tileCtx.makeImage() else { return nil }
        let area = rect.integral.insetBy(dx: -1, dy: -1)
        let w = Int((area.width * scale).rounded(.up)), h = Int((area.height * scale).rounded(.up))
        guard w > 0, h > 0, w * h <= 16_777_216, let space = CGColorSpace(name: CGColorSpace.sRGB),
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

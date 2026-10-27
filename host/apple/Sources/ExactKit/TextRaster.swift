// Immutable paragraph inputs and worker-confined CoreText paint, shared by Apple hosts.
// @ref LLP 1044.000 §5 items 2, 4, 7
import Foundation
import CoreText
import IOSurface

struct TextRasterKey: Hashable {
    let spec: Spec
    let size: CGSize
    let box: CGRect
    let scale: CGFloat
    var clip: CGRect? = nil
    func hash(into h: inout Hasher) {
        h.combine(spec); h.combine(size.width); h.combine(size.height)
        for v in [box.minX, box.minY, box.width, box.height, scale] { h.combine(v) }
        if let clip { for v in [clip.minX, clip.minY, clip.width, clip.height] { h.combine(v) } }
    }
}

struct TextRasterImage {
    #if os(iOS) || os(tvOS)
    let image: CGImage
    #else
    let surface: IOSurface
    #endif
    let frame: CGRect
    /// The part of the box these pixels answer for: the box and any ink past
    /// it, within a band's clip. `frame` is the same unless the job crops.
    let covered: CGRect
    /// The ink's peak: 0 for ordinary SDR, 1 for extended SDR storage,
    /// above 1 for HDR (LLP 1100 D8).
    var headroom: Float = 0
    /// An HDR `text-shadow` alone, the same size, for a layer under the ink's.
    #if os(iOS) || os(tvOS)
    var cast: CGImage? = nil
    #else
    var cast: IOSurface? = nil
    #endif
    var castHeadroom: Float = 0
    /// The one colour the paragraph paints in, when `image` is its coverage
    /// alone (one byte a pixel) for a layer to show in that colour
    /// (`TextRasterJob.tints`); nil when `image` holds the colours.
    var tint: CGColor? = nil
}

extension TextRasterImage {
    #if os(iOS) || os(tvOS)
    init(_ pixels: CGImage, frame: CGRect, covered: CGRect, headroom: Float) { self.init(image: pixels, frame: frame, covered: covered, headroom: headroom) }
    #else
    init(_ pixels: IOSurface, frame: CGRect, covered: CGRect, headroom: Float) { self.init(surface: pixels, frame: frame, covered: covered, headroom: headroom) }
    #endif
}

/// A raster's rectangle and, once aligned to physical pixel edges, its size
/// in pixels: counted from those edges, never rounded again from points.
struct TextRasterExtent {
    let rect: CGRect
    let pixels: CGSize?
}

/// Which way a tall paragraph's port travels: down, toward the end, until
/// it moves. Read from the port, not from its band, whose room is mostly
/// ahead of the port. It turns once the port comes back a quarter port from
/// the farthest it went, so a finger's jitter paints no new band.
struct PortTravel {
    private(set) var down = true
    private var farthest: CGFloat?
    mutating func follow(_ port: CGRect) -> Bool {
        let top = port.minY
        guard let farthest else { self.farthest = top; return down }
        if down ? top > farthest : top < farthest { self.farthest = top }
        else if abs(top - farthest) >= port.height / 4 { down.toggle(); self.farthest = top }
        return down
    }
}

struct TextRasterJob {
    var source: NSAttributedString
    let ranges: [CFRange]
    let baselines: [CGFloat]
    let flush: CGFloat
    /// CSS `text-align: justify`, at the box's width (`TextEngine.justified`).
    var justifies = false
    /// Where each line starts (`LineInsets`): `text-indent`, list indents.
    var insets = LineInsets()
    let box: CGRect
    let size: CGSize
    let scale: CGFloat
    var clip: CGRect? = nil
    /// CSS `text-overflow: ellipsis`: a line wider than the box ends in "…"
    /// as it paints (`Paragraph.ellipsized`, the draw path's).
    var ellipsis = false
    /// Pixels only where lines paint (their ink and line boxes), not the
    /// whole box: a label stretched across a row keeps a bitmap of its text.
    var crop = false
    /// CSS `line-clamp`: the last line's range as it broke, made again
    /// ending in "…" (`TextEngine.clampedLine`, as layout made it).
    var clamped: CFRange? = nil
    /// An HDR paragraph `text-shadow`, painted into `TextRasterImage.cast`:
    /// Core Animation doesn't map a layer shadow's color to the limit. Runs'
    /// own HDR shadows (`exactShadow`) are painted there too.
    var shadow: TextRunShadow? = nil
    /// Paint one-colour text as coverage alone, for a layer that colours it
    /// as it composites (`TextRasterImage.tint`): a quarter of the bytes,
    /// and Core Graphics' cheaper glyph path, as a `UILabel`'s backing store.
    var tints = false
    var measuredGlyphs: MeasuredGlyphs? = nil
    var measuredColor: CGColor? = nil

    static let maxInkOverflow: CGFloat = 256
    /// `input` within `limit` (the box and its ink allowance) and `clip`, out
    /// to whole device pixels. The box itself is kept as laid out. A span
    /// is `right - left` pixels: `(right / scale - left / scale) * scale`
    /// rounded up can be one more (787 pixels at 3x allocated 788, and the
    /// bitmap stretched to fit its frame).
    static func alignedExtent(_ input: CGRect, bounds: CGRect, limit: CGRect? = nil, clip: CGRect?, scale: CGFloat) -> TextRasterExtent {
        var rect = input.intersection(limit ?? bounds.insetBy(dx: -maxInkOverflow, dy: -maxInkOverflow))
        if let clip { rect = rect.intersection(clip) }
        guard rect != bounds, !rect.isNull else { return TextRasterExtent(rect: rect, pixels: nil) }
        let left = floor(rect.minX * scale), top = floor(rect.minY * scale)
        let right = ceil(rect.maxX * scale), bottom = ceil(rect.maxY * scale)
        return TextRasterExtent(
            rect: CGRect(x: left / scale, y: top / scale, width: (right - left) / scale, height: (bottom - top) / scale),
            pixels: CGSize(width: right - left, height: bottom - top))
    }
    /// How far past the box a run's own `text-shadow` may reach in these
    /// pixels (LLP 1077 D3): its offset plus 1.5× its blur, per side. Past
    /// it the shadow is cut, so a huge value cannot size an absurd bitmap:
    /// a 390 × 70 paragraph at 3× is at most 1414 × 1094 points, 56 MB,
    /// and a shadow cast 512 points to one side about 2 MB.
    static let maxShadowReach: CGFloat = 512
    private static let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
    private static let gray = CGColorSpaceCreateDeviceGray()
    private static let p3 = CGColorSpace(name: CGColorSpace.displayP3)!
    private static let extended = CGColorSpace(name: CGColorSpace.extendedSRGB)!
    /// The bitmap's space and its colours' peak (LLP 1100 D2, D8): extended
    /// sRGB (at half float) for a colour past SDR white, Display P3 for a
    /// wide one, else sRGB.
    private func format(_ runs: [CTRun]) -> (space: CGColorSpace, headroom: Float, deep: Bool) {
        var wide = false, deep = false, headroom: Float = 0
        // ColorSync round-off at gamut boundaries must not widen ordinary white.
        let outside = { (color: CGColor) in
            color.components?.prefix(3).contains { $0 < -0.00001 || $0 > 1.00001 } == true
        }
        for run in runs {
            let attributes = CTRunGetAttributes(run) as NSDictionary
            guard attributes[NSAttributedString.Key.exactHidden] as? Bool != true else { continue }
            for key in [NSAttributedString.Key.foregroundColor, .strokeColor, .underlineColor, .strikethroughColor, .exactBackground, .exactShadow] {
                let value = attributes[key]
                let cg = Self.paintColor(value)
                // A run's HDR shadow is in the cast, not these pixels.
                if value is TextRunShadow, ColorRange.isHDR(cg) { continue }
                headroom = max(headroom, ColorRange.headroom(cg))
                guard let cg else { continue }
                if let srgb = cg.converted(to: Self.extended, intent: .relativeColorimetric, options: nil) {
                    wide = wide || outside(srgb)
                }
                if let p3 = CGColorSpace(name: CGColorSpace.extendedDisplayP3),
                   let p3Color = cg.converted(to: p3, intent: .relativeColorimetric, options: nil) {
                    deep = deep || outside(p3Color)
                }
            }
        }
        return headroom > 1 || deep ? (Self.extended, max(1, headroom), true) : (wide ? Self.p3 : Self.srgb, 0, false)
    }

    /// A tall paragraph's band around `port`: 32 points past the box each
    /// side, and as far as the shadows in its pixels reach sideways (LLP
    /// 1077 D3, at most `maxShadowReach`), as tall as `maximumBytes` allows.
    /// `down`: the port travels that way (true, toward the paragraph's end),
    /// so three quarters of the room is ahead of it; nil centres it.
    static func band(_ spec: Spec, width: CGFloat, port: CGRect, scale: CGFloat, maximumBytes: CGFloat, down: Bool? = nil) -> CGRect {
        let reach = spec.shadowReach
        let wide = width + 64 + reach.left + reach.right
        let rowBytes = max(1, wide * scale * scale * 4)
        let height = max(port.height, min(port.height * 2, maximumBytes / rowBytes))
        let behind = (height - port.height) * (down.map { $0 ? 0.25 : 0.75 } ?? 0.5)
        return CGRect(x: -32 - reach.left, y: max(-32, port.minY - behind), width: wide, height: height)
    }
    /// Whether a band `clip` that holds `port` still serves it, the port
    /// travelling `down` or up in a paragraph `height` tall: until the port
    /// is within half a port of the band's edge ahead, short of the end.
    static func keeps(_ clip: CGRect, port: CGRect, down: Bool, height: CGFloat) -> Bool {
        let ahead = down ? clip.maxY - port.maxY : port.minY - clip.minY
        return (down ? clip.maxY >= height : clip.minY <= 0) || ahead >= port.height / 2
    }
    func render(lines reused: [CTLine]? = nil) -> TextRasterImage? {
        if let measuredGlyphs, let measuredColor, !ellipsis, clamped == nil, !justifies, shadow == nil,
           ColorRange.headroom(measuredColor) <= 1 && !ColorRange.needsExtendedComponents(measuredColor) {
            return render(measuredGlyphs, color: measuredColor)
        }
        var lines: [CTLine]
        if let reused {
            precondition(Thread.isMainThread, "cached lines stay on their owning thread")
            lines = reused
        } else {
            let typesetter = CTTypesetterCreateWithAttributedString(source)
            lines = ranges.map {
                let inset = insets.at($0.location).width
                return TextEngine.finishedLine(CTTypesetterCreateLine(typesetter, $0), source: source, range: $0,
                                               justify: justifies ? Double(box.width - inset) : nil)
            }
        }
        // First pixels reuse plain range lines; apply the clamp on both paths.
        if let clamped, !lines.isEmpty {
            let range = NSRange(location: clamped.location, length: clamped.length)
            lines[lines.count - 1] = TextEngine.clampedLine(source, range: range, width: Double(box.width)) ?? lines[lines.count - 1]
        }
        let positions = zip(lines, baselines).map { line, baseline in
            let inset = insets.at(CTLineGetStringRange(line).location)
            return CGPoint(x: box.minX + inset.left + TextEngine.penOffset(line, flush: flush, room: Double(box.width - inset.width), rtl: insets.rtl),
                           y: box.minY + TextEngine.paintedBaseline(baseline, scale: scale))
        }
        if ellipsis {
            lines = lines.map { line in
                guard CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)) - CTLineGetTrailingWhitespaceWidth(line) > box.width + 0.5 else { return line }
                let range = CTLineGetStringRange(line)
                return TextEngine.ellipsis(line, range: NSRange(location: range.location, length: range.length),
                                           width: Double(box.width), source: source) ?? line
            }
        }
        // CSS line boxes size layout, not ink. Tight line heights and italic
        // overhang can paint beyond any edge; include that ink in the bitmap.
        let bounds = CGRect(origin: .zero, size: size)
        var painted = CGRect.null
        // Ink may pass the box by `maxInkOverflow`; runs' shadows further.
        var limit = bounds.insetBy(dx: -Self.maxInkOverflow, dy: -Self.maxInkOverflow)
        for (line, position) in zip(lines, positions) {
            let ink = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
            if !ink.isNull, !ink.isEmpty {
                let glyphs = CGRect(x: position.x + ink.minX, y: position.y - ink.maxY, width: ink.width, height: ink.height)
                painted = painted.union(glyphs.insetBy(dx: -1 / scale, dy: -1 / scale))
                // Runs' own shadows are in these pixels (LLP 1077 D3).
                var shadows = TextRunShadow.reach(line, ink: glyphs)
                if let s = shadow {
                    let spread = s.blur * 1.5 + 1
                    shadows = shadows.union(glyphs.offsetBy(dx: s.offset.width, dy: s.offset.height).insetBy(dx: -spread, dy: -spread))
                }
                if !shadows.isNull {
                    painted = painted.union(shadows)
                    let most = bounds.insetBy(dx: -Self.maxShadowReach, dy: -Self.maxShadowReach)
                    limit = limit.union(shadows.intersection(most))
                }
            }
            if crop {
                // The line box too: decorations paint in it, outside the glyphs.
                var ascent: CGFloat = 0, descent: CGFloat = 0
                let width = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, nil))
                if width > 0 { painted = painted.union(CGRect(x: position.x, y: position.y - ascent, width: width, height: ascent + descent)) }
            }
            for fill in TextLinePaint.backgrounds(line, at: position) { painted = painted.union(fill.rect) }
        }
        func aligned(_ r: CGRect) -> TextRasterExtent {
            Self.alignedExtent(r, bounds: bounds, limit: limit, clip: clip, scale: scale)
        }
        let coveredExtent = aligned(painted.isNull ? bounds : bounds.union(painted))
        var frameExtent = coveredExtent
        if crop, !painted.isNull {
            let ink = aligned(painted)
            if !ink.rect.isNull, !ink.rect.isEmpty { frameExtent = ink }
        }
        let covered = coveredExtent.rect, frame = frameExtent.rect
        let pixelWidth = frameExtent.pixels?.width ?? (frame.width * scale).rounded(.up)
        let pixelHeight = frameExtent.pixels?.height ?? (frame.height * scale).rounded(.up)
        guard pixelWidth.isFinite, pixelHeight.isFinite,
              pixelWidth > 0, pixelHeight > 0,
              pixelWidth < CGFloat(Int.max), pixelHeight < CGFloat(Int.max) else { return nil }
        let width = Int(pixelWidth), height = Int(pixelHeight)
        // Truncation inserts runs whose paint need not occur in the source.
        let runs = lines.flatMap { CTLineGetGlyphRuns($0) as! [CTRun] }
        let (space, headroom, deep) = format(runs)
        if let tint = tints && headroom == 0 && !deep ? oneColor(runs) : nil {
            guard let mask = bitmap(width: width, height: height, space: Self.srgb, headroom: 0, deep: false, alphaOnly: true, paint: {
                paint(lines, positions, frame: frame, height: height, scale: scale, pass: .ink, into: $0)
            }) else { return nil }
            var out = TextRasterImage(mask.pixels, frame: frame, covered: covered, headroom: 0)
            // The colour's alpha is in the coverage already.
            out.tint = tint.copy(alpha: 1)
            return out
        }
        // Runs' own HDR shadows, each once.
        var casters: [TextRunShadow] = []
        for run in runs where (CTRunGetAttributes(run) as NSDictionary)[NSAttributedString.Key.exactHidden] as? Bool != true {
            if let s = TextRunShadow.of(run), ColorRange.isHDR(s.color), !casters.contains(s) { casters.append(s) }
        }
        guard let ink = bitmap(width: width, height: height, space: space, headroom: headroom, deep: deep, keepsImage: shadow != nil, paint: {
            paint(lines, positions, frame: frame, height: height, scale: scale, pass: .ink, into: $0)
        }) else { return nil }
        var out = TextRasterImage(ink.pixels, frame: frame, covered: covered, headroom: headroom)
        // What casts each HDR shadow: the ink itself for the paragraph's,
        // the glyphs of its runs for a run's own.
        var casts: [(CGImage, TextRunShadow)] = []
        if let shadow, let image = ink.image { casts.append((image, shadow)) }
        for s in casters {
            let glyphs = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: Self.srgb,
                                   bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            if let glyphs { paint(lines, positions, frame: frame, height: height, scale: scale, pass: .caster(s), into: glyphs) }
            if let image = glyphs?.makeImage() { casts.append((image, s)) }
        }
        if !casts.isEmpty {
            let castHeadroom = casts.reduce(Float(1)) { max($0, ColorRange.headroom($1.1.color)) }
            // Each caster is drawn once, a bitmap's width to the left, and its
            // shadow moved back in: one shadow under all its glyphs, and none
            // of its ink. Pixels, y up.
            out.cast = bitmap(width: width, height: height, space: Self.extended, headroom: castHeadroom, deep: true, paint: { ctx in
                for (image, s) in casts {
                    ctx.saveGState()
                    ctx.setShadow(offset: CGSize(width: s.offset.width * scale + CGFloat(width), height: -s.offset.height * scale),
                                  blur: s.blur * scale, color: s.color)
                    ctx.draw(image, in: CGRect(x: -width, y: 0, width: width, height: height))
                    ctx.restoreGState()
                }
            })?.pixels
            out.castHeadroom = castHeadroom
        }
        return out
    }

    /// Paint the owner's glyph values without constructing another typesetter.
    private func render(_ glyphs: MeasuredGlyphs, color: CGColor) -> TextRasterImage? {
        guard glyphs.lines.count == baselines.count else { return nil }
        let bounds = CGRect(origin: .zero, size: size)
        let positions = zip(glyphs.lines, baselines).map {
            CGPoint(x: box.minX + $0.origin, y: box.minY + TextEngine.paintedBaseline($1, scale: scale))
        }
        var painted = CGRect.null
        for (line, position) in zip(glyphs.lines, positions) {
            let ink = line.ink
            if !ink.isNull, !ink.isEmpty {
                painted = painted.union(CGRect(x: position.x + ink.minX, y: position.y - ink.maxY,
                    width: ink.width, height: ink.height).insetBy(dx: -1 / scale, dy: -1 / scale))
            }
            if crop, line.box.width > 0 { painted = painted.union(line.box.offsetBy(dx: position.x, dy: position.y)) }
        }
        let covered = Self.alignedExtent(painted.isNull ? bounds : bounds.union(painted), bounds: bounds, clip: clip, scale: scale)
        var extent = covered
        if crop, !painted.isNull {
            let ink = Self.alignedExtent(painted, bounds: bounds, clip: clip, scale: scale)
            if !ink.rect.isNull, !ink.rect.isEmpty { extent = ink }
        }
        let frame = extent.rect
        let w = extent.pixels?.width ?? (frame.width * scale).rounded(.up)
        let h = extent.pixels?.height ?? (frame.height * scale).rounded(.up)
        guard w.isFinite, h.isFinite, w > 0, h > 0, w < CGFloat(Int.max), h < CGFloat(Int.max) else { return nil }
        let width = Int(w), height = Int(h)
        // The fast path admits sRGB ink; wide/HDR colors retain the full paint path.
        guard let pixels = bitmap(width: width, height: height, space: Self.srgb, headroom: 0, deep: false, alphaOnly: tints, paint: { ctx in
            ctx.translateBy(x: 0, y: CGFloat(height))
            ctx.scaleBy(x: scale, y: -scale)
            ctx.translateBy(x: -frame.minX, y: -frame.minY)
            ctx.setShouldSmoothFonts(true)
            ctx.setFillColor(color)
            for (line, position) in zip(glyphs.lines, positions) {
                ctx.saveGState()
                ctx.translateBy(x: position.x, y: position.y)
                ctx.scaleBy(x: 1, y: -1)
                ctx.textMatrix = .identity
                ctx.textPosition = .zero
                for run in line.runs where !run.glyphs.isEmpty {
                    CTFontDrawGlyphs(run.font, run.glyphs, run.positions, run.glyphs.count, ctx)
                }
                ctx.restoreGState()
            }
            ctx.flush()
        }) else { return nil }
        var out = TextRasterImage(pixels.pixels, frame: frame, covered: covered.rect, headroom: 0)
        if tints { out.tint = color.copy(alpha: 1) }
        return out
    }

    /// The colour every glyph and decoration paints in, when nothing else in
    /// the paragraph has one (no stroke, inline background, run shadow or
    /// decoration of another colour) and no glyph brings its own (an emoji's).
    private func oneColor(_ runs: [CTRun]) -> CGColor? {
        var color: CGColor?
        for run in runs {
            let attributes = CTRunGetAttributes(run) as NSDictionary
            if attributes[NSAttributedString.Key.exactHidden] as? Bool == true { continue }
            let ink = Self.paintColor(attributes[NSAttributedString.Key.foregroundColor])
            guard let ink, color == nil || color == ink,
                  attributes[NSAttributedString.Key.exactBackground] == nil, attributes[NSAttributedString.Key.exactShadow] == nil,
                  attributes[NSAttributedString.Key.shadow] == nil, attributes[NSAttributedString.Key.strokeColor] == nil,
                  ((attributes[NSAttributedString.Key.strokeWidth] as? NSNumber)?.doubleValue ?? 0) == 0,
                  [NSAttributedString.Key.underlineColor, .strikethroughColor].allSatisfy({ key in
                      attributes[key].map { Self.paintColor($0) == ink } ?? true
                  })
            else { return nil }
            color = ink
            let value = attributes[kCTFontAttributeName] as CFTypeRef?
            if let value, CFGetTypeID(value) == CTFontGetTypeID(),
               CTFontGetSymbolicTraits(value as! CTFont).contains(.traitColorGlyphs) { return nil }
        }
        return color
    }

    private static func paintColor(_ value: Any?) -> CGColor? {
        if let color = (value as? InlineBackground)?.color ?? (value as? TextRunShadow)?.color ?? (value as? PlatformColor)?.cgColor { return color }
        guard let value else { return nil }
        let ref = value as CFTypeRef
        return CFGetTypeID(ref) == CGColor.typeID ? (ref as! CGColor) : nil
    }

    #if os(iOS) || os(tvOS)
    private typealias Pixels = CGImage
    #else
    private typealias Pixels = IOSurface
    #endif

    /// One bitmap: `deep` is half float in extended sRGB, tagged with its
    /// headroom; `alphaOnly` one byte a pixel, coverage alone; else four
    /// bytes a pixel. `keepsImage`: the pixels as an image too, to draw from.
    private func bitmap(width: Int, height: Int, space: CGColorSpace, headroom: Float, deep: Bool, alphaOnly: Bool = false,
                        keepsImage: Bool = false, paint: (CGContext) -> Void) -> (pixels: Pixels, image: CGImage?)? {
        let pixelBytes = deep ? 8 : alphaOnly ? 1 : 4
        let bitmapInfo = deep
            ? CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder16Little.rawValue
            : alphaOnly ? CGImageAlphaInfo.alphaOnly.rawValue
            : CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        #if os(macOS)
        guard width > 0, height > 0,
              let surface = IOSurface(properties: [.width: width, .height: height, .bytesPerElement: pixelBytes,
                                                   .pixelFormat: UInt32(deep ? 0x52476841 : 0x42475241)]) // 'RGhA' : 'BGRA'
        else { return nil }
        surface.lock(options: [], seed: nil)
        defer {
            surface.unlock(options: [], seed: nil)
            if let profile = space.copyPropertyList() { IOSurfaceSetValue(surface, kIOSurfaceColorSpace, profile) }
            if deep, #available(macOS 15, *) { IOSurfaceSetValue(surface, kIOSurfaceContentHeadroom, NSNumber(value: headroom)) }
        }
        guard let ctx = CGContext(data: surface.baseAddress, width: width, height: height, bitsPerComponent: deep ? 16 : 8,
                                  bytesPerRow: surface.bytesPerRow, space: space, bitmapInfo: bitmapInfo)
        else { return nil }
        paint(ctx)
        return (surface, keepsImage ? ctx.makeImage() : nil)
        #else
        // UIKit accepts a CGImage. Paint into a scratch bitmap and keep Core
        // Graphics' copy of it (`makeImage`), as ImageIO does for a decoded
        // image: Core Animation shares that memory with the render server as
        // it is. A context's own buffer would stay the image's copy-on-write
        // storage, and be copied again at the layer's first commit.
        // Rows padded to 64 bytes, as Core Animation needs to take them as they are.
        let (row, rowOverflow) = (width * pixelBytes).addingReportingOverflow(63)
        let (bytes, overflow) = (row & ~63).multipliedReportingOverflow(by: height)
        guard !rowOverflow, !overflow else { return nil }
        return withScratch(bytes) { scratch -> (pixels: Pixels, image: CGImage?)? in
            guard let ctx = CGContext(data: scratch, width: width, height: height, bitsPerComponent: deep ? 16 : 8,
                                      bytesPerRow: row & ~63, space: alphaOnly ? Self.gray : space, bitmapInfo: bitmapInfo)
            else { return nil }
            paint(ctx)
            guard var image = ctx.makeImage() else { return nil }
            if deep, #available(iOS 18, tvOS 18, *), let tagged = CGImageCreateCopyWithContentHeadroom(headroom, image) { image = tagged }
            return (image, image)
        }
        #endif
    }

    #if os(macOS)
    /// A raster's headroom, as `render` tagged its surface, or 0.
    static func headroom(of surface: IOSurface) -> Float {
        // Float text always needs extended presentation, even on macOS 14,
        // before IOSurface has a standard content-headroom key.
        let storage: Float = IOSurfaceGetBytesPerElement(surface) == 8 ? 1 : 0
        guard #available(macOS 15, *) else { return storage }
        return max(storage, (IOSurfaceCopyValue(surface, kIOSurfaceContentHeadroom) as? NSNumber)?.floatValue ?? 0)
    }
    #endif

    private func paint(_ lines: [CTLine], _ positions: [CGPoint], frame: CGRect, height: Int, scale: CGFloat, pass: TextLinePaint.Pass, into ctx: CGContext) {
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: scale, y: -scale)
        ctx.translateBy(x: -frame.minX, y: -frame.minY)
        ctx.setShouldSmoothFonts(true)
        for (line, position) in zip(lines, positions) { TextLinePaint.draw(line, at: position, in: ctx, scale: scale, pass: pass) }
        ctx.flush()
    }
}

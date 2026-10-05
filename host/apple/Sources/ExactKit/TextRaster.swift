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
    /// The ink's peak over SDR white, or 0 for SDR ink (LLP 1100 D8).
    var headroom: Float = 0
}

struct TextRasterJob {
    let source: NSAttributedString
    let ranges: [CFRange]
    let baselines: [CGFloat]
    let flush: CGFloat
    /// CSS `text-align: justify`, at the box's width (`TextEngine.justified`).
    var justifies = false
    /// The paragraph's first line's `text-indent` (`Spec.firstLineInset`).
    var firstLineInset: (left: CGFloat, width: CGFloat) = (0, 0)
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

    static let maxInkOverflow: CGFloat = 256
    /// How far past the box a run's own `text-shadow` may reach in these
    /// pixels (LLP 1077 D3): its offset plus 1.5× its blur, per side. Past
    /// it the shadow is cut, so a huge value cannot size an absurd bitmap:
    /// a 390 × 70 paragraph at 3× is at most 1414 × 1094 points, 56 MB,
    /// and a shadow cast 512 points to one side about 2 MB.
    static let maxShadowReach: CGFloat = 512
    private static let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
    private static let p3 = CGColorSpace(name: CGColorSpace.displayP3)!
    private static let extended = CGColorSpace(name: CGColorSpace.extendedSRGB)!
    /// The bitmap's space and its colours' peak (LLP 1100 D2, D8): extended
    /// sRGB (at half float) for a colour past SDR white, Display P3 for a
    /// wide one, else sRGB.
    private var format: (space: CGColorSpace, headroom: Float, deep: Bool) {
        var wide = false, deep = false, headroom: Float = 0
        // ColorSync round-off at gamut boundaries must not widen ordinary white.
        let outside = { (color: CGColor) in
            color.components?.prefix(3).contains { $0 < -0.00001 || $0 > 1.00001 } == true
        }
        let all = NSRange(location: 0, length: source.length)
        for key in [NSAttributedString.Key.foregroundColor, .strokeColor, .exactBackground, .exactShadow] {
            source.enumerateAttribute(key, in: all) { value, _, _ in
                let ref = value.map { $0 as CFTypeRef }
                let cg = (value as? InlineBackground)?.color ?? (value as? TextRunShadow)?.color ?? (value as? PlatformColor)?.cgColor
                    ?? ref.flatMap { CFGetTypeID($0) == CGColor.typeID ? ($0 as! CGColor) : nil }
                headroom = max(headroom, ColorRange.headroom(cg))
                guard let cg else { return }
                if let srgb = cg.converted(to: Self.extended, intent: .relativeColorimetric, options: nil) {
                    wide = wide || outside(srgb)
                }
                if let p3 = CGColorSpace(name: CGColorSpace.extendedDisplayP3),
                   let converted = cg.converted(to: p3, intent: .relativeColorimetric, options: nil) {
                    deep = deep || outside(converted)
                }
            }
        }
        return headroom > 1 || deep ? (Self.extended, max(1, headroom), true) : (wide ? Self.p3 : Self.srgb, 0, false)
    }

    /// A tall paragraph's band around `port`: 32 points past the box each
    /// side, and as far as runs' own shadows reach sideways (LLP 1077 D3,
    /// at most `maxShadowReach`), as tall as `maximumBytes` allows.
    static func band(_ spec: Spec, width: CGFloat, port: CGRect, scale: CGFloat, maximumBytes: CGFloat) -> CGRect {
        let reach = spec.runShadowReach
        let wide = width + 64 + reach.left + reach.right
        let rowBytes = max(1, wide * scale * scale * 4)
        let height = max(port.height, min(port.height * 2, maximumBytes / rowBytes))
        return CGRect(x: -32 - reach.left, y: max(-32, port.minY - (height - port.height) / 2), width: wide, height: height)
    }
    func render(lines reused: [CTLine]? = nil) -> TextRasterImage? {
        var lines: [CTLine]
        if let reused {
            precondition(Thread.isMainThread, "cached lines stay on their owning thread")
            lines = reused
        } else {
            let typesetter = CTTypesetterCreateWithAttributedString(source)
            lines = ranges.map {
                let inset = $0.location == 0 ? firstLineInset.width : 0
                return TextEngine.finishedLine(CTTypesetterCreateLine(typesetter, $0), source: source, range: $0,
                                               justify: justifies ? Double(box.width - inset) : nil)
            }
            if let clamped, !lines.isEmpty {
                let range = NSRange(location: clamped.location, length: clamped.length)
                lines[lines.count - 1] = TextEngine.clampedLine(source, range: range, width: Double(box.width)) ?? lines[lines.count - 1]
            }
        }
        let positions = zip(lines, baselines).map { line, baseline in
            let inset: (left: CGFloat, width: CGFloat) = CTLineGetStringRange(line).location == 0 ? firstLineInset : (0, 0)
            return CGPoint(x: box.minX + inset.left + CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(box.width - inset.width))),
                           y: box.minY + baseline.rounded())
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
                let shadows = TextRunShadow.reach(line, ink: glyphs)
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
            for (fill, _) in TextLinePaint.backgrounds(line, at: position) { painted = painted.union(fill) }
        }
        func aligned(_ r: CGRect) -> CGRect {
            var r = r.intersection(limit)
            if let clip { r = r.intersection(clip) }
            guard r != bounds, !r.isNull else { return r }
            let left = floor(r.minX * scale) / scale
            let top = floor(r.minY * scale) / scale
            return CGRect(x: left, y: top, width: ceil(r.maxX * scale) / scale - left,
                          height: ceil(r.maxY * scale) / scale - top)
        }
        let covered = aligned(painted.isNull ? bounds : bounds.union(painted))
        var frame = covered
        if crop, !painted.isNull {
            let ink = aligned(painted)
            if !ink.isNull, !ink.isEmpty { frame = ink }
        }
        let pixelWidth = (frame.width * scale).rounded(.up)
        let pixelHeight = (frame.height * scale).rounded(.up)
        guard pixelWidth.isFinite, pixelHeight.isFinite,
              pixelWidth > 0, pixelHeight > 0,
              pixelWidth < CGFloat(Int.max), pixelHeight < CGFloat(Int.max) else { return nil }
        let width = Int(pixelWidth), height = Int(pixelHeight)
        let (space, headroom, deep) = format
        let hdr = headroom > 1
        let pixelBytes = deep ? 8 : 4
        let bitmapInfo = deep
            ? CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder16Little.rawValue
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
            if hdr, #available(macOS 15, *) { IOSurfaceSetValue(surface, kIOSurfaceContentHeadroom, NSNumber(value: headroom)) }
        }
        guard let ctx = CGContext(data: surface.baseAddress, width: width, height: height, bitsPerComponent: deep ? 16 : 8,
                                  bytesPerRow: surface.bytesPerRow, space: space, bitmapInfo: bitmapInfo)
        else { return nil }
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
        return withScratch(bytes) { scratch -> TextRasterImage? in
            guard let ctx = CGContext(data: scratch, width: width, height: height, bitsPerComponent: deep ? 16 : 8,
                                      bytesPerRow: row & ~63, space: space, bitmapInfo: bitmapInfo)
            else { return nil }
            paint(lines, positions, frame: frame, height: height, scale: scale, into: ctx)
            guard var image = ctx.makeImage() else { return nil }
            if hdr, #available(iOS 18, tvOS 18, *), let tagged = CGImageCreateCopyWithContentHeadroom(headroom, image) { image = tagged }
            return TextRasterImage(image: image, frame: frame, covered: covered, headroom: headroom)
        }
        #endif
        #if os(macOS)
        paint(lines, positions, frame: frame, height: height, scale: scale, into: ctx)
        return TextRasterImage(surface: surface, frame: frame, covered: covered, headroom: headroom)
        #endif
    }

    #if os(macOS)
    /// A raster's headroom, as `render` tagged its surface, or 0.
    static func headroom(of surface: IOSurface) -> Float {
        guard #available(macOS 15, *) else { return 0 }
        return (IOSurfaceCopyValue(surface, kIOSurfaceContentHeadroom) as? NSNumber)?.floatValue ?? 0
    }
    #endif

    private func paint(_ lines: [CTLine], _ positions: [CGPoint], frame: CGRect, height: Int, scale: CGFloat, into ctx: CGContext) {
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: scale, y: -scale)
        ctx.translateBy(x: -frame.minX, y: -frame.minY)
        ctx.setShouldSmoothFonts(true)
        for (line, position) in zip(lines, positions) { TextLinePaint.draw(line, at: position, in: ctx, scale: scale) }
        ctx.flush()
    }
}

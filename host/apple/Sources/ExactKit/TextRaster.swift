// Immutable paragraph inputs and worker-confined CoreText paint, shared by Apple hosts.
// @ref LLP 1044.000 §5 items 2, 4, 7
import Foundation
import CoreText
import IOSurface

struct TextRasterKey: Equatable {
    let spec: Spec
    let size: CGSize
    let box: CGRect
    let scale: CGFloat
    var clip: CGRect? = nil
}

struct TextRasterImage {
    #if os(iOS)
    let image: CGImage
    #else
    let surface: IOSurface
    #endif
    let frame: CGRect
}

struct TextRasterJob {
    let source: NSAttributedString
    let ranges: [CFRange]
    let baselines: [CGFloat]
    let flush: CGFloat
    let box: CGRect
    let size: CGSize
    let scale: CGFloat
    var clip: CGRect? = nil

    static let maxInkOverflow: CGFloat = 256
    private static let space = CGColorSpace(name: CGColorSpace.sRGB)!
    func render(lines reused: [CTLine]? = nil) -> TextRasterImage? {
        let lines: [CTLine]
        if let reused {
            precondition(Thread.isMainThread, "cached lines stay on their owning thread")
            lines = reused
        } else {
            let typesetter = CTTypesetterCreateWithAttributedString(source)
            lines = ranges.map { CTTypesetterCreateLine(typesetter, $0) }
        }
        let positions = zip(lines, baselines).map { line, baseline in
            CGPoint(x: box.minX + CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(box.width))),
                    y: box.minY + baseline.rounded())
        }
        // CSS line boxes size layout, not ink. Tight line heights and italic
        // overhang can paint beyond any edge; include that ink in the bitmap.
        let bounds = CGRect(origin: .zero, size: size)
        var frame = bounds
        for (line, position) in zip(lines, positions) {
            let ink = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
            if !ink.isNull, !ink.isEmpty {
                frame = frame.union(CGRect(x: position.x + ink.minX, y: position.y - ink.maxY,
                                           width: ink.width, height: ink.height).insetBy(dx: -1 / scale, dy: -1 / scale))
            }
        }
        frame = frame.intersection(bounds.insetBy(dx: -Self.maxInkOverflow, dy: -Self.maxInkOverflow))
        if let clip { frame = frame.intersection(clip) }
        if frame != bounds {
            let left = floor(frame.minX * scale) / scale
            let top = floor(frame.minY * scale) / scale
            frame = CGRect(x: left, y: top, width: ceil(frame.maxX * scale) / scale - left,
                           height: ceil(frame.maxY * scale) / scale - top)
        }
        let pixelWidth = (frame.width * scale).rounded(.up)
        let pixelHeight = (frame.height * scale).rounded(.up)
        guard pixelWidth.isFinite, pixelHeight.isFinite,
              pixelWidth > 0, pixelHeight > 0,
              pixelWidth < CGFloat(Int.max), pixelHeight < CGFloat(Int.max) else { return nil }
        let width = Int(pixelWidth), height = Int(pixelHeight)
        #if os(macOS)
        guard width > 0, height > 0,
              let surface = IOSurface(properties: [.width: width, .height: height, .bytesPerElement: 4,
                                                   .pixelFormat: UInt32(0x42475241)]) // 'BGRA'
        else { return nil }
        surface.lock(options: [], seed: nil)
        defer {
            surface.unlock(options: [], seed: nil)
            if let profile = Self.space.copyPropertyList() { IOSurfaceSetValue(surface, kIOSurfaceColorSpace, profile) }
        }
        guard let ctx = CGContext(data: surface.baseAddress, width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: surface.bytesPerRow, space: Self.space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        #else
        // UIKit accepts a CGImage. Let Core Graphics own its storage; an image
        // made from an externally backed IOSurface must not outlive that surface.
        guard let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: width * 4, space: Self.space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        #endif
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: scale, y: -scale)
        ctx.translateBy(x: -frame.minX, y: -frame.minY)
        ctx.setShouldSmoothFonts(true)
        ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        for (line, position) in zip(lines, positions) {
            ctx.textPosition = position
            CTLineDraw(line, ctx)
        }
        ctx.flush()
        #if os(iOS)
        guard let image = ctx.makeImage() else { return nil }
        return TextRasterImage(image: image, frame: frame)
        #else
        return TextRasterImage(surface: surface, frame: frame)
        #endif
    }
}

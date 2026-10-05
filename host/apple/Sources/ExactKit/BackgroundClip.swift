// CSS `background-clip` (LLP 1077 D6), on both Apple platforms: the
// background colour and image painted within the border box (initial), the
// padding box, the content box, or the glyphs of the node's own paragraph.
// Anything but the border box draws (`draw(_:)`); `text` also keeps the
// paragraph off its raster, so the glyph outlines that clip the background
// are the same lines the paragraph paints.
import CoreGraphics
import CoreText
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    var backgroundClip: String { style["background_clip"]?.string ?? "border-box" }

    /// The outline the background paints inside; nil for `text`, whose
    /// background paints with the glyphs (`paintBackgroundThroughText`).
    func backgroundOutline(_ border: CGPath) -> CGPath? {
        let clip = backgroundClip
        if clip == "text" { return nil }
        guard clip == "padding-box" || clip == "content-box" else { return border }
        let uniform = number("border_width")
        var inset = ["top", "right", "bottom", "left"].map { number("border_width_" + $0, uniform) }
        if clip == "content-box" {
            inset = zip(inset, ["top", "right", "bottom", "left"].map { number("padding_" + $0) }).map { $0 + $1 }
        }
        let inner = CGRect(x: inset[3], y: inset[0], width: max(0, bounds.width - inset[1] - inset[3]),
                           height: max(0, bounds.height - inset[0] - inset[2]))
        let outer = BorderPaint.reduced(BorderPaint.radii(style, in: bounds), in: bounds)
        let radii = [CGSize(width: outer[0].width - inset[3], height: outer[0].height - inset[0]),
                     CGSize(width: outer[1].width - inset[1], height: outer[1].height - inset[0]),
                     CGSize(width: outer[2].width - inset[1], height: outer[2].height - inset[2]),
                     CGSize(width: outer[3].width - inset[3], height: outer[3].height - inset[2])]
            .map { CGSize(width: max(0, $0.width), height: max(0, $0.height)) }
        return BorderPaint.roundedRect(inner, radii, shape: CornerShape(style["corner_shape"]))
    }

    /// The background colour (when `color`) and gradients within the
    /// clip's outline.
    func paintBackground(_ ctx: CGContext, border: CGPath, color: Bool) {
        guard let clip = backgroundOutline(border) else { return }
        if color, let c = cgColor("background_color"), c.alpha > 0 {
            ctx.saveGState()
            ctx.addPath(clip)
            ctx.setFillColor(c)
            ctx.fillPath()
            ctx.restoreGState()
        }
        paintGradient(ctx, clip: clip)
    }

    #if os(macOS)
    /// `draw(_:)`'s background colour and gradients, within the clip.
    func drawBackground(_ path: NSBezierPath, rounded: Bool) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        if backgroundClip != "border-box" {
            if surface == nil { paintBackground(ctx, border: path.cgPath, color: true) }
            return
        }
        let bg = color("background_color", .clear)
        if bg.alphaComponent > 0, surface == nil {
            bg.setFill()
            if rounded { path.fill() } else { ctx.fill(bounds) }
        }
        if style["background_image"] != nil { paintGradient(ctx, clip: path.cgPath) }
    }
    #endif

    /// `background-clip: text`: the background colour and gradients inside
    /// the paragraph's glyphs, before the glyphs themselves paint.
    func paintBackgroundThroughText(_ ctx: CGContext, paragraph: Paragraph, spec: Spec, in box: CGRect) {
        guard backgroundClip == "text" else { return }
        let glyphs = TextEngine.glyphPath(paragraph, spec: spec, in: box)
        guard !glyphs.isEmpty else { return }
        ctx.saveGState()
        ctx.addPath(glyphs)
        ctx.clip()
        if let c = channels("background_color"), c[3] > 0 {
            ctx.setFillColor(TextEngine.color(c).cgColor)
            ctx.fill(bounds)
        }
        paintGradient(ctx, clip: CGPath(rect: bounds, transform: nil))
        ctx.restoreGState()
    }
}

extension TextEngine {
    /// The outlines of a paragraph's glyphs where `draw` paints them, in
    /// the view's y-down space (LLP 1077 D6).
    static func glyphPath(_ p: Paragraph, spec: Spec, in bounds: CGRect) -> CGPath {
        let path = CGMutablePath()
        for index in p.lines.indices {
            let line = spec.ellipsis ? p.ellipsized(index, spec: spec, width: bounds.width) : p.lines[index]
            let origin = CGPoint(x: bounds.minX + p.origin(index, align: spec.align, width: bounds.width),
                                 y: bounds.minY + p.baselines[index].rounded())
            for run in (CTLineGetGlyphRuns(line) as? [CTRun]) ?? [] {
                let count = CTRunGetGlyphCount(run)
                guard count > 0, let font = (CTRunGetAttributes(run) as? [CFString: Any])?[kCTFontAttributeName] else { continue }
                var glyphs = [CGGlyph](repeating: 0, count: count), positions = [CGPoint](repeating: .zero, count: count)
                CTRunGetGlyphs(run, CFRange(location: 0, length: count), &glyphs)
                CTRunGetPositions(run, CFRange(location: 0, length: count), &positions)
                for i in 0..<count {
                    // Glyph outlines are y up from the baseline.
                    var t = CGAffineTransform(translationX: origin.x + positions[i].x, y: origin.y - positions[i].y).scaledBy(x: 1, y: -1)
                    if let g = CTFontCreatePathForGlyph(font as! CTFont, glyphs[i], &t) { path.addPath(g) }
                }
            }
        }
        return path
    }

}

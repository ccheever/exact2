// Paragraph text rasterized off the main thread. @ref LLP 1044 F4, LLP 1008 §3
//
// AppKit paints a layer-backed view only where it is visible, so a paragraph
// scrolling into view is painted in strips: a backing buffer, a display-list
// replay and a texture upload per paragraph per frame, on the main thread, in
// the frame that has to move — half of what that thread did during a scroll.
// Here a paragraph's text is drawn once, whole, on a worker, into a surface the
// view's layer shows. The main thread mounts the row; the pixels arrive behind it,
// while the row is still a screen away. Scrolling mounted text is compositing.
//
// `NodeView.draw` still paints text for everything this declines: a
// selection, a capture, a canvas, a decorated text box, a clamp, and a
// paragraph too tall to hold as one bitmap.
#if os(macOS)
import AppKit
import CoreText
import IOSurface

/// What one raster was made from. A paragraph whose key is unchanged keeps
/// its image; a worker's answer for a key no longer wanted is dropped.
struct TextRasterKey: Equatable {
    let spec: Spec
    let size: CGSize
    let box: CGRect
    let scale: CGFloat
}

private struct TextRasterImage {
    let surface: IOSurface
    let frame: CGRect
}

final class TextRasterizer {
    /// Points. A taller paragraph is left to AppKit's strips: one bitmap of it
    /// would be tens of megabytes to show a screenful.
    static let maxHeight: CGFloat = 4096

    /// Everything a worker needs, none of it shared with the main thread's
    /// layout: CoreText line objects belong to one thread at a time, so the
    /// worker typesets its own from the same source and the same breaks.
    private struct Job {
        let source: NSAttributedString
        let ranges: [CFRange]
        let baselines: [CGFloat]
        let flush: CGFloat
        let box: CGRect
        let size: CGSize
        let scale: CGFloat
    }

    private let queue = DispatchQueue(label: "exact.text-raster", qos: .userInitiated, attributes: .concurrent)
    private static let space = CGColorSpace(name: CGColorSpace.sRGB)!
    // A slice limits admission rate, not outstanding work. Keep no backlog:
    // the next pump tries the still-nearby paragraphs again when a worker is
    // free, so navigation and resize cannot queue obsolete document pixels.
    private var active = 0
    private static let maxConcurrent = 2

    /// Make sure `node` shows a current raster. `urgent` means it is on
    /// screen now: painted here rather than shown blank for a frame.
    @discardableResult
    func ensure(_ node: NodeView, urgent: Bool) -> Bool {
        guard node.rastersText, let engine = node.text else {
            node.dropTextRaster()
            return true
        }
        let spec = node.paragraphSpec()
        let scale = node.window?.backingScaleFactor ?? 2
        let key = TextRasterKey(spec: spec, size: node.bounds.size, box: node.contentBox(), scale: scale)
        if node.textRasterKey == key, node.textRasterReady || !urgent {
            if node.textRasterReady && node.textRasterPending { node.presentTextRaster() }
            return true
        }
        guard urgent || active < Self.maxConcurrent else { return false }
        // The breaks the kernel measured at this width, when they are still
        // resident: the worker typesets its own lines, so the painted
        // paragraph need not exist on this thread until something reads it.
        let measured = engine.measuredBreaks(spec, width: key.box.width)
        let paragraph = measured == nil ? node.paragraphLayout() : nil
        guard let (ranges, baselines) = measured
                ?? paragraph.map({ ($0.lines.map { CTLineGetStringRange($0) }, $0.baselines) }) else {
            node.dropTextRaster()
            return true
        }
        node.textRasterKey = key
        node.textRasterReady = false
        node.textRasterPending = false
        let reused = urgent && paragraph == nil ? engine.rasterLines(spec, ranges: ranges) : nil
        let source = paragraph?.shape?.attributed ?? reused?.0 ?? engine.attributed(spec)
        let job = Job(source: source.copy() as! NSAttributedString,
                      ranges: ranges, baselines: baselines,
                      flush: spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0,
                      box: key.box, size: key.size, scale: scale)
        if urgent {
            let image = Self.render(job, lines: paragraph?.lines ?? reused?.1)
            node.showTextRaster(image?.surface, for: key, frame: image?.frame)
            return true
        }
        active += 1
        queue.async { [weak self, weak node] in
            let image = Self.render(job)
            DispatchQueue.main.async { [weak self, weak node] in
                self?.active -= 1
                node?.showTextRaster(image?.surface, for: key, frame: image?.frame, deferOffscreen: true)
            }
        }
        return true
    }

    /// The same paint `TextEngine.draw` makes — a y-down context, one
    /// `CTLineDraw` per line, baselines rounded to points — into an sRGB
    /// IOSurface. A surface is what the render server composites: a CGImage
    /// would be converted and copied for it on the main thread, at commit.
    /// Existing lines are supplied only synchronously on their owning thread.
    /// Workers create their own lines from source and ranges.
    private static func render(_ job: Job, lines existingLines: [CTLine]? = nil) -> TextRasterImage? {
        let lines: [CTLine]
        if let existingLines {
            lines = existingLines
        } else {
            let typesetter = CTTypesetterCreateWithAttributedString(job.source)
            lines = job.ranges.map { CTTypesetterCreateLine(typesetter, $0) }
        }
        let positions = zip(lines, job.baselines).map { line, baseline in
            CGPoint(x: job.box.minX + CGFloat(CTLineGetPenOffsetForFlush(line, job.flush, Double(job.box.width))),
                    y: job.box.minY + baseline.rounded())
        }
        // CSS line boxes size layout, not ink. Tight line heights and italic
        // overhang can paint beyond any edge; include that ink in the bitmap.
        let box = CGRect(origin: .zero, size: job.size)
        var frame = box
        for (line, position) in zip(lines, positions) {
            let ink = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
            if !ink.isNull, !ink.isEmpty {
                frame = frame.union(CGRect(x: position.x + ink.minX, y: position.y - ink.maxY,
                                           width: ink.width, height: ink.height).insetBy(dx: -1 / job.scale, dy: -1 / job.scale))
            }
        }
        if frame != box {
            let left = floor(frame.minX * job.scale) / job.scale
            let top = floor(frame.minY * job.scale) / job.scale
            frame = CGRect(x: left, y: top, width: ceil(frame.maxX * job.scale) / job.scale - left,
                           height: ceil(frame.maxY * job.scale) / job.scale - top)
        }
        let width = Int((frame.width * job.scale).rounded(.up)), height = Int((frame.height * job.scale).rounded(.up))
        guard width > 0, height > 0,
              let surface = IOSurface(properties: [.width: width, .height: height, .bytesPerElement: 4,
                                                   .pixelFormat: UInt32(0x42475241)]) // 'BGRA'
        else { return nil }
        surface.lock(options: [], seed: nil)
        defer {
            surface.unlock(options: [], seed: nil)
            if let profile = space.copyPropertyList() { IOSurfaceSetValue(surface, kIOSurfaceColorSpace, profile) }
        }
        guard let ctx = CGContext(data: surface.baseAddress, width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: surface.bytesPerRow, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: job.scale, y: -job.scale)
        ctx.translateBy(x: -frame.minX, y: -frame.minY)
        ctx.setShouldSmoothFonts(true)
        ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        for (line, position) in zip(lines, positions) {
            ctx.textPosition = position
            CTLineDraw(line, ctx)
        }
        ctx.flush()
        return TextRasterImage(surface: surface, frame: frame)
    }
}

extension NodeView {
    /// Whether this paragraph's text is a rasterized surface rather than
    /// something `draw` paints. Asked by AppKit through `wantsUpdateLayer`.
    var rastersText: Bool {
        guard kind == "text", isParagraph, flowShapes.isEmpty, !hasBoxPaint, !Capture.capturing, window != nil,
              bounds.width > 0, bounds.height > 0, bounds.height <= TextRasterizer.maxHeight,
              number("line_clamp") == 0, canvasAbove == nil, let presenter else { return false }
        if presenter.session?.regions.owns(self) == true { return false }
        if presenter.selection.isActive, let selected = presenter.selection.range(self), selected.length > 0 { return false }
        return true
    }

    /// Whether the pump still owes this paragraph pixels.
    var needsTextRaster: Bool { !textRasterReady || textRasterKey == nil || textRasterPending }

    /// The box a raster was painted for is gone — the layer would stretch its
    /// surface to whatever the paragraph is now. Retire the key so the pump
    /// asks a worker for pixels at the new geometry. AppKit's own dirty flag
    /// cannot be that record: a paragraph resized while it is off screen is
    /// not redrawn there, and an `ensure` that finds every worker busy
    /// declines after `updateLayer` has already cleared the flag. The old
    /// pixels stay up until the new ones replace them, as `invalidateText`
    /// leaves them for a changed paragraph.
    func textRasterGeometryChanged() {
        guard let key = textRasterKey, key.size != bounds.size || key.box != contentBox() else { return }
        textRasterKey = nil
        textRasterPending = false
    }

    func showTextRaster(_ image: IOSurface?, for key: TextRasterKey, frame: CGRect? = nil, deferOffscreen: Bool = false) {
        // An urgent paint can overtake its worker. Keep the accepted surface
        // instead of committing identical pixels again when that worker ends.
        guard textRasterKey == key, !textRasterReady, let image else { return }
        textRaster = image
        textRasterScale = key.scale
        textRasterFrame = frame ?? CGRect(origin: .zero, size: key.size)
        textRasterReady = true
        guard rastersText else { return }
        if deferOffscreen, let presenter, !presenter.textIsVisible(self) {
            textRasterPending = true
            presenter.requestTextPublication()
        } else { presentTextRaster() }
    }

    /// Fitting ink uses the view's contents. Overflow ink needs a positioned
    /// sublayer so it can escape the layout box, subject to authored clipping.
    func presentTextRaster() {
        guard let layer, let surface = textRaster else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        if textRasterFrame == CGRect(origin: .zero, size: bounds.size) {
            textRasterOverflowLayer?.removeFromSuperlayer()
            textRasterOverflowLayer = nil
            layer.contentsScale = textRasterScale
            layer.contentsGravity = .resize
            layer.contents = surface
        } else {
            layer.contents = nil
            let ink = textRasterOverflowLayer ?? CALayer()
            if ink.superlayer == nil { layer.addSublayer(ink) }
            textRasterOverflowLayer = ink
            ink.frame = textRasterFrame
            ink.contentsScale = textRasterScale
            ink.contentsGravity = .resize
            ink.contents = surface
        }
        textRasterPending = false
        CATransaction.commit()
    }

    func dropTextRaster() {
        textRasterOverflowLayer?.removeFromSuperlayer()
        textRasterOverflowLayer = nil
        if textRaster != nil, wantsUpdateLayer { layer?.contents = nil }
        textRaster = nil
        textRasterKey = nil
        textRasterReady = false
        textRasterPending = false
    }
}
#endif

// Paragraph text rasterized off the main thread. @ref LLP 1044 F4, LLP 1008 §3
//
// AppKit paints a layer-backed view only where it is visible, so a paragraph
// scrolling into view is painted in strips: a backing buffer, a display-list
// replay and a texture upload per paragraph per frame, on the main thread, in
// the frame that has to move — half of what that thread did during a scroll.
// Here a paragraph's text is drawn once, whole, on a worker, into an image a
// sublayer shows. The main thread mounts the row; the pixels arrive behind it,
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

    /// Make sure `node` shows a current raster. `urgent` means it is on
    /// screen now: painted here rather than shown blank for a frame.
    func ensure(_ node: NodeView, urgent: Bool) {
        guard node.rastersText, let engine = node.text else {
            node.dropTextRaster()
            return
        }
        let spec = node.paragraphSpec()
        let scale = node.window?.backingScaleFactor ?? 2
        let key = TextRasterKey(spec: spec, size: node.bounds.size, box: node.contentBox(), scale: scale)
        if node.textRasterKey == key, node.textRasterReady || !urgent { return }
        // The breaks the kernel measured at this width, when they are still
        // resident: the worker typesets its own lines, so the painted
        // paragraph need not exist on this thread until something reads it.
        guard let (ranges, baselines) = engine.measuredBreaks(spec, width: key.box.width)
                ?? node.paragraphLayout().map({ ($0.lines.map { CTLineGetStringRange($0) }, $0.baselines) }) else {
            node.dropTextRaster()
            return
        }
        node.textRasterKey = key
        node.textRasterReady = false
        let job = Job(source: engine.attributed(spec).copy() as! NSAttributedString,
                      ranges: ranges, baselines: baselines,
                      flush: spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0,
                      box: key.box, size: key.size, scale: scale)
        if urgent {
            node.showTextRaster(Self.render(job), for: key)
            return
        }
        queue.async {
            let image = Self.render(job)
            DispatchQueue.main.async { [weak node] in node?.showTextRaster(image, for: key) }
        }
    }

    /// The same paint `TextEngine.draw` makes — a y-down context, one
    /// `CTLineDraw` per line, baselines rounded to points — into an sRGB
    /// IOSurface. A surface is what the render server composites: a CGImage
    /// would be converted and copied for it on the main thread, at commit.
    private static func render(_ job: Job) -> IOSurface? {
        let width = Int((job.size.width * job.scale).rounded(.up)), height = Int((job.size.height * job.scale).rounded(.up))
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
        ctx.setShouldSmoothFonts(true)
        ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        let typesetter = CTTypesetterCreateWithAttributedString(job.source)
        for (range, baseline) in zip(job.ranges, job.baselines) {
            let line = CTTypesetterCreateLine(typesetter, range)
            let x = CGFloat(CTLineGetPenOffsetForFlush(line, job.flush, Double(job.box.width)))
            ctx.textPosition = CGPoint(x: job.box.minX + x, y: job.box.minY + baseline.rounded())
            CTLineDraw(line, ctx)
        }
        ctx.flush()
        return surface
    }
}

extension NodeView {
    /// Whether this paragraph's text is a rasterized sublayer rather than
    /// something `draw` paints. Asked by AppKit through `wantsUpdateLayer`.
    var rastersText: Bool {
        guard kind == "text", isParagraph, !hasBoxPaint, !Capture.capturing, window != nil,
              bounds.width > 0, bounds.height > 0, bounds.height <= TextRasterizer.maxHeight,
              number("line_clamp") == 0, canvasAbove == nil, let presenter else { return false }
        if presenter.session?.regions.owns(self) == true { return false }
        if presenter.selection.isActive, let selected = presenter.selection.range(self), selected.length > 0 { return false }
        return true
    }

    /// Whether the pump still owes this paragraph pixels.
    var needsTextRaster: Bool { !textRasterReady || textRasterKey == nil }

    func showTextRaster(_ image: IOSurface?, for key: TextRasterKey) {
        guard textRasterKey == key, let image, let host = layer else { return }
        let target: CALayer
        if let existing = textRaster { target = existing } else {
            target = CALayer()
            // A row's pixels arrive; they do not fade in or slide.
            target.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull(), "hidden": NSNull(), "onOrderIn": NSNull(), "onOrderOut": NSNull()]
            target.anchorPoint = .zero
            target.contentsGravity = .resize
            host.addSublayer(target)
            textRaster = target
        }
        target.contentsScale = key.scale
        target.bounds = CGRect(origin: .zero, size: key.size)
        target.position = .zero
        target.contents = image
        target.isHidden = !rastersText
        textRasterReady = true
    }

    /// `draw` is painting this paragraph's text itself (a selection, a capture).
    func hideTextRaster() { textRaster?.isHidden = true }

    func dropTextRaster() {
        textRaster?.removeFromSuperlayer()
        textRaster = nil
        textRasterKey = nil
        textRasterReady = false
    }
}
#endif

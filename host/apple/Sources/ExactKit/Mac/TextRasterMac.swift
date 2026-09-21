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

final class TextRasterizer {
    /// Points. A taller paragraph is left to AppKit's strips: one bitmap of it
    /// would be tens of megabytes to show a screenful.
    static let maxHeight: CGFloat = 4096
    // Keep italic overhang and ink outside tight line boxes, but never size
    // a surface to an unbreakable line's potentially unbounded advance.
    static let maxInkOverflow = TextRasterJob.maxInkOverflow

    // A slice limits admission rate, not outstanding work. Keep no backlog:
    // the next pump tries the still-nearby paragraphs again when a worker is
    // free, so navigation and resize cannot queue obsolete document pixels.
    private var active = 0
    private static let maxConcurrent = 2

    private func key(_ node: NodeView) -> TextRasterKey {
        TextRasterKey(spec: node.paragraphSpec(), size: node.bounds.size,
                      box: node.contentBox(), scale: node.window?.backingScaleFactor ?? 2)
    }

    private func prepare(_ node: NodeView, key: TextRasterKey) -> TextRasterJob? {
        guard let engine = node.text else { return nil }
        let measured = engine.measuredBreaks(key.spec, width: key.box.width)
        let paragraph = measured == nil ? node.paragraphLayout() : nil
        guard let (ranges, baselines) = measured
                ?? paragraph.map({ ($0.lines.map { CTLineGetStringRange($0) }, $0.baselines) }) else { return nil }
        node.textRasterKey = key
        node.textRasterReady = false
        node.textRasterFailed = false
        node.textRasterPending = false
        let source = paragraph?.shape?.attributed ?? engine.attributed(key.spec)
        return TextRasterJob(source: source.copy() as! NSAttributedString,
                   ranges: ranges, baselines: baselines,
                   flush: key.spec.align == 1 ? 0.5 : key.spec.align == 2 ? 1 : 0,
                   box: key.box, size: key.size, scale: key.scale)
    }

    /// Only first pixels may rasterize synchronously. A replacement uses the
    /// visible batch below, or a worker if an AppKit display pass gets here first.
    @discardableResult
    func ensure(_ node: NodeView, urgent: Bool) -> Bool {
        if node.textRasterFailed, node.textRasterKey != nil { return true }
        guard node.rastersText else { return true }
        let key = key(node)
        if node.textRasterKey == key, node.textRasterReady || !urgent || node.textRaster != nil {
            if node.textRasterReady && node.textRasterPending { node.presentTextRaster() }
            return true
        }
        let firstPixels = urgent && node.textRaster == nil && !node.textRasterUsesStrips
        guard firstPixels || active < Self.maxConcurrent else { return false }
        guard let job = prepare(node, key: key) else { node.dropTextRaster(); return true }
        if firstPixels {
            let image = Self.render(job, firstPixels: true)
            node.showTextRaster(image?.surface, for: key, frame: image?.frame)
            return true
        }
        if urgent { node.useTextStrips() }
        active += 1
        RegionTextExecutor.queue.addOperation { [weak self, weak node] in
            let image = Self.render(job)
            DispatchQueue.main.async { [weak self, weak node] in
                self?.active -= 1
                node?.showTextRaster(image?.surface, for: key, frame: image?.frame, deferOffscreen: true)
            }
        }
        return true
    }

    // One visible replacement set at a time, on the existing raster workers.
    // The main thread reads results only after both workers leave the group.
    private final class Replacement {
        let group = DispatchGroup()
        let lock = NSLock()
        var images: [Int: TextRasterImage] = [:]
        var published = false
        func put(_ image: TextRasterImage?, at index: Int) {
            lock.lock(); defer { lock.unlock() }
            images[index] = image
        }
    }
    private var replacing = false

    /// New geometry and its pixels become visible in one transaction. Spend
    /// at most a quarter frame waiting; on timeout AppKit draws the new-width
    /// visible strips in this display pass. No stale line breaks cross a frame.
    /// A live resize skips full rasters when at least half their pixels are hidden.
    @discardableResult
    func replaceVisible(_ nodes: [NodeView], wait: TimeInterval) -> Bool {
        var jobs: [(NodeView, TextRasterKey, TextRasterJob)] = []
        var deferred = false
        for node in nodes where node.needsTextRaster && node.canRasterText && (node.textRaster != nil || node.textRasterUsesStrips) {
            let key = key(node)
            if node.textRasterKey == key {
                if node.textRasterReady && node.textRasterPending { node.presentTextRaster() }
                continue
            }
            let visible = node.presenter?.textIsVisible(node) == true
            guard visible else { continue }
            let port = node.presenter?.textScrollportRect(node) ?? node.visibleRect
            if replacing || active > 0 || (node.inLiveResize && port.height < node.bounds.height * 0.5) {
                node.useTextStrips()
                deferred = true
                continue
            }
            if let job = prepare(node, key: key) { jobs.append((node, key, job)) }
        }
        guard !jobs.isEmpty else { return deferred }
        replacing = true
        let result = Replacement()
        let work = jobs
        let workers = min(Self.maxConcurrent, jobs.count)
        active += workers
        // Two bounded lanes, no job per paragraph queued behind old widths.
        for lane in 0..<workers {
            result.group.enter()
            RegionTextExecutor.queue.addOperation {
                for index in stride(from: lane, to: work.count, by: workers) {
                    result.put(Self.render(work[index].2), at: index)
                }
                result.group.leave()
            }
        }
        let publish = { [weak self] in
            guard !result.published else { return }
            result.published = true
            self?.active -= workers
            self?.replacing = false
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            for (index, entry) in jobs.enumerated() {
                let image = result.images[index]
                entry.0.showTextRaster(image?.surface, for: entry.1, frame: image?.frame)
            }
            CATransaction.commit()
            // A later width may have superseded this entire group.
            jobs.first?.0.presenter?.requestTextPublication()
        }
        if result.group.wait(timeout: .now() + max(0, wait)) == .success {
            publish()
        } else {
            for (node, _, _) in jobs { node.useTextStrips() }
            result.group.notify(queue: .main, execute: publish)
        }
        return deferred
    }

    /// The same paint `TextEngine.draw` makes — a y-down context, one
    /// `CTLineDraw` per line, baselines rounded to points — into an sRGB
    /// IOSurface. A surface is what the render server composites: a CGImage
    /// would be converted and copied for it on the main thread, at commit.
    /// Workers create their own lines from source and ranges.
    private static func render(_ job: TextRasterJob, firstPixels: Bool = false) -> TextRasterImage? {
        assert(!Thread.isMainThread || firstPixels, "replacement rasterization belongs to workers")
        return job.render()
    }
}

extension NodeView {
    /// Whether this paragraph's text is a rasterized surface rather than
    /// something `draw` paints. Asked by AppKit through `wantsUpdateLayer`.
    var rastersText: Bool { canRasterText && !textRasterUsesStrips }

    var canRasterText: Bool {
        if textRasterFailed, textRasterKey != nil { return false }
        guard readerParagraph == nil, kind == "text", isParagraph, flowShapes.isEmpty, !hasBoxPaint, !Capture.capturing, window != nil,
              bounds.width > 0, bounds.height > 0, bounds.height <= TextRasterizer.maxHeight,
              number("line_clamp") == 0, canvasAbove == nil, let presenter else { return false }
        if presenter.session?.regions.owns(self) == true { return false }
        if presenter.selection.isActive, let selected = presenter.selection.range(self), selected.length > 0 { return false }
        return true
    }

    /// Whether the pump still owes this paragraph pixels.
    var needsTextRaster: Bool {
        if textRasterFailed, textRasterKey != nil { return false }
        return !textRasterReady || textRasterKey == nil || textRasterPending
    }

    /// The box a raster was painted for is gone — the layer would stretch its
    /// surface to whatever the paragraph is now. Retire the key so the pump
    /// asks a worker for pixels at the new geometry. AppKit's own dirty flag
    /// cannot be that record: a paragraph resized while it is off screen is
    /// not redrawn there, and an `ensure` that finds every worker busy
    /// declines after `updateLayer` has already cleared the flag. The old
    /// pixels stay up until the new ones replace them, as `invalidateText`
    /// leaves them for a changed paragraph.
    func textRasterGeometryChanged() {
        if let key = textRasterKey, key.size == bounds.size && key.box == contentBox() { return }
        textRasterKey = nil
        textRasterPending = false
        // Position the accepted surface at its original dimensions immediately;
        // the node's new frame must never stretch old glyphs.
        if textRaster != nil { presentTextRaster() }
    }

    func useTextStrips() {
        textRasterUsesStrips = true
        needsDisplay = true
    }

    func showTextRaster(_ image: IOSurface?, for key: TextRasterKey, frame: CGRect? = nil, deferOffscreen: Bool = false) {
        // An urgent paint can overtake its worker. Keep the accepted surface
        // instead of committing identical pixels again when that worker ends.
        guard textRasterKey == key, !textRasterReady else { return }
        guard let image else {
            dropTextRaster()
            textRasterKey = key
            textRasterFailed = true
            needsDisplay = true
            // A synchronous failure can occur inside updateLayer, whose dirty
            // flag AppKit is about to clear. Ask again after that display pass,
            // now through draw. A late successful worker can still replace it.
            DispatchQueue.main.async { [weak self] in
                guard let self, self.textRasterFailed, self.textRasterKey == key else { return }
                self.needsDisplay = true
            }
            return
        }
        textRasterFailed = false
        textRasterUsesStrips = false
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
        guard !textRasterUsesStrips, let layer, let surface = textRaster else { return }
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
        textRasterFailed = false
        textRasterPending = false
        textRasterUsesStrips = false
    }
}
#endif

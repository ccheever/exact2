// UIKit paragraph layers consume pixels from the same workers and paint routine
// as AppKit. Native views, inline links and accessibility keep their identities.
// @ref LLP 1044.000 §6 S5
#if os(iOS)
import UIKit
import CoreText

final class TextRasterizer {
    private final class Work {
        weak var node: NodeView?
        let key: TextRasterKey
        let group = DispatchGroup()
        var result: TextRasterImage?
        init(_ node: NodeView, key: TextRasterKey) {
            self.node = node; self.key = key
            group.enter()
        }
    }
    private var working: [Work] = []
    private static let maxConcurrent = 2
    private static let maximumBytes: CGFloat = 16 * 1024 * 1024

    private func key(_ node: NodeView) -> TextRasterKey {
        let scale = node.window?.screen.scale ?? node.traitCollection.displayScale
        var key = TextRasterKey(spec: node.paragraphSpec(), size: node.bounds.size,
            box: node.contentBox(), scale: max(1, scale))
        // Whole ordinary paragraphs, viewport bands for tall ones. Never allocate
        // to an unbreakable line's advance; the shared painter clips its ink.
        if node.bounds.height > 4096 || node.bounds.width * node.bounds.height * key.scale * key.scale * 4 > Self.maximumBytes {
            let port = node.presenter?.textPreparationRect(node) ?? node.bounds
            if let old = node.textRasterKey, let clip = old.clip, clip.contains(port), old.size == key.size,
               old.box == key.box, old.spec == key.spec, old.scale == key.scale { return old }
            let rowBytes = max(1, (node.bounds.width + 64) * key.scale * key.scale * 4)
            let height = max(port.height, min(port.height * 2, Self.maximumBytes / rowBytes))
            key.clip = CGRect(x: -32, y: max(-32, port.minY - (height - port.height) / 2),
                width: node.bounds.width + 64, height: height)
        }
        return key
    }

    @discardableResult
    func ensure(_ node: NodeView, urgent: Bool) -> Bool {
        guard node.canRasterText else { return true }
        let key = key(node)
        let visible = node.presenter?.textScrollportRect(node) ?? .zero
        let missingPixels = node.textRaster == nil || !node.textRasterFrame.contains(visible)
        if node.textRasterKey == key && (node.textRasterReady || !urgent || !missingPixels) { return true }
        let firstPixels = urgent && missingPixels
        guard firstPixels || working.count < Self.maxConcurrent else { return false }
        guard let engine = node.text else { return true }
        let measured = engine.measuredBreaks(key.spec, width: key.box.width)
        let paragraph = measured == nil ? node.paragraphLayout() : nil
        guard let (ranges, baselines) = measured
            ?? paragraph.map({ ($0.lines.map { CTLineGetStringRange($0) }, $0.baselines) }) else { return true }
        let source = paragraph?.shape?.attributed ?? engine.attributed(key.spec)
        let job = TextRasterJob(source: source.copy() as! NSAttributedString, ranges: ranges, baselines: baselines,
            flush: key.spec.align == 1 ? 0.5 : key.spec.align == 2 ? 1 : 0,
            box: key.box, size: key.size, scale: key.scale, clip: key.clip)
        node.textRasterKey = key; node.textRasterReady = false; node.textRasterFailed = false
        if firstPixels {
            let post = Presenter.signposts.beginInterval("text-raster-urgent")
            node.showTextRaster(job.render(), for: key)
            Presenter.signposts.endInterval("text-raster-urgent", post)
        } else {
            let work = Work(node, key: key)
            working.append(work)
            work.group.notify(queue: .main) { [weak self] in self?.publish(work) }
            RegionTextExecutor.queue.addOperation {
                let post = Presenter.signposts.beginInterval("text-raster-worker")
                work.result = job.render()
                Presenter.signposts.endInterval("text-raster-worker", post)
                work.group.leave()
            }
        }
        return true
    }
    private func publish(_ work: Work) {
        guard let index = working.firstIndex(where: { $0 === work }) else { return }
        // Only a completed worker's mailbox is read, including by agent settlement.
        guard work.group.wait(timeout: .now()) == .success else { return }
        working.remove(at: index)
        work.node?.showTextRaster(work.result, for: work.key)
        work.node?.presenter?.requestTextPublication()
    }

    /// A synchronous agent screenshot must observe the requested appearance,
    /// not a previous accepted raster. Rendering stays on the workers; their
    /// mailboxes can be published here without draining the main queue reentrantly.
    func settleVisible(_ nodes: [NodeView]) {
        let deadline = CACurrentMediaTime() + 1
        repeat {
            for node in nodes where node.canRasterText { ensure(node, urgent: false) }
            let batch = working
            guard !batch.isEmpty else { return }
            for work in batch {
                let left = max(0, deadline - CACurrentMediaTime())
                if work.group.wait(timeout: .now() + left) == .success { publish(work) }
            }
            if nodes.allSatisfy({ !$0.canRasterText || ($0.textRasterReady && $0.textRasterKey != nil) }) { return }
        } while CACurrentMediaTime() < deadline
    }

}

extension NodeView {
    var canRasterText: Bool {
        if textRasterFailed && textRasterKey != nil { return false }
        return isParagraph && flowShapes.isEmpty && !Capture.capturing && window != nil
            && bounds.width > 0 && bounds.height > 0 && number("line_clamp") == 0 && canvasAbove == nil
    }
    func textRasterGeometryChanged() {
        if let key = textRasterKey, key.size == bounds.size, key.box == contentBox() { return }
        textRasterKey = nil
        presenter?.requestTextPublication()
    }
    func showTextRaster(_ result: TextRasterImage?, for key: TextRasterKey) {
        guard presenter?.views[id] === self, textRasterKey == key, !textRasterReady else { return }
        guard let result else {
            dropTextRaster()
            textRasterKey = key; textRasterFailed = true
            // Retrying display through UIKit supplies the allocation fallback.
            setNeedsDisplay()
            return
        }
        textRaster = result.image; textRasterFrame = result.frame; textRasterScale = key.scale
        textRasterReady = true; textRasterFailed = false
        let ink = textRasterLayer ?? CALayer()
        CATransaction.begin(); CATransaction.setDisableActions(true)
        // Above the box's border (`applyBoxLayer`), under everything else.
        if ink.superlayer == nil {
            if let border = boxBorder { layer.insertSublayer(ink, above: border) } else { layer.insertSublayer(ink, at: 0) }
        }
        ink.frame = result.frame
        ink.contentsScale = key.scale
        ink.contents = result.image
        textRasterLayer = ink
        CATransaction.commit()
    }
    func dropTextRaster() {
        textRasterLayer?.removeFromSuperlayer(); textRasterLayer = nil
        textRaster = nil; textRasterKey = nil; textRasterReady = false; textRasterFailed = false
    }
}

/// One refresh's clip geometry: each ancestor's accumulated clips (at no
/// reach and at the lead's) and its offset in the viewport's space, found
/// once per ancestor rather than once per paragraph beneath it. Valid only
/// while no frame or offset changes.
final class TextClips {
    private struct Entry {
        /// Local-to-viewport translation; nil under a transform (convert).
        var shift: CGPoint?
        /// Clips at or above this view, in viewport space; `.null` if hidden.
        var near: CGRect, far: CGRect
    }
    private var memo: [ObjectIdentifier: Entry] = [:]
    unowned let viewport: UIView
    let reach: CGFloat
    init(_ viewport: UIView, reach: CGFloat) { self.viewport = viewport; self.reach = reach }

    private func entry(_ view: UIView?) -> Entry {
        guard let view else { return Entry(shift: nil, near: .infinite, far: .infinite) }
        if let known = memo[ObjectIdentifier(view)] { return known }
        var result = entry(view.superview)
        result.shift = view === viewport ? .zero : shift(view, above: result.shift)
        if view.isHidden || view.alpha == 0 {
            result.near = .null; result.far = .null
        } else if !result.near.isNull && (view.clipsToBounds || view is UIWindow) {
            let box = rect(view.bounds, of: view, shift: result.shift)
            result.near = result.near.intersection(box)
            result.far = result.far.intersection(box.insetBy(dx: -reach, dy: -reach))
        }
        memo[ObjectIdentifier(view)] = result
        return result
    }
    private func rect(_ r: CGRect, of view: UIView, shift: CGPoint?) -> CGRect {
        shift.map { r.offsetBy(dx: $0.x, dy: $0.y) } ?? view.convert(r, to: viewport)
    }
    private func shift(_ view: UIView, above: CGPoint?) -> CGPoint? {
        guard let s = above, view.transform.isIdentity, CATransform3DIsIdentity(view.layer.transform) else { return nil }
        return CGPoint(x: s.x + view.frame.minX - view.bounds.minX, y: s.y + view.frame.minY - view.bounds.minY)
    }
    /// `node`'s bounds in viewport space.
    func frame(_ node: NodeView) -> CGRect {
        rect(node.bounds, of: node, shift: shift(node, above: entry(node.superview).shift))
    }
    /// What `Presenter.textBand` finds by walking, for this reach or none;
    /// nil for any other reach.
    func band(_ node: NodeView, reach: CGFloat) -> CGRect? {
        guard reach == 0 || reach == self.reach else { return nil }
        if node.isHidden || node.alpha == 0 { return .zero }
        let above = entry(node.superview)
        let clip = reach == 0 ? above.near : above.far
        guard !clip.isNull else { return .zero }
        if clip.isInfinite { return node.bounds }
        let own = shift(node, above: above.shift)
        let band = rect(node.bounds, of: node, shift: own).intersection(clip)
        guard !band.isNull else { return .zero }
        let local = own.map { band.offsetBy(dx: -$0.x, dy: -$0.y) } ?? node.convert(band, from: viewport)
        return local.intersection(node.bounds)
    }
}

extension Presenter {
    /// Every clipping ancestor participates; an inner scroller can itself sit
    /// outside an outer viewport. All geometry stays in the paragraph's space.
    private func textBand(_ node: NodeView, reach: CGFloat) -> CGRect {
        guard node.window != nil else { return .zero }
        if let band = textClips?.band(node, reach: reach) { return band }
        var result = node.bounds
        var ancestor: UIView? = node
        while let view = ancestor {
            if view.isHidden || view.alpha == 0 { return .zero }
            if view !== node && (view.clipsToBounds || view is UIWindow) {
                result = result.intersection(node.convert(view.bounds, from: view).insetBy(dx: -reach, dy: -reach))
            }
            ancestor = view.superview
        }
        return result.isNull ? .zero : result
    }
    func textScrollportRect(_ node: NodeView) -> CGRect { textBand(node, reach: 0) }
    func textIsVisible(_ node: NodeView) -> Bool { !textScrollportRect(node).isEmpty }
    func textPreparationRect(_ node: NodeView) -> CGRect {
        let visible = textScrollportRect(node)
        return visible.isEmpty ? textBand(node, reach: viewport.bounds.height) : visible
    }

    /// Called after layout/scroll returns, never by the scroll callback. Lead
    /// rows receive workers before display; only uncovered visible pixels are urgent.
    /// Each ancestor's clip is found once (`TextClips`), each paragraph's
    /// distance from the viewport once, then sorted by that number.
    @discardableResult
    func refreshVisibleText(deadline: TimeInterval? = nil) -> Bool {
        guard !applying else { return true }
        let reach = viewport.bounds.height, port = viewport.bounds
        let clips = TextClips(viewport, reach: reach)
        textClips = clips
        defer { textClips = nil }
        var ranked: [(distance: CGFloat, node: NodeView)] = []
        ranked.reserveCapacity(textViews.count)
        for node in textViews.values where !node.bounds.isEmpty && !textBand(node, reach: reach).isEmpty {
            let r = clips.frame(node)
            ranked.append((max(0, port.minY - r.maxY, r.minY - port.maxY), node))
        }
        ranked.sort { $0.distance == $1.distance ? $0.node.id < $1.node.id : $0.distance < $1.distance }
        var deferred = false, admitted = 0
        for (_, node) in ranked where node.canRasterText {
            let visible = textScrollportRect(node)
            let urgent = !visible.isEmpty && (node.textRaster == nil || !node.textRasterFrame.contains(visible))
            if urgent {
                textRasters.ensure(node, urgent: true)
                continue
            }
            guard !node.textRasterReady || node.textRasterKey == nil || node.textRasterKey?.clip != nil else { continue }
            if admitted >= 6 || deadline.map({ CACurrentMediaTime() >= $0 }) == true { deferred = true; continue }
            if !textRasters.ensure(node, urgent: false) { deferred = true }
            admitted += 1
        }
        return deferred
    }
}
#endif

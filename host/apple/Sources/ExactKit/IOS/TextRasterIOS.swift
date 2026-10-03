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
        let namespace: Int
        /// The node's incarnation when the job was issued (LLP 1068 §4.9).
        let incarnation: UInt64
        let group = DispatchGroup()
        var result: TextRasterImage?
        weak var operation: Operation?
        private let lock = NSLock()
        private var started = false, abandoned = false
        init(_ node: NodeView, key: TextRasterKey, namespace: Int) {
            self.node = node; self.key = key; self.namespace = namespace; incarnation = node.incarnation
            group.enter()
        }
        /// The worker's claim: false once the job was abandoned unstarted.
        func begin() -> Bool {
            lock.lock(); defer { lock.unlock() }
            started = !abandoned
            return started
        }
        var wasAbandoned: Bool { lock.lock(); defer { lock.unlock() }; return abandoned }
        /// Drop a job no worker has begun: true if it will never render.
        func abandon() -> Bool {
            lock.lock()
            let unstarted = !started
            if unstarted { abandoned = true }
            lock.unlock()
            if unstarted { operation?.cancel() }
            return unstarted
        }
    }
    private var working: [Work] = []
    /// A worker per two cores, from two to four; twice that many jobs in
    /// flight, so a worker finishing one finds the next already queued.
    static let concurrency = max(2, min(4, ProcessInfo.processInfo.activeProcessorCount / 2))
    private static let maxInFlight = concurrency * 2
    private static let maximumBytes: CGFloat = 16 * 1024 * 1024
    var inFlight: Int { working.count }
    var hasRoom: Bool { working.count < Self.maxInFlight }

    /// Pixels by what they paint. Rows repeat small labels (a reaction's
    /// emoji and count, a relative time), and a list that turns back shows
    /// what it just left: those take the pixels another view already paid
    /// for. Least recently used first, bounded by bytes; only small images,
    /// so what stays beyond the views that show it is a few megabytes.
    private struct Kept {
        let image: TextRasterImage
        let bytes: Int
        var used: UInt64
    }
    private var kept: [TextRasterKey: Kept] = [:]
    private var keptBytes = 0
    private var keptClock: UInt64 = 0
    private var keptNamespace = 0
    private static let keptLimit = 4 * 1024 * 1024
    private static let keptEntryLimit = 64 * 1024
    private func keptImage(_ key: TextRasterKey, namespace: Int) -> TextRasterImage? {
        if namespace != keptNamespace { kept.removeAll(); keptBytes = 0; keptNamespace = namespace; return nil }
        guard var hit = kept[key] else { return nil }
        keptClock += 1; hit.used = keptClock; kept[key] = hit
        return hit.image
    }
    private func keep(_ image: TextRasterImage?, for key: TextRasterKey, namespace: Int) {
        guard let image, namespace == keptNamespace, kept[key] == nil else { return }
        let bytes = image.image.bytesPerRow * image.image.height
        guard bytes <= Self.keptEntryLimit else { return }
        keptClock += 1
        kept[key] = Kept(image: image, bytes: bytes, used: keptClock)
        keptBytes += bytes
        guard keptBytes > Self.keptLimit else { return }
        for (key, entry) in kept.sorted(by: { $0.value.used < $1.value.used }) {
            kept.removeValue(forKey: key); keptBytes -= entry.bytes
            if keptBytes <= Self.keptLimit * 3 / 4 { break }
        }
    }

    /// Let go of the pixels no view shows.
    func dropKept() { kept.removeAll(); keptBytes = 0 }

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

    /// Pixels for `node`'s paragraph. `urgent`: what shows has none, so they
    /// are painted now — taken from its worker if that has finished or is
    /// running, else rendered here. `now`: what shows is stale, and is
    /// painted now too (a presented colour). Otherwise a worker job at
    /// `priority`, if there is room for one (false when there is not).
    @discardableResult
    func ensure(_ node: NodeView, urgent: Bool, now: Bool = false, priority: Operation.QueuePriority = .normal) -> Bool {
        guard node.canRasterText else { return true }
        let key = key(node)
        let visible = node.presenter?.textUrgentRect(node) ?? .zero
        let missingPixels = node.textRaster == nil || !node.textRasterFrame.contains(visible) || now
        if node.textRasterKey == key && (node.textRasterReady || !urgent || !missingPixels) { return true }
        let firstPixels = urgent && missingPixels
        let pending = working.last { $0.node === node }
        guard let engine = node.text else { return true }
        if key.clip == nil, let image = keptImage(key, namespace: engine.namespace) {
            if let pending { _ = pending.abandon() }
            node.textRasterKey = key; node.textRasterReady = false; node.textRasterFailed = false
            node.showTextRaster(image, for: key)
            return true
        }
        if firstPixels, let pending, pending.key == key, take(pending) { return true }
        guard firstPixels || hasRoom else { return false }
        let measured = engine.measuredBreaks(key.spec, width: key.box.width)
        let paragraph = measured == nil ? node.paragraphLayout() : nil
        guard let geometry = measured ?? paragraph.map(LineGeometry.init) else { return true }
        let source = paragraph?.shape?.attributed ?? engine.attributed(key.spec)
        let job = TextRasterJob(source: source.copy() as! NSAttributedString, ranges: geometry.ranges, baselines: geometry.baselines,
            flush: key.spec.align == 1 ? 0.5 : key.spec.align == 2 ? 1 : 0,
            box: key.box, size: key.size, scale: key.scale, clip: key.clip,
            ellipsis: key.spec.ellipsis, crop: true, clamped: geometry.clamped)
        // A job for the paragraph's previous text or box paints nothing now.
        if let pending { _ = pending.abandon() }
        node.textRasterKey = key; node.textRasterReady = false; node.textRasterFailed = false
        if firstPixels {
            let post = Presenter.signposts.beginInterval("text-raster-urgent")
            let image = job.render()
            node.showTextRaster(image, for: key)
            if key.clip == nil { keep(image, for: key, namespace: engine.namespace) }
            Presenter.signposts.endInterval("text-raster-urgent", post)
        } else {
            let work = Work(node, key: key, namespace: engine.namespace)
            working.append(work)
            work.group.notify(queue: .main) { [weak self] in self?.publish(work) }
            let operation = BlockOperation {
                guard work.begin() else { return }
                let post = Presenter.signposts.beginInterval("text-raster-worker")
                work.result = job.render()
                Presenter.signposts.endInterval("text-raster-worker", post)
            }
            operation.queuePriority = priority
            // Also when abandoned unstarted: the mailbox always closes.
            operation.completionBlock = { work.group.leave() }
            work.operation = operation
            RegionTextExecutor.queue.addOperation(operation)
        }
        return true
    }
    /// A worker's pixels for what shows now: its finished result, or the
    /// running one waited for (it has the paragraph partly painted; the main
    /// thread would start over). False when it has not begun: it is dropped
    /// and the caller paints.
    private func take(_ work: Work) -> Bool {
        guard !work.wasAbandoned else { return false }
        if work.group.wait(timeout: .now()) != .success {
            if work.abandon() { return false }
            let post = Presenter.signposts.beginInterval("text-raster-wait")
            let done = work.group.wait(timeout: .now() + .milliseconds(8)) == .success
            Presenter.signposts.endInterval("text-raster-wait", post)
            guard done else { return false }
        }
        publish(work)
        return true
    }
    private func publish(_ work: Work) {
        guard let index = working.firstIndex(where: { $0 === work }) else { return }
        // Only a completed worker's mailbox is read, including by agent settlement.
        guard work.group.wait(timeout: .now()) == .success else { return }
        working.remove(at: index)
        // Dropped unstarted: no pixels, and no failure either.
        guard !work.wasAbandoned else { return }
        if work.key.clip == nil { keep(work.result, for: work.key, namespace: work.namespace) }
        guard let node = work.node, node.incarnation == work.incarnation else { return }
        node.showTextRaster(work.result, for: work.key)
        node.presenter?.requestTextPublication()
    }
    /// Jobs whose paragraph left, or whose text or box changed since, or
    /// that the list has already carried past (`passed`), and that no worker
    /// has begun, are dropped: their pixels would never show.
    func abandonStale(passed: (NodeView) -> Bool) {
        for work in working {
            guard let node = work.node, node.textRasterKey == work.key else { _ = work.abandon(); continue }
            // A passed paragraph owes a job again if the list comes back.
            if passed(node), work.abandon() { node.textRasterKey = nil }
        }
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
        // `text-overflow: ellipsis` truncates in the raster job, as `draw(_:)`
        // does (LLP 1053 G5): a label stretched across a row rasters its text,
        // not a backing store of the row's width. A `line-clamp`'s last line
        // is made again from the range it broke at (`LineGeometry.clamped`).
        guard isParagraph && flowShapes.isEmpty && !Capture.capturing && window != nil && backgroundClip != "text"
            && bounds.width > 0 && bounds.height > 0 else { return false }
        return canvasAbove == nil
    }
    /// The whole paragraph's pixels are up for its current text and box: a
    /// refresh has nothing to do for it until a change clears its key
    /// (`invalidateText`, `textRasterGeometryChanged`, `dropTextRaster`).
    var textRasterSettled: Bool { textRasterReady && textRasterWhole }
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
        textRaster = result.image; textRasterFrame = result.covered; textRasterScale = key.scale
        textRasterReady = true; textRasterFailed = false
        // The ink layer never animates (`InkLayer`), so no transaction of
        // its own: a worker's result published between frames commits with
        // the next frame's changes, rather than alone.
        let ink = textRasterLayer ?? InkLayer()
        // Above the box's border (`applyBoxLayer`), under everything else.
        // Inside a vibrancy view when it draws in a system colour over a
        // blur (`VibrancyIOS.swift`).
        if ink.superlayer == nil { textRasterLayer = ink; if syncVibrancy() == nil { insertBoxSublayer(ink) } }
        ink.frame = result.frame
        ink.contentsScale = key.scale
        NumeralRoll.roll(ink, node: self)
        ink.contents = result.image
        TextShadowLayer.apply(key.spec.shadow, to: ink)
        textRasterLayer = ink
    }
    func dropTextRaster() {
        textRasterLayer?.removeFromSuperlayer(); textRasterLayer = nil
        textRaster = nil; textRasterKey = nil; textRasterReady = false; textRasterFailed = false
    }
}

/// A paragraph's worker-rendered pixels (`showTextRaster`): its changes show
/// at once, as under `setDisableActions`, in whatever transaction is open.
final class InkLayer: CALayer {
    override func action(forKey event: String) -> (any CAAction)? { nil }
}

/// One refresh's clip geometry: each ancestor's accumulated clips (at no
/// reach, at the next frames' travel, and at the lead's) and its offset in
/// the viewport's space, found once per ancestor rather than once per
/// paragraph beneath it. Valid only while no frame or offset changes.
final class TextClips {
    private struct Entry {
        /// Local-to-viewport translation; nil under a transform (convert).
        var shift: CGPoint?
        /// Clips at or above this view per reach, in viewport space; `.null` if hidden.
        var clips: [CGRect]
    }
    private var memo: [ObjectIdentifier: Entry] = [:]
    unowned let viewport: UIView
    /// No reach, the travel the next frames will show, and the lead's.
    let reaches: [CGFloat]
    var soon: CGFloat { reaches[1] }
    init(_ viewport: UIView, soon: CGFloat, reach: CGFloat) {
        self.viewport = viewport; reaches = [0, soon, reach]
    }

    private func entry(_ view: UIView?) -> Entry {
        guard let view else { return Entry(shift: nil, clips: reaches.map { _ in .infinite }) }
        if let known = memo[ObjectIdentifier(view)] { return known }
        var result = entry(view.superview)
        result.shift = view === viewport ? .zero : shift(view, above: result.shift)
        if view.isHidden || view.alpha == 0 {
            result.clips = reaches.map { _ in .null }
        } else if !result.clips[0].isNull && (view.clipsToBounds || view is UIWindow) {
            let box = rect(view.bounds, of: view, shift: result.shift)
            result.clips = zip(result.clips, reaches).map { $0.intersection(box.insetBy(dx: -$1, dy: -$1)) }
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
        guard let index = reaches.firstIndex(of: reach) else { return nil }
        if node.isHidden || node.alpha == 0 { return .zero }
        let above = entry(node.superview)
        let clip = above.clips[index]
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
    /// What a refresh paints now rather than on a worker: what shows, and
    /// during a refresh what the next frames' travel will show.
    func textUrgentRect(_ node: NodeView) -> CGRect { textBand(node, reach: textClips?.soon ?? 0) }
    func textIsVisible(_ node: NodeView) -> Bool { !textScrollportRect(node).isEmpty }
    func textPreparationRect(_ node: NodeView) -> CGRect {
        let visible = textScrollportRect(node)
        return visible.isEmpty ? textBand(node, reach: viewport.bounds.height) : visible
    }

    /// Before the frame commits (the scroll callback, a rescue): a paragraph
    /// that shows without pixels for what shows gets them now, from its
    /// worker if that has finished or is running, else painted here — the
    /// reader never sees it blank (LLP 1050.000 D1). A settled paragraph
    /// costs one flag; the rest, their frame against the port.
    func paintVisibleText() {
        guard !applying else { return }
        let port = viewport.bounds
        let culls = viewport.clipsToBounds
        var clips: TextClips?
        defer { textClips = nil }
        // A paragraph that never rasters (it truncates as it paints, say)
        // is out before its geometry, which is most of a scan's cost.
        for node in textViews.values where !node.textRasterSettled && !node.bounds.isEmpty && node.canRasterText {
            let c = clips ?? TextClips(viewport, soon: 0, reach: 0)
            clips = c; textClips = c
            if culls && !c.frame(node).intersects(port) { continue }
            let visible = textScrollportRect(node)
            guard !visible.isEmpty, node.textRaster == nil || !node.textRasterFrame.contains(visible) else { continue }
            textRasters.ensure(node, urgent: true)
        }
    }

    /// Paint motion's colour reaches the screen with its value (LLP 1062
    /// D6): a paragraph whose presented colour a batch changed is painted
    /// as the batch ends, where it shows, not on a worker a frame later.
    func paintPresentedText() {
        guard !presentedText.isEmpty else { return }
        let ids = presentedText
        presentedText = []
        for id in ids {
            guard let node = views[id], node.canRasterText, !node.bounds.isEmpty, textIsVisible(node) else { continue }
            textRasters.ensure(node, urgent: true, now: true)
        }
    }

    /// Called after layout/scroll returns. What shows without pixels is
    /// painted (as `paintVisibleText`); built paragraphs within the lead get
    /// worker jobs, nearest first, what the next frames of travel
    /// (`velocity`, points a second, over `interval`) will show before the
    /// rest. At speed, what the list has carried past gets none, and its
    /// unstarted jobs are dropped. Each ancestor's clip is found once
    /// (`TextClips`), each paragraph's distance from the viewport once.
    /// True while jobs wait for room or time.
    @discardableResult
    func refreshVisibleText(deadline: TimeInterval? = nil, velocity: CGFloat = 0, interval: TimeInterval = 1.0 / 60) -> Bool {
        guard !applying else { return true }
        let port = viewport.bounds
        let soon = min(port.height, abs(velocity) * CGFloat(interval) * 2)
        // A quarter second of travel, at least a viewport.
        let reach = max(port.height, abs(velocity) / 4)
        let clips = TextClips(viewport, soon: 0, reach: reach)
        textClips = clips
        defer { textClips = nil }
        // More than two viewports a second: what is behind will not show.
        let fast = abs(velocity) > port.height * 2
        func passed(_ r: CGRect) -> Bool { fast && (velocity > 0 ? r.maxY <= port.minY : r.minY >= port.maxY) }
        if textRasters.inFlight > 0 { textRasters.abandonStale { passed(clips.frame($0)) } }
        // The viewport clips its content: a paragraph farther than the reach
        // from it has no band, so its distance alone rules it out.
        let culls = viewport.clipsToBounds
        var ranked: [(distance: CGFloat, node: NodeView)] = []
        for node in textViews.values where !node.textRasterSettled && !node.bounds.isEmpty && node.canRasterText {
            let r = clips.frame(node)
            let distance = max(0, port.minY - r.maxY, r.minY - port.maxY)
            guard !(culls && distance > reach), !passed(r), !textBand(node, reach: reach).isEmpty else { continue }
            ranked.append((distance, node))
        }
        ranked.sort { $0.distance == $1.distance ? $0.node.id < $1.node.id : $0.distance < $1.distance }
        var deferred = false
        for (distance, node) in ranked {
            let visible = textScrollportRect(node)
            if !visible.isEmpty && (node.textRaster == nil || !node.textRasterFrame.contains(visible)) {
                textRasters.ensure(node, urgent: true)
                continue
            }
            guard !node.textRasterSettled else { continue }
            if !textRasters.hasRoom || deadline.map({ CACurrentMediaTime() >= $0 }) == true { deferred = true; continue }
            let priority: Operation.QueuePriority = distance == 0 ? .veryHigh : distance <= soon ? .high : .normal
            if !textRasters.ensure(node, urgent: false, priority: priority) { deferred = true }
        }
        return deferred
    }
}
#endif

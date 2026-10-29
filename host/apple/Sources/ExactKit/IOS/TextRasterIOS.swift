// UIKit paragraph layers consume pixels from the same workers and paint routine
// as AppKit. Native views, inline links and accessibility keep their identities.
// @ref LLP 1044.000 §6 S5
#if os(iOS) || os(tvOS)
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
    /// A tall paragraph's band a worker paints ahead of the scroll: room
    /// for two ports at 3x on the iPhone Duo's open display, so the band is
    /// ready before the port leaves the one shown.
    private static let leadBytes: CGFloat = 48 * 1024 * 1024
    /// Every tall paragraph's band, shown or being painted, takes its bytes
    /// beyond `maximumBytes` from this one allowance, so they do not grow
    /// with the number of tall paragraphs near the port: a band left
    /// without any is as tall as the main thread's.
    private static let leadRoom = leadBytes - maximumBytes
    /// The paragraphs that have had a band, weakly.
    private let banded = NSHashTable<NodeView>.weakObjects()
    /// The most text (UTF-16 units) a changed paragraph paints on the main
    /// thread to replace its old words at once: a paragraph's worth. A
    /// longer one, which a short box may clip to a few of its lines, keeps
    /// the worker path, as every line of it is laid out to paint.
    private static let urgentWords = 1_000
    var inFlight: Int { working.count }
    var hasRoom: Bool { working.count < Self.maxInFlight }

    /// Pixels by what they paint. Rows repeat small labels (a reaction's
    /// emoji and count, a relative time), and a list that turns back shows
    /// what it just left: those take the pixels another view already paid
    /// for. Least recently used first, bounded by bytes; only small images,
    /// so what stays beyond the views that show it is a few megabytes.
    /// Kept by `TextRasterKey.anyRoom`, with the key it was painted for: a
    /// view of another size, or the same view after its padding changed,
    /// takes pixels that serve it (`TextRasterKey.serves`).
    private struct Kept {
        let image: TextRasterImage
        let key: TextRasterKey
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
        guard var hit = kept[key.anyRoom], hit.key.serves(key, ink: hit.image.ink, shadows: hit.image.shadows) else { return nil }
        keptClock += 1; hit.used = keptClock; kept[key.anyRoom] = hit
        return hit.image
    }
    private func keep(_ image: TextRasterImage?, for key: TextRasterKey, namespace: Int) {
        let any = key.anyRoom
        guard let image, namespace == keptNamespace else { return }
        let bytes = image.image.bytesPerRow * image.image.height + (image.cast.map { $0.bytesPerRow * $0.height } ?? 0)
        guard bytes <= Self.keptEntryLimit else { return }
        // A newer image replaces one that cannot serve its room (a narrower view's, its ink cut)
        if let old = kept[any] {
            guard !old.key.serves(key, ink: old.image.ink, shadows: old.image.shadows) else { return }
            kept.removeValue(forKey: any); keptBytes -= old.bytes
        }
        keptClock += 1
        kept[any] = Kept(image: image, key: key, bytes: bytes, used: keptClock)
        keptBytes += bytes
        guard keptBytes > Self.keptLimit else { return }
        for (key, entry) in kept.sorted(by: { $0.value.used < $1.value.used }) {
            kept.removeValue(forKey: key); keptBytes -= entry.bytes
            if keptBytes <= Self.keptLimit * 3 / 4 { break }
        }
    }

    /// Let go of the pixels no view shows.
    func dropKept() { kept.removeAll(); keptBytes = 0 }

    /// What the paragraph's pixels hold: its words and style, the box they
    /// are laid out in (its width breaks and aligns the lines), the scale,
    /// and where its ink is kept (`TextRasterKey.room`), all in the box's own
    /// space, never where the box is in its view (`NodeView.placeTextRaster`).
    /// The view's bounds are that room, and a tall view's paragraph is
    /// painted in bands of the view's rows.
    /// `urgent`: the band is painted on the main thread now, so it is no
    /// taller than it must be; a worker's band leads the scroll instead.
    private func key(_ node: NodeView, urgent: () -> Bool) -> TextRasterKey {
        let scale = max(1, node.window?.screen.scale ?? node.traitCollection.displayScale)
        let box = node.paragraphBox(), room = node.textRasterRoom(box)
        var key = TextRasterKey(spec: node.paragraphSpec(), size: box.size, box: CGRect(origin: .zero, size: box.size), scale: scale, room: room)
        // Whole ordinary paragraphs, viewport bands for tall ones. Never allocate
        // to an unbreakable line's advance; the shared painter clips its ink.
        if room.height > 4096 || room.width * room.height * scale * scale * 4 > Self.maximumBytes {
            // `band` and `keeps` measure from the view's top left.
            let inView = { (r: CGRect) in r.offsetBy(dx: -room.minX, dy: -room.minY) }
            let port = (node.presenter?.textPreparationRect(node) ?? node.bounds).offsetBy(dx: -box.minX, dy: -box.minY)
            let down = node.textMore.bandTravel.follow(port)
            // Kept until the port nears its edge ahead; then a worker paints the next.
            if let old = node.textRasterKey, var clip = old.clip, clip.minY <= port.minY, clip.maxY >= port.maxY,
               TextRasterJob.keeps(inView(clip), port: inView(port), down: down, height: room.height) || urgent() {
                // Its rows, across the view as it is now
                let columns = TextRasterJob.bandColumns(key.spec, width: room.width).offsetBy(dx: room.minX, dy: 0)
                clip.origin.x = columns.minX; clip.size.width = columns.width
                var kept = key
                kept.clip = clip
                if kept == old || node.textRasterServes(kept) { return kept }
            }
            let now = urgent()
            let lead = now ? 0 : max(0, Self.leadRoom - leadTaken(besides: node))
            key.clip = TextRasterJob.band(key.spec, width: room.width, port: inView(port), scale: scale,
                                          maximumBytes: Self.maximumBytes + lead, down: now ? nil : down)
                .offsetBy(dx: room.minX, dy: room.minY)
            banded.add(node)
        }
        return key
    }

    /// The allowance other paragraphs' bands hold, counted as a band is
    /// sized, four bytes a pixel: the pixels each shows, which stay up while
    /// new ones are asked for (`invalidateText` clears the key and keeps
    /// them), and each job still to paint, whatever its paragraph asks for
    /// now (one begun runs to its end). `node`'s own is the band its new one
    /// replaces.
    private func leadTaken(besides node: NodeView) -> CGFloat {
        func beyond(_ width: CGFloat, _ height: CGFloat, _ scale: CGFloat) -> CGFloat {
            max(0, width * height * scale * scale * 4 - Self.maximumBytes)
        }
        let shown = banded.allObjects.reduce(0) { taken, other in
            guard other !== node, other.textRaster != nil else { return taken }
            return taken + beyond(other.textRasterFrame.width, other.textRasterFrame.height, other.textRasterScale)
        }
        return working.reduce(shown) { taken, work in
            guard work.node !== node, !work.wasAbandoned, let clip = work.key.clip else { return taken }
            return taken + beyond(clip.width, clip.height, work.key.scale)
        }
    }

    /// Pixels for `node`'s paragraph. `urgent`: what shows has none, so they
    /// are painted now — taken from its worker if that has finished or is
    /// running, else rendered here. `now`: what shows is stale, and is
    /// painted now too (a presented colour). Otherwise a worker job at
    /// `priority`, if there is room for one (false when there is not).
    /// `urgent` and what shows are asked only of a paragraph whose pixels
    /// are not already its words', and `urgent` also of a tall one whose
    /// port nears its band's edge (`key`): they walk its clipping ancestors,
    /// and a batch's display pass asks again of every paragraph it gave new
    /// words.
    @discardableResult
    func ensure(_ node: NodeView, urgent isUrgent: @autoclosure () -> Bool, now: Bool = false, priority: Operation.QueuePriority = .normal) -> Bool {
        guard node.canRasterText else { return true }
        // Asked once, and only when needed: it walks the clipping ancestors.
        lazy var urgent = isUrgent()
        let key = key(node, urgent: { urgent })
        // Pixels that hold what `key`'s would are its own (`TextRasterKey.serves`).
        if node.textRasterKey != key, node.textRasterServes(key) { node.adoptTextRaster(key) }
        if node.textRasterKey == key && node.textRasterReady { return true }
        let visible = node.presenter?.textUrgentRect(node) ?? .zero
        // A whole paragraph's pixels of other words are none of this text's:
        // an urgent paint replaces them now, rather than a worker a frame
        // later. A tall paragraph's bands, a long text and a shadowed one
        // (each line a blurred layer) keep the worker path.
        let otherWords = key.clip == nil && node.textRasterSpec != key.spec
            && key.spec.shadow == nil && !key.spec.runs.contains { $0.shadow != nil }
            && key.spec.runs.reduce(0) { $0 + $1.text.utf16.count } <= Self.urgentWords
        let missingPixels = node.textRaster == nil || !node.textRasterFrame.contains(visible) || now || otherWords
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
        // A finished worker for this key shows only if the node still asks
        // for it: the key a batch's change cleared is asked for again first
        if firstPixels, let pending, pending.key == key {
            node.textRasterKey = key; node.textRasterReady = false
            if take(pending), node.textRasterReady { return true }
        }
        guard firstPixels || hasRoom else { return false }
        let measured = engine.measuredBreaks(key.spec, width: key.box.width)
        let paragraph = measured == nil ? node.paragraphLayout() : nil
        guard let geometry = measured ?? paragraph.map(LineGeometry.init) else { return true }
        let source = paragraph?.shape?.attributed ?? engine.attributed(key.spec)
        let job = TextRasterJob(source: source.copy() as! NSAttributedString, ranges: geometry.ranges, baselines: geometry.baselines,
            flush: key.spec.align == 1 ? 0.5 : key.spec.align == 2 ? 1 : 0, justifies: key.spec.align == 3, insets: paragraph?.insets ?? LineInsets(key.spec, source: source),
            box: key.box, size: key.size, scale: key.scale, clip: key.clip, room: key.room,
            ellipsis: key.spec.ellipsis, crop: true, clamped: geometry.clamped, shadow: key.spec.hdrShadow.map(TextRunShadow.init),
            // A layer shadow and a vibrancy view each take the ink as it is.
            tints: key.spec.shadow == nil && (node.vibrantColor == nil || node.enclosingBlur == nil),
            antialiased: key.spec.antialiased,
            measuredGlyphs: key.spec.color.count == 4 ? engine.measuredGlyphs(key.spec, width: key.box.width) : nil,
            measuredColor: TextEngine.color(key.spec.color).cgColor)
        // A job for the paragraph's previous text or box paints nothing now.
        if let pending { _ = pending.abandon() }
        node.textRasterKey = key; node.textRasterReady = false; node.textRasterFailed = false
        if firstPixels, urgentBatch != nil {
            urgentBatch?.append(UrgentPaint(node: node, key: key, job: job, namespace: engine.namespace))
        } else if firstPixels {
            let post = Presenter.signposts.beginInterval("text-raster-urgent"), phase = HostPhases.shared.begin(.text)
            defer { HostPhases.shared.end(phase) }
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
    private struct UrgentPaint {
        let node: NodeView
        let key: TextRasterKey
        let job: TextRasterJob
        let namespace: Int
    }
    /// The paragraphs `ensureUrgent` paints together, while it runs.
    private var urgentBatch: [UrgentPaint]?

    /// How long `ensureUrgent` paints on the main thread alone before it
    /// paints the rest at once. A paragraph painted on another thread makes
    /// its glyphs' bitmaps again (each frame of a weight transition is a new
    /// font): twelve paragraphs painted at once every frame took twice the
    /// CPU time of the same paragraphs painted in turn.
    private static let urgentAlone: CFTimeInterval = 0.002

    /// `ensure(urgent: true)` for each of `nodes`. The paragraphs that must
    /// be painted now are painted in turn on the main thread for up to
    /// `urgentAlone`, then the rest at once, the main thread among the
    /// workers. For about ten frames after an idle stretch the CPU runs
    /// slowly: twelve paragraphs of body text painted in turn then took 4 ms
    /// of a frame's 5.4, against 1.1 later (iPhone 17 Pro Max, Time
    /// Profiler), and those were the frames that came late.
    func ensureUrgent(_ nodes: [NodeView], now: Bool = false) {
        urgentBatch = []
        for node in nodes { ensure(node, urgent: true, now: now) }
        let batch = urgentBatch ?? []
        urgentBatch = nil
        guard !batch.isEmpty else { return }
        let post = Presenter.signposts.beginInterval("text-raster-urgent"), phase = HostPhases.shared.begin(.text)
        defer { HostPhases.shared.end(phase) }
        var images = [TextRasterImage?](repeating: nil, count: batch.count)
        images.withUnsafeMutableBufferPointer { slots in
            let start = CACurrentMediaTime()
            var next = 0
            while next < batch.count && CACurrentMediaTime() - start < Self.urgentAlone {
                slots[next] = batch[next].job.render()
                next += 1
            }
            let first = next
            guard first < batch.count else { return }
            DispatchQueue.concurrentPerform(iterations: batch.count - first) { slots[first + $0] = batch[first + $0].job.render() }
        }
        Presenter.signposts.endInterval("text-raster-urgent", post)
        for (paint, image) in zip(batch, images) {
            paint.node.showTextRaster(image, for: paint.key)
            if paint.key.clip == nil { keep(image, for: paint.key, namespace: paint.namespace) }
        }
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
        guard isParagraph && flowShapes.isEmpty && columnRecord == nil && !Capture.capturing && window != nil && backgroundClip != "text"
            && bounds.width > 0 && bounds.height > 0 else { return false }
        return canvasAbove == nil && !paintsVisibleInlineRun && !rollsNumerals
    }
    /// LLP 1077 D15: the paragraph's numerals roll per glyph, drawn by the
    /// Numerals capability rather than as a raster (`NumeralLink`). Not in a
    /// capture, which draws the paragraph as a picture.
    var rollsNumerals: Bool {
        NumeralLink.installed != nil && !Capture.capturing
            && (style["content_transition"]?.string ?? "none") != "none"
    }
    /// The whole paragraph's pixels are up for its current text and box: a
    /// refresh has nothing to do for it until a change clears its key
    /// (`invalidateText`, `textRasterGeometryChanged`, `dropTextRaster`).
    var textRasterSettled: Bool { textRasterReady && textRasterWhole }
    func textRasterGeometryChanged() {
        // Laid out to nothing (a text that became a collapsed space, as
        // stopwatch's status line did on Reset, LLP 1116.005 D4), it paints
        // nothing. Neither the pump nor UIKit's `draw(_:)`, which lets go of
        // a raster that no longer applies, visits an empty box: the old ink
        // goes here, or it stays on screen beside the text that replaced it.
        if bounds.isEmpty {
            if textRaster != nil || textRasterLayer != nil { dropTextRaster() }
            return
        }
        // A box that moves keeps its pixels, which move with it; only one of
        // another size, or whose pixels the view's new bounds cut otherwise
        // (`TextRasterKey.serves`), is painted again.
        placeTextRaster()
        let box = paragraphBox()
        if var key = textRasterKey, key.size == box.size {
            let room = textRasterRoom(box)
            if key.room == room { return }
            key.room = room
            if textRasterServes(key) { adoptTextRaster(key); return }
        }
        textRasterKey = nil
        presenter?.requestTextPublication()
    }
    /// The view's bounds in its text's box's space: where the paragraph's
    /// ink is kept (`TextRasterKey.room`).
    func textRasterRoom(_ box: CGRect) -> CGRect { bounds.offsetBy(dx: -box.minX, dy: -box.minY) }
    /// Whether the pixels shown are exactly `key`'s (`TextRasterKey.serves`).
    func textRasterServes(_ key: TextRasterKey) -> Bool {
        guard textRasterReady, let shown = textRasterKey else { return false }
        return shown.serves(key, ink: textMore.rasterInk.ink, shadows: textMore.rasterInk.shadows)
    }
    /// Takes `key` for the pixels shown, which serve it: they answer for its
    /// room too (`placeTextRaster`).
    func adoptTextRaster(_ key: TextRasterKey) {
        textRasterKey = key
        textMore.rasterRoom = key.room ?? key.box
        placeTextRaster()
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
        textRaster = result.image; textRasterScale = key.scale; textRasterSpec = key.spec
        textMore.rasterCovered = result.covered; textMore.rasterBand = key.clip != nil; textMore.rasterInk = (result.ink, result.shadows)
        textMore.rasterRoom = key.room ?? key.box
        textRasterReady = true; textRasterFailed = false
        // One colour's coverage masks that colour; other ink carries its
        // own colours. Each has its own layer.
        let tint = result.tint
        if (tint != nil) != (textRasterLayer is TintInk), let old = textRasterLayer {
            old.dropTextCast(); old.removeFromSuperlayer(); textRasterLayer = nil
        }
        // The ink layer never animates (`InkLayer`), so no
        // transaction of its own: a worker's result published between frames
        // commits with the next frame's changes, rather than alone.
        let ink: CALayer
        if let tint {
            let layer = textRasterLayer as? TintInk ?? TintInk()
            layer.show(result.image, tint: tint, frame: result.frame, scale: key.scale)
            ink = layer
        } else {
            ink = textRasterLayer ?? InkLayer()
        }
        hostText(ink)
        ink.frame = result.frame
        ink.contentsScale = key.scale
        let roll = NumeralRoll.roll(ink, node: self)
        if tint == nil { ink.contents = result.image }
        TextShadowLayer.apply(key.spec.hdrShadow == nil ? key.spec.shadow : nil, to: ink)
        ink.applyTextRange(headroom: result.headroom, limit: style["dynamic_range_limit"]?.string)
        ink.applyTextCast(result.cast, headroom: result.castHeadroom, limit: style["dynamic_range_limit"]?.string, rolling: roll)
        textRasterLayer = ink
        presenter?.boxFilters.changed(self)
    }
    /// Release a raster still off-window a turn after any within-batch move; the next display
    /// repaints it when the view returns.
    func dropTextRasterOffWindow() {
        guard window == nil, textRaster != nil else { return }
        dropTextRaster()
        layer.setNeedsDisplay()
    }

    func dropTextRaster() {
        textRasterLayer?.dropTextCast(); textRasterLayer?.removeFromSuperlayer(); textRasterLayer = nil
        removeTextBox()
        textRaster = nil; textRasterKey = nil; textRasterReady = false; textRasterFailed = false; textRasterSpec = nil
    }

    /// The paragraph's ink in its text's box (`placeTextRaster`), made now
    /// if it has none. The box is inside a vibrancy view when the ink draws
    /// in a system colour over a blur (`VibrancyIOS.swift`).
    func hostText(_ ink: CALayer) {
        let made = textBoxLayer == nil
        if made { CATransaction.begin(); CATransaction.setDisableActions(true) }
        defer { if made { CATransaction.commit() } }
        let box = textBoxLayer ?? makeTextBox()
        if ink.superlayer !== box { box.addSublayer(ink) }
        textRasterLayer = ink
        if box.superlayer == nil, syncVibrancy() == nil { mountTextBox() }
        placeTextRaster()
    }

    private func makeTextBox() -> CALayer {
        let box = BoxPartLayer()
        box.anchorPoint = .zero
        textBoxLayer = box
        return box
    }

    /// The text's box above the box's border (`applyBoxLayer`), under everything else.
    func mountTextBox() {
        if let box = textBoxLayer { insertBoxSublayer(box) }
    }

    func removeTextBox() {
        textBoxLayer?.removeFromSuperlayer()
        textBoxLayer = nil
    }

    /// Where the paragraph's box is drawn in this view: its content box
    /// placed on the device's pixel grid as a box of its own is
    /// (`PixelGrid.place`, from this view's absolute origin), where a label's
    /// frame lies, so its glyphs are as crisp as the box's own edges.
    func textPlace(scale: CGFloat) -> CGPoint {
        let origin = grid?.origin ?? .zero
        return PixelGrid.place(paragraphBox(), in: origin, scale: origin.scale > 0 ? CGFloat(origin.scale) : scale).drawn.origin
    }

    /// The paragraph's pixels at its box (`textPlace`), as a label is at
    /// its frame: the text's box layer, the ink inside it where its pixels
    /// lie in the box. Moved inside an animation UIKit has open, the box
    /// moves with it (`BoxPartLayer`); what it holds changes only with the
    /// pixels (`showTextRaster`).
    func placeTextRaster() {
        guard let box = textBoxLayer, let specs else { return }
        let at = textPlace(scale: textRasterScale), size = paragraphBox().size
        if box.position != at { box.position = at }
        if box.bounds.size != size { box.bounds = CGRect(origin: .zero, size: size) }
        // What the pixels answer for in this view: the room they were painted
        // for (the view's bounds then, `TextRasterKey.room`) and the ink past
        // it; a band, only its rows. Bounds that grew since are not theirs
        // until they are painted again or shown to serve them.
        let covered = specs.rasterCovered.offsetBy(dx: at.x, dy: at.y), room = specs.rasterRoom.offsetBy(dx: at.x, dy: at.y)
        let rows = specs.rasterBand ? CGRect(x: room.minX, y: covered.minY, width: room.width, height: covered.height) : room
        textRasterFrame = covered.isNull ? .zero : covered.union(rows)
    }
}

/// A paragraph's worker-rendered pixels (`showTextRaster`): its changes show
/// at once, as under `setDisableActions`, in whatever transaction is open.
class InkLayer: CALayer {
    override func action(forKey event: String) -> (any CAAction)? { nil }
}

/// A one-colour paragraph's ink (`TextRasterImage.tint`): a layer of that
/// colour masked by its coverage. A layer, not a view: Core Animation's
/// sublayer methods are for layers you create, never a view's own layer
/// ("Building a Layer Hierarchy"). A paragraph with a `text-shadow` is
/// never tinted (`TextRasterizer`'s `tints`), so the mask clips no shadow.
final class TintInk: InkLayer {
    func show(_ image: CGImage, tint: CGColor, frame: CGRect, scale: CGFloat) {
        self.frame = frame
        backgroundColor = tint
        let coverage = mask ?? InkLayer()
        coverage.frame = bounds
        coverage.contentsScale = scale
        coverage.contents = image
        if mask !== coverage { mask = coverage }
    }
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
        // The document's mount stands in the viewport's place (`DocumentMountIOS.swift`).
        result.shift = view === viewport || view is DocumentMount ? .zero : shift(view, above: result.shift)
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
    /// nil for any other reach, and outside the viewport or under a
    /// transform, where clips meet as their bounding boxes in the
    /// viewport's space (a rotated parent's covers a corner its own bounds
    /// do not) and only the walk is exact.
    func band(_ node: NodeView, reach: CGFloat) -> CGRect? {
        guard let index = reaches.firstIndex(of: reach) else { return nil }
        if node.isHidden || node.alpha == 0 { return .zero }
        let above = entry(node.superview)
        let clip = above.clips[index]
        guard !clip.isNull else { return .zero }
        if clip.isInfinite { return node.bounds }
        guard let own = shift(node, above: above.shift) else { return nil }
        let band = rect(node.bounds, of: node, shift: own).intersection(clip)
        guard !band.isNull else { return .zero }
        return band.offsetBy(dx: -own.x, dy: -own.y).intersection(node.bounds)
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
    /// costs one flag; the rest, their frame against the port. True while
    /// some paragraph lacks pixels: the pump's refresh has work then.
    @discardableResult
    func paintVisibleText() -> Bool {
        guard !applying else { return true }
        let port = viewport.bounds
        let culls = viewport.clipsToBounds
        let presentations = surfaces(includingRetiring: true).filter { $0.kind != .document }
        func presented(_ node: UIView) -> Bool { presentations.contains { node.isDescendant(of: $0.view) } }
        var clips: TextClips?
        defer { textClips = nil }
        var owed = false
        // A paragraph that never rasters (it truncates as it paints, say)
        // is out before its geometry, which is most of a scan's cost.
        func unsettled(_ node: NodeView) -> Bool { !node.textRasterSettled && !node.bounds.isEmpty && node.canRasterText }
        textIndex.visit(textViews, reach: 0, viewport: viewport) { node in
            guard unsettled(node) else { return true }
            owed = true
            let c = clips ?? TextClips(viewport, soon: 0, reach: 0)
            clips = c; textClips = c
            if culls && !presented(node) && !c.frame(node).intersects(port) { return true }
            let visible = textScrollportRect(node)
            guard !visible.isEmpty, node.textRaster == nil || !node.textRasterFrame.contains(visible) else { return true }
            textRasters.ensure(node, urgent: true)
            return true
        }
        // The refresh has work while a paragraph within its least lead
        // (a viewport) lacks pixels; one farther waits until it is nearer.
        if !owed { textIndex.visit(textViews, reach: port.height, viewport: viewport) { node in owed = unsettled(node); return !owed } }
        return owed
    }

    /// Paint motion's colour reaches the screen with its value (LLP 1062
    /// D6): a paragraph whose presented colour a batch changed is painted
    /// as the batch ends, where it shows, not on a worker a frame later.
    /// The paragraphs a batch gave new words (`changedText`) show them in
    /// the batch's frame, where they show (LLP 1116.005 D4), as UIKit draws a
    /// label's new text in the commit that sets it, never the old words a
    /// frame longer while a worker paints.
    func paintChangedText() {
        guard !changedText.isEmpty else { return }
        let ids = changedText
        changedText = []
        // Each clipping ancestor's geometry once, not once per paragraph
        // (a log's hundred rows, each given its neighbour's words).
        textClips = TextClips(viewport, soon: 0, reach: 0)
        defer { textClips = nil }
        textRasters.ensureUrgent(ids.compactMap { id in
            guard let node = views[id], node.canRasterText, !node.bounds.isEmpty, textIsVisible(node) else { return nil }
            return node
        })
    }

    func paintPresentedText() {
        guard !presentedText.isEmpty else { return }
        let ids = presentedText
        presentedText = []
        textRasters.ensureUrgent(ids.compactMap { id in
            guard let node = views[id], node.canRasterText, !node.bounds.isEmpty, textIsVisible(node) else { return nil }
            return node
        }, now: true)
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

        // The viewport clips its content: a paragraph farther than the reach
        // from it has no band, so its distance alone rules it out.
        let culls = viewport.clipsToBounds
        let presentations = surfaces(includingRetiring: true).filter { $0.kind != .document }
        func presented(_ node: UIView) -> Bool { presentations.contains { node.isDescendant(of: $0.view) } }
        if textRasters.inFlight > 0 { textRasters.abandonStale { !presented($0) && passed(clips.frame($0)) } }
        var ranked: [(distance: CGFloat, node: NodeView)] = []
        textIndex.visit(textViews, reach: reach, viewport: viewport) { node in
            guard !node.textRasterSettled && !node.bounds.isEmpty && node.canRasterText else { return true }
            let r = clips.frame(node)
            let distance = presented(node) ? 0 : max(0, port.minY - r.maxY, r.minY - port.maxY)
            guard !(culls && distance > reach), (presented(node) || !passed(r)), !textBand(node, reach: reach).isEmpty else { return true }
            ranked.append((distance, node))
            return true
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

// @ref LLP 1044.000 §6 S3(d) — reader paragraphs use the region text service.
#if os(macOS)
import AppKit
import CExact

/// Durations of outermost signposted reader work on the UI executor. A sliding
/// display-length interval is stricter than choosing favorable frame boundaries.
enum RegionReaderTiming {
    private static var depth = 0
    private static var started = 0.0
    private static var intervals: [(Double, Double)] = []
    private(set) static var maximumFrameSeconds = 0.0
    static func reset() { intervals.removeAll(); maximumFrameSeconds = 0 }
    static func begin() {
        precondition(Thread.isMainThread)
        if depth == 0 { started = CACurrentMediaTime() }
        depth += 1
    }
    static func end() {
        depth -= 1
        guard depth == 0 else { return }
        let now = CACurrentMediaTime(), cutoff = now - 1.0 / 60
        intervals.removeAll { $0.1 <= cutoff }
        intervals.append((started, now))
        maximumFrameSeconds = max(maximumFrameSeconds, intervals.reduce(0) { $0 + $1.1 - max(cutoff, $1.0) })
    }
}

/// UI receipt for one paragraph allocation/revision. CoreText and the ink index
/// remain in RegionService; this owner receives compact geometry and viewport pixels.
final class RegionReaderParagraph {
    let view: UInt32
    let index: UInt32
    let generation: UInt32
    let revision: UInt64
    private let estimatedBytes: Int
    private let lineHeight: CGFloat
    private let fontSize: CGFloat
    private weak var node: NodeView?
    private var source: RegionTextSource?
    private var candidate: RegionArtifact?
    private(set) var accepted: RegionArtifact?
    private(set) var raster: RegionRaster?
    private var image: CGImage?
    private var imageFrame = CGRect.zero
    private var wantedWidth: CGFloat = 0
    private var shapeWidth: CGFloat?
    private var serial: UInt64 = 0
    private var wantedRaster: RegionRasterRequest?
    private var failed: String?
    private lazy var service = RegionService { [weak self] in self?.receive($0) }
    private let profile: NativeProfile
    private let profileAccount = NativeProfileAccount()
    private var publishing = false
    private var maxDemand = 0.0, maxPublish = 0.0, maxDraw = 0.0
    private var candidateTop: CGFloat?
    private var pointRequest: RegionPointRequest?
    private var pointReply: ((Int?, String?) -> Void)?

    init(_ request: ExactMeasureRequest, bytes: Int) {
        view = request.view; index = request.node_index; generation = request.node_generation
        revision = request.revision; estimatedBytes = bytes
        fontSize = CGFloat(request.strut.font_size)
        lineHeight = request.strut.has_line_height != 0 ? CGFloat(request.strut.line_height) : fontSize * 1.2
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        profile = NativeProfile.capture(original: space, data: space.copyICCData(), account: profileAccount).owner!
    }
    deinit { service.close() }
    func matches(_ r: ExactMeasureRequest) -> Bool {
        index == r.node_index && generation == r.node_generation && revision == r.revision
    }
    func metrics(width: CGFloat) -> ExactMetrics {
        if let p = accepted?.metadata, p.offeredWidth == width {
            return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
        }
        // Pending geometry is never painted as text. Preserve the accepted
        // logical extent while successive intermediate widths replace demand.
        let height = accepted?.metadata.height
            ?? max(lineHeight, ceil(CGFloat(estimatedBytes) * fontSize * 0.5 / max(1, width)) * lineHeight)
        return ExactMetrics(width: Float(width), height: Float(height), baseline: -2)
    }

    func invalidatePaint() {
        guard source != nil else { return }
        service.reset()
        source = nil; candidate = nil; accepted = nil; raster = nil; image = nil
        shapeWidth = nil; wantedRaster = nil; candidateTop = nil; failed = nil
        let reply = pointReply; pointReply = nil; pointRequest = nil
        reply?(nil, nil)
    }
    var diagnostics: [String: Any] {
        ["id": view, "pending": accepted == nil || accepted?.metadata.offeredWidth != wantedWidth,
         "width": accepted?.metadata.offeredWidth ?? 0, "wantedWidth": wantedWidth,
         "height": accepted?.metadata.height ?? 0, "lines": accepted?.metadata.lines.count ?? 0,
         "utf16": accepted?.metadata.source.utf16Count ?? 0, "sha256": accepted?.metadata.sourceSHA256 ?? "",
         "viewportY": raster?.request.scroll.y ?? 0, "viewportHeight": raster?.request.size.height ?? 0,
         "maxDemandMs": maxDemand * 1000, "maxPublishMs": maxPublish * 1000, "maxDrawMs": maxDraw * 1000,
         "failure": failed as Any? ?? NSNull()]
    }

    func update(_ node: NodeView) {
        RegionReaderTiming.begin()
        defer { RegionReaderTiming.end() }
        let started = CACurrentMediaTime()
        let post = Presenter.signposts.beginInterval("reader-demand")
        defer {
            Presenter.signposts.endInterval("reader-demand", post)
            maxDemand = max(maxDemand, CACurrentMediaTime() - started)
        }
        guard !publishing, node.window != nil, node.flowShapes.isEmpty else { return }
        self.node = node
        // Captured exactly once after the native batch supplies resolved paint.
        // The measure callback never shapes or copies the giant source.
        if source == nil, let engine = node.text {
            let spec = node.paragraphSpec()
            source = RegionTextSource.capture(spec, engine: engine)
        }
        guard let source, failed == nil else { return }
        let width = CGFloat(Float(node.contentBox().width))
        guard width > 0 else { return }
        wantedWidth = width
        if let candidate, candidate.metadata.offeredWidth != width {
            self.candidate = nil; candidateTop = nil; wantedRaster = nil
        }
        if candidate?.metadata.offeredWidth != width && accepted?.metadata.offeredWidth != width {
            if shapeWidth != width {
                serial += 1
                shapeWidth = width
                candidate = nil; candidateTop = nil; wantedRaster = nil
                service.updateShapeRequest(serial, generation: 0)
                service.submit(.shape(RegionShapeRequest(id: serial, sourceID: revision, source: source,
                    width: width, height: -1, generation: 0, compact: true)))
            }
            return
        }
        // Returning to the accepted width supersedes an unstarted width job too.
        if accepted?.metadata.offeredWidth == width { shapeWidth = nil }
        requestViewport()
    }

    private func requestViewport() {
        guard let node, let artifact = pointRequest == nil ? candidate ?? accepted : accepted,
              failed == nil else { return }
        let p = artifact.metadata
        let content = node.contentBox()
        let port = node.presenter?.textScrollportRect(node) ?? node.visibleRect
        guard !port.isEmpty else { return }
        let scale = Int(node.window?.backingScaleFactor ?? 2)
        // Overscan within the existing pixel budget, always horizontally bounded
        // even when CSS normal lets a million-character word overflow.
        let top = (pointRequest == nil ? candidateTop : nil) ?? max(0, port.minY - content.minY)
        let width = ceil(min(p.offeredWidth + 32, port.width + 32) * CGFloat(scale)) / CGFloat(scale)
        let capacity = floor(CGFloat(8 * 1024 * 1024) / (width * CGFloat(scale) * 4)) / CGFloat(scale)
        let height = min(capacity, ceil(port.height * 2 * CGFloat(scale)) / CGFloat(scale))
        let before = min(port.height / 2, max(0, (height - port.height) / 2))
        let y = max(0, floor((top - before) * CGFloat(scale)) / CGFloat(scale))
        let box = CGRect(x: 0, y: 0, width: p.offeredWidth, height: p.height)
        let selection = node.presenter?.selection.range(node) ?? NSRange(location: 0, length: 0)
        let selected = NSColor.selectedTextBackgroundColor.withAlphaComponent(0.45).usingColorSpace(.sRGB)!
        let next = RegionRasterRequest(serial: serial + 1, publication: artifact.id, generation: 0,
            rows: [RegionPaintRow(artifact: artifact.id, box: box, selection: selection)],
            scroll: CGPoint(x: 0, y: y), size: CGSize(width: width, height: height), scale: scale,
            profile: profile, format: CGImageAlphaInfo.premultipliedLast.rawValue,
            // The existing view owns CSS backgrounds, including rounded corners
            // and opacity. Overflow ink must not repaint its ancestor's box.
            background: [0, 0, 0, 0],
            selectionColor: [selected.redComponent, selected.greenComponent, selected.blueComponent, selected.alphaComponent],
            interaction: pointRequest)
        if let old = wantedRaster, old.sameOutput(as: next) { return }
        if pointRequest == nil, let raster, raster.request.publication == artifact.id,
           raster.request.rows == next.rows, raster.request.size.width == next.size.width,
           raster.request.scale == scale,
           raster.request.scroll.y <= top, raster.request.scroll.y + raster.request.size.height >= top + port.height {
            return
        }
        guard next.bytes != nil else { failed = "Text viewport exceeds region pixel budget"; return }
        serial += 1; wantedRaster = next
        service.submit(.raster(next))
    }

    private func receive(_ answer: RegionAnswer) {
        RegionReaderTiming.begin()
        defer { RegionReaderTiming.end() }
        let started = CACurrentMediaTime()
        let post = Presenter.signposts.beginInterval("reader-publish")
        defer {
            Presenter.signposts.endInterval("reader-publish", post)
            maxPublish = max(maxPublish, CACurrentMediaTime() - started)
        }
        guard let node, node.text?.readerParagraphs[view] === self else { return }
        switch answer {
        case .shape(let artifact):
            guard artifact.metadata.offeredWidth == wantedWidth else { update(node); return }
            shapeWidth = nil; candidate = artifact
            candidateTop = anchoredTop(artifact.metadata)
            requestViewport()
        case .raster(let value):
            if let reply = value.interaction, reply.query == pointRequest {
                let complete = pointReply
                pointRequest = nil; pointReply = nil; wantedRaster = nil
                complete?(reply.index, reply.link)
                // The hit belongs to the displayed width. A candidate width
                // may have arrived meanwhile; its pixels still publish whole.
                update(node)
                return
            }
            guard wantedRaster?.serial == value.request.serial,
                  let artifact = candidate ?? accepted, value.request.publication == artifact.id,
                  artifact.metadata.offeredWidth == wantedWidth,
                  let image = MainActor.assumeIsolated({ value.image() }) else { return }
            let changed = accepted?.id != artifact.id
            let top = candidateTop
            accepted = artifact; candidate = nil; candidateTop = nil
            raster = value; self.image = image
            imageFrame = CGRect(origin: value.request.scroll, size: value.request.size)
            wantedRaster = nil
            publishing = true
            if changed, let session = node.presenter?.session {
                session.apply(session.runtime.textReady(index: index, generation: generation, revision: revision))
                if let top, let scroll = node.enclosingScrollView, let document = scroll.documentView {
                    let y = node.convert(CGPoint(x: 0, y: node.contentBox().minY + top), to: document).y
                    scroll.contentView.scroll(to: CGPoint(x: scroll.contentView.bounds.minX, y: max(0, y)))
                    scroll.reflectScrolledClipView(scroll.contentView)
                }
            }
            publishing = false
            node.needsDisplay = true
            node.presenter?.requestTextPublication()
        case .abandoned:
            shapeWidth = nil; update(node)
        case .refused(_, let reason):
            failed = reason
            node.presenter?.session?.log("reader region: \(reason)")
            node.needsDisplay = true
        }
    }

    /// A width change follows the source offset on the top visible line, with
    /// its fractional distance from that line. It never follows a height ratio.
    private func anchoredTop(_ next: RegionParagraph) -> CGFloat? {
        guard let old = accepted?.metadata, let node else { return nil }
        let port = node.presenter?.textScrollportRect(node) ?? node.visibleRect
        let top = max(0, port.minY - node.contentBox().minY)
        guard let line = old.lineIndex(at: top) else { return nil }
        let offset = old.lines[line].range.location
        var lo = 0, hi = next.lines.count
        while lo < hi {
            let mid = (lo + hi) / 2
            if NSMaxRange(next.lines[mid].range) <= offset { lo = mid + 1 } else { hi = mid }
        }
        let before = line == 0 ? 0 : old.lineBottoms[line - 1]
        let after = lo == 0 ? 0 : next.lineBottoms[min(lo, next.lineBottoms.count) - 1]
        return max(0, after + top - before)
    }

    func draw(_ node: NodeView, in context: CGContext, dirty: CGRect) {
        RegionReaderTiming.begin()
        defer { RegionReaderTiming.end() }
        let started = CACurrentMediaTime()
        let post = Presenter.signposts.beginInterval("reader-draw")
        defer {
            Presenter.signposts.endInterval("reader-draw", post)
            maxDraw = max(maxDraw, CACurrentMediaTime() - started)
        }
        update(node)
        let box = node.contentBox()
        if let image {
            let frame = imageFrame.offsetBy(dx: box.minX, dy: box.minY)
            context.saveGState()
            context.translateBy(x: frame.minX, y: frame.maxY)
            context.scaleBy(x: 1, y: -1)
            context.draw(image, in: CGRect(origin: .zero, size: frame.size))
            context.restoreGState()
        } else {
            let label = failed == nil ? "Preparing text…" : "Text unavailable"
            (label as NSString).draw(at: CGPoint(x: box.minX, y: max(box.minY, dirty.minY)),
                withAttributes: [.font: NSFont.systemFont(ofSize: 13), .foregroundColor: NSColor.secondaryLabelColor])
        }
    }

    func offset(at point: CGPoint, node: NodeView) -> Int? {
        guard let accepted, let raster else { return nil }
        let p = accepted.metadata
        let box = CGRect(origin: node.contentBox().origin, size: CGSize(width: p.offeredWidth, height: p.height))
        return p.cachedIndex(at: point, in: box, artifact: accepted.id, hits: raster.hits)
    }

    /// A huge overflowing line can exceed the bounded caret cache. Ask the
    /// existing raster worker for this exact point rather than guessing a caret
    /// or bringing its CTLine onto the UI executor. New motion replaces old.
    func resolveOffset(at point: CGPoint, node: NodeView, reply: @escaping (Int?, String?) -> Void) {
        guard let accepted else { reply(nil, nil); return }
        let p = accepted.metadata
        let box = CGRect(origin: node.contentBox().origin, size: CGSize(width: p.offeredWidth, height: p.height))
        if let index = offset(at: point, node: node) {
            reply(index, p.link(at: point, in: box, exactIndex: index)); return
        }
        serial += 1
        let local = CGPoint(x: point.x - box.minX, y: point.y - box.minY)
        pointRequest = RegionPointRequest(gesture: serial, sequence: 1, artifact: accepted.id,
            anchor: 0, begin: nil, point: local, terminal: true, dragged: false, selectAll: false)
        pointReply = reply
        requestViewport()
    }
}

extension TextEngine {
    /// Size and run count bound the ordinary synchronous unit. Known revisions
    /// use only the request header; no source decode/hash on intermediate widths.
    func readerMeasure(_ request: ExactMeasureRequest) -> ExactMetrics? {
        guard request.view != 0, request.width >= 0, request.exclusion_count == 0, request.line_clamp == 0 else {
            // Intrinsic offers retain the ordinary exact contract. Reader block
            // widths are definite; estimates never become min/max-content facts.
            readerParagraphs.removeValue(forKey: request.view)
            return nil
        }
        if let old = readerParagraphs[request.view], old.matches(request) {
            RegionReaderTiming.begin()
            defer { RegionReaderTiming.end() }
            return old.metrics(width: CGFloat(request.width))
        }
        let bytes = UnsafeBufferPointer(start: request.runs, count: request.count).reduce(0) { $0 + $1.len }
        guard bytes >= 64 * 1024 || (bytes >= 16 * 1024 && request.count >= 256) else {
            readerParagraphs[request.view] = nil; return nil
        }
        RegionReaderTiming.begin()
        defer { RegionReaderTiming.end() }
        let value = RegionReaderParagraph(request, bytes: bytes)
        readerParagraphs[request.view] = value
        return value.metrics(width: CGFloat(request.width))
    }
}

extension NodeView {
    var readerParagraph: RegionReaderParagraph? {
        guard flowShapes.isEmpty else { return nil }
        return text?.readerParagraphs[id]
    }
}
#endif

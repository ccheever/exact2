// The AppKit presenter (LLP 1008 §5): the page's scroll view over a flipped
// document, one `NodeView` per kernel node, the host's batches applied. It
// belongs to one session (LLP 1031 D1) and reaches the session's canvases,
// web views, and menus through it.
#if os(macOS)
import AppKit
import os

/// The viewport: a click that reached it — on no node that takes the focus
/// or a press — ends the editing, as a click on a page's blank ground blurs
/// the field (LLP 1008 §9).
final class PageScrollView: NSScrollView {
    private struct DocumentFit {
        let document: ObjectIdentifier
        let documentFrame: NSRect
        let clipSize: NSSize
        let frameSize: NSSize
        let style: NSScroller.Style
        let border: NSBorderType
        let vertical: Bool
        let horizontal: Bool
        let autohides: Bool
    }
    private var documentFit: DocumentFit?
    private var prefittingDocument = false
    var permitsDocumentPrefit: (() -> Bool)?

    func invalidateDocumentFit() { documentFit = nil }

    /// Only a completed root fit may certify axes for the next outer shrink.
    func acceptDocumentFit() {
        guard !prefittingDocument, let document = documentView,
              document.frame.origin == .zero, document.bounds.size == document.frame.size else { return }
        documentFit = DocumentFit(document: ObjectIdentifier(document), documentFrame: document.frame,
            clipSize: contentView.bounds.size, frameSize: frame.size, style: scrollerStyle,
            border: borderType, vertical: hasVerticalScroller, horizontal: hasHorizontalScroller,
            autohides: autohidesScrollers)
    }

    override func setFrameSize(_ newSize: NSSize) {
        if newSize != frame.size, !prefittingDocument {
            prefittingDocument = true
            prefitDocument(for: newSize)
            super.setFrameSize(newSize)
            prefittingDocument = false
        } else {
            super.setFrameSize(newSize)
        }
    }

    /// An old fitting document must not manufacture gutters during tiling.
    /// Keep overflowing axes and authored root frames untouched; ordinary layout
    /// can still introduce real overflow and its necessary viewport correction.
    private func prefitDocument(for newSize: NSSize) {
        let accepted = documentFit
        documentFit = nil
        guard permitsDocumentPrefit?() == true, let fit = accepted, let document = documentView,
              ObjectIdentifier(document) == fit.document, document.frame == fit.documentFrame,
              document.bounds.size == document.frame.size, frame.size == fit.frameSize,
              bounds.size == frame.size, contentView.bounds.size == fit.clipSize,
              scrollerStyle == fit.style, borderType == fit.border,
              hasVerticalScroller == fit.vertical, hasHorizontalScroller == fit.horizontal,
              autohidesScrollers == fit.autohides, !automaticallyAdjustsContentInsets,
              contentInsets.top == 0, contentInsets.left == 0,
              contentInsets.bottom == 0, contentInsets.right == 0,
              newSize.width.isFinite, newSize.height.isFinite,
              newSize.width > 0, newSize.height > 0 else { return }
        let width = fit.clipSize.width + newSize.width - fit.frameSize.width
        let height = fit.clipSize.height + newSize.height - fit.frameSize.height
        guard width.isFinite, height.isFinite, width > 0, height > 0 else { return }
        var size = document.frame.size
        if size.width <= fit.clipSize.width, newSize.width < fit.frameSize.width {
            size.width = min(size.width, width)
        }
        if size.height <= fit.clipSize.height, newSize.height < fit.frameSize.height {
            size.height = min(size.height, height)
        }
        if size != document.frame.size { document.setFrameSize(size) }
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(nil)
        super.mouseDown(with: event)
    }
    /// AppKit turns automatic titlebar insets back on when this view
    /// becomes a window's content view, which leaves a black strip the
    /// height of the titlebar above the document (the night, the deck).
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        pinInsets()
    }
    override func tile() {
        super.tile()
        pinInsets()
        syncElasticity()
    }
    /// A page that fits does not rubber-band.
    ///
    /// AppKit's `.automatic` elasticity bounces an axis the page cannot
    /// actually scroll, so an inner pane that reaches its end and chains the
    /// rest of the gesture up here (`ChainingScrollView`) drags the whole
    /// window's content — the app's own chrome with it — even though the
    /// document is exactly the viewport. The browser's rule is the one to
    /// match: an axis is elastic only while it has somewhere to go. Chaining
    /// itself is unchanged; a page that really does scroll still bounces.
    func syncElasticity() {
        let document = documentView?.frame.size ?? .zero
        let visible = contentView.bounds.size
        // Half a point of slack: a fractional layout must not read as scrollable.
        let x: NSScrollView.Elasticity = document.width - visible.width > 0.5 ? .automatic : .none
        let y: NSScrollView.Elasticity = document.height - visible.height > 0.5 ? .automatic : .none
        if horizontalScrollElasticity != x { horizontalScrollElasticity = x }
        if verticalScrollElasticity != y { verticalScrollElasticity = y }
    }
    func pinInsets() {
        if automaticallyAdjustsContentInsets { automaticallyAdjustsContentInsets = false }
        if contentInsets.top != 0 || contentInsets.left != 0 || contentInsets.bottom != 0 || contentInsets.right != 0 {
            contentInsets = NSEdgeInsetsZero
        }
    }
}

final class Presenter {
    /// Intervals a trace can lay beside its frames (Instruments' os_signpost):
    /// what the main thread spent on a list window, a batch, a text slice.
    static let signposts = OSSignposter(subsystem: "com.exact.host", category: "scroll")
    /// The session this presenter shows (LLP 1031 D1).
    weak var session: ExactSession?
    /// The document: the roots live here, content-sized like a page.
    let root = FlippedView(frame: .zero)
    /// The viewport over it: the window's content view, scrolling like a browser's.
    let viewport = PageScrollView(frame: .zero)
    var views: [UInt32: NodeView] = [:]
    var heightBindings: [UInt32: HeightDragBinding] = [:]
    var transformBindings: [UInt32: TransformDragBinding] = [:]
    lazy var transformGeometry = TransformGeometryHost(self)
    lazy var collections = CollectionHost(self)
    lazy var selection = TextSelection(self)
    let textRasters = TextRasterizer()
    lazy var mouseSwipe = MouseSwipe(self)
    lazy var mouseHeightDrag = MouseHeightDrag(self)
    lazy var mouseTransformDrag = MouseTransformDrag(self)
    private var scrollObserver: NSObjectProtocol?
    private var visibleText: [UInt32: NSRect] = [:]
    private var textViewportIndex: TextViewportIndex?
    /// The native menu arm (LLP 1021 D3).
    lazy var menus = MenuHost(presenter: self)
    lazy var navigation = NavigationHost(presenter: self)
    lazy var segments = SegmentHost(self)
    lazy var shortcuts = ShortcutHost(presenter: self)
    lazy var toolbar = WindowToolbarHost(self)
    /// The first root's `viewportFit` prop (`"cover"` or nothing), as of the
    /// last batch; `onViewportFit` fires when it changes. macOS maps `cover`
    /// to a full-size-content window (the titlebar overlays the viewport;
    /// its height is `safe-area-inset-top`). @ref LLP 1008 §9
    private(set) var viewportFit: String?
    var onViewportFit: (() -> Void)?
    /// The safe-area insets the kernel was given: the titlebar under
    /// `viewport-fit=cover`, zero when the viewport is the content view
    /// below it. Reported to the agent as `env`.
    var insets = NSEdgeInsetsZero

    init() {
        viewport.permitsDocumentPrefit = { [weak self] in
            guard let self else { return false }
            return !self.applying && !self.resetting
        }
        viewport.documentView = root
        viewport.hasVerticalScroller = true
        viewport.hasHorizontalScroller = true
        viewport.autohidesScrollers = true
        viewport.scrollerStyle = .overlay
        viewport.automaticallyAdjustsContentInsets = false
        viewport.contentInsets = NSEdgeInsetsZero
        viewport.drawsBackground = true
        viewport.backgroundColor = .white
        viewport.contentView.postsBoundsChangedNotifications = true
        scrollObserver = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification,
            object: viewport.contentView, queue: .main) { [weak self] _ in self?.scrolled(); self?.transformGeometry.changed() }
    }

    deinit {
        if let scrollObserver { NotificationCenter.default.removeObserver(scrollObserver) }
        pumpLink?.invalidate()
    }

    /// How far past its visible part a paragraph's text is painted, and how
    /// close to that painted edge the visible part may come before the band
    /// is painted again.
    ///
    /// AppKit scrolls a contained list on its own thread and the main thread
    /// follows (LLP 1044 F3), so whatever scrolls into view must already be
    /// painted: a strip painted when it is exposed is a strip shown blank
    /// first. Text is therefore painted for a band around the scrollport —
    /// never for a whole long document, which layer-backed AppKit would
    /// otherwise repaint offscreen — and a paragraph inside its band is only
    /// composited. Bands are admitted a few per frame by `pump`, nearest
    /// first, so mounting a row and rasterizing its text are different
    /// frames; only text already visible is painted at once.
    static let textBandReach: CGFloat = 1400
    static let textBandSlack: CGFloat = 500
    /// Paragraphs admitted to painting per pump slice, beyond the urgent ones.
    static let textBandsPerSlice = 2
    /// How far from the scrollport a paragraph's text is rasterized, and how
    /// many rasters one pump slice may ask a worker for. Asking costs the main
    /// thread a cached layout and an attributed string; the pixels are the
    /// worker's (TextRasterMac.swift).
    static let textRasterReach: CGFloat = 1600
    static let textRastersPerSlice = 6

    private func textBand(_ node: NodeView, reach: CGFloat) -> NSRect {
        guard let document = node.enclosingScrollView?.documentView else { return node.bounds }
        return node.convert(document.visibleRect, from: document)
            .insetBy(dx: -reach, dy: -reach).intersection(node.bounds)
    }

    /// The part of a paragraph whose text is painted: its band. A paragraph
    /// with none paints no text until it is admitted — unless it is on screen.
    func textVisibleRect(_ node: NodeView) -> NSRect {
        if let band = visibleText[node.id] { return band }
        guard !textBand(node, reach: 0).isEmpty else { return .zero }
        let band = textBand(node, reach: Self.textBandReach)
        visibleText[node.id] = band
        return band
    }

    /// Bring bands up to date. Visible paragraphs always; others up to
    /// `limit` of them (nil: all). True when some were left for a later slice.
    @discardableResult
    func refreshVisibleText(limit: Int? = nil) -> Bool {
        // Bounds notifications can arrive while a batch is still changing the
        // hierarchy. Query its final geometry once the outermost batch ends.
        guard !applying else { return false }
        if textViewportIndex == nil { textViewportIndex = TextViewportIndex(selection.paragraphs) }
        var next: [UInt32: NSRect] = [:]
        var waiting: [(CGFloat, NodeView)] = []
        var rasters: [(CGFloat, NodeView)] = []
        for node in textViewportIndex!.candidates(reach: Self.textRasterReach) where node.needsTextRaster && node.rastersText {
            // On screen without pixels: now. Otherwise nearest first, a few a slice.
            if !textBand(node, reach: 0).isEmpty { textRasters.ensure(node, urgent: true) }
            else { rasters.append((textBand(node, reach: Self.textRasterReach).height, node)) }
        }
        rasters.sort { $0.0 > $1.0 }
        var rasterBudget = limit.map { $0 == 0 ? 0 : Self.textRastersPerSlice } ?? rasters.count
        var rastersDeferred = false
        for (_, node) in rasters {
            guard rasterBudget > 0 else { rastersDeferred = true; break }
            rasterBudget -= 1
            if !textRasters.ensure(node, urgent: false) { rastersDeferred = true }
        }
        for node in textViewportIndex!.candidates(reach: Self.textBandSlack) where node.needsTextRaster && !node.rastersText {
            let want = textBand(node, reach: Self.textBandSlack)
            guard !want.isEmpty else { continue }
            let old = visibleText[node.id]
            if let old, old.contains(want) { next[node.id] = old; continue }
            let shown = textBand(node, reach: 0)
            if !shown.isEmpty, old.map({ !$0.contains(shown) }) ?? true {
                next[node.id] = admit(node, after: old)
            } else {
                if let old { next[node.id] = old }
                // Nearest the scrollport first: `want` is what is within slack.
                waiting.append((want.height * want.width, node))
            }
        }
        waiting.sort { $0.0 > $1.0 }
        var left = limit ?? waiting.count
        var deferred = false
        for (_, node) in waiting {
            guard left > 0 else { deferred = true; break }
            left -= 1
            next[node.id] = admit(node, after: visibleText[node.id])
        }
        visibleText = next
        return deferred || rastersDeferred
    }

    private func admit(_ node: NodeView, after old: NSRect?) -> NSRect {
        let band = textBand(node, reach: Self.textBandReach)
        for exposed in Self.exposedTextRects(band, after: old) { node.setNeedsDisplay(exposed) }
        return band
    }

    // MARK: The pump — list fill and text admission, a slice per frame

    private var pumpLink: CADisplayLink?
    private let pumpTarget = PumpTarget()
    private var listSyncPending = false
    private var textPending = false

    /// A scroll container moved. Nothing here may take long: AppKit is inside
    /// its scroll synchronizer, and the scrolling thread is waiting on it.
    func scrolled() {
        guard !applying else { return }
        let post = Self.signposts.beginInterval("scrolled")
        defer { Self.signposts.endInterval("scrolled", post) }
        // Most ticks move inside the band the mounted rows already cover: then
        // there is nothing to report, and nothing here reads or writes the
        // scroll view again until AppKit next calls in.
        switch listsNeed() {
        case .nothing: break
        case .soon: listSyncPending = true
        case .now: syncLists()
        }
        // Only what is already on screen without paint; the rest is pumped.
        textPending = refreshVisibleText(limit: 0) || textPending
        if listSyncPending || textPending { startPump() }
    }

    /// After a batch: paint what is visible now, admit the rest over frames.
    private func batchApplied() {
        syncLists()
        coverLists()
        if refreshVisibleText(limit: Self.textBandsPerSlice) { textPending = true; startPump() }
    }

    private func startPump() {
        guard pumpLink == nil else { return }
        // A display link, not a timer: a slice's commit must keep one phase
        // against the refresh. A free-running 120 Hz timer drifts through the
        // frame, and about once a second its commit landed in the scrolling
        // thread's own commit window and cost that frame — a hitch every
        // 1.2 s with this thread idle (LLP 1044, the pump's first version).
        pumpTarget.fire = { [weak self] in self?.pump() }
        let link = viewport.displayLink(target: pumpTarget, selector: #selector(PumpTarget.tick(_:)))
        // Only speculative list/text admission is paced here. AppKit owns
        // scrolling; visible gaps and the agent's settle path still fill now.
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: 30, preferred: 30)
        link.add(to: .main, forMode: .common)
        pumpLink = link
    }

    private func stopPump() {
        pumpLink?.invalidate()
        pumpLink = nil
    }

    /// Everything the pump owes, now. The agent's wheel is synchronous — it
    /// reads the tree right after — and so is anything that must not observe
    /// a half-filled window (LLP 1012: an agent never waits).
    func settlePump() {
        if listSyncPending { listSyncPending = false; syncLists() }
        refreshVisibleText()
        textPending = false
        stopPump()
    }

    /// One slice: the list window if it is owed, else a few text bands.
    private func pump() {
        if listSyncPending {
            listSyncPending = false
            let post = Self.signposts.beginInterval("pump-list")
            syncLists()
            Self.signposts.endInterval("pump-list", post)
            return
        }
        if textPending {
            let post = Self.signposts.beginInterval("pump-text")
            textPending = refreshVisibleText(limit: Self.textBandsPerSlice)
            Self.signposts.endInterval("pump-text", post)
        }
        if !listSyncPending && !textPending { stopPump() }
    }

    private enum ListNeed { case nothing, soon, now }

    /// What the mounted rows of each windowed list cover, from the last batch
    /// that changed them: `scrolled` compares an offset against four numbers
    /// instead of walking the rows on every tick.
    private struct ListCover {
        var top: CGFloat, bottom: CGFloat, origin: CGFloat
        var atStart: Bool, atEnd: Bool
    }
    private var listCovers: [UInt32: ListCover] = [:]

    private func coverLists() {
        listCovers.removeAll(keepingCapacity: true)
        for list in listViews.values {
            guard list.props["itemHeight"] != nil || list.props["estimatedItemHeight"] != nil,
                  let content = list.container.subviews.first as? NodeView else { continue }
            let rows = content.container.subviews.compactMap { $0 as? NodeView }
            let positions = rows.compactMap { Int($0.props["accessibilityPosInSet"] ?? "") }.sorted()
            // Focus and interaction can pin a distant row. Its bounds do not
            // cover the unmounted gap: use the normal viewport report until
            // the mounted rows are contiguous again.
            guard positions.count == rows.count, !positions.isEmpty,
                  zip(positions, positions.dropFirst()).allSatisfy({ $1 == $0 + 1 }) else { continue }
            var cover = ListCover(top: .infinity, bottom: -.infinity, origin: content.frame.minY, atStart: false, atEnd: false)
            for row in rows {
                let index = Int(row.props["accessibilityPosInSet"] ?? "") ?? 0, count = Int(row.props["accessibilitySetSize"] ?? "") ?? -1
                if row.frame.minY < cover.top { cover.top = row.frame.minY; cover.atStart = index <= 1 }
                if row.frame.maxY > cover.bottom { cover.bottom = row.frame.maxY; cover.atEnd = index == count }
            }
            if cover.top.isFinite { listCovers[list.id] = cover }
        }
    }

    /// Whether a windowed list has scrolled to where the runner would mount
    /// rows (its window is a scrollport either side), and whether it is close
    /// to showing past the mounted ones — a jump, a scroller drag, a fling
    /// faster than the pump. Then it fills now.
    private func listsNeed() -> ListNeed {
        var need = ListNeed.nothing
        for list in listViews.values {
            guard list.props["itemHeight"] != nil || list.props["estimatedItemHeight"] != nil, let scroll = list.scroll else { continue }
            guard let cover = listCovers[list.id] else { return .now }
            let visible = scroll.contentView.bounds, port = visible.height
            let first = cover.top + cover.origin, last = cover.bottom + cover.origin
            if (!cover.atEnd && visible.maxY + port * 0.35 > last) || (!cover.atStart && visible.minY - port * 0.35 < first) { return .now }
            if (!cover.atEnd && visible.maxY + port + 1 > last) || (!cover.atStart && visible.minY - port - 1 < first) { need = .soon }
        }
        return need
    }

    /// Scrolling exposes strips of an existing backing store. Repainting the
    /// overlapping area redraws every visible glyph on every scroll tick.
    /// Content/style changes still invalidate through the node's normal path.
    static func exposedTextRects(_ rect: NSRect, after previous: NSRect?) -> [NSRect] {
        guard !rect.isEmpty else { return [] }
        guard let previous else { return [rect] }
        let overlap = rect.intersection(previous)
        guard !overlap.isEmpty else { return [rect] }
        return [
            NSRect(x: rect.minX, y: rect.minY, width: rect.width, height: overlap.minY - rect.minY),
            NSRect(x: rect.minX, y: overlap.maxY, width: rect.width, height: rect.maxY - overlap.maxY),
            NSRect(x: rect.minX, y: overlap.minY, width: overlap.minX - rect.minX, height: overlap.height),
            NSRect(x: overlap.maxX, y: overlap.minY, width: rect.maxX - overlap.maxX, height: overlap.height),
        ].filter { !$0.isEmpty }
    }

    /// The viewport's size in points: what the kernel lays out under.
    var viewportSize: CGSize { viewport.contentSize }
    /// The first root's frame size, zero before the first batch.
    var rootSize: CGSize { root.subviews.first?.frame.size ?? .zero }
    /// The page's canvas colour: the first root's background (white when unset).
    var pageBackground: NSColor { viewport.backgroundColor }

    /// An asset's bytes changed (LLP 1030 D10): every image showing it loads
    /// it again — the old picture stays until the new one is decoded, as a
    /// browser keeps the old `src`.
    func assetChanged(_ name: String) {
        session?.rasters.invalidate(name)
        for v in views.values where v.kind == "image" && v.imageSource == name { v.loadImage(name) }
    }

    /// A restart: every view goes.
    func reset() {
        viewport.invalidateDocumentFit()
        session?.regions.reset()
        session?.rasters.reset()
        mouseSwipe.cancel()
        mouseHeightDrag.cancel()
        mouseTransformDrag.cancel()
        collections.reset()
        resetting = true
        defer { resetting = false }
        toolbar.reset()
        navigation.reset()
        segments.reset()
        session?.canvases.reset()
        views.values.forEach { $0.forget() }
        root.subviews.forEach { $0.removeFromSuperview() }
        views.removeAll()
        heightBindings.removeAll()
        transformBindings.removeAll()
        transformGeometry.reset()
        selection.structureChanged()
        visibleText.removeAll()
        textViewportIndex = nil
        stopPump()
        listCovers.removeAll()
        listSyncPending = false
        textPending = false
        listGeometry.removeAll()
        listViews.removeAll()
    }

    /// Size the document to its roots, never smaller than the viewport.
    func fitDocument() {
        viewport.invalidateDocumentFit()
        var size = viewport.contentSize
        for r in root.subviews {
            size.width = max(size.width, r.frame.maxX)
            size.height = max(size.height, r.frame.maxY)
        }
        if root.frame.size != size { root.frame = NSRect(origin: .zero, size: size) }
        // The document just changed size; whether the page can scroll — and
        // so whether it may bounce — changed with it.
        viewport.syncElasticity()
        viewport.acceptDocumentFit()
    }
    var onPress: ((UInt32) -> Void)?
    var onChange: ((UInt32, String) -> Void)?
    var onIntrinsic: ((UInt32, CGSize?) -> Void)?
    /// A capability an action called (LLP 1005 §3), after its commit.
    var onCommand: ((String, [Any]) -> Void)?

    /// The action's focus(html-id), delivered only after the batch is mounted.
    func focusElement(_ args: [Any], selectText: Bool = false) {
        guard args.count == 1, let name = args.first as? String,
              let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }),
              let window = target.window, !target.disabled,
              target.bounds.width > 0, target.bounds.height > 0 else { return }
        var ancestor: NSView? = target
        while let view = ancestor {
            if view.isHidden || (view as? NodeView)?.inert == true { return }
            ancestor = view.superview
        }
        if selectText, target.textArea == nil, target.field == nil { return }
        if let field = target.field, window.firstResponder === field.currentEditor() {
            if selectText { field.currentEditor()?.selectAll(nil) }
            return
        }
        let responder: NSView = target.textArea ?? target.field ?? target
        if responder.acceptsFirstResponder { window.makeFirstResponder(responder) }
        if selectText {
            if let editor = target.textArea, window.firstResponder === editor { editor.selectAll(nil) }
            else { target.field?.currentEditor()?.selectAll(nil) }
        }
    }

    /// The events beyond press and change (LLP 1005 §3).
    var onHover: ((UInt32, Bool) -> Void)?
    var onFocus: ((UInt32) -> Void)?
    var onBlur: ((UInt32) -> Void)?
    var onKey: ((UInt32, String) -> Void)?
    var onContextmenu: ((UInt32) -> Void)?
    var onDblclick: ((UInt32) -> Void)?
    var onSwiperight: ((UInt32) -> Void)?
    var onScroll: ((UInt32, Double, Double) -> Void)?
    var onList: ((UInt32, Double, Double, Double, Double, UInt32, UInt32) -> Void)?
    var onListIndex: ((UInt32, String) -> Int?)?
    var onListText: ((UInt32, (String, Int, Int)?, (String, Int, Int)?) -> String)?
    var interacting: UInt32 = 0
    private var listGeometry: [UInt32: [Double]] = [:]
    private var listViews: [UInt32: NodeView] = [:]
    private var listSyncDepth = 0
    private var listSyncQueued = false

    /// Fill and measure the row window before paint. Unusual documents with
    /// many zero-height rows continue next turn instead of recursing forever.
    func syncLists() {
        guard !applying else { return }
        guard listSyncDepth < 8 else {
            if !listSyncQueued {
                listSyncQueued = true
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    self.listSyncQueued = false
                    self.syncLists()
                }
            }
            return
        }
        listSyncDepth += 1
        defer { listSyncDepth -= 1 }
        listGeometry = listGeometry.filter { views[$0.key] != nil }
        for list in Array(listViews.values) {
            guard list.props["itemHeight"] != nil || list.props["estimatedItemHeight"] != nil else { continue }
            guard views[list.id] === list, let scroll = list.scroll,
                  let content = list.container.subviews.first as? NodeView else { continue }
            var responder = root.window?.firstResponder as? NSView
            if let owner = (responder as? NSTextView)?.delegate as? NSView { responder = owner }
            while responder != nil && !(responder is NodeView) { responder = responder?.superview }
            let focused = responder as? NodeView
            let focus = focused?.isDescendant(of: list) == true ? focused!.id : 0
            let interaction = views[interacting]?.isDescendant(of: list) == true ? interacting : 0
            let top = Double(scroll.contentView.bounds.minY)
            let height = Double(scroll.contentSize.height)
            let width = Double(content.frame.width)
            let origin = Double(content.frame.minY)
            let rows = content.container.subviews.compactMap { $0 as? NodeView }
            let stamp = [top, height, width, origin, Double(focus), Double(interaction)]
                + rows.flatMap { [Double($0.id), Double($0.frame.height)] }
            if listGeometry[list.id] == stamp { continue }
            listGeometry[list.id] = stamp
            onList?(list.id, top, height, width, origin, focus, interaction)
        }
    }
    var onSubmit: ((UInt32) -> Void)?
    var onLoad: ((UInt32) -> Void)?
    var onMessage: ((UInt32, String) -> Void)?
    /// The node the pointer is over, of those with a hover handler: it hears
    /// the leave when the pointer moves onto another (the agent's `hover`).
    weak var hovered: NodeView?

    func press(_ id: UInt32) {
        guard let node = views[id], !node.inert else { return }
        onPress?(id)
        // An invoker's press also drops its menu (LLP 1021 D3).
        menus.pressed(id)
    }
    func change(_ id: UInt32, _ value: String) { onChange?(id, value) }
    /// An event a view reports: sent only while the presenter still has the
    /// view (the platform fires editing-ended as a destroyed field leaves the
    /// window; the browser fires no blur on removal, so neither does this
    /// host), and never while a batch is being applied — it waits for the
    /// batch to finish, then goes if its view survived it.
    private var applying = false
    private var resetting = false
    private var pendingGeometry: (() -> Void)?

    /// Window chrome can synchronously resize ExactView while an older batch
    /// is still being installed. Commit its geometry after that batch, so the
    /// remainder cannot overwrite the newer inset/layout result.
    func deferGeometry(_ update: @escaping () -> Void) -> Bool {
        guard applying || resetting else { return false }
        pendingGeometry = update
        return true
    }
    private var waiting: [(UInt32, () -> Void)] = []
    private func send(_ id: UInt32, _ f: @escaping () -> Void) {
        guard views[id] != nil else { return }
        if applying { waiting.append((id, f)) } else { f() }
    }
    func hover(_ view: NodeView, _ over: Bool) {
        guard views[view.id] === view else { return }
        if over {
            if let h = hovered, h !== view { send(h.id) { [self] in onHover?(h.id, false) } }
            hovered = view
            send(view.id) { [self] in onHover?(view.id, true) }
        } else {
            if hovered === view { hovered = nil }
            send(view.id) { [self] in onHover?(view.id, false) }
        }
    }
    func focus(_ id: UInt32) { send(id) { [self] in onFocus?(id) } }
    func blur(_ id: UInt32) { send(id) { [self] in onBlur?(id) } }
    func key(_ id: UInt32, _ name: String) { send(id) { [self] in onKey?(id, name) } }
    func contextmenu(_ id: UInt32) { send(id) { [self] in onContextmenu?(id) } }
    func dblclick(_ id: UInt32) { send(id) { [self] in onDblclick?(id) } }
    func swiperight(_ id: UInt32) { send(id) { [self] in onSwiperight?(id) } }
    func scroll(_ id: UInt32, _ left: Double, _ top: Double) { send(id) { [self] in onScroll?(id, left, top) } }
    func submit(_ id: UInt32) { send(id) { [self] in onSubmit?(id) } }
    func load(_ id: UInt32) { send(id) { [self] in onLoad?(id) } }
    func message(_ id: UInt32, _ value: String) { send(id) { [self] in onMessage?(id, value) } }
    func intrinsic(_ id: UInt32, _ size: CGSize?) { onIntrinsic?(id, size) }

    func apply(_ batch: Batch) {
        let post = Self.signposts.beginInterval("apply", "\(batch.ops.count) ops")
        defer { Self.signposts.endInterval("apply", post) }
        viewport.invalidateDocumentFit()
        collections.beginBatch(batch)
        toolbar.prepare()
        for node in views.values where !collections.owns(node.id) { node.captureScrollPosition() }
        if let e = batch.error { FileHandle.standardError.write(Data("exact: \(e)\n".utf8)) }
        let outermost = !applying
        applying = true
        defer {
            collections.endBatch()
            if outermost {
                applying = false
                let geometry = pendingGeometry
                pendingGeometry = nil
                let q = waiting
                waiting = []
                geometry?()
                for (id, f) in q where views[id] != nil { f() }
                batchApplied()
            }
        }
        if !batch.ops.isEmpty { textViewportIndex = nil }
        let structureChanged = batch.ops.contains { ["children", "roots", "destroy", "create", "style"].contains($0["op"] as? String ?? "") }
        if structureChanged { selection.structureChanged() }
        for op in batch.ops {
            guard let kind = op["op"] as? String else { continue }
            let id = UInt32(op["id"] as? Int ?? 0)
            if kind == "children" { touched(id, children: true) } else if kind != "roots" && kind != "create" { touched(id, textChanged: kind == "props" || kind == "style" || kind == "destroy") }
            switch kind {
            case "transform-drag":
                if let binding = TransformDragBinding(op) {
                    if binding.target == nil {
                        if transformBindings[binding.id]?.handleKey == binding.handleKey
                            && transformBindings[binding.id]?.runtime == binding.runtime {
                            transformBindings.removeValue(forKey: binding.id)
                            transformGeometry.retire(binding.id)
                        }
                    } else { transformBindings[binding.id] = binding }
                }
            case "retire-motion":
                if let rawRuntime = op["runtime"] as? String, let runtime = UInt64(rawRuntime),
                   let rawToken = op["token"] as? String, let token = UInt64(rawToken) {
                    session?.transformInputHold?.retire(runtime: runtime, token: token)
                }
            case "height-drag":
                if let binding = HeightDragBinding(op) {
                    if binding.target == nil {
                        if heightBindings[binding.id]?.handleKey == binding.handleKey {
                            heightBindings.removeValue(forKey: binding.id)
                        }
                    } else { heightBindings[binding.id] = binding }
                }
            case "create":
                let v = NodeView(id: id, kind: op["kind"] as? String ?? "view", presenter: self)
                v.handlers = Set(op["handlers"] as? [String] ?? [])
                v.applyStyle(op["style"] as? [String: Any] ?? [:])
                v.applyProps(set: op["props"] as? [String: String] ?? [:], clear: [])
                views[id] = v
                if v.kind == "list" { listViews[id] = v }
            case "props":
                views[id]?.applyProps(set: op["set"] as? [String: String] ?? [:], clear: op["clear"] as? [String] ?? [])
            case "style":
                views[id]?.applyStyle(op["style"] as? [String: Any] ?? [:])
            case "children":
                guard let parent = views[id] else { continue }
                let want = (op["ids"] as? [Int] ?? []).compactMap { views[UInt32($0)] }
                if parent.kind == "text" { parent.setTextChildren(want); continue }
                for child in want { child.textParent = nil }
                let container = parent.container
                for child in container.subviews where !(want as [NSView]).contains(child) && child is NodeView { child.removeFromSuperview() }
                for (i, child) in want.enumerated() {
                    if child.superview !== container {
                        child.prepareToMount()
                        container.addSubview(child)
                    }
                    if container.subviews.firstIndex(of: child) != i {
                        child.removeFromSuperview()
                        container.addSubview(child, positioned: .above, relativeTo: i > 0 ? want[i - 1] : nil)
                    }
                }
            case "surface":
                if let v = views[id] { session?.canvases.surface(view: v, name: op["name"] as? String ?? "", values: op["values"] as? [Any] ?? []) }
            case "command":
                onCommand?(op["name"] as? String ?? "", op["args"] as? [Any] ?? [])
            case "destroy":
                mouseSwipe.retire(id)
                mouseHeightDrag.retire(id)
                mouseTransformDrag.retire(id)
                session?.canvases.destroy(view: id)
                views[id]?.forget()
                // Out of the map before out of the window: the editing-ended
                // notification removal fires finds no view to send for.
                heightBindings.removeValue(forKey: id)
                transformBindings.removeValue(forKey: id)
                transformGeometry.retire(id)
                let gone = views.removeValue(forKey: id)
                listViews.removeValue(forKey: id)
                gone?.removeFromSuperview()
            case "roots":
                root.subviews.forEach { $0.removeFromSuperview() }
                for r in (op["ids"] as? [Int] ?? []).compactMap({ views[UInt32($0)] }) {
                    r.prepareToMount()
                    root.addSubview(r)
                }
            case "frame":
                guard let v = views[id] else { continue }
                v.frame = NSRect(x: op["x"] as? Double ?? 0, y: op["y"] as? Double ?? 0, width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                v.scroll?.frame = v.bounds
                v.field?.frame = v.contentBox()
                v.layoutTextArea()
                v.metal?.frame = v.bounds
                v.overlay?.frame = v.bounds
                v.web?.frame = v.bounds
                v.fitScroll()
                v.applyTransform()
            case "content":
                if let v = views[id] {
                    v.content = CGSize(width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                    v.fitScroll()
                }
            case "present":
                guard let v = views[id] else { continue }
                let x = CGFloat(op["x"] as? Double ?? 0)
                switch op["property"] as? String {
                case "translate": v.translate = CGPoint(x: x, y: CGFloat(op["y"] as? Double ?? 0)); v.applyTransform()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.alphaValue = x
                default: break
                }
            default: break
            }
        }
        navigation.sync()
        fitDocument()
        // The page's canvas colour is the first root's background — what
        // shows beyond a document shorter than the viewport, as a browser
        // paints the root element's background over the whole canvas.
        let color = (root.subviews.first as? NodeView)?.color("background_color", .white) ?? .white
        if viewport.backgroundColor != color { viewport.backgroundColor = color }
        let first = root.subviews.first as? NodeView
        let fit = first?.props["viewportFit"]
        if fit != viewportFit { viewportFit = fit; onViewportFit?() }
        session?.canvases.captureIfNeeded()
        for node in views.values {
            if !collections.owns(node.id) { node.restoreScrollPosition() }
            if node.pendingScrollTop != nil || node.pendingScrollLeft != nil { collections.userIntent(node.id) }
            node.applyPendingScroll()
        }
        segments.sync()
        menus.sync()
        positionContexts()
        toolbar.sync()
        shortcuts.sync()
        if structureChanged { selection.structureChanged() }
        if structureChanged || batch.ops.contains(where: { $0["op"] as? String == "props" }) { syncKeyViewLoop() }
    }

    /// Align an enclosing context panel's preview with its source, while
    /// keeping the panel inside the visible viewport.
    private func positionContexts() {
        for preview in views.values {
            guard let target = preview.props["contextTarget"],
                  let source = views.values.first(where: { $0.props["id"] == target }),
                  source.window != nil else { continue }
            var ancestor = preview.superview as? NodeView
            while let node = ancestor, node.style["position_type"] as? String != "absolute" {
                ancestor = node.superview as? NodeView
            }
            guard let panel = ancestor, let parent = panel.superview else { continue }
            let sourceBox = source.convert(source.bounds, to: parent)
            let port = viewport.convert(viewport.bounds, to: parent)
            let minimum = max(parent.bounds.minY, port.minY + 8)
            let maximum = min(parent.bounds.maxY, port.maxY - 8) - panel.bounds.height
            let top = max(minimum, min(sourceBox.minY - preview.convert(preview.bounds, to: panel).minY, maximum))
            if panel.frame.origin.y != top { panel.setFrameOrigin(CGPoint(x: panel.frame.minX, y: top)) }
        }
    }

    /// The view that takes Tab for this node: an input's field, else itself.
    private func keyView(of v: NodeView) -> NSView { v.textArea ?? v.field ?? v }

    /// Sequential focus after a batch: tree order, then `tabIndex` > 0, as
    /// HTML. `autorecalculatesKeyViewLoop` stays false so nothing is focused
    /// at launch (LLP 1014); Tab from the viewport still reaches the first
    /// tabbable. Hidden popover rows stay out (their container is hidden).
    func syncKeyViewLoop() {
        var listed: [NodeView] = []
        func walk(_ v: NodeView) {
            if v.inert || v.isHidden { return }
            if Self.tabbable(v) { listed.append(v) }
            for child in v.container.subviews.compactMap({ $0 as? NodeView }) { walk(child) }
        }
        for r in root.subviews.compactMap({ $0 as? NodeView }) { walk(r) }
        let tabbable = listed.enumerated().sorted { a, b in
            let ia = Self.tabIndex(a.element), ib = Self.tabIndex(b.element)
            let pa = ia > 0 ? ia : Int.max, pb = ib > 0 ? ib : Int.max
            if pa != pb { return pa < pb }
            return a.offset < b.offset
        }.map(\.element)
        if tabbable.isEmpty {
            viewport.nextKeyView = nil
            return
        }
        for (i, v) in tabbable.enumerated() {
            keyView(of: v).nextKeyView = keyView(of: tabbable[(i + 1) % tabbable.count])
        }
        viewport.nextKeyView = keyView(of: tabbable[0])
    }

    private static func tabIndex(_ v: NodeView) -> Int { Int(v.props["tabIndex"] ?? "0") ?? 0 }

    private static func tabbable(_ v: NodeView) -> Bool {
        if v.props["disabled"] == "true" { return false }
        let index = tabIndex(v)
        if index < 0 { return false }
        if v.field != nil || v.textArea != nil { return true }
        if v.kind == "button" || v.kind == "toggle" || v.handlers.contains("press") { return true }
        if v.acceptsFirstResponder { return true }
        return index > 0
    }

    /// An op touched a node (LLP 1014 D4 a): every canvas it is painted
    /// through captures again at the end of the batch — the canvas above
    /// it, and itself for its own `children` op.
    func touched(_ id: UInt32, children: Bool = false, textChanged: Bool = false) {
        guard let start = views[id] else { return }
        var paragraph: NodeView? = start
        while let node = paragraph, node.kind == "text" {
            if textChanged || children { node.invalidateText() }
            node.needsDisplay = true
            paragraph = node.textParent ?? node.superview as? NodeView
        }
        if children, start.overlay != nil { start.needsCapture = true }
        if let c = start.paragraphOwner.canvasAbove { c.needsCapture = true }
    }
}

/// A view's subtree as pixels (LLP 1014 D3).
enum Capture {
    /// A capture is drawing: its draws are not repaints (D4 b).
    nonisolated(unsafe) static var capturing = false
    /// Guest pictures for this turn; the remote platform views are hidden
    /// while their owning nodes draw these (@ref LLP 1020 D4/D6).
    nonisolated(unsafe) static var web: [UInt32: ExactWebImage] = [:]

    /// The subtree painted at `scale`: premultiplied RGBA, rows top-down,
    /// `pixelsWide * 4` bytes per row, transparent where nothing painted.
    static func bitmap(of view: NSView, scale: CGFloat) -> NSBitmapImageRep? {
        let w = Int((view.bounds.width * scale).rounded()), h = Int((view.bounds.height * scale).rounded())
        guard w > 0, h > 0,
              let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: w * 4, bitsPerPixel: 32)
        else { return nil }
        rep.size = view.bounds.size
        if let p = rep.bitmapData { memset(p, 0, h * w * 4) }
        // A subtree painted through its canvas composites at alpha 0; paint
        // it opaque into the bitmap regardless.
        let alpha = view.alphaValue
        view.alphaValue = 1
        // A canvas nested under this one that is painted through its own
        // surface: its picture comes by readback (its draw), not from its
        // overlay's views, which cacheDisplay would paint regardless of their
        // alpha — so those are hidden for the duration.
        var hidden: [NSView] = []
        func hide(_ v: NSView) {
            for s in v.subviews {
                if let n = s as? NodeView, let o = n.overlay, o.alphaValue == 0, !o.isHidden { o.isHidden = true; hidden.append(o); continue }
                hide(s)
            }
        }
        hide(view)
        capturing = true
        view.cacheDisplay(in: view.bounds, to: rep)
        capturing = false
        for o in hidden { o.isHidden = false }
        view.alphaValue = alpha
        return rep
    }
}

/// Paragraph geometry in each scroll document's own coordinates. A scroll
/// changes the query rectangle, never the index. Layout/structure changes
/// discard it. Selection and copy keep their complete document order.
/// @ref LLP 1033 (long documents), LLP 1010 (native scrolling)
struct TextViewportIndex {
    private struct Entry {
        let node: NodeView
        let rect: NSRect
        var bottom: CGFloat
    }
    private struct Group {
        let document: NSView
        var entries: [Entry]
    }
    private var groups: [Group] = []

    init(_ paragraphs: [NodeView]) {
        var positions: [ObjectIdentifier: Int] = [:]
        for node in paragraphs {
            guard let document = node.enclosingScrollView?.documentView else { continue }
            let key = ObjectIdentifier(document)
            let index: Int
            if let found = positions[key] { index = found }
            else {
                index = groups.count
                positions[key] = index
                groups.append(Group(document: document, entries: []))
            }
            let rect = node.convert(node.bounds, to: document)
            groups[index].entries.append(Entry(node: node, rect: rect, bottom: rect.maxY))
        }
        for i in groups.indices {
            groups[i].entries.sort { $0.rect.minY < $1.rect.minY }
            var bottom = -CGFloat.infinity
            for j in groups[i].entries.indices {
                bottom = max(bottom, groups[i].entries[j].rect.maxY)
                groups[i].entries[j].bottom = bottom
            }
        }
    }

    /// Paragraphs within `reach` of each scroll document's visible rect.
    func candidates(reach: CGFloat = 0) -> [NodeView] {
        var result: [NodeView] = []
        for group in groups {
            let shown = group.document.visibleRect
            guard !shown.isEmpty else { continue }
            let visible = shown.insetBy(dx: -reach, dy: -reach)
            // Prefix maxima include tall/overlapping paragraphs that start
            // before the viewport. Binary-searching only minY loses them.
            var lo = 0, hi = group.entries.count
            while lo < hi {
                let mid = lo + (hi - lo) / 2
                if group.entries[mid].bottom <= visible.minY { lo = mid + 1 }
                else { hi = mid }
            }
            var index = lo
            while index < group.entries.count && group.entries[index].rect.minY < visible.maxY {
                let entry = group.entries[index]
                if entry.rect.intersects(visible) { result.append(entry.node) }
                index += 1
            }
        }
        return result
    }
}

extension NSRect {
    /// The rect inside the given edges (never negative in size).
    func insetBy(left: CGFloat, top: CGFloat, right: CGFloat, bottom: CGFloat) -> NSRect {
        NSRect(x: minX + left, y: minY + top, width: max(0, width - left - right), height: max(0, height - top - bottom))
    }
}
/// The display link's Objective-C target: `Presenter` is not an `NSObject`.
final class PumpTarget: NSObject {
    var fire: (() -> Void)?
    @objc func tick(_ link: CADisplayLink) { fire?() }
}
#endif

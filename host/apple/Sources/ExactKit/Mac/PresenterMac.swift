// The AppKit presenter (LLP 1008 §5): the page's scroll view over a flipped
// document, one `NodeView` per kernel node, the host's batches applied. It
// belongs to one session (LLP 1031 D1) and reaches the session's canvases,
// web views, and menus through it.
#if os(macOS)
import AppKit
import CoreText
import os

/// Which live views carry the few props the chrome passes look for.
///

final class Presenter {
    var documentLanguage = ""
    var documentDirection = "ltr"
    /// Intervals a trace can lay beside its frames (Instruments' os_signpost):
    /// what the main thread spent on a list window, a batch, a text slice.
    static let signposts = OSSignposter(subsystem: "com.exact.host", category: "scroll")
    var autofocusProcessed: Set<ObjectIdentifier> = []
    /// The session this presenter shows (LLP 1031 D1).
    weak var session: ExactSession?
    /// The document: the roots live here, content-sized like a page.
    let root = FlippedView(frame: .zero)
    /// The viewport over it: the window's content view, scrolling like a browser's.
    let viewport = PageScrollView(frame: .zero)
    var views: [UInt32: NodeView] = [:]
    var inlineOwners: [UInt32: (owner: UInt32, index: Int)] = [:]
    private(set) var chrome = ChromeIndex()
    /// Views leaving with their exit, by id (LLP 1063, `PresenceMac.swift`).
    var leaving: [UInt32: Leaving] = [:]
    /// A view's props were written (`NodeView.props`' own observer).
    func propsChanged(_ view: NodeView) { chrome.note(view.id, props: view.props) }
    /// Views carrying an indexed prop, in id order (the passes' old order was
    /// a dictionary's, which is none).
    func carrying(_ key: String) -> [NodeView] {
        chrome.ids(key).sorted().compactMap { views[$0] }
    }
    /// Reparenting into/out of the top layer changes text's paint/selection walk.
    func topLayerChanged() {
        selection.clear()
        selection.structureChanged()
        textViewportIndex = nil
        refreshVisibleText()
    }
    /// Scroll containers: the only views with a position to keep across a batch.
    var scrollers: Set<UInt32> = []
    let glassGroups = GlassGroups()
    /// Views with an authored offset waiting for their frames.
    var pendingScrolls: Set<UInt32> = []
    var heightBindings: [UInt32: HeightDragBinding] = [:]
    var transformBindings: [UInt32: TransformDragBinding] = [:]
    lazy var transformGeometry = TransformGeometryHost(self)
    var videoVisibility: VideoVisibilityHost?
    lazy var collections = CollectionHost(self)
    /// Heavy leaves held while their rows are far or flying (LLP 1068 §5.1).
    lazy var leaves = HeavyLeaves(self)
    lazy var selection = TextSelection(self)
    let textRasters = TextRasterizer()
    lazy var mouseSwipe = MouseSwipe(self)
    lazy var mouseLayoutPan = MouseLayoutPan(self)
    lazy var mouseHeightDrag = MouseHeightDrag(self)
    lazy var mouseTransformDrag = MouseTransformDrag(self)
    lazy var mouseReorder = MouseReorder(self)
    lazy var mouseChain = MouseChain(self)
    /// The one Arrange contact, until its source settles; a test's calls.
    var reorder: ReorderHold?
    var reorderCalls: ReorderCalls?
    private var scrollObserver: NSObjectProtocol?
    private var visibleText: [UInt32: NSRect] = [:]
    private var textViewportIndex: TextViewportIndex?
    /// The native menu arm (LLP 1021 D3).
    lazy var menus = MenuHost(presenter: self)
    lazy var dialogs = DialogHost(self)
    lazy var navigation = NavigationHost(presenter: self)
    /// SVG scenes and CSS animations (LLP 1055 D4, D7).
    let svg = SvgHost()
    /// Boxes under CSS `filter`, drawn again after each batch (LLP 1055.000 D14).
    let boxFilters = BoxFilters()
    let canvas2d = Canvas2DHost()
    lazy var segments = SegmentHost(self)
    lazy var controls = ControlHost(self)
    lazy var shortcuts = ShortcutHost(presenter: self)
    lazy var toolbar = WindowToolbarHost(self)
    /// The head's title goes to the window the app attached, through the
    /// toolbar host that already owns its title (LLP 1048.003 D1).
    func headTitle(_ title: String?) { toolbar.headTitle(title) }
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
        // LLP 1050.000 stage 1: a collection reports its travel and builds
        // ahead in the pump's slices.
        collections.motion = { [unowned self] id in
            let velocity = listVelocity(id)
            return velocity == 0 ? nil : velocity
        }
        // A slice's own continuation waits for the next frame's link.
        collections.requestFill = { [unowned self] in
            startPump()
            if !pumping { queuePostSyncSlice() }
        }
        scrollObserver = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification,
            object: viewport.contentView, queue: .main) { [weak self] _ in self?.scrolled(); self?.transformGeometry.changed(); self?.videoVisibility?.changed() }
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

    /// A paragraph's own visibleRect can include AppKit's overdraw. Urgency
    /// follows the scroll document's clipped viewport instead.
    func textIsVisible(_ node: NodeView) -> Bool { !textScrollportRect(node).isEmpty }
    func textScrollportRect(_ node: NodeView) -> NSRect { textBand(node, reach: 0) }

    /// The part of a paragraph whose text is painted: its band. A paragraph
    /// with none paints no text until it is admitted — unless it is on screen.
    func textVisibleRect(_ node: NodeView) -> NSRect {
        if node.textRasterUsesStrips { return textScrollportRect(node) }
        if let band = visibleText[node.id] { return band }
        guard textIsVisible(node) else { return .zero }
        let band = textBand(node, reach: Self.textBandReach)
        visibleText[node.id] = band
        return band
    }

    /// Bring bands up to date. Visible paragraphs always; others up to
    /// `limit` of them (nil: all). True when some were left for a later slice.
    @discardableResult
    func refreshVisibleText(limit: Int? = nil, afterFrame: Bool = false) -> Bool {
        // Bounds notifications can arrive while a batch is still changing the
        // hierarchy. Query its final geometry once the outermost batch ends.
        guard !applying else { return false }
        if textViewportIndex == nil { textViewportIndex = TextViewportIndex(selection.paragraphs) }
        var next: [UInt32: NSRect] = [:]
        var waiting: [(CGFloat, NodeView)] = []
        var rasters: [NodeView] = []
        let candidates = textViewportIndex!.candidates(reach: Self.textRasterReach)
        var readersDeferred = false
        for node in candidates {
            node.readerParagraph?.update(node, afterFrame: afterFrame)
            readersDeferred = readersDeferred || node.readerParagraph?.waitingForPixels == true
        }
        let replacementsDeferred = !inScrollCallback && textRasters.replaceVisible(candidates, wait: sliceBudget)
        var rastersDeferred = false
        for node in candidates where node.needsTextRaster && node.rastersText {
            // On screen without pixels: now. Otherwise nearest first, a few a slice.
            if textIsVisible(node) {
                if inScrollCallback {
                    // AppKit paints these first pixels in updateLayer, outside
                    // its scroll synchronizer but before the display commit.
                    node.needsDisplay = true
                    rastersDeferred = true
                } else { textRasters.ensure(node, urgent: true) }
            } else { rasters.append(node) }
        }
        var rasterBudget = limit.map { $0 == 0 ? 0 : Self.textRastersPerSlice } ?? rasters.count
        for node in rasters {
            guard rasterBudget > 0 else { rastersDeferred = true; break }
            rasterBudget -= 1
            if !textRasters.ensure(node, urgent: false) { rastersDeferred = true }
        }
        // Region readers own their pixel bands and publish positioned layers.
        // The ordinary backing-store band must not invalidate them a second time.
        for node in textViewportIndex!.candidates(reach: Self.textBandSlack)
            where node.readerParagraph == nil && node.needsTextRaster && !node.rastersText && !node.textIsSmall {
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
        return deferred || rastersDeferred || replacementsDeferred || readersDeferred
    }

    private func admit(_ node: NodeView, after old: NSRect?) -> NSRect {
        let band = textBand(node, reach: Self.textBandReach)
        for exposed in Self.exposedTextRects(band, after: old) { node.setNeedsDisplay(exposed) }
        return band
    }

    // MARK: The pump — list fill and text admission, a slice per frame

    private var pumpLink: CADisplayLink?
    private let pumpTarget = PumpTarget()
    private var inScrollCallback = false
    private var textPending = false
    private var pumping = false
    private var pumpSchedule = PumpSchedule()
    private var pumpScreen: UInt32?
    private var refreshInterval: TimeInterval { pumpSchedule.refreshInterval }
    /// Scroll notifications can arrive repeatedly before the next refresh.
    /// One queued turn leaves AppKit's synchronizer first; the display link
    /// remains the fallback when notifications stop. Tokens retire old turns
    /// without allowing them to consume a new session's pending work.
    struct PumpSchedule {
        private(set) var refreshInterval: TimeInterval = 1.0 / 60
        private var queued: UInt64?
        private var serial: UInt64 = 0
        private var lastPostSyncSlice: TimeInterval?

        mutating func updateInterval(_ interval: TimeInterval) {
            if interval.isFinite && interval > 0 { refreshInterval = interval }
        }
        private func recentPostSyncSlice(at time: TimeInterval) -> Bool {
            lastPostSyncSlice.map { time - $0 < refreshInterval * 0.75 } ?? false
        }
        mutating func queuePostSync(at time: TimeInterval) -> UInt64? {
            guard queued == nil, !recentPostSyncSlice(at: time) else { return nil }
            serial &+= 1
            queued = serial
            return serial
        }
        mutating func takePostSync(_ token: UInt64, at time: TimeInterval) -> Bool {
            guard queued == token else { return false }
            queued = nil
            lastPostSyncSlice = time
            return true
        }
        mutating func takeDisplayLink(interval: TimeInterval, at time: TimeInterval) -> Bool {
            updateInterval(interval)
            return queued == nil && !recentPostSyncSlice(at: time)
        }
        mutating func cancel() {
            queued = nil
            // Emptying or settling the pump cancels its queued turn, but
            // fresh work in this same frame still shares the slice allowance.
        }
    }
    static func listSliceBudget(_ interval: TimeInterval) -> TimeInterval {
        min(0.004, max(0.001, interval * 0.24))
    }
    var sliceBudget: TimeInterval { Self.listSliceBudget(refreshInterval) }
    /// Charge the report (decode, layout and apply) to the
    /// rows it created. Cheap rows share those fixed costs in the next report.
    /// Grow at most twofold and retain recent expensive samples conservatively.
    struct ListFillCost {
        private(set) var secondsPerRow: TimeInterval?
        private var lastRows = 1
        mutating func record(seconds: TimeInterval, rows: Int) {
            guard rows > 0 else { return }
            let sample = max(0.000001, seconds / Double(rows))
            secondsPerRow = max(sample, (secondsPerRow ?? sample) * 0.75 + sample * 0.25)
            lastRows = rows
        }
        func rows(within remaining: TimeInterval) -> UInt32 {
            guard remaining > 0 else { return 0 }
            guard let secondsPerRow else { return 1 }
            let fitting = floor(remaining * 0.9 / secondsPerRow)
            return UInt32(max(0, min(Double(lastRows) * 2, fitting, Double(UInt32.max - 1))))
        }
    }
    private var listFillCosts: [UInt32: ListFillCost] = [:]
    private struct ListTravel {
        var top: CGFloat, time: TimeInterval, velocity: Double = 0, step: Double = 0
    }
    private var listTravel: [UInt32: ListTravel] = [:]
    func listVelocity(_ id: UInt32) -> Double {
        guard !ExactEnv.agentMode, let travel = listTravel[id],
              CACurrentMediaTime() - travel.time < 0.15 else { return 0 }
        return travel.velocity
    }
    private func sampleListTravel(only: UInt32? = nil) {
        let now = CACurrentMediaTime()
        for (id, list) in listViews where only == nil || only == id {
            guard let scroll = list.scroll else { continue }
            let port = scroll.contentView.bounds
            // Along the list's own axis (LLP 1070 H3): a row list travels on x.
            let horizontal = collections.entries[id]?.snapshot.horizontal == true
            let at = horizontal ? port.minX : port.minY
            var travel = listTravel[id] ?? ListTravel(top: at, time: now)
            let delta = at - travel.top, elapsed = now - travel.time
            // A step longer than the port that the travel so far doesn't
            // predict is a jump, not travel (as ScrollPumpIOS.sample).
            let length = horizontal ? port.width : port.height
            let predicted = CGFloat((travel.velocity != 0 ? travel.velocity : travel.step) * max(elapsed, 0))
            if abs(delta) > length && abs(delta - predicted) > length {
                listTravel[id] = ListTravel(top: at, time: now, step: elapsed > 0 ? Double(delta) / max(elapsed, refreshInterval / 2) : 0)
                continue
            }
            if delta != 0, elapsed > 0 {
                let speed = Double(delta) / max(elapsed, refreshInterval / 2)
                travel.velocity = elapsed > 0.15 || speed * travel.velocity <= 0
                    ? speed : travel.velocity * 0.5 + speed * 0.5
                travel.top = at; travel.time = now
            }
            listTravel[id] = travel
        }
    }
    /// A collection's clip view moved (`NodeView.clipScrolled`): its travel
    /// first, so the rescue or the slice that follows leads the right way.
    func collectionScrolled(_ id: UInt32) {
        // A correction's move is not the reader's travel (LLP 1070.000 §2.5).
        if collections.owns(id), !collections.correcting { sampleListTravel(only: id) }
        collections.changed(id, user: true)
    }

    /// Build each owed collection's rows for this slice: as many as its
    /// measured per-row cost fits, at least one, and at least what the next
    /// two frames of travel uncover, so the scroll callback that follows
    /// finds them built (LLP 1050.000 stage 1; the iOS `ScrollPump`'s rule).
    private func fillCollections(deadline: TimeInterval) {
        for id in collections.fillPending.sorted() where !collections.ancestorMoving(id) {
            let started = CACurrentMediaTime()
            let fits = (listFillCosts[id] ?? ListFillCost()).rows(within: deadline - started)
            let needed = collections.rowsToCover(id, ahead: CGFloat(listVelocity(id) * refreshInterval * 2))
            let created = collections.fillSlice(id, limit: max(1, fits, needed))
            listFillCosts[id, default: ListFillCost()].record(seconds: CACurrentMediaTime() - started, rows: created)
        }
    }

    /// A scroll container moved. Nothing here may take long: AppKit is inside
    /// its scroll synchronizer, and the scrolling thread is waiting on it.
    func scrolled() {
        AnimatedRasters.shared.poke()
        guard !applying, !inScrollCallback else { return }
        menus.layout()
        inScrollCallback = true
        defer { inScrollCallback = false }
        let post = Self.signposts.beginInterval("scrolled")
        defer { Self.signposts.endInterval("scrolled", post) }
        // Most ticks move inside the band the mounted rows already cover: then
        // there is nothing to report, and nothing here reads or writes the
        // scroll view again until AppKit next calls in.
        sampleListTravel()
        leaves.scrolled()
        // Only what is already on screen without paint; the rest is pumped.
        textPending = refreshVisibleText(limit: 0) || textPending
        if textPending {
            startPump()
            queuePostSyncSlice()
        }
    }

    private func queuePostSyncSlice() {
        guard !ExactEnv.agentMode,
              let token = pumpSchedule.queuePostSync(at: CACurrentMediaTime()) else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self,
                  self.pumpSchedule.takePostSync(token, at: CACurrentMediaTime()) else { return }
            self.pump()
        }
    }

    /// After a batch: paint what is visible now, admit the rest over frames.
    private func batchApplied() {
        // Mounting and layout already spent this frame's main-thread time.
        // Keep visible pixels urgent; prepare offscreen text in a later slice.
        if refreshVisibleText(limit: 0) { textPending = true; startPump() }
    }

    /// An offscreen worker supplied pixels; publish them with the next text slice.
    func requestTextPublication() {
        textPending = true
        startPump()
    }

    private func startPump() {
        // A view-bound link follows display moves. Before its first tick on a
        // new screen or after idle, seed the budget from that screen instead
        // of spending a stale 60 Hz allowance on a 120 Hz display. Ticks supply the
        // actual interval (including variable refresh), even when skipped.
        if let screen = viewport.window?.screen,
           let id = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32,
           pumpLink == nil || pumpScreen != id {
            pumpScreen = id
            pumpSchedule.updateInterval(1 / Double(max(1, screen.maximumFramesPerSecond)))
        }
        guard pumpLink == nil else { return }
        // While scrolling, the queued slice runs just after AppKit releases
        // its scroll synchronizer (LLP 1044.000 S3). Otherwise this link owns
        // idle fill and text publication, including the last pending slice
        // after a gesture ends. A timer would drift through the commit phase.
        pumpTarget.fire = { [weak self] interval in
            self?.displayPump(interval)
        }
        let link = viewport.displayLink(target: pumpTarget, selector: #selector(PumpTarget.tick(_:)))
        link.add(to: .main, forMode: .common)
        pumpLink = link
    }

    func displayPump(_ interval: TimeInterval) {
        guard pumpSchedule.takeDisplayLink(interval: interval, at: CACurrentMediaTime()) else { return }
        pump()
    }

    private func stopPump() {
        pumpSchedule.cancel()
        pumpLink?.invalidate()
        pumpLink = nil
    }

    /// Everything the pump owes, now. The agent's wheel is synchronous — it
    /// reads the tree right after — and so is anything that must not observe
    /// a half-filled window (LLP 1012: an agent never waits).
    func settlePump() {
        pumpSchedule.cancel()
        // The agent reads the tree right after: every held leaf is made (LLP 1068 §5.1).
        leaves.settle()
        // Match the previous bounded native-feedback depth while keeping
        // background admission out of this synchronous agent boundary.
        for _ in 0..<8 {
            collections.settle()
            guard !collections.fillPending.isEmpty else { break }
            for id in collections.fillPending.sorted() { collections.fillSlice(id, limit: UInt32.max - 1) }
        }
        collections.settle()
        refreshVisibleText()
        textPending = false
        if !collections.fillPending.isEmpty { startPump() } else { stopPump() }
    }

    /// The refresh interval sets the deadline. A report sizes its overscan from
    /// measured row cost, reserving the shared finalization pass. A fill can
    /// briefly grow the lead while still losing ground over two frames: keep
    /// filling during travel, and admit text after it instead of alternating.
    func pump() {
        pumping = true
        defer { pumping = false }
        if !collections.fillPending.isEmpty {
            let post = Self.signposts.beginInterval("pump-collection")
            fillCollections(deadline: CACurrentMediaTime() + sliceBudget)
            Self.signposts.endInterval("pump-collection", post)
        }
        if textPending {
            let post = Self.signposts.beginInterval("pump-text")
            textPending = refreshVisibleText(limit: Self.textBandsPerSlice, afterFrame: true)
            Self.signposts.endInterval("pump-text", post)
        }
        if !textPending && collections.fillPending.isEmpty { stopPump() }
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
        mouseLayoutPan.abandon()
        mouseHeightDrag.cancel()
        mouseTransformDrag.cancel()
        mouseReorder.cancel()
        reorder?.abandon()
        collections.reset()
        leaves.reset()
        autofocusProcessed.removeAll()
        resetting = true
        defer { resetting = false }
        menus.reset()
        dialogs.reset()
        toolbar.reset()
        navigation.reset()
        segments.reset()
        controls.reset()
        edited = nil
        session?.canvases.reset()
        for id in Array(leaving.keys) { _ = endExit(id) }
        views.values.forEach { $0.forget() }
        root.subviews.forEach { $0.removeFromSuperview() }
        views.removeAll()
        inlineOwners.removeAll()
        hoveredInline = nil
        chrome = ChromeIndex()
        scrollers.removeAll()
        pendingScrolls.removeAll()
        heightBindings.removeAll()
        transformBindings.removeAll()
        transformGeometry.reset()
        videoVisibility?.reset()
        selection.structureChanged()
        visibleText.removeAll()
        textViewportIndex = nil
        stopPump()
        listTravel.removeAll()
        listFillCosts.removeAll()
        textPending = false
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
    /// Images' intrinsic sizes, several at once under one layout.
    var onIntrinsic: (([(UInt32, CGSize?)]) -> Void)?
    /// A capability an action called (LLP 1005 §3), after its commit.
    var onCommand: ((String, [Any], UInt32?) -> Void)?

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
        if target.kind == "native", !selectText { _ = session?.natives.focus(target); return }
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

    /// The action's blur(): drop the first responder; blur(html-id) only when
    /// that node holds it (a field's responder is its editor).
    func blurElement(_ args: [Any]) {
        guard let window = viewport.window else { return }
        if let name = args.first as? String {
            guard let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else { return }
            if target.kind == "native" {
                guard window.firstResponder === target || session?.natives.ownsFocus(target) == true else { return }
                window.makeFirstResponder(nil)
                return
            }
            let responder: NSView = target.textArea ?? target.field ?? target
            guard window.firstResponder === responder || window.firstResponder === target.field?.currentEditor() else { return }
        }
        window.makeFirstResponder(nil)
    }

    /// The events beyond press and change (LLP 1005 §3).
    var onHover: ((UInt32, Bool) -> Void)?
    var onFocus: ((UInt32) -> Void)?
    var onBlur: ((UInt32) -> Void)?
    var onKey: ((UInt32, String) -> Void)?
    var onContextmenu: ((UInt32) -> Void)?
    var onDblclick: ((UInt32) -> Void)?
    var onSwiperight: ((UInt32) -> Void)?
    /// Pull-to-refresh is UIKit's; AppKit has no such control, so this never fires.
    var onRefresh: ((UInt32) -> Void)?
    var onPan: ((UInt32, Double, Double) -> Void)?
    /// A pan that began ended (LLP 1057 §10.6); the tracker below measures
    /// where the platform gives no velocity (LLP 1057.001 §3).
    var onPanRelease: ((UInt32, Double, Double) -> Void)?
    var onPanSample: ((Bool, Double, Double, Double) -> Void)?
    var panVelocity: ((Double) -> (Double, Double))?
    var onScroll: ((UInt32, Double, Double) -> Void)?
    var onListIndex: ((UInt32, String) -> Int?)?
    var onListText: ((UInt32, (String, Int, Int)?, (String, Int, Int)?) -> String)?
    var interacting: UInt32 = 0
    private var listViews: [UInt32: NodeView] = [:]

    /// Everything kept for `id`, out of the maps (not out of the window);
    /// `forget` releases its resources too, as a destroy does and a leaving
    /// view (LLP 1063) does not until its exit ends.
    @discardableResult
    func release(_ id: UInt32, forget: Bool) -> NodeView? {
        mouseSwipe.retire(id)
        mouseLayoutPan.retire(id)
        mouseHeightDrag.retire(id)
        mouseTransformDrag.retire(id)
        mouseReorder.retire(id)
        session?.canvases.destroy(view: id)
        svg.forget(id)
        canvas2d.forget(id)
        if forget { views[id]?.forget() }
        // Out of the map before out of the window: the editing-ended
        // notification removal fires finds no view to send for.
        heightBindings.removeValue(forKey: id)
        transformBindings.removeValue(forKey: id)
        transformGeometry.retire(id)
        session?.text.readerParagraphs.removeValue(forKey: id)
        let gone = views.removeValue(forKey: id)
        chrome.forget(id)
        scrollers.remove(id)
        pendingScrolls.remove(id)
        listViews.removeValue(forKey: id)
        listTravel.removeValue(forKey: id)
        listFillCosts.removeValue(forKey: id)
        return gone
    }
    var onSubmit: ((UInt32) -> Void)?
    var onLoad: ((UInt32) -> Void)?
    var onMessage: ((UInt32, String) -> Void)?
    /// The node the pointer is over, of those with a hover handler: it hears
    /// the leave when the pointer moves onto another (the agent's `hover`).
    weak var hovered: NodeView?
    var hoveredInline: UInt32?

    func press(_ id: UInt32, fromNativeMenu: Bool = false) {
        guard let node = textHost(id), !node.inert, !node.disabled,
              fromNativeMenu || !node.isHiddenOrHasHiddenAncestor || toolbar.contains(node) else { return }
        let command = dialogs.command(node, fromNativeMenu: fromNativeMenu)
        let popover = menus.command(node, fromNativeMenu: fromNativeMenu)
        if (command == nil && popover == nil) || node.handlers.contains("press") { onPress?(id) }
        command?()
        popover?()
    }
    func change(_ id: UInt32, _ value: String) { onChange?(id, value) }
    /// A text field typed into since it took the focus: its `change` fires
    /// when the editing ends or Enter commits it, HTML's `change` (LLP
    /// 1069.001 D4); every keystroke is its `input`.
    var edited: UInt32?
    var onInput: ((UInt32, String) -> Void)?
    /// A checkbox's new state, reported as HTML's `input` then `change`.
    var onChecked: ((UInt32, Bool) -> Void)?
    func typed(_ id: UInt32, _ value: String, input: Bool) {
        edited = id
        if input { onInput?(id, value) }
    }
    func commitEdit(_ id: UInt32, _ value: String, change: Bool) {
        guard edited == id else { return }
        edited = nil
        if change { onChange?(id, value) }
    }
    func checked(_ id: UInt32, _ on: Bool) { onChecked?(id, on) }
    /// A select's, range's or date's new value (LLP 1069.001 D4): HTML's
    /// `input` as it moves, `change` as it is committed.
    var onControlValue: ((UInt32, String, Bool, Bool) -> Void)?
    func controlValue(_ id: UInt32, _ value: String, input: Bool, change: Bool) { onControlValue?(id, value, input, change) }
    /// A select's options and the one it shows, read from the kernel.
    var selectOptions: ((UInt32) -> SelectMenu)?
    var buttonFace: ((UInt32) -> ButtonFace)?

    /// An event a view reports: sent only while the presenter still has the
    /// view (the platform fires editing-ended as a destroyed field leaves the
    /// window; the browser fires no blur on removal, so neither does this
    /// host), and never while a batch is being applied — it waits for the
    /// batch to finish, then goes if its view survived it.
    private var applying = false
    var isApplying: Bool { applying }
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
        guard !resetting, textHost(id) != nil else { return }
        if applying { waiting.append((id, f)) } else { f() }
    }
    func hover(_ view: NodeView, _ over: Bool) {
        if over { hoverInline(nil) }
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
    func pan(_ id: UInt32, _ dx: Double, _ dy: Double) { send(id) { [self] in onPan?(id, dx, dy) } }
    /// Once per pan that began, after its last delta; only to a node that hears it.
    func panRelease(_ id: UInt32, _ vx: Double, _ vy: Double) {
        guard views[id]?.handlers.contains("panrelease") == true else { return }
        send(id) { [self] in onPanRelease?(id, vx, vy) }
    }
    func scroll(_ id: UInt32, _ left: Double, _ top: Double) { send(id) { [self] in onScroll?(id, left, top) } }
    func submit(_ id: UInt32) { send(id) { [self] in onSubmit?(id) } }
    func load(_ id: UInt32) { send(id) { [self] in onLoad?(id) } }
    func message(_ id: UInt32, _ value: String) {
        guard let view = views[id], view.handlers.contains("message") else { return }
        send(id) { [weak self, weak view] in
            guard let self, let view, views[id] === view, view.handlers.contains("message") else { return }
            onMessage?(id, value)
        }
    }
    func intrinsic(_ id: UInt32, _ size: CGSize?) { onIntrinsic?([(id, size)]) }

    func apply(_ batch: Batch) {
        defer { applyLanguage(batch) }
        let post = Self.signposts.beginInterval("apply", "\(batch.ops.count) ops")
        defer { Self.signposts.endInterval("apply", post) }
        viewport.invalidateDocumentFit()
        collections.beginBatch(batch)
        toolbar.prepare()
        for id in scrollers where !collections.owns(id) { views[id]?.captureScrollPosition() }
        if let e = batch.error { FileHandle.standardError.write(Data("exact: \(e)\n".utf8)) }
        if let text = session?.text {
            // SVG text shapes with the session's fonts (LLP 1055.000 D11).
            svg.fonts = { [weak text] size, weight, family, italic in
                (text?.font(size: size, weight: weight, family: family, italic: italic)).map { $0 as CTFont } ?? SvgScene.systemFonts(size, weight, family, italic)
            }
        }
        svg.seek(clock: session?.clock)
        let outermost = !applying
        applying = true
        // Create, frame or content ops: rows may have come or moved (`HeavyLeaves.batchApplied`).
        let moved = batch.ops.contains { [.create, .frame, .content].contains($0.op) }
        defer {
            collections.endBatch()
            collections.observeKnobDrags()
            collections.limitPrepared()
            if outermost {
                applying = false
                videoVisibility?.changed()
                let geometry = pendingGeometry
                pendingGeometry = nil
                let q = waiting
                waiting = []
                geometry?()
                for (id, f) in q where textHost(id) != nil { f() }
                batchApplied()
                if !boxFilters.isEmpty { boxFilters.render() }
                leaves.batchApplied(moved: moved)
            }
        }
        if !batch.ops.isEmpty { textViewportIndex = nil }
        var reparented = Set<UInt32>()
        let structureChanged = batch.ops.contains { [.children, .roots, .destroy, .create, .style].contains($0.op) }
        if structureChanged { selection.structureChanged() }
        for op in batch.ops {
            let kind = op.op
            let id = op.id
            if kind == .children { touched(id, children: true) } else if kind != .roots && kind != .create { touched(id, textChanged: kind == .props || kind == .style || kind == .destroy) }
            switch kind {
            case .transformDrag:
                if let binding = TransformDragBinding(op.payload) {
                    if binding.target == nil {
                        if transformBindings[binding.id]?.handleKey == binding.handleKey
                            && transformBindings[binding.id]?.runtime == binding.runtime {
                            transformBindings.removeValue(forKey: binding.id)
                            transformGeometry.retire(binding.id)
                        }
                    } else { transformBindings[binding.id] = binding }
                }
            case .retireMotion:
                if let rawRuntime = op.payload["runtime"] as? String, let runtime = UInt64(rawRuntime),
                   let rawToken = op.payload["token"] as? String, let token = UInt64(rawToken) {
                    session?.transformInputHold?.retire(runtime: runtime, token: token)
                }
            case .heightDrag:
                if let binding = HeightDragBinding(op.payload) {
                    if binding.target == nil {
                        if heightBindings[binding.id]?.handleKey == binding.handleKey {
                            heightBindings.removeValue(forKey: binding.id)
                        }
                    } else { heightBindings[binding.id] = binding }
                }
            case .create:
                let v = NodeView(id: id, kind: op.kind, presenter: self)
                v.handlers = op.handlers
                v.applyStyle(op.style)
                v.applyProps(set: op.props, clear: [])
                views[id] = v
                if v.kind == "list" { listViews[id] = v }
                if v.kind == "video" { leaves.created(v) }
            case .paragraph:
                applyParagraph(id, op.runs)
            case .props:
                views[id]?.applyProps(set: op.props, clear: op.clear)
            case .flow:
                views[id]?.applyFlow(op.payload["shapes"] as? [[String: Any]] ?? [])
            case .style:
                guard let v = views[id] ?? leaving[id]?.view else { continue }
                let color = v.style["text_color"]
                v.applyStyle(op.style)
                if v.surface != nil { v.applySurface() }
                // Paint motion re-sends a style per frame (LLP 1055.000 D6);
                // a view that paints in an appearance of its own says so
                // (LLP 1062 D4).
                if v.style["text_color"] != color { session?.noteAppearance(v) }
            case .children:
                guard let parent = views[id] else { continue }
                let want = op.ids.compactMap { views[UInt32($0)] }
                let container = parent.container
                dialogs.children(container, want)
                menus.children(container, want)
                let wanted = Set(want.map { ObjectIdentifier($0) })
                for child in container.subviews where child is NodeView && !wanted.contains(ObjectIdentifier(child)) && !isLeaving(child) {
                    if let node = child as? NodeView { reparented.insert(node.id) }
                    child.removeFromSuperview()
                }
                let mounted = want.filter { !dialogs.owns($0) && !menus.owns($0) }
                for (i, child) in mounted.enumerated() {
                    if child.superview !== container {
                        reparented.insert(child.id)
                        child.prepareToMount()
                        // Appending then moving the first child above nil puts
                        // it last and needlessly remounts every retained sibling.
                        if i == 0, let first = container.subviews.first,
                           container.subviews.allSatisfy({ $0 is NodeView }) {
                            container.addSubview(child, positioned: .below, relativeTo: first)
                        } else { container.addSubview(child) }
                    }
                    let siblings = container.subviews
                    if !collections.owns(id), i >= siblings.count || siblings[i] !== child {
                        child.removeFromSuperview()
                        container.addSubview(child, positioned: .above, relativeTo: i > 0 ? mounted[i - 1] : nil)
                    }
                }
                if collections.owns(id) { collections.orderChildren(want, in: container) }
            case .surface:
                if let v = views[id] { session?.canvases.surface(view: v, name: op.payload["name"] as? String ?? "", values: op.payload["values"] ?? []) }
            case .canvas2d: if let v = views[id] { canvas2d.apply(id, op.payload, layer: v.layer) }
            case .svg: if let v = views[id] { svg.scene(id, op.payload, layer: v.layer, dark: v.drawsDark, clock: session?.clock) }
            case .animations: svg.animations(id, op.payload, layer: views[id]?.layer, clock: session?.clock)
            case .command:
                onCommand?(op.payload["name"] as? String ?? "", op.payload["args"] as? [Any] ?? [], (op.payload["source"] as? NSNumber)?.uint32Value)
            case .exit: beginExit(id)
            case .destroy:
                if endExit(id) { continue }
                release(id, forget: true)?.removeFromSuperview()
            case .roots:
                dialogs.children(root, op.ids.compactMap { views[UInt32($0)] })
                menus.children(root, op.ids.compactMap { views[UInt32($0)] })
                root.subviews.forEach { $0.removeFromSuperview() }
                for r in op.ids.compactMap({ views[UInt32($0)] }) {
                    if dialogs.owns(r) || menus.owns(r) { continue }
                    r.prepareToMount()
                    root.addSubview(r)
                }
            case .frame:
                guard let v = views[id] else { continue }
                let frame = NSRect(x: op.x, y: op.y, width: op.w, height: op.h)
                if !dialogs.frame(v, frame) && !menus.frame(v, frame) { v.frame = frame }
                v.arrangeShift = .zero
                v.textRasterGeometryChanged()
                v.scroll?.frame = v.bounds
                v.field?.frame = v.contentBox()
                v.layoutTextArea()
                v.metal?.frame = v.bounds
                v.overlay?.frame = v.bounds
                v.web?.frame = v.bounds
                v.applyShadow()
                v.fitScroll()
                v.applyTransform()
            case .content:
                if let v = views[id] {
                    v.content = CGSize(width: op.w, height: op.h)
                    v.fitScroll()
                }
            case .present:
                guard let v = views[id] ?? leaving[id]?.view else { continue }
                let x = CGFloat(op.x)
                switch op.property {
                case "translate": v.translate = CGPoint(x: x, y: CGFloat(op.y)); v.applyTransform()
                case "layout": v.layoutOffset = CGPoint(x: x, y: CGFloat(op.y)); v.layoutScale = CGPoint(x: CGFloat(op.w), y: CGFloat(op.h)); v.applyTransform(); v.applySurface()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.alphaValue = x
                default: break
                }
            default: break
            }
        }
        navigation.sync(batch, reparented: reparented)
        fitDocument()
        // The page's canvas colour is the first root's background — what
        // shows beyond a document shorter than the viewport, as a browser
        // paints the root element's background over the whole canvas.
        let color = (root.subviews.first as? NodeView)?.color("background_color", .white) ?? .white
        if viewport.backgroundColor != color { viewport.backgroundColor = color }
        let first = root.subviews.first as? NodeView
        let fit = first?.props["viewportFit"]
        if fit != viewportFit { viewportFit = fit; onViewportFit?() }
        session?.canvases.cancelMovedControls()
        session?.canvases.captureIfNeeded()
        for id in scrollers.union(pendingScrolls) {
            guard let node = views[id] else { continue }
            if !collections.owns(node.id) { node.restoreScrollPosition() }
            if node.pendingScrollTop != nil || node.pendingScrollLeft != nil { collections.userIntent(node.id) }
            node.applyPendingScroll()
        }
        pendingScrolls.removeAll()
        segments.sync()
        controls.sync()
        dialogs.sync()
        menus.sync()
        glassGroups.reconcile()
        positionContexts()
        toolbar.sync()
        shortcuts.sync()
        if structureChanged { selection.structureChanged() }
        // The key-view loop is read only by a key event (Tab): it is marked
        // stale here and rebuilt when one arrives (`flushKeyViewLoop`), not
        // walked over every mounted node on every batch of a scroll.
        if structureChanged || batch.ops.contains(where: { $0.op == .props }) {
            keyViewLoopStale = true
        }
        refreshVisibleText()
        // The nodes this batch touched and the views above them, as iOS
        // passes them: a full pass sorted every view in the session on
        // every batch, a list's scroll included.
        syncAccessibility(changed: touchedAndAbove(batch.ops.lazy.filter { Self.accessibilityOps.contains($0.op) }.map(\.id)))
        _ = chrome.takeChangedNames()
    }

    private static let accessibilityOps: Set<BatchOp.Kind> = [.create, .props, .style, .children, .paragraph, .flow, .frame]

    /// The views a batch touched and every view above them, as the batch
    /// left the hierarchy: what a pass reading a subtree must revisit.
    private func touchedAndAbove<S: Sequence>(_ ids: S) -> Set<UInt32> where S.Element == UInt32 {
        var seen = Set<UInt32>()
        for id in ids {
            var view: NSView? = views[id]
            while let current = view {
                if let node = current as? NodeView, !seen.insert(node.id).inserted { break }
                view = current.superview
            }
        }
        return seen
    }

    /// Align an enclosing context panel's preview with its source, while
    /// keeping the panel inside the visible viewport.
    private func positionContexts() {
        for preview in carrying("contextTarget") {
            guard let target = preview.props["contextTarget"],
                  let source = views.values.first(where: { $0.props["id"] == target }),
                  source.window != nil else { continue }
            var ancestor = preview.superview as? NodeView
            while let node = ancestor, node.style["position_type"]?.string != "absolute" {
                ancestor = node.superview as? NodeView
            }
            guard let panel = ancestor, let parent = panel.superview else { continue }
            let sourceBox = source.convert(source.bounds, to: parent)
            let port = viewport.convert(viewport.bounds, to: parent)
            // Static wrappers do not contain the panel. Clamp against its
            // nearest positioned ancestor, expressed in the parent coordinates.
            var containing = parent
            while containing !== root && containing.superview !== root {
                if let node = containing as? NodeView,
                   node.style["position_type"]?.string != nil,
                   node.style["position_type"]?.string != "static" { break }
                guard let ancestor = containing.superview else { break }
                containing = ancestor
            }
            let region = containing.convert(containing.bounds, to: parent)
            let minimum = max(region.minY, port.minY + 8)
            let maximum = min(region.maxY, port.maxY - 8) - panel.bounds.height
            let top = max(minimum, min(sourceBox.minY - preview.convert(preview.bounds, to: panel).minY, maximum))
            if panel.frame.origin.y != top { panel.setFrameOrigin(CGPoint(x: panel.frame.minX, y: top)) }
        }
    }

    /// The same editing descendant takes explicit, sequential and modal focus.
    func keyView(of v: NodeView) -> NSView {
        if v.kind == "native", let target = session?.natives.focusTarget(v) { return target }
        return v.textArea ?? v.field ?? v
    }

    /// Sequential focus after a batch: tree order, then `tabIndex` > 0, as
    /// HTML. `autorecalculatesKeyViewLoop` stays false so nothing is focused
    /// at launch (LLP 1014); Tab from the viewport still reaches the first
    /// tabbable. Hidden popover rows stay out (their container is hidden).
    /// Whether a batch changed what the key-view loop is built from.
    var keyViewLoopStale = false

    /// Rebuild the key-view loop if a batch left it stale: called for each
    /// key event before AppKit reads `nextKeyView` (the view's monitor, and
    /// the agent's `type`, which sends to the window directly).
    func flushKeyViewLoop() {
        guard keyViewLoopStale else { return }
        keyViewLoopStale = false
        syncKeyViewLoop()
    }

    func syncKeyViewLoop() {
        var listed: [NodeView] = []
        // A paragraph selected by a click is where Tab starts from, as on the
        // web: it points on to the next stop after it, and no stop to it.
        var starts: [(NodeView, Int)] = []
        func walk(_ v: NodeView) {
            if v.inert || v.isHidden { return }
            if Self.tabbable(v) { listed.append(v) } else if v.isParagraph { starts.append((v, listed.count)) }
            for child in v.container.subviews.compactMap({ $0 as? NodeView }) { walk(child) }
            for popover in menus.following(v) { walk(popover) }
        }
        if let dialog = dialogs.active { walk(dialog) }
        else { for r in root.subviews.compactMap({ $0 as? NodeView }) { walk(r) } }
        let tabbable = listed.enumerated().sorted { a, b in
            let ia = Self.tabIndex(a.element), ib = Self.tabIndex(b.element)
            let pa = ia > 0 ? ia : Int.max, pb = ib > 0 ? ib : Int.max
            if pa != pb { return pa < pb }
            return a.offset < b.offset
        }.map(\.element)
        // First, so the loop's own links below decide each stop's previous.
        for (paragraph, next) in starts {
            paragraph.nextKeyView = tabbable.isEmpty ? nil : keyView(of: next < listed.count ? listed[next] : tabbable[0])
        }
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

    static func tabbable(_ v: NodeView) -> Bool {
        if v.props["disabled"] == "true" { return false }
        let index = tabIndex(v)
        if index < 0 { return false }
        if v.field != nil || v.textArea != nil { return true }
        if v.kind == "native", v.presenter?.session?.natives.focusTarget(v) != nil { return true }
        if v.kind == "button" || v.kind == "toggle" || v.handlers.contains("press") { return true }
        if v.canBecomeKeyView { return true }
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
            paragraph = node.superview as? NodeView
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
                if let n = s as? NodeView, n.placement != nil, !n.isHidden { n.isHidden = true; hidden.append(n); continue }
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

    /// Paragraphs within `reach`, nearest each scroll document's visible rect
    /// first. A tall distant paragraph must not delay a short imminent one.
    func candidates(reach: CGFloat = 0) -> [NodeView] {
        var result: [(CGFloat, NodeView)] = []
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
                if entry.rect.intersects(visible) {
                    let dx = max(0, shown.minX - entry.rect.maxX, entry.rect.minX - shown.maxX)
                    let dy = max(0, shown.minY - entry.rect.maxY, entry.rect.minY - shown.maxY)
                    result.append((max(dx, dy), entry.node))
                }
                index += 1
            }
        }
        return result.sorted {
            $0.0 == $1.0 ? $0.1.id < $1.1.id : $0.0 < $1.0
        }.map { $0.1 }
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
    var fire: ((TimeInterval) -> Void)?
    @objc func tick(_ link: CADisplayLink) { fire?(link.targetTimestamp - link.timestamp) }
}
#endif

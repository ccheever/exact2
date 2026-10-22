// The AppKit presenter (LLP 1008 §5): the page's scroll view over a flipped
// document, one `NodeView` per kernel node, the host's batches applied. It
// belongs to one session (LLP 1031 D1) and reaches the session's canvases,
// web views, and menus through it.
#if os(macOS)
import AppKit
import CoreText
import os

/// Which live views carry the few props the chrome passes look for.

package final class Presenter {
    var documentLanguage = ""
    var documentDirection = "ltr"
    /// Intervals a trace can lay beside its frames (Instruments' os_signpost):
    /// what the main thread spent on a list window, a batch, a text slice.
    static let signposts = OSSignposter(subsystem: "com.exact.host", category: "scroll")
    var autofocusProcessed: Set<ObjectIdentifier> = []
    /// Set the turn after the session's first activation. A booted session's autofocus waits for it.
    var launchAutofocusReleased = false
    /// What VoiceOver was told of the modal and the menus, and the menus a row of which held the focus (`tellLanding`).
    var toldModal: ObjectIdentifier?, shownMenus: [NodeView] = [], landing = VoiceOverLanding()
    let menusHeldFocus = NSHashTable<NodeView>.weakObjects()
    /// The session this presenter shows (LLP 1031 D1).
    weak package var session: ExactSession?
    /// The document: the roots live here, content-sized like a page.
    let root = FlippedView(frame: .zero)
    /// The viewport over it: the window's content view, scrolling like a browser's.
    package let viewport = PageScrollView(frame: .zero)
    package var views: [UInt32: NodeView] = [:]
    /// The page viewport's programmatic element reveal, while AppKit owns its interpolation.
    var pageElementAnimation: UUID?
    lazy var focusEvents = FocusEvents(self)
    var inlineOwners: [UInt32: (owner: UInt32, index: Int)] = [:]
    private(set) var chrome = ChromeIndex()
    /// Views leaving with their exit, by id (LLP 1063, `PresenceMac.swift`).
    var leaving: [UInt32: Leaving] = [:]
    /// Shared elements in flight, by the arriver's id (LLP 1013.000, `FlightsMac.swift`).
    var flights: [UInt32: Flight] = [:]
    /// A view's props were written (`NodeView.props`' own observer).
    func propsChanged(_ view: NodeView) {
        chrome.note(view.id, props: view.props)
        if view.disabled { view.showFieldFocus(false) }
        view.syncDisabledLook()
        view.relook() // a look holds on no disabled node (LLP 1121 D8)
        // HTML's `title`: the platform's tooltip (studio diary R24).
        // An icon-only button's help tag is its label (LLP 1115 wave 1), as
        // a toolbar item's is; settled once the batch has mounted its face.
        if view.isButton && !view.isNativeButton && view.props["title"] == nil && view.authoredLabel != nil { pendingTips.insert(view.id) }
        else if view.toolTip != view.props["title"] { view.toolTip = view.props["title"] }
    }
    /// Custom buttons with a label and no `title`, whose help tag waits for
    /// the batch to mount their content.
    var pendingTips: Set<UInt32> = []
    func settleTips() {
        for id in pendingTips {
            guard let view = views[id] else { continue }
            let tip = view.props["title"] ?? (view.accessibleText.isEmpty ? view.authoredLabel : nil)
            if view.toolTip != tip { view.toolTip = tip }
        }
        pendingTips.removeAll()
    }
    /// Views carrying an indexed prop, in id order (the passes' old order was
    /// a dictionary's, which is none).
    package func carrying(_ key: String) -> [NodeView] {
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
    /// The batch's suppression triggers for scroll anchoring (`ScrollAnchoring.swift`).
    var anchorChanges = ScrollAnchoring.Changes()
    package var heightBindings: [UInt32: HeightDragBinding] = [:]
    package var transformBindings: [UInt32: TransformDragBinding] = [:]
    lazy package var transformGeometry = TransformGeometryHost(self)
    var videoVisibility: VideoVisibilityHost?
    lazy package var collections = CollectionHost(self)
    lazy var stickies = StickyHost(self)
    /// Heavy leaves held while their rows are far or flying (LLP 1068 §5.1).
    lazy var leaves = HeavyLeaves(self)
    lazy package var selection = TextSelection(self)
    let textRasters = TextRasterizer()
    lazy var mouseSwipe = MouseSwipe(self)
    lazy var mouseLayoutPan = MouseLayoutPan(self)
    /// The mouse drags (LLP 1047.001 D4: the Drag module's, when linked).
    lazy var mouseDrags = DragLink.installed?.mouseDrags(self) ?? .none
    lazy var mouseChain = MouseChain(self)
    /// The one Arrange contact, until its source settles; a test's calls.
    package var reorder: ReorderLift?
    package var reorderCalls: ReorderCalls?
    /// A grouped session (LLP 1094), until its ghost lands; a test's calls.
    package var reorderGroup: ReorderGroupHold?
    var reorderGroupCalls: ReorderGroupCalls?
    private var scrollObserver: NSObjectProtocol?
    private var visibleText: [UInt32: NSRect] = [:]
    private var textViewportIndex: TextViewportIndex?
    /// The native menu arm (LLP 1021 D3).
    lazy var menus = MenuHost(presenter: self)
    lazy var dialogs = DialogHost(self)
    /// Subtrees shown in a native container's view (LLP 1115.002 D8.2).
    lazy var blocks = BlockHost(self)
    /// Lists whose rows a table holds (LLP 1115.002 D5, stage 3a).
    lazy var tables = TableHost(self)
    lazy var alerts = AlertHost(self)
    lazy var navigation = NavigationHost(presenter: self)
    lazy var backSwipe = BackSwipe(self)
    /// SVG scenes and CSS animations (LLP 1055 D4, D7).
    let svg = SvgHost()
    /// Boxes under CSS `filter`, drawn again after each batch (LLP 1055.000 D14).
    let boxFilters = BoxFilters()
    let canvas2d: Canvas2DCanvases = SurfacesLink.installed?.canvas2D() ?? NoCanvas2D()
    lazy var segments = SegmentHost(self)
    lazy var controls = ControlHost(self)
    lazy var fieldSelections = FieldSelections(self)
    /// The one editor every `input`'s field of this session edits in (`FieldEditor`).
    lazy var fieldEditor = FieldEditor.make()
    /// Nodes marked `hatch="word"` (LLP 1075.003.000).
    lazy var elements = ElementHatches(self)
    lazy var shortcuts = ShortcutHost(presenter: self)
    lazy var toolbar = WindowToolbarHost(self)
    /// What native presentations cover of boxes (LLP 1115.002 D8.1, CoversMac.swift).
    lazy var covers = HostCovers(self)
    var onCovers: (([(UInt32, HostCover?)]) -> Void)?
    /// The root tab bar as the window's sidebar (LLP 1115.002 D3), and the kernel's rule for a root's tabs.
    lazy var sidebar = SidebarHost(self)
    var rootTabs: ((UInt32) -> RootTabs?)?
    /// The head's title goes to the window the app attached, through the
    /// toolbar host that already owns its title (LLP 1048.003 D1).
    func headTitle(_ title: String?) { toolbar.headTitle(title) }
    /// `head edited` (LLP 1069.010 D6): the window's edited mark.
    func headEdited(_ edited: Bool) { toolbar.headEdited(edited) }
    /// The first root's `viewportFit` prop (`"cover"` or nothing), as of the
    /// last batch; `onViewportFit` fires when it changes. macOS maps `cover`
    /// to a full-size-content window (the titlebar overlays the viewport;
    /// its height is `safe-area-inset-top`). @ref LLP 1008 §9
    private(set) var viewportFit: String?
    var onViewportFit: (() -> Void)?
    /// The safe-area insets the kernel was given (the titlebar under `viewport-fit=cover`, zero
    /// when the viewport is the content view below it) and its layout regions (LLP 1123 D10,
    /// `nil` for none). Reported to the agent as `env`.
    var insets = NSEdgeInsetsZero, regions: [CGFloat]?
    /// The title-bar area the kernel was given (`TitleBarAreaMac`): x, y, width, height, `nil` for none.
    var titlebarArea: [CGFloat]?
    /// The posture and the viewport segments last told (LLP 1078 D5): flat, unless an agent preferred otherwise.
    var fold = ViewportFold.flat

    init() {
        viewport.permitsDocumentPrefit = { [weak self] in
            guard let self else { return false }
            return !self.applying && !self.resetting
        }
        viewport.pressedGround = { [weak self] in self?.selection.clear() }
        viewport.pointerTracking = { [weak self] event in self?.trackPointer(event) }
        viewport.lostKey = { [weak self] in self?.windowLostKey() }
        viewport.modalAccessibilityChildren = { [weak self] in
            self?.macModalAccessibilityChildren
        }
        viewport.documentView = root
        viewport.hasVerticalScroller = true
        viewport.hasHorizontalScroller = true
        viewport.autohidesScrollers = true
        viewport.automaticallyAdjustsContentInsets = false
        viewport.contentInsets = NSEdgeInsetsZero
        viewport.drawsBackground = true
        // LLP 1115 D2: an unset page background is the platform's window
        // background, which follows Dark Mode, never a fixed white.
        viewport.backgroundColor = .windowBackgroundColor
        viewport.contentView.postsBoundsChangedNotifications = true
        // LLP 1050.000 stage 1: a collection reports its travel and builds
        // ahead in the pump's slices.
        collections.motion = { [unowned self] id in
            // A smooth correction's animation is not the reader's travel.
            if collections.animating.contains(id) { return nil }
            let velocity = listVelocity(id)
            return velocity == 0 ? nil : velocity
        }
        // A slice's own continuation waits for the next frame's link.
        collections.requestFill = { [unowned self] in
            startPump()
            if !pumping { queuePostSyncSlice() }
        }
        scrollObserver = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification,
            object: viewport.contentView, queue: .main) { [weak self] _ in
            if let self { let o = viewport.contentView.bounds.origin; onScrolled?(nil, Double(o.x), Double(o.y)) }
            self?.stickies.scrolled(nil); self?.scrolled(); self?.transformGeometry.changed(); self?.videoVisibility?.changed() }
    }

    deinit {
        if let scrollObserver { NotificationCenter.default.removeObserver(scrollObserver) }
        pumpLink?.invalidate()
        hoverLink?.invalidate()
        followLink?.invalidate()
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
        if textViewportIndex == nil { textViewportIndex = TextViewportIndex(selection.shownParagraphs) }
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
            // Animating to a smooth correction: no travel, and none read
            // across the animation once it lands.
            if collections.animating.contains(id) { listTravel[id] = nil; continue }
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
        followPointer()
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

    /// An offscreen worker supplied pixels, or a display pass found both
    /// workers busy: publish them, or ask again, with the next text slice.
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
        MainActor.assumeIsolated { HeldState.retire(self) } // before its views leave the map (HeldState.swift)
        WakeLock.hold(self, false) // the new host's first batch says it again
        cancelPageElementScroll(viewport.contentView)
        focusEvents.reset()
        pointerHeld = nil; pointerPath = []; pointerSource = nil; mouseChain.bubbledPress?.cancel()
        elements.reset()
        resetFlights()
        viewport.invalidateDocumentFit()
        session?.regions.reset()
        session?.rasters.reset()
        mouseSwipe.cancel()
        mouseLayoutPan.abandon()
        mouseDrags.height.cancel()
        mouseDrags.transform.cancel()
        mouseDrags.reorder.cancel()
        reorder?.abandon()
        reorderGroup?.abandon()
        collections.reset()
        tables.reset()
        leaves.reset()
        autofocusProcessed.removeAll()
        resetting = true
        defer { resetting = false }
        menus.reset()
        dialogs.reset()
        toolbar.reset() // its header's cover goes with `covers.reset()` below, untold
        navigation.reset()
        sidebar.reset()
        covers.reset()
        segments.reset()
        controls.reset()
        fieldSelections.reset()
        edited = nil
        session?.canvases.reset()
        for id in Array(leaving.keys) { _ = endExit(id) }
        views.values.forEach { $0.forget() }
        root.subviews.forEach { $0.removeFromSuperview() }
        views.removeAll()
        inlineOwners.removeAll()
        hoveredInline = nil
        hoveredNodes = []; hovered = nil
        chrome = ChromeIndex()
        scrollers.removeAll()
        pendingScrolls.removeAll()
        heightBindings.removeAll()
        transformBindings.removeAll()
        transformGeometry.reset()
        videoVisibility?.reset()
        selection.structureChanged(); selection.clear() // a selection of the retired tree, unreported
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
            // What overflows a root that does not clip is the document's (`content`).
            let v = r as? NodeView, extent = v.flatMap { $0.scroll == nil && !$0.overflowClips && !$0.isHidden ? $0.content : nil }
            // Through the root's layer transform, which AppKit's `convert` does not see (`plane`).
            let reach = extent.map { e -> CGRect in
                guard let h = v?.plane else { return r.convert(CGRect(origin: .zero, size: e), to: root) }
                let corners = [CGPoint.zero, CGPoint(x: e.width, y: 0), CGPoint(x: 0, y: e.height), CGPoint(x: e.width, y: e.height)].map { NodeView.map(h, $0) }
                return CGRect(x: 0, y: 0, width: corners.map(\.x).max() ?? 0, height: corners.map(\.y).max() ?? 0)
            } ?? .zero
            size.width = max(size.width, r.frame.maxX, reach.maxX)
            size.height = max(size.height, r.frame.maxY, reach.maxY)
        }
        if root.frame.size != size { root.frame = NSRect(origin: .zero, size: size) }
        // The document just changed size; whether the page can scroll — and
        // so whether it may bounce — changed with it.
        viewport.syncElasticity()
        viewport.acceptDocumentFit()
    }
    var onPress, onClose: ((UInt32) -> Void)?
    var onChange: ((UInt32, String) -> Void)?
    /// Images' intrinsic sizes, several at once under one layout.
    var onIntrinsic: (([(UInt32, CGSize?)]) -> Void)?
    /// A capability an action called (LLP 1005 §3), after its commit.
    var onCommand: ((String, [Any], UInt32?) -> Void)?
    /// A `key` handler called `preventDefault()` (`keyDown(at:_:)`, KeyEvents.swift).
    var defaultPrevented = false
    /// The last wheel or magnify event and whether a `wheel` handler
    /// prevented it: every view it passes up asks once (MouseEventsMac.swift).
    var lastWheel: (NSEvent, Bool)?
    /// A `key` handler called `stopPropagation()` (`keyDown(at:_:)`, KeyEvents.swift).
    var propagationStopped = false
    /// The ⌘ chord that last ended a composition (`endComposition`, KeyEvents.swift).
    var composedChord: NSEvent?

    lazy var popoverEvents = PopoverEvents(self)
    /// `showModal(id)`, `close(id)` from an action (LLP 1115 D6): a
    /// `dialog` by its id, in the session's top layer as its invoker's
    /// `command="show-modal"` puts it (DialogsMac).
    func dialogCommand(_ name: String, _ id: String) {
        guard let dialog = carrying("tag:dialog").first(where: { $0.props["id"] == id }) else {
            if name == "showModal" { session?.log("showModal \(id) refused: no dialog has that id") }
            return
        }
        if name == "showModal", dialogs.owns(dialog) || alerts.isOpen(dialog) {
            session?.log("showModal \(id) ignored: dialog is already open")
        } else if name == "showModal" { dialogs.show(dialog) } else { dialogs.close(dialog) }
    }

    /// The action's focus(html-id), delivered only after the batch is mounted.
    func focusElement(_ args: [Any], selectText: Bool = false) {
        guard args.count == 1, let name = args.first as? String,
              let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else { return }
        if !selectText, menus.focus(target) { return }
        // A search input the window's toolbar hosts, or a prompt's input lent to its
        // alert, has no box in the page (LLP 1115.002 D2): the caret goes to its field.
        if target.pageField != nil {
            if !target.formDisabled, toolbar.focusSearch(target) || alerts.focus(target), selectText { fieldSelections.selectAll(target) }
            return
        }
        guard let window = target.window, !target.formDisabled,
              target.bounds.width > 0, target.bounds.height > 0 else { return }
        var ancestor: NSView? = target
        while let view = ancestor {
            if view.isHidden || (view as? NodeView)?.inert == true { return }
            ancestor = view.superview
        }
        if target.kind == "native", !selectText { _ = session?.natives.focus(target); return }
        if selectText, target.textArea == nil, target.field == nil { return }
        if let field = target.field, window.firstResponder === field.currentEditor() {
            if selectText { fieldSelections.selectAll(target) }
            return
        }
        let responder = target.web ?? keyView(of: target)
        if responder.acceptsFirstResponder { window.makeFirstResponder(responder) }
        if selectText, window.firstResponder === target.textArea || target.field?.currentEditor() != nil { fieldSelections.selectAll(target) }
    }

    /// The action's blur(): drop the first responder; blur(html-id) only when
    /// that node holds it (a field's responder is its editor).
    func blurElement(_ args: [Any]) {
        // In the window the keyboard is in: an open dialog's sheet, else the page's.
        guard var window = frontWindow else { return }
        if let name = args.first as? String {
            guard let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else { return }
            window = target.window ?? window
            if target.kind == "native" {
                guard window.firstResponder === target || session?.natives.ownsFocus(target) == true else { return }
                window.makeFirstResponder(nil)
                return
            }
            let responder = keyView(of: target)
            guard window.firstResponder === responder || window.firstResponder === target.field?.currentEditor() else { return }
        }
        window.makeFirstResponder(nil)
    }

    /// The events beyond press and change (LLP 1005 §3).
    var onHover: ((UInt32, Bool) -> Void)?
    var onFocus, onBlur: ((UInt32) -> Void)?
    /// A `key` or `keyup` (`KeyPress.up`) at a node (KeyEvents.swift).
    var onKey: ((UInt32, KeyPress) -> Void)?
    var onClipboard: ((UInt32, UInt32, String) -> Void)?
    /// A `text`'s part of the selection changed: its text and source offsets.
    var onSelectionChange: ((UInt32, String, Int, Int, [CGRect]) -> Void)?
    var onContextmenu: ((UInt32) -> Void)?
    var onDblclick: ((UInt32) -> Void)?
    /// The primary button went down on a node (`true`) or came up (LLP 1005 §3).
    var onPointer: ((UInt32, PointerKind, PointerSample) -> Void)?
    /// The innermost node hearing a pointer event that the button went down on, until it comes up: it hears the drags.
    var pointerHeld: UInt32?
    /// The nodes from it up that hear `pointerdown` or `pointerup`, innermost first: each hears the up (LLP 1005 §3).
    var pointerPath: [UInt32] = []
    /// The last input was a pointer's press, not a key: a focus it, or a
    /// handler it ran, moves shows no ring (`:focus-visible`, `FocusMac.swift`).
    var focusByPointer = false
    /// The view AppKit sends the held button's drags and up to: the one it
    /// went down on, perhaps a child of the held node, kept in the window
    /// until the button comes up even if a batch removes it (`MouseChainMac`).
    var pointerSource: NodeView?
    /// The drag last delivered as a `pointermove` (LLP 1056 §3 stage 3).
    weak var pointerDrag: NSEvent?
    /// Each node's latest free move, in the order the pointer reached them,
    /// sent at the next display frame or before a button goes down or up
    /// (`hoverMoved`).
    var hoverMoves: [(UInt32, PointerSample)] = []
    var hoverLink: CADisplayLink?
    let hoverTarget = PumpTarget()
    /// The next display frame's hit-test of a resting pointer the layout or
    /// a scroll moved content under (`followPointer`); one a frame.
    var followLink: CADisplayLink?
    let followTarget = PumpTarget()
    /// Where the agent's pointer rests, in window points (its last `tap …
    /// hover`); under the agent it stands for the system cursor.
    var agentPointer: NSPoint?
    var onSwiperight: ((UInt32) -> Void)?
    /// Pull-to-refresh is UIKit's; AppKit has no such control, so this never fires.
    var onRefresh: ((UInt32) -> Void)?
    var onPan: ((UInt32, Double, Double) -> Void)?
    /// A pan that began ended (LLP 1057 §10.6); the tracker below measures
    /// where the platform gives no velocity (LLP 1057.001 §3).
    var onPanRelease: ((UInt32, Double, Double) -> Void)?
    var onPanSample: ((Bool, Double, Double, Double) -> Void)?
    var panVelocity: ((Double) -> (Double, Double))?
    /// A scroller with a `scroll` handler moved: left, top, then its
    /// `scrollWidth`, `scrollHeight`, `clientWidth`, `clientHeight`.
    var onScroll: ((UInt32, [Double]) -> Void)?
    var onScrollend: ((UInt32) -> Void)?
    /// A scroller (nil: the page) moved, handler or not: `frame()` reads
    /// boxes where the viewer sees them (LLP 1051.000 D1).
    var onScrolled: ((UInt32?, Double, Double) -> Void)?
    var onListIndex: ((UInt32, String) -> Int?)?
    var onListText: ((UInt32, (String, Int, Int)?, (String, Int, Int)?) -> String)?
    var interacting: UInt32 = 0
    private(set) var listViews: [UInt32: NodeView] = [:]

    /// Everything kept for `id`, out of the maps (not out of the window);
    /// `forget` releases its resources too, as a destroy does and a leaving
    /// view (LLP 1063) does not until its exit ends.
    @discardableResult
    func release(_ id: UInt32, forget: Bool) -> NodeView? {
        if let node = views[id] { mouseChain.bubbledPress?.cancel(in: node) }
        focusEvents.removed(id)
        // A held node that goes has no handler left to hear its up.
        if pointerHeld == id { pointerHeld = nil }
        pointerPath.removeAll { $0 == id }
        mouseSwipe.retire(id)
        mouseLayoutPan.retire(id)
        mouseDrags.height.retire(id)
        mouseDrags.transform.retire(id)
        mouseDrags.reorder.retire(id)
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
        // A view made later may have this one's identifier: its `autofocus` is its own to offer.
        if let view = views[id] { autofocusProcessed.remove(ObjectIdentifier(view)) }
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
    /// The innermost hovered listener, with its listening ancestors below.
    weak var hovered: NodeView?
    var hoveredNodes: [NodeView] = [] // innermost first, the hit's ancestor path
    var lookHovered: [WeakNodeView] = [] // those whose `:hover` look holds (HoverMac.swift)
    weak var trackedPointerEvent: NSEvent?
    /// The canvas input the tracked move reached through the hit's ancestors.
    weak var trackedCanvas: AnyObject?
    /// Whether the pointer was last over text that shows the I-beam, and
    /// whether this presenter set that cursor (`pointerCursor`).
    var pointerOverText = false, textCursorSet = false
    /// The node whose key down the system's key bindings are naming, and
    /// whether the command they named was taken (`NodeView.keyCommand`).
    var keyCommand: (source: NodeView, taken: Bool)?
    private var hoverTransition: UInt64 = 0
    var hoveredInline: UInt32?
    var inlineHoverTransition: UInt64 = 0

    /// The modifiers held for the press being sent (its `MouseEvent`'s; gallery F20).
    var pressHeld = ""
    /// The click being sent as a press, given by the mouse paths only (`pressMount`, FocusMac.swift).
    var pressClick: NSEvent?
    package func press(_ id: UInt32, fromNativeMenu: Bool = false, held: String = "", bubbled: Bool = false, outsideModal: Bool = false, click: NSEvent? = nil) {
        pressHeld = held; defer { pressHeld = "" } // `outsideModal`: an app-wide menu item's (ShortcutsMac `enabled`)
        guard let node = textHost(id), !(bubbled || outsideModal ? node.authoredInert : node.inert), !node.disabled,
              fromNativeMenu || (segments.shown(node) ?? sidebar.shows(node) ?? !node.isHiddenOrHasHiddenAncestor) || toolbar.contains(node) else { return }
        commitPendingEdit() // the press no longer ends a field's editing (FocusMac.swift)
        let command = dialogs.command(node, fromNativeMenu: fromNativeMenu, bubbled: bubbled)
        let popover = menus.command(node, fromNativeMenu: fromNativeMenu, bubbled: bubbled)
        if command == nil && popover == nil, node.id == id, let url = node.defaultLink, node.activateLink(url, bubbled: bubbled) { return }
        if (command == nil && popover == nil) || node.handlers.contains("press") { pressClick = click ?? pressClick; onPress?(id); pressClick = nil }
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
        var value = value
        // A number input's typed value past `min` or `max` commits as the
        // bound (LLP 1116.008 D5), told as a step is: `input`, then `change`.
        if let node = views[id], let bound = node.clampTypedNumber(value) {
            value = bound
            if node.handlers.contains("input") { onInput?(id, bound) }
        }
        if change { onChange?(id, value) }
    }
    package func checked(_ id: UInt32, _ on: Bool) { onChecked?(id, on) }
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
    private var afterBatchWork: [() -> Void] = []
    /// Work for after the batch being applied, or now: a hatch's act on an
    /// element never lands inside a batch (LLP 1075.003 §3.4).
    func afterBatch(_ work: @escaping () -> Void) { if applying { afterBatchWork.append(work) } else { work() } }
    private func send(_ id: UInt32, _ f: @escaping () -> Void) {
        guard !resetting, textHost(id) != nil else { return }
        if applying { waiting.append((id, f)) } else { f() }
    }
    /// Pointerenter/leave cover a node's subtree, including visible overflow.
    /// Only the hit's ancestors enter; overlapping tracking rectangles do not
    /// give their occluded owners a separate hover.
    func setHoverPath(_ nodes: [NodeView], inline: UInt32? = nil) {
        if applying { afterBatch { [weak self] in self?.setHoverPath(nodes, inline: inline) }; return }
        hoverTransition &+= 1
        inlineHoverTransition &+= 1 // cancel an inline enter awaiting its leave callback
        let generation = hoverTransition
        defer { if hoverTransition == generation { hoverLooks(nodes) } } // LLP 1121 D8 (HoverMac.swift)
        func eligible(_ node: NodeView) -> Bool {
            views[node.id] === node && node.isDescendant(of: viewport) && !node.inert && !node.isHiddenOrHasHiddenAncestor && node.handlers.contains("hover")
        }
        let next = nodes.filter(eligible)
        if hoveredInline != inline {
            hoverInline(nil)
            guard hoverTransition == generation else { return }
        }
        for node in hoveredNodes where !next.contains(where: { $0 === node }) {
            hoveredNodes.removeAll { $0 === node }; hovered = hoveredNodes.first
            send(node.id) { [self] in
                guard hoverTransition == generation else { return }
                onHover?(node.id, false)
            }
            guard hoverTransition == generation else { return }
        }
        for node in next.reversed() where !hoveredNodes.contains(where: { $0 === node }) {
            guard eligible(node) else { continue }
            hoveredNodes.insert(node, at: 0); hovered = hoveredNodes.first
            send(node.id) { [self] in
                guard hoverTransition == generation, eligible(node) else { return }
                onHover?(node.id, true)
            }
            guard hoverTransition == generation else { return }
        }
        hoveredNodes.removeAll { !eligible($0) }; hovered = hoveredNodes.first
        guard hoverTransition == generation else { return }
        if let inline, inlineEnabled(inline) { hoverInline(inline) }
    }
    func key(_ id: UInt32, _ press: KeyPress) { send(id) { [self] in onKey?(id, press) } }
    func clipboard(_ id: UInt32, _ kind: UInt32, _ text: String) { send(id) { [self] in onClipboard?(id, kind, text) } }
    func selectionChange(_ id: UInt32, _ text: String, _ start: Int, _ end: Int, rects: [CGRect] = []) { send(id) { [self] in onSelectionChange?(id, text, start, end, rects) } }
    func contextmenu(_ id: UInt32) { send(id) { [self] in onContextmenu?(id) } }
    /// A `contextmenu` with its point (10), a `wheel` (37) or a `drop` (38)
    /// with its line (studio diary R22, R3, R19; MouseEventsMac.swift).
    func mouseEvent(_ id: UInt32, _ kind: UInt32, _ line: String) { send(id) { [self] in onClipboard?(id, kind, line) } }
    func dblclick(_ id: UInt32) { send(id) { [self] in onDblclick?(id) } }
    func pointer(_ id: UInt32, _ kind: PointerKind, _ sample: PointerSample) { send(id) { [self] in onPointer?(id, kind, sample) } }
    func swiperight(_ id: UInt32) { send(id) { [self] in onSwiperight?(id) } }
    func pan(_ id: UInt32, _ dx: Double, _ dy: Double) { send(id) { [self] in onPan?(id, dx, dy) } }
    /// Once per pan that began, after its last delta; only to a node that hears it.
    func panRelease(_ id: UInt32, _ vx: Double, _ vy: Double) {
        guard views[id]?.handlers.contains("panrelease") == true else { return }
        send(id) { [self] in onPanRelease?(id, vx, vy) }
    }
    func scroll(_ id: UInt32, _ metrics: [Double]) { send(id) { [self] in onScroll?(id, metrics) } }
    func scrollend(_ id: UInt32) { send(id) { [self] in onScrollend?(id) } }
    func submit(_ id: UInt32) { send(id) { [self] in onSubmit?(id) } }
    func load(_ id: UInt32) { send(id) { [self] in onLoad?(id) } }
    package func message(_ id: UInt32, _ value: String) {
        guard let view = views[id], view.handlers.contains("message") else { return }
        send(id) { [weak self, weak view] in
            guard let self, let view, views[id] === view, view.handlers.contains("message") else { return }
            onMessage?(id, value)
        }
    }
    func intrinsic(_ id: UInt32, _ size: CGSize?) { onIntrinsic?([(id, size)]) }

    func apply(_ batch: Batch) {
        session?.fieldChrome.presented(batch.layoutProvisional)
        defer { applyLanguage(batch); applyWakeLock(batch) }
        PaintOrder.begin()
        let post = Self.signposts.beginInterval("apply", "\(batch.ops.count) ops")
        defer { Self.signposts.endInterval("apply", post) }
        viewport.invalidateDocumentFit()
        collections.beginBatch(batch)
        // The window's bar is read again only by a batch that can change it (`WindowToolbarHost.concerns`).
        let bar = toolbar.concerns(batch)
        if bar { toolbar.prepare() }
        anchorChanges.reset()
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
        // Hatched nodes this batch destroys end first, while their views are
        // still in the window (a row's root is destroyed before its children).
        elements.begin(batch)
        // Create, frame or content ops: rows may have come or moved (`HeavyLeaves.batchApplied`).
        let moved = batch.ops.contains { [.create, .frame, .content].contains($0.op) }
        // What may now lie under a resting pointer: boxes made, moved, gone or transformed.
        let relaid = batch.ops.contains { [.create, .frame, .children, .roots, .destroy, .present, .style, .props, .rank, .sticky, .fragments, .exit].contains($0.op) }
        defer {
            collections.endBatch()
            collections.observeKnobDrags()
            collections.limitPrepared()
            PaintOrder.end()
            if outermost {
                applying = false
                videoVisibility?.changed()
                let geometry = pendingGeometry
                pendingGeometry = nil
                let q = waiting
                waiting = []
                geometry?()
                for (id, f) in q where textHost(id) != nil { f() }
                let later = afterBatchWork
                afterBatchWork = []
                later.forEach { $0() }
                batchApplied()
                if !boxFilters.isEmpty { boxFilters.render() }
                leaves.batchApplied(moved: moved)
            }
            // Only scheduled: the hit-test is the next display frame's.
            if relaid { followPointer() }
        }
        if !batch.ops.isEmpty { textViewportIndex = nil }
        var reparented = Set<UInt32>()
        let structureChanged = batch.ops.contains { [.children, .roots, .destroy, .create, .style].contains($0.op) }
        if structureChanged { selection.structureChanged() }
        for op in batch.ops {
            let kind = op.op
            let id = op.id
            if kind == .children { touched(id, children: true) } else if kind != .roots && kind != .create { touched(id, textChanged: kind == .props || kind == .style || kind == .destroy || kind == .fieldContent) }
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
                v.applyStyle(v.presented(op.style))
                v.applyProps(set: op.props, clear: [])
                views[id] = v
                elements.created(v)
                if v.kind == "list" { listViews[id] = v }
                if v.kind == "video" { leaves.created(v) }
            case .paragraph:
                applyParagraph(id, op.runs)
            case .props:
                views[id]?.applyProps(set: op.props, clear: op.clear)
                elements.propsChanged(id)
            case .flow:
                views[id]?.applyFlow(op.payload["shapes"] as? [[String: Any]] ?? [])
            case .style:
                guard let v = views[id] ?? leaving[id]?.view else { continue }
                let color = v.style["text_color"], old = v.style
                v.applyStyle(v.presented(op.style))
                anchorChanges.note(id, from: old, to: v.style, popover: v.props["popover"] != nil)
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
                tables.children(id, want)
                let wanted = Set(want.map { ObjectIdentifier($0) })
                for child in container.subviews where child is NodeView && !wanted.contains(ObjectIdentifier(child)) && !isLeaving(child) {
                    if let node = child as? NodeView { reparented.insert(node.id) }
                    child.removeFromSuperview()
                }
                let mounted = want.filter { !dialogs.owns($0) && !menus.owns($0) && !blocks.owns($0) && !tables.owns($0, in: id) && !isFlying($0) }
                for child in mounted where child.superview !== container {
                    reparented.insert(child.id)
                    child.rejoinUnderHold()
                    child.prepareToMount()
                    container.addSubview(child)
                }
                orderChildren(want, in: container)
            case .surface:
                if let v = views[id] { session?.canvases.surface(view: v, name: op.payload["name"] as? String ?? "", values: op.payload["values"] ?? []) }
            case .canvas2d: if let v = views[id] { canvas2d.apply(id, op.payload, layer: v.layer) }
            case .svg: if let v = views[id] { svg.scene(id, op.payload, layer: v.layer, dark: v.drawsDark, clock: session?.clock, limit: v.style["dynamic_range_limit"]?.string, tint: v.viewTint) }
            case .animations: svg.animations(id, op.payload, layer: views[id]?.layer, clock: session?.clock)
            case .command:
                let name = op.payload["name"] as? String ?? ""
                if name == "preventDefault" { defaultPrevented = true; break }
                if name == "stopPropagation" { propagationStopped = true; break }
                onCommand?(name, op.payload["args"] as? [Any] ?? [], (op.payload["source"] as? NSNumber)?.uint32Value)
            case .exit: beginExit(id)
            case .flight: beginFlight(op)
            case .land: if let f = flights[id] { landFlight(f) }
            case .rank:
                if let rank = op.payload["rank"] as? NSNumber { views[id]?.setRank(rank.int64Value) }
            case .sticky: stickies.apply(id, op.payload)
            case .fragments: views[id]?.applyColumns(op.payload)
            case .unknown: blocks.applied(op)
            case .destroy:
                elements.destroyed(id)
                blocks.destroyed(id)
                stickies.forget(id)
                forgetFlight(id)
                views[id]?.holdGone()
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
                if flightFrame(id, frame) { continue }
                if !dialogs.frame(v, frame) && !menus.frame(v, frame) && !tables.frame(v, frame) { v.frame = frame }
                v.arrangeShift = .zero
                v.textRasterGeometryChanged()
                v.scroll?.frame = v.bounds
                v.layoutField()
                v.layoutTextArea()
                v.metal?.frame = v.bounds
                v.overlay?.frame = v.bounds
                v.web?.frame = v.bounds
                v.applyShadow()
                v.fitScroll()
                v.applyTransform()
                v.layoutContent()
            case .fieldContent:
                views[id]?.applyFieldContent(op.payload)
            case .content:
                if let v = views[id] {
                    v.content = CGSize(width: op.w, height: op.h)
                    v.fitScroll()
                }
            case .present:
                guard let v = views[id] ?? leaving[id]?.view else { continue }
                let x = CGFloat(op.x)
                switch op.property {
                case "translate": v.translatePx = CGPoint(x: x, y: CGFloat(op.y)); v.translatePercent = CGPoint(x: CGFloat(op.w), y: CGFloat(op.h)); v.applyTransform()
                case "layout": v.layoutOffset = CGPoint(x: x, y: CGFloat(op.y)); v.layoutScale = CGPoint(x: CGFloat(op.w), y: CGFloat(op.h)); v.applyTransform(); v.applySurface()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.presentedAlpha = x; v.alphaValue = v.disabledShown ? x * NodeView.disabledOpacity : x; v.refreshPressOpacity()
                case "flight": presentFlight(id, x)
                default: break
                }
            default: break
            }
        }
        if !flights.isEmpty { flightsBatchApplied() }
        tables.sync(batch)
        navigation.sync(batch, reparented: reparented)
        sidebar.sync(batch)
        if !pendingTips.isEmpty { settleTips() }
        fitDocument()
        // The page's canvas colour is the first root's background — what
        // shows beyond a document shorter than the viewport, as a browser
        // paints the root element's background over the whole canvas.
        // Unset, the platform's window background (LLP 1115 D2).
        let ground = ThemeRoles.color("background", in: root) ?? ThemeRoles.color("Canvas", in: root, or: .windowBackgroundColor)
        let color = (root.subviews.first as? NodeView)?.color("background_color", ground) ?? ground
        if viewport.backgroundColor != color { viewport.backgroundColor = color; navigation.groundChanged() }
        let first = root.subviews.first as? NodeView
        let fit = first?.props["viewportFit"]
        if fit != viewportFit { viewportFit = fit; onViewportFit?() }
        session?.canvases.cancelMovedControls()
        PaintOrder.flush()
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
        if bar { toolbar.sync() }
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
        let changed = touchedAndAbove(batch.ops.lazy.filter { Self.accessibilityOps.contains($0.op) }.map(\.id)).union(reparented)
        let ariaProps = batch.ops.filter { $0.op == .props && ($0.props["accessibilityDisabled"] != nil || $0.clear.contains("accessibilityDisabled")) }.map(\.id)
        let disabledRoots = Set(ariaProps).union(reparented.filter { id in
            guard let node = views[id] else { return false }
            return node.isAccessibilityEnabled() != (!node.disabled && !node.ariaUnavailable)
        })
        syncAccessibility(changed: changed, disabledRoots: disabledRoots)
        _ = chrome.takeChangedNames()
    }

    func orderChildren(_ children: [NodeView], in container: NSView) {
        // Detaching a retained child clears AppKit's first responder. Sort
        // in place; a flight's slot stands for its lifted child (LLP 1013.000).
        // Exit ghosts keep their slots among both live and flying children.
        let wanted = Set(children.map(ObjectIdentifier.init))
        let current = container.subviews
        var ordered: [NSView] = children.compactMap { child in
            if child.superview === container { return child }
            guard let flight = flights[child.id], flight.view === child,
                  let slot = flight.slot, slot.superview === container else { return nil }
            return slot
        }
        let placed = Set(ordered.map(ObjectIdentifier.init))
        let peers = current.filter { $0 is NodeView || $0 is FlightSlot }
        for (index, child) in peers.enumerated() where !placed.contains(ObjectIdentifier(child)) &&
            ((child as? NodeView)?.paintGhost == true || child is FlightSlot) {
            ordered.insert(child, at: min(index, ordered.count))
        }
        let leaving = current.filter { ($0 as? NodeView).map { !$0.paintGhost } ?? false && !wanted.contains(ObjectIdentifier($0)) }
        let native = current.filter { !($0 is NodeView) && !($0 is FlightSlot) }
        ordered = native + ordered + leaving
        guard !ordered.elementsEqual(current, by: { $0 === $1 }) else { return }
        var ranks = Dictionary(uniqueKeysWithValues: ordered.enumerated().map { (ObjectIdentifier($0.element), $0.offset) })
        withUnsafeMutablePointer(to: &ranks) { context in
            container.sortSubviews({ left, right, raw in
                let order = raw!.assumingMemoryBound(to: [ObjectIdentifier: Int].self).pointee
                let a = order[ObjectIdentifier(left)]!, b = order[ObjectIdentifier(right)]!
                return a < b ? .orderedAscending : a > b ? .orderedDescending : .orderedSame
            }, context: context)
        }
    }

    private static let accessibilityOps: Set<BatchOp.Kind> = [.create, .props, .style, .children, .paragraph, .flow, .frame, .fieldContent]

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
    /// A scroll whose batch skipped the pass (`ExactSession.applyUnlessEmpty`):
    /// a context preview follows its source out of the scrolled box.
    func scrolledWithoutPass() { positionContexts() }

    private func positionContexts() {
        for preview in carrying("contextTarget") {
            guard let target = preview.props["contextTarget"],
                  let source = views.values.first(where: { $0.props["id"] == target }),
                  source.window != nil else { continue }
            var ancestor = preview.superview as? NodeView
            while let node = ancestor, node.layoutPosition != "absolute" {
                ancestor = node.superview as? NodeView
            }
            guard let panel = ancestor, let parent = panel.superview else { continue }
            let sourceBox = source.convert(source.bounds, to: parent)
            let port = viewport.convert(viewport.bounds, to: parent)
            // Static wrappers do not contain the panel. Clamp against its
            // nearest positioned ancestor, expressed in the parent coordinates.
            var containing = parent
            while containing !== root && containing.superview !== root {
                if let node = containing as? NodeView, node.layoutPosition != "static" { break }
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

    /// The key-view loop (KeyViewLoopMac.swift): whether a batch changed
    /// what it is built from, the Keyboard Navigation setting it was built
    /// for, and for each view that can hold the focus and is no stop, the
    /// stop before it (where Shift-Tab goes).
    var keyViewLoopStale = false
    var keyViewLoopMode: Bool?
    let keyViewBefore = NSMapTable<NSView, NSView>.weakToWeakObjects()
    /// The control that lost the focus by becoming disabled, while nothing
    /// has taken it since: HTML's sequential focus navigation starting
    /// point, which its focus fixup rule leaves there, so Tab goes on from
    /// the control's place (`tabFromStart`).
    weak var tabStart: NSView?

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
package enum Capture {
    /// A capture is drawing: its draws are not repaints (D4 b).
    nonisolated(unsafe) static var capturing = false
    /// Guest pictures for this turn; the remote platform views are hidden
    /// while their owning nodes draw these (@ref LLP 1020 D4/D6).
    nonisolated(unsafe) static var web: [UInt32: ExactWebImage] = [:]

    /// The subtree painted at `scale`: premultiplied RGBA, rows top-down,
    /// `pixelsWide * 4` bytes per row, transparent where nothing painted.
    package static func bitmap(of view: NSView, scale: CGFloat) -> NSBitmapImageRep? {
        // A subtree painted through its canvas composites at alpha 0; paint
        // it opaque into the bitmap regardless.
        let alpha = view.alphaValue
        view.alphaValue = 1
        // A canvas nested under this one that is painted through its own
        // surface: its picture comes by readback (its draw), not from its
        // overlay's views, so those are hidden for the duration.
        var hidden: [NSView] = []
        func hide(_ v: NSView) {
            for s in v.subviews {
                if let n = s as? NodeView, n.placement != nil, !n.isHidden { n.isHidden = true; hidden.append(n); continue }
                if let n = s as? NodeView, let o = n.overlay, o.alphaValue == 0, !o.isHidden { o.isHidden = true; hidden.append(o); continue }
                hide(s)
            }
        }
        hide(view)
        let rep = paintOrderBitmap(of: view, scale: scale)
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
            // A paragraph outside every scroll view (a heading over a split's
            // detail pane, a popover's text) is in view wherever its window
            // is: its window's content view stands in as its document.
            guard let document = node.enclosingScrollView?.documentView ?? node.window?.contentView else { continue }
            let key = ObjectIdentifier(document)
            let index = positions[key] ?? groups.count
            if index == groups.count { positions[key] = index; groups.append(Group(document: document, entries: [])) }
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

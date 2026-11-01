#if os(macOS)
import AppKit

/// Host UI over the window's content. The card takes the clicks within its bounds, and clicks
/// outside it reach the app (LLP 1079.000 §7.2).
final class FrameHealthOverlayMac: NSView {
    weak var health: FrameHealth?
    private let card = FrameHealthCardViewMac()
    private let reader = FrameHealthCardReader()
    /// The corner the card rests in; the window's coordinates rise, so the top is `maxY`
    private var corner = FrameHealthCardFace.Corner(maxY: true)
    private var paused = false
    /// While paused, the request and display the frozen samples were read under
    private var frozen: (FrameHealthCardText.Rates, FrameHealthCardText.Display)?
    /// The generation the charts last took frames from
    private var chartGeneration: UInt64?
    var desiredVisible = false
    var showDetails: (() -> Void)?

    init(health: FrameHealth) {
        self.health = health
        super.init(frame: .zero)
        identifier = NSUserInterfaceItemIdentifier("exact.host.frame-health-overlay")
        setAccessibilityIdentifier("exact.host.frame-health-overlay")
        isHidden = true
        addSubview(card)
        card.grabbed = { [weak self] in self?.stopSnap() }
        card.moved = { [weak self] origin in self?.health?.hudDragging = true; self?.place(at: origin) }
        card.released = { [weak self] velocity in
            guard let self else { return }
            self.health?.hudDragging = false
            if let velocity { settle(velocity: velocity); render() } else { rest() }
        }
        card.activate = { [weak self] control in self?.activate(control) }
        card.expandedChanged = { [weak self] in self?.render() }
        card.setAccessibilityCustomActions([("Move up", CGVector(dx: 0, dy: 1)), ("Move down", CGVector(dx: 0, dy: -1)),
            ("Move left", CGVector(dx: -1, dy: 0)), ("Move right", CGVector(dx: 1, dy: 0))].map { name, edge in
            NSAccessibilityCustomAction(name: name) { [weak self] in self?.move(toward: edge) ?? false }
        })
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override var isFlipped: Bool { false }

    func setVisible(_ visible: Bool) {
        desiredVisible = visible; isHidden = !visible
        health?.setHUDVisible(visible)
        health?.changed = visible ? { [weak self] in self?.refresh() } : nil
        if !visible { health?.hudDragging = false; card.cancelDrag() }
        if visible { layoutSubtreeIfNeeded() }
        refresh()
        needsLayout = true
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        // A covered window is not seen: the reader stops and resumes with its occlusion
        occlusion.map(NotificationCenter.default.removeObserver)
        occlusion = window.map { window in
            NotificationCenter.default.addObserver(forName: NSWindow.didChangeOcclusionStateNotification, object: window,
                queue: .main) { [weak self] _ in self?.refresh() }
        }
        refresh()
    }

    deinit { occlusion.map(NotificationCenter.default.removeObserver) }

    /// Sample on each publication. A pause freezes the reader, so the face, accessibility and Copy
    /// all describe the same samples; a drag only defers drawing.
    /// Shown, fitting, and in a window that is on screen and not covered, with no hidden ancestor.
    private var canBeSeen: Bool {
        guard desiredVisible, !card.isHidden, !card.isHiddenOrHasHiddenAncestor, let window else { return false }
        return window.isVisible && window.occlusionState.contains(.visible)
    }
    private var occlusion: NSObjectProtocol?

    func refresh(now: Double = CACurrentMediaTime()) {
        guard let health else { return }
        health.hudEffectiveVisible = canBeSeen
        guard canBeSeen else {
            // Release the history; the next read after the card shows again is a fresh baseline
            if reader.reading { reader.stop(); card.clear(); feed(false); card.chart.stop() }
            return
        }
        if !reader.reading {
            // The first read is a baseline; its maxima cover time the card did not watch
            guard reader.start() else { render(); return }
            card.chart.start()
            reader.sample(health, now: now, force: true)
            feed(!paused)
        } else if !paused {
            // A stopped source changes only at lifecycle boundaries, so every one is read
            reader.sample(health, now: now, force: health.snapshot().running == 0)
        }
        guard card.drag?.moved != true else { return }
        render()
    }

    /// Each frame feeds the charts while the card reads and is not paused; a pause holds them, and
    /// the first frame after it bounds no interval. A new generation during a pause clears them, as
    /// the reader's next sample clears it.
    private func feed(_ on: Bool) {
        guard let health else { return }
        let generation = health.snapshot().generation
        if on, generation != chartGeneration { card.chart.reset() }
        chartGeneration = generation
        health.chart = on ? card.chart : nil
        card.chart.endRun()
    }

    private var rates: FrameHealthCardText.Rates {
        frozen?.0 ?? FrameHealthCardText.Rates(requestedHz: Double(health?.requestedHertz ?? 0),
            periodMs: Double(reader.samples.last?.periodNs ?? health?.snapshot().period_ns ?? 0) / 1e6)
    }

    private var display: FrameHealthCardText.Display { frozen?.1 ?? Self.display(window?.screen) }

    private func render() {
        guard let health else { return }
        card.update(FrameHealthCardPresentation(reader: reader, rates: rates, display: display,
            status: paused ? .paused : health.snapshot().running != 0 ? .live : .idle,
            unavailable: health.enabled ? reader.reading ? nil : FrameHealthReaders.full : health.unavailableReason))
    }

    /// Fixed refresh has one interval; Adaptive-Sync takes any rate in its range (a granularity of 0,
    /// per `NSScreen.displayUpdateGranularity`); otherwise the system chooses among a set of rates.
    static func display(_ screen: NSScreen?) -> FrameHealthCardText.Display {
        guard let screen else { return .init(name: "Display", detail: "unknown", maximumHz: 60, adaptiveSync: false) }
        let maximum = Double(screen.maximumFramesPerSecond)
        let fastest = screen.minimumRefreshInterval, slowest = screen.maximumRefreshInterval
        if fastest <= 0 || slowest <= 0 || abs(slowest - fastest) < 1e-6 {
            return .init(name: "Fixed refresh", detail: "\(Int(maximum)) Hz", maximumHz: maximum, adaptiveSync: false)
        }
        if screen.displayUpdateGranularity == 0 {
            return .init(name: "VRR (Adaptive-Sync)", detail: "\(Int((1 / slowest).rounded()))–\(Int((1 / fastest).rounded())) Hz",
                maximumHz: maximum, adaptiveSync: true)
        }
        return .init(name: "Adaptive refresh", detail: "up to \(Int(maximum)) Hz", maximumHz: maximum, adaptiveSync: false)
    }

    private func activate(_ control: FrameHealthCardFace.Control) {
        switch control {
        case .status:
            frozen = paused ? nil : (rates, display)
            paused.toggle()
            feed(!paused)
            NSAccessibility.post(element: card, notification: .announcementRequested,
                userInfo: [.announcement: paused ? "Paused" : "Resumed"])
            // Resuming reads at once; the first sample covers the paused span and names it
            if paused { render() } else { refresh() }
        case .close: setVisible(false)
        case .title: card.showDefinitions()
        case .copy: copySummary()
        case .charts, .table:
            card.state.table = control == .table
            render()
        }
    }

    /// One pasteboard item with the text summary and a picture of the charts and every figure.
    private func copySummary() {
        let (text, picture) = FrameHealthCardText.copy(reader: reader, chart: card.chart, rates: rates, display: display, paused: paused,
            palette: card.palette)
        let item = NSPasteboardItem()
        item.setString(text, forType: .string)
        if let picture { item.setData(picture, forType: .png) }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.writeObjects([item])
        card.showCopied(picture == nil ? "Text copied" : "Picture and text copied")
    }

    override func layout() {
        super.layout()
        let available = bounds.insetBy(dx: 8, dy: 8)
        let compact = available.height < FrameHealthCardFace.height
        let height = compact ? FrameHealthCardFace.compactHeight : FrameHealthCardFace.height
        card.compact = compact
        let size = CGSize(width: min(FrameHealthCardFace.width, available.width), height: height)
        if card.frame.size != size { card.setFrameSize(size); card.needsDisplay = true }
        // A spring toward a corner the new geometry moved turns toward where it is now
        let target = corner.origin(of: size, in: available)
        if card.drag?.moved == true { place(at: card.frame.origin) }
        else if !snapping { place(at: target) } else if card.frame.origin != target { stopSnap(); spring(velocity: .zero) }
        let wasHidden = card.isHidden
        card.isHidden = available.height < height || available.width < FrameHealthCardFace.minimumWidth
        health?.hudEffectiveVisible = canBeSeen
        if card.isHidden != wasHidden { refresh() }
    }

    func place(at point: CGPoint) {
        card.setFrameOrigin(CGPoint(x: min(max(bounds.minX + 8, point.x), max(bounds.minX + 8, bounds.maxX - card.frame.width - 8)),
            y: min(max(bounds.minY + 8, point.y), max(bounds.minY + 8, bounds.maxY - card.frame.height - 8))))
        // In the window's coordinates, wherever the embedder's container sits
        health?.hudBounds = convert(card.frame, to: nil)
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        // A springing card is where it is drawn, not where it will rest
        guard desiredVisible, !card.isHidden, (card.layer?.presentation()?.frame ?? card.frame).contains(convert(point, from: superview))
        else { return nil }
        return card
    }

    private var snapping: Bool { card.layer?.animation(forKey: "snap.position.x") != nil || card.layer?.animation(forKey: "snap.position.y") != nil }

    /// Spring to the corner nearest where the released card would come to rest.
    private func settle(velocity: CGVector) {
        corner = FrameHealthCardFace.Corner(nearest: card.frame.origin, velocity: velocity, size: card.frame.size, in: bounds.insetBy(dx: 8, dy: 8))
        spring(velocity: velocity)
    }

    private func spring(velocity: CGVector) {
        guard let layer = card.layer else { return }
        let from = layer.position
        place(at: corner.origin(of: card.frame.size, in: bounds.insetBy(dx: 8, dy: 8)))
        let to = layer.position
        for (key, start, end, speed) in [("position.x", from.x, to.x, velocity.dx), ("position.y", from.y, to.y, velocity.dy)] where start != end {
            let spring = CASpringAnimation(keyPath: key)
            (spring.fromValue, spring.toValue) = (start, end)
            (spring.stiffness, spring.damping) = (FrameHealthCardFace.spring.stiffness, FrameHealthCardFace.spring.damping)
            // Core Animation takes the initial velocity as a fraction of the distance a second
            spring.initialVelocity = speed / (end - start)
            spring.duration = spring.settlingDuration
            layer.add(spring, forKey: "snap." + key)
        }
    }

    /// After a press that did not drag, as a click that stopped the spring, the card returns to its corner.
    private func rest() {
        guard !snapping, card.frame.origin != corner.origin(of: card.frame.size, in: bounds.insetBy(dx: 8, dy: 8)) else { return }
        spring(velocity: .zero)
    }

    /// A press stops the spring where the card is drawn.
    private func stopSnap() {
        guard snapping, let layer = card.layer, let shown = layer.presentation() else { return }
        let origin = CGPoint(x: card.frame.minX + shown.position.x - layer.position.x, y: card.frame.minY + shown.position.y - layer.position.y)
        layer.removeAnimation(forKey: "snap.position.x")
        layer.removeAnimation(forKey: "snap.position.y")
        place(at: origin)
    }

    private func move(toward edge: CGVector) -> Bool {
        stopSnap()
        if edge.dx != 0 { corner.maxX = edge.dx > 0 }
        if edge.dy != 0 { corner.maxY = edge.dy > 0 }
        spring(velocity: .zero)
        return true
    }
}

/// The card: one opaque view that draws its face and its charts or table by hand, and turns
/// clicks into presses, taps and drags.
final class FrameHealthCardViewMac: NSView {
    var state = FrameHealthCardFace.State()
    var compact = false { didSet { if compact != oldValue { needsDisplay = true; updateAccessibility(); updatePlots() } } }
    /// The charts, fed by each frame while the card reads, and the scales and view their labels were
    /// last drawn for
    private(set) lazy var chart = FrameHealthChart(in: layer!)
    private var chartLabels: [Double?] = []
    private(set) var expanded: Int?
    private(set) var drag: FrameHealthCardFace.Drag?
    private(set) var presentation = FrameHealthCardPresentation()
    private(set) var palette: FrameHealthCardDrawing.Palette!
    private var copiedUntil: DispatchWorkItem?
    private var elements: [Element] = []
    var grabbed: (() -> Void)?
    var moved: ((CGPoint) -> Void)?
    /// At the end of a press: with the release velocity in points a second after a drag, or nil
    var released: ((CGVector?) -> Void)?
    var activate: ((FrameHealthCardFace.Control) -> Void)?
    var expandedChanged: (() -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        wantsLayer = true
        // The drawing fills the bounds; the layer clips it to the card's rounded outline
        layer?.cornerRadius = 12
        layer?.masksToBounds = true
        layer?.borderWidth = 1
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        setAccessibilityLabel("Frame health")
        elements = [Element(.close, card: self), Element(.title, card: self), Element(.status, card: self), Element(.copy, card: self),
            Element(.charts, card: self), Element(.table, card: self), Element(.rateChart, card: self),
            Element(.timeChart, card: self), Element(.tableView, card: self)]
        resolveColors()
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override var isFlipped: Bool { true }
    override var isOpaque: Bool { true }
    private var regions: FrameHealthCardFace.Layout { .init(width: bounds.width, compact: compact) }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        resolveColors()
    }

    override func layout() {
        super.layout()
        updateAccessibility()
        updatePlots()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updatePlots(force: true)
    }

    private func resolveColors() {
        effectiveAppearance.performAsCurrentDrawingAppearance {
            let dark = effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            palette = .init(background: NSColor.textBackgroundColor.cgColor, label: NSColor.labelColor.cgColor,
                secondaryLabel: NSColor.secondaryLabelColor.cgColor, separator: NSColor.separatorColor.cgColor,
                fill: NSColor.systemFill.cgColor, secondaryFill: NSColor.secondarySystemFill.cgColor,
                tertiaryFill: NSColor.tertiarySystemFill.cgColor, quaternaryFill: NSColor.quaternarySystemFill.cgColor,
                blue: NSColor.systemBlue.cgColor, orange: NSColor.systemOrange.cgColor, green: NSColor.systemGreen.cgColor,
                gray: NSColor.systemGray.cgColor, dark: dark)
        }
        layer?.backgroundColor = palette.background
        layer?.borderColor = palette.separator
        needsDisplay = true
        updatePlots(force: true)
    }

    /// Show a presentation.
    func update(_ next: FrameHealthCardPresentation) {
        let header = next.status != presentation.status
        presentation = next
        next.apply(to: &state)
        setAccessibilityValue([state.request.0, state.request.1, state.display].filter { !$0.isEmpty }.joined(separator: ", ")
            + ". " + state.rows.map { "\($0.0): \($0.1) FPS, \($0.2) missed" }.joined(separator: ". ")
            + ". Pacing, \(state.pacingWindow): " + [state.pacing.0, state.pacing.1].filter { !$0.isEmpty }.joined(separator: ", "))
        updateAccessibility()
        // The table changes with each sample; the charts' labels only with their scales
        let labels = [next.input?.rateTop, next.input?.timeTop, state.table ? 1 : 0]
        if header { needsDisplay = true } else {
            setNeedsDisplay(regions.figures)
            if !compact, state.table || labels != chartLabels { setNeedsDisplay(regions.chartArea) }
        }
        chartLabels = labels
        updatePlots()
    }

    /// Place the shown plots' layers; a change of place, scale or palette draws them whole.
    private func updatePlots(force: Bool = false) {
        guard let palette else { return }
        let shown = presentation.input.map { compact || state.table ? [] : FrameHealthCardDrawing.plots(regions.chartArea, $0, expanded: expanded) } ?? []
        chart.configure(shown, scale: 1, contentsScale: window?.backingScaleFactor ?? 2, palette: palette, force: force)
    }

    /// Release the samples the drawing holds, when the reader stops.
    func clear() { update(FrameHealthCardPresentation()) }

    func showCopied(_ message: String) {
        state.copied = true
        NSAccessibility.post(element: self, notification: .announcementRequested, userInfo: [.announcement: message])
        needsDisplay = true
        copiedUntil?.cancel()
        let reset = DispatchWorkItem { [weak self] in self?.state.copied = false; self?.needsDisplay = true }
        copiedUntil = reset
        DispatchQueue.main.asyncAfter(deadline: .now() + 2, execute: reset)
    }

    /// Expand a chart to the whole chart area, or return it: 0 frame rate, 1 frame time.
    func toggle(chart: Int) {
        expanded = expanded != nil ? nil : chart
        updateAccessibility()
        needsDisplay = true
        updatePlots()
        expandedChanged?()
    }

    func showDefinitions() {
        let text = NSTextView(frame: CGRect(x: 0, y: 0, width: 420, height: 10))
        text.isEditable = false
        text.textContainerInset = NSSize(width: 12, height: 12)
        let result = NSMutableAttributedString()
        for (term, definition) in FrameHealthCardText.definitions {
            result.append(NSAttributedString(string: term + "\n", attributes: [.font: NSFont.boldSystemFont(ofSize: 13),
                .foregroundColor: NSColor.labelColor]))
            result.append(NSAttributedString(string: definition + "\n\n", attributes: [.font: NSFont.systemFont(ofSize: 13),
                .foregroundColor: NSColor.secondaryLabelColor]))
        }
        text.textStorage?.setAttributedString(result)
        let scroll = NSScrollView(frame: CGRect(x: 0, y: 0, width: 420, height: 480))
        scroll.hasVerticalScroller = true
        scroll.documentView = text
        text.autoresizingMask = [.width]
        let controller = NSViewController()
        controller.view = scroll
        let popover = NSPopover()
        popover.contentViewController = controller
        popover.behavior = .transient
        popover.show(relativeTo: regions.title, of: self, preferredEdge: .maxY)
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext, let palette else { return }
        ctx.setFillColor(palette.background)
        ctx.fill(bounds)
        FrameHealthCardFace.draw(ctx, width: bounds.width, compact: compact, state: state, palette: palette)
        guard !compact else { return }
        let area = regions.chartArea, p = presentation
        if state.table {
            FrameHealthCardFace.table(ctx, area, columns: p.columns, rows: p.table, caption: p.caption, palette: palette)
        } else if let input = p.input {
            FrameHealthCardDrawing.charts(ctx, area, input, expanded: expanded, palette: palette)
        }
    }

    // MARK: Clicks: a press anywhere drags once it moves 4 points; one that does not move is a click,
    // which presses a button, or on a chart expands it or returns it.

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func mouseDown(with event: NSEvent) {
        grabbed?()
        drag = .init(at: event.locationInWindow, time: event.timestamp, origin: frame.origin,
            control: regions.control(at: convert(event.locationInWindow, from: nil)))
        press(drag?.control)
    }

    override func mouseDragged(with event: NSEvent) {
        guard var current = drag else { return }
        current.track(event.locationInWindow, at: event.timestamp)
        let dx = event.locationInWindow.x - current.start.x, dy = event.locationInWindow.y - current.start.y
        defer { drag = current }
        guard current.moved || hypot(dx, dy) >= 4 else {
            if let control = current.control { press(regions.control(at: convert(event.locationInWindow, from: nil)) == control ? control : nil) }
            return
        }
        current.moved = true
        press(nil)
        moved?(CGPoint(x: current.origin.x + dx, y: current.origin.y + dy))
    }

    override func mouseUp(with event: NSEvent) {
        guard var current = drag else { return }
        drag = nil
        press(nil)
        current.track(event.locationInWindow, at: event.timestamp)
        // Where the release lands on the card, before a resumed spring moves it
        let point = convert(event.locationInWindow, from: nil)
        released?(current.moved ? current.velocity(releasedAt: event.timestamp) : nil)
        if current.moved { return }
        if let control = current.control {
            if regions.control(at: point) == control { activate?(control) }
        } else if !compact, !state.table, regions.chartArea.contains(point) {
            toggle(chart: point.y - regions.chartArea.minY < 88 ? 0 : 1)
        }
    }

    func cancelDrag() {
        guard let current = drag else { return }
        drag = nil
        press(nil)
        released?(current.moved ? .zero : nil)
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: bounds, options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow],
            owner: self))
    }
    override func mouseMoved(with event: NSEvent) { hover(regions.control(at: convert(event.locationInWindow, from: nil))) }
    override func mouseExited(with event: NSEvent) { hover(nil) }

    private func hover(_ control: FrameHealthCardFace.Control?) {
        guard state.hovered != control else { return }
        state.hovered = control
        setNeedsDisplay(CGRect(x: 0, y: 0, width: bounds.width, height: 206))
    }

    private func press(_ control: FrameHealthCardFace.Control?) {
        guard state.pressed != control else { return }
        state.pressed = control
        setNeedsDisplay(CGRect(x: 0, y: 0, width: bounds.width, height: 206))
    }

    /// An accessibility element over a region the card draws: its buttons, charts and table.
    private final class Element: NSAccessibilityElement {
        enum Role { case close, title, status, copy, charts, table, rateChart, timeChart, tableView }
        let role: Role
        weak var card: FrameHealthCardViewMac?
        init(_ role: Role, card: FrameHealthCardViewMac) {
            self.role = role
            self.card = card
            super.init()
            setAccessibilityParent(card)
        }
        override func accessibilityPerformPress() -> Bool {
            guard let card else { return false }
            switch role {
            case .close: card.activate?(.close)
            case .title: card.activate?(.title)
            case .status: card.activate?(.status)
            case .copy: card.activate?(.copy)
            case .charts: card.activate?(.charts)
            case .table: card.activate?(.table)
            case .rateChart: card.toggle(chart: 0)
            case .timeChart: card.toggle(chart: 1)
            case .tableView: return false
            }
            return true
        }
    }

    private func updateAccessibility() {
        let l = regions, area = l.chartArea, p = presentation
        var shown: [Element] = []
        for element in elements {
            var frame = CGRect.zero, label = "", role = NSAccessibility.Role.button, hidden = false
            switch element.role {
            case .status: frame = l.status; label = state.status == .paused ? "Resume frame health" : "Pause frame health"
            case .close: frame = l.close; label = "Close frame health"
            case .title: frame = l.title; label = "Frame health"; element.setAccessibilityHelp("Shows what these numbers mean.")
            case .copy: frame = l.copy; label = "Copy a picture and a text summary"
            case .charts, .table:
                frame = element.role == .charts ? l.segments.0 : l.segments.1
                label = element.role == .charts ? "Charts" : "Table"
                element.setAccessibilitySelected((element.role == .table) == state.table)
                hidden = compact
            case .rateChart, .timeChart:
                let rate = element.role == .rateChart
                hidden = compact || state.table || (expanded != nil && expanded != (rate ? 0 : 1))
                frame = expanded != nil ? area : rate ? CGRect(x: area.minX, y: area.minY, width: area.width, height: 80)
                    : CGRect(x: area.minX, y: area.minY + 96, width: area.width, height: 104)
                label = rate ? p.rateSummary : p.timeSummary
                role = .image
            case .tableView:
                hidden = compact || !state.table
                frame = area
                label = p.table.map { "\($0.0): " + zip(p.columns, $0.1).map { "\($1) (\($0))" }.joined(separator: ", ") }
                    .joined(separator: ". ") + ". " + p.caption
                role = .staticText
            }
            // In the card's own coordinates, which are flipped, as the parent space is
            element.setAccessibilityFrameInParentSpace(frame)
            element.setAccessibilityLabel(label)
            element.setAccessibilityRole(role)
            if !hidden { shown.append(element) }
        }
        setAccessibilityChildren(shown)
    }
}

extension ExactView {
    @objc func toggleFrameHealth(_ sender: Any?) {
        guard let container = session.frameHealth.macContainer else { return }
        let health = session.frameHealth
        let overlay: FrameHealthOverlayMac
        if let existing = health.desktopOverlay, existing.superview === container { overlay = existing }
        else {
            health.desktopOverlay?.removeFromSuperview()
            overlay = FrameHealthOverlayMac(health: health)
            overlay.frame = container.bounds
            overlay.autoresizingMask = [.width, .height]
            container.addSubview(overlay)
            health.desktopOverlay = overlay
            health.ended = { [weak overlay] in overlay?.setVisible(false); overlay?.removeFromSuperview() }
            overlay.showDetails = { [weak self] in self?.showFrameHealthDetails(nil) }
        }
        overlay.setVisible(!health.hudVisible)
    }
    func validateFrameHealthItem(_ item: NSMenuItem) -> Bool {
        item.title = session.frameHealth.hudVisible ? "Hide Frame Health" : "Show Frame Health"
        return session.frameHealth.macContainer != nil
    }
    @objc func showFrameHealthDetails(_ sender: Any?) { FrameHealthInspector.show(health: session.frameHealth, window: window) }
    @objc func shareFrameHealth(_ sender: Any?) {
        FrameHealthReport.prepare(health: session.frameHealth.reportSource, session: session) { [weak self] data in
            guard let self else { return }
            NSSharingServicePicker(items: [String(decoding: data, as: UTF8.self)]).show(relativeTo: self.visibleRect, of: self, preferredEdge: .minY)
        }
    }
}
#endif

#if os(iOS)
import UIKit
import CExact
import ObjectiveC

/// Host UI outside the app's node tree, below sheets and alerts. The card takes the touches within
/// its bounds, and touches outside it reach the app (LLP 1079.000 §7.2).
final class FrameHealthOverlay: UIView {
    fileprivate weak var health: FrameHealth?
    let card = FrameHealthCardView()
    private let reader = FrameHealthCardReader()
    /// The corner the card rests in, and its spring there after a drag
    private var corner = FrameHealthCardFace.Corner()
    private var snap: UIViewPropertyAnimator?
    private var paused = false
    /// While paused, the request and display the frozen samples were read under
    private var frozen: (FrameHealthCardText.Rates, FrameHealthCardText.Display)?
    /// The generation the charts last took frames from
    private var chartGeneration: UInt64?
    private var keyboard: CGRect?
    private var availableFrame = CGRect.zero
    private var notifications: [NSObjectProtocol] = []
    private(set) var desiredVisible = false
    private var dragging: Bool { card.drag?.moved == true }

    init(health: FrameHealth) {
        self.health = health
        super.init(frame: .zero)
        accessibilityIdentifier = "exact.host.frame-health-overlay"
        accessibilityViewIsModal = false
        backgroundColor = .clear
        isHidden = true
        addSubview(card)
        card.grabbed = { [weak self] in self?.stopSnap() }
        card.moved = { [weak self] origin in self?.move(to: origin) }
        card.released = { [weak self] velocity in
            guard let self else { return }
            self.health?.hudDragging = false
            if let velocity { settle(velocity: velocity); render() } else { rest() }
        }
        card.activate = { [weak self] control in self?.activate(control) }
        card.expandedChanged = { [weak self] in self?.render() }
        card.accessibilityMove = { [weak self] edge in self?.move(toward: edge) ?? false }
        // The card grows with the text size
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (overlay: FrameHealthOverlay, _: UITraitCollection) in
            overlay.setNeedsLayout()
        }
        health.ended = { [weak self] in self?.detach() }
        let center = NotificationCenter.default
        // Scenes change state independently, so only the card's own scene's transitions count
        let leaving = [UIScene.didEnterBackgroundNotification, UIScene.didDisconnectNotification]
        for name in leaving + [UIScene.willEnterForegroundNotification, UIScene.didActivateNotification] {
            notifications.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] notification in
                guard let self, let scene = notification.object as? UIScene, scene === self.window?.windowScene else { return }
                if leaving.contains(notification.name) { self.health?.stop(); self.card.cancelDrag() }
                self.refresh()
            })
        }
        notifications.append(center.addObserver(forName: UIResponder.keyboardWillChangeFrameNotification, object: nil, queue: .main) { [weak self] notification in
            guard let self, let frame = notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect else { return }
            self.keyboard = self.window.map { self.convert($0.convert(frame, from: nil), from: $0) }
            self.setNeedsLayout()
        })
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    deinit { notifications.forEach(NotificationCenter.default.removeObserver) }

    fileprivate func detach() {
        setVisible(false)
        notifications.forEach(NotificationCenter.default.removeObserver)
        notifications.removeAll()
        removeFromSuperview()
    }

    func setVisible(_ visible: Bool) {
        if !visible { card.cancelDrag() }
        desiredVisible = visible
        isHidden = !visible
        health?.setHUDVisible(visible)
        health?.changed = visible ? { [weak self] in self?.refresh() } : nil
        if visible { layoutIfNeeded() }
        refresh()
        if visible { setNeedsLayout() }
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        refresh()
    }

    /// Shown, fitting, and in a visible window of a scene on screen, with no hidden ancestor.
    private var canBeSeen: Bool {
        guard desiredVisible, !card.isHidden, let window, !window.isHidden,
              let scene = window.windowScene, scene.activationState != .background, scene.activationState != .unattached
        else { return false }
        var view: UIView? = self
        while let current = view {
            if current.isHidden || current.alpha < 0.01 { return false }
            view = current.superview
        }
        return true
    }

    /// Sample on each publication. A pause freezes the reader, so the face, accessibility and Share
    /// all describe the same samples; a drag only defers drawing.
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
        guard !dragging else { return }
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

    private var display: FrameHealthCardText.Display { frozen?.1 ?? Self.display(window?.screen ?? UIScreen.main) }

    private func render() {
        guard let health else { return }
        card.update(FrameHealthCardPresentation(reader: reader, rates: rates, display: display,
            status: paused ? .paused : health.snapshot().running != 0 ? .live : .idle,
            unavailable: health.enabled ? reader.reading ? nil : FrameHealthReaders.full : health.unavailableReason, health: health,
            details: card.state.showsDetails))
    }

    static func display(_ screen: UIScreen) -> FrameHealthCardText.Display {
        let maximum = Double(screen.maximumFramesPerSecond)
        return maximum > 60
            ? .init(name: "ProMotion", detail: "adaptive refresh up to \(Int(maximum)) Hz", maximumHz: maximum, adaptiveSync: false)
            : .init(name: "Fixed refresh", detail: "\(Int(maximum)) Hz", maximumHz: maximum, adaptiveSync: false)
    }

    private func activate(_ control: FrameHealthCardFace.Control) {
        switch control {
        case .status:
            frozen = paused ? nil : (rates, display)
            paused.toggle()
            feed(!paused)
            UIAccessibility.post(notification: .announcement, argument: paused ? "Paused" : "Resumed")
            // Resuming reads at once; the first sample covers the paused span and names it
            if paused { render() } else { refresh() }
        case .close: hideReadout()
        case .title: DevMenu.showFrameHealthDefinitions()
        case .details:
            card.showDetails(!card.state.showsDetails)
            render()
        case .share: chooseShare()
        case .charts, .table:
            card.state.table = control == .table
            render()
        }
    }

    /// What to share, as an action sheet from the Share button, which UIKit shows as a popover from it
    /// where there is room.
    private func chooseShare() {
        guard let presenter = ViewPresentation.presenter(for: card) else { return }
        let sheet = UIAlertController(title: nil, message: nil, preferredStyle: .actionSheet)
        for kind in FrameHealthShare.allCases {
            sheet.addAction(UIAlertAction(title: kind.title, style: .default) { [weak self] _ in self?.share(kind) })
        }
        sheet.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        sheet.popoverPresentationController?.sourceView = card
        sheet.popoverPresentationController?.sourceRect = card.shareFrame
        presenter.present(sheet, animated: !ExactEnv.agentFreezes)
    }

    /// The system's share sheet with one item: the report an agent reads, the summary, or the image.
    /// It leaves out Assign to Contact, which an image would offer.
    func share(_ kind: FrameHealthShare) {
        guard let health else { return }
        let present = { [weak self] (item: Any) in
            guard let self, let presenter = ViewPresentation.presenter(for: card) else { return }
            let sheet = UIActivityViewController(activityItems: [item], applicationActivities: nil)
            sheet.excludedActivityTypes = [.assignToContact]
            sheet.popoverPresentationController?.sourceView = card
            sheet.popoverPresentationController?.sourceRect = card.shareFrame
            presenter.present(sheet, animated: !ExactEnv.agentFreezes)
        }
        switch kind {
        case .report:
            FrameHealthReport.prepare(health: health.reportSource, session: DevMenu.session) { present(String(decoding: $0, as: UTF8.self)) }
        case .summary: present(FrameHealthCardText.summary(reader: reader, rates: rates, display: display, paused: paused, health: health).text)
        case .image:
            let picture = FrameHealthCardText.picture(reader: reader, chart: card.chart, rates: rates, display: display, paused: paused,
                health: health, palette: card.palette)
            if let image = picture.flatMap(UIImage.init(data:)) { present(image) }
        }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        // Corners 8 points from the window's edges, past its safe area, and above any keyboard
        var available = bounds.insetBy(dx: 8, dy: 8)
        if let keyboard, keyboard.intersects(available), keyboard.maxY >= available.maxY {
            available.size.height = max(0, keyboard.minY - 8 - available.minY)
        }
        availableFrame = available
        // The card grows with the text size, as far as the window allows; where the whole card does
        // not fit, it leaves out its toolbar and chart area
        let text = max(1, UIFontMetrics(forTextStyle: .footnote).scaledValue(for: 1, compatibleWith: traitCollection))
        func scale(_ height: CGFloat) -> CGFloat? {
            let fitting = min(text, available.width / FrameHealthCardFace.minimumWidth, available.height / height)
            return fitting >= 1 ? fitting : nil
        }
        let full = scale(FrameHealthCardFace.height), compact = full == nil ? scale(FrameHealthCardFace.compactHeight) : nil
        let wasHidden = card.isHidden
        card.isHidden = full == nil && compact == nil
        card.compact = full == nil
        card.scale = full ?? compact ?? 1
        health?.hudEffectiveVisible = canBeSeen
        let size = CGSize(width: min(FrameHealthCardFace.width, available.width / card.scale) * card.scale,
            height: (full == nil ? FrameHealthCardFace.compactHeight : FrameHealthCardFace.height) * card.scale)
        if card.frame.size != size { card.frame.size = size; card.setNeedsDisplay() }
        // A dragged card stays under the press; a spring toward a corner the new geometry moved turns
        // toward where it is now
        let target = corner.origin(of: size, in: available)
        if !dragging, snap == nil { card.frame.origin = target }
        else if !dragging, card.frame.origin != target { stopSnap(); spring(velocity: .zero) }
        health?.hudBounds = window.map { card.convert(card.bounds, to: $0) }
        if card.isHidden != wasHidden { refresh() }
    }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        // A springing card is where it is drawn, not where it will rest
        guard !isHidden, !card.isHidden, (card.layer.presentation()?.frame ?? card.frame).contains(point) else { return nil }
        // Details rows that scroll take the touches on them; everywhere else the card takes them, to drag or press
        let scroll = card.detailsScroll
        return !scroll.isHidden && scroll.isScrollEnabled && scroll.frame.contains(convert(point, to: card)) ? scroll : card
    }

    private func move(to origin: CGPoint) {
        health?.hudDragging = true
        UIView.performWithoutAnimation {
            card.frame.origin = FrameHealthCardFace.resisted(origin, size: card.frame.size, in: availableFrame)
            health?.hudBounds = window.map { card.convert(card.bounds, to: $0) }
        }
    }

    /// Spring to the corner nearest where the released card would come to rest.
    private func settle(velocity: CGVector) {
        corner = FrameHealthCardFace.Corner(nearest: card.frame.origin, velocity: velocity, size: card.frame.size, in: availableFrame)
        spring(velocity: velocity)
    }

    private func spring(velocity: CGVector) {
        let target = corner.origin(of: card.frame.size, in: availableFrame)
        // UIKit takes a spring's initial velocity as a fraction of the distance a second
        let dx = target.x - card.frame.minX, dy = target.y - card.frame.minY
        let initial = CGVector(dx: dx == 0 ? 0 : velocity.dx / dx, dy: dy == 0 ? 0 : velocity.dy / dy)
        let spring = FrameHealthCardFace.spring
        let animator = UIViewPropertyAnimator(duration: 0.5, timingParameters: UISpringTimingParameters(mass: 1,
            stiffness: spring.stiffness, damping: spring.damping, initialVelocity: initial))
        animator.addAnimations { self.card.frame.origin = target }
        animator.addCompletion { [weak self] _ in self?.snap = nil; self?.setNeedsLayout() }
        snap = animator
        animator.startAnimation()
        health?.hudBounds = window.map { convert(CGRect(origin: target, size: card.frame.size), to: $0) }
    }

    /// A press stops the spring where the card is drawn.
    private func stopSnap() {
        snap?.stopAnimation(true)
        snap = nil
    }

    /// After a press that did not drag, as a tap that stopped the spring, the card returns to its corner.
    private func rest() {
        guard snap == nil, card.frame.origin != corner.origin(of: card.frame.size, in: availableFrame) else { return }
        spring(velocity: .zero)
    }

    private func move(toward edge: CGVector) -> Bool {
        stopSnap()
        if edge.dx != 0 { corner.maxX = edge.dx > 0 }
        if edge.dy != 0 { corner.maxY = edge.dy > 0 }
        spring(velocity: .zero)
        return true
    }

    @objc @discardableResult func hideReadout() -> Bool { setVisible(false); return true }
}

/// The card: one opaque view that draws its face and its charts or table by hand, and turns
/// touches into presses, taps and drags. It draws at `scale`, the text size's, in the face's points.
final class FrameHealthCardView: UIView {
    var state = FrameHealthCardFace.State()
    var compact = false { didSet { if compact != oldValue { setNeedsDisplay(); updateAccessibility(); updatePlots() } } }
    var scale: CGFloat = 1 { didSet { if scale != oldValue { setNeedsDisplay(); updateAccessibility(); updatePlots() } } }
    /// The charts, fed by each frame while the card reads, and the scales and view their labels were
    /// last drawn for
    private(set) lazy var chart = FrameHealthChart(in: layer)
    private var chartLabels: [Double?] = []
    /// The chart filling the chart area after a tap: 0 frame rate, 1 frame time
    private(set) var expanded: Int?
    private(set) var drag: FrameHealthCardFace.Drag?
    private(set) var presentation = FrameHealthCardPresentation()
    private(set) var palette: FrameHealthCardDrawing.Palette!
    var grabbed: (() -> Void)?
    var moved: ((CGPoint) -> Void)?
    /// At the end of a press: with the release velocity in points a second after a drag, or nil
    var released: ((CGVector?) -> Void)?
    var activate: ((FrameHealthCardFace.Control) -> Void)?
    var expandedChanged: (() -> Void)?
    /// Toward an edge: a negative or positive component, or zero to keep that axis
    var accessibilityMove: ((CGVector) -> Bool)?
    private var elements: [Element] = []
    /// Details in place of everything below the header, its rows drawn on one opaque view that scrolls
    /// where they outgrow the area; it shows no scroll indicator or edge effect, which would blend, and
    /// takes no safe-area inset, since the card can rest past the safe area
    let detailsScroll = UIScrollView()
    private let detailsView = DetailsView()

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = true
        detailsScroll.isHidden = true
        detailsScroll.showsVerticalScrollIndicator = false
        detailsScroll.showsHorizontalScrollIndicator = false
        detailsScroll.contentInsetAdjustmentBehavior = .never
        if #available(iOS 26.0, *) {
            for edge in [detailsScroll.topEdgeEffect, detailsScroll.leftEdgeEffect, detailsScroll.bottomEdgeEffect, detailsScroll.rightEdgeEffect] {
                edge.isHidden = true
            }
        }
        detailsScroll.isOpaque = true
        detailsScroll.addSubview(detailsView)
        addSubview(detailsScroll)
        // The drawing fills the bounds; the layer clips it to the card's rounded outline, which in a
        // corner of the screen follows the screen's own rounding, so that rounding never cuts the card
        if #available(iOS 26.0, *) {
            cornerConfiguration = .corners(radius: .containerConcentric(minimum: 12))
        } else {
            layer.cornerRadius = 12
            layer.cornerCurve = .continuous
        }
        layer.masksToBounds = true
        isMultipleTouchEnabled = false
        resolveColors()
        // Colors resolve again as soon as the appearance changes: UIKit updates a view's traits lazily,
        // so without this the card would keep its colors until something laid it out
        registerForTraitChanges(UITraitCollection.systemTraitsAffectingColorAppearance + [UITraitDisplayScale.self]) {
            (card: FrameHealthCardView, _: UITraitCollection) in card.resolveColors()
        }
        addGestureRecognizer(UIHoverGestureRecognizer(target: self, action: #selector(hover(_:))))
        elements = Element.all(self)
        accessibilityElements = elements
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    private var layout: FrameHealthCardFace.Layout { .init(width: bounds.width / scale, compact: compact, showsDetails: state.showsDetails) }
    private func face(_ point: CGPoint) -> CGPoint { CGPoint(x: point.x / scale, y: point.y / scale) }
    private func view(_ rect: CGRect) -> CGRect { rect.applying(CGAffineTransform(scaleX: scale, y: scale)) }

    override func layoutSubviews() {
        super.layoutSubviews()
        layoutDetails()
        updateAccessibility()
        updatePlots()
    }

    /// Show Details in place of everything below the header, from its first row, or the charts or table again.
    func showDetails(_ shown: Bool) {
        state.showsDetails = shown
        detailsScroll.isHidden = !shown
        detailsScroll.contentOffset = .zero
        layoutDetails()
        setNeedsDisplay()
        updateAccessibility()
        updatePlots()
    }

    private func layoutDetails() {
        guard state.showsDetails, let palette else { return }
        let l = layout, area = view(l.detailsArea)
        let height = FrameHealthCardFace.detailsHeight(width: l.width, rows: presentation.details, caption: presentation.detailsCaption) * scale
        detailsScroll.frame = area
        detailsScroll.backgroundColor = UIColor(cgColor: palette.background)
        detailsScroll.contentSize = CGSize(width: area.width, height: max(area.height, height))
        detailsScroll.isScrollEnabled = height > area.height
        detailsView.frame = CGRect(origin: .zero, size: detailsScroll.contentSize)
        detailsView.set(rows: presentation.details, caption: presentation.detailsCaption, width: l.width, scale: scale, palette: palette)
    }

    func resolveColors() {
        let t = traitCollection
        func c(_ color: UIColor) -> CGColor { color.resolvedColor(with: t).cgColor }
        palette = .init(background: c(.systemBackground), label: c(.label), secondaryLabel: c(.secondaryLabel),
            separator: c(.separator), fill: c(.systemFill), secondaryFill: c(.secondarySystemFill),
            tertiaryFill: c(.tertiarySystemFill), quaternaryFill: c(.quaternarySystemFill), blue: c(.systemBlue),
            orange: c(.systemOrange), green: c(.systemGreen), gray: c(.systemGray), dark: t.userInterfaceStyle == .dark)
        backgroundColor = UIColor(cgColor: palette.background)
        layer.borderWidth = 1 / max(1, t.displayScale)
        layer.borderColor = palette.separator
        setNeedsDisplay()
        layoutDetails()
        updatePlots(force: true)
    }

    /// Show a presentation; only the regions it can change are drawn again.
    func update(_ next: FrameHealthCardPresentation) {
        let header = next.status != presentation.status
        presentation = next
        next.apply(to: &state)
        updateAccessibility()
        // The table changes with each sample; the charts' labels only with their scales
        let labels = [next.input?.rateTop, next.input?.timeTop, state.table ? 1 : 0]
        // Under Details only the header shows, and Details draws its own rows
        if header { setNeedsDisplay() } else if !state.showsDetails {
            setNeedsDisplay(view(layout.figures))
            if !compact, state.table || labels != chartLabels { setNeedsDisplay(view(layout.chartArea)) }
        }
        chartLabels = labels
        layoutDetails()
        updatePlots()
    }

    /// Place the shown plots' layers; a change of place, scale or palette draws them whole.
    private func updatePlots(force: Bool = false) {
        guard let palette else { return }
        let shown = presentation.input.map { compact || state.table || state.showsDetails ? [] : FrameHealthCardDrawing.plots(layout.chartArea, $0, expanded: expanded) } ?? []
        chart.configure(shown, scale: scale, contentsScale: traitCollection.displayScale, palette: palette, force: force)
    }

    /// Release the samples the drawing holds, when the reader stops.
    func clear() { update(FrameHealthCardPresentation()) }

    /// Where Share is drawn, for the share sheet's popover
    var shareFrame: CGRect { view(layout.share) }

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext(), let palette else { return }
        ctx.setFillColor(palette.background)
        ctx.fill(bounds)
        ctx.scaleBy(x: scale, y: scale)
        FrameHealthCardFace.draw(ctx, width: layout.width, compact: compact, state: state, palette: palette)
        guard !compact, !state.showsDetails else { return }
        let area = layout.chartArea, p = presentation
        if state.table {
            FrameHealthCardFace.table(ctx, area, columns: p.columns, rows: p.table, caption: p.caption, palette: palette)
        } else if let input = p.input {
            FrameHealthCardDrawing.charts(ctx, area, input, expanded: expanded, palette: palette)
        }
    }

    /// Expand a chart to the whole chart area, or return it: 0 frame rate, 1 frame time.
    func toggle(chart: Int) {
        expanded = expanded != nil ? nil : chart
        updateAccessibility()
        setNeedsDisplay(view(layout.chartArea))
        updatePlots()
        expandedChanged?()
    }

    // MARK: Touches: a press anywhere drags once it moves 4 points; one that does not move is a tap,
    // which presses a button, or on a chart expands it or returns it.

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let touch = touches.first else { return }
        grabbed?()
        drag = .init(at: touch.location(in: superview), time: touch.timestamp, origin: frame.origin,
            control: layout.control(at: face(touch.location(in: self))))
        press(drag?.control)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let touch = touches.first, var current = drag else { return }
        let at = touch.location(in: superview)
        current.track(at, at: touch.timestamp)
        let dx = at.x - current.start.x, dy = at.y - current.start.y
        defer { drag = current }
        guard current.moved || hypot(dx, dy) >= 4 else {
            if let control = current.control { press(layout.control(at: face(touch.location(in: self))) == control ? control : nil) }
            return
        }
        current.moved = true
        press(nil)
        moved?(CGPoint(x: current.origin.x + dx, y: current.origin.y + dy))
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let touch = touches.first, var current = drag else { return }
        drag = nil
        press(nil)
        current.track(touch.location(in: superview), at: touch.timestamp)
        // Where the release lands on the card, before a resumed spring moves it
        let point = face(touch.location(in: self))
        released?(current.moved ? current.velocity(releasedAt: touch.timestamp) : nil)
        if current.moved { return }
        if let control = current.control {
            if layout.control(at: point) == control { activate?(control) }
        } else if !compact, !state.table, !state.showsDetails, layout.chartArea.contains(point) {
            // The frame-rate chart is the top 80 points with half the gap below it
            toggle(chart: point.y - layout.chartArea.minY < 88 ? 0 : 1)
        }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { cancelDrag() }

    func cancelDrag() {
        guard let current = drag else { return }
        drag = nil
        press(nil)
        released?(current.moved ? .zero : nil)
    }

    private func press(_ control: FrameHealthCardFace.Control?) {
        guard state.pressed != control else { return }
        state.pressed = control
        setNeedsDisplay(view(CGRect(x: 0, y: 0, width: layout.width, height: 206)))
    }

    @objc private func hover(_ gesture: UIHoverGestureRecognizer) {
        let control = gesture.state == .ended || gesture.state == .cancelled ? nil : layout.control(at: face(gesture.location(in: self)))
        guard state.hovered != control else { return }
        state.hovered = control
        setNeedsDisplay(view(CGRect(x: 0, y: 0, width: layout.width, height: 206)))
    }

    // MARK: Accessibility: an element for each region the card draws.

    fileprivate final class Element: UIAccessibilityElement {
        enum Role { case close, title, status, details, share, context, figures, pacing, charts, table, rateChart, timeChart, tableView, detailsView }
        let role: Role
        weak var card: FrameHealthCardView?
        init(_ role: Role, card: FrameHealthCardView) {
            self.role = role
            self.card = card
            super.init(accessibilityContainer: card)
        }
        static func all(_ card: FrameHealthCardView) -> [Element] {
            [.close, .title, .status, .details, .share, .context, .figures, .pacing, .charts, .table, .rateChart, .timeChart, .tableView,
             .detailsView].map { Element($0, card: card) }
        }
        override func accessibilityActivate() -> Bool {
            guard let card else { return false }
            switch role {
            case .close: card.activate?(.close)
            case .title: card.activate?(.title)
            case .status: card.activate?(.status)
            case .details: card.activate?(.details)
            case .share: card.activate?(.share)
            case .charts: card.activate?(.charts)
            case .table: card.activate?(.table)
            case .rateChart: card.toggle(chart: 0)
            case .timeChart: card.toggle(chart: 1)
            default: return false
            }
            return true
        }
    }

    private func updateAccessibility() {
        let l = layout, chartArea = l.chartArea, p = presentation
        for element in elements {
            var frame = CGRect.zero, label = "", traits: UIAccessibilityTraits = .staticText, hidden = false
            switch element.role {
            case .close: frame = l.close; label = "Close frame health"; traits = .button
            case .title:
                frame = l.title; label = "Frame health"; traits = [.header, .button]
                element.accessibilityHint = "Shows what these numbers mean. Actions move the card to an edge."
                element.accessibilityCustomActions = [("Move up", CGVector(dx: 0, dy: -1)), ("Move down", CGVector(dx: 0, dy: 1)),
                    ("Move left", CGVector(dx: -1, dy: 0)), ("Move right", CGVector(dx: 1, dy: 0))].map { name, edge in
                    UIAccessibilityCustomAction(name: name) { [weak self] _ in self?.accessibilityMove?(edge) ?? false }
                }
            case .status:
                frame = l.status; traits = .button
                label = state.status == .paused ? "Resume frame health" : "Pause frame health"
                element.accessibilityValue = state.status == .paused ? "Paused" : state.status == .live ? "Live" : "Idle"
            case .details: frame = l.details; label = "Frame health details"; traits = state.showsDetails ? [.button, .selected] : .button
            case .share: frame = l.share; label = "Share frame health"; traits = .button
            case .context:
                frame = CGRect(x: 0, y: 44, width: l.width, height: 36)
                label = [state.request.0, state.request.1, state.display].filter { !$0.isEmpty }.joined(separator: ", ")
            case .figures:
                frame = CGRect(x: 0, y: 80, width: l.width, height: 45)
                label = state.rows.map { "\($0.0): \($0.1) FPS, \($0.2) missed" }.joined(separator: ". ")
            case .pacing:
                frame = CGRect(x: 0, y: 125, width: l.width, height: 41)
                label = "Pacing, \(state.pacingWindow): " + [state.pacing.0, state.pacing.1].filter { !$0.isEmpty }.joined(separator: ", ")
            case .charts, .table:
                let chosen = (element.role == .table) == state.table
                frame = element.role == .charts ? l.segments.0 : l.segments.1
                label = element.role == .charts ? "Charts" : "Table"
                traits = chosen ? [.button, .selected] : .button
                hidden = compact
            case .rateChart, .timeChart:
                let rate = element.role == .rateChart
                hidden = compact || state.table || (expanded != nil && expanded != (rate ? 0 : 1))
                frame = expanded != nil ? chartArea : rate ? CGRect(x: chartArea.minX, y: chartArea.minY, width: chartArea.width, height: 80)
                    : CGRect(x: chartArea.minX, y: chartArea.minY + 96, width: chartArea.width, height: 104)
                label = rate ? p.rateSummary : p.timeSummary
                traits = .image
                element.accessibilityHint = expanded == nil ? "Activate to fill the chart area." : "Activate to show both charts."
            case .tableView:
                hidden = compact || !state.table
                frame = chartArea
                label = p.table.map { "\($0.0): " + zip(p.columns, $0.1).map { "\($1) (\($0))" }.joined(separator: ", ") }
                    .joined(separator: ". ") + ". " + p.caption
            case .detailsView:
                frame = l.detailsArea
                label = p.details.map { "\($0.name): \($0.value)" + ($0.reason.map { ", " + $0 } ?? "") }.joined(separator: ". ") + ". " + p.detailsCaption
            }
            // Details takes the place of everything below the header
            if state.showsDetails, ![.close, .title, .status, .details, .share, .detailsView].contains(element.role) { hidden = true }
            if element.role == .detailsView, !state.showsDetails { hidden = true }
            element.accessibilityFrameInContainerSpace = view(frame)
            element.accessibilityLabel = label
            element.accessibilityTraits = traits
            element.isAccessibilityElement = !hidden
        }
    }
}

extension FrameHealthCardView {
    /// Details' rows and caption on one opaque surface, at the card's scale.
    final class DetailsView: UIView {
        private var rows: [FrameHealthCardText.Detail] = [], caption = "", width: CGFloat = 0, scale: CGFloat = 1
        private var palette: FrameHealthCardDrawing.Palette?
        override init(frame: CGRect) { super.init(frame: frame); isOpaque = true }
        required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
        func set(rows: [FrameHealthCardText.Detail], caption: String, width: CGFloat, scale: CGFloat, palette: FrameHealthCardDrawing.Palette) {
            backgroundColor = UIColor(cgColor: palette.background)
            (self.rows, self.caption, self.width, self.scale, self.palette) = (rows, caption, width, scale, palette)
            setNeedsDisplay()
        }
        override func draw(_ rect: CGRect) {
            guard let ctx = UIGraphicsGetCurrentContext(), let palette else { return }
            ctx.scaleBy(x: scale, y: scale)
            FrameHealthCardFace.details(ctx, width: width, rows: rows, caption: caption, palette: palette)
        }
    }
}

final class FrameHealthDefinitionsController: UIViewController {
    override func viewDidLoad() {
        super.viewDidLoad()
        title = "What these numbers mean"
        view.backgroundColor = .systemGroupedBackground
        navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in
            self?.dismiss(animated: true)
        })
        let text = UITextView()
        text.isEditable = false
        text.backgroundColor = .clear
        text.adjustsFontForContentSizeCategory = true
        text.textContainerInset = UIEdgeInsets(top: 16, left: 12, bottom: 24, right: 12)
        let result = NSMutableAttributedString()
        for (term, definition) in FrameHealthCardText.definitions {
            result.append(NSAttributedString(string: term + "\n", attributes: [.font: UIFont.preferredFont(forTextStyle: .headline),
                .foregroundColor: UIColor.label]))
            result.append(NSAttributedString(string: definition + "\n\n", attributes: [.font: UIFont.preferredFont(forTextStyle: .body),
                .foregroundColor: UIColor.secondaryLabel]))
        }
        text.attributedText = result
        text.frame = view.bounds
        text.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(text)
    }
}

extension DevMenu {
    private static var overlayKey: UInt8 = 0

    static func frameHealthOverlay() -> FrameHealthOverlay? {
        guard let controller, let session, let container = frameHealthContainer, !(container is ExactView) else { return nil }
        if let existing = objc_getAssociatedObject(controller, &overlayKey) as? FrameHealthOverlay {
            if existing.health === session.frameHealth, existing.superview === container { return existing }
            existing.detach()
        }
        let overlay = FrameHealthOverlay(health: session.frameHealth)
        overlay.frame = container.bounds
        overlay.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        container.addSubview(overlay)
        objc_setAssociatedObject(controller, &overlayKey, overlay, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return overlay
    }

    static func showFrameHealthDefinitions() {
        let navigation = UINavigationController(rootViewController: FrameHealthDefinitionsController())
        navigation.modalPresentationStyle = .pageSheet
        navigation.sheetPresentationController?.detents = [.medium(), .large()]
        navigation.sheetPresentationController?.prefersGrabberVisible = true
        present(navigation)
    }
}
#endif

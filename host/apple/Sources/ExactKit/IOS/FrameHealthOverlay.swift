#if os(iOS)
import UIKit
import CExact
import ObjectiveC

/// Host UI outside the app's node tree, below sheets and alerts. The card takes the touches within
/// its bounds, and touches outside it reach the app (LLP 1079.000 §7.2).
final class FrameHealthOverlay: UIView {
    fileprivate weak var health: FrameHealth?
    let card = FrameHealthCardView()
    /// Exact's own frames, which the face shows where the screen's cannot be observed, and Details always
    private let reader = FrameHealthCardReader()
    /// The screen's frames and the main thread's work, which the face shows while the card reads on iOS 18 and later
    /// and a second reader's place is free, and their reader
    private var screen: FrameHealthScreen?
    private let screenReader = FrameHealthCardReader()
    /// False keeps the face on Exact's own frames, as where the screen cannot be observed
    private let measuresScreen: Bool
    /// The session whose app the card measures, whose workload the screen's frames record
    private weak var session: ExactSession?
    /// Whether the card shows Exact's frames for want of a second reader's place, until one is free
    private var waitingForPlace = false
    /// The card's resting position
    private var position = FrameHealthCardFace.Position()
    private var snap: UIViewPropertyAnimator?
    /// While the card follows a finger or springs to its edge, the `UIUpdateLink` that states the display's highest
    /// rates as its preferred range
    private var motion: AnyObject?
    /// Whether it asks for the display's highest rates now
    var movesFast: Bool { motion != nil }
    /// Paused, the card shows what it read at the pause, and its readers take no samples while the collectors go on
    /// measuring, so the first read after it covers the paused span
    private var paused = false
    /// While paused, the request, display and source the readers were read under
    private var frozen: (rates: FrameHealthCardText.Rates, display: FrameHealthCardText.Display, source: FrameHealthCardText.Source)?
    /// A change waits for the app's next frame, to go out with it
    private var needsShow = false
    /// While the card measures the screen, it reads twice a second in place of the collectors' publications, so its
    /// figures follow the time while the app is idle; paused or not, it stops reading once the card cannot be seen
    private var reading: Timer?
    private var keyboard: CGRect?
    private var availableFrame = CGRect.zero
    private var notifications: [NSObjectProtocol] = []
    private(set) var desiredVisible = false
    private var dragging: Bool { card.drag?.moved == true }

    init(health: FrameHealth, session: ExactSession? = nil, measuresScreen: Bool = true) {
        self.health = health
        self.session = session
        self.measuresScreen = measuresScreen
        super.init(frame: .zero)
        accessibilityIdentifier = "exact.host.frame-health-overlay"
        accessibilityViewIsModal = false
        backgroundColor = .clear
        isHidden = true
        addSubview(card)
        card.grabbed = { [weak self] in self?.stopSnap(); self?.fast(true) }
        card.moved = { [weak self] origin in self?.asCard { self?.move(to: origin) } }
        card.released = { [weak self] velocity in
            guard let self else { return }
            self.dragged(false)
            if let velocity { settle(velocity: velocity); render() } else { rest() }
            if snap == nil { fast(false) }
        }
        card.activate = { [weak self] control in self?.asCard { self?.activate(control) } }
        card.chart.moved = { [weak self] in self?.asCard { self?.showWaiting() } }
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
    deinit {
        notifications.forEach(NotificationCenter.default.removeObserver)
        reading?.invalidate()
    }

    fileprivate func detach() {
        setVisible(false)
        notifications.forEach(NotificationCenter.default.removeObserver)
        notifications.removeAll()
        removeFromSuperview()
    }

    func setVisible(_ visible: Bool) {
        if !visible { card.cancelDrag() }
        desiredVisible = visible
        // The card shows in a change of its own, which takes none of the app's frames (`alone`)
        CATransaction.begin()
        isHidden = !visible
        health?.setHUDVisible(visible)
        health?.changed = visible ? { [weak self] in self?.published() } : nil
        if visible { layoutIfNeeded() }
        refresh()
        if visible {
            setNeedsLayout()
            card.drawNow()
        }
        CATransaction.commit()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        // A new window may be another scene's, which the screen's frames then come from
        if reader.reading, let screen, screen.running, window?.windowScene !== screen.scene { stopReading() }
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

    /// A collector published: read it, unless the card reads twice a second anyway (`follow`).
    private func published() { if reading == nil { refresh() } }

    /// Sample, as a collector publishes or the card's state changes. A pause keeps what the card shows, so the face,
    /// accessibility and Share all describe the same samples; a drag only defers drawing.
    func refresh(now: Double = CACurrentMediaTime()) { asCard { read(now: now) } }

    private func read(now: Double) {
        guard let health else { return }
        health.hudEffectiveVisible = canBeSeen
        guard canBeSeen else {
            if reader.reading { stopReading() }
            return
        }
        // A second reader's place came free: the card measures the screen, from a fresh baseline
        if waitingForPlace, !paused, FrameHealthReaders.hasRoom { stopReading() }
        if !reader.reading {
            guard reader.start() else { render(); return }
            startReading(now)
        } else if !paused {
            sample(now: now)
        }
        guard !dragging else { return }
        show()
    }

    /// Read from no history, the face from the screen where it can be observed, and follow the time. The first read is a
    /// baseline; its maxima cover time the card did not watch.
    private func startReading(_ now: Double) {
        guard let health else { return }
        card.chart.start()
        if measuresScreen, health.enabled, !health.virtual, let scene = window?.windowScene {
            let screen = self.screen ?? FrameHealthScreen()
            self.screen = screen
            screen.chart = card.chart
            screen.willDraw = { [weak self] in self?.drawWithUpdate() }
            screen.workload = { [weak self] in self?.session.map { FrameHealth.workload($0.frames, session: $0) } ?? 0 }
            // The face and the plots draw as Core Animation commits: the card's work, not the app's
            let drawing = { [weak screen] (draw: () -> Void) in if let screen { screen.card(draw) } else { draw() } }
            (card.drawing, card.chart.drawing) = (drawing, drawing)
            screen.start(scene)
            waitingForPlace = screen.running && !screenReader.start()
            // Its recent late frames are what Share's report gives
            if screen.running, !waitingForPlace { screen.health.setHUDVisible(true) } else { screen.stop() }
        }
        // Beside the screen's, Exact's frames give only Details, over 5 seconds
        reader.keep(observing ? 5 : 30)
        // The screen's frames hold Exact's, so the charts take those only without them; measuring the screen, the charts
        // glide with the time, which takes no update of the app's
        health.chart = observing ? nil : card.chart
        card.chart.glides = observing
        sample(now: now, force: true)
        follow(true)
    }

    /// Follow the time as the card starts reading, or stop as it hides. Measuring the screen, the charts glide on their
    /// own, and the card reads twice a second and draws what changed in one change of its own. Otherwise the charts
    /// follow the time on their own clock, which a pause stops.
    private func follow(_ on: Bool) {
        reading?.invalidate()
        reading = nil
        card.chart.follow(on && !observing)
        guard on, observing else { return }
        let timer = Timer(timeInterval: 0.5, repeats: true) { [weak self] _ in
            guard let self else { return }
            // Hidden though nothing of its own changed, as under a hidden ancestor, the card stops reading
            guard self.canBeSeen else { self.refresh(); return }
            guard !self.paused else { return }
            asCard {
                let now = CACurrentMediaTime()
                if self.sample(now: now) { self.needsShow = true }
                // While the app draws, the charts' changes wait for its next frame too (`drawWithUpdate`)
                guard self.screen?.drawing != true, !self.dragging else { return }
                self.alone {
                    self.card.chart.tick(at: now)
                    self.showWaiting()
                    self.card.chart.present()
                }
            }
        }
        timer.tolerance = 0.1
        RunLoop.main.add(timer, forMode: .common)
        reading = timer
    }

    /// Stop reading and release the history, as the card hides; it shows again unpaused, from a fresh baseline.
    private func stopReading() {
        fast(false)
        screen?.stop()
        screen?.health.setHUDVisible(false)
        follow(false)
        reader.stop()
        screenReader.stop()
        health?.chart = nil
        card.clear()
        card.chart.stop()
        (paused, frozen, needsShow, waitingForPlace) = (false, nil, false, false)
    }

    /// Whether the card measures the screen now, its face showing the screen's frames
    var observing: Bool { screen?.running == true }

    /// The screen's frames since the card started reading
    var screenFrames: UInt64 { screen?.health.snapshot().callbacks ?? 0 }

    /// Read the collectors at most twice a second; a stopped Exact source changes only at lifecycle boundaries, so every
    /// one is read, unless the card reads twice a second anyway.
    @discardableResult
    private func sample(now: Double, force: Bool = false) -> Bool {
        guard let health else { return false }
        var read = reader.sample(health, now: now, force: force || reading == nil && health.snapshot().running == 0)
        if let screen, screen.running {
            read = screenReader.sample(screen.health, now: now, force: force, main: {
                let main = screen.takeMainThread()
                return (main.longest, main.long)
            }) || read
        }
        return read
    }

    /// Show what changed. While the app draws, the change waits for its next frame, to go out with it; otherwise it goes
    /// out now, alone.
    private func show() {
        // Paused, the card shows what it showed
        guard !paused else { return }
        if let screen, screen.running, screen.drawing { needsShow = true; return }
        needsShow = false
        alone {
            render()
            card.chart.present()
        }
    }

    /// Make a change of the card's own between the app's frames. Measuring the screen, the card draws it and commits it
    /// in a Core Animation transaction of its own, which takes no UI update; after a change of the app's in this turn
    /// of the run loop, UIKit's implicit transaction holds it, and it goes out in that change's update, one of the
    /// app's frames (LLP 1079.000 §7.4).
    private func alone(_ change: () -> Void) {
        guard observing else { change(); return }
        CATransaction.begin()
        change()
        // A picture Core Animation draws as it commits takes an update
        card.drawNow()
        CATransaction.commit()
    }

    /// Within an update, as an animation's changes are: the card's waiting change and the charts' new frames go out with
    /// it.
    private func drawWithUpdate() {
        asCard {
            showWaiting()
            card.chart.present()
        }
    }

    /// Do the card's own work, whose time is no part of the main thread's work it measures.
    private func asCard(_ work: () -> Void) {
        if let screen { screen.card(work) } else { work() }
    }

    private func showWaiting() {
        guard needsShow, !dragging, !paused else { return }
        needsShow = false
        render()
    }

    private var rates: FrameHealthCardText.Rates {
        frozen?.rates ?? FrameHealthCardText.Rates(requestedHz: Double(health?.requestedHertz ?? 0),
            periodMs: Double(reader.samples.last?.periodNs ?? health?.snapshot().period_ns ?? 0) / 1e6)
    }

    private var display: FrameHealthCardText.Display { frozen?.display ?? Self.display(window?.screen ?? UIScreen.main) }

    private var source: FrameHealthCardText.Source {
        if let frozen { return frozen.source }
        guard let screen, screen.running else { return .exact }
        return .screen(refreshMs: screen.refresh * 1000, exact: .init(running: (health?.snapshot().running ?? 0) != 0, rates: rates))
    }

    /// The readers the card shows: the face's, and Exact's own frames' for Details where the face shows the screen's.
    private var shown: (face: FrameHealthCardReader, clock: FrameHealthCardReader?) {
        observing ? (screenReader, reader) : (reader, nil)
    }

    private func render() {
        guard let health else { return }
        let source = source, shown = shown
        card.update(FrameHealthCardPresentation(reader: shown.face, clock: shown.clock, rates: rates, display: display, source: source,
            status: paused ? .paused : source.screen || health.snapshot().running != 0 ? .live : .idle,
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
            // Paused, the card keeps what it showed and goes on measuring; resumed, it shows the present, the paused
            // span included
            frozen = paused ? nil : (rates, display, source)
            paused.toggle()
            card.chart.hold(paused, at: CACurrentMediaTime())
            if !observing { card.chart.follow(!paused) }
            UIAccessibility.post(notification: .announcement, argument: paused ? "Paused" : "Resumed")
            if paused { render() } else { refresh() }
        case .close: hideReadout()
        case .title: DevMenu.showFrameHealthDefinitions(source)
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
            // The report is of what the face shows: the screen's frames, with their recent late frames, where it measures them
            FrameHealthReport.prepare(health: observing ? screen?.health ?? health.reportSource : health.reportSource, session: session) {
                present(String(decoding: $0, as: UTF8.self))
            }
        case .summary:
            present(FrameHealthCardText.summary(reader: shown.face, clock: shown.clock, rates: rates, display: display, source: source,
                paused: paused, health: health).text)
        case .image:
            let picture = FrameHealthCardText.picture(reader: shown.face, clock: shown.clock, chart: card.chart, rates: rates,
                display: display, source: source, paused: paused, health: health, palette: card.palette)
            if let image = picture.flatMap(UIImage.init(data:)) { present(image) }
        }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        // Keep the card 8 points inside the window and above the keyboard, ignoring safe-area insets
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
        // Keep a dragged card under the press and retarget a spring when layout changes its destination
        let target = position.origin(of: size, in: available)
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
        dragged(true)
        UIView.performWithoutAnimation {
            card.frame.origin = FrameHealthCardFace.resisted(origin, size: card.frame.size, in: availableFrame)
            health?.hudBounds = window.map { card.convert(card.bounds, to: $0) }
        }
    }

    /// The card's frames, Exact's and the screen's, say whether it is dragged.
    private func dragged(_ on: Bool) {
        health?.hudDragging = on
        screen?.health.hudDragging = on
    }

    private func settle(velocity: CGVector) {
        position = FrameHealthCardFace.Position(nearest: card.frame.origin, velocity: velocity, size: card.frame.size, in: availableFrame)
        spring(velocity: velocity)
    }

    private func spring(velocity: CGVector) {
        let target = position.origin(of: card.frame.size, in: availableFrame)
        // UIKit takes a spring's initial velocity as a fraction of the distance a second
        let dx = target.x - card.frame.minX, dy = target.y - card.frame.minY
        let initial = CGVector(dx: dx == 0 ? 0 : velocity.dx / dx, dy: dy == 0 ? 0 : velocity.dy / dy)
        let spring = FrameHealthCardFace.spring
        let animator = UIViewPropertyAnimator(duration: 0.5, timingParameters: UISpringTimingParameters(mass: 1,
            stiffness: spring.stiffness, damping: spring.damping, initialVelocity: initial))
        animator.addAnimations { self.card.frame.origin = target }
        animator.addCompletion { [weak self] _ in self?.snap = nil; self?.fast(false); self?.setNeedsLayout() }
        snap = animator
        animator.startAnimation()
        health?.hudBounds = window.map { convert(CGRect(origin: target, size: card.frame.size), to: $0) }
    }

    /// Ask for the display's highest rates while the card moves, as Apple asks of an object moving fast across the screen:
    /// without, iOS moves a view a finger drags at its default rate, 60 Hz on a ProMotion iPhone. The link only states the
    /// range, which `UIUpdateLink.h` says expresses the intention without forcing updates, so the touches still make the
    /// drag's updates, and a finger held still makes none.
    private func fast(_ on: Bool) {
        guard #available(iOS 18.0, *) else { return }
        (motion as? UIUpdateLink)?.isEnabled = false
        motion = nil
        guard on, let screen = window?.windowScene?.screen else { return }
        let link = UIUpdateLink(view: self), highest = Float(screen.maximumFramesPerSecond)
        link.preferredFrameRateRange = CAFrameRateRange(minimum: min(80, highest), maximum: highest, preferred: highest)
        link.isEnabled = true
        motion = link
    }

    /// A press stops the spring where the card is drawn.
    private func stopSnap() {
        snap?.stopAnimation(true)
        snap = nil
    }

    /// Return to the resting position after a tap interrupts the spring.
    private func rest() {
        guard snap == nil, card.frame.origin != position.origin(of: card.frame.size, in: availableFrame) else { return }
        spring(velocity: .zero)
    }

    private func move(toward edge: CGVector) -> Bool {
        stopSnap()
        if edge.dx != 0 { position.x = edge.dx > 0 ? 1 : 0 }
        if edge.dy != 0 { position.y = edge.dy > 0 ? 1 : 0 }
        spring(velocity: .zero)
        return true
    }

    @objc @discardableResult func hideReadout() -> Bool { setVisible(false); return true }
}

/// The card: one opaque view that draws its face and its charts or table by hand, and turns
/// touches into presses, taps and drags. It draws at `scale`, the text size's, in the face's points.
final class FrameHealthCardView: UIView {
    var state = FrameHealthCardFace.State()
    /// Runs the face's drawing, which Core Animation asks for as it commits, so the card can count it as its own work
    var drawing: ((() -> Void) -> Void)?
    var compact = false { didSet { if compact != oldValue { setNeedsDisplay(); updateAccessibility(); updatePlots() } } }
    var scale: CGFloat = 1 { didSet { if scale != oldValue { setNeedsDisplay(); updateAccessibility(); updatePlots() } } }
    /// The charts, fed by each frame while the card reads, and the scales and view their labels were
    /// last drawn for
    private(set) lazy var chart = FrameHealthChart(in: content.layer)
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
    /// The face, Details and the charts, clipped to the card's outline by a view of their own: the card's layer keeps
    /// its border, the hairline, and neither draws nor clips. Under a corner configuration's corners a border fades
    /// around them where its own layer clips drawn content (iOS 27.1).
    private let content = UIView()
    private let faceView = Face()

    override init(frame: CGRect) {
        super.init(frame: frame)
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
        (faceView.card, detailsView.card) = (self, self)
        faceView.isUserInteractionEnabled = false
        content.addSubview(faceView)
        content.addSubview(detailsScroll)
        addSubview(content)
        // The drawing fills the bounds; the content clips it to the card's rounded outline, which in a
        // corner of the screen follows the screen's own rounding, so that rounding never cuts the card
        for view in [self, content] {
            if #available(iOS 26.0, *) {
                view.cornerConfiguration = .corners(radius: .containerConcentric(minimum: 12))
            } else {
                view.layer.cornerRadius = 12
                view.layer.cornerCurve = .continuous
            }
        }
        content.layer.masksToBounds = true
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

    var layout: FrameHealthCardFace.Layout {
        .init(width: bounds.width / scale, compact: compact, showsDetails: state.showsDetails, corners: topCorners)
    }
    /// The radii of the card's top corners in the face's points, whole: they change only as the card comes to or leaves
    /// a corner of the screen, where it follows the screen's rounding
    private var topCorners: (topLeft: CGFloat, topRight: CGFloat) = (12, 12)
    private func face(_ point: CGPoint) -> CGPoint { CGPoint(x: point.x / scale, y: point.y / scale) }
    private func view(_ rect: CGRect) -> CGRect { rect.applying(CGAffineTransform(scaleX: scale, y: scale)) }

    override func layoutSubviews() {
        super.layoutSubviews()
        content.frame = bounds
        faceView.frame = bounds
        // Redraw the header only when a top corner's radius changes: UIKit lays the card out again when it moves, with
        // its corners already for where it is
        if #available(iOS 26.0, *) {
            let corners = (topLeft: (effectiveRadius(corner: .topLeft) / scale).rounded(),
                           topRight: (effectiveRadius(corner: .topRight) / scale).rounded())
            if corners != topCorners {
                topCorners = corners
                setNeedsDisplay(view(CGRect(x: 0, y: 0, width: layout.width, height: 44)))
            }
        }
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

    // Pass redraws to the face, since the card's own layer draws nothing
    // The face's layer draws the whole face whatever part changed; a view that draws nothing of its own ignores
    // `setNeedsDisplay()`, so the layer is asked directly
    override func setNeedsDisplay() { faceView.layer.setNeedsDisplay() }

    /// Lay out and draw now what needs it: the face, Details and the plots.
    func drawNow() {
        layoutIfNeeded()
        faceView.layer.displayIfNeeded()
        detailsView.layer.displayIfNeeded()
        chart.drawNow()
    }
    override func setNeedsDisplay(_ rect: CGRect) { faceView.layer.setNeedsDisplay() }

    /// A view of the card's that renders its picture as an image, which Core Animation commits in the update that asks
    /// for it; drawn into a backing store by `draw(_:)`, it would take UIKit another update a refresh later to show
    /// each drawing (LLP 1079.000 §7.4). Its drawing is the card's work (`drawing`).
    class Pictured: UIView {
        weak var card: FrameHealthCardView?
        override class var layerClass: AnyClass { Picture.self }

        override init(frame: CGRect) {
            super.init(frame: frame)
            // A view that draws nothing of its own keeps its layer's scale at 1, so the picture follows the display's
            registerForTraitChanges([UITraitDisplayScale.self]) { (view: Pictured, _: UITraitCollection) in view.layer.setNeedsDisplay() }
        }

        required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

        /// Draw the picture in the current context, in the view's points.
        func paint() {}

        final class Picture: CALayer {
            override func display() {
                guard let view = delegate as? Pictured, let card = view.card, bounds.width > 0, bounds.height > 0 else { return }
                // At the display's scale, which the layer then takes too
                // 8-bit sRGB, as the plots: the card's colours are all sRGB, which a wide-colour display's extended range
                // would hold in twice the bytes
                let format = UIGraphicsImageRendererFormat(for: view.traitCollection)
                (format.opaque, format.preferredRange) = (true, .standard)
                contentsScale = format.scale
                let draw = { self.contents = UIGraphicsImageRenderer(bounds: self.bounds, format: format).image { _ in view.paint() }.cgImage }
                if let drawing = card.drawing { drawing(draw) } else { draw() }
            }
        }
    }

    private final class Face: Pictured {
        override func paint() { card?.drawFace() }
    }

    private func drawFace() {
        guard let ctx = UIGraphicsGetCurrentContext(), let palette else { return }
        ctx.setFillColor(palette.background)
        ctx.fill(bounds)
        ctx.scaleBy(x: scale, y: scale)
        FrameHealthCardFace.draw(ctx, layout, state: state, palette: palette)
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
                label = state.rows.map { "\($0.0): \($0.1) FPS, \($0.2) \(state.unit)" }.joined(separator: ". ")
            case .pacing:
                frame = CGRect(x: 0, y: 125, width: l.width, height: 41)
                label = "\(state.pacingName), \(state.pacingWindow): " + [state.pacing.0, state.pacing.1].filter { !$0.isEmpty }.joined(separator: ", ")
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
    final class DetailsView: Pictured {
        /// What it shows, at the card's width and scale
        private struct Shown: Equatable {
            var rows: [FrameHealthCardText.Detail], caption: String, width: CGFloat, scale: CGFloat
            var palette: FrameHealthCardDrawing.Palette
        }
        private var shown: Shown?
        override init(frame: CGRect) {
            super.init(frame: frame)
            isOpaque = true
            // Its size follows its rows and the card's
            layer.needsDisplayOnBoundsChange = true
        }
        required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
        /// Show these rows; only a change draws them again.
        func set(rows: [FrameHealthCardText.Detail], caption: String, width: CGFloat, scale: CGFloat, palette: FrameHealthCardDrawing.Palette) {
            let next = Shown(rows: rows, caption: caption, width: width, scale: scale, palette: palette)
            guard next != shown else { return }
            shown = next
            backgroundColor = UIColor(cgColor: palette.background)
            layer.setNeedsDisplay()
        }
        override func paint() {
            guard let ctx = UIGraphicsGetCurrentContext(), let shown else { return }
            ctx.scaleBy(x: shown.scale, y: shown.scale)
            FrameHealthCardFace.details(ctx, width: shown.width, rows: shown.rows, caption: shown.caption, palette: shown.palette)
        }
    }
}

final class FrameHealthDefinitionsController: UIViewController {
    private let source: FrameHealthCardText.Source

    init(_ source: FrameHealthCardText.Source) {
        self.source = source
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

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
        for (term, definition) in FrameHealthCardText.definitions(source) {
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
        let overlay = FrameHealthOverlay(health: session.frameHealth, session: session)
        overlay.frame = container.bounds
        overlay.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        container.addSubview(overlay)
        objc_setAssociatedObject(controller, &overlayKey, overlay, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return overlay
    }

    /// The definitions of the figures the card gives from `source`.
    static func showFrameHealthDefinitions(_ source: FrameHealthCardText.Source) {
        let navigation = UINavigationController(rootViewController: FrameHealthDefinitionsController(source))
        navigation.modalPresentationStyle = .pageSheet
        navigation.sheetPresentationController?.detents = [.medium(), .large()]
        navigation.sheetPresentationController?.prefersGrabberVisible = true
        present(navigation)
    }
}
#endif

// The view that presents a session on UIKit (LLP 1031 D1, D3): an ordinary
// UIView the containing app places. Its bounds are the session's viewport
// — the safe area of its own container, or the whole of it when the first
// root says `viewport-fit="cover"`, the safe-area insets then going to the
// kernel for its `env()` lengths (LLP 1008 §9) — and, under
// `interactive-widget="resizes-content"`, the keyboard's top. The plan
// boots at the first layout that has a size and follows every later size
// (a rotation, a split) and every change of the insets. Bounded
// containment: the session's page scrolls inside this view exactly as the
// standalone host's does.
#if os(iOS)
import UIKit

public final class ExactView: UIView {
    public let session: ExactSession
    private var fitPending = false
    private var lastSize = CGSize.zero
    private var lastInsets = UIEdgeInsets.zero
    private var lastFold = ViewportFold.flat
    private var lastDisplayScale: CGFloat = 0
    private var keyboardProbe: UIView?
    private var keyboardObserver: NSObjectProtocol?
    /// The adapter's hook for the first root's `viewport-fit` and its
    /// canvas colour (the window's background under the safe areas is the
    /// window's business).
    public var onViewportFit: (() -> Void)?
    public var onCanvasColor: ((UIColor) -> Void)?
    /// The adapter's hook for the head's title (LLP 1048.003 D1): the scene
    /// title is the app's to set. It hears the current title when set.
    public var onTitle: ((String?) -> Void)? { didSet { onTitle?(session.presenter.title) } }

    public init(session: ExactSession) {
        self.session = session
        super.init(frame: .zero)
        // The launch screen's colour (the manifest's `launch`) until the first frame names the canvas.
        backgroundColor = UIColor(named: "ExactLaunch") ?? .white
        addSubview(session.presenter.viewport)
        keyboardObserver = NotificationCenter.default.addObserver(
            forName: UIResponder.keyboardWillChangeFrameNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.installKeyboardProbe() }
        }
        session.view = self
        session.presenter.onViewportFit = { [weak self] in self?.setNeedsLayout(); self?.onViewportFit?() }
        session.presenter.onCanvasColor = { [weak self] color in self?.backgroundColor = color; self?.onCanvasColor?(color) }
        session.presenter.onTitle = { [weak self] title in self?.onTitle?(title) }
        session.presenter.onKeyboardResize = { [weak self] in self?.fit() }
        session.presenter.observeKeyboard()
        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitDisplayScale.self]) { (view: ExactView, _: UITraitCollection) in view.reportScheme(); view.setNeedsLayout() }
        // A hinge moving from flat to a book angle changes the division
        // regions' `isActive` without changing any bounds; nothing else would
        // lay out again (LLP 1076 D5).
        ReservedRegions.observeHinge(on: self) { [weak self] status in
            guard let self else { return }
            if status != nil { session.presenter.hasFold = true }
            setNeedsLayout()
        }
    }

    /// Paint motion resolves `light-dark()` by this view's appearance (LLP 1062).
    /// A system appearance change reaches the view as a trait change too:
    /// `prefers-color-scheme` is told again (LLP 1069.000 D1).
    private func reportScheme() { session.scheme(dark: traitCollection.userInterfaceStyle == .dark); session.tellPreferences() }

    required init?(coder: NSCoder) { nil }
    deinit { keyboardObserver.map(NotificationCenter.default.removeObserver) }

    /// A zero-size dependent makes UIKit lay this view out as its keyboard
    /// guide moves: frame notifications alone omit interactive drag frames.
    /// It comes with the first keyboard, not before: a constraint anywhere in
    /// the window gives the window a layout engine, and every view added
    /// after joins it (`_switchToLayoutEngine:`), which a list pays for each
    /// view of each row it builds. No node view uses a constraint.
    private func installKeyboardProbe() {
        guard keyboardProbe == nil else { return }
        keyboardObserver.map(NotificationCenter.default.removeObserver)
        keyboardObserver = nil
        let probe = UIView()
        probe.isHidden = true
        probe.translatesAutoresizingMaskIntoConstraints = false
        addSubview(probe)
        NSLayoutConstraint.activate([
            probe.topAnchor.constraint(equalTo: keyboardLayoutGuide.topAnchor),
            probe.leadingAnchor.constraint(equalTo: leadingAnchor),
            probe.widthAnchor.constraint(equalToConstant: 0),
            probe.heightAnchor.constraint(equalToConstant: 0),
        ])
        keyboardProbe = probe
    }

    /// The first root's `viewport-fit` prop (`"cover"` or nothing).
    public var viewportFit: String? { session.presenter.viewportFit }

    public override func layoutSubviews() {
        super.layoutSubviews()
        fit()
    }

    public override func willMove(toWindow newWindow: UIWindow?) {
        // Child didMoveToWindow callbacks can retry focus before our own
        // didMoveToWindow. Wait until their native owners have been installed.
        if newWindow != nil { session.presenter.navigation.willMount() }
        super.willMove(toWindow: newWindow)
    }

    public override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil { reportScheme() }
        session.rasters.setPaused(window == nil)
        session.canvases.lifecycle.refresh()
        if window == nil {
            session.presenter.menus.unmounted()
            session.presenter.modals.unmounted()
            session.presenter.navigation.unmounted()
        } else {
            // Install native ownership after UIKit finishes attaching this
            // view. A retained session may return under a different controller.
            DispatchQueue.main.async { [weak self] in
                guard let self, window != nil, session.state != .destroyed else { return }
                fit()
                session.presenter.navigation.mounted()
                session.presenter.syncAccessibility()
            }
        }
        // Mounted and visible participate in frame demand (D3): an unmounted
        // view wants no frames; a mounted one asks again.
        session.frames.run(window != nil && (session.frames.motion || session.canvases.wantsFrames))
    }

    /// Frame the viewport to the safe area or the whole view — and, under
    /// `interactive-widget="resizes-content"`, to the keyboard's top, where
    /// the bottom inset is the keyboard's and not the home indicator's (the
    /// web's rule) — and, once booted, tell the kernel about new insets or a
    /// new size. Called inside the keyboard's animation block, so the frames
    /// the batch sets animate with the keyboard (LLP 1008 §9).
    func fit() {
        let presenter = session.presenter
        // Containment may synchronously lay us out during a partial batch.
        // Let the next layout read the fully mounted tree before resizing it.
        if presenter.applying {
            if !fitPending {
                fitPending = true
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    self.fitPending = false
                    self.fit()
                }
            }
            return
        }
        // UIKit moves the keyboard sideways with an interactive pop. Its
        // hide notification is not a request to drop the composer below
        // those moving keys; keep the current viewport until it settles.
        guard !presenter.navigation.preservesKeyboardViewport else { return }
        let container = presenter.modals.coordinateView ?? self
        let safe = container.safeAreaInsets
        let cover = presenter.viewportFit == "cover"
        var frame = cover ? container.bounds : container.bounds.inset(by: safe)
        var insets = cover ? safe : .zero
        if presenter.interactiveWidget == "resizes-content" {
            let top: CGFloat
            // A sheet's guide remains in its local coordinates as UIKit moves
            // the sheet, including during interactive dismissal.
            // A focused editor uses that same local guide. After rotation the
            // notification can include margin above the keys; after a cancelled
            // pop it can still announce hiding. Neither replaces the guide's
            // occupied geometry while this session retains its editor.
            if presenter.interactiveKeyboardDrag || presenter.modals.active ||
                presenter.hasKeyboardEditor {
                let guide = container.keyboardLayoutGuide.layoutFrame
                top = guide.height > safe.bottom + 1 ? guide.minY : .infinity
            } else if let edge = presenter.keyboardTop, let window {
                top = container.convert(CGPoint(x: 0, y: edge), from: window).y
            } else { top = .infinity }
            presenter.keyboardInset = min(max(0, frame.maxY - max(top, frame.minY)), frame.height)
            if top < frame.maxY {
                frame.size.height = max(0, top - frame.minY)
                insets.bottom = 0
            }
        }
        guard frame.width > 0, frame.height > 0 else { return }
        // Display changes can leave logical bounds unchanged. Update resources
        // independently of the resize commit, from this view's local traits.
        // The raster budget follows this view's own bounds — the panel it
        // covers — never the frame a keyboard or a sheet clips the viewport
        // to: a keyboard rising is not a smaller display, and a shrink trims
        // the cold cache and halves queued decodes in place.
        let scale = max(1, traitCollection.displayScale)
        session.rasters.fit(size: bounds.size, scale: scale)
        if scale != lastDisplayScale {
            lastDisplayScale = scale
            session.apply(session.runtime.canvasDisplay(scale: scale, memory: ProcessInfo.processInfo.physicalMemory))
            session.rasters.displayChanged()
        }
        var size = frame.size
        // The agent's explicit viewport size is shared with web/macOS/Linux.
        // Fit those logical points into the device window; hit testing and
        // captures still use the viewport's own coordinate system.
        let env = ExactEnv.environment
        if env["EXACT_AGENT"] == "1", let width = Double(env["EXACT_WINDOW_WIDTH"] ?? ""),
           let height = Double(env["EXACT_WINDOW_HEIGHT"] ?? ""),
           width.isFinite, height.isFinite, width > 0, height > 0 {
            size = CGSize(width: width, height: height)
            let scale = min(frame.width / size.width, frame.height / size.height)
            presenter.viewport.transform = CGAffineTransform(scaleX: scale, y: scale)
            presenter.viewport.bounds = CGRect(origin: .zero, size: size)
            presenter.viewport.center = CGPoint(x: frame.midX, y: frame.midY)
            insets = .zero
        } else {
            presenter.viewport.transform = .identity
            if presenter.viewport.frame != frame { presenter.viewport.frame = frame }
        }
        guard size.width > 0, size.height > 0 else { return }
        // The fold (LLP 1076 D5): the container's active division regions,
        // converted into the viewport's space, split it into segments; the
        // posture is folded while any is active. Below 27.1 there are none.
        let fold = Self.fold(of: container, viewport: frame, size: size)
        if fold.hasFold { presenter.hasFold = true }
        if !session.booted {
            lastSize = size
            lastInsets = insets
            lastFold = fold.fold
            session.boot(size: size)
            // The first batch made the roots: one that covers the screen is
            // framed to it now, before anything is drawn.
            fit()
            return
        }
        if insets != lastInsets {
            lastInsets = insets
            presenter.insets = insets
            session.insets(top: insets.top, right: insets.right, bottom: insets.bottom, left: insets.left)
        }
        if size != lastSize {
            lastSize = size
            session.resize(size)
        }
        if fold.fold != lastFold {
            lastFold = fold.fold
            session.segments(fold.fold)
        }
    }

    /// The segments the container's division regions make of the viewport
    /// framed at `frame` in the container's coordinates (`size` is the
    /// viewport's own, which an agent's window override may scale), and
    /// whether the device reported a fold at all.
    static func fold(of container: UIView, viewport frame: CGRect, size: CGSize) -> (fold: ViewportFold, hasFold: Bool) {
        guard let divisions = ReservedRegions.divisions(of: container) else { return (.flat, false) }
        let scale = CGPoint(x: size.width / frame.width, y: size.height / frame.height)
        let active = divisions.filter(\.active).map { d in
            CGRect(x: (d.frame.minX - frame.minX) * scale.x, y: (d.frame.minY - frame.minY) * scale.y, width: d.frame.width * scale.x, height: d.frame.height * scale.y)
        }
        return (Segments.split(viewport: size, dividers: active), !divisions.isEmpty)
    }

    /// After a restart from a new plan (the dev loop): the new runner knows
    /// nothing of the insets or the fold — hand them over again (the fold
    /// always: the bake's flat answer must never stand in for the device's,
    /// LLP 1076 D5), and fit the root.
    func rebooted() {
        if lastInsets != .zero { session.insets(top: lastInsets.top, right: lastInsets.right, bottom: lastInsets.bottom, left: lastInsets.left) }
        session.segments(lastFold)
        reportScheme()
        fit()
    }
}
#endif

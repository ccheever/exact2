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
#if os(iOS) || os(tvOS)
import UIKit

public final class ExactView: UIView {
    public let session: ExactSession
    private var fitPending = false
    private var lastSize = CGSize.zero
    private var lastInsets = UIEdgeInsets.zero
    private var lastFold = ViewportFold.flat
    private var lastScreen: CGSize?
    /// The hinge's last status from `UIHingeInteraction` (1 closed, 2
    /// partially open, 3 fully open; nil before it reports or without a
    /// hinge), and how many layouts have re-read the division regions since
    /// it last changed while they disagreed with it (LLP 1078 D5).
    private var hingeStatus: Int?
    private var regionRereads = 0
    private static let regionRereadLimit = 5
    private var lastDisplayScale: CGFloat = 0
    private var keyboardProbe: UIView?
    /// What this view adds beside the viewport, for `layout agree` (LLP 1080.001 D3).
    var inspectionOwned: [UIView] { keyboardProbe.map { [$0] } ?? [] }
    private var keyboardObserver: NSObjectProtocol?
    /// Fits again once the keyboard has stopped moving (see `init`).
    private var keyboardSettled: [NSObjectProtocol] = []
    /// The adapter's hook for the first root's `viewport-fit` and its
    /// canvas colour (the window's background under the safe areas is the
    /// window's business).
    public var onViewportFit: (() -> Void)?
    public var onCanvasColor: ((UIColor) -> Void)?
    /// The adapter's hook for the head's title (LLP 1048.003 D1): the scene
    /// title is the app's to set. It hears the current title when set.
    public var onTitle: ((String?) -> Void)? { didSet { onTitle?(session.presenter.title) } }
    #if os(iOS)
    /// The status bar's style the app declared (LLP 1105), for the
    /// controller that hosts this view to return from
    /// `preferredStatusBarStyle`, and the hook that says it changed, called
    /// inside the change's animation (a fade, when the app asks for one) and
    /// with the current style when set: call `setNeedsStatusBarAppearanceUpdate`.
    public var statusBarStyle: UIStatusBarStyle { session.presenter.statusBar.style }
    public var onStatusBarStyle: ((UIStatusBarStyle) -> Void)? { didSet { onStatusBarStyle?(statusBarStyle) } }
    #endif

    public init(session: ExactSession) {
        self.session = session
        super.init(frame: .zero)
        // The launch screen's colour (the manifest's `launch`) until the first frame names the canvas.
        backgroundColor = UIColor(named: "ExactLaunch") ?? .white
        addSubview(session.presenter.viewport)
        #if !os(tvOS)
        keyboardObserver = NotificationCenter.default.addObserver(
            forName: UIResponder.keyboardWillChangeFrameNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.installKeyboardProbe() }
        }
        #endif
        session.view = self
        session.presenter.onViewportFit = { [weak self] in self?.setNeedsLayout(); self?.onViewportFit?() }
        session.presenter.onCanvasColor = { [weak self] color in self?.backgroundColor = color; self?.onCanvasColor?(color) }
        session.presenter.onTitle = { [weak self] title in self?.onTitle?(title) }
        #if os(iOS)
        session.presenter.onStatusBar = { [weak self] choice in self?.onStatusBarStyle?(choice.style) }
        #endif
        session.presenter.onKeyboardResize = { [weak self] in self?.fit() }
        // A pop that takes the editing route away moves the keys with it: the
        // hide arrives while the transition freezes the viewport, and once
        // the editor is gone no later notification concerns this session.
        // Without this the page stayed laid out above keys that had left —
        // the bottom of the list missing after a swipe back. Fitting is
        // idempotent, so fit again when the keyboard has settled.
        // tvOS has no keyboard frame notifications.
        #if !os(tvOS)
        keyboardSettled = [UIResponder.keyboardDidHideNotification, UIResponder.keyboardDidChangeFrameNotification].map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.fit() }
            }
        }
        #endif
        session.presenter.observeKeyboard()
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self, UITraitLegibilityWeight.self, UITraitDisplayScale.self]) { (view: ExactView, _: UITraitCollection) in view.session.controlTextChanged() }
        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitDisplayScale.self, UITraitAccessibilityContrast.self]) { (view: ExactView, _: UITraitCollection) in view.reportScheme(); view.setNeedsLayout() }
        // The tvOS SDK has no Swift UITraitDefinition for this trait. OS
        // suppression still applies to its layers without a re-decode (D9).
        #if os(iOS)
        if #available(iOS 26, *) {
            registerForTraitChanges([UITraitHDRHeadroomUsageLimit.self]) { (view: ExactView, _: UITraitCollection) in view.session.rasters.displayChanged() }
        }
        #endif
        // A hinge moving from flat to a book angle changes the division
        // regions' `isActive` without changing any bounds; nothing else would
        // lay out again (LLP 1078 D5). The status is kept: the posture
        // follows it, and a layout whose regions disagree with it re-reads
        // them on the following frames (`fit`).
        ReservedRegions.observeHinge(on: self) { [weak self] status in
            guard let self else { return }
            session.presenter.hingeReported = true
            if status != nil { session.presenter.hasFold = true }
            if status != hingeStatus { hingeStatus = status; regionRereads = 0 }
            setNeedsLayout()
        }
    }

    /// Paint motion resolves `light-dark()` by this view's appearance (LLP 1062).
    /// A system appearance change reaches the view as a trait change too:
    /// `prefers-color-scheme` is told again (LLP 1069.000 D1).
    private func reportScheme() { session.scheme(dark: traitCollection.userInterfaceStyle == .dark); session.tellPreferences(); session.natives.scopesChanged() }
    /// An app or window tint changed: `AccentColor` is reported again (LLP 1095 D9).
    public override func tintColorDidChange() { super.tintColorDidChange(); session.reportColors() }

    required init?(coder: NSCoder) { nil }
    deinit {
        keyboardObserver.map(NotificationCenter.default.removeObserver)
        keyboardSettled.forEach(NotificationCenter.default.removeObserver)
    }

    /// A zero-size dependent makes UIKit lay this view out as its keyboard
    /// guide moves: frame notifications alone omit interactive drag frames.
    /// It comes with the first keyboard, not before: a constraint anywhere in
    /// the window gives the window a layout engine, and every view added
    /// after joins it (`_switchToLayoutEngine:`), which a list pays for each
    /// view of each row it builds. No node view uses a constraint.
    #if !os(tvOS)
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
    #endif

    /// The first root's `viewport-fit` prop (`"cover"` or nothing).
    public var viewportFit: String? { session.presenter.viewportFit }

    public override func layoutSubviews() {
        super.layoutSubviews()
        fit()
    }

    #if os(iOS)
    // aria-keyshortcuts as key commands (ShortcutsIOS, gallery F18).
    // Shortcut focus is not text entry. UIKit may reload a menu's previous
    // responder's input views; an explicit empty view prevents its default
    // software keyboard while preserving the hardware-key responder chain.
    // Input views are inherited up that chain: return nil when a text field
    // owns focus, so it still receives the system keyboard.
    private lazy var shortcutInputView = UIView(frame: .zero)
    public override var inputView: UIView? { isFirstResponder ? shortcutInputView : nil }
    public override var canBecomeFirstResponder: Bool { true }
    public override var keyCommands: [UIKeyCommand]? { session.presenter.shortcutCommands(#selector(exactShortcut(_:))) }
    @objc private func exactShortcut(_ command: UIKeyCommand) { session.presenter.performShortcut(command) }
    #endif

    public override func willMove(toWindow newWindow: UIWindow?) {
        // Child didMoveToWindow callbacks can retry focus before our own
        // didMoveToWindow. Wait until their native owners have been installed.
        if newWindow != nil { session.presenter.navigation.willMount() }
        if newWindow !== window { session.natives.windowLeaving(window) }   // its window hatch ends while the window is there
        super.willMove(toWindow: newWindow)
    }

    public override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil { reportScheme() }
        session.tellPage() // `hasFocus` is this window's scene's (#114)
        session.natives.scopesChanged()
        session.rasters.setPaused(window == nil)
        session.canvases.refreshLifecycle()
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
                #if os(iOS)
                // Nothing focused (autofocus has run): this view holds the
                // focus, so a hardware keyboard's shortcuts are heard (ShortcutsIOS).
                if session.presenter.focusedNode == nil { _ = becomeFirstResponder() }
                #endif
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
        // A native container showing its bars takes the whole view, as
        // UIKit's do: the bar's material reaches under the status bar. Its
        // routes keep their content inside its safe area through covers
        // (LLP 1075.003 §3.5); `env()` stays what the page authored.
        let whole = cover || presenter.navigation.wantsWholeView
        var frame = whole ? container.bounds : container.bounds.inset(by: safe)
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
                #if os(tvOS)
                // tvOS has no keyboard layout guide.
                top = .infinity
                #else
                let guide = container.keyboardLayoutGuide.layoutFrame
                top = guide.height > safe.bottom + 1 ? guide.minY : .infinity
                #endif
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
        // LLP 1075.003 §9.11: a fit-content sheet's viewport is the sheet,
        // and its `vh` the screen's — this view's own viewport, as it is
        // without the sheet — so no length the sheet measures follows it.
        let fits = presenter.modals.fitsContent
        let screenFrame = whole ? bounds : bounds.inset(by: safeAreaInsets)
        var screen = fits ? screenFrame.size : nil
        // The agent's explicit viewport size is shared with web/macOS/Linux.
        // Fit those logical points into the device window; hit testing and
        // captures still use the viewport's own coordinate system.
        let env = ExactEnv.environment
        if env["EXACT_AGENT"] == "1", let width = Double(env["EXACT_WINDOW_WIDTH"] ?? ""),
           let height = Double(env["EXACT_WINDOW_HEIGHT"] ?? ""),
           width.isFinite, height.isFinite, width > 0, height > 0 {
            size = CGSize(width: width, height: height)
            if fits { screen = size }
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
        // The fold (LLP 1078 D5): the container's active division regions,
        // converted into the viewport's space, split it into segments; the
        // posture is folded while any is active or the hinge says it is
        // partially open. Below 27.1 there are none.
        let bent = hingeStatus == 2
        // Under a fit-content sheet the segments are the screen's too.
        let fold = fits ? Self.fold(of: self, viewport: screenFrame, size: screen ?? size, hingeBent: bent)
            : Self.fold(of: container, viewport: frame, size: size, hingeBent: bent)
        if fold.hasFold { presenter.hasFold = true }
        // The regions can trail the hinge's update by a frame (the handler
        // runs before UIKit flips `isActive`), so a reading that disagrees
        // with the hinge is taken again on the next frames, a bounded number
        // of times per hinge change: a scene whose regions are never
        // reported (seen on the 27.1 simulator) keeps the hinge's posture and
        // stops asking.
        if ReservedRegions.available, hingeStatus != nil, bent != (fold.activeDivisions > 0), regionRereads < Self.regionRereadLimit {
            regionRereads += 1
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in self?.setNeedsLayout() }
        }
        if !session.booted {
            // A refused boot is terminal (LLP 1031 D8): the session says why
            // in `bootError`, and a layout does not ask again.
            if case .failed = session.state { return }
            lastSize = size
            lastInsets = insets
            lastFold = fold.fold
            session.boot(size: size)
            // The first batch made the roots: one that covers the screen is
            // framed to it now, before anything is drawn. A refused boot made
            // none, and fitting it again would boot again, without end.
            if session.booted { fit() }
            return
        }
        // Before the resize, so the sheet's viewport is never laid out with
        // `vh` its own.
        if screen != lastScreen {
            lastScreen = screen
            session.screen(screen)
        }
        if insets != lastInsets {
            lastInsets = insets
            presenter.insets = insets
            session.insets(top: insets.top, right: insets.right, bottom: insets.bottom, left: insets.left)
            session.natives.scopesChanged()
        }
        if size != lastSize {
            lastSize = size
            session.resize(size)
            session.natives.scopesChanged()
        }
        if fold.fold != lastFold {
            lastFold = fold.fold
            session.segments(fold.fold)
        }
    }

    /// The segments the container's division regions make of the viewport
    /// framed at `frame` in the container's coordinates (`size` is the
    /// viewport's own, which an agent's window override may scale), with
    /// the posture `folded` while the hinge is bent even without an active
    /// region; whether the device reported a fold at all (a region, active
    /// or not); and how many regions are active.
    static func fold(of container: UIView, viewport frame: CGRect, size: CGSize, hingeBent: Bool = false) -> (fold: ViewportFold, hasFold: Bool, activeDivisions: Int) {
        guard let divisions = ReservedRegions.divisions(of: container) else { return (.flat, false, 0) }
        let scale = CGPoint(x: size.width / frame.width, y: size.height / frame.height)
        let active = divisions.filter(\.active).map { d in
            CGRect(x: (d.frame.minX - frame.minX) * scale.x, y: (d.frame.minY - frame.minY) * scale.y, width: d.frame.width * scale.x, height: d.frame.height * scale.y)
        }
        return (Segments.split(viewport: size, dividers: active, hingeBent: hingeBent), !divisions.isEmpty, active.count)
    }

    /// After a restart from a new plan (the dev loop): the new runner knows
    /// nothing of the insets or the fold — hand them over again (the fold
    /// always: the bake's flat answer must never stand in for the device's,
    /// LLP 1078 D5), and fit the root.
    func rebooted() {
        if lastInsets != .zero { session.insets(top: lastInsets.top, right: lastInsets.right, bottom: lastInsets.bottom, left: lastInsets.left) }
        session.segments(lastFold)
        reportScheme()
        fit()
    }
}
#endif

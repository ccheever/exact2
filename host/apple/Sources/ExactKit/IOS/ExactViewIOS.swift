// The view that presents a session on UIKit (LLP 1031 D1, D3): an ordinary
// UIView the containing app places. Its bounds are the session's viewport
// — the safe area of its own container, or the whole of it when the first
// root says `viewport-fit="cover"`, the safe-area insets then going to the
// kernel for its `env()` lengths (LLP 1008 §9). Native routes avoid their
// own keyboard inside that viewport; an ordinary page resizes or insets it.
// The plan boots at the first layout that has a size and follows every later size
// (a rotation, a split) and every change of the insets. Bounded
// containment: the session's page scrolls inside this view exactly as the
// standalone host's does.
#if os(iOS) || os(tvOS)
import UIKit

/// The launch screen's colour, by the name `UILaunchScreen` gives it, which
/// host/apple/assets.mjs makes from the colour itself; nil where the build
/// declares none.
public let launchScreenColor = ((Bundle.main.object(forInfoDictionaryKey: "UILaunchScreen") as? [String: Any])?["UIColorName"] as? String)
    .flatMap { UIColor(named: $0) }

public final class ExactView: UIView {
    public let session: ExactSession
    private var fitPending = false
    /// The canvas's colour, which this view shows wherever the viewport does
    /// not reach (`paintUnderKeys`).
    private var canvas: UIColor?
    private var lastSize = CGSize.zero
    private var lastInsets = UIEdgeInsets.zero
    /// The layout regions last told (LLP 1123 D10); `nil` until a booted
    /// runner has them.
    private var lastRegions: [CGFloat]?
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
    var hasKeyboardGuide: Bool { keyboardProbe != nil }
    /// What this view adds beside the viewport, for `layout agree` (LLP 1080.001 D3).
    var inspectionOwned: [UIView] { keyboardProbe.map { [$0] } ?? [] }
    private var editingObservers: [NSObjectProtocol] = []
    private var sceneObservers: [NSObjectProtocol] = []
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
    /// The session's orientation lock (LLP 1133.029 D5), for the controller
    /// that hosts this view to return from `supportedInterfaceOrientations`
    /// (nil: its own answer) and `prefersInterfaceOrientationLocked`. The
    /// presenter asks the window's controllers to read them again; a session
    /// that does not own its window (`hatchesOwnWindow`) asks for none.
    public var orientationMask: UIInterfaceOrientationMask? { session.presenter.orientationLock.supported }
    public var orientationLocked: Bool { session.presenter.orientationLock.locked }
    #endif

    public init(session: ExactSession) {
        self.session = session
        super.init(frame: .zero)
        // The launch screen's colour (the manifest's `launch`) until the first
        // frame names the canvas; unset, the platform's (LLP 1115 D2).
        backgroundColor = launchScreenColor ?? .platformBackground(in: nil)
        canvas = backgroundColor
        addSubview(session.presenter.viewport)
        addSubview(session.presenter.documentMount)
        session.view = self
        session.presenter.onViewportFit = { [weak self] in self?.setNeedsLayout(); self?.onViewportFit?() }
        session.presenter.onCanvasColor = { [weak self] color in self?.canvas = color; self?.paintUnderKeys(); self?.onCanvasColor?(color) }
        session.presenter.onTitle = { [weak self] title in self?.onTitle?(title) }
        #if os(iOS)
        session.presenter.onStatusBar = { [weak self] choice in self?.onStatusBarStyle?(choice.style) }
        #endif
        #if !os(tvOS)
        editingObservers = [UITextField.textDidBeginEditingNotification, UITextView.textDidBeginEditingNotification,
                            UITextField.textDidEndEditingNotification, UITextView.textDidEndEditingNotification].map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated {
                    // Reconcile after UIKit finishes handing focus to the next responder
                    self?.setNeedsLayout()
                }
            }
        }
        #endif
        // While the scene is in the background UIKit can size the primary
        // container for the other display (the iPhone Duo's inner one while
        // its cover shows), and nothing lays it out again on return:
        // `fitOwner` puts it back.
        sceneObservers = [UIScene.willEnterForegroundNotification, UIScene.didActivateNotification].map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, let scene = note.object as? UIScene, scene === self.window?.windowScene else { return }
                    self.session.presenter.navigation.fitOwner()
                }
            }
        }
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
        (editingObservers + sceneObservers).forEach(NotificationCenter.default.removeObserver)
    }

    /// A zero-size dependent makes UIKit lay this view out as its keyboard
    /// guide moves: frame notifications alone omit interactive drag frames.
    /// It comes with the first keyboard, not before: a constraint anywhere in
    /// the window gives the window a layout engine, and every view added
    /// after joins it (`_switchToLayoutEngine:`), which a list pays for each
    /// view of each row it builds. No node view uses a constraint.
    #if !os(tvOS)
    func installKeyboardProbe() {
        guard keyboardProbe == nil else { return }
        keyboardLayoutGuide.usesBottomSafeArea = false
        let probe = UIView()
        keyboardProbe = probe
        probe.isHidden = true
        probe.translatesAutoresizingMaskIntoConstraints = false
        addSubview(probe)
        NSLayoutConstraint.activate([
            probe.topAnchor.constraint(equalTo: keyboardLayoutGuide.topAnchor),
            probe.leadingAnchor.constraint(equalTo: leadingAnchor),
            probe.widthAnchor.constraint(equalToConstant: 0),
            probe.heightAnchor.constraint(equalToConstant: 0),
        ])
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
    public override var canBecomeFirstResponder: Bool { session.presenter.needsKeyboardFocus && !session.presenter.menus.focus.suspended }
    public override var keyCommands: [UIKeyCommand]? { session.presenter.shortcutCommands(#selector(exactShortcut(_:))) }
    @objc private func exactShortcut(_ command: UIKeyCommand) { session.presenter.performShortcut(command) }
    // This view holds the focus while nothing else does: a hardware key that
    // no shortcut took reaches the first root's `keycapture`, `key` and
    // `keyup` handlers, and ⌘X, ⌘C and ⌘V its `cut`, `copy` and `paste`,
    // as the DOM's `window` listeners hear them at `<body>` (`keyRoot`).
    public override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        if isFirstResponder, let key = presses.first?.key, let root = session.presenter.keyRoot, root.hardwareKey(key, down: true) { return }
        super.pressesBegan(presses, with: event)
    }
    public override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        if isFirstResponder, let key = presses.first?.key, let root = session.presenter.keyRoot { _ = root.hardwareKey(key, down: false) }
        super.pressesEnded(presses, with: event)
    }
    private func rootHearer(_ action: Selector) -> NodeView? {
        guard isFirstResponder, let root = session.presenter.keyRoot, root.clipboardHearer(action)?.node === root else { return nil }
        return root
    }
    public override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if [#selector(copy(_:)), #selector(cut(_:)), #selector(paste(_:))].contains(action) { return rootHearer(action) != nil }
        return super.canPerformAction(action, withSender: sender)
    }
    public override func copy(_ sender: Any?) { rootHearer(#selector(copy(_:)))?.clipboard(#selector(copy(_:))) }
    public override func cut(_ sender: Any?) { rootHearer(#selector(cut(_:)))?.clipboard(#selector(cut(_:))) }
    public override func paste(_ sender: Any?) { rootHearer(#selector(paste(_:)))?.clipboard(#selector(paste(_:))) }
    #endif

    public override func willMove(toWindow newWindow: UIWindow?) {
        // Child didMoveToWindow callbacks can retry focus before our own
        // didMoveToWindow. Wait until their native owners have been installed.
        if newWindow != nil { session.presenter.navigation.willMount() }
        if newWindow !== window {
            session.natives.windowLeaving(window) // its window hatch ends while the window is there
        }
        super.willMove(toWindow: newWindow)
    }

    public override func didMoveToWindow() {
        super.didMoveToWindow()
        ExactApp.shared.refreshDevPills() // in the window it joined, or another's (DevPill.swift)
        // The window it joined takes the accent; one it left has its own back
        session.presenter.accent.apply()
        if window != nil { reportScheme() }
        #if os(iOS)
        if window != nil { session.presenter.resolveOrientationLock() } // a lock told before there was a window
        #endif
        session.tellPage() // `hasFocus` is this window's scene's (#114)
        session.natives.scopesChanged()
        session.rasters.setPaused(window == nil)
        session.canvases.refreshLifecycle()
        if window == nil {
            session.presenter.menus.unmounted()
            session.presenter.modals.unmounted()
            session.presenter.navigation.unmounted()
            if session.state != .destroyed { _ = fitKeyboard(in: session.presenter.pageFrame) }
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
                syncKeyboardFocus()
                #endif
            }
        }
        // Mounted and visible participate in frame demand (D3): an unmounted
        // view wants no frames; a mounted one asks again.
        session.frames.run(window != nil && (session.frames.motion || session.canvases.wantsFrames))
    }

    /// The viewport's bands past the page's area (LLP 1130 D1); the document
    /// moves inside them and keeps its offset, as the area does.
    private func setPageMargins(_ margins: UIEdgeInsets) {
        let presenter = session.presenter
        guard presenter.pageMargins != margins else { return }
        presenter.pageMargins = margins
        presenter.fitDocument()
        presenter.reachPageEdges()
    }

    /// Keep the simulated fold in window coordinates so keyboard dismissal
    /// restores its full segments (`prefer`, LLP 1078 D7)
    private(set) var agentFold: ViewportFold?

    /// Apply the simulated fold under the current keyboard, retaining the
    /// previous fold if the kernel refuses it
    func prefer(_ fold: ViewportFold) -> String? {
        let was = (agentFold, lastFold)
        (agentFold, lastFold) = (fold, underKeyboard(fold, size: lastSize))
        guard let refused = session.segments(lastFold) else { return nil }
        (agentFold, lastFold) = was
        return refused
    }

    /// Native routes keep the window's division regions. An ordinary page clips
    /// them to its layout viewport, or only the last row under visual resizing.
    private func underKeyboard(_ fold: ViewportFold, size: CGSize) -> ViewportFold {
        if session.presenter.navigation.wantsWholeView { return fold }
        let presenter = session.presenter
        let visual = !["resizes-content", "overlays-content"].contains(presenter.keyboardPolicy)
        return fold.ending(at: visual ? CGSize(width: size.width, height: size.height - presenter.keyboardInset) : size, lastRow: visual)
    }

    /// Read the session guide at layout. Sheets read their own guide in OwnerBlocks.
    private func fitKeyboard(in frame: CGRect) -> (top: CGFloat?, changed: Bool) {
        let presenter = session.presenter
        let oldTop = presenter.keyboardTop, oldOverlap = presenter.keyboardInset
        #if !os(tvOS)
        if presenter.hasKeyboardEditor { installKeyboardProbe() }
        #endif
        let top = presenter.keyboardGuideTop(in: self)
        presenter.keyboardTop = top.map { convert(CGPoint(x: 0, y: $0), to: window).y }
        let overlap = presenter.menus.sheets.top == nil
            ? top.map { min(max(0, frame.maxY - max($0, frame.minY)), frame.height) } ?? 0 : 0
        presenter.setKeyboardInset(overlap)
        presenter.keyboardToolbars.ride(overlap: overlap)
        if top == nil { presenter.pinnedRoom.release() }
        return (top, oldTop != presenter.keyboardTop || oldOverlap != presenter.keyboardInset)
    }

    /// Fit the session to its safe area and derive keyboard avoidance from its guide.
    /// Native containers stay full size; their editing route owns avoidance.
    /// An ordinary page resizes its viewport or insets its content.
    func fit(boot: Bool = true) {
        guard session.state != .destroyed else { return }
        let presenter = session.presenter
        // A native callback served for the owner cannot resize the runtime
        // yet. Keep the latest geometry pending until that callback returns;
        // nor can a restart's reset (a keyboard it ends). While a restart holds
        // the runtime (its own reset or another session's), the fit is owed,
        // the last-told geometry untouched: ResetGate replays it after the batch.
        if session.runtime.holds("view(_:)") { return }
        if Owner.shared.isServingMain || presenter.resetting {
            if !fitPending {
                fitPending = true
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    self.fitPending = false
                    guard self.session.state != .destroyed else { return }
                    self.fit()
                }
            }
            return
        }
        // Containment may synchronously lay us out during a partial batch.
        // Finish that batch, then resize synchronously before returning to UIKit.
        if presenter.applying {
            if !fitPending {
                fitPending = true
                presenter.afterBatch { [weak self] in
                    guard let self else { return }
                    self.fitPending = false
                    guard self.session.state != .destroyed else { return }
                    self.fit()
                }
            }
            return
        }
        guard bounds.width > 0, bounds.height > 0 else { return }
        // @ref LLP 1123 D1 — the document is the session view's; a
        // presentation lays its routes out in their own blocks (D2)
        let safe = safeAreaInsets
        let cover = presenter.viewportFit == "cover" || (!session.booted && session.launchCoversViewport)
        // A native primary container takes the whole view, its bars shown
        // or not, as UIKit's do: a bar's material reaches under the status
        // bar. Its routes keep their content inside its safe area through
        // covers (LLP 1075.003 §3.5); `env()` stays what the page authored.
        let whole = cover || presenter.navigation.wantsWholeView
        var frame = whole ? bounds : bounds.inset(by: safe)
        var insets = cover ? safe : .zero
        let keyboard = fitKeyboard(in: frame)
        if presenter.keyboardPolicy == "resizes-content", !presenter.navigation.wantsWholeView,
           let top = keyboard.top, top < frame.maxY, presenter.menus.sheets.top == nil {
            frame.size.height = max(0, top - frame.minY)
            insets.bottom = 0
        }
        guard frame.width > 0, frame.height > 0 else { return }
        // A page with no container keeps its area (`frame`) and is drawn in a
        // viewport that takes the whole view, past the status bar and the
        // home indicator, which its content scrolls under (LLP 1130 D1); not
        // past keys that end the area (`resizes-content`), nor on tvOS,
        // whose safe area is the overscan margin.
        var physical = frame
        #if os(iOS)
        if !whole {
            let bottom = frame.maxY < bounds.inset(by: safe).maxY - 0.5 ? frame.maxY : bounds.maxY
            physical = CGRect(x: bounds.minX, y: bounds.minY, width: bounds.width, height: bottom - bounds.minY)
        }
        #endif
        #if os(iOS)
        if physical.maxY < bounds.maxY {
            physical.size.height += min(presenter.keyboardAccessoryHeight, bounds.maxY - physical.maxY)
        }
        #endif
        // Display changes can leave logical bounds unchanged. Update resources
        // independently of the resize commit, from this view's local traits.
        // The raster budget follows this view's own bounds — the panel it
        // covers — never the frame a keyboard clips the viewport to: a
        // keyboard rising is not a smaller display, and a shrink trims
        // the cold cache and halves queued decodes in place.
        let scale = max(1, traitCollection.displayScale)
        session.rasters.fit(size: bounds.size, scale: scale)
        if scale != lastDisplayScale {
            lastDisplayScale = scale
            session.apply(session.runtime.canvasDisplay(scale: scale, memory: ProcessInfo.processInfo.physicalMemory))
            session.rasters.displayChanged()
            session.tellDisplayScale() // a hairline is one device pixel of this display (LLP 1115.000 D10.3)
            session.presenter.regridAll()
        }
        var size = frame.size // the page's area, whatever the viewport's frame
        // LLP 1075.003 §9.11: the window, the document's frame before the
        // keyboard trims it, which the viewport units and the segments' count,
        // tops and lefts read
        let screenFrame = whole ? bounds : bounds.inset(by: safe)
        var screen: CGSize? = screenFrame.size
        // The agent's explicit viewport size is shared with web/macOS/Linux.
        // Fit those logical points into the device window; hit testing and
        // captures still use the viewport's own coordinate system.
        let env = ExactEnv.environment
        var regions: [CGFloat]? = nil
        if env["EXACT_AGENT"] == "1", let width = Double(env["EXACT_WINDOW_WIDTH"] ?? ""),
           let height = Double(env["EXACT_WINDOW_HEIGHT"] ?? ""),
           width.isFinite, height.isFinite, width > 0, height > 0 {
            size = CGSize(width: width, height: height)
            screen = size
            let scale = min(frame.width / size.width, frame.height / size.height)
            presenter.viewport.transform = CGAffineTransform(scaleX: scale, y: scale)
            presenter.viewport.bounds = CGRect(origin: .zero, size: size)
            presenter.viewport.center = CGPoint(x: frame.midX, y: frame.midY)
            insets = .zero
            setPageMargins(.zero)
        } else {
            presenter.viewport.transform = .identity
            if presenter.viewport.frame != physical { presenter.viewport.frame = physical }
            setPageMargins(UIEdgeInsets(top: frame.minY - physical.minY, left: frame.minX - physical.minX,
                                        bottom: physical.maxY - frame.maxY, right: physical.maxX - frame.maxX))
            // UIKit's layout regions from the viewport (LLP 1123 D10), or,
            // without `cover`, from its safe area, as `env()` is zero then
            // even where a native container takes the whole view; none under
            // the agent's explicit size, where the insets are zero too
            regions = LayoutRegionsIOS.insets(of: self, viewport: cover ? frame : frame.intersection(bounds.inset(by: safe)))
        }
        presenter.placeDocumentMount()
        guard size.width > 0, size.height > 0 else { return }
        // The fold (LLP 1078 D5): the container's active division regions,
        // converted into the viewport's space, split it into segments; the
        // posture is folded while any is active or the hinge says it is
        // partially open. Below 27.1 there are none.
        let bent = hingeStatus == 2
        // The window's, not a sheet's (LLP 1075.003 §9.11), or the agent's
        // on a device with none, under the keyboard.
        let fold = Self.fold(of: self, viewport: screenFrame, size: screen ?? size, hingeBent: bent)
        if fold.hasFold { presenter.hasFold = true }
        // Discard an override accepted before UIKit first reported the hardware fold
        if presenter.hasFold { agentFold = nil }
        let segments = underKeyboard(agentFold ?? fold.fold, size: size)
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
            presenter.insets = insets
            lastFold = segments
            guard boot else { return }
            session.boot(size: size, fallbackSize: {
                // Refusal can change the root policy before the bake settles.
                self.fit(boot: false)
                return self.lastSize
            })
            // The first batch made the roots: one that covers the screen is
            // framed to it now, before anything is drawn. A refused boot made
            // none, and fitting it again would boot again, without end.
            if session.booted { fit() }
            return
        }
        // The window's size, the insets, the regions and the viewport in one
        // batch: after a boot under a tab bar all four change at once.
        var change = Runtime.ViewChange()
        if screen != lastScreen {
            lastScreen = screen
            change.screen = .some(screen)
        }
        if insets != lastInsets {
            lastInsets = insets
            presenter.insets = insets
            change.insets = (insets.top, insets.right, insets.bottom, insets.left)
        }
        if regions ?? [] != lastRegions {
            lastRegions = regions ?? []
            presenter.regions = regions
            change.regions = .some(regions)
        }
        let resized = size != lastSize
        if resized { lastSize = size; change.size = size }
        // Rects that change on the same grid (a keyboard ending the
        // segments, a resize) are told with the viewport, in its one layout;
        // a new posture or grid goes through `segments`. The kernel refuses
        // only rects off its grid, which this one, `lastFold`'s, never is.
        var keyed: ViewportFold?
        if segments != lastFold, (segments.posture, segments.cols, segments.rows) == (lastFold.posture, lastFold.cols, lastFold.rows) {
            keyed = segments; lastFold = segments
        }
        session.view(change, fold: keyed)
        if change.insets != nil || resized { session.natives.scopesChanged() }
        if resized {
            // The keys came or went: the tab bar stays under them, and the
            // field being edited is shown in what they leave (LLP 1116.003
            // D2.1). Usually already done in the keyboard's animation; this
            // catches a guide that moved after its notification.
            presenter.navigation.fitOwner()
        }
        if segments != lastFold {
            lastFold = segments
            session.segments(segments)
        }
        // Ownership can change without changing the document or sheet size
        presenter.owners.publish()
        if keyboard.changed || resized {
            presenter.menus.rehostPopovers()
            presenter.revealEditorAsTheKeysMove()
        }
        paintUnderKeys()
    }

    /// What shows below the viewport while the keyboard ends it above the
    /// screen's bottom (`resizes-content`): behind the keyboard's accessory
    /// bar (the pads' ± and Done, LLP 1116.003 D7), which UIKit draws with no
    /// fill of its own, the colour the content at the viewport's bottom edge
    /// goes on with, a grouped list's background as a hand-built screen's
    /// list shows there, never the canvas's band across the width (LLP
    /// 1116.006 D6).
    private func paintUnderKeys() {
        let edge = session.presenter.keyboardInset > 0 ? session.presenter.bottomEdgeColor() : nil
        let color = edge ?? canvas
        if let color, backgroundColor != color { backgroundColor = color }
        // A sheet's own surface is what shows there while one is up (LLP
        // 1116.008 D8: a number pad's Done bar over a grouped form's sheet
        // drew white across the list's grey): the list's colour goes on
        // under the keys, and the sheet's own surface returns with them.
        #if os(iOS)
        (session.presenter.modals.coordinateView?.next as? PaintsSheetSurface)?.paintSurface()
        #endif
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
        // A new runner has no screen: tell it again first (Grok's review).
        if let screen = lastScreen { session.view(Runtime.ViewChange(screen: .some(screen))) }
        if lastInsets != .zero { session.insets(top: lastInsets.top, right: lastInsets.right, bottom: lastInsets.bottom, left: lastInsets.left) }
        lastRegions = nil  // the next fit tells the new runner
        session.segments(lastFold)
        reportScheme()
        fit()
    }
}
#endif

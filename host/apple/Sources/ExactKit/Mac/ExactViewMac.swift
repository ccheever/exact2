// The view that presents a session on AppKit (LLP 1031 D1, D3): an
// ordinary NSView the containing app places with Auto Layout or frames.
// Its bounds are the session's viewport; a bounds change is the existing
// resize path; the safe-area insets (the titlebar under
// `viewport-fit=cover`) are its own container's, sent through the existing
// `env(safe-area-inset-*)` path. Bounded containment: the session's page
// scrolls inside this view exactly as the standalone host's does.
#if os(macOS)
import AppKit
import AVFoundation

extension ExactSession {
    /// The page's canvas colour (the first root's background): what a
    /// full-size-content window paints behind its titlebar.
    public var pageBackground: NSColor { presenter.pageBackground }
}

public final class ExactView: NSView {
    public let session: ExactSession
    public override func selectAll(_ sender: Any?) { session.presenter.selection.selectAll() }
    /// The first root's `copy` with nothing focused (`RootEdits`), else the text selection's.
    @objc public func copy(_ sender: Any?) { if !rootEdits.perform(#selector(NodeView.copy(_:))) { session.presenter.selection.copy() } }
    /// Edit ▸ Cut, Copy and Paste with nothing focused, after the window in its responder chain.
    private lazy var rootEdits: RootEdits = { let edits = RootEdits(); edits.presenter = session.presenter; return edits }()
    /// Edit ▸ Speech over the selected text, as a browser speaks a page's
    /// selection (#141); a field's `NSTextView` answers first while it has
    /// the focus. Spoken Content's voice and rate, as AppKit's own.
    @objc public func startSpeaking(_ sender: Any?) {
        let text = session.presenter.selection.selectedText()
        guard !text.isEmpty else { return }
        let utterance = AVSpeechUtterance(string: text)
        utterance.prefersAssistiveTechnologySettings = true
        Self.speech.stopSpeaking(at: .immediate)
        Self.speech.speak(utterance)
    }
    @objc public func stopSpeaking(_ sender: Any?) { Self.speech.stopSpeaking(at: .immediate) }
    private static let speech = AVSpeechSynthesizer()
    private var lastSize = CGSize.zero
    private var fitPending = false
    private var lastDisplayScale: CGFloat = 0
    private var shortcutMonitor: Any?
    private var windowToolbarObservation: NSKeyValueObservation?
    private var toolbarVisibilityObservation: NSKeyValueObservation?
    private var geometryObservers: [NSObjectProtocol] = []
    private var insetsPending = false

    private func scheduleInsets() {
        guard !insetsPending else { return }
        insetsPending = true
        // AppKit finishes updating safe areas after its frame/toolbar notification.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.insetsPending = false
            guard self.session.state != .destroyed else { return }
            self.syncInsets()
        }
    }

    private func observeEnvironment() {
        geometryObservers.forEach(NotificationCenter.default.removeObserver)
        geometryObservers = []
        var ancestor: NSView? = self
        while let view = ancestor {
            view.postsFrameChangedNotifications = true
            view.postsBoundsChangedNotifications = true
            for name in [NSView.frameDidChangeNotification, NSView.boundsDidChangeNotification] {
                geometryObservers.append(NotificationCenter.default.addObserver(forName: name, object: view, queue: .main) { [weak self] _ in
                    self?.scheduleInsets()
                })
            }
            ancestor = view.superview
        }
        scheduleInsets()
    }

    public override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        observeEnvironment()
    }
    /// Screen changes and HDR suppression: HDR pictures re-plan (LLP 1100 D9).
    private var rangeObservers: [NSObjectProtocol] = []
    /// The adapter's hook for the first root's `viewport-fit` (window chrome
    /// is the window's business, LLP 1008 §9); the insets themselves are
    /// computed here.
    public var onViewportFit: (() -> Void)?
    /// The window's owner asks that its new window put the caret in the
    /// session's first key view (LLP 1115.000 D7.5), as AppKit does a
    /// window's `initialFirstResponder`: with Keyboard Navigation off the
    /// first text field, with none nothing. Done once, the turn after the
    /// first frame, unless the author's `autofocus` said where the focus
    /// goes or the first key view is in a canvas that takes input
    /// (`Presenter.focusInitialKeyView`). Off by default: a view embedded in
    /// someone else's window takes no focus they did not give it.
    public var focusesFirstKeyView = false

    /// The window owner opts in; embedded Exact views never claim chrome by
    /// mounting. Returns false rather than replacing an existing toolbar.
    @discardableResult public func attachWindowToolbar(to window: NSWindow) -> Bool {
        guard self.window === window else { return false }
        session.presenter.toolbar.onChange = { [weak self] in
            self?.onViewportFit?()
            self?.syncInsets()
        }
        return session.presenter.toolbar.attach(to: window)
    }
    public func detachWindowToolbar() { session.presenter.toolbar.detach() }

    /// Whether the session's root tab bar should be this view's window's
    /// sidebar now (LLP 1115.002 D3): the app has root tabs whose placement
    /// the author left unsaid, and this is not the agent's own chrome. The
    /// window's owner composes it (`ExactSidebarController`), as it opts in
    /// to the toolbar; a view mounted in someone else's window claims none.
    public var wantsWindowSidebar: Bool { session.presenter.sidebar.wanted }
    /// Whether a stack of the session shows its list beside its detail (LLP
    /// 1115.002 D4): the window's owner composes the same controller for it,
    /// with or without a sidebar, and a view in someone else's window keeps
    /// a stack that pushes.
    public var wantsWindowSplit: Bool { session.presenter.navigation.split.wanted }
    /// The owner hears when `wantsWindowSidebar` or `wantsWindowSplit`
    /// changes, after the batch that changed it.
    public var onWindowSidebar: (() -> Void)? {
        get { session.presenter.sidebar.onWanted }
        set { session.presenter.sidebar.onWanted = newValue; session.presenter.navigation.split.onWanted = newValue }
    }
    /// The window's owner put `window-controls-overlay`'s toolbar in the
    /// title bar (LLP 1039 D8): the title-bar area is beside the traffic
    /// lights in its band, and the display mode is that overlay.
    public var controlsOverlay = false {
        didSet { if controlsOverlay != oldValue { session.tellPreferences() } }
    }
    public var hasWindowToolbar: Bool {
        guard let toolbar = session.presenter.toolbar.toolbar else { return false }
        return window?.toolbar === toolbar
    }

    public init(session: ExactSession) {
        // Under the agent its light is the app's before this view reads an
        // appearance, which a windowless view keeps: the control text is
        // primed from it before the first layout (LLP 1104 D4).
        _ = Agent.systemAppearance
        self.session = session
        super.init(frame: .zero)
        let viewport = session.presenter.viewport
        viewport.frame = bounds
        viewport.autoresizingMask = [.width, .height]
        addSubview(viewport)
        session.view = self
        viewport.visibleSizeChanged = { [weak self] in self?.fit() }
        session.presenter.onViewportFit = { [weak self] in
            self?.syncInsets()
            self?.onViewportFit?()
        }
        var names: [Notification.Name] = [NSWindow.didChangeScreenNotification, NSApplication.didChangeScreenParametersNotification,
                                          NSWindow.didEnterFullScreenNotification, NSWindow.didExitFullScreenNotification]
        if #available(macOS 26, *) {
            names += [.NSApplicationShouldBeginSuppressingHighDynamicRangeContent,
                      .NSApplicationShouldEndSuppressingHighDynamicRangeContent]
        }
        rangeObservers = names.map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, note.object == nil || note.object as AnyObject? === self.window || note.object is NSApplication else { return }
                    self.session.tellPreferences()
                    self.session.rasters.displayChanged()
                    // The title bar leaves the window in full screen and comes back after it.
                    if note.name == NSWindow.didEnterFullScreenNotification || note.name == NSWindow.didExitFullScreenNotification { self.syncInsets() }
                }
            }
        }
    }

    /// Whether this session's shortcuts and `key` handlers hear the window's
    /// keys: its view holds the focus, or nothing does — the window itself is
    /// the first responder — as a page's shortcuts hear keys with no element
    /// focused (jukebox F11). Of several sessions in one window, the first in
    /// view order hears them then.
    func ownsShortcutFocus(in other: NSWindow? = nil) -> Bool {
        // A sheet's window, or a popover's, holds this session's nodes alone (SheetsMac.swift).
        if let other, other !== window { return self.session.presenter.hosts(other) }
        let first = window?.firstResponder
        let responder = first as? NSView
        let editorOwner = (responder as? NSTextView)?.delegate as? NSView
        if responder?.isDescendant(of: self) == true || editorOwner?.isDescendant(of: self) == true { return true }
        // In the pane that holds this session's detail beside its list (LLP 1115.002 D4).
        let split = self.session.presenter.navigation.split
        if split.holdsFocus(responder) || split.holdsFocus(editorOwner) { return true }
        // This session's search field, which the window's toolbar hosts (LLP 1115.002 D2).
        if let node = self.session.presenter.keyTarget(first), node.pageField != nil { return true }
        guard let window, first == nil || first === window, let content = window.contentView else { return false }
        func session(in view: NSView) -> ExactView? {
            if let found = view as? ExactView { return found }
            for sub in view.subviews { if let found = session(in: sub) { return found } }
            return nil
        }
        return session(in: content) === self
    }

    /// The trackpad's swipe back from a pushed route (LLP 1115.000 D5.1,
    /// `BackSwipe`): the page's scroll view passes a sideways gesture up
    /// here when it is pinned for it and this says a swipe may be tracked.
    public override func wantsScrollEventsForSwipeTracking(on axis: NSEvent.GestureAxis) -> Bool {
        axis == .horizontal && session.presenter.navigation.canGoBack
    }
    public override func scrollWheel(with event: NSEvent) {
        if !session.presenter.backSwipe.scroll(event) { super.scrollWheel(with: event) }
    }
    public override func swipe(with event: NSEvent) {
        if !session.presenter.backSwipe.swiped(deltaX: event.deltaX) { super.swipe(with: event) }
    }

    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if session.presenter.menus.key(event) || session.presenter.dialogs.key(event) { return true }
        if ownsShortcutFocus(), session.presenter.shortcuts.perform(event) { return true }
        return super.performKeyEquivalent(with: event)
    }

    deinit {
        if let shortcutMonitor { NSEvent.removeMonitor(shortcutMonitor) }
        rangeObservers.forEach(NotificationCenter.default.removeObserver)
        geometryObservers.forEach(NotificationCenter.default.removeObserver)
        session.presenter.menus.reset()
        session.presenter.dialogs.reset()
        session.presenter.toolbar.detach()
    }

    required init?(coder: NSCoder) { nil }

    /// The first root's `viewport-fit` prop (`"cover"` or nothing).
    public var viewportFit: String? { session.presenter.viewportFit }

    public override func layout() {
        super.layout()
        fit()
    }

    /// A frame change from autoresizing does not run `layout()`; this does.
    public override func resizeSubviews(withOldSize oldSize: NSSize) {
        super.resizeSubviews(withOldSize: oldSize)
        fit()
    }

    /// Boot at the first real size (an embedder's view), else resize; the
    /// insets follow.
    private func fit() {
        if Owner.shared.isServingMain {
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
        if session.presenter.deferGeometry({ [weak self] in self?.fit() }) { return }
        session.presenter.dialogs.layout()
        session.presenter.menus.layout()
        let size = session.presenter.viewportSize
        guard size.width > 0, size.height > 0 else { return }
        let scale = window?.backingScaleFactor ?? 1
        session.rasters.fit(size: size, scale: scale)
        if scale != lastDisplayScale {
            lastDisplayScale = scale
            session.apply(session.runtime.canvasDisplay(scale: scale, memory: ProcessInfo.processInfo.physicalMemory))
            session.rasters.displayChanged()
            session.tellDisplayScale() // a hairline is one device pixel of this display (LLP 1115.000 D10.3)
        }
        if !session.booted {
            // A refused boot is terminal (LLP 1031 D8): the session says why
            // in `bootError`, and a layout does not ask again.
            if case .failed = session.state { return }
            // An embedder's view boots the session at its first real size;
            // the standalone adapter booted it before the window showed.
            lastSize = size
            session.boot(size: size)
            syncInsets()
            return
        }
        if size != lastSize {
            lastSize = size
            session.resize(size)
            session.natives.scopesChanged()
        }
        syncInsets()
    }

    public override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        session.controlTextChanged()
        fit()
    }

    public override func viewWillMove(toWindow newWindow: NSWindow?) {
        if newWindow !== window { session.natives.windowLeaving(window); rootEdits.leave(window) }   // its window hatch ends while the window is there
        super.viewWillMove(toWindow: newWindow)
    }

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        observeEnvironment()
        toolbarVisibilityObservation = nil
        windowToolbarObservation = window?.observe(\.toolbar, options: [.initial, .new]) { [weak self] window, _ in
            guard let self else { return }
            self.toolbarVisibilityObservation = window.toolbar?.observe(\.isVisible, options: [.new]) { [weak self] _, _ in self?.scheduleInsets() }
            self.scheduleInsets()
        }
        session.natives.scopesChanged()
        if window == nil { session.presenter.menus.reset(); session.presenter.dialogs.reset() }
        session.tellPreferences()
        session.tellPage() // `hasFocus` is this window's (#114)
        session.rasters.setPaused(window == nil)
        session.canvases.refreshLifecycle()
        if session.presenter.toolbar.window !== window { session.presenter.toolbar.detach() }
        else { session.presenter.toolbar.sync() }
        if let shortcutMonitor { NSEvent.removeMonitor(shortcutMonitor); self.shortcutMonitor = nil }
        if let window { rootEdits.join(window) }
        if window != nil {
            // Text editors can consume control chords before the responder chain.
            // Route declared commands first, scoped to this session's focused view.
            shortcutMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged, .leftMouseDown, .leftMouseUp, .rightMouseDown, .rightMouseUp, .otherMouseUp]) { [weak self] event in
                guard let self else { return event }
                #if EXACT_INSPECT
                // Inspect mode's monitor has every event (InspectMac.swift): none is the app's.
                if Inspect.on || InspectHost.draining { return event }
                #endif
                // AppKit can route an up to no view after the pressed subtree leaves its window.
                if [.leftMouseUp, .rightMouseUp, .otherMouseUp].contains(event.type),
                   let source = self.session.presenter.pointerSource, source.window == nil {
                    source.pointerReleased(event)
                }
                guard self.session.presenter.hosts(event.window) else { return event }
                // A modifier pressed is a keydown on the web (`"Shift"`), and
                // released a keyup (#140); to AppKit a flags change, which
                // goes on to it either way.
                if event.type == .flagsChanged {
                    if self.ownsShortcutFocus(in: event.window) { _ = self.session.presenter.keyDown(event); self.session.presenter.keyUp(event) }
                    return event
                }
                if event.type != .keyDown && event.type != .keyUp {
                    // A click is a new place for Tab to start from (`tabStart`): the window's, as before.
                    if event.type == .leftMouseDown || event.type == .rightMouseDown {
                        // A press outside a field composing text commits it before the press runs, as Chrome's mousedown does.
                        self.session.presenter.commitCompositionBefore(event)
                        self.session.presenter.tabStart = nil
                        // A press on a held node, or in it, lets its hold go (HeldState.swift), whatever view takes it.
                        self.session.presenter.heldPointerDown(event)
                    }
                    self.session.presenter.menus.pointer(event)
                    // A click on the window behind a `closedby="any"` sheet is the sheet's.
                    return self.session.presenter.dialogs.pointer(event) ? nil : event
                }
                if event.type == .keyDown { self.session.presenter.flushKeyViewLoop() }
                let focused = self.ownsShortcutFocus(in: event.window)
                let code=KeyCodes.mac[Int(event.keyCode)] ?? "Unidentified"
                if focused, event.modifierFlags.intersection([.command,.control]).isEmpty,
                   self.session.canvases.pressedControlKey(code,down:event.type == .keyDown,timestamp:event.timestamp) {return nil}
                // Shortcuts, then the `key` handlers (before a field editor can
                // take the key), then the menus' and dialogs' defaults.
                return self.session.presenter.routeKey(event, focused: focused) ? nil : event
            }
        }
        // Mounted and visible participate in frame demand (D3): an unmounted
        // view wants no frames; a mounted one asks again.
        session.frames.run(window != nil && (session.frames.motion || session.canvases.wantsFrames))
        if window != nil { fit(); session.presenter.syncAccessibility() }
        // In a window's sidebar pane, or out of it: the sidebar stands for
        // the tablist only while it is in a window (SidebarMac.swift).
        session.presenter.sidebar.sync()
    }

    /// The insets the kernel gets: under `viewport-fit=cover` the view's own
    /// safe area (the titlebar, when the window's content includes it);
    /// zero otherwise. Then AppKit's layout regions (LLP 1123 D10) and the
    /// title-bar area beside the window's controls.
    public func syncInsets() {
        if session.presenter.deferGeometry({ [weak self] in self?.fit() }) { return }
        let next = viewportFit == "cover" ? safeAreaInsets : NSEdgeInsetsZero
        let prev = session.presenter.insets
        if next.top != prev.top || next.left != prev.left || next.bottom != prev.bottom || next.right != prev.right {
            session.presenter.insets = next
            session.insets(top: next.top, right: next.right, bottom: next.bottom, left: next.left)
        }
        guard session.booted else { return }
        let regions = LayoutRegionsMac.insets(of: self)
        if !regionsTold || regions != session.presenter.regions {
            regionsTold = true
            session.presenter.regions = regions
            session.regions(regions)
        }
        let area = TitleBarAreaMac.rect(of: self, cover: viewportFit == "cover" || controlsOverlay, overlay: controlsOverlay)
        if !titlebarTold || area != session.presenter.titlebarArea {
            titlebarTold = true
            session.presenter.titlebarArea = area
            session.titlebarArea(area)
        }
    }
    /// Whether the runner has the layout regions (LLP 1123 D10) and the title-bar area.
    private var regionsTold = false, titlebarTold = false

    /// After a restart from a new plan: the new runner knows nothing of the
    /// insets — hand them over again, and fit the root. AppKit has no fold
    /// (LLP 1078 D5): `continuous`, one segment, told once at every boot so
    /// the facts are never the bake's — or what an agent preferred.
    func rebooted() {
        let i = session.presenter.insets
        if i.top != 0 || i.left != 0 || i.bottom != 0 || i.right != 0 { session.insets(top: i.top, right: i.right, bottom: i.bottom, left: i.left) }
        regionsTold = false; titlebarTold = false  // the next fit tells the new runner
        session.segments(session.presenter.fold)
        viewDidChangeEffectiveAppearance()
        needsLayout = true
    }

    /// Paint motion resolves `light-dark()` by this view's appearance (LLP 1062).
    public override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        session.controlTextChanged()
        session.scheme(dark: effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua)
        // The app's `setScheme` arrives here, as the system's does; iOS's, through `tellPreferences` (LLP 1122 D4).
        session.app.scheduleStatus()
    }
}

extension ExactView: NSMenuItemValidation {
    /// Start Speaking with text selected, Stop Speaking while it speaks.
    public func validateMenuItem(_ item: NSMenuItem) -> Bool {
        switch item.action {
        case #selector(startSpeaking(_:)): !session.presenter.selection.selectedText().isEmpty
        case #selector(stopSpeaking(_:)): Self.speech.isSpeaking
        default: true
        }
    }
}
#endif

// The view that presents a session on AppKit (LLP 1031 D1, D3): an
// ordinary NSView the containing app places with Auto Layout or frames.
// Its bounds are the session's viewport; a bounds change is the existing
// resize path; the safe-area insets (the titlebar under
// `viewport-fit=cover`) are its own container's, sent through the existing
// `env(safe-area-inset-*)` path. Bounded containment: the session's page
// scrolls inside this view exactly as the standalone host's does.
#if os(macOS)
import AppKit

extension ExactSession {
    /// The page's canvas colour (the first root's background): what a
    /// full-size-content window paints behind its titlebar.
    public var pageBackground: NSColor { presenter.pageBackground }
}

public final class ExactView: NSView {
    public let session: ExactSession
    public override func selectAll(_ sender: Any?) { session.presenter.selection.selectAll() }
    @objc public func copy(_ sender: Any?) { session.presenter.selection.copy() }
    private var lastSize = CGSize.zero
    private var lastDisplayScale: CGFloat = 0
    private var shortcutMonitor: Any?
    /// Screen changes and HDR suppression: HDR pictures re-plan (LLP 1100 D9).
    private var rangeObservers: [NSObjectProtocol] = []
    /// The adapter's hook for the first root's `viewport-fit` (window chrome
    /// is the window's business, LLP 1008 §9); the insets themselves are
    /// computed here.
    public var onViewportFit: (() -> Void)?

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
    public var hasWindowToolbar: Bool {
        guard let toolbar = session.presenter.toolbar.toolbar else { return false }
        return window?.toolbar === toolbar
    }

    public init(session: ExactSession) {
        self.session = session
        super.init(frame: .zero)
        let viewport = session.presenter.viewport
        viewport.frame = bounds
        viewport.autoresizingMask = [.width, .height]
        addSubview(viewport)
        session.view = self
        session.presenter.onViewportFit = { [weak self] in
            self?.syncInsets()
            self?.onViewportFit?()
        }
        var names: [Notification.Name] = [NSWindow.didChangeScreenNotification, NSApplication.didChangeScreenParametersNotification]
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
                }
            }
        }
    }

    /// Whether this session's shortcuts and `key` handlers hear the window's
    /// keys: its view holds the focus, or nothing does — the window itself is
    /// the first responder — as a page's shortcuts hear keys with no element
    /// focused (jukebox F11). Of several sessions in one window, the first in
    /// view order hears them then.
    func ownsShortcutFocus() -> Bool {
        let first = window?.firstResponder
        let responder = first as? NSView
        let editorOwner = (responder as? NSTextView)?.delegate as? NSView
        if responder?.isDescendant(of: self) == true || editorOwner?.isDescendant(of: self) == true { return true }
        guard let window, first == nil || first === window, let content = window.contentView else { return false }
        func session(in view: NSView) -> ExactView? {
            if let found = view as? ExactView { return found }
            for sub in view.subviews { if let found = session(in: sub) { return found } }
            return nil
        }
        return session(in: content) === self
    }

    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if session.presenter.menus.key(event) || session.presenter.dialogs.key(event) { return true }
        if ownsShortcutFocus(), session.presenter.shortcuts.perform(event) { return true }
        return super.performKeyEquivalent(with: event)
    }

    deinit {
        if let shortcutMonitor { NSEvent.removeMonitor(shortcutMonitor) }
        rangeObservers.forEach(NotificationCenter.default.removeObserver)
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
        }
        if !session.booted {
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
        }
        syncInsets()
    }

    public override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        fit()
    }

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil { session.presenter.menus.reset(); session.presenter.dialogs.reset() }
        session.rasters.setPaused(window == nil)
        session.canvases.lifecycle.refresh()
        if session.presenter.toolbar.window !== window { session.presenter.toolbar.detach() }
        else { session.presenter.toolbar.sync() }
        if let shortcutMonitor { NSEvent.removeMonitor(shortcutMonitor); self.shortcutMonitor = nil }
        if window != nil {
            // Text editors can consume control chords before the responder chain.
            // Route declared commands first, scoped to this session's focused view.
            shortcutMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged, .leftMouseDown, .leftMouseUp, .rightMouseDown, .rightMouseUp]) { [weak self] event in
                guard let self, event.window === self.window else { return event }
                // A modifier pressed is a keydown on the web (`"Shift"`), and
                // released a keyup (#140); to AppKit a flags change, which
                // goes on to it either way.
                if event.type == .flagsChanged {
                    if self.ownsShortcutFocus() { _ = self.session.presenter.keyDown(event); self.session.presenter.keyUp(event) }
                    return event
                }
                if event.type != .keyDown && event.type != .keyUp {
                    self.session.presenter.menus.pointer(event)
                    return event
                }
                if event.type == .keyDown { self.session.presenter.flushKeyViewLoop() }
                let focused = self.ownsShortcutFocus()
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
    }

    /// The insets the kernel gets: under `viewport-fit=cover` the view's own
    /// safe area (the titlebar, when the window's content includes it);
    /// zero otherwise.
    public func syncInsets() {
        if session.presenter.deferGeometry({ [weak self] in self?.fit() }) { return }
        let next = viewportFit == "cover" ? safeAreaInsets : NSEdgeInsetsZero
        let prev = session.presenter.insets
        guard next.top != prev.top || next.left != prev.left || next.bottom != prev.bottom || next.right != prev.right else { return }
        session.presenter.insets = next
        session.insets(top: next.top, right: next.right, bottom: next.bottom, left: next.left)
    }

    /// After a restart from a new plan: the new runner knows nothing of the
    /// insets — hand them over again, and fit the root. AppKit has no fold
    /// (LLP 1078 D5): `continuous`, one segment, told once at every boot so
    /// the facts are never the bake's — or what an agent preferred.
    func rebooted() {
        let i = session.presenter.insets
        if i.top != 0 || i.left != 0 || i.bottom != 0 || i.right != 0 { session.insets(top: i.top, right: i.right, bottom: i.bottom, left: i.left) }
        session.segments(session.presenter.fold)
        viewDidChangeEffectiveAppearance()
        needsLayout = true
    }

    /// Paint motion resolves `light-dark()` by this view's appearance (LLP 1062).
    public override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        session.scheme(dark: effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua)
    }
}
#endif

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
    private var shortcutMonitor: Any?
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
    }

    private func ownsShortcutFocus() -> Bool {
        let responder = window?.firstResponder as? NSView
        let editorOwner = (responder as? NSTextView)?.delegate as? NSView
        return responder?.isDescendant(of: self) == true || editorOwner?.isDescendant(of: self) == true
    }

    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if ownsShortcutFocus(), session.presenter.shortcuts.perform(event) { return true }
        return super.performKeyEquivalent(with: event)
    }

    deinit {
        if let shortcutMonitor { NSEvent.removeMonitor(shortcutMonitor) }
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
        let size = session.presenter.viewportSize
        guard size.width > 0, size.height > 0 else { return }
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

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        session.rasters.setPaused(window == nil)
        session.canvases.lifecycle.refresh()
        if session.presenter.toolbar.window !== window { session.presenter.toolbar.detach() }
        else { session.presenter.toolbar.sync() }
        if let shortcutMonitor { NSEvent.removeMonitor(shortcutMonitor); self.shortcutMonitor = nil }
        if window != nil {
            // Text editors can consume control chords before the responder chain.
            // Route declared commands first, scoped to this session's focused view.
            shortcutMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp]) { [weak self] event in
                guard let self, event.window === self.window, self.ownsShortcutFocus() else { return event }
                let code=KeyCodes.mac[Int(event.keyCode)] ?? "Unidentified"
                if event.modifierFlags.intersection([.command,.control]).isEmpty,
                   self.session.canvases.pressedControlKey(code,down:event.type == .keyDown,timestamp:event.timestamp) {return nil}
                return event.type == .keyDown && self.session.presenter.shortcuts.perform(event) ? nil : event
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
    /// insets — hand them over again, and fit the root.
    func rebooted() {
        let i = session.presenter.insets
        if i.top != 0 || i.left != 0 || i.bottom != 0 || i.right != 0 { session.insets(top: i.top, right: i.right, bottom: i.bottom, left: i.left) }
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

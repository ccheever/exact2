#if os(macOS)
import AppKit

/// Lane r8-keys: the right panel's surface launcher ("Open a surface",
/// RightPanelTabs.tsx; MIT, see LICENSE-T3). The reference focuses it on mount
/// (`focusOnMount`), and its letter shortcuts work while it is visible unless
/// the focus is in a typing context: a window capture-phase handler runs
/// before the chat's type-to-focus. Hook `t3-launcher` marks the launcher;
/// `data-surface-launcher-keys` lists its available letters ("FLD").
final class R8KeysLauncher {
    private weak var element: ExactElement?
    private var observer: NSObjectProtocol?
    private var focused = false

    func install(_ element: ExactElement) {
        guard element.hatch == .t3Launcher else { return }
        if self.element !== element {
            detach()
            self.element = element
            focused = false
        }
        focusWhenLaidOut()
    }

    func remove(_ element: ExactElement) {
        guard self.element === element else { return }
        detach()
        self.element = nil
    }

    private func detach() {
        if let observer { NotificationCenter.default.removeObserver(observer) }
        observer = nil
    }

    /// The launcher's view once it is on screen with a size (it mounts inside a panel
    /// that is laid out after the batch that opened it).
    var liveView: NSView? {
        guard let element, element.isLive, let view = element.view, view.window != nil,
              !view.isHiddenOrHasHiddenAncestor, view.bounds.width > 0, view.bounds.height > 0 else { return nil }
        return view
    }

    private func focusWhenLaidOut() {
        guard !focused else { return }
        if let view = liveView {
            detach()
            focused = true
            DispatchQueue.main.async { [weak self] in self?.focusNow(view) }
            return
        }
        guard observer == nil, let view = element?.view else { return }
        view.postsFrameChangedNotifications = true
        observer = NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: view, queue: .main) { [weak self] _ in
            self?.focusWhenLaidOut()
        }
    }

    private func focusNow(_ view: NSView) {
        guard let window = view.window, liveView === view else { return }
        element?.focus()
        if window.firstResponder !== view, view.acceptsFirstResponder { window.makeFirstResponder(view) }
    }

    /// The letters the launcher answers, upper-cased.
    var keys: Set<Character> { Set((element?.data[.surfaceLauncherKeys] ?? "").uppercased()) }

    /// What the window's key monitor does with a letter the visible launcher answers.
    enum Route: Equatable {
        /// Not the launcher's: the key goes on (type-to-focus may take it).
        case none
        /// The launcher holds the focus: the key goes on, unredirected, to Exact's key route, whose
        /// `key` handlers at the launcher open the surface (and prevent the key's default).
        case pass
        /// The focus was elsewhere: the launcher took it and the key was posted again at the head
        /// of the queue, so Exact's key route hears it at the launcher; this copy goes no further.
        case taken
    }

    /// The copy `route` posted again (a real key's timestamp is its own); it is never posted a second time.
    private var reposted: (timestamp: TimeInterval, keyCode: UInt16, window: Int)?

    /// A plain letter the visible launcher answers, typed outside any typing context, is the
    /// launcher's (the reference's window capture listener). Its `key` handlers run only from
    /// Exact's own key monitor, never from a view's `keyDown`, and AppKit calls local monitors in
    /// no fixed order (it changes as monitors come and go), so the letter is never consumed here:
    /// it reaches Exact's route with the launcher focused, whichever monitor runs first.
    func route(_ event: NSEvent, typing: Bool) -> Route {
        guard event.type == .keyDown, !typing, !event.isARepeat, let view = liveView, let window = view.window, event.window === window,
              window.attachedSheet == nil,
              event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
              let characters = event.charactersIgnoringModifiers, characters.count == 1,
              let letter = characters.uppercased().first, keys.contains(letter),
              // Exact's hit test refuses an inert or hidden node: a dialog over the panel blocks it.
              view.hitTest(NSPoint(x: view.frame.midX, y: view.frame.midY)) != nil else { return .none }
        // The mark lasts for one launcher letter: Exact's route may take the copy before it comes back here.
        let copy = (event.timestamp, event.keyCode, event.windowNumber)
        var again = false
        if let reposted { again = reposted == copy }
        reposted = nil
        if window.firstResponder === view { return .pass }
        if again { return .none }
        window.makeFirstResponder(view)
        guard window.firstResponder === view else { return .none }
        reposted = copy
        NSApp.postEvent(event, atStart: true)
        return .taken
    }

    deinit { detach() }
}
#endif

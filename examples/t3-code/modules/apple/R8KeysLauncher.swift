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
        guard element.hook == .t3Launcher else { return }
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

    /// A plain letter the visible launcher answers, typed outside any typing context:
    /// the launcher takes the focus and the key (its own `key` handler opens the surface).
    func consume(_ event: NSEvent, typing: Bool) -> Bool {
        guard event.type == .keyDown, !typing, !event.isARepeat, let view = liveView, let window = view.window, event.window === window,
              window.attachedSheet == nil,
              event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
              let characters = event.charactersIgnoringModifiers, characters.count == 1,
              let letter = characters.uppercased().first, keys.contains(letter),
              // Exact's hit test refuses an inert or hidden node: a dialog over the panel blocks it.
              view.hitTest(NSPoint(x: view.frame.midX, y: view.frame.midY)) != nil else { return false }
        if window.firstResponder !== view { window.makeFirstResponder(view) }
        guard window.firstResponder === view else { return false }
        view.keyDown(with: event)
        return true
    }

    deinit { detach() }
}
#endif

// The desktop shell's context menu over selected text (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/window/DesktopWindow.ts `installContextMenu`): a right-click the page leaves to the shell shows
// Cut, Copy, Paste and Select All, each enabled by the page's edit flags. Over read-only text with a selection that
// is Cut and Paste disabled, Copy and Select All enabled (realinput-1010d RD-4).
//
// ExactKit answers a secondary click on selected text that wrote no `contextmenu` with the menu an NSTextView shows
// for read-only text (Look Up, Copy, Speech, Services; LLP 1115 D8). The T3 desktop shell writes its own menu, so the
// window answers that click with the shell's. Contract has no window-wide context-menu hook (Electron's
// `context-menu` event on the renderer), so a local monitor takes the right-click wherever ExactKit would show its
// text menu (its menu carries the Look Up action) and pops the shell's in its place, on the same text view: Copy is
// that view's `copy:` (the page's `copy` event first, then the selection, as ⌘C) and Select All its `selectAll:`.
#if os(macOS)
import AppKit

final class T3TextContextMenu: NSObject {
    /// ExactKit's read-only text menu's Look Up item (TextSelectionMac.swift `lookUpSelection(_:)`).
    static let hostTextMenuAction = NSSelectorFromString("lookUpSelection:")

    private let agent: Bool
    private var monitor: Any?
    /// What the last replaced click showed (the agent's log line, and the tests).
    private(set) var lastShown: [String] = []
    /// Pops the shell's menu (tracks until the menu closes); a test records it instead.
    var present: (NSMenu, NSEvent, NSView) -> Void = { NSMenu.popUpContextMenu($0, with: $1, for: $2) }

    init(agent: Bool) { self.agent = agent }

    func install() {
        guard monitor == nil else { return }
        // Not `self?.handle(event) ?? event`: optional chaining flattens handle's `NSEvent?`, so its nil ("taken") would
        // turn back into the event and ExactKit's text menu would pop after the shell's.
        monitor = NSEvent.addLocalMonitorForEvents(matching: .rightMouseDown) { [weak self] event in
            guard let self else { return event }
            return self.handle(event)
        }
    }

    func destroy() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }

    /// The click goes on unless ExactKit would answer it with its text menu; then the shell's menu pops up there. Under
    /// the agent nothing pops up (its window is never key, so the menu would track until a real click, as
    /// T3ContextMenu.perform notes): the menu's items go to the log instead.
    func handle(_ event: NSEvent) -> NSEvent? {
        guard let (menu, view) = Self.replacement(for: event) else { return event }
        lastShown = Self.describe(menu)
        if agent {
            FileHandle.standardError.write(Data("t3.textmenu: \(lastShown.joined(separator: " | "))\n".utf8))
            return nil
        }
        present(menu, event, view)
        return nil
    }

    /// The shell's menu for this click and the text view it acts on, or nil where ExactKit shows no text menu (no
    /// selection under the pointer, a field, a node that wrote its own `contextmenu`, another window's view).
    static func replacement(for event: NSEvent) -> (NSMenu, NSView)? {
        guard event.type == .rightMouseDown, let window = event.window, let content = window.contentView else { return nil }
        let point = content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow
        guard let view = content.hitTest(point), let hostMenu = view.menu(for: event),
              hostMenu.items.contains(where: { $0.action == hostTextMenuAction }) else { return nil }
        return (menu(for: view), view)
    }

    /// DesktopWindow's template for read-only text with a selection: the four editing roles, with the accelerators
    /// Electron shows for them.
    static func menu(for view: NSView) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        let roles: [(String, Selector, String, Bool)] = [
            ("Cut", #selector(NSText.cut(_:)), "x", false),
            ("Copy", #selector(NSText.copy(_:)), "c", true),
            ("Paste", #selector(NSText.paste(_:)), "v", false),
            ("Select All", #selector(NSResponder.selectAll(_:)), "a", true),
        ]
        for (title, action, key, enabled) in roles {
            let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
            item.keyEquivalentModifierMask = .command
            item.target = view
            item.isEnabled = enabled
            menu.addItem(item)
        }
        return menu
    }

    static func describe(_ menu: NSMenu) -> [String] {
        menu.items.map { $0.isSeparatorItem ? "—" : $0.isEnabled ? $0.title : "\($0.title) (disabled)" }
    }
}
#endif

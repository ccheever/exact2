#if os(macOS)
import AppKit

/// Lane r8-keys: the menu bar as the reference desktop app builds it
/// (apps/desktop/src/window/DesktopApplicationMenu.ts; MIT, see LICENSE-T3), laid
/// over the host's.
///
/// - File holds only Close Window (⌘W). The host also files every button that
///   declares a ⌘ chord there; those items stay as hidden key equivalents.
/// - Edit holds the reference's items alone: since exact2 #226 the host files a
///   ⌘F, ⌘D or ⌘G button there after Select All (⇧⌘G is the branch picker), so each
///   stays a hidden key equivalent too; a command standing in for one of Edit's own
///   items (the app's Undo ⌘Z) keeps its place.
/// - View starts with Reload (⌘R) and Force Reload (⇧⌘R), which reload the window,
///   as the reference's roles reload the page. The host's Develop menu (Reload ⌘R,
///   Open Project… ⌘O, App Info… ⌘D) and its Go menu are not part of the reference
///   bar: they go, so ⌘D (diff.toggle) and ⌘O (editor.openFavorite) reach the window.
/// - The reference renderer sees a keystroke before Electron's menu: ⌘Z is
///   thread.undo while no editable text has the focus, ⌘W closes an open right panel
///   before it closes the window. AppKit keeps one key equivalent per chord across
///   the bar (a later item with a different action loses it), so Edit › Undo and
///   File › Close Window route a keystroke to the window's command first.
final class R8KeysMenus: NSObject, NSMenuDelegate, NSMenuItemValidation {
    private var reloadAction: Selector?
    private weak var reloadTarget: AnyObject?
    private weak var file: NSMenu?
    private weak var edit: NSMenu?
    private var logMonitor: Any?
    /// The host's command items (ShortcutsMac): titled by their button's label.
    static let undoTitle = "Undo", closePanelTitle = "Close Right Panel"

    func arrange(_ bar: NSMenu) {
        if let develop = bar.items.first(where: { $0.submenu?.title == "Develop" }) {
            if let reload = develop.submenu?.items.first(where: { $0.keyEquivalent == "r" && $0.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask) == .command }) {
                reloadAction = reload.action
                reloadTarget = reload.target as AnyObject?
            }
            bar.removeItem(develop)
        }
        if let go = bar.items.first(where: { $0.submenu?.title == "Go" }) { bar.removeItem(go) }
        // One window, no window tabs: AppKit's Show Tab Bar / Show All Tabs are not in the reference View menu.
        NSWindow.allowsAutomaticWindowTabbing = false
        if let view = bar.items.first(where: { $0.submenu?.title == "View" })?.submenu {
            for item in view.items where item.action == #selector(NSWindow.toggleTabBar(_:)) || item.action == #selector(NSWindow.toggleTabOverview(_:)) { view.removeItem(item) }
        }
        if reloadAction != nil, let view = bar.items.first(where: { $0.submenu?.title == "View" })?.submenu, view.item(withTitle: "Reload") == nil {
            let reload = NSMenuItem(title: "Reload", action: #selector(reloadWindow(_:)), keyEquivalent: "r")
            let force = NSMenuItem(title: "Force Reload", action: #selector(reloadWindow(_:)), keyEquivalent: "r")
            force.keyEquivalentModifierMask = [.command, .shift]
            for (index, item) in [reload, force, .separator()].enumerated() {
                item.target = self
                view.insertItem(item, at: index)
            }
        }
        if let edit = bar.items.first(where: { $0.submenu?.title == "Edit" })?.submenu,
           let undo = edit.items.first(where: { $0.keyEquivalent == "z" && $0.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask) == .command }) {
            undo.action = #selector(undo(_:))
            undo.target = self
        }
        if let edit = bar.items.first(where: { $0.submenu?.title == "Edit" })?.submenu {
            self.edit = edit
            if edit.delegate == nil || edit.delegate === self { edit.delegate = self }
            concealCommands(edit)
        }
        if let file = bar.items.first(where: { $0.submenu?.title == "File" })?.submenu {
            self.file = file
            if file.delegate == nil || file.delegate === self { file.delegate = self }
            if let close = file.items.first(where: { $0.keyEquivalent == "w" && $0.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask) == .command }) {
                close.action = #selector(closeWindow(_:))
                close.target = self
            }
            conceal(file)
        }
        startLog()
    }

    /// Everything but Close Window is a hidden key equivalent.
    func conceal(_ menu: NSMenu) {
        for item in menu.items where item.action != #selector(closeWindow(_:)) {
            if !item.isHidden { item.isHidden = true }
            item.allowsKeyEquivalentWhenHidden = true
        }
    }
    func menuNeedsUpdate(_ menu: NSMenu) { if menu === file { conceal(menu) } else if menu === edit { concealCommands(menu) } }

    /// Edit's app commands, but one standing in for Edit's own (Undo ⌘Z, Redo ⇧⌘Z, Cut,
    /// Copy, Paste, Select All), are hidden key equivalents; a separator the hidden ones
    /// leave beside another (or first) hides with them.
    func concealCommands(_ menu: NSMenu) {
        for item in menu.items where Self.isCommand(item) && !Self.standsIn(item) {
            if !item.isHidden { item.isHidden = true }
            item.allowsKeyEquivalentWhenHidden = true
        }
        var afterSeparator = true
        for item in menu.items {
            if item.isSeparatorItem {
                if item.isHidden != afterSeparator { item.isHidden = afterSeparator }
                afterSeparator = true
            } else if !item.isHidden { afterSeparator = false }
        }
    }
    static func isCommand(_ item: NSMenuItem) -> Bool { item.target.map { String(describing: type(of: $0)) == "ShortcutHost" } == true }
    static func standsIn(_ item: NSMenuItem) -> Bool {
        let key = item.keyEquivalent.lowercased(), mask = item.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask)
        return (mask == .command && ["z", "x", "c", "v", "a"].contains(key)) || (mask == [.command, .shift] && key == "z")
    }

    /// The host's command item for a button label in File (hidden there), or nil.
    func command(_ title: String) -> NSMenuItem? {
        file?.items.first { item in
            item.title == title && item.action != nil && item.target.map { String(describing: type(of: $0)) == "ShortcutHost" } == true
        }
    }
    private func fire(_ item: NSMenuItem) -> Bool {
        guard let action = item.action, let target = item.target,
              (target as? NSMenuItemValidation)?.validateMenuItem(item) ?? true else { return false }
        return NSApp.sendAction(action, to: target, from: item)
    }
    /// Whether the menu action came from a keystroke (the tests inject it).
    var keystroke: () -> Bool = { NSApp.currentEvent?.type == .keyDown }
    /// The window Close Window closes (the tests inject it).
    var closeTarget: () -> NSWindow? = { NSApp.keyWindow ?? NSApp.mainWindow }
    /// An editable text view with something to undo holds the focus.
    private static var textUndo: Bool {
        guard let text = NSApp.keyWindow?.firstResponder as? NSTextView, text.isEditable else { return false }
        return text.undoManager?.canUndo == true
    }

    @objc func undo(_ sender: Any?) {
        if Self.textUndo { NSApp.sendAction(Selector(("undo:")), to: nil, from: sender); return }
        if keystroke(), let thread = command(Self.undoTitle), fire(thread) { return }
        NSApp.sendAction(Selector(("undo:")), to: nil, from: sender)
    }
    @objc func closeWindow(_ sender: Any?) {
        if keystroke(), let close = command(Self.closePanelTitle), fire(close) { return }
        closeTarget()?.performClose(sender)
    }
    @objc func reloadWindow(_ sender: Any?) {
        guard let reloadAction else { return }
        NSApp.sendAction(reloadAction, to: reloadTarget, from: sender)
    }

    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        switch item.action {
        case #selector(undo(_:)):
            if Self.textUndo { return true }
            guard let thread = command(Self.undoTitle) else { return false }
            return (thread.target as? NSMenuItemValidation)?.validateMenuItem(thread) ?? true
        case #selector(closeWindow(_:)): return closeTarget() != nil || command(Self.closePanelTitle) != nil
        case #selector(reloadWindow(_:)): return reloadAction != nil
        default: return true
        }
    }

    // MARK: Diagnostics (R8_KEYS_LOG=<file>): each ⌘ keystroke, the focus and the bar's matching items.

    private func startLog() {
        guard logMonitor == nil, let path = ProcessInfo.processInfo.environment["R8_KEYS_LOG"], !path.isEmpty else { return }
        let write: (String) -> Void = { line in
            guard let data = (line + "\n").data(using: .utf8) else { return }
            if let handle = FileHandle(forWritingAtPath: path) { handle.seekToEndOfFile(); handle.write(data); handle.closeFile() }
            else { FileManager.default.createFile(atPath: path, contents: data) }
        }
        logMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard event.modifierFlags.contains(.command) else { return event }
            let key = (event.charactersIgnoringModifiers ?? "").lowercased()
            var matches: [String] = []
            func walk(_ menu: NSMenu, _ path: String) {
                for item in menu.items {
                    if let sub = item.submenu { walk(sub, path + "/" + item.title); continue }
                    if item.keyEquivalent.lowercased() == key || item.title.hasPrefix("Thread") || item.title == "Undo" || item.title.hasPrefix("New") {
                        matches.append("\(path)/\(item.title)[\(item.keyEquivalent)|\(item.keyEquivalentModifierMask.rawValue)|on=\(item.isEnabled)|hid=\(item.isHidden)]")
                    }
                }
            }
            if let bar = NSApp.mainMenu { walk(bar, "") }
            let responder = event.window?.firstResponder.map { String(describing: type(of: $0)) } ?? "nil"
            write("\(Date().timeIntervalSince1970) key=\(key) flags=\(event.modifierFlags.intersection(.deviceIndependentFlagsMask).rawValue) responder=\(responder) key-window=\(NSApp.keyWindow === event.window) items=\(matches.joined(separator: " "))")
            return event
        }
        NotificationCenter.default.addObserver(forName: NSMenu.didSendActionNotification, object: nil, queue: .main) { note in
            let item = note.userInfo?["MenuItem"] as? NSMenuItem
            write("\(Date().timeIntervalSince1970) sent \(item?.title ?? "?") target=\(item?.target.map { String(describing: type(of: $0)) } ?? "nil")")
        }
    }
}
#endif

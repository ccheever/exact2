// The standalone Mac menu bar: standard application and window commands,
// with app-declared shortcuts in File, Edit, View, Go and Settings by their
// chords (ShortcutsMac.swift; studio diary R16). File — New Window ⌘N
// exists under `launch_handler` `navigate-new` (LLP 1069.010 D4), Open… ⌘O
// when the app declares documents it opens (LLP 1033);
// Edit always (the field editor's command keys — ⌘A/X/C/V/Z — are menu
// equivalents, not key bindings; without this they are dead); Develop —
// Reload ⌘R, Open Project… ⇧⌘O behind a document app, App Info… ⌘D —
// unless EXACT_DEV_MENU=0. Native AppKit above the presenter, so it is
// alive even when the plan is broken; reload re-fetches a live connection,
// else restarts from the dev loop's plan when one is named (the watcher's
// own path, state carried), else from the baked plan, fresh. Host
// apparatus for the standalone adapter (LLP 1031 D11): an embedder installs
// none of this.
#if os(macOS)
import AppKit

final class DevMenuTarget: NSObject {
    @objc func reload(_ sender: Any?) { DevMenu.reload() }
    @objc func info(_ sender: Any?) { DevMenu.showInfo() }
    @objc func saveTrace(_ sender: Any?) { DevMenu.saveTrace() }
    @objc func openProject(_ sender: Any?) { DevMenu.openProject() }
    @objc func openDocument(_ sender: Any?) { DevMenu.openDocument() }
}

/// Select All on a focused field. A secure field editor can ignore
/// `selectAll:` (Weird Castle's password); forcing the range still
/// highlights so the next key replaces. Anything else (a WKWebView)
/// gets the action through the responder chain.
final class EditMenuTarget: NSObject {
    @objc func selectAll(_ sender: Any?) {
        let r = NSApp.keyWindow?.firstResponder
        if let editor = r as? NSText {
            editor.selectAll(sender)
            if editor.selectedRange.length == 0 {
                let n = (editor.string as NSString).length
                if n > 0 { editor.selectedRange = NSRange(location: 0, length: n) }
            }
            return
        }
        if let field = r as? NSTextField {
            field.selectText(sender)
            return
        }
        NSApp.sendAction(#selector(NSText.selectAll(_:)), to: nil, from: sender)
    }
}

/// File ▸ Open Recent without `NSDocument` (LLP 1069.010 D5): the list is
/// `NSDocumentController`'s own — what `noteNewRecentDocumentURL` adds to,
/// what the Dock's menu shows, what "Clear Menu" empties — drawn here with
/// public API. AppKit fills its own recent menu only for one a nib names
/// (or one named through a private selector, which the Mac App Store
/// refuses; ruled: nothing here may make that harder). Choosing an entry is
/// one more route in, to the same router as Launch Services' (LLP 1033 D3).
final class RecentMenu: NSObject, NSMenuDelegate {
    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let urls = NSDocumentController.shared.recentDocumentURLs
        let names = urls.map { FileManager.default.displayName(atPath: $0.path) }
        for (url, name) in zip(urls, names) {
            // Two of the same name say which folder each is in, as AppKit's
            // own menu does.
            let title = names.filter { $0 == name }.count > 1 ? "\(name) — \(url.deletingLastPathComponent().lastPathComponent)" : name
            let item = menu.addItem(withTitle: title, action: #selector(open(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = url
            let icon = NSWorkspace.shared.icon(forFile: url.path)
            icon.size = NSSize(width: 16, height: 16)
            item.image = icon
        }
        if !urls.isEmpty { menu.addItem(.separator()) }
        let clear = menu.addItem(withTitle: "Clear Menu", action: #selector(NSDocumentController.clearRecentDocuments(_:)), keyEquivalent: "")
        clear.target = NSDocumentController.shared
    }

    @objc func open(_ sender: NSMenuItem) {
        guard let url = sender.representedObject as? URL else { return }
        let path = url.standardizedFileURL.path
        if let route = ExactDocuments.route { route([path]) }
        else if let session = DevMenu.session { ExactDocuments.deliver([path], to: session) }
    }
}

public enum DevMenu {
    static let target = DevMenuTarget()
    static let editTarget = EditMenuTarget()
    public static var enabled: Bool { ExactEnv.environment["EXACT_DEV_MENU"] != "0" }
    /// The session the menu reloads, describes and opens into: the key
    /// window's, which the adapter that owns the windows keeps current.
    nonisolated(unsafe) public static weak var session: ExactSession?
    /// Where a reload without a connection restarts from: the dev loop's
    /// plan file (`EXACT_DEV_PLAN`) or `EXACT_PLAN`, else the baked plan.
    nonisolated(unsafe) static var planPath: String?

    public static func install(session: ExactSession, planPath: String?) {
        DevMenu.session = session
        DevMenu.planPath = planPath
        NSApp.mainMenu = makeMenu(shortcuts: session.presenter.shortcuts, documents: ExactDocuments.declared)
    }

    static func makeMenu(shortcuts: ShortcutHost, documents: Bool) -> NSMenu {
        let bar = NSMenu()
        let appItem = NSMenuItem()
        bar.addItem(appItem)
        let appMenu = ShortcutMenu(title: ExactEnv.appName)
        appMenu.addItem(withTitle: "About \(ExactEnv.appName)", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(.separator())
        let services = NSMenu(title: "Services")
        appMenu.addItem(withTitle: "Services", action: nil, keyEquivalent: "").submenu = services
        NSApp.servicesMenu = services
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Hide \(ExactEnv.appName)", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        let hideOthers = appMenu.addItem(withTitle: "Hide Others", action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthers.keyEquivalentModifierMask = [.command, .option]
        appMenu.addItem(withTitle: "Show All", action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: "")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit \(ExactEnv.appName)", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appItem.submenu = appMenu
        let fileItem = NSMenuItem()
        bar.addItem(fileItem)
        let file = ShortcutMenu(title: "File")
        fileItem.submenu = file
        // ⌘N and the tab bar's + open an empty window with a session of its
        // own (LLP 1069.010 D4), whether or not the app opens documents:
        // `newWindowForTab:` goes up the responder chain to the adapter that
        // owns the windows.
        if ExactDocuments.launchMode == "navigate-new" {
            file.addItem(withTitle: "New Window", action: #selector(NSResponder.newWindowForTab(_:)), keyEquivalent: "n")
        }
        // ⌘O belongs to the app's own documents when it declares any
        // (`file_handlers`, LLP 1033) — that is what ⌘O means on this
        // platform, and Develop ▸ Open Project… takes ⇧⌘O behind it.
        if documents {
            file.addItem(withTitle: "Open…", action: #selector(DevMenuTarget.openDocument(_:)), keyEquivalent: "o").target = target
            file.addItem(withTitle: "Open Recent", action: nil, keyEquivalent: "").submenu = recentMenu()
        }
        if !file.items.isEmpty { file.addItem(.separator()) }
        file.addItem(withTitle: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        // AppKit does not bind ⌘A itself (`StandardKeyBinding.dict` has no
        // `selectAll`); the Edit menu is how a field hears select-all, cut,
        // copy, paste, and undo.
        let editItem = NSMenuItem()
        bar.addItem(editItem)
        let edit = ShortcutMenu(title: "Edit")
        edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        let redo = edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "z")
        redo.keyEquivalentModifierMask = [.command, .shift]
        edit.addItem(.separator())
        edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "Delete", action: #selector(NSText.delete(_:)), keyEquivalent: "")
        let selectAll = edit.addItem(withTitle: "Select All", action: #selector(EditMenuTarget.selectAll(_:)), keyEquivalent: "a")
        selectAll.target = editTarget
        editItem.submenu = edit
        let view = ShortcutMenu(title: "View")
        let fullScreen = view.addItem(withTitle: "Enter Full Screen", action: #selector(NSWindow.toggleFullScreen(_:)), keyEquivalent: "f")
        fullScreen.keyEquivalentModifierMask = [.command, .control]
        bar.addItem(withTitle: "View", action: nil, keyEquivalent: "").submenu = view
        let go = ShortcutMenu(title: "Go")
        bar.addItem(withTitle: "Go", action: nil, keyEquivalent: "").submenu = go
        let window = NSMenu(title: "Window")
        window.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        window.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        window.addItem(.separator())
        window.addItem(withTitle: "Bring All to Front", action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: "")
        bar.addItem(withTitle: "Window", action: nil, keyEquivalent: "").submenu = window
        NSApp.windowsMenu = window
        shortcuts.attach(file, application: appMenu, navigation: go, edit: edit, view: view)
        if enabled {
            let devItem = NSMenuItem()
            bar.addItem(devItem)
            let dev = NSMenu(title: "Develop")
            dev.addItem(withTitle: "Reload", action: #selector(DevMenuTarget.reload(_:)), keyEquivalent: "r").target = target
            let project = dev.addItem(withTitle: "Open Project…", action: #selector(DevMenuTarget.openProject(_:)), keyEquivalent: "o")
            project.target = target
            if documents { project.keyEquivalentModifierMask = [.command, .shift] }
            dev.addItem(withTitle: "App Info…", action: #selector(DevMenuTarget.info(_:)), keyEquivalent: "d").target = target
            // LLP 1079 D5: the session's journal, frames and work, for the agent (`agent.mjs trace <file>`).
            let trace = dev.addItem(withTitle: "Save Trace", action: #selector(DevMenuTarget.saveTrace(_:)), keyEquivalent: "t")
            trace.keyEquivalentModifierMask = [.command, .option]
            trace.target = target
            devItem.submenu = dev
        }
        return bar
    }

    /// File ▸ Open Recent (LLP 1069.010 D5): `NSDocumentController`'s
    /// list, drawn by `RecentMenu` each time the menu opens.
    static let recent = RecentMenu()
    static func recentMenu() -> NSMenu {
        let menu = NSMenu(title: "Open Recent")
        menu.delegate = recent
        recent.menuNeedsUpdate(menu)
        return menu
    }

    /// What File ▸ Open Recent shows now, drawn as it is when the menu
    /// opens: its items' titles (the agent's `state`, LLP 1069.010).
    public static var openRecentTitles: [String] {
        guard let menu = NSApp.mainMenu?.items.lazy.compactMap({ $0.submenu?.item(withTitle: "Open Recent")?.submenu }).first
        else { return [] }
        menu.delegate?.menuNeedsUpdate?(menu)
        return menu.items.map { $0.isSeparatorItem ? "—" : $0.title }
    }

    /// The menu bar as drawn, for the agent's `state` (studio diary R16):
    /// each menu's shown items, with the chord each answers (`⇧⌘Z`).
    public static var menuBar: [[String: Any]] {
        (NSApp.mainMenu?.items ?? []).dropFirst().compactMap(\.submenu).map { menu in
            menu.delegate?.menuNeedsUpdate?(menu)
            return ["title": menu.title, "items": menu.items.filter { !$0.isHidden }.map { item -> String in
                if item.isSeparatorItem { return "—" }
                guard !item.keyEquivalent.isEmpty else { return item.title }
                let m = item.keyEquivalentModifierMask
                let chord = (m.contains(.control) ? "⌃" : "") + (m.contains(.option) ? "⌥" : "") + (m.contains(.shift) ? "⇧" : "") + (m.contains(.command) ? "⌘" : "")
                return "\(item.title) \(chord)\(item.keyEquivalent.uppercased())"
            }]
        }
    }

    /// The typed URL — the affordance a physical device actually uses
    /// (LLP 1023 Stage 1): seeded with the last value, kept in defaults.
    static func openProject() {
        let alert = NSAlert()
        alert.messageText = "Open Project"
        alert.informativeText = "The app URL the dev server printed."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 280, height: 24))
        field.stringValue = UserDefaults.standard.string(forKey: "exact.dev.url")
            ?? ExactApp.shared.connectedPage?.absoluteString ?? "http://"
        alert.accessoryView = field
        alert.addButton(withTitle: "Connect")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let url = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !url.isEmpty else { return }
        UserDefaults.standard.set(url, forKey: "exact.dev.url")
        ExactApp.shared.connect(url)
    }

    /// File ▸ Open… — the app's own documents (LLP 1033), not a project.
    static func openDocument() {
        ExactDocuments.open(into: session)
    }

    public static func reload() {
        // A live URL session re-fetches (and clears a rebuilt stop).
        if ExactApp.shared.connectionStatus != nil { ExactApp.shared.reloadConnection(); return }
        guard let session else { return }
        if let path = planPath {
            ExactDevelopmentPlan(path).apply(to: ExactApp.shared)
            return
        }
        let started = CACurrentMediaTime()
        let batch = session.boot(size: session.presenter.viewportSize)
        // stderr, never stdout: stdout is the agent's reply channel on macOS,
        // and a reply-shaped line there answers the driver's next request.
        fputs("exact: reloaded in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")\n", stderr)
    }

    static func info() -> String {
        var lines = [CommandLine.arguments[0]]
        if let status = ExactApp.shared.connectionStatus { lines.append("url: \(status)") }
        if let path = ExactEnv.environment["EXACT_DEV_PLAN"] {
            let m = (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date
            lines.append("plan: \(path)\(m.map { " (\(time.string(from: $0)))" } ?? " (missing)")")
        } else if let path = ExactEnv.environment["EXACT_PLAN"] {
            lines.append("plan: \(path)")
        } else {
            lines.append("plan: baked")
        }
        lines.append("app dir: \(ExactApp.shared.assetRoot.path)")
        if let session {
            let size = session.presenter.viewportSize
            lines.append("session: \(session.label) · viewport: \(Int(size.width))×\(Int(size.height)) · \(session.viewCount) views")
            lines.append("boot: \(String(format: "%.1f", session.bootMs)) ms")
        }
        return lines.joined(separator: "\n")
    }

    static func showInfo() {
        let text = info()
        let alert = NSAlert()
        alert.messageText = "Exact"
        alert.informativeText = text
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Copy")
        if alert.runModal() == .alertSecondButtonReturn {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
    }

    /// Save Trace (LLP 1079 D5), ⌥⌘T: where it went, or why not.
    static func saveTrace() {
        guard let session else { return }
        let alert = NSAlert()
        alert.messageText = "Trace"
        switch session.saveTrace() {
        case .success(let url):
            alert.informativeText = url.path
            alert.addButton(withTitle: "OK")
            alert.addButton(withTitle: "Show in Finder")
            if alert.runModal() == .alertSecondButtonReturn { NSWorkspace.shared.activateFileViewerSelecting([url]) }
        case .failure(let error):
            alert.informativeText = "Not saved: \(error)"
            alert.runModal()
        }
    }

    static let time: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()
}
#endif

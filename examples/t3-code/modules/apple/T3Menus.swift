#if os(macOS)
import AppKit
import UniformTypeIdentifiers

/// The reference desktop menu (apps/desktop/src/window/DesktopApplicationMenu.ts, 74541bab17) over the
/// host's: Edit gains Paste as Text (⇧⌘V), the responder chain's `pasteAsPlainText:` as the reference's
/// pasteAndMatchStyle is: a field's `paste`, which the composer arms as Paste as Text by its key; chosen with the
/// pointer, the item arms it here first (`pasteAsTextArmed`, the reference's DESKTOP_PASTE_AS_TEXT_EVENT). An
/// `-exact-menu` button's item is a press, not a responder-chain action, so this item stays native; the
/// root's `-exact-menu-remove="paste-and-match-style"` takes the host's ⌥⇧⌘V, which names that action, so this
/// item sends it from its own selector. The rest of the bar is Contract's: Check for Updates… (settings-shortcuts
/// UpdateMenuItems), View's zoom (the host's), the quit shortcut's hold (quit-hold.contract) and the removals
/// (app-window.contract). What stays here is the parentless "Updates unavailable" box (a Contract `alertdialog`
/// is a sheet on the window) and the folder and favicon panels.
final class T3Menus: NSObject, NSMenuItemValidation {
    private weak var window: NSWindow?
    /// Why this build cannot check for updates, or nil when it links an update store.
    var updatesDisabledReason: () -> String? = { T3Menus.updatesDisabledReason() }
    /// How the "Updates unavailable" box is shown (the tests record it instead).
    var presentAlert: (NSAlert) -> Void = { alert in T3MenuTurn.run { _ = alert.runModal() } }

    func install(_ element: ExactElement) {
        guard element.hatch == .t3Composer, let window = element.view?.window else { return }
        self.window = window
        augment(NSApp.mainMenu)
    }

    /// Adds Paste as Text; safe to repeat after the host rebuilds the bar.
    func augment(_ bar: NSMenu?) {
        guard let bar else { return }
        if let edit = bar.items.first(where: { $0.submenu?.title == "Edit" })?.submenu, edit.item(withTitle: "Paste as Text") == nil {
            let paste = edit.indexOfItem(withTitle: "Paste")
            let item = NSMenuItem(title: "Paste as Text", action: #selector(pasteAsText(_:)), keyEquivalent: "v")
            item.keyEquivalentModifierMask = [.command, .shift]
            item.target = self
            edit.insertItem(item, at: paste >= 0 ? paste + 1 : edit.numberOfItems)
            // The reference's Edit has a separator between Delete and Select All, where the host's has none.
            let selectAll = edit.indexOfItem(withTitle: "Select All")
            if selectAll > 0, edit.item(at: selectAll - 1)?.isSeparatorItem == false { edit.insertItem(.separator(), at: selectAll) }
        }
    }

    @objc func pasteAsText(_ sender: Any?) {
        armedUntil = ProcessInfo.processInfo.systemUptime + 1
        NSApp.sendAction(#selector(NSTextView.pasteAsPlainText(_:)), to: nil, from: sender)
    }

    /// Paste as Text armed by the menu item for one second (armPasteAsTextShortcut's deadline): TypeScript reads it for
    /// the paste it holds or the files it attaches (`composerPasteAsText`), once (consumePasteAsTextShortcut).
    private var armedUntil: TimeInterval = 0
    func takePasteAsText(now: TimeInterval = ProcessInfo.processInfo.systemUptime) -> Bool {
        defer { armedUntil = 0 }
        return now <= armedUntil
    }

    // MARK: Check for Updates…

    /// DesktopUpdates getAutoUpdateDisabledReason: a bundle without an update store (its
    /// receipt's composition "embedded", app.json deploy.store "0") has no update feed.
    static func updatesDisabledReason(receipt: URL? = Bundle.main.url(forResource: "receipt", withExtension: "json")) -> String? {
        let unavailable = "Automatic updates are not available because no update feed is configured."
        guard let receipt, let data = try? Data(contentsOf: receipt),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return unavailable }
        return json["composition"] as? String == "updating" ? nil : unavailable
    }
    /// handleCheckForUpdatesMenuClick's disabled branch: an informational box, no parent window.
    static func updatesUnavailableAlert(_ reason: String) -> NSAlert {
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "Automatic updates are not available right now."
        alert.informativeText = reason
        alert.addButton(withTitle: "OK")
        return alert
    }
    @objc func checkForUpdates(_ sender: Any?) {
        // A build with an update store would run the host's check; this module has no hook into it.
        guard let reason = updatesDisabledReason() else { return }
        presentAlert(Self.updatesUnavailableAlert(reason))
    }
    /// Enabled while some responder up the key window's chain takes `pasteAsPlainText:`, as AppKit validates the action.
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard item.action == #selector(pasteAsText(_:)) else { return true }
        return NSApp.target(forAction: #selector(NSTextView.pasteAsPlainText(_:)), to: nil, from: item) != nil
    }

    /// Open in Finder: the native folder picker, starting where the palette is browsing.
    func pickFolder(startingAt path: String, completion: @escaping (String?) -> Void) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        panel.prompt = "Add Project"
        let expanded = (path as NSString).expandingTildeInPath
        var isDirectory: ObjCBool = false
        if !expanded.isEmpty, FileManager.default.fileExists(atPath: expanded, isDirectory: &isDirectory) {
            panel.directoryURL = URL(fileURLWithPath: isDirectory.boolValue ? expanded : (expanded as NSString).deletingLastPathComponent)
        }
        let finish: (NSApplication.ModalResponse) -> Void = { response in completion(response == .OK ? panel.url?.path : nil) }
        if let window { panel.beginSheetModal(for: window, completionHandler: finish) } else { finish(panel.runModal()) }
    }

    /// PROJECT_FAVICON_EXTENSIONS (packages/shared/src/projectFavicon.ts), without the dot.
    static let faviconExtensions = ["avif", "gif", "icns", "ico", "jpeg", "jpg", "png", "svg", "webp"]

    /// pickProjectFavicon (apps/desktop/src/ipc/methods/window.ts): one image file, starting in the
    /// project's workspace root, as a sheet on the main window. The panel is built by `faviconPanel`
    /// so the tests can read its configuration.
    static func faviconPanel(startingAt path: String) -> NSOpenPanel {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = faviconExtensions.compactMap { UTType(filenameExtension: $0) }
        var isDirectory: ObjCBool = false
        let expanded = (path as NSString).expandingTildeInPath
        if !expanded.isEmpty, FileManager.default.fileExists(atPath: expanded, isDirectory: &isDirectory) {
            panel.directoryURL = URL(fileURLWithPath: isDirectory.boolValue ? expanded : (expanded as NSString).deletingLastPathComponent, isDirectory: true)
        }
        return panel
    }
    /// The picked file's absolute path, or nil on cancel. Under the agent no panel opens: the
    /// isolated data root's imports/ gives its first image file, so a drive can pick without one.
    func pickProjectFavicon(startingAt path: String, importsRoot: URL?, completion: @escaping (String?) -> Void) {
        if let importsRoot {
            let files = (try? FileManager.default.contentsOfDirectory(at: importsRoot, includingPropertiesForKeys: nil)) ?? []
            return completion(files.filter { Self.faviconExtensions.contains($0.pathExtension.lowercased()) }.map(\.path).sorted().first)
        }
        let panel = Self.faviconPanel(startingAt: path)
        let finish: (NSApplication.ModalResponse) -> Void = { response in completion(response == .OK ? panel.url?.path : nil) }
        if let window { panel.beginSheetModal(for: window, completionHandler: finish) } else { finish(panel.runModal()) }
    }

    func destroy() {}
}
#endif

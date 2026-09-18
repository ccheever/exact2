// The standalone Mac menu bar: standard application and window commands,
// with app-declared shortcuts in File, Go and Settings. File — Open… ⌘O
// exists when the app declares documents it opens (LLP 1033);
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

public enum DevMenu {
    static let target = DevMenuTarget()
    static let editTarget = EditMenuTarget()
    public static var enabled: Bool { ExactEnv.environment["EXACT_DEV_MENU"] != "0" }
    /// The session the menu reloads and describes.
    nonisolated(unsafe) static weak var session: ExactSession?
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
        let appMenu = NSMenu(title: ExactEnv.appName)
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
        let file = NSMenu(title: "File")
        fileItem.submenu = file
        // ⌘O belongs to the app's own documents when it declares any
        // (`file_handlers`, LLP 1033) — that is what ⌘O means on this
        // platform, and Develop ▸ Open Project… takes ⇧⌘O behind it.
        if documents {
            file.addItem(withTitle: "Open…", action: #selector(DevMenuTarget.openDocument(_:)), keyEquivalent: "o").target = target
            file.addItem(.separator())
        }
        file.addItem(withTitle: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        // AppKit does not bind ⌘A itself (`StandardKeyBinding.dict` has no
        // `selectAll`); the Edit menu is how a field hears select-all, cut,
        // copy, paste, and undo.
        let editItem = NSMenuItem()
        bar.addItem(editItem)
        let edit = NSMenu(title: "Edit")
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
        let view = NSMenu(title: "View")
        let fullScreen = view.addItem(withTitle: "Enter Full Screen", action: #selector(NSWindow.toggleFullScreen(_:)), keyEquivalent: "f")
        fullScreen.keyEquivalentModifierMask = [.command, .control]
        bar.addItem(withTitle: "View", action: nil, keyEquivalent: "").submenu = view
        let go = NSMenu(title: "Go")
        bar.addItem(withTitle: "Go", action: nil, keyEquivalent: "").submenu = go
        let window = NSMenu(title: "Window")
        window.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        window.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        window.addItem(.separator())
        window.addItem(withTitle: "Bring All to Front", action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: "")
        bar.addItem(withTitle: "Window", action: nil, keyEquivalent: "").submenu = window
        NSApp.windowsMenu = window
        shortcuts.attach(file, application: appMenu, navigation: go)
        if enabled {
            let devItem = NSMenuItem()
            bar.addItem(devItem)
            let dev = NSMenu(title: "Develop")
            dev.addItem(withTitle: "Reload", action: #selector(DevMenuTarget.reload(_:)), keyEquivalent: "r").target = target
            let project = dev.addItem(withTitle: "Open Project…", action: #selector(DevMenuTarget.openProject(_:)), keyEquivalent: "o")
            project.target = target
            if documents { project.keyEquivalentModifierMask = [.command, .shift] }
            dev.addItem(withTitle: "App Info…", action: #selector(DevMenuTarget.info(_:)), keyEquivalent: "d").target = target
            devItem.submenu = dev
        }
        return bar
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
        guard let session else { return }
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
        print("reloaded in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")")
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

    static let time: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()
}
#endif

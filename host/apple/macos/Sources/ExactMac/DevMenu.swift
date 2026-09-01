// The dev menu, the macOS half of the iOS presenter's: a real menu bar
// where iOS has a four-finger tap. The app menu (Quit ⌘Q — the bare window
// had no menu bar at all) always; Edit always (the field editor's command
// keys — ⌘A/X/C/V/Z — are menu equivalents, not key bindings; without this
// they are dead); Develop — Reload ⌘R, App Info… ⌘D — unless
// EXACT_DEV_MENU=0. Native AppKit above the presenter, so it is alive even
// when the plan is broken; reload restarts from the dev loop's plan when
// one is named (the watcher's own path, state carried), else from the
// baked plan, fresh.
import AppKit

final class DevMenuTarget: NSObject {
    @objc func reload(_ sender: Any?) { DevMenu.reload() }
    @objc func info(_ sender: Any?) { DevMenu.showInfo() }
    @objc func openProject(_ sender: Any?) { DevMenu.openProject() }
}

/// Select All on a focused field. A secure field editor can ignore
/// `selectAll:` (Weird Castle's password); forcing the range still
/// highlights so the next key replaces. Anything else (a WKWebView)
/// gets the action through the responder chain.
final class EditMenuTarget: NSObject {
    @objc func selectAll(_ sender: Any?) {
        let r = (NSApp.keyWindow ?? window).firstResponder
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

enum DevMenu {
    static let target = DevMenuTarget()
    static let editTarget = EditMenuTarget()
    static var enabled: Bool { ProcessInfo.processInfo.environment["EXACT_DEV_MENU"] != "0" }

    static func install() {
        let bar = NSMenu()
        let appItem = NSMenuItem()
        bar.addItem(appItem)
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "Quit Exact", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appItem.submenu = appMenu
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
        if enabled {
            let devItem = NSMenuItem()
            bar.addItem(devItem)
            let dev = NSMenu(title: "Develop")
            dev.addItem(withTitle: "Reload", action: #selector(DevMenuTarget.reload(_:)), keyEquivalent: "r").target = target
            dev.addItem(withTitle: "Open Project…", action: #selector(DevMenuTarget.openProject(_:)), keyEquivalent: "o").target = target
            dev.addItem(withTitle: "App Info…", action: #selector(DevMenuTarget.info(_:)), keyEquivalent: "d").target = target
            devItem.submenu = dev
        }
        app.mainMenu = bar
    }

    /// The typed URL — the affordance a physical device actually uses
    /// (LLP 1023 Stage 1): seeded with the last value, kept in defaults.
    static func openProject() {
        let alert = NSAlert()
        alert.messageText = "Open Project"
        alert.informativeText = "The app URL the dev server printed."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 280, height: 24))
        field.stringValue = UserDefaults.standard.string(forKey: "exact.dev.url")
            ?? PlanURL.current?.page.absoluteString ?? "http://"
        alert.accessoryView = field
        alert.addButton(withTitle: "Connect")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let url = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !url.isEmpty else { return }
        UserDefaults.standard.set(url, forKey: "exact.dev.url")
        PlanURL.open(url)
    }

    static func reload() {
        // A live URL session re-fetches (and clears a rebuilt stop).
        if let s = PlanURL.current { s.reload(); return }
        let started = CACurrentMediaTime()
        let size = presenter.viewport.contentSize
        Exact.wake = exactWake
        let env = ProcessInfo.processInfo.environment
        let batch: Batch
        if let path = env["EXACT_DEV_PLAN"] ?? env["EXACT_PLAN"], let bytes = FileManager.default.contents(atPath: path) {
            batch = Exact.bootPlan(bytes, width: size.width, height: size.height)
        } else {
            batch = Exact.boot(width: size.width, height: size.height)
        }
        if let error = batch.error {
            FileHandle.standardError.write(Data("exact: \(error)\n".utf8))
        } else {
            presenter.reset()
            apply(batch)
        }
        print("reloaded in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")")
    }

    static func info() -> String {
        let env = ProcessInfo.processInfo.environment
        var lines = [CommandLine.arguments[0]]
        if let s = PlanURL.current {
            lines.append("url: \(s.page)\(s.terminal != nil ? " (rebuild the host)" : " (live)")")
        }
        if let path = env["EXACT_DEV_PLAN"] {
            let m = (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date
            lines.append("plan: \(path)\(m.map { " (\(time.string(from: $0)))" } ?? " (missing)")")
        } else if let path = env["EXACT_PLAN"] {
            lines.append("plan: \(path)")
        } else {
            lines.append("plan: baked")
        }
        lines.append("app dir: \(env["EXACT_ASSETS"] ?? FileManager.default.currentDirectoryPath)")
        let size = presenter.viewport.contentSize
        lines.append("viewport: \(Int(size.width))×\(Int(size.height)) · \(presenter.views.count) views")
        lines.append("boot: \(String(format: "%.1f", bootMs)) ms")
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

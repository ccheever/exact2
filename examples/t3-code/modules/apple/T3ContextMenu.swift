// The desktop shell's context menu (reference apps/desktop ElectronMenu.ts
// showContextMenu): a native NSMenu at the pointer. A destructive item gets a
// separator before it and the 12pt template trash symbol, as Electron draws it
// on macOS. The reply names the chosen item, or null when the menu closes.
import AppKit
import UniformTypeIdentifiers

final class T3ContextMenu: NSObject {
    private var chosen: String?

    /// Items: [{ id, label, destructive?, disabled?, separatorBefore? }]. Main thread only.
    func show(_ items: [[String: Any]], in window: NSWindow?, keyboard: Bool = false) -> String? {
        let menu = self.menu(for: items)
        chosen = nil
        let target = window ?? NSApp.keyWindow ?? NSApp.mainWindow
        if let view = target?.contentView, let window = target {
            let focused = window.firstResponder as? NSView
            let point: NSPoint
            if keyboard, let focused, focused.isDescendant(of: view) {
                point = view.convert(NSPoint(x: focused.bounds.midX, y: focused.bounds.midY), from: focused)
            } else { point = view.convert(window.mouseLocationOutsideOfEventStream, from: nil) }
            menu.popUp(positioning: nil, at: point, in: view)
        } else {
            menu.popUp(positioning: nil, at: NSEvent.mouseLocation, in: nil)
        }
        return chosen
    }

    /// The menu `show` pops up: Electron's showContextMenu layout (a separator before the
    /// first destructive item unless the items already declared their sections).
    func menu(for items: [[String: Any]]) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        var destructiveSeparated = false, explicitSection = false
        for item in items {
            guard let id = item["id"] as? String, let label = item["label"] as? String else { continue }
            let destructive = item["destructive"] as? Bool ?? false
            if item["separatorBefore"] as? Bool == true, menu.items.last.map({ !$0.isSeparatorItem }) ?? false {
                menu.addItem(.separator()); explicitSection = true
            }
            if destructive, !destructiveSeparated, !explicitSection, !menu.items.isEmpty {
                menu.addItem(.separator()); destructiveSeparated = true
            }
            let entry = NSMenuItem(title: label, action: #selector(pick(_:)), keyEquivalent: "")
            entry.target = self
            entry.representedObject = id
            entry.isEnabled = !(item["disabled"] as? Bool ?? false)
            if destructive, let trash = NSImage(systemSymbolName: "trash", accessibilityDescription: nil) {
                trash.size = NSSize(width: 12, height: 12)
                trash.isTemplate = true
                entry.image = trash
            }
            menu.addItem(entry)
        }
        return menu
    }

    /// The item the last popped menu returned (nil when it closed without a choice).
    var picked: String? { chosen }

    @objc private func pick(_ sender: NSMenuItem) { chosen = sender.representedObject as? String }

    /// The module's `contextMenu` request on the main thread. Under the agent no menu pops up:
    /// its window is never key and has no pointer, so `popUp` would track until a real click
    /// or Escape, leaving this request (and every main-queue turn and `clock settle` behind it)
    /// pending. It answers dismissed, as Escape would, as T3Sidebar's menu does; menus the
    /// agent must choose from are Contract `contextPopover`s (the right-panel tab menu).
    static func perform(_ request: [String: Any], agent: Bool = false, reply: @escaping ([String: Any]) -> Void) {
        if agent { return reply(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["clicked": NSNull(), "shown": false] as [String: Any]]) }
        DispatchQueue.main.async {
            let items = request["items"] as? [[String: Any]] ?? []
            // NSMenuItem.target is weak: keep the owner alive across the modal popUp.
            let owner = T3ContextMenu()
            let picked = owner.show(items, in: NSApp.keyWindow, keyboard: request["anchor"] as? String == "focus")
            withExtendedLifetime(owner) {}
            let clicked: Any = picked.map { $0 as Any } ?? NSNull()
            let value: [String: Any] = ["clicked": clicked]
            reply(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": value])
        }
    }

    /// Export a text file the way the desktop shell's download does: an NSSavePanel
    /// with the suggested name. Under the agent the file goes into the isolated
    /// data root's exports/ instead, so a drive can check it without a panel.
    static func saveText(_ request: [String: Any], exportsRoot: URL?, reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let name = (request["suggestedName"] as? String ?? "theme.json").replacingOccurrences(of: "/", with: "-")
        guard let text = request["text"] as? String, text.utf8.count <= 1_048_576, !name.isEmpty, name.count <= 128 else {
            return reply(["ok": false, "generation": generation, "error": ["kind": "Arguments", "message": "That file could not be exported.", "uncertain": false]])
        }
        let write = { (url: URL) -> [String: Any] in
            do {
                try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                try Data(text.utf8).write(to: url, options: .atomic)
                return ["ok": true, "generation": generation, "value": ["saved": true, "name": url.lastPathComponent]]
            } catch {
                return ["ok": false, "generation": generation, "error": ["kind": "Write", "message": "Could not save \(name).", "uncertain": false]]
            }
        }
        if let exportsRoot { return reply(write(exportsRoot.appendingPathComponent(name))) }
        DispatchQueue.main.async {
            let panel = NSSavePanel()
            panel.nameFieldStringValue = name
            panel.allowedContentTypes = [.json]
            panel.canCreateDirectories = true
            guard panel.runModal() == .OK, let url = panel.url else {
                return reply(["ok": true, "generation": generation, "value": ["saved": false]])
            }
            reply(write(url))
        }
    }

    /// pickThemeFiles (apps/desktop/src/ipc/methods/window.ts, 1e2ecbd975): an NSOpenPanel of
    /// .json files, several at once, opening in ~/.vscode/extensions when it exists. Theme files
    /// are a few KB: one over 256 KiB comes back as {name, size, text: ""} without being read, an
    /// unreadable one as {name, size: 0, text: ""}; the client reports both. Cancel answers
    /// `cancelled`. Under the agent the files are the isolated data root's imports/ directory, so
    /// a drive can supply them without a panel.
    static let pickedThemeFileMaxBytes = 256 * 1024
    static func readThemeFile(_ url: URL) -> [String: Any] {
        let name = url.lastPathComponent
        guard let size = (try? FileManager.default.attributesOfItem(atPath: url.path))?[.size] as? NSNumber else { return ["name": name, "size": 0, "text": ""] }
        if size.intValue > pickedThemeFileMaxBytes { return ["name": name, "size": size.intValue, "text": ""] }
        guard let data = try? Data(contentsOf: url), let text = String(data: data, encoding: .utf8) else { return ["name": name, "size": 0, "text": ""] }
        return ["name": name, "size": size.intValue, "text": text]
    }
    /// os.homedir()/.vscode/extensions when it exists (HOME first, as Node reads it).
    static func themeStartDirectory(home: String = ProcessInfo.processInfo.environment["HOME"] ?? NSHomeDirectory()) -> URL? {
        let extensions = URL(fileURLWithPath: home, isDirectory: true).appendingPathComponent(".vscode/extensions", isDirectory: true)
        var isDirectory: ObjCBool = false
        return FileManager.default.fileExists(atPath: extensions.path, isDirectory: &isDirectory) && isDirectory.boolValue ? extensions : nil
    }
    static func themePanel() -> NSOpenPanel {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        if let start = themeStartDirectory() { panel.directoryURL = start }
        return panel
    }
    static func openText(_ request: [String: Any], importsRoot: URL?, reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let read = { (urls: [URL]) -> [String: Any] in
            ["ok": true, "generation": generation, "value": ["files": urls.map(readThemeFile), "cancelled": urls.isEmpty]]
        }
        if let importsRoot {
            let urls = (try? FileManager.default.contentsOfDirectory(at: importsRoot, includingPropertiesForKeys: nil))?
                .filter { $0.pathExtension.lowercased() == "json" }.sorted { $0.lastPathComponent < $1.lastPathComponent } ?? []
            return reply(read(urls))
        }
        DispatchQueue.main.async {
            let panel = themePanel()
            guard panel.runModal() == .OK else { return reply(read([])) }
            reply(read(panel.urls))
        }
    }

    /// Open VSX reads for Add a theme: https://open-vsx.org only, 1 MB, 10 s.
    static func fetchText(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let failure = { (message: String) in reply(["ok": true, "generation": generation, "value": ["ok": false, "message": message]]) }
        guard let raw = request["url"] as? String, let url = URL(string: raw), url.scheme == "https", url.host?.lowercased() == "open-vsx.org", url.user == nil else {
            return failure("Only Open VSX can be searched.")
        }
        var urlRequest = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 10)
        urlRequest.setValue("application/json", forHTTPHeaderField: "Accept")
        URLSession.shared.dataTask(with: urlRequest) { data, response, error in
            if error != nil { return failure("Open VSX took too long to respond.") }
            guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode), let data else { return failure("Open VSX search is unavailable right now.") }
            guard data.count <= 1_048_576, let text = String(data: data, encoding: .utf8) else { return failure("Open VSX returned an unexpectedly large response.") }
            reply(["ok": true, "generation": generation, "value": ["ok": true, "text": text]])
        }.resume()
    }
}

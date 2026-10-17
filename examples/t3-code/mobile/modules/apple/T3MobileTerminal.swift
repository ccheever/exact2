#if os(iOS)
// @ref llp/1109.007-mobile-terminal.decision.md#stream-ownership
import UIKit

final class T3MobileTerminal {
    let menus = T3MobileTerminalMenus()
    let sessions: T3MobileTerminalSessions
    private struct WeakView { weak var value: T3MobileTerminalView? }
    private var views: [String: WeakView] = [:]
    init(transport: T3Transport) { sessions = T3MobileTerminalSessions.of(transport) }
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let view = T3MobileTerminalView(owner: self, events: events); try view.setProps(props); return view
    }
    func register(_ view: T3MobileTerminalView, key: String) { views[key] = WeakView(value: view) }
    func unregister(_ view: T3MobileTerminalView, key: String) { if views[key]?.value === view { views.removeValue(forKey: key) } }
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        func answer(_ value: [String: Any]) { reply(["ok": true, "generation": generation, "value": value]) }
        if request["op"] as? String == "mobileTerminalPermissions" {
            let environment = request["environmentId"] as? String ?? "", read = request["read"] as? Bool ?? false, operate = request["operate"] as? Bool ?? false
            sessions.permissions(environment: environment, read: read, operate: operate)
            for entry in views.values { entry.value?.permissions(environment: environment, read: read, operate: operate) }
            answer([:]); return
        }
        if request["op"] as? String == "terminalRetain" {
            sessions.retain(Set(request["sessions"] as? [String] ?? [])); answer([:]); return
        }
        guard request["op"] as? String == "mobileTerminalControl", let key = request["key"] as? String,
              let view = views[key]?.value else {
            reply(["ok": false, "generation": generation, "error": ["kind": "Terminal", "message": "This terminal is no longer open."]]); return
        }
        do { try view.control(request["action"] as? String ?? "", value: request["value"] as? String ?? ""); answer([:]) }
        catch { reply(["ok": false, "generation": generation, "error": ["kind": "Terminal", "message": error.localizedDescription]]) }
    }
    func destroy() { menus.destroy(); for entry in Array(views.values) { entry.value?.destroy() }; views.removeAll(); sessions.destroy() }
}

final class T3MobileTerminalView: ExactNativeInstance {
    private weak var owner: T3MobileTerminal?
    private let surface = T3MobileTerminalSurface()
    private lazy var chrome = T3MobileTerminalChrome(surface: surface) { [weak self] action in
        do { try self?.control(action, value: "") } catch { self?.systemMessage(error.localizedDescription) }
    }
    private var accessory: T3MobileTerminalAccessory?
    private var session: T3MobileTerminalSession?
    private var key = "", status = "", modifier = "", host = "unknown"
    private var writes: [String] = [], writing = false, alive = true
    override var view: UIView { chrome }
    static func json(_ value: Any) -> String { String(data: (try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])) ?? Data(), encoding: .utf8) ?? "" }
    init(owner: T3MobileTerminal, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        surface.onInput = { [weak self] value in self?.input(value["data"] as? String ?? "") }
        surface.onResize = { [weak self] value in
            guard let self, let session, let cols = value["cols"] as? Int, let rows = value["rows"] as? Int else { return }
            owner.sessions.resize(session, cols: cols, rows: rows)
        }
        surface.onCapture = { [weak self] value in
            let text = value["text"] as? String ?? ""
            if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { self?.emit(["type": "capture-empty", "message": "There is no visible output to attach."]) }
            else { self?.emit(["type": "capture", "text": text]) }
        }
    }
    override func setProps(_ props: [String: String]) throws {
        guard let text = props["terminal-source"], let source = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              let environment = source["environmentId"] as? String, !environment.isEmpty,
              let thread = source["threadId"] as? String, !thread.isEmpty,
              let terminal = source["terminalId"] as? String, !terminal.isEmpty,
              let cwd = source["cwd"] as? String, let owner else { throw ExactNativeRefusal("The terminal target is unavailable.") }
        if cwd.isEmpty && source["readOnly"] as? Bool != true { throw ExactNativeRefusal("This thread does not have a workspace.") }
        let next = T3MobileTerminalSessions.key(environment: environment, thread: thread, terminal: terminal)
        if next != key {
            if let session { owner.sessions.unbind(self, from: session); owner.unregister(self, key: key) }
            key = next; status = ""; writes.removeAll(); modifier = ""
            surface.terminalKey = key; surface.initialBuffer = ""
            session = owner.sessions.bind(self, environment: environment, thread: thread, terminal: terminal, cwd: cwd,
                worktreePath: source["worktreePath"] as? String ?? "", env: source["env"] as? [String: String] ?? [:], readOnly: source["readOnly"] as? Bool ?? true)
            owner.register(self, key: key)
        }
        let nextReadOnly = source["readOnly"] as? Bool ?? true
        if nextReadOnly { writes.removeAll(); modifier = "" }
        surface.readOnly = nextReadOnly
        chrome.readOnly = nextReadOnly
        if let session { owner.sessions.setReadOnly(nextReadOnly, session: session) }
        surface.autoFocus = source["autoFocus"] as? Bool ?? false
        host = source["hostPlatform"] as? String ?? "unknown"
        let size = source["fontSize"] as? Double ?? 10.5
        surface.fontSize = CGFloat(max(6, min(14, size.isFinite ? size : 10.5)))
        surface.themeConfig = source["themeConfig"] as? String ?? ""
        surface.appearanceScheme = source["appearance"] as? String ?? "dark"
        surface.backgroundColorHex = source["background"] as? String ?? "#0a0a0a"
        surface.foregroundColorHex = source["foreground"] as? String ?? "#adadb1"
        chrome.colors(background: surface.backgroundColorHex, foreground: surface.foregroundColorHex)
        surface.mutedForegroundColorHex = source["mutedForeground"] as? String ?? "#8E8E95"
        if accessory?.host != host {
            let next = T3MobileTerminalAccessory(host: host) { [weak self] action, value in
                guard let self else { return }
                if action == "clear" { modifier = ""; accessory?.selectModifier(""); emit(["type": "action", "action": "clear"]); return }
                do { try control(action, value: value) } catch { systemMessage(error.localizedDescription) }
            }
            accessory = next
        }
        accessory?.colors(background: source["background"] as? String ?? "#0a0a0a", foreground: source["foreground"] as? String ?? "#adadb1", border: source["border"] as? String ?? "#2e2e30")
        chrome.setAccessory(surface.readOnly ? nil : accessory)
        if let session { sessionChanged(session) }
    }
    func sessionChanged(_ session: T3MobileTerminalSession) {
        guard alive, self.session === session else { return }
        surface.initialBuffer = session.output.text
        let next = Self.json(["status": session.status, "error": session.error ?? "", "label": session.label, "lifecycle": session.lifecycleVersion])
        if next != status { status = next; emit(["type": "status", "status": session.status, "version": session.version, "error": session.error ?? "", "label": session.label]) }
    }
    func permissions(environment: String, read: Bool, operate: Bool) {
        guard let parts = (try? JSONSerialization.jsonObject(with: Data(key.utf8))) as? [String], parts.first == environment else { return }
        if !operate { writes.removeAll(); modifier = ""; accessory?.selectModifier(""); surface.readOnly = true; chrome.readOnly = true; chrome.setAccessory(nil) }
        if !read {
            surface.initialBuffer = ""
            if let session { owner?.sessions.unbind(self, from: session) }
            owner?.unregister(self, key: key); session = nil; key = ""
        }
    }
    func systemMessage(_ message: String) { emit(["type": "error", "message": message]) }
    private func emit(_ value: [String: Any]) { guard alive else { return }; var value = value; value["key"] = key; events.change(Self.json(value)) }
    private func input(_ data: String) {
        guard alive, !surface.readOnly, !data.isEmpty else { return }
        let armed = modifier; modifier = ""; accessory?.selectModifier("")
        if !armed.isEmpty { emit(["type": "modifier", "value": ""]) }
        if data.lowercased() == "v", armed == (host == "mac" ? "meta" : "ctrl") { paste(); return }
        if armed == "meta" { enqueue("\u{1b}" + data) }
        else if armed == "ctrl" { enqueue(Self.controlByte(data)) }
        else { enqueue(data) }
    }
    private static func controlByte(_ text: String) -> String {
        guard let first = text.lowercased().unicodeScalars.first else { return text }
        if (97...122).contains(first.value) { return String(UnicodeScalar(first.value - 96)!) }
        let mapped: [String: String] = ["@": "\0", "[": "\u{1b}", "\\": "\u{1c}", "]": "\u{1d}", "^": "\u{1e}", "_": "\u{1f}", "?": "\u{7f}"]
        return mapped[String(first)] ?? text
    }
    private func enqueue(_ text: String) {
        // Source chunkTerminalWrite: at most65536 UTF16 units, never split a surrogate pair.
        var chunk = "", count = 0
        for scalar in text.unicodeScalars {
            let length = scalar.value > 0xffff ? 2 : 1
            if count + length > 65_536 { writes.append(chunk); chunk = ""; count = 0 }
            chunk.unicodeScalars.append(scalar); count += length
        }
        if !chunk.isEmpty { writes.append(chunk) }; flush()
    }
    private func flush() {
        guard alive, !surface.readOnly, !writing, !writes.isEmpty, let session, let owner else { return }
        writing = true; let data = writes.removeFirst()
        owner.sessions.write(data, to: session, from: self) { [weak self] in
            guard let self else { return }; writing = false
            flush()
        }
    }
    private func paste() {
        guard !surface.readOnly, let text = UIPasteboard.general.string else { return }
        let safe = String(text.unicodeScalars.map { scalar -> Character in
            (scalar.value < 32 && ![9,10,13].contains(scalar.value)) || scalar.value == 127 ? " " : Character(scalar)
        }).replacingOccurrences(of: "\r\n", with: "\r").replacingOccurrences(of: "\n", with: "\r")
        enqueue(safe)
    }
    func control(_ action: String, value: String) throws {
        guard alive else { throw ExactNativeRefusal("This terminal is no longer open.") }
        if action == "capture" { surface.dismissKeyboard(); surface.captureRequest += 1; return }
        if action == "hide-keyboard" { surface.dismissKeyboard(); return }
        guard !surface.readOnly else { throw ExactNativeRefusal("This connection does not have permission to operate terminals.") }
        switch action {
        case "show-keyboard": surface.focusRequest += 1
        case "modifier":
            guard ["ctrl", "meta"].contains(value) else { throw ExactNativeRefusal("Unknown terminal modifier.") }
            modifier = modifier == value ? "" : value; accessory?.selectModifier(modifier); emit(["type": "modifier", "value": modifier])
        case "paste": modifier = ""; accessory?.selectModifier(""); emit(["type": "modifier", "value": ""]); paste()
        case "input": input(value)
        default: throw ExactNativeRefusal("Unknown terminal control.")
        }
    }
    override func agentInput(_ input: ExactNativeInput) throws {
        switch input {
        case .text(let value): try control("input", value: value)
        case .key(let name, let phase):
            guard phase != "up" else { return }
            let keys = ["Enter": "\r", "Tab": "\t", "Escape": "\u{1b}", "Backspace": "\u{7f}", "ArrowUp": "\u{1b}[A", "ArrowDown": "\u{1b}[B", "ArrowLeft": "\u{1b}[D", "ArrowRight": "\u{1b}[C", "Ctrl+C": "\u{3}"]
            guard let data = keys[name] else { throw ExactNativeRefusal("That terminal key is unavailable.") }; try control("input", value: data)
        }
    }
    override func destroy() {
        guard alive else { return }; alive = false; writes.removeAll(); chrome.destroy(); surface.dispose()
        if let session { owner?.sessions.unbind(self, from: session) }; owner?.unregister(self, key: key); session = nil
    }
}
#endif

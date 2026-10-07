#if os(macOS)
import AppKit

/// ThreadTerminalDrawer's selection popup and context menu (T3 Code 1e2ecbd975, MIT).
/// The vendored page's observeSelectionActions owns release/multi-click timing. This side
/// presents its settled selection as an AppKit menu and owns synchronous clipboard access.
/// A newer gesture invalidates an older popup before any action can write, focus, or emit a chip.
final class T3TerminalActions: NSObject {
    struct Selection {
        let clipboard: String, text: String
        let lineStart: Int, lineEnd: Int
        init?(_ body: [String: Any]) {
            guard let text = body["text"] as? String,
                  let position = body["position"] as? [String: Any],
                  let start = position["start"] as? [String: Any], let y = start["y"] as? Int,
                  let end = position["end"] as? [String: Any], let endY = end["y"] as? Int else { return nil }
            let normalized = text.replacingOccurrences(of: "\r\n", with: "\n").trimmingCharacters(in: CharacterSet(charactersIn: "\n"))
            guard !normalized.isEmpty else { return nil }
            clipboard = text; self.text = normalized
            lineStart = y + 1; lineEnd = max(lineStart, endY + 1)
        }
    }

    private weak var view: T3TerminalView?
    private var menu: NSMenu?
    private var request = 0
    private var openRequest: Int?
    private var chosen: String?
    private(set) var items: [[String: Any]] = []
    /// Under the agent the menu is not tracked: AppKit's popUp would hold the main thread until a
    /// real click or Escape (as T3ContextMenu explains). It is reported open at this view point
    /// (top-left origin) with its items until the page dismisses it; the actions are `perform`'s.
    private(set) var agentShown: [Double]?

    init(view: T3TerminalView) { self.view = view }

    var canAddToChat: Bool { view?.sessionMode == true && view?.props["can-add-to-chat"] != "false" }

    static func menuItems(context: Bool, hasSelection: Bool, canAddToChat: Bool) -> [[String: Any]] {
        var result: [[String: Any]] = []
        if canAddToChat { result.append(["id": "add-to-chat", "label": "Add to chat", "disabled": context && !hasSelection]) }
        result.append(["id": "copy", "label": "Copy", "disabled": context && !hasSelection])
        if context { result.append(["id": "paste", "label": "Paste", "disabled": false]) }
        return result
    }

    func receive(_ body: [String: Any]) {
        switch body["type"] as? String {
        case "selection-ready": show(body, context: false)
        case "contextmenu": show(body, context: true)
        case "selection-dismiss": dismiss(supersede: body["reason"] as? String == "interaction")
        case "selection":
            if (body["text"] as? String ?? "").isEmpty, openRequest == request { dismiss(supersede: true) }
        default: break
        }
    }

    /// Passive dismissal may close only the popup it owns; it cannot invalidate a newer
    /// right-click menu while an older popup's nested event loop is returning.
    func dismiss(supersede: Bool) {
        let ownsMenu = openRequest == request
        if supersede || ownsMenu { request += 1 }
        if ownsMenu { menu?.cancelTrackingWithoutAnimation() }
        if ownsMenu, agentShown != nil { agentShown = nil; openRequest = nil }
    }

    func makeMenu(_ rows: [[String: Any]]) -> NSMenu {
        let result = NSMenu()
        result.autoenablesItems = false
        for row in rows {
            guard let action = row["id"] as? String, let label = row["label"] as? String else { continue }
            let item = NSMenuItem(title: label, action: #selector(pick(_:)), keyEquivalent: "")
            item.target = self; item.representedObject = action
            item.isEnabled = row["disabled"] as? Bool != true
            result.addItem(item)
        }
        return result
    }

    private func show(_ body: [String: Any], context: Bool) {
        guard let view, view.web.window != nil, !view.authMode else { return }
        if !context, openRequest == request { return }
        let captured = Selection(body)
        if !context, captured == nil { dismiss(supersede: true); return }
        dismiss(supersede: true)
        request += 1
        let token = request
        let rows = Self.menuItems(context: context, hasSelection: captured != nil, canAddToChat: canAddToChat)
        openRequest = token
        // Let the bridge reply return before entering NSMenu's nested tracking loop.
        DispatchQueue.main.async { [weak self, weak view] in
            guard let self, let view, self.request == token, view.web.window != nil else { return }
            self.items = rows; self.chosen = nil
            let x = body["x"] as? Double ?? 8, top = body["y"] as? Double ?? 8
            if view.agent { self.agentShown = [x, top]; self.openRequest = token; return }
            let menu = self.makeMenu(rows)
            self.menu = menu; self.openRequest = token
            let point = NSPoint(x: x, y: view.web.isFlipped ? top : view.web.bounds.height - top)
            menu.popUp(positioning: nil, at: point, in: view.web)
            let action = self.chosen
            if self.openRequest == token { self.openRequest = nil; self.menu = nil }
            guard token == self.request, let action else { return }
            self.perform(action, selection: captured, request: token)
        }
    }

    @objc private func pick(_ sender: NSMenuItem) { chosen = sender.representedObject as? String }

    /// Also used by the native regression tests: the same captured selection and request
    /// token that a real menu returns, without making the test wait for a person's click.
    func perform(_ action: String, selection: Selection?, request token: Int) {
        guard token == request, let view else { return }
        switch action {
        case "add-to-chat":
            guard canAddToChat, let selection else { return }
            view.emit(["type": "selectionAction", "action": "add-to-chat", "text": selection.text,
                       "lineStart": selection.lineStart, "lineEnd": selection.lineEnd])
            view.send(["type": "clearSelection"])
        case "copy":
            guard let selection else { return }
            NSPasteboard.general.clearContents()
            if !NSPasteboard.general.setString(selection.clipboard, forType: .string) { view.systemMessage("Unable to copy terminal selection") }
        case "paste":
            if let text = NSPasteboard.general.string(forType: .string) { view.send(["type": "paste", "text": text]) }
        default: return
        }
        if token == request { view.focusTerminal() }
    }

    var status: [String: Any] { ["open": menu != nil || agentShown != nil, "request": request, "items": items, "at": agentShown ?? []] }

    /// TS owns the URL-vs-path decision and editor preference. This boundary performs only
    /// the system-browser branch and reports a terminal-local failure without opening files.
    static func perform(_ request: [String: Any], reply: ExactReply) -> Bool {
        guard let op = request["op"] as? String, ["terminalOpenExternal", "terminalSystemMessage"].contains(op) else { return false }
        DispatchQueue.main.async {
            let value: [String: Any]
            if op == "terminalOpenExternal" {
                let url = (request["url"] as? String).flatMap(URL.init(string:))
                let opened = url.map { ["http", "https"].contains($0.scheme?.lowercased() ?? "") && NSWorkspace.shared.open($0) } ?? false
                value = ["opened": opened]
            } else {
                let views = T3Terminals.shared.all.filter { view in
                    view.identity == request["terminalId"] as? String && view.props["environment"] == request["environmentId"] as? String &&
                        view.props["thread"] == request["threadId"] as? String
                }
                let message = request["message"] as? String ?? ""
                for view in views { view.systemMessage(message) }
                value = ["written": !views.isEmpty]
            }
            reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": value])
        }
        return true
    }
}
#endif

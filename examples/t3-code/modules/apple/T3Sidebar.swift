// The sidebar's native pieces (MIT reference, see LICENSE-T3): the legacy
// sidebar's New thread choice of project members as the desktop shell shows it
// (ElectronMenu.ts showContextMenu: a native menu at the pointer), and the repaint a
// timed sidebar change asks for. Every other row menu is a Contract context popover,
// which the host presents as an NSMenu (exact2 #223, #314).
// The members' menu stays here: LegacySidebar's handleCreateThreadClick shows it at
// the click (event.clientX, clientY) from a left press, where a Contract button's
// menu is a pull-down below the button and a press's MouseEvent carries no position.
// Held modifiers (thread-jump hints, the Pull Requests page's speed mode) are the
// window's Contract key events (app.contract modifierKey).
import AppKit

final class T3Sidebar: NSObject {
    private let changed: (String) -> Void
    private let agent: Bool
    private(set) var picked: String?

    init(agent: Bool, changed: @escaping (String) -> Void) {
        self.agent = agent
        self.changed = changed
        super.init()
    }

    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        func answer(_ value: [String: Any]) { reply(["ok": true, "generation": generation, "value": value]) }
        switch request["op"] as? String {
        case "sidebarNotify":
            let delay = (request["delay"] as? Double ?? 0) / 1000
            DispatchQueue.main.asyncAfter(deadline: .now() + max(0, delay)) { [weak self] in self?.changed("t3.status") }
            answer([:])
        case "sidebarMenu":
            let template = request["items"] as? [[String: Any]] ?? []
            // The agent's window has no pointer and must not enter menu tracking:
            // it answers dismissed, as an Escape would.
            let window = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp.windows.first { ($0.firstResponder as? NSView).map { $0 !== $0.window?.contentView } ?? false }
            guard !agent, let window, let view = window.contentView else {
                answer(["id": NSNull(), "shown": false]); return
            }
            let menu = Self.menu(template, target: self)
            picked = nil
            menu.popUp(positioning: nil, at: view.convert(window.mouseLocationOutsideOfEventStream, from: nil), in: view)
            answer(["id": picked.map { $0 as Any } ?? NSNull(), "shown": true])
        default:
            reply(["ok": false, "generation": generation, "error": ["kind": "Sidebar", "message": "Unknown sidebar operation.", "uncertain": false]])
        }
    }

    /// Where a keyboard-opened menu's top-left corner goes, for the
    /// focused view: its centre (Chromium's keyboard contextmenu, truncated to whole
    /// points) or its bottom-left corner. `point` is in `view`'s coordinates; `topLeft`
    /// in the window content's top-left space. The media menu's (T3MediaActions.swift).
    static func anchorPoint(_ anchor: String, focus: NSView?, in view: NSView) -> (point: NSPoint, topLeft: NSPoint)? {
        guard let focus, focus !== view, focus.isDescendant(of: view), anchor == "center" || anchor == "bottom-left" else { return nil }
        let rect = view.convert(focus.bounds, from: focus)
        let top = view.isFlipped ? rect.minY : view.bounds.height - rect.maxY
        let topLeft = anchor == "center" ? NSPoint(x: floor(rect.minX + rect.width / 2), y: floor(top + rect.height / 2))
            : NSPoint(x: rect.minX, y: top + rect.height)
        return (NSPoint(x: topLeft.x, y: view.isFlipped ? topLeft.y : view.bounds.height - topLeft.y), topLeft)
    }

    /// ElectronMenu.ts buildTemplate already applied by the client (separators
    /// placed, empty submenus dropped): items, submenus and separators map 1:1.
    static func menu(_ template: [[String: Any]], target: AnyObject?) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        for entry in template {
            let type = entry["type"] as? String ?? "item"
            if type == "separator" { menu.addItem(.separator()); continue }
            let item = NSMenuItem(title: entry["label"] as? String ?? "", action: nil, keyEquivalent: "")
            item.isEnabled = entry["enabled"] as? Bool ?? true
            if let checked = entry["checked"] as? Bool { item.state = checked ? .on : .off }
            if type == "submenu", let children = entry["children"] as? [[String: Any]] {
                item.submenu = Self.menu(children, target: target)
            } else {
                item.representedObject = entry["id"] as? String
                item.action = #selector(T3Sidebar.pick(_:))
                item.target = target
                if entry["destructive"] as? Bool == true, let trash = NSImage(systemSymbolName: "trash", accessibilityDescription: nil) {
                    trash.isTemplate = true
                    trash.size = NSSize(width: 12, height: 12)
                    item.image = trash
                }
            }
            menu.addItem(item)
        }
        return menu
    }
    @objc func pick(_ sender: NSMenuItem) { picked = sender.representedObject as? String }
}

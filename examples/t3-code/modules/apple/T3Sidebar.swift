// The sidebar's native pieces (MIT reference, see LICENSE-T3): the thread
// action menu as the desktop shell shows it (ElectronMenu.ts showContextMenu:
// a native menu at the pointer with separators, submenus, checkmarks and a
// trash glyph on the destructive row), the modifier state a row press reads
// (⌘-click toggles, ⇧-click extends the multi-selection; read from the click
// event, r8-pointer D8), and thread-jump
// hints while ⌘ alone is held for 200 ms (THREAD_JUMP_HINT_SHOW_DELAY_MS).
import AppKit

final class T3Sidebar: NSObject {
    private let changed: (String) -> Void
    private let agent: Bool
    private var monitor: Any?
    private var clickMonitor: Any?
    /// r8-pointer (D8): the newest primary click's modifiers, read from the
    /// click event itself. A row's press reaches the client after the click
    /// ends, by when ⌘ or ⇧ may be up; `NSEvent.modifierFlags` then reads the
    /// keyboard as it is now, not as it was when the row was clicked.
    private(set) var click: (flags: NSEvent.ModifierFlags, at: TimeInterval)?
    private var pending: DispatchWorkItem?
    private(set) var jumpHints = false
    private(set) var picked: String?

    init(agent: Bool, changed: @escaping (String) -> Void) {
        self.agent = agent
        self.changed = changed
        super.init()
    }

    var status: [String: Any] { ["sidebarJumpHints": jumpHints] }

    /// Exactly ⌘ (caps lock, fn and the numeric pad never count): shown after
    /// the delay, hidden at once on release or when ⇧, ⌥ or ⌃ joins.
    static func jumpModifiers(_ flags: NSEvent.ModifierFlags) -> Bool {
        flags.intersection([.command, .shift, .option, .control]) == [.command]
    }

    func install() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.flagsChanged, .keyDown]) { [weak self] event in
            self?.flags(event.type == .keyDown ? [] : event.modifierFlags)
            return event
        }
        clickMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp]) { [weak self] event in
            self?.record(event); return event
        }
        NotificationCenter.default.addObserver(self, selector: #selector(resign), name: NSApplication.didResignActiveNotification, object: nil)
    }
    /// Internal so tests can drive it with constructed events. The click event
    /// (mouse up) wins over its press, as the web's click event carries them.
    func record(_ event: NSEvent, at time: TimeInterval = ProcessInfo.processInfo.systemUptime) {
        guard event.type == .leftMouseDown || event.type == .leftMouseUp else { return }
        click = (event.modifierFlags.intersection([.command, .shift, .option, .control]), time)
    }
    /// The modifiers a row press reads: the click's own when one ended within
    /// the last two seconds (the press it caused), else the keyboard's now
    /// (an agent's synthetic press never passes the monitor). Read once.
    func pressModifiers(at time: TimeInterval = ProcessInfo.processInfo.systemUptime) -> NSEvent.ModifierFlags {
        defer { click = nil }
        if let click, time - click.at >= 0, time - click.at <= 2 { return click.flags }
        return NSEvent.modifierFlags
    }
    @objc private func resign() { flags([]) }
    func flags(_ flags: NSEvent.ModifierFlags) {
        if Self.jumpModifiers(flags) {
            guard pending == nil, !jumpHints else { return }
            let work = DispatchWorkItem { [weak self] in
                guard let self else { return }
                self.pending = nil
                guard Self.jumpModifiers(NSEvent.modifierFlags), !self.jumpHints else { return }
                self.jumpHints = true
                self.changed("t3.status")
            }
            pending = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: work)
        } else {
            pending?.cancel(); pending = nil
            if jumpHints { jumpHints = false; changed("t3.status") }
        }
    }

    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        func answer(_ value: [String: Any]) { reply(["ok": true, "generation": generation, "value": value]) }
        switch request["op"] as? String {
        case "sidebarModifiers":
            let flags = pressModifiers()
            answer(["command": flags.contains(.command), "shift": flags.contains(.shift), "option": flags.contains(.option), "control": flags.contains(.control)])
        case "sidebarNotify":
            let delay = (request["delay"] as? Double ?? 0) / 1000
            DispatchQueue.main.asyncAfter(deadline: .now() + max(0, delay)) { [weak self] in self?.changed("t3.status") }
            answer([:])
        case "sidebarMenu":
            let template = request["items"] as? [[String: Any]] ?? []
            // r12-sidebar: a keyboard menu (`anchor`) opens at the focused row, not the pointer.
            // The agent's window is never key or main: take the one whose first responder is a view.
            let window = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp.windows.first { ($0.firstResponder as? NSView).map { $0 !== $0.window?.contentView } ?? false }
            let anchored = (request["anchor"] as? String).flatMap { anchor in
                window.flatMap { window in window.contentView.flatMap { Self.anchorPoint(anchor, focus: window.firstResponder as? NSView, in: $0) } }
            }
            // The agent's window has no pointer to anchor to and must not enter
            // menu tracking; it answers dismissed, as an Escape would (with where a keyboard menu would open).
            guard !agent, let window, let view = window.contentView else {
                answer(["id": NSNull(), "shown": false, "anchor": anchored.map { [$0.topLeft.x, $0.topLeft.y] as Any } ?? NSNull()]); return
            }
            let menu = Self.menu(template, target: self)
            picked = nil
            let point = anchored?.point ?? view.convert(window.mouseLocationOutsideOfEventStream, from: nil)
            menu.popUp(positioning: nil, at: point, in: view)
            answer(["id": picked.map { $0 as Any } ?? NSNull(), "shown": true])
        default:
            reply(["ok": false, "generation": generation, "error": ["kind": "Sidebar", "message": "Unknown sidebar operation.", "uncertain": false]])
        }
    }

    /// r12-sidebar: where a keyboard-opened menu's top-left corner goes, for the
    /// focused row view: its centre (Chromium's keyboard contextmenu on a thread row,
    /// truncated to whole points) or its bottom-left corner (the draft row's handler).
    /// `point` is in `view`'s coordinates; `topLeft` in the window content's top-left space.
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

    func destroy() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        if let clickMonitor { NSEvent.removeMonitor(clickMonitor) }
        monitor = nil; clickMonitor = nil
        pending?.cancel(); pending = nil
        NotificationCenter.default.removeObserver(self)
    }
}

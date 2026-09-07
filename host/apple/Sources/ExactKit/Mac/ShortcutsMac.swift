// App-declared aria-keyshortcuts: one button action for keys, menus, and clicks.
#if os(macOS)
import AppKit

private struct Shortcut {
    let key: String
    let modifiers: NSEvent.ModifierFlags
    init?(_ text: Substring) {
        var parts = text.split(separator: "+", omittingEmptySubsequences: false)
        guard let last = parts.popLast() else { return nil }
        if last == "Escape", parts.isEmpty { key = "Escape"; modifiers = []; return }
        guard last.count == 1 else { return nil }
        var mask: NSEvent.ModifierFlags = []
        for part in parts {
            switch part {
            case "Meta": mask.insert(.command)
            case "Control": mask.insert(.control)
            case "Alt": mask.insert(.option)
            case "Shift": mask.insert(.shift)
            default: return nil
            }
        }
        guard !mask.intersection([.command, .control]).isEmpty else { return nil }
        key = last.lowercased()
        modifiers = mask
    }
    func matches(_ event: NSEvent) -> Bool {
        (key == "Escape" ? event.keyCode == 53 : event.charactersIgnoringModifiers?.lowercased() == key)
            && event.modifierFlags.intersection([.command, .control, .option, .shift]) == modifiers
    }
}

final class ShortcutHost: NSObject, NSMenuItemValidation {
    private weak var presenter: Presenter?
    private weak var menu: NSMenu?
    init(presenter: Presenter) { self.presenter = presenter }
    func attach(_ menu: NSMenu) { self.menu = menu; sync() }

    private func declarations(_ view: NodeView) -> [Shortcut] {
        (view.props["accessibilityKeyShortcuts"] ?? "").split(whereSeparator: \.isWhitespace).compactMap(Shortcut.init)
    }
    private func nodes() -> [NodeView] {
        guard let presenter else { return [] }
        return presenter.views.values.filter {
            $0.kind == "button" && $0.handlers.contains("press") && !$0.isHiddenOrHasHiddenAncestor
                && $0.window != nil && $0.props["accessibilityKeyShortcuts"] != nil
        }.sorted { $0.id < $1.id }
    }
    func perform(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown,
              let view = nodes().first(where: { declarations($0).contains(where: { $0.matches(event) }) }) else { return false }
        if !event.isARepeat && !view.disabled { presenter?.press(view.id) }
        return true
    }
    func sync() {
        guard let menu else { return }
        menu.removeAllItems()
        for view in nodes() {
            // A platform alternative does not create a second menu item.
            guard let shortcut = declarations(view).first(where: { $0.modifiers.contains(.command) }) else { continue }
            let item = NSMenuItem(title: title(view), action: #selector(activate(_:)), keyEquivalent: shortcut.key)
            item.keyEquivalentModifierMask = shortcut.modifiers
            item.target = self
            item.representedObject = NSNumber(value: view.id)
            item.isEnabled = !view.disabled
            menu.addItem(item)
        }
    }
    private func title(_ view: NodeView) -> String {
        if let label = view.props["accessibilityLabel"] { return label }
        if view.kind == "text" { return view.paragraphSpec().runs.map(\.text).joined() }
        return view.container.subviews.compactMap { $0 as? NodeView }.map(title).filter { !$0.isEmpty }.joined(separator: " ")
    }
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard let id = (item.representedObject as? NSNumber)?.uint32Value,
              let view = nodes().first(where: { $0.id == id }) else { return false }
        return !view.disabled && view.window === NSApp.keyWindow
    }
    @objc private func activate(_ item: NSMenuItem) {
        guard validateMenuItem(item), let id = (item.representedObject as? NSNumber)?.uint32Value else { return }
        if let event = NSApp.currentEvent, event.type == .keyDown, event.isARepeat { return }
        presenter?.press(id)
    }
}
#endif

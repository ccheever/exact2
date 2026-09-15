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
    private weak var fileMenu: NSMenu?
    private weak var applicationMenu: NSMenu?
    private weak var navigationMenu: NSMenu?
    private var items: [UInt32: NSMenuItem] = [:]
    private let fileSeparator = NSMenuItem.separator()
    private let settingsSeparator = NSMenuItem.separator()
    init(presenter: Presenter) { self.presenter = presenter }
    func attach(_ file: NSMenu, application: NSMenu? = nil, navigation: NSMenu? = nil) {
        for item in Array(items.values) + [fileSeparator, settingsSeparator] { item.menu?.removeItem(item) }
        fileMenu = file
        applicationMenu = application
        navigationMenu = navigation
        sync()
    }

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
              let view = nodes().first(where: {
                  !$0.inert && $0.window === event.window && $0.window?.attachedSheet == nil
                      && declarations($0).contains(where: { $0.matches(event) })
              }) else { return false }
        if !event.isARepeat && !view.disabled { presenter?.press(view.id) }
        return true
    }
    func sync() {
        guard let fileMenu else { return }
        var file: [NSMenuItem] = []
        var application: [NSMenuItem] = []
        var navigation: [NSMenuItem] = []
        var live: Set<UInt32> = []
        for view in nodes() {
            // A platform alternative does not create a second menu item.
            guard let shortcut = declarations(view).first(where: { $0.modifiers.contains(.command) }) else { continue }
            live.insert(view.id)
            let item = items[view.id] ?? NSMenuItem(title: "", action: #selector(activate(_:)), keyEquivalent: "")
            items[view.id] = item
            item.title = title(view)
            item.keyEquivalent = shortcut.key
            item.keyEquivalentModifierMask = shortcut.modifiers
            item.target = self
            item.representedObject = NSNumber(value: view.id)
            item.isEnabled = !view.disabled && !view.inert && view.window?.attachedSheet == nil
            item.state = view.props["accessibilitySelected"] == "true" ? .on : .off
            // Placement follows declared semantics and standard chords, never
            // app names, test ids, or a second registry of commands.
            if shortcut.key == ",", shortcut.modifiers == .command, applicationMenu != nil {
                item.title = "Settings…"
                application.append(item)
            } else if navigationMenu != nil, isNavigation(view, shortcut: shortcut) {
                navigation.append(item)
            } else {
                file.append(item)
            }
        }
        for id in Array(items.keys) where !live.contains(id) {
            if let item = items.removeValue(forKey: id) { item.menu?.removeItem(item) }
        }
        // Reconcile only our own items. In particular, Open… and Close must
        // survive the next plan batch. Existing command objects stay stable.
        if !file.isEmpty { file.append(fileSeparator) } else { fileSeparator.menu?.removeItem(fileSeparator) }
        place(file, in: fileMenu, at: 0)
        if let applicationMenu {
            if !application.isEmpty { application.append(settingsSeparator) }
            else { settingsSeparator.menu?.removeItem(settingsSeparator) }
            place(application, in: applicationMenu, at: min(2, applicationMenu.numberOfItems))
        }
        if let navigationMenu {
            // History commands precede destinations regardless of where the
            // toolbar sits in the authored tree; destinations keep tree order.
            let history = navigation.filter { $0.keyEquivalentModifierMask == .command && ["[", "]"].contains($0.keyEquivalent) }
            navigation = history.sorted { $0.keyEquivalent < $1.keyEquivalent }
                + navigation.filter { item in !history.contains(where: { $0 === item }) }
            place(navigation, in: navigationMenu, at: 0)
            navigationMenu.supermenu?.items.first(where: { $0.submenu === navigationMenu })?.isHidden = navigation.isEmpty
        }
    }
    private func place(_ items: [NSMenuItem], in menu: NSMenu, at start: Int) {
        for (offset, item) in items.enumerated() {
            let index = start + offset
            if item.menu !== menu || menu.index(of: item) != index {
                item.menu?.removeItem(item)
                menu.insertItem(item, at: min(index, menu.numberOfItems))
            }
        }
    }
    private func isNavigation(_ view: NodeView, shortcut: Shortcut) -> Bool {
        if shortcut.modifiers == .command, shortcut.key == "[" || shortcut.key == "]" { return true }
        guard view.props["accessibilityRole"] == "tab" else { return false }
        var ancestor = view.superview
        while let parent = ancestor {
            if let node = parent as? NodeView, node.props["accessibilityRole"] == "tablist" { return true }
            ancestor = parent.superview
        }
        return false
    }
    private func title(_ view: NodeView) -> String {
        if let label = view.props["accessibilityLabel"] { return label }
        if view.kind == "text" { return view.paragraphSpec().runs.map(\.text).joined() }
        return view.container.subviews.compactMap { $0 as? NodeView }.map(title).filter { !$0.isEmpty }.joined(separator: " ")
    }
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard let id = (item.representedObject as? NSNumber)?.uint32Value,
              let view = nodes().first(where: { $0.id == id }) else { return false }
        return !view.disabled && !view.inert && view.window === NSApp.keyWindow && view.window?.attachedSheet == nil
    }
    @objc private func activate(_ item: NSMenuItem) {
        guard validateMenuItem(item), let id = (item.representedObject as? NSNumber)?.uint32Value else { return }
        if let event = NSApp.currentEvent, event.type == .keyDown, event.isARepeat { return }
        presenter?.press(id)
    }
}
#endif

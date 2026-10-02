// App-declared aria-keyshortcuts: one button action for keys, menus, and clicks.
#if os(macOS)
import AppKit

private struct Shortcut {
    let key: String
    let modifiers: NSEvent.ModifierFlags
    /// AppKit represents named keys in menu equivalents by control/function
    /// characters, not the web's names (which remain the declaration syntax).
    private static let namedKeys: [String: String] = [
        "Enter": "\r", "Tab": "\t", "Escape": "\u{1b}", "Space": " ",
        "Backspace": "\u{8}", "Delete": "\u{f728}", "Insert": "\u{f727}",
        "ArrowUp": "\u{f700}", "ArrowDown": "\u{f701}",
        "ArrowLeft": "\u{f702}", "ArrowRight": "\u{f703}",
        "Home": "\u{f729}", "End": "\u{f72b}",
        "PageUp": "\u{f72c}", "PageDown": "\u{f72d}", "Plus": "+",
    ]
    var keyEquivalent: String {
        if let value = Self.namedKeys[key] { return value }
        if key.hasPrefix("F"), let n = Int(key.dropFirst()), (1...35).contains(n) {
            return String(UnicodeScalar(0xf704 + n - 1)!)
        }
        return key
    }
    init?(_ text: Substring) {
        var parts = text.split(separator: "+", omittingEmptySubsequences: false)
        let last: Substring
        // ARIA spells this key "Plus". Accept a literal '+' too, including
        // the trailing separator in a chord such as Meta++.
        if text == "+" { last = "Plus"; parts.removeAll() }
        else if parts.count >= 3, parts.suffix(2).allSatisfy(\.isEmpty) {
            last = "Plus"; parts.removeLast(2)
        } else if let part = parts.popLast() { last = part }
        else { return nil }
        let function = Int(last.dropFirst()).map { (1...35).contains($0) && last == "F\($0)" } == true
        guard last.count == 1 || Self.namedKeys[String(last)] != nil || function else { return nil }
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
        key = last.count == 1 ? last.lowercased() : String(last)
        modifiers = mask
    }
    func matches(_ event: NSEvent) -> Bool {
        guard event.modifierFlags.intersection([.command, .control, .option, .shift]) == modifiers else { return false }
        if ["Enter", "Tab", "Escape", "Backspace", "Delete", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].contains(key) {
            return NodeView.keyName(event) == key
        }
        // Option changes the logical key (Option-C can produce ç). An empty
        // character is a dead key, not the unmodified letter. Control may
        // instead emit a control character; retranslate with glyph modifiers.
        var characters = event.charactersIgnoringModifiers
        if modifiers.contains(.option) {
            characters = event.characters
            if modifiers.contains(.control), characters?.unicodeScalars.contains(where: { $0.value < 0x20 }) == true {
                characters = event.characters(byApplyingModifiers: event.modifierFlags.intersection([.shift, .capsLock, .option]))
            }
        }
        return characters?.lowercased() == keyEquivalent.lowercased()
    }
    func permits(_ responder: NSResponder?) -> Bool {
        // A declared command can use Command/Control inside an editor. Text,
        // cursor motion and Option's text input stay with its input client;
        // Escape retains the existing application's cancel shortcut.
        key == "Escape" || !modifiers.intersection([.command, .control]).isEmpty
            || (responder as? NSView)?.inputContext == nil
    }
}

/// AppKit accepts extra Shift for some equivalents, including Return and arrows.
/// Filter only while looking up a shortcut: labels and ordinary menu selection
/// (including Return on the highlighted item) retain AppKit's usual behavior.
final class ShortcutMenu: NSMenu {
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // AppKit caches equivalents internally, so its stored value must be
        // cleared for the lookup. Restore from current declarations: a command
        // may synchronously update the plan and its menu while being dispatched.
        let suppressed: [(NSMenuItem, ShortcutHost)] = items.compactMap { item in
            guard let host = item.target as? ShortcutHost, !item.keyEquivalent.isEmpty,
                  host.menuEquivalent(item, matching: event).isEmpty else { return nil }
            item.keyEquivalent = ""
            return (item, host)
        }
        defer { for (item, host) in suppressed { item.keyEquivalent = host.menuEquivalent(item) } }
        return super.performKeyEquivalent(with: event)
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
    func attach(_ file: ShortcutMenu, application: ShortcutMenu? = nil, navigation: ShortcutMenu? = nil) {
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
        // Buttons that declare a chord, and the window toolbar's own.
        let ids = presenter.chrome.ids("accessibilityKeyShortcuts").union(presenter.toolbar.items.keys)
        return ids.sorted().compactMap { presenter.views[$0] }.filter {
            $0.kind == "button" && $0.handlers.contains("press") && presenter.toolbar.visible($0)
                && ($0.props["accessibilityKeyShortcuts"] != nil || presenter.toolbar.contains($0))
        }
    }
    func perform(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown,
              (event.window?.firstResponder as? NSTextInputClient)?.hasMarkedText() != true,
              let view = nodes().first(where: {
                  !$0.inert && $0.window === event.window && $0.window?.attachedSheet == nil
                      && declarations($0).contains(where: { $0.matches(event) && $0.permits(event.window?.firstResponder) })
              }) else { return false }
        if !event.isARepeat && !view.disabled { presenter?.press(view.id) }
        return true
    }
    fileprivate func menuEquivalent(_ item: NSMenuItem, matching event: NSEvent? = nil) -> String {
        guard let id = (item.representedObject as? NSNumber)?.uint32Value,
              let view = presenter?.views[id],
              let shortcut = declarations(view).first(where: { $0.modifiers.contains(.command) }) else { return "" }
        if let event, !shortcut.matches(event) { return "" }
        return shortcut.keyEquivalent
    }
    func sync() {
        guard let fileMenu else { return }
        var file: [NSMenuItem] = []
        var application: [NSMenuItem] = []
        var navigation: [NSMenuItem] = []
        var live: Set<UInt32> = []
        for view in nodes() {
            // A platform alternative does not create a second menu item.
            let shortcut = declarations(view).first(where: { $0.modifiers.contains(.command) })
            guard shortcut != nil || presenter?.toolbar.contains(view) == true else { continue }
            live.insert(view.id)
            let item = items[view.id] ?? NSMenuItem(title: "", action: #selector(activate(_:)), keyEquivalent: "")
            items[view.id] = item
            item.title = title(view)
            item.keyEquivalent = shortcut?.keyEquivalent ?? ""
            item.keyEquivalentModifierMask = shortcut?.modifiers ?? []
            item.target = self
            item.representedObject = NSNumber(value: view.id)
            item.isEnabled = !view.disabled && !view.inert && view.window?.attachedSheet == nil
            item.state = view.props["accessibilitySelected"] == "true" ? .on : .off
            // Placement follows declared semantics and standard chords, never
            // app names, test ids, or a second registry of commands.
            if shortcut?.key == ",", shortcut?.modifiers == .command, applicationMenu != nil {
                item.title = "Settings…"
                application.append(item)
            } else if navigationMenu != nil, let shortcut, isNavigation(view, shortcut: shortcut) {
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
        // Menu selection by keyboard need not use the declared equivalent.
        // AppKit owns that selection; repeat and composition still cannot fire it.
        if let event = NSApp.currentEvent, event.type == .keyDown,
           event.isARepeat || (NSApp.keyWindow?.firstResponder as? NSTextInputClient)?.hasMarkedText() == true { return }
        presenter?.press(id)
    }
}
#endif

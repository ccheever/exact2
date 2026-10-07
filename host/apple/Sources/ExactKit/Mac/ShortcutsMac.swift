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
        if characters?.lowercased() == keyEquivalent.lowercased() { return true }
        // A key that types no Latin character (ㅠ on B under Korean 2-Set,
        // Russian, Greek) is the one the ASCII-capable layout puts there, as
        // the web falls back to `code`. A Latin layout's own character
        // (AZERTY, Dvorak, German's ö) stays the key, so no chord fires twice.
        guard let typed = characters, Self.typesNonLatin(typed),
              let physical = KeyCodes.asciiCharacters(event.keyCode, shift: event.modifierFlags.contains(.shift),
                                                       option: modifiers.contains(.option)) else { return false }
        return physical.lowercased() == keyEquivalent.lowercased()
    }
    /// Characters outside ASCII, the Latin script and AppKit's function-key range.
    static func typesNonLatin(_ characters: String) -> Bool {
        characters.unicodeScalars.contains { $0.value > 0x7f && !(0xf700...0xf8ff).contains($0.value) }
            && characters.range(of: "\\p{Latin}", options: .regularExpression) == nil
    }
    /// The chords a Mac's Edit menu holds (Apple's HIG): Undo, Redo, Cut,
    /// Copy, Paste and its variants, Select All, Duplicate, and Find with
    /// its next and previous (studio diary R16).
    var isEdit: Bool {
        if isPasteVariant { return true }
        switch (key, modifiers) {
        case ("z", .command), ("z", [.command, .shift]), ("x", .command), ("c", .command), ("v", .command),
             ("a", .command), ("d", .command), ("f", .command), ("g", .command), ("g", [.command, .shift]): return true
        default: return false
        }
    }
    /// A paste variant, which the HIG puts right after Paste: Paste and
    /// Match Style ⌥⇧⌘V (TextEdit, Safari, Chrome) and ⇧⌘V, Chrome's other
    /// chord for it and many apps' Paste as Text (#141).
    var isPasteVariant: Bool {
        key == "v" && (modifiers == [.command, .shift] || modifiers == [.command, .shift, .option])
    }
    /// The chords a Mac's View menu holds: zoom in, out and to actual size,
    /// and the Control-Command ones (Show Sidebar ⌃⌘S, Full Screen ⌃⌘F).
    var isView: Bool {
        if modifiers == [.command, .control] { return true }
        return modifiers.subtracting(.shift) == .command && ["=", "Plus", "+", "-", "0"].contains(key)
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
        // AppKit compares the typed character with the item's, so a key that
        // types no Latin one (ㅠ under Korean 2-Set) misses ⌘B: the item its
        // physical key's chord names runs here instead (its action validates,
        // and AppKit too takes a matching key for a disabled item).
        if let typed = event.charactersIgnoringModifiers, Shortcut.typesNonLatin(typed),
           let index = items.firstIndex(where: { item in
               (item.target as? ShortcutHost)?.menuEquivalent(item, matching: event).isEmpty == false
           }) {
            performActionForItem(at: index)
            return true
        }
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
    private weak var editMenu: NSMenu?
    private weak var viewMenu: NSMenu?
    private var items: [UInt32: NSMenuItem] = [:]
    private let fileSeparator = NSMenuItem.separator()
    private let settingsSeparator = NSMenuItem.separator()
    private let editSeparator = NSMenuItem.separator()
    private let viewSeparator = NSMenuItem.separator()
    /// The host's own items an app command stands in for (Edit ▸ Undo, …),
    /// hidden while it does; and those whose chord an app command took, with
    /// the chord to give back (studio diary R16).
    private var replaced: [NSMenuItem] = []
    private var unbound: [(NSMenuItem, String, NSEvent.ModifierFlags)] = []
    init(presenter: Presenter) { self.presenter = presenter }
    func attach(_ file: ShortcutMenu, application: ShortcutMenu? = nil, navigation: ShortcutMenu? = nil,
                edit: ShortcutMenu? = nil, view: ShortcutMenu? = nil) {
        for item in Array(items.values) + [fileSeparator, settingsSeparator, editSeparator, viewSeparator] { item.menu?.removeItem(item) }
        replaced = []
        unbound = []
        fileMenu = file
        applicationMenu = application
        navigationMenu = navigation
        editMenu = edit
        viewMenu = view
        sync()
    }

    private func declarations(_ view: NodeView) -> [Shortcut] {
        (view.props["accessibilityKeyShortcuts"] ?? "").split(whereSeparator: \.isWhitespace).compactMap(Shortcut.init)
    }
    private func nodes() -> [NodeView] {
        guard let presenter else { return [] }
        // Buttons that declare a chord, and the window toolbar's own.
        let ids = presenter.chrome.ids("accessibilityKeyShortcuts").union(presenter.toolbar.items.keys)
        // A tab a segment shows is shown, though its view is hidden (astra's code review).
        return ids.sorted().compactMap { presenter.views[$0] }.filter {
            $0.isButton && $0.pressable && (presenter.segments.shown($0) ?? presenter.toolbar.visible($0))
                && ($0.props["accessibilityKeyShortcuts"] != nil || presenter.toolbar.contains($0))
        }
    }
    func perform(_ event: NSEvent) -> Bool {
        // Escape belongs to the input method while composition is active.
        if event.keyCode == 53, let editor = event.window?.firstResponder as? NSTextView, editor.hasMarkedText() { return false }
        let focus = presenter?.keyTarget(event.window?.firstResponder)
        guard event.type == .keyDown,
              (event.window?.firstResponder as? NSTextInputClient)?.hasMarkedText() != true,
              let view = nodes().first(where: { node in
                  !node.inert && node.window === event.window && node.window?.attachedSheet == nil
                      && declarations(node).contains(where: { $0.matches(event) && $0.permits(event.window?.firstResponder) })
                      && presenter?.shortcutAdmits(node, key: NodeView.keyName(event), held: KeyCodes.held(event.modifierFlags), focus: focus) == true
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
        var editItems: [NSMenuItem] = []
        var pastes: [NSMenuItem] = []
        var viewItems: [NSMenuItem] = []
        var live: Set<UInt32> = []
        var chords: [(NSMenuItem, Shortcut?)] = []
        for view in nodes() {
            // A platform alternative does not create a second menu item.
            let shortcut = declarations(view).first(where: { $0.modifiers.contains(.command) })
            guard shortcut != nil || presenter?.toolbar.contains(view) == true else { continue }
            live.insert(view.id)
            let item = items[view.id] ?? NSMenuItem(title: "", action: #selector(activate(_:)), keyEquivalent: "")
            items[view.id] = item
            item.title = title(view)
            // A symbol a button shows is its item's image (LLP 1069.011.000 D7).
            item.image = view.isButton ? view.face?.symbol.flatMap { NSImage(systemSymbolName: $0, accessibilityDescription: nil) } : nil
            chords.append((item, shortcut))
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
            } else if editMenu != nil, let shortcut, shortcut.isPasteVariant {
                pastes.append(item)
            } else if editMenu != nil, let shortcut, shortcut.isEdit {
                editItems.append(item)
            } else if viewMenu != nil, let shortcut, shortcut.isView {
                viewItems.append(item)
            } else {
                file.append(item)
            }
        }
        for id in Array(items.keys) where !live.contains(id) {
            if let item = items.removeValue(forKey: id) { item.menu?.removeItem(item) }
        }
        // AppKit keeps one item per chord in the bar: giving an item a chord
        // another has takes it from that one. So the host's items give theirs
        // up before the app's take them, and get them back only once no
        // command claims them (studio diary R16).
        let claimed = chords.compactMap { $0.1.map { ($0.keyEquivalent.lowercased(), $0.modifiers) } }
        release(keeping: claimed)
        claim(claimed)
        for (item, shortcut) in chords {
            item.keyEquivalent = shortcut?.keyEquivalent ?? ""
            item.keyEquivalentModifierMask = shortcut?.modifiers ?? []
        }
        if let editMenu { placeEdit(editItems, pastes: pastes, in: editMenu) }
        if let viewMenu {
            if !viewItems.isEmpty { viewItems.append(viewSeparator) } else { viewSeparator.menu?.removeItem(viewSeparator) }
            place(viewItems, in: viewMenu, at: 0)
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
    /// Edit's commands (studio diary R16), where Apple's HIG puts them: one
    /// whose chord is a host item's own — Undo ⌘Z, Redo ⇧⌘Z, Cut, Copy,
    /// Paste, Select All — stands in its place, the host's hidden while it
    /// does (`claim`), so Edit ▸ Undo is the app's "Undo Move" whatever has
    /// the focus, as its ⌘Z already is; a paste variant follows Paste, as
    /// Paste and Match Style does; the rest follow Select All, after a
    /// separator, ahead of Speech and of what AppKit appends (#141).
    private func placeEdit(_ commands: [NSMenuItem], pastes: [NSMenuItem], in menu: NSMenu) {
        var extra: [NSMenuItem] = []
        for item in commands {
            if let host = unbound.first(where: { host, key, mask in
                host.menu === menu && replaced.contains { $0 === host } && Self.same((key.lowercased(), mask), (item.keyEquivalent.lowercased(), item.keyEquivalentModifierMask))
            })?.0 {
                if item.menu !== menu || menu.index(of: item) != menu.index(of: host) {
                    item.menu?.removeItem(item)
                    menu.insertItem(item, at: menu.index(of: host))
                }
            } else {
                extra.append(item)
            }
        }
        for item in pastes + [editSeparator] + extra { item.menu?.removeItem(item) }
        // After the host's own item: past the command standing in for it.
        let after = { (action: Selector) in menu.items.firstIndex { $0.action == action && !($0.target is ShortcutHost) }.map { $0 + 1 } }
        var rest = extra
        if let start = after(#selector(NSText.paste(_:))) {
            for (offset, item) in pastes.enumerated() { menu.insertItem(item, at: start + offset) }
        } else {
            rest = pastes + extra
        }
        guard !rest.isEmpty else { return }
        let start = after(#selector(EditMenuTarget.selectAll(_:))) ?? menu.numberOfItems
        for (offset, item) in ([editSeparator] + rest).enumerated() { menu.insertItem(item, at: start + offset) }
    }
    /// The host's items whose chord no command claims any longer get it
    /// back, and come back into view.
    private func release(keeping claimed: [(String, NSEvent.ModifierFlags)]) {
        let held = { (key: String, mask: NSEvent.ModifierFlags) in claimed.contains { Self.same($0, (key.lowercased(), mask)) } }
        unbound.removeAll { host, key, mask in
            guard !held(key, mask) else { return false }
            host.keyEquivalent = key
            host.keyEquivalentModifierMask = mask
            host.isHidden = false
            replaced.removeAll { $0 === host }
            return true
        }
    }
    /// No chord twice in the bar: a host item whose chord a command declares
    /// gives it up and keeps its place (File ▸ New Window beside the app's
    /// ⌘N, Develop ▸ App Info beside its ⌘D) — or, for Edit's own roles,
    /// steps aside for the command (`placeEdit`).
    private func claim(_ claimed: [(String, NSEvent.ModifierFlags)]) {
        guard let bar = fileMenu?.supermenu else { return }
        var menus = bar.items.compactMap(\.submenu)
        while let menu = menus.popLast() {
            for host in menu.items where !(host.target is ShortcutHost) {
                if let sub = host.submenu { menus.append(sub) }
                guard !host.keyEquivalent.isEmpty,
                      claimed.contains(where: { Self.sameChord(host, key: $0.0, $0.1) }) else { continue }
                unbound.append((host, host.keyEquivalent, host.keyEquivalentModifierMask))
                if menu === editMenu, Self.editRoles.contains(host.action ?? Selector("")) {
                    host.isHidden = true
                    replaced.append(host)
                }
                host.keyEquivalent = ""
            }
        }
    }
    /// The host's Edit items an app command may stand in for.
    private static let editRoles: Set<Selector> = [Selector(("undo:")), Selector(("redo:")), #selector(NSText.cut(_:)),
                                                   #selector(NSText.copy(_:)), #selector(NSText.paste(_:)), #selector(EditMenuTarget.selectAll(_:))]
    private static let chordMask: NSEvent.ModifierFlags = [.command, .shift, .option, .control]
    private static func same(_ a: (String, NSEvent.ModifierFlags), _ b: (String, NSEvent.ModifierFlags)) -> Bool {
        a.0 == b.0 && a.1.intersection(chordMask) == b.1.intersection(chordMask)
    }
    /// Whether `item` answers `key` with `mask`.
    private static func sameChord(_ item: NSMenuItem, key: String, _ mask: NSEvent.ModifierFlags) -> Bool {
        !key.isEmpty && same((item.keyEquivalent.lowercased(), item.keyEquivalentModifierMask), (key.lowercased(), mask))
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
        // A native button's children are its face, not views (LLP 1069.011.000 D1).
        if view.isNativeButton { return view.face?.title ?? "" }
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
        // AppKit owns that selection; repeat and composition still cannot fire
        // it, nor a chord that ended one (`Presenter.endComposition`, #140).
        if let event = NSApp.currentEvent, event.type == .keyDown,
           event.isARepeat || (NSApp.keyWindow?.firstResponder as? NSTextInputClient)?.hasMarkedText() == true
            || presenter?.endedComposition(event) == true { return }
        presenter?.press(id)
    }
}
#endif

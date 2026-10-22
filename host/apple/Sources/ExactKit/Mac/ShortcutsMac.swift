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
    // LLP 1021 §5.3: AppKit can insert these after the app installs its bar.
    // Keep its objects for restoration, suppressing presentation and equivalents.
    private var removalIDs = Set<String>()
    fileprivate func removeStandardItems(_ ids: Set<String>) {
        removalIDs = ids
        watchTracking()
        applyRemovals(late: tracking)
    }
    /// What each removed item was: hidden (a hidden alternate's chord), and
    /// whether its chord worked hidden.
    private var removed: [(NSMenuItem, Bool, Bool)] = []
    /// Items no removal holds any more, with what a write put off has yet
    /// to give back.
    private var unrestored: [(NSMenuItem, Bool, Bool)] = []
    override func update() {
        super.update()
        applyRemovals(late: false)
    }
    override func insertItem(_ newItem: NSMenuItem, at index: Int) {
        super.insertItem(newItem, at: index)
        applyRemovals(late: false)
    }
    /// Records `hidden` as what `item` goes back to when its removal ends,
    /// if a removal here holds it or its restoration is still to be
    /// written. The host's sync sets its own items' state through this, so
    /// it never writes a state the removal then reverses.
    fileprivate func holdHidden(_ item: NSMenuItem, _ hidden: Bool) -> Bool {
        var held = false
        for index in removed.indices where removed[index].0 === item { removed[index].1 = hidden; held = true }
        for index in unrestored.indices where unrestored[index].0 === item { unrestored[index].1 = hidden; held = true }
        return held
    }
    // An open menu is a table AppKit lays out, and a plan batch can arrive
    // inside that layout: a menu item's own view entering the menu's window
    // can make it key (Help's search field), which posts the window facts.
    // An item shown or hidden there inserts or deletes a row, and AppKit
    // throws (`-[NSTableRowData insertRowsAtIndexes:withRowAnimation:]`).
    // So a removal writes only what changes, and a batch's change while the
    // menu is open is written on the next turn of the run loop, outside any
    // layout.
    private var tracking = false, writeScheduled = false
    private var trackingObservers: [NSObjectProtocol] = []
    private func watchTracking() {
        guard trackingObservers.isEmpty else { return }
        // The object is the menu bar for its menus, or a popped-up menu.
        let center = NotificationCenter.default
        trackingObservers = [(NSMenu.didBeginTrackingNotification, true), (NSMenu.didEndTrackingNotification, false)].map { name, begins in
            center.addObserver(forName: name, object: nil, queue: nil) { [weak self] note in
                guard let self, let root = note.object as? NSMenu,
                      sequence(first: self as NSMenu, next: \.supermenu).contains(where: { $0 === root }) else { return }
                self.tracking = begins
            }
        }
    }
    deinit { trackingObservers.forEach(NotificationCenter.default.removeObserver) }
    /// `late`: write on the run loop's next turn.
    private func applyRemovals(late: Bool) {
        guard !removalIDs.isEmpty || !removed.isEmpty || !unrestored.isEmpty else { return }
        let previous = unrestored + removed
        // Each item as it was before any removal: as saved for one removed
        // or still to be restored, as it is for any other.
        var saved: [ObjectIdentifier: (Bool, Bool)] = [:]
        for (item, hidden, hiddenEquivalent) in previous where saved[ObjectIdentifier(item)] == nil {
            saved[ObjectIdentifier(item)] = (hidden, hiddenEquivalent)
        }
        let restored = { (item: NSMenuItem) in saved[ObjectIdentifier(item)] ?? (item.isHidden, item.allowsKeyEquivalentWhenHidden) }
        var next: [(NSMenuItem, Bool, Bool)] = []
        // A hidden submenu's items keep their chords (AppKit), so a removed
        // submenu's go with it; the app's own commands stay.
        func remove(_ item: NSMenuItem) {
            let (hidden, hiddenEquivalent) = restored(item)
            next.append((item, hidden, hiddenEquivalent))
            for child in item.submenu?.items ?? [] where !(child.target is ShortcutHost) { remove(child) }
        }
        // An item hidden on its own is left out, unless a removal already
        // holds it: the host may hide one of its own items under a removal.
        for item in items where !(item.target is ShortcutHost) {
            let (hidden, hiddenEquivalent) = restored(item)
            guard !hidden || hiddenEquivalent || removed.contains(where: { $0.0 === item }),
                  let id = Self.standardID(item), removalIDs.contains(id) else { continue }
            remove(item)
        }
        let kept = Set(next.map { ObjectIdentifier($0.0) })
        let restores = previous.filter { item, hidden, hiddenEquivalent in
            !kept.contains(ObjectIdentifier(item)) && (item.isHidden != hidden || item.allowsKeyEquivalentWhenHidden != hiddenEquivalent)
        }
        // Removed from now, though written later: `performKeyEquivalent`
        // leaves its chord out at once.
        removed = next
        let hides = next.map(\.0).filter { !$0.isHidden || $0.allowsKeyEquivalentWhenHidden }
        if late && (!restores.isEmpty || !hides.isEmpty) {
            unrestored = restores
            guard !writeScheduled else { return }
            writeScheduled = true
            RunLoop.main.perform(inModes: [.common]) { [weak self] in
                self?.writeScheduled = false
                self?.applyRemovals(late: false)
            }
            return
        }
        unrestored = []
        for (item, hidden, hiddenEquivalent) in restores.reversed() {
            if item.isHidden != hidden { item.isHidden = hidden }
            if item.allowsKeyEquivalentWhenHidden != hiddenEquivalent { item.allowsKeyEquivalentWhenHidden = hiddenEquivalent }
        }
        for item in hides {
            if item.allowsKeyEquivalentWhenHidden { item.allowsKeyEquivalentWhenHidden = false }
            if !item.isHidden { item.isHidden = true }
        }
    }
    static func standardID(_ item: NSMenuItem) -> String? {
        // AutoFill has no command of its own: AppKit identifies the submenu,
        // and the host its own submenus (`DevMenu.removable`).
        if item.identifier?.rawValue == "_NSMenuItemAutoFillIdentifier" { return "autofill" }
        if let id = item.identifier, let name = DevMenu.removable.first(where: { $0.value == id })?.key { return name }
        // Paste and Match Style is the ⌥⇧⌘V `pasteAsPlainText:`; Edit's ⇧⌘V
        // one, when an app command stands in for it, is the app's.
        if item.action == #selector(NSTextView.pasteAsPlainText(_:)) { return "paste-and-match-style" }
        switch item.action.map(NSStringFromSelector) {
        case "back:": return "back"
        case "showHelp:": return "app-help"
        case "actualSize:": return "actual-size"
        case "zoomIn:": return "zoom-in"
        case "zoomOut:": return "zoom-out"
        case "closeAll:": return "close-all"
        case "toggleTabBar:": return "show-tab-bar"
        case "toggleTabOverview:": return "show-all-tabs"
        case "toggleFullScreen:": return "enter-full-screen"
        case "performMiniaturize:": return "minimize"
        case "performZoom:": return "zoom"
        case "arrangeInFront:": return "bring-all-to-front"
        case "selectPreviousTab:": return "show-previous-tab"
        case "selectNextTab:": return "show-next-tab"
        case "moveTabToNewWindow:": return "move-tab-to-new-window"
        case "mergeAllWindows:": return "merge-all-windows"
        default: return nil
        }
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // A removed item's chord, and its submenu's, is out of the lookup:
        // AppKit sends a hidden item's chord whatever its
        // `allowsKeyEquivalentWhenHidden` (measured, macOS 26.6).
        let silenced = removed.compactMap { item, _, _ in item.keyEquivalent.isEmpty ? nil : (item, item.keyEquivalent) }
        for (item, _) in silenced { item.keyEquivalent = "" }
        defer { for (item, key) in silenced where item.keyEquivalent.isEmpty { item.keyEquivalent = key } }
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
    private weak var windowMenu: NSMenu?
    private weak var helpMenu: NSMenu?
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
    /// The commands in the app menu or Help: the app's, not the window's
    /// (`enabled`).
    private var appWide: Set<UInt32> = []
    init(presenter: Presenter) { self.presenter = presenter }
    func attach(_ file: ShortcutMenu, application: ShortcutMenu? = nil, navigation: ShortcutMenu? = nil,
                edit: ShortcutMenu? = nil, view: ShortcutMenu? = nil, window: ShortcutMenu? = nil, help: ShortcutMenu? = nil) {
        for item in Array(items.values) + [fileSeparator, settingsSeparator, editSeparator, viewSeparator] { item.menu?.removeItem(item) }
        replaced = []
        unbound = []
        fileMenu = file
        applicationMenu = application
        navigationMenu = navigation
        editMenu = edit
        viewMenu = view
        windowMenu = window
        helpMenu = help
        sync()
    }

    private func declarations(_ view: NodeView) -> [Shortcut] {
        (view.props["accessibilityKeyShortcuts"] ?? "").split(whereSeparator: \.isWhitespace).compactMap(Shortcut.init)
    }
    private func nodes() -> [NodeView] {
        guard let presenter else { return [] }
        // Buttons that declare a chord or menu, and the window toolbar's own.
        let ids = presenter.chrome.ids("accessibilityKeyShortcuts").union(presenter.chrome.ids("menuPlacement")).union(presenter.toolbar.commands)
        // A tab a segment shows is shown, though its view is hidden (astra's code review).
        return ids.sorted().compactMap { presenter.views[$0] }.filter {
            $0.isButton && ($0.pressable || $0.isSurfaceControl) && (presenter.segments.shown($0) ?? presenter.sidebar.shows($0) ?? presenter.toolbar.visible($0))
                && ($0.props["accessibilityKeyShortcuts"] != nil || $0.props["menuPlacement"] != nil || presenter.toolbar.contains($0))
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
        if !event.isARepeat && !view.disabled { presenter?.pressShortcut(view) }
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
        var windowItems: [NSMenuItem] = []
        var helpItems: [NSMenuItem] = []
        var about: [NSMenuItem] = []
        var concealed: [NSMenuItem] = []
        var live: Set<UInt32> = []
        var chords: [(NSMenuItem, Shortcut?)] = []
        appWide = []
        for view in nodes() {
            // A platform alternative does not create a second menu item.
            let shortcut = declarations(view).first(where: { $0.modifiers.contains(.command) })
            let placement = view.props["menuPlacement"]
            guard shortcut != nil || placement != nil || presenter?.toolbar.contains(view) == true else { continue }
            live.insert(view.id)
            let item = items[view.id] ?? AccessibleMenuItem(title: "", action: #selector(activate(_:)), keyEquivalent: "")
            items[view.id] = item
            (item as? AccessibleMenuItem)?.owner = view
            item.title = title(view)
            // A symbol a button shows is its item's image (LLP 1069.011.000 D7).
            item.image = view.isButton ? view.face?.symbol.flatMap { FaceSymbol.image($0) } : nil
            chords.append((item, shortcut))
            item.target = self
            item.representedObject = NSNumber(value: view.id)
            // `none`: no item to see, the chord a hidden key equivalent.
            item.isHidden = placement == "none"
            item.allowsKeyEquivalentWhenHidden = placement == "none"
            let settings = placement == nil && shortcut?.key == "," && shortcut?.modifiers == .command && applicationMenu != nil
            if settings || ["about", "app", "help"].contains(placement ?? "") { appWide.insert(view.id) }
            item.isEnabled = enabled(view)
            item.state = view.props["accessibilitySelected"] == "true" ? .on : .off
            // Placement follows declared semantics and standard chords, never
            // app names, test ids, or a second registry of commands.
            if let placement {
                switch placement {
                case "about": about.append(item)
                case "none": concealed.append(item)
                case "app": application.append(item)
                case "edit":
                    if shortcut?.isPasteVariant == true { pastes.append(item) }
                    else { editItems.append(item) }
                case "view": viewItems.append(item)
                case "go": navigation.append(item)
                case "window": windowItems.append(item)
                case "help": helpItems.append(item)
                default: file.append(item)
                }
            } else if shortcut?.key == ",", shortcut?.modifiers == .command, applicationMenu != nil {
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
        let standing = chords.filter { !$0.0.isHidden }.compactMap { $0.1.map { ($0.keyEquivalent.lowercased(), $0.modifiers) } }
        release(keeping: claimed)
        claim(claimed)
        stepAside(standing: standing)
        let removals = Set((presenter?.root.subviews ?? []).compactMap { ($0 as? NodeView)?.props["menuRemove"] }
            .flatMap { $0.split(whereSeparator: \.isWhitespace).map(String.init) })
        for (item, shortcut) in chords {
            item.keyEquivalent = shortcut?.keyEquivalent ?? ""
            item.keyEquivalentModifierMask = shortcut?.modifiers ?? []
        }
        if let windowMenu { place(windowItems, in: windowMenu, at: 0) }
        if let helpMenu { place(helpItems, in: helpMenu, at: 0) }
        if let editMenu { placeEdit(editItems, pastes: pastes, in: editMenu, removals: removals) }
        if let viewMenu {
            // A command standing in for Actual Size, Zoom In or Zoom Out takes
            // its place; the rest lead View.
            var top: [NSMenuItem] = []
            for item in viewItems {
                let chord = (item.keyEquivalent.lowercased(), item.keyEquivalentModifierMask)
                guard let host = unbound.first(where: { host, key, mask in
                    host.target is ZoomMenuTarget && host.menu === viewMenu && host.identifier != DevMenu.zoomAlternate
                        && replaced.contains { $0 === host } && Self.answers(host, key: key, mask, to: chord)
                })?.0 else { top.append(item); continue }
                if item.menu !== viewMenu || viewMenu.index(of: item) != viewMenu.index(of: host) {
                    item.menu?.removeItem(item)
                    viewMenu.insertItem(item, at: viewMenu.index(of: host))
                }
            }
            if !top.isEmpty { top.append(viewSeparator) } else { viewSeparator.menu?.removeItem(viewSeparator) }
            place(top, in: viewMenu, at: 0)
            // Show Sidebar is a window with a sidebar's (LLP 1115.002 D3).
            for item in viewMenu.items where item.action == #selector(NSSplitViewController.toggleSidebar(_:)) {
                item.isHidden = presenter?.sidebar.installed != true
            }
        }
        // Reconcile only our own items. In particular, Open… and Close must
        // survive the next plan batch. Existing command objects stay stable.
        if !file.isEmpty { file.append(fileSeparator) } else { fileSeparator.menu?.removeItem(fileSeparator) }
        // Concealed commands first: hidden, they leave File as it looks.
        place(concealed + file, in: fileMenu, at: 0)
        if let applicationMenu {
            // Directly under About, as Check for Updates… is in apps that
            // update themselves; the rest after its separator, with Settings….
            place(about, in: applicationMenu, at: min(1, applicationMenu.numberOfItems))
            if !application.isEmpty { application.append(settingsSeparator) }
            else { settingsSeparator.menu?.removeItem(settingsSeparator) }
            place(application, in: applicationMenu, at: min(2 + about.count, applicationMenu.numberOfItems))
        }
        if let navigationMenu {
            // The host's own Back (LLP 1115.000 D5.1) leads the menu while
            // the app has a navigation root, unless an app command took its
            // ⌘[ and stands in for it (`claim`).
            let own = navigationMenu.items.filter { !($0.target is ShortcutHost) }
            let routed = presenter?.carrying("navigationBack").isEmpty == false
            for item in own { setHidden(item, !routed || replaced.contains { $0 === item }) }
            // History commands precede destinations regardless of where the
            // toolbar sits in the authored tree; destinations keep tree order.
            let history = navigation.filter { $0.keyEquivalentModifierMask == .command && ["[", "]"].contains($0.keyEquivalent) }
            navigation = history.sorted { $0.keyEquivalent < $1.keyEquivalent }
                + navigation.filter { item in !history.contains(where: { $0 === item }) }
            place(navigation, in: navigationMenu, at: own.count)
        }
        for menu in [fileMenu, applicationMenu, navigationMenu, editMenu, viewMenu, windowMenu, helpMenu] {
            (menu as? ShortcutMenu)?.removeStandardItems(removals)
        }
        // Go shows while it has an item to show, Back once `back` removed it.
        if let navigationMenu {
            navigationMenu.supermenu?.items.first(where: { $0.submenu === navigationMenu })?.isHidden = navigationMenu.items.allSatisfy(\.isHidden)
        }
        // What a removal restored, a command still stands in for.
        for item in replaced where !item.isHidden { item.isHidden = true }
        // View's zoom group keeps its separator while it shows an item.
        if let viewMenu, let separator = viewMenu.items.first(where: { $0.identifier == DevMenu.zoomSeparator }) {
            let start = viewSeparator.menu === viewMenu ? viewMenu.index(of: viewSeparator) + 1 : 0, end = viewMenu.index(of: separator)
            separator.isHidden = start >= end || !viewMenu.items[start..<end].contains { !$0.isHidden && !$0.isSeparatorItem }
        }
        // Develop goes from the bar; its chords stay, as a hidden menu's do.
        fileMenu.supermenu?.items.first { $0.identifier == DevMenu.removable["develop"] }?.isHidden = removals.contains("develop")
    }
    /// Edit's commands (studio diary R16), where Apple's HIG puts them: one
    /// whose chord is a host item's own — Undo ⌘Z, Redo ⇧⌘Z, Cut, Copy,
    /// Paste, Select All — stands in its place, the host's hidden while it
    /// does (`claim`), so Edit ▸ Undo is the app's "Undo Move" whatever has
    /// the focus, as its ⌘Z already is; a paste variant follows Paste, as
    /// Paste and Match Style does; the rest follow Select All, after a
    /// separator, ahead of Speech and of what AppKit appends (#141).
    private func placeEdit(_ commands: [NSMenuItem], pastes: [NSMenuItem], in menu: NSMenu, removals: Set<String>) {
        var extra: [NSMenuItem] = []
        // A removed submenu (`find`) holds no command: it would not show.
        let removedSubmenu = { (sub: NSMenu?) in
            menu.items.first { $0.submenu === sub }.flatMap(ShortcutMenu.standardID).map(removals.contains) == true
        }
        for item in commands {
            // In Edit or in its Find submenu, where the host's item was.
            if let host = unbound.first(where: { host, key, mask in
                (host.menu === menu || host.menu?.supermenu === menu && !removedSubmenu(host.menu)) && replaced.contains { $0 === host }
                    && Self.same((key.lowercased(), mask), (item.keyEquivalent.lowercased(), item.keyEquivalentModifierMask))
            })?.0, let into = host.menu {
                if item.menu !== into || into.index(of: item) != into.index(of: host) {
                    item.menu?.removeItem(item)
                    into.insertItem(item, at: into.index(of: host))
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
        unbound.removeAll { host, key, mask in
            guard !claimed.contains(where: { Self.answers(host, key: key, mask, to: $0) }) else { return false }
            host.keyEquivalent = key
            host.keyEquivalentModifierMask = mask
            host.isHidden = host.identifier == DevMenu.zoomAlternate
            replaced.removeAll { $0 === host }
            return true
        }
    }
    /// No chord twice in the bar: a host item whose chord a command declares
    /// gives it up and keeps its place (File ▸ New Window beside the app's
    /// ⌘N, Develop ▸ App Info beside its ⌘D) — or, for Edit's own roles,
    /// steps aside for the command (`placeEdit`), as Go ▸ Back does for the
    /// app's own ⌘[ (LLP 1115.000 D5.1).
    private func claim(_ claimed: [(String, NSEvent.ModifierFlags)]) {
        guard let bar = fileMenu?.supermenu else { return }
        var menus = bar.items.compactMap(\.submenu)
        while let menu = menus.popLast() {
            for host in menu.items where !(host.target is ShortcutHost) {
                if let sub = host.submenu { menus.append(sub) }
                let (key, mask) = (host.keyEquivalent, host.keyEquivalentModifierMask)
                guard !key.isEmpty, claimed.contains(where: { Self.answers(host, key: key, mask, to: $0) }) else { continue }
                unbound.append((host, key, mask))
                host.keyEquivalent = ""
            }
        }
    }
    /// Edit's own roles, Go ▸ Back and View's zoom step aside for a command
    /// that stands in for them, and come back for one concealed (`none`),
    /// which takes the chord, not the place: decided again on every sync.
    private func stepAside(standing: [(String, NSEvent.ModifierFlags)]) {
        for (host, key, mask) in unbound {
            guard let menu = host.menu, ((menu === editMenu || menu.supermenu === editMenu) && Self.standsIn(host))
                || host.target is GoMenuTarget || host.target is ZoomMenuTarget else { continue }
            let stands = standing.contains { Self.answers(host, key: key, mask, to: $0) }
            replaced.removeAll { $0 === host }
            if stands { replaced.append(host) }
            setHidden(host, stands || host.identifier == DevMenu.zoomAlternate)
        }
    }
    /// Shows or hides one of the host's own items, or, while a removal
    /// holds it, sets what the removal's end gives back.
    private func setHidden(_ item: NSMenuItem, _ hidden: Bool) {
        let menus = [fileMenu, applicationMenu, navigationMenu, editMenu, viewMenu, windowMenu, helpMenu].compactMap { $0 as? ShortcutMenu }
        guard !menus.contains(where: { $0.holdHidden(item, hidden) }) else { return }
        if item.isHidden != hidden { item.isHidden = hidden }
    }
    /// The host's Edit items an app command may stand in for.
    private static let editRoles: Set<Selector> = [Selector(("undo:")), Selector(("redo:")), #selector(NSText.cut(_:)),
                                                   #selector(NSText.copy(_:)), #selector(NSText.paste(_:)), #selector(NSTextView.pasteAsPlainText(_:)),
                                                   #selector(EditMenuTarget.selectAll(_:))]
    /// Edit ▸ Find's Find…, Find Next and Find Previous: an app's ⌘F, ⌘G,
    /// ⇧⌘G stands in for them there (LLP 1115 D8).
    private static let findRoles = Set([NSTextFinder.Action.showFindInterface, .nextMatch, .previousMatch].map(\.rawValue))
    private static func standsIn(_ host: NSMenuItem) -> Bool {
        guard let action = host.action else { return false }
        return editRoles.contains(action) || action == #selector(NSResponder.performTextFinderAction(_:)) && findRoles.contains(host.tag)
    }
    private static let chordMask: NSEvent.ModifierFlags = [.command, .shift, .option, .control]
    private static func same(_ a: (String, NSEvent.ModifierFlags), _ b: (String, NSEvent.ModifierFlags)) -> Bool {
        a.0 == b.0 && a.1.intersection(chordMask) == b.1.intersection(chordMask)
    }
    /// Whether the host's `item`, whose chord is `key` with `mask`, answers
    /// a command's chord: the same one, or for Zoom In's ⌘+ any ⌘= or ⌘+,
    /// which AppKit sends it with or without ⇧.
    private static func answers(_ item: NSMenuItem, key: String, _ mask: NSEvent.ModifierFlags, to chord: (String, NSEvent.ModifierFlags)) -> Bool {
        if item.target is ZoomMenuTarget, key == "+" {
            return ["=", "+"].contains(chord.0) && chord.1.intersection([.command, .option, .control]) == .command
        }
        return !key.isEmpty && same((key.lowercased(), mask), (chord.0.lowercased(), chord.1))
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
    /// A window's command is enabled while its window is key with no sheet
    /// and no modal dialog over the button. One in the app menu or Help is
    /// the app's (LLP 1021 §5.3): enabled whatever window is key, under a
    /// sheet and under a modal dialog, as Settings… and Check for Updates…
    /// are in a hand-built Mac app; `disabled` and `inert` still disable it.
    private func enabled(_ view: NodeView, key: Bool = false) -> Bool {
        if view.disabled { return false }
        if appWide.contains(view.id) { return !view.authoredInert }
        return !view.inert && (!key || view.window === NSApp.keyWindow) && view.window?.attachedSheet == nil
    }
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard let id = (item.representedObject as? NSNumber)?.uint32Value,
              let view = nodes().first(where: { $0.id == id }) else { return false }
        return enabled(view, key: true)
    }
    @objc private func activate(_ item: NSMenuItem) {
        guard validateMenuItem(item), let id = (item.representedObject as? NSNumber)?.uint32Value else { return }
        // Menu selection by keyboard need not use the declared equivalent.
        // AppKit owns that selection; repeat and composition still cannot fire
        // it, nor a chord that ended one (`Presenter.endComposition`, #140).
        if let event = NSApp.currentEvent, event.type == .keyDown,
           event.isARepeat || (NSApp.keyWindow?.firstResponder as? NSTextInputClient)?.hasMarkedText() == true
            || presenter?.endedComposition(event) == true { return }
        if let view = presenter?.views[id] { presenter?.pressShortcut(view, appWide: appWide.contains(id)) }
    }
}
#endif

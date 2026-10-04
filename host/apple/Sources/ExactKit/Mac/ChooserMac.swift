// LLP 1021 "The chooser" on AppKit: a `role="alertdialog"` popover of the
// shape iOS presents as an action sheet — one or more press rows that hide
// it (the actions), at most one handlerless hide-only cancel, text rows (the
// message) — is an NSMenu popped up against its invoker by its
// `position-area`, as a button menu is.
// One item per action: its title, its image (a symbol, else its `img` once
// loaded), checked for `aria-checked`, dimmed when disabled, red when
// destructive. A chooser (more than one action, no text) is headed by its
// `aria-label` as a section header, the menu's own titling on macOS; a
// confirmation's text rows head it instead, as disabled lines. The cancel has
// no item: Escape and a click outside cancel a menu, and dispatch nothing.
// A chosen item presses its row by view id once, on the turn after the menu
// has ended (AppKit is still tracking the invoker when it sends the item's
// action, and the press's batch may unmount it), and only while the menu
// still shows what is there: owner live, every action live, in its popover,
// closing it, shown, its title and enablement as presented. Under the agent every popover
// stays painted (D4), so its drives tap the painted rows.
#if os(macOS)
import AppKit

extension MenuHost {
    /// An alertdialog presented as a menu, kept until its menu has ended.
    final class Confirmation {
        weak var source: NodeView?
        weak var popover: NodeView?
        /// An action as presented: the node, its title and whether it could be chosen.
        final class Presented {
            weak var node: NodeView?
            let title: String, enabled: Bool
            init(_ node: NodeView, title: String, enabled: Bool) { self.node = node; self.title = title; self.enabled = enabled }
        }
        let actions: [Presented]
        let heading: String?
        let message: [String]
        /// Chosen, cancelled or ended: no item is taken after.
        var finished = false
        /// The action chosen, pressed on the next turn unless cancelled first.
        var chosen: Int?
        func cancel() { finished = true; chosen = nil }
        init(source: NodeView, popover: NodeView, actions: [Presented], heading: String?, message: [String]) {
            self.source = source; self.popover = popover; self.actions = actions
            self.heading = heading; self.message = message
        }
    }
    /// An item's choice: its owner and the action's index as presented.
    private final class Choice: NSObject {
        let owner: Confirmation
        let index: Int
        init(_ owner: Confirmation, _ index: Int) { self.owner = owner; self.index = index }
    }

    func isConfirmation(_ pop: NodeView) -> Bool {
        pop.props["popover"] != nil && pop.props["accessibilityRole"] == "alertdialog"
    }
    func closes(_ row: NodeView, _ pop: NodeView) -> Bool {
        row.props["popovertarget"] == pop.props["id"] && row.props["popovertargetaction"] == "hide"
    }
    private func opens(_ source: NodeView, _ pop: NodeView) -> Bool {
        source.props["popovertarget"] == pop.props["id"] && source.props["popovertargetaction"] != "hide"
    }
    /// Live, pressable, enabled, not inert, and not hidden by the page — its
    /// own or an ancestor's `display: none` or hiding — though the popover
    /// itself is hidden in place while its menu presents it (iOS's `eligible`).
    private func choosable(_ action: NodeView, in pop: NodeView) -> Bool {
        guard live(action), action.handlers.contains("press"), !action.disabled, !action.inert else { return false }
        return !sequence(first: action as NSView, next: { self.parent(of: $0) }).contains { view in
            (view as? NodeView)?.style["display"]?.string == "none" || (view.isHidden && view !== pop)
        }
    }

    /// The owner for `pop` opened from `source`, or nil — logged with why —
    /// for a shape the menu cannot present (iOS's refusals, word for word).
    func confirmation(of pop: NodeView, from source: NodeView) -> Confirmation? {
        let children = pop.container.subviews.compactMap { $0 as? NodeView }
        let actions = children.filter { $0.isButton && $0.handlers.contains("press") }
        let cancels = children.filter { $0.isButton && !$0.handlers.contains("press") && closes($0, pop) }
        func refuse(_ why: String) -> Confirmation? {
            presenter?.session?.log("confirmation \(pop.props["id"] ?? "?") refused: \(why)")
            return nil
        }
        guard !actions.isEmpty else { return refuse("no action (a button with press)") }
        guard cancels.count <= 1 else { return refuse("\(cancels.count) cancels; at most one hide-only button") }
        guard children.allSatisfy({ child in child.kind == "text" || actions.contains { $0 === child } || cancels.contains { $0 === child } }) else {
            return refuse("only text, actions and one cancel may be its rows")
        }
        guard actions.allSatisfy({ closes($0, pop) }) else { return refuse("each action must also hide it (popovertargetaction=hide)") }
        guard actions.contains(where: { choosable($0, in: pop) }) else { return refuse("every action is disabled") }
        let texts = children.filter { $0.kind == "text" }.map(title(of:)).filter { !$0.isEmpty }
        let label = pop.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 }
        return Confirmation(source: source, popover: pop,
                            actions: actions.map { .init($0, title: title(of: $0), enabled: choosable($0, in: pop)) },
                            heading: texts.isEmpty && actions.count > 1 ? label : nil, message: texts)
    }

    /// The owner still shows what is there: invoker and popover live and
    /// paired, each action live, in the popover, closing it, its title and
    /// enablement as presented. A reused row given another provider ends
    /// the menu rather than dispatching under its old title.
    func valid(_ owner: Confirmation) -> Bool {
        guard let source = owner.source, let pop = owner.popover, live(source), live(pop),
              source.window != nil, !source.disabled, !source.inert, opens(source, pop), isConfirmation(pop) else { return false }
        return owner.actions.allSatisfy { presented(owner, $0) }
    }
    private func presented(_ owner: Confirmation, _ entry: Confirmation.Presented) -> Bool {
        guard let action = entry.node, let pop = owner.popover, live(action) else { return false }
        return closes(action, pop) && action.isDescendant(of: pop) && title(of: action) == entry.title
            && choosable(action, in: pop) == entry.enabled
    }

    func menu(of owner: Confirmation) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        if let heading = owner.heading { menu.addItem(.sectionHeader(title: heading)) }
        for line in owner.message { menu.addItem(Self.message(line)) }
        if !owner.message.isEmpty { menu.addItem(.separator()) }
        for (index, entry) in owner.actions.enumerated() {
            guard let row = entry.node else { continue }
            let item = NSMenuItem(title: entry.title, action: #selector(choose(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = Choice(owner, index)
            item.state = row.props["accessibilityChecked"] == "true" ? .on : .off
            item.isEnabled = entry.enabled
            item.image = image(of: row)
            // A destructive action reads red, as NSAlert's does; dimmed when it cannot be chosen.
            if row.props["destructive"] == "true", entry.enabled {
                item.attributedTitle = NSAttributedString(string: entry.title, attributes: [
                    .foregroundColor: NSColor.systemRed, .font: NSFont.menuFont(ofSize: 0)])
            }
            menu.addItem(item)
        }
        return menu
    }
    /// A confirmation's text: a line the menu shows and never chooses,
    /// wrapped to a menu's width rather than widening it.
    private static func message(_ text: String) -> NSMenuItem {
        let item = NSMenuItem(title: text, action: nil, keyEquivalent: "")
        item.isEnabled = false
        let label = NSTextField(wrappingLabelWithString: text)
        label.font = .menuFont(ofSize: NSFont.smallSystemFontSize)
        label.textColor = .secondaryLabelColor
        label.preferredMaxLayoutWidth = 260
        let size = label.fittingSize
        let box = NSView(frame: NSRect(x: 0, y: 0, width: size.width + 28, height: size.height + 6))
        label.frame = NSRect(x: 14, y: 3, width: size.width, height: size.height)
        box.addSubview(label)
        item.view = box
        return item
    }

    /// An item chosen: recorded now, pressed on the next main-queue turn.
    /// AppKit sends the action inside `popUp`, while it still tracks the
    /// menu in the invoker; a press whose batch unmounts the invoker (a
    /// confirmation that navigates back) must not run under that stack.
    @objc private func choose(_ sender: NSMenuItem) {
        guard let choice = sender.representedObject as? Choice, !choice.owner.finished,
              choice.owner.actions.indices.contains(choice.index) else { return }
        let owner = choice.owner
        owner.finished = true
        owner.chosen = choice.index
        choosing = owner
        DispatchQueue.main.async { [weak self, owner] in self?.dispatch(owner) }
    }
    /// The recorded choice, once, if the menu still showed what is there.
    private func dispatch(_ owner: Confirmation) {
        if choosing === owner { choosing = nil }
        guard let index = owner.chosen else { return }
        owner.chosen = nil
        let entry = owner.actions[index]
        guard valid(owner), entry.enabled, let node = entry.node, let pop = owner.popover,
              choosable(node, in: pop) else { return }
        presenter?.press(node.id, fromNativeMenu: true)
    }

    /// A row's item image: its symbol, custom or native (LLP 1069.011.000
    /// D5), else its `img` — a provider's own icon — once that has loaded.
    /// The menu reads what the hidden row already holds; opening it never
    /// fetches, and an image still loading is no image until the next open.
    func image(of row: NodeView) -> NSImage? {
        guard row.isButton else { return nil }
        if let symbol = row.face?.symbol { return NSImage(systemSymbolName: symbol, accessibilityDescription: nil) }
        guard !row.isNativeButton, let img = Self.firstImage(in: row) else { return nil }
        if img.imageSource?.hasPrefix("symbol:") == true { return img.image }
        return img.raster.map { Self.rowImage($0.image.image) }
    }
    /// A menu item's image is 16 points: fit the bitmap in it, keeping its ratio.
    static func rowImage(_ bitmap: CGImage) -> NSImage {
        let side: CGFloat = 16, natural = CGSize(width: bitmap.width, height: bitmap.height)
        let scale = min(side / max(natural.width, 1), side / max(natural.height, 1))
        return NSImage(cgImage: bitmap, size: NSSize(width: natural.width * scale, height: natural.height * scale))
    }
    private static func firstImage(in row: NodeView) -> NodeView? {
        for case let node as NodeView in row.container.subviews {
            if node.kind == "image" { return node }
            if let found = firstImage(in: node) { return found }
        }
        return nil
    }
}
#endif

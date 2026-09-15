// @ref LLP 1031 D5/D12; LLP 1001 §10 — window-owned presentation of
// Contract commands, not an embedded NativeView or a second action registry.
#if os(macOS)
import AppKit

final class WindowToolbarHost: NSObject, NSToolbarDelegate, NSToolbarItemValidation {
    unowned let presenter: Presenter
    private(set) weak var window: NSWindow?
    private(set) var toolbar: NSToolbar?
    private(set) var items: [UInt32: NSToolbarItem] = [:]
    private var symbols: [UInt32: String] = [:]
    private weak var owner: NodeView?
    private weak var heading: NodeView?
    private var order: [NSToolbarItem.Identifier] = []
    private var projected = false
    private var savedAccessibilityHidden = false
    private var syncing = false
    private var savedTitle = ""
    private var appliedTitle = ""
    private var refusal: String?
    var onChange: (() -> Void)?

    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Only the containing app calls this. Merely mounting an ExactView never
    /// claims window chrome. A pre-existing toolbar is never replaced.
    @discardableResult func attach(to window: NSWindow) -> Bool {
        if self.window === window { sync(); return true }
        guard window.toolbar == nil else { return false }
        detach()
        self.window = window
        sync()
        return true
    }

    func detach() { reset(); window = nil }

    private func refuse(_ reason: String) {
        if refusal != reason { presenter.session?.log("window toolbar: \(reason)"); refusal = reason }
        reset()
    }

    /// Restore the authored visibility before a batch can change it. Hidden
    /// here is a presentation detail, never the command's availability state.
    func prepare() {
        if projected {
            owner?.isHidden = false
            owner?.setAccessibilityHidden(savedAccessibilityHidden)
            projected = false
        }
    }

    func reset() {
        prepare()
        let hadToolbar = toolbar != nil
        let old = toolbar
        toolbar = nil
        if let window, let old, window.toolbar === old {
            window.toolbar = nil
            if window.title == appliedTitle { window.title = savedTitle }
        }
        items.removeAll(); symbols.removeAll(); order.removeAll(); owner = nil; heading = nil
        if hadToolbar { onChange?() }
    }

    private func children(_ node: NodeView) -> [NodeView] {
        node.container.subviews.compactMap { $0 as? NodeView }
    }

    private func text(_ node: NodeView) -> String {
        if node.kind == "text" { return node.paragraphSpec().runs.map(\.text).joined() }
        return children(node).map(text).filter { !$0.isEmpty }.joined(separator: " ")
    }

    /// Ignore just our own projection hiding; honor every authored/route hide.
    func visible(_ node: NodeView) -> Bool {
        var ancestor: NSView? = node
        while let view = ancestor {
            if let node = view as? NodeView, node.style["display"] as? String == "none" { return false }
            if view.isHidden && !(projected && view === owner) { return false }
            ancestor = view.superview
        }
        return node.window != nil
    }

    func contains(_ node: NodeView) -> Bool {
        toolbar != nil && (items[node.id] != nil || node === owner || node === heading)
    }

    func suppresses(_ node: NodeView) -> Bool {
        guard projected, let owner else { return false }
        return node === owner || node.isDescendant(of: owner)
    }

    func available(_ node: NodeView) -> Bool {
        guard let window, let toolbar, window.toolbar === toolbar,
              node.window === window, visible(node), !node.inert, !node.disabled,
              window.attachedSheet == nil, node.handlers.contains("press") else { return false }
        return items[node.id] != nil
    }

    func sync() {
        guard !syncing else { return }
        syncing = true
        defer { syncing = false }
        prepare()
        guard let window, presenter.root.window === window else { reset(); return }
        let owners = presenter.views.values.filter {
            $0.props["toolbarPlacement"] == "window" && $0.props["accessibilityRole"] == "toolbar"
                && visible($0) && $0.window === window
        }
        if owners.count > 1 { refuse("multiple visible declarations; keeping authored rendering"); return }
        guard let next = owners.first else { reset(); return }
        // A host that takes its toolbar back wins. Never overwrite it on a tick.
        guard window.toolbar == nil || window.toolbar === toolbar else { refuse("window owner installed another toolbar"); return }
        let declarations = children(next).filter { visible($0) }
        let buttons = declarations.filter { $0.kind == "button" && $0.handlers.contains("press") }
        let headings = declarations.filter { $0.kind == "text" && $0.props["accessibilityRole"] == "heading" }
        // The bounded initial shape: direct action buttons and one heading.
        // Unsupported content keeps the authored presentation, never vanishes.
        guard !buttons.isEmpty, headings.count <= 1,
              declarations.count == buttons.count + headings.count else {
            refuse("expected direct action buttons and at most one heading; keeping authored rendering"); return
        }
        refusal = nil
        owner = next; heading = headings.first
        let installing = toolbar == nil
        if installing {
            savedTitle = window.title
            let value = NSToolbar(identifier: "Exact.window.commands")
            value.delegate = self
            value.displayMode = .iconOnly
            value.allowsUserCustomization = false
            toolbar = value
        }
        guard let toolbar else { return }
        let live = Set(buttons.map(\.id))
        for id in Array(items.keys) where !live.contains(id) { items.removeValue(forKey: id); symbols.removeValue(forKey: id) }
        for node in buttons {
            let item = items[node.id] ?? NSToolbarItem(itemIdentifier: .init("exact.command.\(node.id)"))
            items[node.id] = item
            item.tag = Int(node.id)
            item.target = self; item.action = #selector(performItem(_:))
            item.autovalidates = true
            let label = node.props["accessibilityLabel"] ?? text(node)
            if item.label != label { item.label = label; item.paletteLabel = label; item.toolTip = label }
            let image = children(node).first { $0.kind == "image" }
            // Standard toolbar items own their image sizing, tint and glass.
            let symbol = image?.props["symbolName"] ?? ""
            if symbol.isEmpty {
                if item.image !== image?.image { item.image = image?.image }
                symbols.removeValue(forKey: node.id)
            } else if symbols[node.id] != symbol {
                item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
                symbols[node.id] = symbol
            }
            item.isBordered = true
            // Text-bearing authored buttons remain labeled in icon-only mode.
            let title = text(node)
            if item.title != title { item.title = title }
            let enabled = !node.disabled && !node.inert && window.attachedSheet == nil
            if item.isEnabled != enabled { item.isEnabled = enabled }
        }
        let nextOrder = declarations.compactMap { node -> NSToolbarItem.Identifier? in
            if node === heading { return .flexibleSpace }
            return items[node.id]?.itemIdentifier
        }
        order = nextOrder
        // Keep both toolbar and command objects stable through route changes.
        for index in stride(from: toolbar.items.count - 1, through: 0, by: -1) {
            if !order.contains(toolbar.items[index].itemIdentifier) { toolbar.removeItem(at: index) }
        }
        for (index, identifier) in order.enumerated() {
            if index < toolbar.items.count && toolbar.items[index].itemIdentifier == identifier { continue }
            if let old = toolbar.items.firstIndex(where: { $0.itemIdentifier == identifier }) { toolbar.removeItem(at: old) }
            toolbar.insertItem(withItemIdentifier: identifier, at: index)
        }
        appliedTitle = heading.map(text) ?? savedTitle
        if window.title != appliedTitle { window.title = appliedTitle }
        // Hide only the authored rendering. Its logical nodes/actions survive.
        if window.toolbar !== toolbar { window.toolbar = toolbar }
        if installing { onChange?() }
        savedAccessibilityHidden = next.isAccessibilityHidden()
        next.isHidden = true; next.setAccessibilityHidden(true); projected = true
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { order }
    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { order }
    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier identifier: NSToolbarItem.Identifier, willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        items.values.first { $0.itemIdentifier == identifier }
    }
    func validateToolbarItem(_ item: NSToolbarItem) -> Bool {
        guard let node = presenter.views[UInt32(item.tag)], items[node.id] === item else { return false }
        return available(node)
    }
    @objc private func performItem(_ item: NSToolbarItem) {
        guard validateToolbarItem(item) else { return }
        presenter.press(UInt32(item.tag))
    }

    /// The driver uses the same target/action as AppKit, including overflow.
    /// This is host activation, not a claim to have injected a physical click.
    func activate(_ node: NodeView) -> Bool? {
        guard let item = items[node.id] else { return nil }
        guard available(node), let action = item.action else { return false }
        return NSApp.sendAction(action, to: item.target, from: item)
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        guard contains(node), let toolbar else { return nil }
        var result: [String: Any] = ["view": node === owner ? "NSToolbar" : (node === heading ? "NSWindow.title" : "NSToolbarItem"),
                                   "placement": "window", "title": window?.title ?? "",
                                   "geometry": "system-owned", "attached": window?.toolbar === toolbar]
        if let item = items[node.id] {
            result["identifier"] = item.itemIdentifier.rawValue
            result["enabled"] = available(node)
            result["overflow"] = !(toolbar.visibleItems ?? []).contains(where: { $0 === item })
        }
        return result
    }
}
#endif

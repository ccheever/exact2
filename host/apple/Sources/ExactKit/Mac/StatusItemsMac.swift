// @ref LLP 1133.025 D4 — a status item: a `-exact-menu="status"` button
// presented as one of the app's `NSStatusItem`s in the menu bar. It takes
// the button's place: the button is covered whole (CoversMac.swift) and
// hidden in the window, so it takes no room there.
//
// The items are the status session's: the oldest live session (the one the
// launch booted, then the oldest left). A status button in any other session
// places nothing, and its journal says so once.
//
// A click opens the item's menu when the button names a menu-shaped popover
// (LLP 1021 D1), filled from its rows as it opens (`MenuHost.fill`); with no
// menu it runs `press`. An accessory app's menu ends with a separator and
// "Quit <App>" unless the root's `-exact-menu-remove` names `status-quit`
// (D7): with no window and no Dock tile, the menu is the only way to quit.
#if os(macOS)
import AppKit

final class StatusItemHost: NSObject {
    private unowned let presenter: Presenter
    /// Every session's host, oldest first.
    nonisolated(unsafe) private static var hosts: [Weak] = []
    private struct Weak { weak var host: StatusItemHost? }
    /// The placed items, by the button's id.
    private var items: [UInt32: NSStatusItem] = [:]
    /// The image each item was given from: a sync runs after every batch.
    private var images: [UInt32: NSImage?] = [:]
    private var saidUnplaced = false
    /// The session has ended: the next oldest places its own.
    private(set) var ended = false
    /// Said when this session's placed items change from some to none, or
    /// back: the host decides again whether anything reaches a hidden
    /// window (D6).
    var onPlacedChange: ((Bool) -> Void)?

    init(_ presenter: Presenter) {
        self.presenter = presenter
        super.init()
        Self.hosts.append(Weak(host: self))
    }

    /// Whether this session places status items now.
    var isStatusSession: Bool {
        Self.hosts.removeAll { $0.host == nil }
        return Self.hosts.first { $0.host?.ended == false }?.host === self
    }

    /// Whether `node` is placed as a status item, so not in the window.
    func places(_ node: NodeView) -> Bool { items[node.id] != nil }
    /// Whether any status item of this session is placed.
    var placed: Bool { !items.isEmpty }
    /// Whether any session's status item is placed.
    static var anyPlaced: Bool { hosts.contains { $0.host?.placed == true } }

    /// The status buttons the page shows now: an authored `display: none`
    /// or a hidden ancestor removes the item; the host's own hiding of a
    /// placed button does not.
    private func buttons() -> [NodeView] {
        presenter.chrome.ids("menuPlacement").sorted().compactMap { presenter.views[$0] }.filter { node in
            guard node.isButton, node.props["menuPlacement"]?.split(whereSeparator: \.isWhitespace).contains("status") == true else { return false }
            var at: NSView? = node
            while let view = at {
                if let n = view as? NodeView, n.style["display"]?.string == "none" { return false }
                if view.isHidden, !(view === node && places(node)) { return false }
                at = view.superview
            }
            return node.window != nil
        }
    }

    /// After each batch: the items follow the buttons.
    func sync() {
        let wanted = isStatusSession ? buttons() : []
        if !isStatusSession, !saidUnplaced, !buttons().isEmpty {
            saidUnplaced = true
            presenter.session?.log("status item: not placed: another window's session holds the app's status items (LLP 1133.025 D4)")
        }
        let had = placed
        let live = Set(wanted.map(\.id))
        for (id, item) in items where !live.contains(id) {
            NSStatusBar.system.removeStatusItem(item)
            items[id] = nil
            images[id] = nil
            if let node = presenter.views[id] { node.isHidden = false }
        }
        for node in wanted { place(node) }
        presenter.covers.report("status", Dictionary(uniqueKeysWithValues: wanted.map { ($0.id, HostCover.whole) }))
        if had != placed { onPlacedChange?(placed) }
    }

    private func place(_ node: NodeView) {
        let item = items[node.id] ?? {
            let made = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
            // Where the person dragged it is kept, as a hand-built app's is.
            made.autosaveName = "exact-status-\(node.props["testId"] ?? node.props["id"] ?? String(items.count))"
            made.button?.target = self
            made.button?.action = #selector(clicked(_:))
            items[node.id] = made
            return made
        }()
        node.isHidden = true
        guard let button = item.button else { return }
        button.tag = Int(node.id)
        let label = presenter.menus.title(of: node)
        let image = presenter.menus.image(of: node)
        if images[node.id] != .some(image) {
            images[node.id] = .some(image)
            let template = image?.copy() as? NSImage
            // The menu bar tints a template image (D7, Apple's HIG).
            template?.isTemplate = true
            if let template, template.size.height > 18 {
                template.size = NSSize(width: template.size.width * 18 / template.size.height, height: 18)
            }
            button.image = template
        }
        button.title = button.image == nil ? label : ""
        button.toolTip = label
        button.setAccessibilityLabel(label)
        button.appearsDisabled = node.disabled
        // A menu AppKit opens on the click, filled from the rows then; a
        // disabled item has neither menu nor press.
        if !node.disabled, presenter.menus.toolbarPopover(of: node) != nil {
            if (item.menu as? InvokerMenu)?.source !== node { item.menu = presenter.menus.toolbarMenu(for: node) }
        } else {
            item.menu = nil
        }
    }

    @objc private func clicked(_ sender: NSStatusBarButton) {
        guard let node = presenter.views[UInt32(sender.tag)], places(node), !node.disabled else { return }
        presenter.press(node.id, fromNativeMenu: true, outsideModal: true)
    }

    /// The end of a status item's menu as it opens: an accessory app's quit.
    func endMenu(_ menu: NSMenu) {
        // The app's own policy: under the agent every app runs as an accessory.
        guard LaunchKind.accessory else { return }
        let removed = (presenter.root.subviews.first as? NodeView)?.props["menuRemove"]?.split(whereSeparator: \.isWhitespace).contains("status-quit") == true
        guard !removed else { return }
        let name = Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String ?? ProcessInfo.processInfo.processName
        if !menu.items.isEmpty { menu.addItem(.separator()) }
        menu.addItem(NSMenuItem(title: "Quit \(name)", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))
    }

    /// A restart: the new runtime's ids are its own, and its first batch
    /// places its items again. The session's end (`destroy`, which resets
    /// the presenter): the next oldest session places its own.
    func reset() {
        for item in items.values { NSStatusBar.system.removeStatusItem(item) }
        items.removeAll()
        images.removeAll()
        agentMenu = nil
        guard presenter.session?.state == .destroyed, !ended else { return }
        ended = true
        for case let host? in Self.hosts.map(\.host) where host !== self && !host.ended { host.sync(); break }
    }

    // MARK: The agent

    /// The agent's tap on a placed status button (LLP 1133.025 §11): the
    /// item's click as AppKit makes it. With a menu, the menu as it would
    /// open, filled from its rows, and kept for a following tap on one of
    /// them; with none, the press. `nil` when `node` is no placed item.
    func agentClick(_ node: NodeView) -> [String: Any]? {
        guard let item = items[node.id] else { return nil }
        if node.disabled { return ["error": "status item #\(node.id) is disabled"] }
        guard let menu = item.menu as? InvokerMenu else {
            presenter.press(node.id, fromNativeMenu: true, outsideModal: true)
            return ["tapped": Int(node.id), "pressed": Int(node.id), "delivery": "host-activation", "native": "NSStatusItem"]
        }
        presenter.menus.fill(menu)
        agentMenu = menu
        let rows = menu.items.map { $0.isSeparatorItem ? "—" : $0.title }
        return ["tapped": Int(node.id), "delivery": "host-activation", "native": "NSStatusItem", "menu": rows]
    }
    /// The menu the agent's last tap opened.
    private var agentMenu: NSMenu?

    /// The agent's tap on a row of the menu its last tap opened: that
    /// item's action, as choosing it does. `nil` when no open menu shows it.
    func agentChoose(_ node: NodeView) -> [String: Any]? {
        guard let menu = agentMenu, let index = menu.items.firstIndex(where: { ($0 as? AccessibleMenuItem)?.owner === node }) else { return nil }
        agentMenu = nil
        guard menu.items[index].isEnabled else { return ["error": "status menu item #\(node.id) is disabled"] }
        menu.performActionForItem(at: index)
        return ["tapped": Int(node.id), "delivery": "host-activation", "native": "NSMenuItem"]
    }

    /// What the agent's `state` says of the placed items: what AppKit shows.
    var observed: [[String: Any]] {
        items.sorted { $0.key < $1.key }.map { id, item in
            ["id": Int(id), "title": item.button?.title ?? "", "toolTip": item.button?.toolTip ?? "",
             "image": item.button?.image != nil, "menu": item.menu != nil, "disabled": item.button?.appearsDisabled ?? false]
        }
    }
}
#endif

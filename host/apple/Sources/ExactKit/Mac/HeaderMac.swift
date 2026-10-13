// @ref LLP 1115.002 D2 — a route's `header` as the window's title bar and
// toolbar. A hand-built Mac window has no second bar under its title bar:
// the screen's title is the window's, and its actions are toolbar items. So
// where a route is header-shaped by the shared rule (`HeaderRule`, the
// kernel's, which UIKit's navigation bar is made from too) and the window's
// owner attached the session's toolbar, the header's heading is the window's
// title and its buttons are `NSToolbarItem`s that press the authored
// buttons, in the toolbar `WindowToolbarHost` already keeps for Back
// (ToolbarMac.swift). The Mac's branch of the rule: there is no large title,
// so a level-1 heading is a title like any other.
//
// A button that opens a menu (`popovertarget` naming a popover of a button
// menu's shape) is an `NSMenuToolbarItem`: AppKit pops the menu up from its
// own item, with its indicator, and the rows are read as it opens.
//
// A `tablist` of text tabs is an `NSToolbarItemGroup` in the bar's centre,
// as SwiftUI's principal `Picker` is: one segment a tab, each pressing its
// authored tab, the selected one the tab the app says is selected.
//
// An `input type="search"` is an `NSSearchToolbarItem` at the bar's end,
// and the field in it is the input's own (`NodeView.lendFieldToBar`): the
// node is its delegate there as in the page, so its value, its caret, its
// `input`, `change`, `submit`, `focus` and `blur`, its test id and what a
// driver types are the authored input's, and its look and place AppKit's.
//
// **Whole, or not at all.** The toolbar shows a heading, buttons whose face
// is a symbol and a title, and nothing else. A header holding anything the
// bar has no place for (a picture, a second line of text, a pressable
// title, a field) stays painted in the page as written, and the session's
// log says what it was: a header is never half projected. While the bar
// shows it the header is hidden and its box is the platform's (`HostCovers`,
// D8.1), reported in the step the projection happened, so the page lays out
// without it.
//
// **Which route.** The route the root shows; or, while that route is a
// modal one (a sheet's content), the route it stands over, as a window
// keeps its own title and items under a sheet. A modal route's own header
// is its sheet's (D7.4, RouteSheetMac.swift).
//
// **Who wins.** A toolbar the author declared for the window
// (`toolbarPlacement="window"`) is the bar, and then no header is projected.
// Under the agent the authored header paints, as on iOS, and
// `--chrome platform` shows the bar.
#if os(macOS)
import AppKit

/// What the window's bar shows of a route's header.
struct WindowHeader {
    let route: NodeView, header: NodeView, heading: NodeView
    /// The heading's text: the window's title, under an authored document
    /// title (the head's) and over the app's name.
    let title: String
    /// The header's buttons before its heading, which lead the bar after
    /// Back as navigational items, and those after it, its trailing items.
    let leading: [NodeView], trailing: [NodeView]
    /// The stack's Back control in the header, where the bar's own Back
    /// stands for it: no item of its own.
    let backs: [NodeView]
    var buttons: [NodeView] { leading + trailing }
    /// The header's tablist of text tabs and those tabs: the bar's centred
    /// segmented group, each segment pressing its tab.
    var segments: NodeView?
    var tabs: [NodeView] = []
    /// The header's search input: the bar's search item, whose field is
    /// the input's own.
    var search: NodeView?

    /// Whether `node` is one of the header's nodes the bar shows or stands for.
    func places(_ node: NodeView) -> Bool {
        node === header || node === heading || node === segments || node === search
            || buttons.contains { $0 === node } || backs.contains { $0 === node } || tabs.contains { $0 === node }
    }

    enum Read {
        /// No header the bar could be made from: nothing to say.
        case none
        /// A header the bar cannot show whole, and why: it stays painted.
        case kept(String)
        case shown(WindowHeader)
    }

    /// `route`'s header as the window's bar shows it. `back` names the
    /// stack's Back control; `pushed`: the bar's own Back stands for it on
    /// this route. `shown`: whether a node is shown as authored.
    static func read(route: NodeView, back: String?, pushed: Bool, shown: (NodeView) -> Bool) -> Read {
        guard let rule = HeaderRule(route: route, back: back, backIsPlatforms: pushed, projected: true) else { return .none }
        let children = { (node: NodeView) in node.container.subviews.compactMap { $0 as? NodeView } }
        let items = Set(rule.items.map(ObjectIdentifier.init))
        let backs = rule.items.filter { item in !rule.leading.contains { $0 === item } && !rule.trailing.contains { $0 === item } }
        // Everything the header shows has a place in the bar, or none of it goes.
        func refusal(in node: NodeView) -> String? {
            for child in children(node) where child.style["display"]?.string != "none" {
                if child === rule.heading { continue }
                if child === rule.tap { return "its heading is inside a pressable (#\(child.id)), and a window's title is not pressed" }
                if child === rule.search {
                    // A search input the author draws is theirs to draw: the bar's field is AppKit's.
                    if !child.lendsFieldToBar { return "its search field (#\(child.id)) is drawn by the app (`appearance: none`), and a toolbar's is AppKit's own" }
                    continue
                }
                if child === rule.segments {
                    if titleTabs(of: child) == nil { return "its tablist (#\(child.id)) is not two or more tabs that each show a title alone, which is all a toolbar's segments show" }
                    continue
                }
                if items.contains(ObjectIdentifier(child)) {
                    // The stack's Back control: the bar's own Back is its face.
                    if backs.contains(where: { $0 === child }) { continue }
                    // One its author keeps (LLP 1075.003 §9.10): the window's
                    // bar shows a header whole or not at all, so it all paints.
                    if child.wrote("appearance"), child.style["appearance"]?.string == "none" {
                        return "button #\(child.id) writes `appearance: none`, and the window's bar shows a header whole or not at all"
                    }
                    // A button that opens a menu is the bar's menu item, and one
                    // that asks first opens the window's alert. Any other
                    // popover is placed against its button's box in the page,
                    // which a bar item does not have.
                    if child.props["popovertarget"] != nil, child.props["popovertargetaction"] != "hide", !opensFromTheBar(child) {
                        return "button #\(child.id) opens a popover that is neither a menu nor an alert, which is placed against the button's own box"
                    }
                    if !showable(child) { return "button #\(child.id) shows more than one symbol and its title, which is all a toolbar item shows" }
                    continue
                }
                // A box that only groups is no part of the bar; what it holds decides.
                guard child.kind == "view", child.props["popover"] == nil, child.props["semanticTag"] != "dialog" else {
                    return "it holds #\(child.id) (\(child.isParagraph ? "text that is not its heading" : child.props["popover"] != nil ? "a popover" : child.kind)), which a toolbar has no place for"
                }
                if let inner = refusal(in: child) { return inner }
            }
            return nil
        }
        if let why = refusal(in: rule.header) { return .kept(why) }
        var read = WindowHeader(route: route, header: rule.header, heading: rule.heading, title: rule.heading.accessibleText,
                                leading: rule.leading.filter(shown), trailing: rule.trailing.filter(shown), backs: backs)
        if let list = rule.segments, shown(list), let tabs = titleTabs(of: list) { (read.segments, read.tabs) = (list, tabs) }
        if let input = rule.search, shown(input) { read.search = input }
        return .shown(read)
    }

    /// A header's tablist as the bar's segments: its tabs, when it holds
    /// two or more and nothing else, each a pressable tab that shows its
    /// title alone. AppKit's toolbar group is made of titles.
    static func titleTabs(of list: NodeView) -> [NodeView]? {
        let children = { (node: NodeView) in node.container.subviews.compactMap { $0 as? NodeView } }
        func textOnly(_ node: NodeView) -> Bool { node.kind == "text" && children(node).allSatisfy(textOnly) }
        let tabs = children(list).filter { $0.style["display"]?.string != "none" }
        let titled = { (tab: NodeView) -> Bool in
            guard tab.isButton, tab.props["accessibilityRole"] == "tab", tab.handlers.contains("press"), !tab.accessibleText.isEmpty else { return false }
            return tab.isNativeButton ? tab.face?.symbol == nil : children(tab).allSatisfy(textOnly)
        }
        return tabs.count >= 2 && tabs.allSatisfy(titled) ? tabs : nil
    }

    /// Whether what a button's `popovertarget` opens needs no place in the
    /// page: a button menu, which is the bar's menu item
    /// (`MenuHost.toolbarPopover`), or a confirmation in the alert's shape,
    /// which is a sheet on the window (AlertsMac.swift).
    static func opensFromTheBar(_ button: NodeView) -> Bool {
        guard let presenter = button.presenter else { return false }
        if presenter.menus.toolbarPopover(of: button) != nil { return true }
        guard let name = button.props["popovertarget"],
              let pop = presenter.carrying("popover").first(where: { $0.props["id"] == name }) else { return false }
        return presenter.alerts.plan(of: pop) != nil
    }

    /// Whether a toolbar item can show all of a button's face: a native
    /// button's symbol and title, or a custom button holding text and at most
    /// one image. A box drawn inside it (an avatar, a badge) is not an
    /// item's to draw.
    static func showable(_ button: NodeView) -> Bool {
        if button.isNativeButton { return button.face?.raster != true }
        let children = { (node: NodeView) in node.container.subviews.compactMap { $0 as? NodeView } }
        let content = children(button)
        func textOnly(_ node: NodeView) -> Bool { node.kind == "text" && children(node).allSatisfy(textOnly) }
        let images = content.filter { $0.kind == "image" && children($0).isEmpty }
        return images.count <= 1 && content.allSatisfy { node in textOnly(node) || images.contains { $0 === node } }
    }
}

/// The bar's segmented group for a header's tablist: AppKit's own
/// `NSToolbarItemGroup` of titles, which is a segmented control while there
/// is room and AppKit's pop-up of the same choices when there is not. A
/// segment chosen presses its authored tab, and the group then shows the
/// tab the app says is selected, which is the app's to decide.
final class HeaderSegments: NSObject {
    private unowned let presenter: Presenter
    private(set) var item: NSToolbarItemGroup?
    private(set) var tablist: UInt32?
    private(set) var tabs: [UInt32] = []
    private var titles: [String] = []
    private var generation = 0
    /// After a segment's press: the bar reads the app's answer.
    var onChosen: (() -> Void)?
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// The group for `header`'s tablist as it is now, or nil with none. A
    /// group's segments are made with it, so new tabs or titles make a new
    /// group, under a new identifier.
    func update(_ header: WindowHeader?, enabled: Bool) -> NSToolbarItemGroup? {
        guard let header, let list = header.segments else {
            (item, tablist, tabs, titles) = (nil, nil, [], [])
            return nil
        }
        let ids = header.tabs.map(\.id), names = header.tabs.map(\.accessibleText)
        if item == nil || tablist != list.id || ids != tabs || names != titles {
            generation += 1
            let group = NSToolbarItemGroup(itemIdentifier: .init("exact.segments.\(list.id).\(generation)"), titles: names,
                                           selectionMode: .selectOne, labels: nil, target: self, action: #selector(chose(_:)))
            group.autovalidates = false
            (item, tablist, tabs, titles) = (group, list.id, ids, names)
        }
        guard let item else { return nil }
        // The tablist's name: what the overflow menu and VoiceOver call the group.
        let label = list.authoredLabel ?? ""
        if item.label != label { item.label = label; item.paletteLabel = label }
        let chosen = header.tabs.firstIndex { $0.props["accessibilitySelected"] == "true" } ?? -1
        if item.selectedIndex != chosen { item.selectedIndex = chosen }
        for (index, tab) in header.tabs.enumerated() where index < item.subitems.count {
            let on = enabled && !tab.disabled && !tab.inert
            if item.subitems[index].isEnabled != on { item.subitems[index].isEnabled = on }
        }
        if item.isEnabled != enabled { item.isEnabled = enabled }
        return item
    }

    /// The tab a segment stands for, where a person could choose it now.
    func tab(at index: Int) -> NodeView? {
        guard item?.isEnabled == true, tabs.indices.contains(index), let tab = presenter.views[tabs[index]], !tab.disabled, !tab.inert else { return nil }
        return tab
    }

    @objc private func chose(_ group: NSToolbarItemGroup) {
        if group === item, let tab = tab(at: group.selectedIndex) { presenter.press(tab.id) }
        // Chosen or refused, the segment shown is the app's selected tab.
        onChosen?()
    }

    /// `state.window.toolbar.header.segments` (the agent).
    func summary(overflow: Bool) -> [String: Any]? {
        guard let item, let tablist else { return nil }
        return ["tablist": Int(tablist), "label": item.label, "selected": item.selectedIndex, "overflow": overflow,
                "tabs": tabs.enumerated().map { index, id -> [String: Any] in
                    ["id": Int(id), "testId": presenter.views[id]?.props["testId"] ?? NSNull(), "title": titles[index],
                     "enabled": index < item.subitems.count && item.subitems[index].isEnabled]
                }]
    }
}

/// The bar's search item for a header's search input: AppKit's
/// `NSSearchToolbarItem`, which is a field while there is room and a button
/// that opens one when there is not, holding the input's own field
/// (`NodeView.lendFieldToBar`). Nothing here hears the field: the input's
/// node does, as it hears its field in the page.
final class HeaderSearch {
    private(set) var item: NSSearchToolbarItem?
    private(set) weak var node: NodeView?
    /// AppKit's words in a search item's empty field, for an input that says none.
    private var stock: String?

    /// The item for `header`'s search input now, or nil with none; the
    /// input that had the bar's field before has its own again.
    func update(_ header: WindowHeader?, enabled: Bool) -> NSSearchToolbarItem? {
        guard let input = header?.search, let field = input.lendFieldToBar() else {
            release()
            return nil
        }
        if node !== input || item == nil {
            release(keeping: input)
            let made = NSSearchToolbarItem(itemIdentifier: .init("exact.search.\(input.id)"))
            stock = made.searchField.placeholderString
            made.autovalidates = false
            (item, node) = (made, input)
        }
        guard let item else { return nil }
        if item.searchField !== field { item.searchField = field }
        if (input.props["placeholder"] ?? "").isEmpty, field.placeholderString != stock { field.placeholderString = stock }
        input.syncBarField()
        // The input's name is the item's, in the overflow menu and to VoiceOver; unnamed, AppKit's "Search".
        if let label = input.authoredLabel, item.label != label { item.label = label; item.paletteLabel = label }
        let on = enabled && !input.disabled && !input.inert
        if item.isEnabled != on { item.isEnabled = on }
        return item
    }

    /// The bar lets the field go: its input edits in its page again.
    func release(keeping kept: NodeView? = nil) {
        if let node, node !== kept { node.reclaimFieldFromBar() }
        (item, node) = (nil, nil)
    }

    /// Put the caret in the bar's field, AppKit's own way in: where the
    /// item has become its button, that opens the field first.
    func focus() -> Bool {
        guard let item, item.isEnabled, item.toolbar != nil else { return false }
        item.beginSearchInteraction()
        return true
    }
}

extension NavigationHost {
    /// The route whose header the window's bar shows, with its navigation
    /// root: the route the root shows, or under a modal route the route it
    /// stands over (`NavigationRules.segments`); and whether that route is
    /// pushed, so the bar's Back is its way back. Read from what the last
    /// batch showed (`selection`).
    var headerRoute: (root: NodeView, route: NodeView, pushed: Bool)? {
        guard let selection, let root = presenter.views[selection.root], selection.stack.indices.contains(selection.at) else { return nil }
        let stack = selection.stack[...selection.at].compactMap { presenter.views[$0] }
        guard stack.count == selection.at + 1,
              let base = NavigationRules.segments(presentations: stack.map { $0.props["navigationPresentation"] }).first,
              let route = stack[base].last else { return nil }
        return (root, route, base.count > 1)
    }
}
#endif

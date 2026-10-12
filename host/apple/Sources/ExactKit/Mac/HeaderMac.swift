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
// modal one (a sheet's content, stage 6c), the route it stands over, as a
// window keeps its own title and items under a sheet. A modal route's own
// header stays painted in it until it is a sheet.
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

    /// Whether `node` is one of the header's nodes the bar shows or stands for.
    func places(_ node: NodeView) -> Bool {
        node === header || node === heading || buttons.contains { $0 === node } || backs.contains { $0 === node }
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
                if child === rule.search { return "it holds a search field (#\(child.id))" }
                if child === rule.segments { return "it holds a tablist (#\(child.id))" }
                if items.contains(ObjectIdentifier(child)) {
                    // The stack's Back control: the bar's own Back is its face.
                    if backs.contains(where: { $0 === child }) { continue }
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
        return .shown(WindowHeader(route: route, header: rule.header, heading: rule.heading, title: rule.heading.accessibleText,
                                   leading: rule.leading.filter(shown), trailing: rule.trailing.filter(shown), backs: backs))
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

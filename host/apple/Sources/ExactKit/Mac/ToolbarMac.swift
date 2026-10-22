// @ref LLP 1031 D5/D12; LLP 1001 §10 — window-owned presentation of
// Contract commands, not an embedded NativeView or a second action registry.
// And the platform's Back (LLP 1115.000 D5.1): the window of an app with a
// navigation stack leads its toolbar with a navigational back chevron, in
// the toolbar the author declared or, with none, in one that holds only it.
// It is there at a stack's root too, disabled, as Finder's and Safari's is:
// a toolbar that came with each push and went with each pop made the title
// bar 8 points taller and moved the content every time. A route that asks
// for no bar (`navigationBar="none"`, D5.2) gets no chevron and the toolbar
// stays, empty, at the same height. An app with no stack, or one whose
// every mounted route asks for no bar, gets no toolbar; one comes with the
// first route that shows a bar and then stays (`heldForBack`), so the title
// bar's height changes at most that once. With no toolbar declared, the
// route's `header` is this bar too (LLP 1115.002 D2, HeaderMac.swift): its
// heading the window's title, its buttons items after Back.
#if os(macOS)
import AppKit

final class WindowToolbarHost: NSObject, NSToolbarDelegate, NSToolbarItemValidation {
    unowned let presenter: Presenter
    private(set) weak var window: NSWindow?
    private(set) var toolbar: NSToolbar?
    private(set) var items: [UInt32: NSToolbarItem] = [:]
    private var symbols: [UInt32: String] = [:]
    /// The authored rendering the toolbar stands for: the declared toolbar,
    /// or the route's header (`header`).
    private weak var owner: NodeView?
    /// LLP 1080.001 D3: the authored rendering hidden while the window's toolbar shows it.
    func hides(_ node: NodeView) -> Bool { (projected && node === owner) || besideParts.contains { $0.node === node } }
    private weak var heading: NodeView?
    /// The route's header this bar shows, with no toolbar declared (LLP
    /// 1115.002 D2), and what was last said of it in the session's log.
    private(set) var header: WindowHeader?
    private var headerDecision: String?
    /// In a window that shows a list beside its detail (LLP 1115.002 D4,
    /// §5 Q10), the header of the route beside the one that owns the
    /// window: its heading stays the first line of its pane, and its
    /// buttons are the bar's items over its own column. What the bar took
    /// of it is hidden in the page, each with what VoiceOver was told of it.
    private(set) var beside: WindowHeader?
    private var besideParts: [(node: NodeView, accessibilityHidden: Bool)] = []
    private var besideDecision: String?
    /// The headers the bar shows: the window's, and the one beside it.
    private var headers: [WindowHeader] { [header, beside].compactMap { $0 } }
    /// The divider between the list's column and the detail's pane: the
    /// separator the bar's items stop at and begin after.
    static let listSeparatorIdentifier = NSToolbarItem.Identifier("exact.split.separator")
    /// Why the selected route's header stays painted, when the bar cannot
    /// show it whole (`state.window.toolbar.headerKept`); nil otherwise.
    private(set) var headerKept: String?
    /// The header's tablist as the bar's centred group (HeaderMac.swift).
    private(set) lazy var segments: HeaderSegments = {
        let segments = HeaderSegments(presenter)
        segments.onChosen = { [weak self] in self?.sync() }
        return segments
    }()
    /// Whether the bar shows this tablist's tabs as its segments: the page's
    /// own segmented control is not made of it (SegmentsMac.swift).
    func adopts(_ tablist: NodeView) -> Bool { toolbar != nil && segments.tablist == tablist.id && headers.contains { $0.segments === tablist } }
    /// The header's search input as the bar's search item, holding the
    /// input's own field (HeaderMac.swift).
    let search = HeaderSearch()
    /// The caret to the bar's field for `node`, the search input it hosts
    /// (an action's `focus()`, an `autofocus`, the driver's `tap`).
    func focusSearch(_ node: NodeView) -> Bool {
        search.node === node && headers.contains { $0.search === node } && usable(node) && search.focus()
    }
    private var order: [NSToolbarItem.Identifier] = []
    /// Back (BackMac.swift): the way back from the pushed route, pressing
    /// what iOS's Back button presses. A standard item, so its size, glass
    /// and place at the leading edge are AppKit's.
    static let backIdentifier = NSToolbarItem.Identifier("exact.navigation.back")
    /// Each toolbar's own: a toolbar that goes and comes back (the view
    /// moving between the window and a sidebar's pane) gets a new one, as
    /// it gets new command items. AppKit lays an item out in one toolbar
    /// only, and one carried over from the toolbar that went threw in
    /// `-[NSToolbarItemViewer minSize]`.
    private(set) lazy var backItem = makeBackItem()
    private func makeBackItem() -> NSToolbarItem {
        let item = NSToolbarItem(itemIdentifier: Self.backIdentifier)
        item.label = "Back"; item.paletteLabel = "Back"; item.toolTip = "Back"
        item.image = NSImage(systemSymbolName: "chevron.backward", accessibilityDescription: "Back")
        item.isBordered = true
        item.isNavigational = true
        item.target = self; item.action = #selector(performBack(_:))
        item.autovalidates = true
        return item
    }
    /// The toolbar is installed for the stack's Back: kept through routes
    /// that ask for no bar, until the stack or the toolbar goes.
    private var heldForBack = false
    private(set) var projected = false
    private var savedAccessibilityHidden = false
    private var syncing = false
    private var savedTitle = ""
    private var appliedTitle = ""
    /// The active head's title (LLP 1048.003 D1) and the window's own title
    /// before one arrived: the head's is the window's title, under a
    /// projected toolbar heading, and its absence gives the app's back.
    private var headTitle: String?
    private var ownTitle: String?
    /// `head edited` (LLP 1069.010 D6): the dot in the window's close button.
    private var headEdited = false
    private var refusal: String?
    var onChange: (() -> Void)?
    /// LLP 1075.003.000 §3.7: the app module's `toolbar` hatch, once a
    /// toolbar is installed; the items it adds after Exact's, which Exact
    /// never removes; its delegate, which hears what Exact does not answer.
    private var hatched = false
    private(set) var appItems: [NSToolbarItem] = []
    private weak var appDelegate: NSToolbarDelegate?

    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Only the containing app calls this. Merely mounting an ExactView never
    /// claims window chrome. A pre-existing toolbar is never replaced.
    @discardableResult func attach(to window: NSWindow) -> Bool {
        if self.window === window { sync(); return true }
        guard window.toolbar == nil else { return false }
        detach()
        self.window = window
        sync()
        if headTitle != nil { applyTitle() }
        if headEdited { window.isDocumentEdited = true }
        return true
    }

    func headTitle(_ title: String?) {
        guard title != headTitle else { return }
        headTitle = title
        applyTitle()
    }

    func headEdited(_ edited: Bool) {
        headEdited = edited
        if let window, window.isDocumentEdited != edited { window.isDocumentEdited = edited }
    }

    private func applyTitle() {
        guard let window else { return }
        if ownTitle == nil { ownTitle = toolbar == nil ? window.title : savedTitle }
        let base = headTitle ?? ownTitle ?? ""
        if toolbar == nil {
            if window.title != base { window.title = base }
            return
        }
        savedTitle = base
        appliedTitle = title(over: base)
        if window.title != appliedTitle { window.title = appliedTitle }
    }

    /// The window's title while the toolbar is its own: a declared
    /// toolbar's heading; else the authored document title (the head's),
    /// else the heading of the route whose header the bar shows, else the
    /// window's own, the app's name (LLP 1115.002 D2, §5 Q2). `base` is the
    /// head's title or the window's own.
    private func title(over base: String) -> String {
        if let heading { return text(heading) }
        if headTitle == nil, let title = header?.title, !title.isEmpty { return title }
        return base
    }

    func detach() {
        let adopted = header?.segments
        syncing = true
        reset()
        window = nil
        syncing = false
        settle(adopted)
    }

    /// After a pass, outside it: the passes that read what the bar stands
    /// for, and the header's cover. The page's own segmented control is not
    /// made of a tablist the bar shows, and is again once the bar lets go.
    private func settle(_ adopted: NodeView?) {
        if adopted !== header?.segments { presenter.segments.sync() }
        reportCover()
    }

    /// The header's box is the platform's while the bar shows it (LLP
    /// 1115.002 D8.1): said in the step in which it was projected, and taken
    /// back in the step in which it stopped. Never from inside a pass:
    /// telling the kernel can apply a batch, whose own pass must run whole.
    private func reportCover() {
        var covers: [UInt32: HostCover] = projected ? owner.map { [$0.id: .whole] } ?? [:] : [:]
        for part in besideParts { covers[part.node.id] = .whole }
        presenter.covers.report("header", covers)
    }

    private func refuse(_ reason: String) {
        if refusal != reason { presenter.session?.log("window toolbar: \(reason)"); refusal = reason }
        reset()
    }

    /// Whether a batch can change what the bar shows: one that makes,
    /// removes or moves a node, writes props, style or words, or carries a
    /// command (a dialog opening). A batch of none of these (a scroll's
    /// frames, an animation's values) leaves the bar as it stands and pays
    /// nothing for it: its pass reads the route's header, each button's
    /// face and AppKit's items.
    func concerns(_ batch: Batch) -> Bool { batch.ops.contains { Self.read.contains($0.op) } }
    private static let read: Set<BatchOp.Kind> = [.create, .destroy, .children, .roots, .props, .style, .paragraph, .command]

    /// Restore the authored visibility before a batch can change it. Hidden
    /// here is a presentation detail, never the command's availability state.
    func prepare() {
        if projected {
            owner?.isHidden = false
            owner?.setAccessibilityHidden(savedAccessibilityHidden)
            projected = false
        }
        for part in besideParts where presenter.views[part.node.id] === part.node {
            part.node.isHidden = false
            part.node.setAccessibilityHidden(part.accessibilityHidden)
        }
        besideParts = []
    }

    func reset() {
        // Taking the toolbar off resizes the window's content, which can lay
        // the page out and apply a batch: its pass must not install the
        // toolbar again while this one is letting it go.
        let was = syncing
        syncing = true
        defer { syncing = was }
        prepare()
        let hadToolbar = toolbar != nil
        let old = toolbar
        toolbar = nil
        let content = contentSize
        if let window, let old, window.toolbar === old {
            window.toolbar = nil
            if window.title == appliedTitle { window.title = savedTitle }
        }
        items.removeAll(); symbols.removeAll(); order.removeAll(); owner = nil; heading = nil
        header = nil; decide(nil); beside = nil; decideBeside(nil)
        _ = segments.update(nil, enabled: false)
        search.release()
        if hadToolbar { backItem = makeBackItem() }
        heldForBack = false
        // The app's items and delegate were for that toolbar; the next one's
        // hatch says its own.
        hatched = false; appItems.removeAll(); appDelegate = nil
        if hadToolbar { onChange?(); keepContent(content) }
    }

    /// The content keeps its size as the toolbar comes or goes: only the
    /// title bar changes, the window's top edge where it was. AppKit sizes
    /// the window for a toolbar by the toolbar's whole height, the title bar
    /// it merges with included, so the content came out 32 points taller
    /// with one and as much shorter without (measured on macOS 27.0, unified
    /// and compact alike, on screen or not). A full-screen window's frame is
    /// the screen's.
    private var contentSize: NSSize? {
        guard let window, !window.styleMask.contains(.fullScreen) else { return nil }
        return window.contentView?.frame.size
    }
    private func keepContent(_ size: NSSize?) {
        guard let size, let window, let now = window.contentView?.frame.size, now != size else { return }
        window.setContentSize(size)
    }

    /// The `toolbar` hatch for the installed toolbar, once (at a cold launch,
    /// when the module connects).
    func hatchToolbar() {
        guard !hatched, let toolbar, let window, let natives = presenter.session?.natives, natives.hatchesConnected else { return }
        hatched = true
        natives.toolbarHatch(toolbar, window: window)
    }

    /// An item the app adds after Exact's (`ExactToolbar.add`).
    func addAppItem(_ item: NSToolbarItem) {
        guard toolbar != nil, !appItems.contains(where: { $0 === item || $0.itemIdentifier == item.itemIdentifier }) else { return }
        appItems.append(item)
        presenter.session?.log("hatch toolbar: the app added \(item.itemIdentifier.rawValue)")
        // From the hatch, inside a sync: the next turn inserts it.
        if syncing { DispatchQueue.main.async { [weak self] in self?.sync() } } else { sync() }
    }

    /// The app's delegate: Exact keeps the toolbar's slot (it supplies its
    /// command items) and forwards what it does not answer. AppKit reads what
    /// a delegate answers when it is set, so it is set again.
    func setAppDelegate(_ delegate: NSToolbarDelegate?) {
        appDelegate = delegate
        toolbar?.delegate = nil
        toolbar?.delegate = self
        presenter.session?.log("hatch toolbar: delegate \(delegate.map { "\(type(of: $0))" } ?? "cleared")")
    }

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (appDelegate?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        appDelegate?.responds(to: selector) == true ? appDelegate : nil
    }

    /// `state.window.toolbar` (the agent).
    var summary: [String: Any] {
        guard let toolbar else { return ["installed": false, "headerKept": headerKept ?? NSNull()] }
        return ["installed": window?.toolbar === toolbar, "displayMode": toolbar.displayMode.rawValue,
                "style": window?.toolbarStyle.rawValue ?? NSNull(), "header": headerSummary ?? NSNull(), "headerKept": headerKept ?? NSNull(),
                "items": toolbar.items.map(\.itemIdentifier.rawValue), "appItems": appItems.map(\.itemIdentifier.rawValue),
                // AppKit's own validation of the app's items (their targets
                // validate them, never Exact's delegate).
                "appEnabled": appItems.map { $0.validate(); return $0.isEnabled }]
    }

    /// `state.window.toolbar.header`: the route's header the bar shows (LLP
    /// 1115.002 D2), its items by their authored buttons, and which of them
    /// AppKit has moved to the overflow menu.
    private var headerSummary: [String: Any]? {
        guard let header, projected, let toolbar else { return nil }
        let shown = Set((toolbar.visibleItems ?? []).map(\.itemIdentifier))
        // The header beside the window's (a split's other column): its heading stays in its pane, and these are the bar's.
        let other: Any = beside.map { beside -> [String: Any] in
            ["route": Int(beside.route.id), "node": Int(beside.header.id), "heading": beside.title, "back": beside.backs.map { Int($0.id) },
             "separator": toolbar.items.contains { $0.itemIdentifier == Self.listSeparatorIdentifier },
             "items": beside.buttons.map { node -> [String: Any] in
                 let item = items[node.id]
                 return ["id": Int(node.id), "testId": node.props["testId"] ?? NSNull(), "label": item?.label ?? "", "menu": item is NSMenuToolbarItem,
                         "enabled": available(node), "overflow": item.map { !shown.contains($0.itemIdentifier) } ?? false]
             }]
        } ?? NSNull()
        return ["route": Int(header.route.id), "node": Int(header.header.id), "title": header.title, "back": header.backs.map { Int($0.id) }, "beside": other,
                "segments": segments.summary(overflow: segments.item.map { !shown.contains($0.itemIdentifier) } ?? false) ?? NSNull(),
                "search": header.search.flatMap { node -> [String: Any]? in
                    guard let item = search.item, search.node === node else { return nil }
                    return ["id": Int(node.id), "testId": node.props["testId"] ?? NSNull(), "label": item.label, "value": item.searchField.stringValue,
                            "placeholder": item.searchField.placeholderString ?? "", "enabled": usable(node) && item.isEnabled,
                            "editing": item.searchField.currentEditor() != nil, "overflow": !shown.contains(item.itemIdentifier)]
                } ?? NSNull(),
                "items": header.buttons.map { node -> [String: Any] in
                    let item = items[node.id]
                    var prominent = false
                    if #available(macOS 26.0, *) { prominent = item?.style == .prominent }
                    return ["id": Int(node.id), "testId": node.props["testId"] ?? NSNull(), "label": item?.label ?? "", "menu": item is NSMenuToolbarItem,
                            "place": header.leading.contains { $0 === node } ? "leading" : "trailing", "enabled": available(node),
                            "prominent": prominent, "overflow": item.map { !shown.contains($0.itemIdentifier) } ?? false]
                }]
    }

    /// Say in the session's log what the bar made of the route's header, once per change.
    private func decide(_ said: String?, kept: Bool = false) {
        headerKept = kept ? said : nil
        guard headerDecision != said else { return }
        headerDecision = said
        if let said { presenter.session?.log("window header: \(said)") }
    }

    private func children(_ node: NodeView) -> [NodeView] {
        node.container.subviews.compactMap { $0 as? NodeView }
    }

    /// A native button's prominent style is a prominent item, tinted by its
    /// `accent-color`, in the macOS 26 design; every other button, and every
    /// button in the earlier design, is the plain bordered item (LLP
    /// 1069.011.000 D2).
    private func prominence(_ item: NSToolbarItem, _ node: NodeView, _ face: ButtonFace?) {
        guard #available(macOS 26.0, *) else { return }
        let prominent = LinkedDesign.liquidGlass
            && ["filled", "bordered-prominent", "prominent-glass", "prominent-clear-glass"].contains(face?.style ?? "")
        let style: NSToolbarItem.Style = prominent ? .prominent : .plain
        if item.style != style { item.style = style }
        let tint = prominent ? node.channels("accent_color").map { TextEngine.color($0) } : nil
        if item.backgroundTintColor != tint { item.backgroundTintColor = tint }
    }

    private func text(_ node: NodeView) -> String {
        if node.kind == "text" { return node.paragraphSpec().runs.map(\.text).joined() }
        return children(node).map(text).filter { !$0.isEmpty }.joined(separator: " ")
    }

    /// Ignore just our own projection hiding; honor every authored/route hide.
    func visible(_ node: NodeView) -> Bool {
        var ancestor: NSView? = node
        while let view = ancestor {
            if let node = view as? NodeView, node.style["display"]?.string == "none" { return false }
            if view.isHidden && !(projected && view === owner) && !besideParts.contains(where: { $0.node === view }) { return false }
            ancestor = view.superview
        }
        return node.window != nil
    }

    func contains(_ node: NodeView) -> Bool {
        toolbar != nil && (items[node.id] != nil || node === owner || node === heading || (projected && header?.places(node) == true)
            || besideParts.contains { $0.node === node })
    }

    func suppresses(_ node: NodeView) -> Bool {
        if besideParts.contains(where: { node === $0.node || node.isDescendant(of: $0.node) }) { return true }
        guard projected, let owner else { return false }
        return node === owner || node.isDescendant(of: owner)
    }

    /// Whether what the bar shows of `node` can be used now: the toolbar is
    /// the window's, the node is shown as authored and neither inert nor
    /// disabled, and no sheet is over the window.
    private func usable(_ node: NodeView) -> Bool {
        guard let window, let toolbar, window.toolbar === toolbar, node.window === window else { return false }
        return visible(node) && !node.inert && !node.disabled && window.attachedSheet == nil
    }

    func available(_ node: NodeView) -> Bool {
        // A button that only opens its menu or its dialog hears no press.
        usable(node) && items[node.id] != nil && node.pressable
    }

    /// The buttons the bar shows as commands, each pressed by its item: a
    /// menu has one of these too (ShortcutsMac.swift). A button the bar
    /// shows as a menu item is not one: its menu is the item's to show.
    var commands: Set<UInt32> { Set(items.filter { !($0.value is NSMenuToolbarItem) }.keys) }

    /// Where a popover opened from one of the bar's buttons is placed. Such
    /// a button has no box in the page and AppKit gives an item no frame,
    /// so it is the top of the viewport, at the end the item is on. The
    /// driver's painted menu uses it (LLP 1021 D4); a person's menu is the
    /// item's own.
    func anchor(of node: NodeView, in view: NSView) -> NSRect? {
        // Asked inside a batch too, between `prepare` and the pass that hides the header again.
        guard toolbar != nil, let header = headers.first(where: { $0.buttons.contains { $0 === node } }) else { return nil }
        let bounds = view.convert(presenter.viewport.bounds, from: presenter.viewport)
        return NSRect(x: header.leading.contains { $0 === node } ? bounds.minX : bounds.maxX, y: bounds.minY, width: 0, height: 0)
    }

    private typealias Declaration = (owner: NodeView, shown: [NodeView], buttons: [NodeView], heading: NodeView?)
    /// The author's toolbar in this window, when the toolbar can show it: its
    /// node, its action buttons, its heading. A shape it cannot show is said
    /// once and keeps its authored rendering.
    private func declaration(in window: NSWindow) -> Declaration? {
        func refused(_ reason: String) -> Declaration? {
            if refusal != reason { presenter.session?.log("window toolbar: \(reason)"); refusal = reason }
            return nil
        }
        let owners = presenter.carrying("toolbarPlacement").filter {
            $0.props["toolbarPlacement"] == "window" && $0.props["accessibilityRole"] == "toolbar"
                && visible($0) && $0.window === window
        }
        if owners.count > 1 { return refused("multiple visible declarations; keeping authored rendering") }
        guard let next = owners.first else { return nil }
        let declarations = children(next).filter { visible($0) }
        let buttons = declarations.filter { $0.isButton && $0.handlers.contains("press") }
        let headings = declarations.filter { $0.kind == "text" && $0.props["accessibilityRole"] == "heading" }
        // The bounded initial shape: direct action buttons and one heading.
        // Unsupported content keeps the authored presentation, never vanishes.
        guard !buttons.isEmpty, headings.count <= 1, declarations.count == buttons.count + headings.count else {
            return refused("expected direct action buttons and at most one heading; keeping authored rendering")
        }
        refusal = nil
        return (next, declarations, buttons, headings.first)
    }

    /// Whether the author declared a toolbar for this window, shown now:
    /// one the toolbar can show or not, the bar is theirs, and no route's
    /// header is projected into it (LLP 1115.002 D2, "Precedence").
    private func declares(in window: NSWindow) -> Bool {
        presenter.carrying("toolbarPlacement").contains {
            $0.props["toolbarPlacement"] == "window" && $0.props["accessibilityRole"] == "toolbar" && visible($0) && $0.window === window
        }
    }

    /// The route's header as this window's bar shows it (LLP 1115.002 D2,
    /// HeaderMac.swift): none under the agent's authored chrome, with a
    /// toolbar the author declared, or for a header the bar cannot show
    /// whole, which stays painted and is said once.
    private func readHeader(in window: NSWindow) -> WindowHeader? {
        guard !ExactEnv.authoredChrome, !declares(in: window),
              let (root, route, pushed) = presenter.navigation.headerRoute, route.window === window, visible(route) else { decide(nil); return nil }
        let back = root.props["navigationBack"].flatMap { $0.isEmpty ? nil : $0 }
        switch WindowHeader.read(route: route, back: back, pushed: pushed, shown: visible) {
        case .none:
            decide(nil)
            return nil
        case .kept(let why):
            decide("route #\(route.id)'s header is kept as authored: \(why)", kept: true)
            return nil
        case .shown(let header):
            decide("route #\(route.id)'s header #\(header.header.id) is the window's title and toolbar (\(header.buttons.count) items)")
            return header
        }
    }

    /// The header beside the window's own, in a window that shows a list
    /// beside its detail: read as the window's is, and kept as authored,
    /// buttons and all, where the bar cannot show the whole of it.
    private func readBeside(in window: NSWindow) -> WindowHeader? {
        guard !ExactEnv.authoredChrome, !declares(in: window), let routes = presenter.navigation.splitHeaderRoutes, let route = routes.beside,
              route.window === window, visible(route) else {
            decideBeside(nil)
            return nil
        }
        let back = routes.root.props["navigationBack"].flatMap { $0.isEmpty ? nil : $0 }
        // The bar's own Back stands for its stack's Back control, enabled or not.
        switch WindowHeader.read(route: route, back: back, pushed: true, shown: visible) {
        case .none:
            decideBeside(nil)
            return nil
        case .kept(let why):
            decideBeside("route #\(route.id)'s header, beside the window's, is kept as authored: \(why)")
            return nil
        case .shown(let header):
            decideBeside("route #\(route.id)'s header #\(header.header.id) keeps its heading in its pane, and its \(header.buttons.count) buttons are the bar's, over its column")
            return header
        }
    }

    private func decideBeside(_ said: String?) {
        guard besideDecision != said else { return }
        besideDecision = said
        if let said { presenter.session?.log("window header: \(said)") }
    }

    func sync() {
        guard !syncing else { return }
        let adopted = header?.segments
        syncing = true
        present()
        syncing = false
        settle(adopted)
    }

    private func present() {
        prepare()
        guard let window, presenter.root.window === window else { reset(); return }
        let declared = declaration(in: window)
        let header = declared == nil ? readHeader(in: window) : nil
        let beside = declared == nil ? readBeside(in: window) : nil
        if declared != nil { decide(nil); decideBeside(nil) }
        // Back for an app with a stack, the author's toolbar or not: the
        // toolbar from the first route that shows a bar, the item on every
        // route that does not ask for none (`navigationBar="none"`). AppKit
        // asks whether it goes now (`validateToolbarItem`), as it does a
        // menu: at a stack's root it does not, and the item is disabled.
        let navigation = presenter.navigation
        let held = navigation.hasStack && (heldForBack || navigation.barRoute)
        let back = held && navigation.backInToolbar
        // A window whose sidebar is the app's root tab bar (LLP 1115.002 D3)
        // has a toolbar for it: the sidebar's button, then the separator
        // that follows its divider, ahead of Back and the rest, which so
        // begin at the page's edge. Both are AppKit's own items. The author
        // hiding the tab bar on a screen takes the two away with the
        // sidebar; the toolbar stays, as it stays for a stack.
        let sidebar = presenter.sidebar.installed(in: window), sidebarItems = sidebar && presenter.sidebar.inToolbar(of: window)
        guard declared != nil || header != nil || beside != nil || held || sidebar else { reset(); return }
        // A host that takes its toolbar back wins. Never overwrite it on a tick.
        guard window.toolbar == nil || window.toolbar === toolbar else { refuse("window owner installed another toolbar"); return }
        if declared == nil, header == nil, beside == nil { items.removeAll(); symbols.removeAll() }
        owner = declared?.owner ?? header?.header; heading = declared?.heading
        self.header = header
        self.beside = beside
        let declarations = declared?.shown ?? [], buttons = declared?.buttons ?? (header?.buttons ?? []) + (beside?.buttons ?? [])
        // A list beside its detail (LLP 1115.002 D4): the bar's items stop
        // at the divider between them and begin again after it, so each
        // header's buttons are over its own column.
        let split = presenter.navigation.split, columns = declared == nil && split.active && split.controller?.view.window === window
        let listIsOwner = header == nil || header?.route === split.shown?.list
        let (over, after): (WindowHeader?, WindowHeader?) = !columns ? (header, nil) : listIsOwner ? (header, beside) : (beside, header)
        let installing = toolbar == nil
        if installing {
            savedTitle = window.title
            // An identifier of its own: AppKit keeps every toolbar of one
            // identifier in a process the same, item for item, and each
            // window's bar is its own route's (an insertion in one window
            // put the item in another's, found as two tests in one process;
            // and the toolbar the window had until the view moved to or
            // from a sidebar's pane, #393).
            let value = NSToolbar(identifier: "Exact.window.commands." + UUID().uuidString)
            value.delegate = self
            value.displayMode = .iconOnly
            value.allowsUserCustomization = false
            toolbar = value
        }
        guard let toolbar else { return }
        heldForBack = held
        let live = Set(buttons.map(\.id))
        for id in Array(items.keys) where !live.contains(id) { items.removeValue(forKey: id); symbols.removeValue(forKey: id) }
        for node in buttons {
            // A header's button that opens a menu is AppKit's menu item
            // (LLP 1115.002 D2): it pops the menu up itself, so it has no
            // action.
            let menu = declared == nil && presenter.menus.toolbarPopover(of: node) != nil
            if let old = items[node.id], (old is NSMenuToolbarItem) != menu { items.removeValue(forKey: node.id); symbols.removeValue(forKey: node.id) }
            let item = items[node.id] ?? (menu ? NSMenuToolbarItem(itemIdentifier: .init("exact.menu.\(node.id)")) : NSToolbarItem(itemIdentifier: .init("exact.command.\(node.id)")))
            items[node.id] = item
            item.tag = Int(node.id)
            if let item = item as? NSMenuToolbarItem {
                if (item.menu as? InvokerMenu)?.source !== node { item.menu = presenter.menus.toolbarMenu(for: node) }
                item.showsIndicator = true
                item.autovalidates = false
            } else {
                item.target = self; item.action = #selector(performItem(_:))
                item.autovalidates = true
            }
            // A native button's children are its face, not views (LLP 1069.011.000 D1, D2).
            let face = node.isNativeButton ? node.face : nil
            let label = node.props["accessibilityLabel"] ?? face?.title ?? text(node)
            if item.label != label { item.label = label; item.paletteLabel = label; item.toolTip = label }
            let image = children(node).first { $0.kind == "image" }
            // Standard toolbar items own their image sizing, tint and glass.
            // An svg-only icon's path is a template image too (stage 9 A2).
            let symbol = face?.symbol ?? image?.faceSymbol ?? ""
            if symbol.isEmpty {
                if item.image !== image?.image { item.image = image?.image }
                symbols.removeValue(forKey: node.id)
            } else if symbols[node.id] != symbol {
                item.image = FaceSymbol.image(symbol, points: FaceSymbol.toolbarPoints)
                symbols[node.id] = symbol
            }
            item.isBordered = true
            // A header's buttons before its heading lead the bar, ahead of the title, as Back does.
            let navigational = node.props["toolbarPlacement"] == "navigation" || over?.leading.contains { $0 === node } == true
            if item.isNavigational != navigational { item.isNavigational = navigational }
            prominence(item, node, face)
            // Text-bearing authored buttons remain labeled in icon-only mode.
            let title = face.map { $0.title ?? "" } ?? text(node)
            if item.title != title { item.title = title }
            let enabled = !node.disabled && !node.inert && window.attachedSheet == nil
            if item.isEnabled != enabled { item.isEnabled = enabled }
        }
        // A header's, in the order SwiftUI's own toolbar keeps (measured on
        // macOS 27): its leading buttons after Back; its tablist as the
        // centred group, then a flexible space; its trailing buttons, which
        // the title's own stretch puts at the bar's end; then a flexible
        // space and its search field, last.
        // One centred group and one search field (AppKit refuses a second):
        // the window's own header's, else those of the one beside it.
        let grouped = headers.first { $0.segments != nil }, searched = headers.first { $0.search != nil }
        let group = segments.update(grouped, enabled: window.attachedSheet == nil)
        let centered: Set<NSToolbarItem.Identifier> = group.map { [$0.itemIdentifier] } ?? []
        if toolbar.centeredItemIdentifiers != centered { toolbar.centeredItemIdentifiers = centered }
        let field = search.update(searched, enabled: window.attachedSheet == nil)
        let identifiers = { (nodes: [NodeView]) in nodes.compactMap { self.items[$0.id]?.itemIdentifier } }
        let nextOrder = header != nil || beside != nil
            ? identifiers(over?.leading ?? []) + (group.map { [$0.itemIdentifier, .flexibleSpace] } ?? []) + identifiers(over?.trailing ?? [])
                + (columns ? [Self.listSeparatorIdentifier] : []) + identifiers(after?.buttons ?? [])
                + (field.map { [.flexibleSpace, $0.itemIdentifier] } ?? [])
            : declarations.compactMap { node -> NSToolbarItem.Identifier? in
                if node === heading { return .flexibleSpace }
                return items[node.id]?.itemIdentifier
            }
        order = (sidebarItems ? [.toggleSidebar, .sidebarTrackingSeparator] : []) + (back ? [Self.backIdentifier] : [])
            + nextOrder + appItems.map(\.itemIdentifier)
        // Keep both toolbar and command objects stable through route changes.
        for index in stride(from: toolbar.items.count - 1, through: 0, by: -1) {
            if !order.contains(toolbar.items[index].itemIdentifier) { toolbar.removeItem(at: index) }
        }
        // Each place in turn: an item of that identifier further along
        // moves here, never one already placed (a bar can hold two
        // flexible spaces); what is left past the last place goes.
        for (index, identifier) in order.enumerated() {
            if index < toolbar.items.count && toolbar.items[index].itemIdentifier == identifier { continue }
            if let old = toolbar.items.indices.dropFirst(index).first(where: { toolbar.items[$0].itemIdentifier == identifier }) { toolbar.removeItem(at: old) }
            toolbar.insertItem(withItemIdentifier: identifier, at: index)
        }
        while toolbar.items.count > order.count { toolbar.removeItem(at: toolbar.items.count - 1) }
        // Shown as it will answer, without waiting for AppKit's next validation.
        if back, backItem.isEnabled != navigation.canGoBack { backItem.isEnabled = navigation.canGoBack }
        appliedTitle = title(over: savedTitle)
        if window.title != appliedTitle { window.title = appliedTitle }
        let content = contentSize, attaching = window.toolbar !== toolbar
        if attaching { window.toolbar = toolbar }
        if installing { onChange?() }
        // After the window's owner has styled the toolbar (`onChange`).
        if attaching { keepContent(content) }
        hatchToolbar()
        // AppKit creates the new items' accessibility elements during layout.
        window.contentView?.superview?.layoutSubtreeIfNeeded()
        syncAccessibility()
        // Of the header beside the window's, what the bar shows or stands
        // for leaves the page; its heading and the rest stay in its pane.
        if let beside {
            let taken = beside.buttons.filter { items[$0.id] != nil } + beside.backs
                + (grouped?.header === beside.header && group != nil ? [beside.segments].compactMap { $0 } : [])
                + (searched?.header === beside.header && field != nil ? [beside.search].compactMap { $0 } : [])
            for node in taken {
                besideParts.append((node, node.isAccessibilityHidden()))
                node.isHidden = true; node.setAccessibilityHidden(true)
            }
        }
        // Hide only the authored rendering. Its logical nodes/actions survive.
        guard let next = declared?.owner ?? header?.header else { return }
        savedAccessibilityHidden = next.isAccessibilityHidden()
        next.isHidden = true; next.setAccessibilityHidden(true); projected = true
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { order }
    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { order }
    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier identifier: NSToolbarItem.Identifier, willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        if identifier == Self.backIdentifier { return backItem }
        // AppKit's own separator, told which divider it follows: the
        // standard one looks for it in the toolbar's window, and these items
        // are placed before the toolbar has one.
        if identifier == .sidebarTrackingSeparator, let split = presenter.sidebar.splitView {
            return NSTrackingSeparatorToolbarItem(identifier: identifier, splitView: split, dividerIndex: 0)
        }
        // The same for the divider between a list and its detail (LLP 1115.002 D4).
        if identifier == Self.listSeparatorIdentifier, let controller = presenter.navigation.split.controller {
            return NSTrackingSeparatorToolbarItem(identifier: identifier, splitView: controller.splitView, dividerIndex: controller.listDivider)
        }
        if let group = segments.item, group.itemIdentifier == identifier { return group }
        if let field = search.item, field.itemIdentifier == identifier { return field }
        return items.values.first { $0.itemIdentifier == identifier } ?? appItems.first { $0.itemIdentifier == identifier }
    }
    func validateToolbarItem(_ item: NSToolbarItem) -> Bool {
        syncAccessibility()
        if item === backItem { return toolbar != nil && window?.toolbar === toolbar && presenter.navigation.canGoBack }
        // An item of a toolbar that went is no node's: its Back, which each
        // toolbar makes again, carries AppKit's own tag, and AppKit may still
        // ask of it once (the run loop of a test's next step did).
        guard let id = UInt32(exactly: item.tag), let node = presenter.views[id], items[node.id] === item else { return false }
        return available(node)
    }

    /// Standard items have no `view`. Their exposed elements are the
    /// toolbar's children, in item order, with spaces/separators omitted.
    /// Set only accessibility state; AppKit's item stays enabled for input.
    private func syncAccessibility() {
        guard let window, let toolbar,
              let bar = window.accessibilityChildren()?.compactMap({ $0 as? NSAccessibilityProtocol })
                .first(where: { $0.accessibilityRole() == .toolbar }) else { return }
        let spaces: Set<NSToolbarItem.Identifier> = [.space, .flexibleSpace, .sidebarTrackingSeparator, Self.listSeparatorIdentifier]
        let visible = (toolbar.visibleItems ?? toolbar.items).filter { !spaces.contains($0.itemIdentifier) }
        let children = (bar.accessibilityChildren() ?? []).compactMap { $0 as? NSAccessibilityProtocol }
        guard children.count == visible.count else { return }
        for (item, child) in zip(visible, children) {
            guard let id = UInt32(exactly: item.tag), let node = presenter.views[id], items[id] === item,
                  child.accessibilityLabel() == item.label else { continue }
            let enabled = item.isEnabled && !node.ariaUnavailable
            if child.isAccessibilityEnabled() != enabled { child.setAccessibilityEnabled(enabled) }
        }
    }
    @objc private func performBack(_ item: NSToolbarItem) {
        guard validateToolbarItem(item) else { return }
        presenter.navigation.goBack()
    }
    @objc private func performItem(_ item: NSToolbarItem) {
        guard validateToolbarItem(item) else { return }
        presenter.press(UInt32(item.tag))
    }

    /// The driver uses the same target/action as AppKit, including overflow.
    /// This is host activation, not a claim to have injected a physical click.
    func activate(_ node: NodeView) -> Bool? {
        // The stack's Back control in a header the bar shows: the bar's own
        // Back stands for it, and is pressed as a person presses it.
        if headers.contains(where: { $0.backs.contains { $0 === node } }), projected || besideParts.contains(where: { $0.node === node }) {
            guard order.contains(Self.backIdentifier), validateToolbarItem(backItem), let action = backItem.action else { return false }
            return NSApp.sendAction(action, to: backItem.target, from: backItem)
        }
        // The header's search input: the caret goes to its field in the bar.
        if search.node === node, headers.contains(where: { $0.search === node }) { return focusSearch(node) }
        // A tab of the header's tablist: its segment's press.
        if let index = headers.first(where: { $0.segments != nil && $0.segments?.id == segments.tablist })?.tabs.firstIndex(where: { $0 === node }) {
            guard usable(node), let tab = segments.tab(at: index) else { return false }
            presenter.press(tab.id)
            sync()
            return true
        }
        guard let item = items[node.id] else { return nil }
        // A menu item's menu is AppKit's to pop up, which no driver can do
        // for it: the button is pressed, and under the agent its menu opens
        // as the painted popover every menu is there (LLP 1021 D4).
        if item is NSMenuToolbarItem {
            guard available(node) else { return false }
            presenter.press(node.id)
            return true
        }
        guard available(node), let action = item.action else { return false }
        return NSApp.sendAction(action, to: item.target, from: item)
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        guard contains(node), let toolbar else { return nil }
        let titled = node === heading || (projected && node === header?.heading)
        var result: [String: Any] = ["view": node === owner ? "NSToolbar" : (titled ? "NSWindow.title" : "NSToolbarItem"),
                                   "placement": "window", "title": window?.title ?? "",
                                   "geometry": "system-owned", "attached": window?.toolbar === toolbar]
        if let item = items[node.id] ?? (headers.contains { $0.backs.contains { $0 === node } } && order.contains(Self.backIdentifier) ? backItem : nil) {
            result["identifier"] = item.itemIdentifier.rawValue
            result["enabled"] = item === backItem ? validateToolbarItem(item) : available(node)
            result["overflow"] = !(toolbar.visibleItems ?? []).contains(where: { $0 === item })
        }
        if let field = search.item, search.node === node {
            result["view"] = "NSSearchToolbarItem"
            result["identifier"] = field.itemIdentifier.rawValue
            result["enabled"] = usable(node) && field.isEnabled
            result["overflow"] = !(toolbar.visibleItems ?? []).contains(where: { $0 === field })
        }
        let grouped = headers.first { $0.segments != nil && $0.segments?.id == segments.tablist }
        if let group = segments.item, let grouped, node === grouped.segments || grouped.tabs.contains(where: { $0 === node }) {
            result["view"] = "NSToolbarItemGroup"
            result["identifier"] = group.itemIdentifier.rawValue
            result["selected"] = group.selectedIndex
            result["overflow"] = !(toolbar.visibleItems ?? []).contains(where: { $0 === group })
            if let index = grouped.tabs.firstIndex(where: { $0 === node }) {
                result["segment"] = index
                result["enabled"] = usable(node) && segments.tab(at: index) != nil
            }
        }
        return result
    }
}
#endif

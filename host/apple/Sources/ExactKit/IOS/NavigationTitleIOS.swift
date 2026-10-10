// @ref LLP 1075.003 §9.10 — what a route's authored header and the root's
// tablist say about the bars beyond a title and items, declared, with no
// module: the heading's group as a richer title (an avatar, a subtitle, a
// press), the bar hidden for a route with no header, and the tab bar hidden
// for a route pushed while the authored tablist is. The web, macOS, Linux
// and the agent paint what is authored, which is where each rule comes from.
#if os(iOS) || os(tvOS)
import UIKit

/// The heading's group in a header-shaped route: the element around the
/// heading that holds no bar item, search field or tablist — or the
/// pressable element around it, which a tap on the title presses. Before
/// the heading, a filled box holding a text or a symbol is the title's
/// avatar; after it, the first text is its subtitle.
struct HeaderTitle: Equatable {
    let id: UInt32, testId: String?
    let avatar: BadgeFace?
    let subtitle: String
    /// The subtitle as a line of symbols and texts, in order, when it is one
    /// (a box of symbol images and texts after the heading): Signal's "🔕 Muted
    /// ⏱ 1w". Empty for a plain text subtitle.
    let glyphs: [Glyph]
    /// What VoiceOver reads for the subtitle: the glyph line's `aria-label`
    /// when it has one (a timer's "1w" means little alone), else its texts.
    let spoken: String
    /// The glyph line's authored direction is right to left.
    let rtl: Bool
    struct Glyph: Equatable { let symbol: String?; let text: String }
    /// The element a tap on the title presses.
    let tap: UInt32?

    init?(header: NodeView, heading: NodeView, tap: NodeView?, apart: [NodeView]) {
        // The heading's ancestors below the header, innermost first.
        var chain: [NodeView] = [], at = heading.superview
        while let view = at, view !== header {
            if let node = view as? NodeView { chain.append(node) }
            at = view.superview
        }
        guard let group = tap ?? chain.last(where: { a in !apart.contains { $0.isDescendant(of: a) } }) else { return nil }
        var avatar: BadgeFace?, subtitle = "", glyphs: [Glyph] = [], passed = false, spoken: String?, rtl = false
        func shown(_ node: NodeView) -> [NodeView] {
            node.container.subviews.compactMap { $0 as? NodeView }.filter { $0.style["display"]?.string != "none" }
        }
        /// A passive box: no button, no press, no fill (a pill or badge
        /// keeps its own look, so it is no line of glyphs).
        func passive(_ node: NodeView) -> Bool {
            !node.isButton && !node.actsAsButton && !node.handlers.contains("press") && node.channels("background_color") == nil
        }
        /// A passive box of symbol images and texts only, one of each at
        /// least: a glyph line. A symbol with no name or an empty text adds
        /// nothing and costs the line nothing; any other child is not one.
        func line(_ node: NodeView) -> [Glyph]? {
            guard !node.isParagraph, passive(node) else { return nil }
            var out: [Glyph] = [], symbols = false
            for child in shown(node) {
                guard passive(child) else { return nil }
                if child.kind == "image", child.props["imageSource"]?.hasPrefix("symbol:") == true {
                    // A symbol source counts toward the shape even while its
                    // name is blank, so the line keeps its texts and label.
                    symbols = true
                    if let name = child.props["symbolName"], !name.isEmpty { out.append(Glyph(symbol: name, text: "")) }
                } else if child.isParagraph {
                    if !child.accessibleText.isEmpty { out.append(Glyph(symbol: nil, text: child.accessibleText)) }
                } else { return nil }
            }
            return symbols && out.contains { $0.symbol == nil } ? out : nil
        }
        /// A control or a link is its own (a bar item), not the title's to
        /// read, unless it holds the heading (the pressable group).
        func control(_ node: NodeView) -> Bool {
            (node.isButton || node.actsAsButton || node.handlers.contains("press")) && !heading.isDescendant(of: node)
        }
        func walk(_ node: NodeView) {
            for child in shown(node) {
                if child === heading { passed = true; continue }
                if control(child) { continue }
                if !passed, avatar == nil, !heading.isDescendant(of: child), let face = BadgeFace(box: child, authored: true) { avatar = face; continue }
                if passed, subtitle.isEmpty, child.isParagraph, !child.accessibleText.isEmpty { subtitle = child.accessibleText; continue }
                if passed, subtitle.isEmpty, let pieces = line(child) {
                    glyphs = pieces
                    subtitle = pieces.compactMap { $0.symbol == nil ? $0.text : nil }.joined(separator: "  ")
                    // What it says, as the author named it, else its texts.
                    spoken = child.authoredLabel ?? subtitle
                    rtl = child.style["direction"]?.string == "rtl"
                    continue
                }
                // After the heading, a filled box (a pill, a badge) keeps its
                // own look: nothing in it is the subtitle.
                if passed, child.channels("background_color") != nil { continue }
                walk(child)
            }
        }
        walk(group)
        // A wrapper around the heading alone adds nothing to the title.
        guard tap != nil || avatar != nil || !subtitle.isEmpty else { return nil }
        id = group.id
        testId = group.props["testId"]
        self.avatar = avatar
        self.subtitle = subtitle
        self.glyphs = glyphs
        self.spoken = spoken ?? subtitle
        self.rtl = rtl
        self.tap = tap?.id
    }

    /// Everything the title is drawn from.
    var source: String { "\(id):\(avatar?.source ?? ""):\(subtitle):\(spoken):\(rtl):\(glyphs.map { "\($0.symbol ?? "")/\($0.text)" }.joined(separator: "\u{1F}")):\(tap ?? 0)" }
    static func == (a: HeaderTitle, b: HeaderTitle) -> Bool { a.source == b.source }

    static func holdsHeading(_ node: NodeView) -> Bool {
        node.container.subviews.contains { child in
            guard let child = child as? NodeView else { return false }
            return (child.isParagraph && child.headingLevel != nil) || holdsHeading(child)
        }
    }
}

extension HeaderShape {
    /// The shape, unless its header is hidden as authored (`display: none`):
    /// a route whose header the web does not show has no bar (§9.10).
    static func shown(_ shape: HeaderShape) -> HeaderShape? {
        shape.header.style["display"]?.string == "none" ? nil : shape
    }
}

/// A title drawn from the heading's group: the avatar, the heading over
/// its subtitle in the bar's own type, a tap pressing the group.
final class HeaderTitleView: UIControl {
    private weak var host: NavigationHost?
    private var tap: UInt32?
    let avatar = UIImageView(), title = UILabel(), subtitle = UILabel()
    private let stack = UIStackView(), texts = UIStackView()
    /// The avatar's box, its face's size: the image's own size is not to be
    /// trusted, as its dark variant from the asset came back at 1x and drew
    /// the avatar three times too large in dark mode.
    private lazy var avatarWidth = avatar.widthAnchor.constraint(equalToConstant: BadgeFace.size)
    private lazy var avatarHeight = avatar.heightAnchor.constraint(equalToConstant: BadgeFace.size)

    init(host: NavigationHost) {
        self.host = host
        super.init(frame: CGRect(x: 0, y: 0, width: 200, height: 44))
        title.font = .preferredFont(forTextStyle: .headline)
        subtitle.font = .preferredFont(forTextStyle: .footnote)
        subtitle.textColor = .secondaryLabel
        for label in [title, subtitle] { label.adjustsFontForContentSizeCategory = true; label.lineBreakMode = .byTruncatingTail }
        avatar.contentMode = .scaleAspectFit
        texts.axis = .vertical
        texts.addArrangedSubview(title)
        texts.addArrangedSubview(subtitle)
        stack.axis = .horizontal
        stack.alignment = .center
        stack.spacing = 8
        stack.addArrangedSubview(avatar)
        stack.addArrangedSubview(texts)
        NSLayoutConstraint.activate([avatarWidth, avatarHeight])
        stack.isUserInteractionEnabled = false
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor), stack.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
            stack.centerXAnchor.constraint(equalTo: centerXAnchor), stack.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        addTarget(self, action: #selector(pressed), for: .touchUpInside)
        // The subtitle's symbols are sized from its font: rebuild them when
        // the text size changes (their colour is the run's, already dynamic).
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (view: HeaderTitleView, _: UITraitCollection) in
            guard let (group, text) = view.shown else { return }
            view.shown = nil
            view.subtitle.font = .preferredFont(forTextStyle: .footnote, compatibleWith: view.traitCollection)
            view.update(group, title: text)
        }
        isAccessibilityElement = true
    }
    required init?(coder: NSCoder) { nil }

    override var intrinsicContentSize: CGSize {
        let fitted = stack.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        return CGSize(width: fitted.width, height: max(44, fitted.height))
    }
    override var isHighlighted: Bool { didSet { stack.alpha = isHighlighted ? 0.5 : 1 } }

    /// The subtitle as drawn: its glyph line's symbols inline at the text's
    /// size and colour, an item two spaces from the next, or its text.
    static func line(_ group: HeaderTitle, font: UIFont, colour: UIColor) -> NSAttributedString {
        let paragraph = NSMutableParagraphStyle()
        paragraph.baseWritingDirection = group.rtl ? .rightToLeft : .natural
        paragraph.lineBreakMode = .byTruncatingTail
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: colour, .paragraphStyle: paragraph]
        guard !group.glyphs.isEmpty else { return NSAttributedString(string: group.subtitle, attributes: attributes) }
        let out = NSMutableAttributedString()
        for (i, glyph) in group.glyphs.enumerated() {
            if i > 0, group.glyphs[i - 1].symbol == nil { out.append(NSAttributedString(string: "  ", attributes: attributes)) }
            if let name = glyph.symbol {
                // A template symbol in the run's font: TextKit tints it with
                // the run's (dynamic) colour, so it follows the appearance.
                guard let image = UIImage(systemName: name, withConfiguration: UIImage.SymbolConfiguration(font: font))?.withRenderingMode(.alwaysTemplate) else { continue }
                let symbol = NSMutableAttributedString(attachment: NSTextAttachment(image: image))
                symbol.addAttributes(attributes, range: NSRange(location: 0, length: symbol.length))
                out.append(symbol)
                out.append(NSAttributedString(string: "\u{2009}", attributes: attributes))
            } else {
                out.append(NSAttributedString(string: glyph.text, attributes: attributes))
            }
        }
        return out
    }

    private var shown: (HeaderTitle, String)?

    func update(_ group: HeaderTitle, title text: String) {
        // Every batch projects the routes again: an unchanged title is left
        // alone, since invalidating its size relaid the whole navigation
        // bar out each time (~1 ms a batch, a fling's every frame).
        if let shown, shown.0 == group, shown.0.testId == group.testId, shown.1 == text { return }
        shown = (group, text)
        tap = group.tap
        avatar.image = group.avatar?.image
        avatar.isHidden = group.avatar == nil
        avatarWidth.constant = group.avatar?.size ?? 0
        avatarHeight.constant = group.avatar?.size ?? 0
        title.text = text
        subtitle.attributedText = Self.line(group, font: subtitle.font, colour: subtitle.textColor)
        subtitle.isHidden = group.subtitle.isEmpty
        texts.alignment = group.avatar == nil ? .center : .leading
        isEnabled = group.tap != nil
        accessibilityLabel = text
        accessibilityValue = group.subtitle.isEmpty ? nil : group.spoken
        accessibilityTraits = group.tap == nil ? .header : .button
        accessibilityIdentifier = group.testId
        invalidateIntrinsicContentSize()
    }

    @objc private func pressed() {
        if let tap { _ = host?.act(tap, 0) }
    }
}

extension NavigationHost {
    /// Whether a route shows its stack's bar: the stack has one and the
    /// route's header is shaped for it and shown (§9.10).
    func routeShowsBar(_ c: RouteController, in nav: UINavigationController) -> Bool {
        barShows(nav) && HeaderShape(route: c.node, back: nil).flatMap(HeaderShape.shown) != nil
    }

    /// Whether the bar is UIKit's for now on a route's account: its header
    /// search is active or being presented or dismissed, and UIKit hides and
    /// shows the bar for it (`hidesNavigationBarDuringPresentation`, §9.6).
    func searching(_ c: RouteController?) -> Bool {
        guard let search = c?.search?.controller else { return false }
        return search.isActive || search.isBeingPresented || search.isBeingDismissed
    }

    /// Whether the bar shows for the route on top of a stack.
    func topShowsBar(_ nav: UINavigationController) -> Bool {
        (nav.topViewController as? RouteController).map { routeShowsBar($0, in: nav) } ?? barShows(nav)
    }

    /// Show or hide a stack's bar for `c` (its top by default), on change.
    /// Called from `willShow`, UIKit's place for it: the bar moves with the
    /// transition, an interactive pop's included, and back on a cancel.
    func showBar(_ nav: UINavigationController, for c: RouteController? = nil, animated: Bool) {
        let c = c ?? nav.topViewController as? RouteController
        guard stacks[ObjectIdentifier(nav)] != nil, !searching(c) else { return }
        let shows = c.map { routeShowsBar($0, in: nav) } ?? barShows(nav)
        if nav.isNavigationBarHidden == shows { nav.setNavigationBarHidden(!shows, animated: animated) }
    }

    /// The heading's group as the item's title (§9.10). A subtitle alone is
    /// UIKit's own (`navigationItem.subtitle`, iOS 26); an avatar or a press,
    /// or a subtitle before iOS 26, is a drawn title view. A tablist's
    /// segments take the title view first (§9.8). What a hatch set instead —
    /// another title view, its own subtitle — is left alone.
    func richTitle(_ shape: HeaderShape?, in c: RouteController) {
        let item = c.navigationItem
        let group = shape?.segments == nil ? shape?.group : nil
        var subtitled = false
        if #available(iOS 26.0, *) { subtitled = true }
        let drawn = group.map { $0.avatar != nil || $0.tap != nil || !$0.glyphs.isEmpty || !subtitled } ?? false
        // tvOS's navigation item has no subtitle.
        #if !os(tvOS)
        if #available(iOS 26.0, *) {
            let subtitle = drawn ? nil : group?.subtitle
            if subtitle != c.subtitle {
                if item.subtitle == c.subtitle { item.subtitle = subtitle }
                c.subtitle = subtitle
            }
        }
        #endif
        guard drawn, let group, let shape else {
            if let old = c.titleView, item.titleView === old { item.titleView = nil }
            c.titleView = nil
            return
        }
        let view = c.titleView ?? HeaderTitleView(host: self)
        view.update(group, title: shape.title)
        if c.titleView !== view {
            c.titleView = view
            if item.titleView == nil { item.titleView = view }
        }
    }

    /// A route pushed while the root's authored tablist is hidden hides the
    /// tab bar (`hidesBottomBarWhenPushed`), as the web hides the tablist.
    /// UIKit reads it at the push and keeps the bar hidden for the routes
    /// above. The route the root names follows the tablist; routes pushed
    /// with it in one batch (a cold launch's) take the same. Written on
    /// change, so a hatch's own value stands till then. The root itself
    /// follows too (LLP 1075.003 §3.7, amended 2026-10-05): its tablist
    /// hidden hides the bar, shown shows it, both with UIKit's own animated
    /// `setTabBarHidden` (iOS 18), as Signal hides its tab bar for the chat
    /// list's multi-select.
    func followTablist(_ routes: [RouteController], in nav: UINavigationController) {
        guard !ExactEnv.authoredChrome, let key = container?.props["navigationKey"],
              let list = adoptedTablist.flatMap({ presenter.views[$0] }) ?? container.flatMap({ NavigationTabs.of($0, presenter)?.tablist })
        else { return }
        let hidden = list.style["display"]?.string == "none"
        if let root = routes.first, root.key == key {
            followTablistAtRoot(root, hidden: hidden, in: nav)
            return
        }
        guard let at = routes.indices.dropFirst().first(where: { routes[$0].key == key }) else { return }
        // A pushed route selected: the root, back on top later, arrives anew.
        if nav.tabBarController === tabController { tablistRoot = nil }
        for (index, c) in routes.enumerated() where index > 0 && index <= at && (index == at || c.tablistHidden == nil) && c.tablistHidden != hidden {
            c.tablistHidden = hidden
            #if !os(tvOS)
            c.hidesBottomBarWhenPushed = hidden
            #endif
            // On top already, with no push to read it (a route revealed by a
            // pop with a guessed flag, or the tablist changing under it): the
            // bar follows now (iOS 18).
            guard c === nav.topViewController, nav.transitionCoordinator == nil, tabBarShows, let tabs = tabController, nav.tabBarController === tabs else { continue }
            if #available(iOS 18.0, tvOS 18.0, *), tabs.isTabBarHidden != hidden {
                tabs.setTabBarHidden(hidden, animated: nav.view.window != nil)
                tablistHidBar = hidden
            }
        }
        // A bar Exact hid with `setTabBarHidden` (a root's hidden tablist, or
        // a pushed route's changing in place) is the controller's stored
        // state: a push of a route whose tablist shows does not clear it,
        // only `hidesBottomBarWhenPushed` would. The selected route wanting
        // it shown brings it back now, pushed or not yet.
        if !hidden, tablistHidBar, nav.transitionCoordinator == nil, tabBarShows, let tabs = tabController, nav.tabBarController === tabs {
            if #available(iOS 18.0, tvOS 18.0, *), tabs.isTabBarHidden { tabs.setTabBarHidden(false, animated: nav.view.window != nil) }
            tablistHidBar = false
        }
    }

    /// The root on top (LLP 1075.003 §3.7, amended 2026-10-05). Its wish is
    /// recorded whenever it is the selected route or a transition settles on
    /// it (`tablistHidden`), and the bar moves, animated
    /// once the stack is on screen, when the wish changes or the root has
    /// just arrived on top: a transition settled on it (`settleTablist`, from
    /// `didShow`, after a pop UIKit may have brought the bar back for) or
    /// another tab's root was selected. A hidden tablist hides the bar; a
    /// shown one shows it only if Exact hid it (`tablistHidBar`), so a hatch's
    /// own hide stands, and between those moments nothing is written, so a
    /// hatch's own show stands too. Never mid-transition. A root first seen
    /// with its tablist shown writes nothing. iOS 17 has no
    /// `setTabBarHidden`: there the bar stays.
    func followTablistAtRoot(_ root: RouteController, hidden: Bool, in nav: UINavigationController) {
        let changed = root.tablistHidden != hidden
        root.tablistHidden = hidden
        guard root === nav.topViewController, nav.transitionCoordinator == nil, tabBarShows,
              let tabs = tabController, nav.tabBarController === tabs else { return }
        guard #available(iOS 18.0, tvOS 18.0, *) else { return }
        // Arrived: the root on top at rest for the first time since another
        // route or root was (a pushed route selected clears `tablistRoot`).
        let arrived = tablistRoot !== root
        tablistRoot = root
        guard changed || arrived else { return }
        let animated = nav.view.window != nil
        if hidden {
            // Exact owns the hide only if it made it: a hatch's own stands.
            if !tabs.isTabBarHidden { tabs.setTabBarHidden(true, animated: animated); tablistHidBar = true }
        } else if tablistHidBar {
            if tabs.isTabBarHidden { tabs.setTabBarHidden(false, animated: animated) }
            tablistHidBar = false
        }
    }

    /// A transition settled (`didShow`): the root, if it is on top now,
    /// reconciles the bar with its tablist.
    func settleTablist(_ nav: UINavigationController) {
        guard !ExactEnv.authoredChrome, let key = container?.props["navigationKey"],
              let root = nav.viewControllers.first as? RouteController, root.key == key, nav.topViewController === root,
              let list = adoptedTablist.flatMap({ presenter.views[$0] }) ?? container.flatMap({ NavigationTabs.of($0, presenter)?.tablist })
        else { return }
        followTablistAtRoot(root, hidden: list.style["display"]?.string == "none", in: nav)
    }
}
#endif

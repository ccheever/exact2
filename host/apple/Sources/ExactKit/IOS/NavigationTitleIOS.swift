// @ref LLP 1075.003 §9.10 — what a route's authored header and the root's
// tablist say about the bars beyond a title and items, declared, with no
// module: the heading's group as a richer title (an avatar, a subtitle, a
// press), the bar hidden for a route with no header, and the tab bar hidden
// for a route pushed while the authored tablist is. The web, macOS, Linux
// and the agent paint what is authored, which is where each rule comes from.
#if os(iOS)
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
        var avatar: BadgeFace?, subtitle = "", passed = false
        func walk(_ node: NodeView) {
            for case let child as NodeView in node.container.subviews where child.style["display"]?.string != "none" {
                if child === heading { passed = true; continue }
                if !passed, avatar == nil, !heading.isDescendant(of: child), let face = BadgeFace(box: child) { avatar = face; continue }
                if passed, subtitle.isEmpty, child.isParagraph, !child.accessibleText.isEmpty { subtitle = child.accessibleText; continue }
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
        self.tap = tap?.id
    }

    /// Everything the title is drawn from.
    var source: String { "\(id):\(avatar?.source ?? ""):\(subtitle):\(tap ?? 0)" }
    static func == (a: HeaderTitle, b: HeaderTitle) -> Bool { a.source == b.source }

    static func holdsHeading(_ node: NodeView) -> Bool {
        node.container.subviews.contains { child in
            guard let child = child as? NodeView else { return false }
            return (child.isParagraph && child.props["accessibilityHeadingLevel"] != nil) || holdsHeading(child)
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
        stack.isUserInteractionEnabled = false
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor), stack.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
            stack.centerXAnchor.constraint(equalTo: centerXAnchor), stack.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        addTarget(self, action: #selector(pressed), for: .touchUpInside)
        isAccessibilityElement = true
    }
    required init?(coder: NSCoder) { nil }

    override var intrinsicContentSize: CGSize {
        let fitted = stack.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        return CGSize(width: fitted.width, height: max(44, fitted.height))
    }
    override var isHighlighted: Bool { didSet { stack.alpha = isHighlighted ? 0.5 : 1 } }

    func update(_ group: HeaderTitle, title text: String) {
        tap = group.tap
        avatar.image = group.avatar?.image
        avatar.isHidden = group.avatar == nil
        title.text = text
        subtitle.text = group.subtitle
        subtitle.isHidden = group.subtitle.isEmpty
        texts.alignment = group.avatar == nil ? .center : .leading
        isEnabled = group.tap != nil
        accessibilityLabel = text
        accessibilityValue = group.subtitle.isEmpty ? nil : group.subtitle
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

    /// Whether the bar is UIKit's for now: the top route's header search is
    /// active, and UIKit hides and shows the bar for it
    /// (`hidesNavigationBarDuringPresentation`, §9.6).
    func searching(_ nav: UINavigationController) -> Bool {
        (nav.topViewController as? RouteController)?.search?.controller.isActive == true
    }

    /// Whether the bar shows for the route on top of a stack.
    func topShowsBar(_ nav: UINavigationController) -> Bool {
        (nav.topViewController as? RouteController).map { routeShowsBar($0, in: nav) } ?? barShows(nav)
    }

    /// Show or hide a stack's bar for `c` (its top by default), on change.
    /// Called from `willShow`, UIKit's place for it: the bar moves with the
    /// transition, an interactive pop's included, and back on a cancel.
    func showBar(_ nav: UINavigationController, for c: RouteController? = nil, animated: Bool) {
        guard stacks[ObjectIdentifier(nav)] != nil, !searching(nav) else { return }
        let shows = (c ?? nav.topViewController as? RouteController).map { routeShowsBar($0, in: nav) } ?? barShows(nav)
        if nav.isNavigationBarHidden == shows { nav.setNavigationBarHidden(!shows, animated: animated) }
    }

    /// The heading's group as the item's title (§9.10). A subtitle alone is
    /// UIKit's own (`navigationItem.subtitle`, iOS 26); an avatar or a press,
    /// or a subtitle before iOS 26, is a drawn title view. A tablist's
    /// segments take the title view first (§9.8). What a hook set instead —
    /// another title view, its own subtitle — is left alone.
    func richTitle(_ shape: HeaderShape?, in c: RouteController) {
        let item = c.navigationItem
        let group = shape?.segments == nil ? shape?.group : nil
        var subtitled = false
        if #available(iOS 26.0, *) { subtitled = true }
        let drawn = group.map { $0.avatar != nil || $0.tap != nil || !subtitled } ?? false
        if #available(iOS 26.0, *) {
            let subtitle = drawn ? nil : group?.subtitle
            if subtitle != c.subtitle {
                if item.subtitle == c.subtitle { item.subtitle = subtitle }
                c.subtitle = subtitle
            }
        }
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
    /// change, so a hook's own value stands till then.
    func followTablist(_ routes: [RouteController], in nav: UINavigationController) {
        guard let key = container?.props["navigationKey"], let at = routes.indices.dropFirst().first(where: { routes[$0].key == key }),
              let list = adoptedTablist.flatMap({ presenter.views[$0] }) ?? container.flatMap({ NavigationTabs.of($0, presenter)?.tablist })
        else { return }
        let hidden = list.style["display"]?.string == "none"
        for (index, c) in routes.enumerated() where index > 0 && index <= at && (index == at || c.tablistHidden == nil) && c.tablistHidden != hidden {
            c.tablistHidden = hidden
            c.hidesBottomBarWhenPushed = hidden
            // On top already, with no push to read it (a route revealed by a
            // pop with a guessed flag, or the tablist changing under it): the
            // bar follows now (iOS 18).
            guard c === nav.topViewController, nav.transitionCoordinator == nil, let tabs = nav.tabBarController else { continue }
            if #available(iOS 18.0, *), tabs.isTabBarHidden != hidden { tabs.setTabBarHidden(hidden, animated: nav.view.window != nil) }
        }
    }
}
#endif

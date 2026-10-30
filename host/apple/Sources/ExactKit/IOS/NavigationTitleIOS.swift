// @ref LLP 1075.003 §9.10 — what a route's authored header and the root's
// tablist say about the bars beyond a title and items, declared, with no
// module: the heading's group as a richer title (an avatar, a subtitle, a
// press), and the tab bar hidden for a route pushed while the authored
// tablist is. The web, macOS, Linux and the agent paint what is authored,
// which is where each rule comes from.
// @ref LLP 1115.000 D5.2, D5.3 — which routes show the bar (`routeShowsBar`,
// by `NavigationRules.bar`): each route's own header, the system Back alone
// on a pushed route with none, and no bar where the route says so.
#if os(iOS) || os(tvOS)
import UIKit

/// The heading's group in a header-shaped route: the element around the
/// heading that holds no bar item, search field or tablist — or the
/// pressable element around it, which a tap on the title presses — else
/// the header itself (LLP 1116 D2). Before the heading, a filled box
/// holding a text or a symbol is the title's avatar (in a wrapper group
/// only: a box beside the heading in the header is no avatar); after it,
/// the first text is its subtitle.
struct HeaderTitle: Equatable {
    let id: UInt32, testId: String?
    let avatar: BadgeFace?
    let subtitle: String
    /// The nodes the title shows besides the heading: the avatar's box and
    /// the subtitle's text or glyph line (`HeaderShape.placed`).
    let placed: [NodeView]
    /// The subtitle as a line of symbols and texts, in order, when it is one
    /// (a box of symbol images and texts after the heading): Signal's "🔕 Muted
    /// ⏱ 1w". Empty for a plain text subtitle.
    let glyphs: [Glyph]
    let titleGlyphs: [TitleGlyph]
    struct TitleGlyph: Equatable {
        let name: String
        let before: Bool
        let light: [Double]?, dark: [Double]?
        var source: String { "\(name)/\(before)/\(light ?? [])/\(dark ?? [])" }
    }
    /// What VoiceOver reads for the subtitle: the glyph line's `aria-label`
    /// when it has one (a timer's "1w" means little alone), else its texts.
    let spoken: String
    /// The glyph line's authored direction is right to left.
    let rtl: Bool
    struct Glyph: Equatable { let symbol: String?; let text: String }
    /// The element a tap on the title presses.
    let tap: UInt32?
    let accessibilityDisabled: Bool

    init?(header: NodeView, heading: NodeView, tap: NodeView?, apart: [NodeView]) {
        // The heading's ancestors below the header, innermost first.
        var chain: [NodeView] = [], at = heading.superview
        while let view = at, view !== header {
            if let node = view as? NodeView { chain.append(node) }
            at = view.superview
        }
        // No wrapper qualifies (the heading is the header's own child, or
        // what wraps it holds a bar item): the header is the group, whose
        // walk passes over the items, the search field and the tablist as
        // controls (LLP 1116 D2, amending LLP 1075.003 §9.10).
        let wrapper = tap ?? chain.last(where: { a in !apart.contains { $0.isDescendant(of: a) } })
        let group = wrapper ?? header
        var avatar: BadgeFace?, subtitle = "", glyphs: [Glyph] = [], passed = false, spoken: String?, rtl = false
        var placed: [NodeView] = []
        func shown(_ node: NodeView) -> [NodeView] {
            node.container.subviews.compactMap { $0 as? NodeView }.filter { $0.style["display"]?.string != "none" }
        }
        /// A passive box: no button, no press, no fill (a pill or badge
        /// keeps its own look, so it is no line of glyphs).
        func passive(_ node: NodeView) -> Bool {
            !node.isButton && !node.actsAsButton && !node.handlers.contains("press") && node.channels("background_color") == nil
        }
        var titleGlyphs: [TitleGlyph] = [], headingImages: [NodeView] = []
        if let row = heading.superview as? NodeView, row.style["display"]?.string == "flex",
           (row.style["flex_direction"]?.string ?? "row") == "row" {
            let children = shown(row).filter { child in !apart.contains { $0 === child } }
            if children.allSatisfy({ $0 === heading || passive($0) && $0.kind == "image" && $0.faceSymbol != nil }) {
                var before = true
                for child in children {
                    if child === heading { before = false; continue }
                    guard let name = child.faceSymbol else { continue }
                    headingImages.append(child)
                    let tinted = child.ownColor("tint_color") != nil
                    titleGlyphs.append(TitleGlyph(name: name, before: before,
                        light: tinted ? child.undimmedChannels("tint_color", dark: false) : nil,
                        dark: tinted ? child.undimmedChannels("tint_color", dark: true) : nil))
                }
            }
        }
        /// A passive box of symbol images and texts only, one of each at
        /// least: a glyph line. A symbol with no name or an empty text adds
        /// nothing and costs the line nothing; any other child is not one.
        func line(_ node: NodeView) -> [Glyph]? {
            guard !node.isParagraph, passive(node) else { return nil }
            var out: [Glyph] = [], symbols = false
            for child in shown(node) {
                // A readout beside the glyphs is its own item.
                if apart.contains(where: { $0 === child }) { continue }
                guard passive(child) else { return nil }
                if child.kind == "image", child.props["imageSource"]?.hasPrefix("symbol:") == true {
                    // A symbol source counts toward the shape even while its
                    // name is blank, so the line keeps its texts and label.
                    symbols = true
                    if let name = child.faceSymbol { out.append(Glyph(symbol: name, text: "")) }
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
        func take(_ child: NodeView, _ pieces: [Glyph]) {
            glyphs = pieces
            placed.append(child)
            subtitle = pieces.compactMap { $0.symbol == nil ? $0.text : nil }.joined(separator: "  ")
            // What it says, as the author named it, else its texts.
            spoken = child.authoredLabel ?? subtitle
            rtl = child.style["direction"]?.string == "rtl"
        }
        func walk(_ node: NodeView) {
            for child in shown(node) {
                if child === heading { passed = true; continue }
                if headingImages.contains(where: { $0 === child }) { placed.append(child); continue }
                // A bar item, a readout among them, is not the title's to read.
                if control(child) || apart.contains(where: { $0 === child }) { continue }
                // A box holding one is no avatar; a row of glyphs beside a
                // readout is still the glyph line, without it; else what it
                // holds is read alone.
                if apart.contains(where: { $0.isDescendant(of: child) }) {
                    if passed, subtitle.isEmpty, let pieces = line(child) { take(child, pieces); continue }
                    if !(passed && child.channels("background_color") != nil) { walk(child) }
                    continue
                }
                if wrapper != nil, !passed, avatar == nil, !heading.isDescendant(of: child), let face = BadgeFace(box: child, authored: true) {
                    avatar = face; placed.append(child); continue
                }
                if passed, subtitle.isEmpty, child.isParagraph, !child.accessibleText.isEmpty { subtitle = child.accessibleText; placed.append(child); continue }
                if passed, subtitle.isEmpty, let pieces = line(child) { take(child, pieces); continue }
                // After the heading, a filled box (a pill, a badge) keeps its
                // own look: nothing in it is the subtitle.
                if passed, child.channels("background_color") != nil { continue }
                walk(child)
            }
        }
        walk(group)
        // A wrapper around the heading alone adds nothing to the title.
        guard tap != nil || avatar != nil || !subtitle.isEmpty || !titleGlyphs.isEmpty else { return nil }
        id = group.id
        testId = group.props["testId"]
        self.avatar = avatar
        self.subtitle = subtitle
        self.placed = placed
        self.glyphs = glyphs
        self.titleGlyphs = titleGlyphs
        self.spoken = spoken ?? subtitle
        self.rtl = rtl
        self.tap = tap?.id
        accessibilityDisabled = tap?.ariaUnavailable == true
    }

    /// Whether the title is a drawn title view rather than UIKit's own
    /// title and subtitle: an avatar, a press or a glyph line, or any
    /// subtitle before iOS 26, which has no `navigationItem.subtitle`.
    var drawn: Bool {
        var subtitled = false
        if #available(iOS 26.0, *) { subtitled = true }
        return avatar != nil || tap != nil || !glyphs.isEmpty || !titleGlyphs.isEmpty || !subtitled
    }

    /// Everything the title is drawn from.
    var source: String { "\(id):\(avatar?.source ?? ""):\(subtitle):\(spoken):\(rtl):\(glyphs.map { "\($0.symbol ?? "")/\($0.text)" }.joined(separator: "\u{1F}")):\(tap ?? 0):\(accessibilityDisabled):\(titleGlyphs.map(\.source).joined(separator: "\u{1F}"))" }
    static func == (a: HeaderTitle, b: HeaderTitle) -> Bool { a.source == b.source }

    static func holdsHeading(_ node: NodeView) -> Bool {
        node.container.subviews.contains { child in
            guard let child = child as? NodeView else { return false }
            return child.isHeading || holdsHeading(child)
        }
    }
}

extension HeaderShape {
    /// The header a route's bar is projected from in the app: its shape,
    /// unless the route says it has no bar (`navigationBar="none"`, LLP
    /// 1115.000 D5.2), where its header paints in the page as written, or
    /// its header is hidden as authored (`display: none`): a header the web
    /// does not show is no bar's (§9.10), and its route is one with no
    /// header. Both are the shared rule's (`HeaderRule`, `projected`).
    static func bar(of route: NodeView, back: String? = nil, backIsUIKits: Bool = true) -> HeaderShape? {
        HeaderShape(route: route, back: back, backIsUIKits: backIsUIKits, projected: true)
    }

    /// The heading's group as the bar shows it (§9.10). A tablist's
    /// segments take the title view first (§9.8): beside them a group
    /// shows only as UIKit's own subtitle (iOS 26), never drawn.
    static func presented(_ group: HeaderTitle?, segments: NodeView?) -> HeaderTitle? {
        guard let group else { return nil }
        return segments == nil || !group.drawn ? group : nil
    }

    /// Whether the bar shows `node`, a node of the header it replaces, or
    /// stands for it (LLP 1116 D2): a node it places, one inside such a
    /// node (a button's face), or one holding one (the header, a wrapper).
    /// Everything else in the header is hidden with it on iOS, which the
    /// agent says under its own chrome (`unshownByBar`). A node outside the
    /// header, or one the web does not show either (`display: none`), is
    /// not the bar's to judge.
    func presents(_ node: NodeView) -> Bool {
        if authored.contains(where: { node === $0 || node.isDescendant(of: $0) }) { return true }
        guard node === header || node.isDescendant(of: header) else { return true }
        var at: UIView? = node
        while let view = at, view !== header.superview {
            if (view as? NodeView)?.style["display"]?.string == "none" { return true }
            // A menu or dialog written in the header is the top layer's, as
            // the shared rule reads it (`fact::LAYER`): shown when opened.
            if let box = view as? NodeView, box.props["popover"] != nil || box.props["semanticTag"] == "dialog" { return true }
            at = view.superview
        }
        return placed.contains { node === $0 || node.isDescendant(of: $0) || $0.isDescendant(of: node) }
    }
}

/// A title drawn from the heading's group: the avatar, the heading over
/// its subtitle in the bar's own type, a tap pressing the group.
final class HeaderTitleView: UIControl {
    private weak var host: NavigationHost?
    private var tap: UInt32?
    #if EXACT_INSPECT
    /// The heading's group this title is drawn from (inspect mode selects it).
    private(set) var groupId: UInt32?
    #endif
    let avatar = UIImageView(), title = UILabel(), subtitle = UILabel()
    private let stack = UIStackView(), texts = UIStackView()
    /// The avatar's box, its face's size: the image's own size is not to be
    /// trusted, as its dark variant from the asset came back at 1x and drew
    /// the avatar three times too large in dark mode.
    private lazy var avatarWidth = avatar.widthAnchor.constraint(equalToConstant: BadgeFace.size)
    private lazy var avatarHeight = avatar.heightAnchor.constraint(equalToConstant: BadgeFace.size)

    /// The theme's `secondary-label` on the subtitle, set again when a live
    /// edit moves the roles (LLP 1121 stage 5).
    func retheme() {
        guard let host else { return }
        subtitle.textColor = ThemeRoles.color("secondary-label", in: host.presenter.root, or: .secondaryLabel)
    }

    init(host: NavigationHost) {
        self.host = host
        super.init(frame: CGRect(x: 0, y: 0, width: 200, height: 44))
        title.textColor = .label
        title.font = .preferredFont(forTextStyle: .headline)
        subtitle.font = .preferredFont(forTextStyle: .footnote)
        retheme()
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
            view.title.font = .preferredFont(forTextStyle: .headline, compatibleWith: view.traitCollection)
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
                guard let image = FaceSymbol.image(name, points: font.pointSize, configuration: UIImage.SymbolConfiguration(font: font))?.withRenderingMode(.alwaysTemplate) else { continue }
                let attachment = NSTextAttachment(image: image)
                // An svg-only icon's path (stage 9 A2) is no symbol TextKit
                // seats on the line: its em square centred on the cap height.
                if FaceSymbol.isPath(name) {
                    attachment.bounds = CGRect(x: 0, y: (font.capHeight - image.size.height) / 2, width: image.size.width, height: image.size.height)
                }
                let symbol = NSMutableAttributedString(attachment: attachment)
                symbol.addAttributes(attributes, range: NSRange(location: 0, length: symbol.length))
                out.append(symbol)
                out.append(NSAttributedString(string: "\u{2009}", attributes: attributes))
            } else {
                out.append(NSAttributedString(string: glyph.text, attributes: attributes))
            }
        }
        return out
    }

    private static func titleLine(_ group: HeaderTitle, text: String, font: UIFont, colour: UIColor) -> NSAttributedString {
        let out = NSMutableAttributedString()
        func append(_ glyph: HeaderTitle.TitleGlyph) {
            guard let image = FaceSymbol.image(glyph.name, points: font.pointSize,
                configuration: UIImage.SymbolConfiguration(font: font))?.withRenderingMode(.alwaysTemplate) else { return }
            let attachment = NSTextAttachment(image: image)
            if FaceSymbol.isPath(glyph.name) {
                attachment.bounds = CGRect(x: 0, y: (font.capHeight - image.size.height) / 2, width: image.size.width, height: image.size.height)
            }
            let symbol = NSMutableAttributedString(attachment: attachment)
            symbol.addAttribute(.foregroundColor, value: colour, range: NSRange(location: 0, length: symbol.length))
            if let light = glyph.light, let dark = glyph.dark {
                symbol.addAttribute(.foregroundColor, value: UIColor { TextEngine.color($0.userInterfaceStyle == .dark ? dark : light) },
                    range: NSRange(location: 0, length: symbol.length))
            }
            out.append(symbol)
        }
        for glyph in group.titleGlyphs where glyph.before { append(glyph); out.append(NSAttributedString(string: "\u{2009}", attributes: [.foregroundColor: colour])) }
        out.append(NSAttributedString(string: text, attributes: [.foregroundColor: colour]))
        for glyph in group.titleGlyphs where !glyph.before { out.append(NSAttributedString(string: "\u{2009}", attributes: [.foregroundColor: colour])); append(glyph) }
        out.addAttribute(.font, value: font, range: NSRange(location: 0, length: out.length))
        return out
    }

    private var shown: (HeaderTitle, String)?
    private var shownInk: UIColor?

    func update(_ group: HeaderTitle, title text: String) {
        // Every batch projects the routes again: an unchanged title is left
        // alone, since invalidating its size relaid the whole navigation
        // bar out each time (~1 ms a batch, a fling's every frame).
        if let shown, shown.0 == group, shown.0.testId == group.testId, shown.1 == text, shownInk?.isEqual(title.textColor) == true { return }
        shown = (group, text)
        shownInk = title.textColor
        #if EXACT_INSPECT
        groupId = group.id
        #endif
        tap = group.tap
        avatar.image = group.avatar?.image
        avatar.isHidden = group.avatar == nil
        avatarWidth.constant = group.avatar?.size ?? 0
        avatarHeight.constant = group.avatar?.size ?? 0
        title.attributedText = group.titleGlyphs.isEmpty ? nil : Self.titleLine(group, text: text, font: title.font, colour: title.textColor)
        if group.titleGlyphs.isEmpty { title.text = text }
        subtitle.attributedText = Self.line(group, font: subtitle.font, colour: subtitle.textColor)
        subtitle.isHidden = group.subtitle.isEmpty
        texts.alignment = group.avatar == nil ? .center : .leading
        isEnabled = group.tap != nil
        accessibilityLabel = text
        accessibilityValue = group.subtitle.isEmpty ? nil : group.spoken
        accessibilityTraits = group.tap == nil ? .header : group.accessibilityDisabled ? [.button, .notEnabled] : .button
        accessibilityIdentifier = group.testId
        invalidateIntrinsicContentSize()
    }

    @objc private func pressed() {
        if let tap { _ = host?.act(tap, 0) }
    }
}

extension NavigationHost {
    /// What the agent says of a header node the bar leaves out.
    static let unshownReason = "the iOS navigation bar does not show this node; in the app it is not visible (the bar shows a header's heading, the first text after it as its subtitle, one search field, one tablist and its buttons)"

    /// The authored header nodes the iOS navigation bar would not show
    /// (LLP 1116 D2), by id. Under the agent's own chrome the header paints
    /// as on the web, so such a node still answers `expect` and shows in a
    /// screenshot; the agent marks it, and refuses a tap on it, by this.
    /// Each shown header of a route whose stack has a bar in the app (not
    /// as the agent's chrome draws it) is judged by `HeaderShape.presents`.
    func unshownByBar() -> [UInt32: String] {
        var out: [UInt32: String] = [:]
        let back = container?.props["navigationBack"]
        for routes in barStackRoutes {
            for (index, controller) in routes.enumerated() {
                guard let c = controller as? RouteController,
                      let shape = HeaderShape.bar(of: c.node, back: back, backIsUIKits: index > 0) else { continue }
                func walk(_ node: NodeView) {
                    for case let child as NodeView in node.container.subviews {
                        if !shape.presents(child) { out[child.id] = Self.unshownReason }
                        walk(child)
                    }
                }
                walk(shape.header)
            }
        }
        return out
    }

    /// Under the agent's chrome, what is painted in the page that the app's
    /// bars stand for: each route's header its navigation bar projects, and
    /// the tablist its tab bar does. Empty under `--chrome platform`.
    func paintedForBars() -> [NodeView] {
        guard ExactEnv.authoredChrome else { return [] }
        var out: [NodeView] = []
        let back = container?.props["navigationBack"]
        for routes in barStackRoutes {
            for (index, controller) in routes.enumerated() {
                if let c = controller as? RouteController, let shape = HeaderShape.bar(of: c.node, back: back, backIsUIKits: index > 0) { out.append(shape.header) }
            }
        }
        if let list = adoptedTablist.flatMap({ presenter.views[$0] }) ?? container.flatMap({ NavigationTabs.of($0, presenter)?.tablist }) { out.append(list) }
        return out.filter { $0.window != nil && !$0.isHidden }
    }

    /// Whether the platform's bar has a Back button for a pushed route to
    /// show alone: tvOS's has none (its Back is the remote's Menu).
    #if os(tvOS)
    static let barHasBackButton = false
    #else
    static let barHasBackButton = true
    #endif

    /// What a route shows of the bar, where its stack's routes may show it
    /// (`NavigationRules.bar`, LLP 1115.000 D5.2): its header, the system
    /// Back alone, or nothing.
    func bar(of route: NodeView, pushed: Bool) -> NavigationRules.Bar {
        if HeaderRule.keepsAuthoredHeader(route) { return .hidden }
        return NavigationRules.bar(asked: route.props["navigationBar"], header: HeaderShape.bar(of: route) != nil,
                                   pushed: pushed, backButton: Self.barHasBackButton)
    }

    /// Whether a route shows the bar: its stack's routes may, and the route
    /// has something to show in it. A held stack's (no `nav`) may, as no
    /// hatch has said otherwise.
    func routeShowsBar(_ c: RouteController, in nav: UINavigationController?) -> Bool {
        (nav.map(barShows) ?? !ExactEnv.authoredChrome) && bar(of: c.node, pushed: c.pushed) != .hidden
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
        let group = HeaderShape.presented(shape?.group, segments: shape?.segments)
        let drawn = group?.drawn ?? false
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
        view.title.textColor = shape.heading?.ownUIColor("text_color") ?? .label
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
    func followTablist(_ routes: [RouteController], in stack: UIViewController) {
        guard !ExactEnv.authoredChrome, let key = selectedKey,
              let list = adoptedTablist.flatMap({ presenter.views[$0] }) ?? container.flatMap({ NavigationTabs.of($0, presenter)?.tablist })
        else { return }
        let hidden = list.style["display"]?.string == "none"
        if let root = routes.first, root.key == key {
            followTablistAtRoot(root, hidden: hidden, in: stack)
            return
        }
        guard let nav = stack as? UINavigationController, let at = routes.indices.dropFirst().first(where: { routes[$0].key == key }) else { return }
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
            // (An iPad's floating bar: the whole stack's wish, as on a push. A
            // bar a route under this one hides, by UIKit's flag, is not shown.)
            let wish = floatingStackHides(nav) ?? hidden
            if !wish, Self.underHides(nav, c) { continue }
            if #available(iOS 18.0, tvOS 18.0, *), tabs.isTabBarHidden != wish {
                tabs.setTabBarHidden(wish, animated: nav.view.window != nil)
                tablistHidBar = wish
            }
        }
        // A bar Exact hid with `setTabBarHidden` (a root's hidden tablist, or
        // a pushed route's changing in place) is the controller's stored
        // state: a push of a route whose tablist shows does not clear it,
        // only `hidesBottomBarWhenPushed` would. The selected route wanting
        // it shown brings it back now, pushed or not yet.
        // (Not while a route above the root still hides it, on a phone by UIKit's flag, on an iPad by Exact's.)
        if !hidden, !nav.viewControllers.dropFirst().contains(where: Self.hidesBar),
           tablistHidBar, nav.transitionCoordinator == nil, tabBarShows, let tabs = tabController, nav.tabBarController === tabs {
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
    /// hatch's own show stands too. Never mid-transition: a change made then
    /// (a sheet presenting, a push or pop in flight) waits, and the root
    /// arrives again at the next call that can move the bar. A shown tablist
    /// leaves the bar as it is unless Exact hid it. iOS 17 has no
    /// `setTabBarHidden`: there the bar stays.
    func followTablistAtRoot(_ root: RouteController, hidden: Bool, in stack: UIViewController) {
        let changed = root.tablistHidden != hidden
        root.tablistHidden = hidden
        guard root === stack.stackRoutes.last, stack.transitionCoordinator == nil, tabBarShows,
              let tabs = tabController, stack.tabBarController === tabs else {
            if changed, tablistRoot === root { tablistRoot = nil }
            return
        }
        guard #available(iOS 18.0, tvOS 18.0, *) else { return }
        // Arrived: the root on top at rest for the first time since another
        // route or root was (a pushed route selected clears `tablistRoot`), or
        // since a change of its own the bar could not follow (above).
        let arrived = tablistRoot !== root
        tablistRoot = root
        guard changed || arrived else { return }
        let animated = stack.viewIfLoaded?.window != nil
        if hidden {
            // Exact owns the hide only if it made it: a hatch's own stands.
            if !tabs.isTabBarHidden { tabs.setTabBarHidden(true, animated: animated); tablistHidBar = true }
        } else if tablistHidBar {
            if tabs.isTabBarHidden { tabs.setTabBarHidden(false, animated: animated) }
            tablistHidBar = false
        }
    }

    /// iPadOS 18's floating tab bar (a regular width) does not take
    /// `hidesBottomBarWhenPushed`, so a pushed route whose tablist is hidden
    /// would keep it: as the transition to a pushed route starts, the bar
    /// takes that route's wish with `setTabBarHidden`, hidden if it or a
    /// route below it (the root aside) hides it, as UIKit reads the flag on
    /// a phone, and shown again only if Exact hid it (`tablistHidBar`). The
    /// root on top is `followTablistAtRoot`'s, after the transition (the
    /// lead's ruling, 2026-10-19). An interactive pop that is cancelled shows
    /// its source again, and so its wish.
    func followFloatingBar(_ nav: UINavigationController, for c: UIViewController, animated: Bool) {
        #if os(iOS)
        // The selected tab's stack only: another's late transition is not what shows.
        guard #available(iOS 18.0, *), let tabs = tabController, tabs.selectedViewController === nav else { return }
        // A cancelled pop's source comes back before it is in the stack again.
        let stack = nav.viewControllers.contains { $0 === c } ? nav.viewControllers : nav.viewControllers + [c]
        // However the transition ends (a custom interactive pop's cancel tells
        // the delegate nothing), the stack it leaves takes its wish then.
        if let coordinator = nav.transitionCoordinator {
            coordinator.animate(alongsideTransition: nil) { [weak self, weak nav] _ in
                guard let self, let nav, nav.transitionCoordinator == nil, let top = nav.topViewController else { return }
                self.followFloatingBar(nav, for: top, animated: false)
            }
        }
        guard let at = stack.firstIndex(where: { $0 === c }), at > 0, let hidden = floatingStackHides(nav, through: Array(stack[...at])) else { return }
        if hidden, !tabs.isTabBarHidden {
            tabs.setTabBarHidden(true, animated: animated)
            tablistHidBar = true
        } else if !hidden, tablistHidBar {
            if tabs.isTabBarHidden { tabs.setTabBarHidden(false, animated: animated) }
            tablistHidBar = false
        }
        #endif
    }

    /// Whether a stack whose routes are `stack` (its root first) hides the
    /// floating tab bar: a route above the root asks it, as UIKit reads
    /// `hidesBottomBarWhenPushed` on a phone. Nil where the bar is not
    /// iPadOS 18's floating one (a phone, a compact width, iOS 17, an
    /// authored or hatched tab bar), where UIKit's own flag does it.
    func floatingStackHides(_ nav: UINavigationController, through stack: [UIViewController]? = nil) -> Bool? {
        #if os(iOS)
        guard #available(iOS 18.0, *), !ExactEnv.authoredChrome, tabBarShows, let tabs = tabController, nav.tabBarController === tabs,
              tabs.traitCollection.userInterfaceIdiom == .pad, tabs.traitCollection.horizontalSizeClass == .regular else { return nil }
        return (stack ?? nav.viewControllers).dropFirst().contains { $0.hidesBottomBarWhenPushed }
        #else
        return nil
        #endif
    }

    /// Whether a route between the root and `c` (not `c`) hides the bar by
    /// UIKit's flag, which keeps it hidden over `c` on a phone too.
    static func underHides(_ nav: UINavigationController, _ c: UIViewController) -> Bool {
        guard let at = nav.viewControllers.firstIndex(where: { $0 === c }), at > 1 else { return false }
        return nav.viewControllers[1..<at].contains(where: hidesBar)
    }

    /// UIKit's flag, which tvOS has none of: its tab bar is the top's, and a push never hides it.
    static func hidesBar(_ c: UIViewController) -> Bool {
        #if os(tvOS)
        false
        #else
        c.hidesBottomBarWhenPushed
        #endif
    }

    /// The selected stack's top takes its wish again where nothing pushed or
    /// popped: a tab selected, the width becoming regular (once a resize's
    /// own transition is over; a push or pop has its own didShow).
    func refollowFloatingBar() {
        guard let tabs = tabController, let nav = tabs.selectedViewController as? UINavigationController, nav.topViewController != nil else { return }
        if let resizing = tabs.transitionCoordinator ?? nav.transitionCoordinator {
            let taken = resizing.animate(alongsideTransition: nil) { [weak self, weak nav] _ in
                guard let self, let nav, nav.transitionCoordinator == nil, let top = nav.topViewController else { return }
                self.followFloatingBar(nav, for: top, animated: false)
            }
            if taken { return }
        }
        followFloatingBar(nav, for: nav.topViewController!, animated: false)
    }

    /// A transition settled (`didShow`): the root, if it is on top now,
    /// reconciles the bar with its tablist.
    func settleTablist(_ nav: UINavigationController) {
        guard !ExactEnv.authoredChrome, let key = selectedKey,
              let root = nav.viewControllers.first as? RouteController, root.key == key, nav.topViewController === root,
              let list = adoptedTablist.flatMap({ presenter.views[$0] }) ?? container.flatMap({ NavigationTabs.of($0, presenter)?.tablist })
        else { return }
        followTablistAtRoot(root, hidden: list.style["display"]?.string == "none", in: nav)
    }
}
#endif

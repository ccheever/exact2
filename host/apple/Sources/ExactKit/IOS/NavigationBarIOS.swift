// @ref LLP 1075.003 §3.5, §3.7 — Exact's navigation bar on iOS, one per
// stack and fixed (titles that do not collapse): the header-shaped route's
// projection, the content area the bar leaves (`HostCover`), the delegate
// slot Exact keeps and forwards, the authored elements a hatch acts on, and
// the development check of what Exact owns. When the hatches run is
// NavigationIOS.swift's; the module side is ExactNativeModule.swift.
#if os(iOS) || os(tvOS)
import UIKit

/// What a route's bar item is projected from. A change projects it again.
struct BarSource: Equatable {
    let header: UInt32?, title: String, level: Int, leading: [String], trailing: [String], group: String, canGoBack: Bool
}

/// The inputs the route hatch last ran with. A change runs it again.
struct HatchSource: Equatable {
    let bar: BarSource, back: String, navigation: ObjectIdentifier, dataset: String, scroll: ObjectIdentifier?
}

/// A route whose first child is a `header` holding exactly one heading
/// (LLP 1035.001 D9's header-shaped route). The bar shows the heading as
/// its title — large for a level-1 heading, inline for any other — and the
/// header's buttons as its items, those before the heading leading and
/// those after it trailing, each pressing its authored button. The stack's
/// Back control is UIKit's back button, never an item. iOS does not paint
/// the header; the web and the agent do (LLP 1021 D4's one presentation).
struct HeaderShape: Equatable {
    struct Item: Equatable {
        let id: UInt32, title: String, symbol: String?, label: String?, disabled: Bool
        /// A face drawn from the button's one filled box (an avatar or a
        /// badge: a shape holding a text or a symbol), when the button has
        /// one: the item's image, in the box's own colours and corners.
        let badge: BadgeFace?
        /// The popover a button opens (`popovertarget`): the item's menu,
        /// its rows read when it opens, as LLP 1021 D3's pull-down.
        let menu: String?
        /// A native button's prominent style: a prominent bar item (iOS 26),
        /// as LLP 1069.011.000 D2 maps `NSToolbarItem`.
        let prominent: Bool
        /// The authored colour of the item's face, light and dark: a
        /// symbol's tint, else the text's `color` (CSS's, inherited from
        /// the button). Nil for a platform colour, which leaves UIKit's.
        let tint: [[Double]]?
        init(_ button: NodeView) {
            var symbol: String?, text = "", ink: (NodeView, String)?
            func walk(_ node: NodeView) {
                for case let child as NodeView in node.container.subviews {
                    if child.kind == "image", let name = child.props["symbolName"], !name.isEmpty {
                        if symbol == nil { symbol = name; ink = (child, "tint_color") }
                    } else if child.isParagraph, text.isEmpty { text = child.accessibleText; ink = ink ?? (child, "text_color") }
                    else { walk(child) }
                }
            }
            // A native button's children are its face, not views (LLP 1069.011.000 D1).
            let face = button.isNativeButton ? button.face : nil
            if let face {
                symbol = face.symbol
                text = face.title ?? ""
            } else {
                walk(button)
            }
            id = button.id
            badge = button.isNativeButton ? nil : BadgeFace(button)
            menu = button.props["popovertargetaction"] == "hide" ? nil : button.props["popovertarget"]
            self.symbol = symbol
            title = text
            label = button.props["accessibilityLabel"] ?? face?.label
            disabled = button.disabled
            prominent = ["filled", "bordered-prominent", "prominent-glass", "prominent-clear-glass"].contains(face?.style ?? "")
            let (node, key) = ink ?? (button, "text_color")
            let row = [node.style[key], button.style["text_color"]].lazy.compactMap { $0 }.first { !$0.isSystemColor }
            if let row, let light = row.channels(dark: false), let dark = row.channels(dark: true) {
                tint = [light, dark]
            } else {
                tint = nil
            }
        }
        /// Everything a bar item is made from.
        var source: String { "\(id):\(title):\(symbol ?? ""):\(label ?? ""):\(disabled):\(prominent):\(badge?.source ?? ""):\(menu ?? ""):\(tint ?? [])" }
    }
    let header: NodeView
    let title: String
    let level: Int
    let leading: [Item], trailing: [Item]
    /// The header's `input type="search"`, if it has one: the item's
    /// search controller (LLP 1075.003 §9.6).
    let search: NodeView?
    /// The header's tablist of text tabs, if it has one: the item's title
    /// view, a segmented control (LLP 1075.003 §9.8).
    let segments: NodeView?
    /// What the heading's group holds besides it (an avatar, a subtitle) and
    /// whether pressing it does something: the item's title, richer (§9.10).
    let group: HeaderTitle?

    /// `back` names the stack's Back control, left to UIKit's back button
    /// when there is one (`backIsUIKits`): the root of a presented stack
    /// has none, so there it is an item like the others.
    init?(route: NodeView, back: String?, backIsUIKits: Bool = true) {
        guard let header = route.container.subviews.lazy.compactMap({ $0 as? NodeView }).first,
              header.props["semanticTag"] == "header" else { return nil }
        var headings: [NodeView] = [], before: [Item] = [], after: [Item] = [], search: NodeView?, segments: NodeView?
        var tap: NodeView?, items: [NodeView] = []
        func walk(_ node: NodeView) {
            for case let child as NodeView in node.container.subviews {
                if child.isParagraph, child.props["accessibilityHeadingLevel"] != nil {
                    headings.append(child)
                } else if child.kind == "input", child.props["type"] == "search" {
                    search = search ?? child
                } else if child.props["accessibilityRole"] == "tablist" {
                    // Its tabs press, but they are the title view's
                    // segments, never bar items.
                    segments = segments ?? child
                } else if child.handlers.contains("press") || (child.isButton && child.props["popovertarget"] != nil && child.props["popovertargetaction"] != "hide") {
                    // A pressable heading's group is the title, tapped (§9.10).
                    if tap == nil, child.handlers.contains("press"), HeaderTitle.holdsHeading(child) { tap = child; walk(child); continue }
                    items.append(child)
                    guard !backIsUIKits || back == nil || child.props["id"] != back else { continue }
                    if headings.isEmpty { before.append(Item(child)) } else { after.append(Item(child)) }
                } else {
                    walk(child)
                }
            }
        }
        walk(header)
        guard headings.count == 1 else { return nil }
        self.header = header
        title = headings[0].accessibleText
        level = Int(headings[0].props["accessibilityHeadingLevel"] ?? "") ?? 2
        leading = before
        trailing = after
        self.search = search
        self.segments = segments
        group = HeaderTitle(header: header, heading: headings[0], tap: tap, apart: items + [search, segments].compactMap { $0 })
    }

    static func == (a: HeaderShape, b: HeaderShape) -> Bool {
        a.header === b.header && a.title == b.title && a.level == b.level && a.leading == b.leading && a.trailing == b.trailing && a.search === b.search && a.segments === b.segments && a.group == b.group
    }
}

/// A bar item's face drawn from a button whose one child is a filled box
/// holding a text or a symbol (an avatar, a badge): CSS's colours and
/// corners at the bar's image size, light and dark, as UIKit draws a
/// raster item image (`.alwaysOriginal`).
struct BadgeFace: Equatable {
    /// A bar item's image size; a title's avatar takes the author's.
    static let size: CGFloat = 36
    let size: CGFloat
    let text: String, symbol: String?
    let light: [[Double]], dark: [[Double]]
    let corners: [CGSize]
    init?(_ button: NodeView) {
        let kids = button.container.subviews.compactMap { $0 as? NodeView }
        guard kids.count == 1, let box = kids.first else { return nil }
        self.init(box: box)
    }
    /// The face of a filled box holding a text or a symbol (a title's
    /// avatar, §9.10, too). `authored`: draw it at the size the author gave
    /// the box (`faceBoxSize`, the authored points a replaced header has no frame for), between 20
    /// and 44, as a title's avatar is; otherwise at a bar item's 36.
    init?(box: NodeView, authored: Bool = false) {
        guard box.channels("background_color") != nil else { return nil }
        var text = "", symbol: String?, ink: NodeView?
        func walk(_ node: NodeView) {
            for case let child as NodeView in node.container.subviews {
                if child.kind == "image", let name = child.props["symbolName"], !name.isEmpty { symbol = symbol ?? name; ink = ink ?? child }
                else if child.isParagraph, text.isEmpty { text = child.accessibleText; ink = ink ?? child }
                else { walk(child) }
            }
        }
        walk(box)
        guard !text.isEmpty || symbol != nil else { return nil }
        let key = symbol == nil ? "text_color" : "tint_color"
        func colours(_ dark: Bool) -> [[Double]] {
            [box.channels("background_color", dark: dark) ?? [0, 0, 0, 0], ink?.channels(key, dark: dark) ?? (dark ? [1, 1, 1, 1] : [0, 0, 0, 1])]
        }
        let given = authored ? box.props["faceBoxSize"].flatMap { s -> CGFloat? in
            let parts = s.split(separator: "x").compactMap { Double($0) }
            return parts.count == 2 ? CGFloat(min(parts[0], parts[1])) : nil
        } : nil
        size = min(max(given ?? Self.size, 20), 44)
        self.text = text; self.symbol = symbol
        light = colours(false); dark = colours(true)
        corners = box.cornerSizes(in: CGRect(x: 0, y: 0, width: size, height: size))
    }
    // Joined by hand: interpolating an array goes through reflection, and
    // every batch builds each route's source.
    var source: String {
        let rgba = { (c: [[Double]]) in c.map { $0.map { String($0) }.joined(separator: ",") }.joined(separator: ";") }
        return "\(text)|\(symbol ?? "")|\(rgba(light))|\(rgba(dark))|\(corners.map { "\($0.width)x\($0.height)" }.joined(separator: ","))|\(size)"
    }

    var image: UIImage {
        let light = draw(self.light).withRenderingMode(.alwaysOriginal)
        light.imageAsset?.register(draw(self.dark).withRenderingMode(.alwaysOriginal), with: UITraitCollection(userInterfaceStyle: .dark))
        return light
    }
    private func draw(_ c: [[Double]]) -> UIImage {
        let rect = CGRect(x: 0, y: 0, width: size, height: size)
        return UIGraphicsImageRenderer(size: rect.size).image { _ in
            TextEngine.color(c[0]).setFill()
            UIBezierPath(cgPath: BorderPaint.roundedRect(rect, corners, shape: nil)).fill()
            let ink = TextEngine.color(c[1])
            if let symbol, let glyph = UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: size * 0.42))?.withTintColor(ink, renderingMode: .alwaysOriginal) {
                glyph.draw(at: CGPoint(x: (rect.width - glyph.size.width) / 2, y: (rect.height - glyph.size.height) / 2))
            } else {
                let attrs: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: size * 0.42, weight: .medium), .foregroundColor: ink]
                let s = (text as NSString).size(withAttributes: attrs)
                (text as NSString).draw(at: CGPoint(x: (rect.width - s.width) / 2, y: (rect.height - s.height) / 2), withAttributes: attrs)
            }
        }
    }
}

/// The delegate Exact gives each navigation controller it builds: Exact's
/// own handling first (pop reconciliation, LLP 1035.001 D1, D2), then the
/// app's delegate, which receives straight anything Exact does not
/// implement — a custom transition's animator (LLP 1075.003 §3.5).
final class NavigationDelegateProxy: NSObject, UINavigationControllerDelegate {
    weak var host: NavigationHost?
    weak var app: UINavigationControllerDelegate?

    func navigationController(_ nav: UINavigationController, willShow controller: UIViewController, animated: Bool) {
        // The bar as the route coming into view wants it, alongside UIKit's
        // transition: a cancelled pop calls this again for the source (§9.10).
        host?.showBar(nav, for: controller as? RouteController, animated: animated)
        host?.navigationController(nav, willShow: controller, animated: animated)
        app?.navigationController?(nav, willShow: controller, animated: animated)
    }

    func navigationController(_ nav: UINavigationController, didShow controller: UIViewController, animated: Bool) {
        // Restoring a hidden bar changes the route's safe area. Lay out its
        // container before reporting that cover to the kernel.
        nav.view.setNeedsLayout()
        nav.view.layoutIfNeeded()
        host?.navigationController(nav, didShow: controller, animated: animated)
        app?.navigationController?(nav, didShow: controller, animated: animated)
    }

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (app?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        app?.responds(to: selector) == true ? app : nil
    }
}

/// A header's search field as UIKit's search controller (LLP 1075.003
/// §9.6): text, focus and blur go to the authored field's handlers.
final class HeaderSearch: NSObject, UISearchResultsUpdating, UISearchBarDelegate, UISearchControllerDelegate {
    let controller = UISearchController(searchResultsController: nil)
    weak var host: NavigationHost?
    weak var field: NodeView?
    private var last: String?
    init(host: NavigationHost) {
        self.host = host
        super.init()
        controller.searchResultsUpdater = self
        controller.searchBar.delegate = self
        controller.delegate = self
        controller.obscuresBackgroundDuringPresentation = false
        controller.hidesNavigationBarDuringPresentation = true
    }
    func updateSearchResults(for search: UISearchController) {
        let text = search.searchBar.text ?? ""
        guard text != last, let field, let presenter = host?.presenter, presenter.views[field.id] === field else { return }
        last = text
        presenter.typed(field.id, text, input: field.handlers.contains("input"))
    }
    func searchBarTextDidBeginEditing(_ bar: UISearchBar) {
        guard let field, field.handlers.contains("focus") else { return }
        host?.presenter.focus(field.id)
    }
    func searchBarTextDidEndEditing(_ bar: UISearchBar) {
        guard let field else { return }
        host?.presenter.commitEdit(field.id, bar.text ?? "", change: field.handlers.contains("change"))
        if field.handlers.contains("blur") { host?.presenter.blur(field.id) }
    }
}

/// The header's tablist as a route's title view (§9.8), kept beside the
/// controller rather than in it.
final class TitleSegments: NSObject {
    let control = UISegmentedControl()
    /// What it was last sized for: its titles and the traits that size text.
    private(set) var sized: (titles: [String], traits: [AnyHashable])?
    let press: SegmentPress
    init(host: NavigationHost) {
        press = SegmentPress(host: host)
        super.init()
        control.addTarget(press, action: #selector(SegmentPress.changed(_:)), for: .valueChanged)
        // Sized again when text size or weight changes, no batch needed.
        MainActor.assumeIsolated {
            control.registerForTraitChanges([UITraitPreferredContentSizeCategory.self, UITraitLegibilityWeight.self]) { [weak self] (_: UISegmentedControl, _: UITraitCollection) in
                if let titles = self?.sized?.titles { self?.fit(titles) }
            }
        }
    }
    /// Sized for `titles` unless it already is: sizing lays it out, and
    /// every batch projects every tab's routes.
    func fit(_ titles: [String]) {
        let t = control.traitCollection
        let traits: [AnyHashable] = [t.preferredContentSizeCategory, t.legibilityWeight.rawValue]
        guard sized?.titles != titles || sized?.traits != traits else { return }
        sized = (titles, traits)
        control.sizeToFit()
        control.frame.size.width = max(control.frame.width, CGFloat(titles.count) * 90)
    }
}
private var titleSegmentsKey: UInt8 = 0
extension RouteController {
    var titleSegments: TitleSegments? {
        get { objc_getAssociatedObject(self, &titleSegmentsKey) as? TitleSegments }
        set { objc_setAssociatedObject(self, &titleSegmentsKey, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }
}

/// A title-view segment's tap presses its authored tab.
final class SegmentPress: NSObject {
    weak var host: NavigationHost?
    var tabs: [UInt32] = []
    /// The selection last reported or applied (`ExactSegmentedControl.settled`).
    var settled = UISegmentedControl.noSegment
    init(host: NavigationHost) { self.host = host }
    @objc func changed(_ control: UISegmentedControl) {
        let i = control.selectedSegmentIndex
        settled = i
        guard tabs.indices.contains(i) else { return }
        _ = host?.act(tabs[i], 0)
    }
}

/// What a projected bar item presses: its authored button, as a tap would.
final class BarPress: NSObject {
    let id: UInt32
    weak var host: NavigationHost?
    init(_ id: UInt32, _ host: NavigationHost) { self.id = id; self.host = host }
    @objc func press() { _ = host?.act(id, 0) }
}

/// One stack Exact built: its delegate, whether it shows the bar (LLP 1037
/// F1: for its whole life), and what the development check last saw.
final class NavigationStack {
    let proxy = NavigationDelegateProxy()
    var showsBar: Bool
    let order: Int
    let label: String
    var hatched = false
    var written: [ObjectIdentifier] = []
    init(showsBar: Bool, order: Int) {
        self.showsBar = showsBar
        self.order = order
        label = "#\(order)"
    }
}

extension NavigationHost {
    /// Whether a stack's bar shows: the stack's choice, except under the
    /// agent's own chrome, where the authored header paints (LLP 1075.003
    /// §3.4).
    func barShows(_ nav: UINavigationController) -> Bool {
        !ExactEnv.authoredChrome && stacks[ObjectIdentifier(nav)]?.showsBar == true
    }

    /// The agent's tap on an authored control a native bar stands for, under
    /// `--chrome platform`: a tab the tab bar shows, or a control in the
    /// header a shown bar replaces (its items, its back button). Pressed as
    /// its item presses it (host activation: the agent names the node, UIKit
    /// owns its pixels); nil when no bar shows `node`.
    func activateChrome(_ node: NodeView) -> [String: Any]? {
        guard !ExactEnv.authoredChrome else { return nil }
        let id = Int(node.id)
        if tabBarShows, let root = container, let tabs = NavigationTabs.of(root, presenter), tabs.tabs.contains(where: { $0 === node }) {
            guard !node.disabled else { return ["error": "tab #\(id) is disabled"] }
            presenter.press(node.id)
            return ["tapped": id, "pressed": id, "delivery": "host-activation", "native": "tab-bar-item"]
        }
        let shown = controllers.values.filter { c in
            guard let nav = c.navigationController, nav.topViewController === c, barShows(nav) else { return false }
            return c.viewIfLoaded?.window != nil
        }
        guard shown.contains(where: { c in
            c.barPresses.contains { $0.id == node.id } || c.lifted.map { node === $0 || node.isDescendant(of: $0) } == true
        }) else { return nil }
        guard node.handlers.contains("press"), !node.disabled else { return ["error": "bar item #\(id) is disabled or presses nothing"] }
        presenter.press(node.id)
        return ["tapped": id, "pressed": id, "delivery": "host-activation", "native": "bar-button-item"]
    }

    /// A navigation controller for a stack whose first route is `first`:
    /// Exact's delegate, the bar's visibility from the plan, then the
    /// `navigation` hatch (LLP 1075.003 Q3 (c)). Large titles are on, and each
    /// route's item says whether its title is large, so a level-1 heading
    /// pushed over an inline one is large; `prefersLargeTitles` is the app's
    /// from the hatch on (§3.5).
    func makeNavigation(first: NodeView?) -> UINavigationController {
        let nav = UINavigationController()
        let shape = first.flatMap { HeaderShape(route: $0, back: container?.props["navigationBack"]) }
        stackCount += 1
        let stack = NavigationStack(showsBar: shape != nil, order: stackCount)
        stack.proxy.host = self
        stacks[ObjectIdentifier(nav)] = stack
        nav.delegate = stack.proxy
        #if !os(tvOS)
        nav.navigationBar.prefersLargeTitles = true
        #endif
        if presenter.session?.natives.hatchesConnected == true {
            stack.showsBar = presenter.session?.natives.navigationHatch(nav, built: true, showsBar: stack.showsBar, label: stack.label) ?? stack.showsBar
            stack.hatched = true
        }
        nav.setNavigationBarHidden(!barShows(nav), animated: false)
        return nav
    }

    /// The `navigation` hatch for a stack built before the module connected
    /// (a cold launch's), before any of its routes' hatches run. A changed
    /// `showsBar` moves the content once, journaled (LLP 1075.003 Q3 (c)).
    func hatchNavigation(_ nav: UINavigationController) {
        guard let natives = presenter.session?.natives, natives.hatchesConnected,
              let stack = stacks[ObjectIdentifier(nav)], !stack.hatched else { return }
        let before = barShows(nav)
        stack.showsBar = natives.navigationHatch(nav, built: true, showsBar: stack.showsBar, label: stack.label)
        stack.hatched = true
        guard barShows(nav) != before else { return }
        showBar(nav, animated: false)
        for case let c as RouteController in nav.viewControllers { c.projectedSource = nil }
        presenter.session?.log("hatch navigation \(stack.label): showsBar changed after the first frame; the content moves once")
    }

    /// The navigation controller holding `controller`, among Exact's.
    func stackController(of controller: UIViewController) -> UINavigationController? {
        allNavigations.first { $0.viewControllers.contains(controller) }
    }

    /// Project each route of a stack into its navigation item when its
    /// authored source changed, then run the route hatch — before UIKit lays
    /// the stack out (LLP 1075.003 §3.2, §3.5 "projected defaults").
    func prepareRoutes(_ routes: [RouteController], in nav: UINavigationController) {
        hatchNavigation(nav)
        #if os(iOS)
        presenter.menus.focus.watch(nav.navigationBar) // a bar item's menu (MenuFocusIOS), every stack, rebuilt or not
        #endif
        projectBack(routes, in: nav)
        followTablist(routes, in: nav)
        for (index, c) in routes.enumerated() {
            let shows = barShows(nav)
            let back = container?.props["navigationBack"]
            // A route with no header, or a hidden one, has no bar (§9.10).
            let shape = shows ? HeaderShape(route: c.node, back: back, backIsUIKits: index > 0).flatMap(HeaderShape.shown) : nil
            let canGoBack = index > 0 && canInvokeBack
            let scroll = contentScroll(of: c)
            let dataset = c.node.props["dataset"]
            let source = BarSource(header: shape?.header.id, title: shape?.title ?? "", level: shape?.level ?? 0, leading: shape?.leading.map(\.source) ?? [],
                                   trailing: shape?.trailing.map(\.source) ?? [], group: shape?.group?.source ?? "", canGoBack: canGoBack)
            // The hatch runs again after anything Exact wrote to the item (the
            // Back control the route above gives it, too) and when the route
            // moves to another stack (a root whose tabs changed).
            let signature = HatchSource(bar: source, back: c.backSource ?? "", navigation: ObjectIdentifier(nav), dataset: dataset ?? "",
                                       scroll: scroll.map(ObjectIdentifier.init))
            if c.projectedSource != source {
                c.projectedSource = source
                project(shape, into: c, canGoBack: canGoBack, shows: shows)
            }
            collapse(c, shape: shape, scroll: scroll)
            if shows { searchField(shape?.search, in: c); segmentedTitle(shape?.segments, in: c); richTitle(shape, in: c) }
            guard c.projected != signature || !c.hatched else { continue }
            c.projected = signature
            guard presenter.session?.natives.hatchesConnected == true else { continue }
            presenter.session?.natives.routeHatch(c.hatched ? .changed : .built, controller: c, navigation: nav, scroll: scroll,
                                                 key: c.key, dataset: dataset)
            c.hatched = true
            if let scroll { c.ownedScroll = element(named: c.node.props["navigationScroll"] ?? "", in: c.node).flatMap { $0.scroll === scroll ? $0 : nil } }
        }
        // A route's header shown or hidden in place; a push or pop sets the
        // bar in `willShow` instead, with UIKit's transition.
        if nav.transitionCoordinator == nil, let top = nav.topViewController as? RouteController, routes.contains(top) {
            showBar(nav, animated: nav.view.window != nil)
        }
    }

    /// Write the defaults an authored header gives a navigation item, and
    /// lift the header out of the route's layout while the bar shows it.
    private func project(_ shape: HeaderShape?, into c: RouteController, canGoBack: Bool, shows: Bool) {
        let item = c.navigationItem
        if let lifted = c.lifted, lifted !== shape?.header {
            lifted.isHidden = false
            c.lifted = nil
        }
        guard shows else { return }
        item.title = shape?.title
        // tvOS has no large titles or back button.
        #if !os(tvOS)
        item.largeTitleDisplayMode = shape?.level == 1 ? .always : .never
        item.hidesBackButton = !canGoBack
        item.leftItemsSupplementBackButton = true
        #endif
        c.barPresses = []
        item.leftBarButtonItems = shape?.leading.map { barItem($0, c) }
        item.rightBarButtonItems = shape?.trailing.reversed().map { barItem($0, c) }
        if let header = shape?.header {
            header.isHidden = true
            c.lifted = header
        }
    }

    /// A header's tablist of text tabs is the item's title view: a segmented
    /// control whose selection is the tabs' `aria-selected`, a tap on a
    /// segment pressing its tab (§9.8), as the content's segmented control
    /// does (LLP 1035.001 D10). The heading stays the item's title, which
    /// the back button on the next route reads.
    private func segmentedTitle(_ list: NodeView?, in c: RouteController) {
        let tabs = list?.container.subviews.compactMap { $0 as? NodeView }.filter {
            $0.isButton && $0.props["accessibilityRole"] == "tab" && $0.handlers.contains("press")
        } ?? []
        guard !tabs.isEmpty else {
            if let old = c.titleSegments?.control, c.navigationItem.titleView === old { c.navigationItem.titleView = nil }
            c.titleSegments = nil
            return
        }
        let segments = c.titleSegments ?? TitleSegments(host: self)
        c.titleSegments = segments
        let control = segments.control
        segments.press.tabs = tabs.map(\.id)
        let titles = tabs.map(\.accessibleName)
        if control.numberOfSegments != titles.count {
            control.removeAllSegments()
            segments.press.settled = UISegmentedControl.noSegment
            for (i, t) in titles.enumerated() { control.insertSegment(withTitle: t, at: i, animated: false) }
        } else {
            for (i, t) in titles.enumerated() where control.titleForSegment(at: i) != t { control.setTitle(t, forSegmentAt: i) }
        }
        let selected = tabs.firstIndex { $0.props["accessibilitySelected"] == "true" } ?? UISegmentedControl.noSegment
        // A finger's choice not yet reported stays, as in the content's segments (`SegmentHost.sync`).
        let pending = control.isTracking || control.selectedSegmentIndex != segments.press.settled
        if control.selectedSegmentIndex != selected, !pending { control.selectedSegmentIndex = selected }
        if control.selectedSegmentIndex == selected { segments.press.settled = selected }
        control.accessibilityIdentifier = list?.props["testId"]
        segments.fit(titles)
        if c.navigationItem.titleView !== control { c.navigationItem.titleView = control }
    }

    /// A header's search field is the item's `UISearchController` (§9.6):
    /// its placeholder and value are the field's; what the reader types is
    /// the field's `input` (and `focus`, `blur`), as typing in it would be.
    private func searchField(_ field: NodeView?, in c: RouteController) {
        // tvOS navigation items carry no search controller.
        #if !os(tvOS)
        guard let field else {
            if c.search != nil { c.navigationItem.searchController = nil; c.search = nil }
            return
        }
        let search = c.search ?? HeaderSearch(host: self)
        if c.search !== search {
            c.search = search
            c.navigationItem.searchController = search.controller
            c.navigationItem.hidesSearchBarWhenScrolling = false
            if #available(iOS 16.0, *) { c.navigationItem.preferredSearchBarPlacement = .stacked }
            c.definesPresentationContext = true
        }
        search.field = field
        let bar = search.controller.searchBar
        let placeholder = field.props["placeholder"] ?? ""
        if bar.placeholder != placeholder { bar.placeholder = placeholder }
        // The Contract's value wins unless the reader is typing it.
        let value = field.props["value"] ?? ""
        if !bar.isFirstResponder, bar.text != value { bar.text = value }
        bar.accessibilityIdentifier = field.props["testId"]
        #endif
    }

    /// UIKit's back button stands for each route's authored Back control,
    /// shaped as it is (LLP 1075.003, James's review): a symbol alone is the
    /// chevron alone (`.minimal`); text is the button's title. UIKit reads
    /// both from the item beneath, which is where they are written, on change.
    private func projectBack(_ routes: [RouteController], in nav: UINavigationController) {
        guard barShows(nav), routes.count > 1 else { return }
        let name = container?.props["navigationBack"]
        for index in 1..<routes.count {
            let control = name.flatMap { element(named: $0, in: routes[index].node) }
            let shape = control.map { HeaderShape.Item($0) }
            let source = shape.map { "\($0.symbol ?? "")|\($0.title)" } ?? "-"
            let below = routes[index - 1]
            guard below.backSource != source else { continue }
            below.backSource = source
            // tvOS has no back button.
            #if !os(tvOS)
            let item = below.navigationItem
            switch shape {
            case let s? where s.title.isEmpty: item.backButtonDisplayMode = .minimal; item.backButtonTitle = nil
            case let s?: item.backButtonDisplayMode = .default; item.backButtonTitle = s.title
            case nil: item.backButtonDisplayMode = .default; item.backButtonTitle = nil
            }
            #endif
        }
    }

    /// Stage 3 (LLP 1075.003 §3.7): a large title collapses with its route's
    /// content scroll view when that scroller comes right after the header
    /// the bar replaces. The scroller then goes under the bar, UIKit insets it
    /// and follows its offset, and CSS `scrollTop` is measured from the
    /// expanded title's inset (`scrollOrigin`), so an authored offset lands
    /// where the browser's does while the bar's height changes. An inline
    /// title's scroller goes under the bar the same way (§9.10), so the bar's
    /// scroll edge appearance follows it; its inset does not change.
    private func collapse(_ c: RouteController, shape: HeaderShape?, scroll: UIScrollView?) {
        let kids = c.node.container.subviews.compactMap { $0 as? NodeView }
        let node = kids.firstIndex { $0 === shape?.header }.flatMap { kids.indices.contains($0 + 1) ? kids[$0 + 1] : nil }
        let target = node?.scroll != nil && node?.scroll === scroll ? node : nil
        guard c.collapseScroll !== target else { return }
        if let old = c.collapseScroll, let sv = old.scroll {
            let top = sv.contentOffset.y + old.scrollTopInset(sv)
            sv.contentInsetAdjustmentBehavior = .never
            old.scrollOrigin = 0
            old.scrollCollapsed = 0
            sv.contentOffset.y = top
        }
        c.collapseScroll = target
        if let sv = target?.scroll { sv.contentInsetAdjustmentBehavior = .always }
        c.setContentScrollView(target?.scroll, for: .top)
        let under = shape?.level == 1 ? "collapses its title with its scroller" : "scrolls its content under the bar"
        presenter.session?.log("navigation: route \(c.key) \(target == nil ? "keeps its title still" : under)")
    }

    private func barItem(_ i: HeaderShape.Item, _ c: RouteController) -> UIBarButtonItem {
        let press = BarPress(i.id, self)
        c.barPresses.append(press)
        let action = #selector(BarPress.press)
        let image = i.badge?.image ?? i.symbol.flatMap { UIImage(systemName: $0) }
        let item: UIBarButtonItem
        if let name = i.menu {
            // A pull-down: UIKit opens it on tap; the rows are the popover's,
            // read as it opens, after the button's own press (both fire, as
            // LLP 1021 D1 has it on the web).
            let id = i.id
            let deferred = UIDeferredMenuElement.uncached { [weak self] completion in
                guard let self else { return completion([]) }
                if self.presenter.views[id]?.handlers.contains("press") == true { self.presenter.press(id) }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { [weak self] in
                    guard let self, let pop = self.presenter.carrying("popover").first(where: { $0.props["id"] == name }) else { return completion([]) }
                    completion(self.presenter.menus.items(of: pop))
                }
            }
            item = image.map { UIBarButtonItem(image: $0, menu: UIMenu(children: [deferred])) }
                ?? UIBarButtonItem(title: i.title, menu: UIMenu(children: [deferred]))
        } else {
            item = image.map { UIBarButtonItem(image: $0, style: .plain, target: press, action: action) }
                ?? UIBarButtonItem(title: i.title, style: .plain, target: press, action: action)
        }
        item.accessibilityLabel = i.label ?? (i.title.isEmpty ? nil : i.title)
        item.isEnabled = !i.disabled
        // The authored `color` of a plain item's face; a prominent item's
        // tint is its fill, which `color` is not.
        if let tint = i.tint, !i.prominent, i.badge == nil {
            item.tintColor = UIColor { TextEngine.color(tint[$0.userInterfaceStyle == .dark ? 1 : 0]) }
        }
        if #available(iOS 26.0, tvOS 26.0, *) {
            if i.prominent { item.style = .prominent }
            // A drawn face is its own shape: no glass capsule around it.
            #if !os(tvOS)
            if i.badge != nil { item.hidesSharedBackground = true }
            #endif
        }
        return item
    }

    /// The scroll view a route names with `navigationScroll`, resolved
    /// inside it by HTML id as its Back control is (E7: nothing searches).
    func contentScroll(of c: RouteController) -> UIScrollView? {
        guard let name = c.node.props["navigationScroll"], !name.isEmpty else { return nil }
        return element(named: name, in: c.node)?.scroll
    }

    private func element(named name: String, in route: NodeView) -> NodeView? {
        presenter.carrying("id").first { $0.props["id"] == name && ($0 === route || $0.isDescendant(of: route)) }
    }

    /// The live node a route (by key) holds under an HTML id: a hatch's
    /// `route.element(id)` (LLP 1075.003 §3.4).
    func resolve(route key: String, id: String) -> NodeView? {
        guard let route = controllers.values.first(where: { $0.key == key && presenter.views[$0.node.id] === $0.node })?.node else { return nil }
        return element(named: id, in: route)
    }

    /// A hatch's act on an authored element, as the DOM's: `click()` presses
    /// it as a tap does, `focus()` and `blur()` follow the focus rules. Each
    /// is queued (`ElementHatches.later`) and runs on a later turn. False when
    /// refused.
    func act(_ id: UInt32, _ action: UInt32) -> Bool {
        guard let node = presenter.views[id] else { return false }
        switch action {
        case 0:
            guard node.handlers.contains("press"), !node.disabled else { return false }
            // Still the node it was when asked: a reload restarts node ids.
            return presenter.elements.later(id, "click") { [weak presenter = self.presenter, weak node] in
                if let presenter, let node, presenter.views[id] === node { presenter.session?.log("hatch \(node.props["hatch"].map { "element \($0) #\(id)" } ?? "node #\(id)"): click (delivery: hatch)"); presenter.press(id) }
            }
        case 1:
            return presenter.elements.later(id, "focus") { [weak presenter = self.presenter, weak node] in
                if let presenter, let node, presenter.views[id] === node { presenter.focusNode(node) }
            }
        case 2:
            return presenter.elements.later(id, "blur") { [weak presenter = self.presenter, weak node] in
                if let presenter, let node, presenter.views[id] === node { _ = (node.textArea ?? node.field ?? node).resignFirstResponder() }
            }
        default: return false
        }
        return true
    }

    /// The app's delegate for a controller whose own slot Exact keeps.
    /// UIKit reads which methods a delegate answers when it is set, so the
    /// proxy is set again.
    func setAppDelegate(_ controller: AnyObject, _ delegate: AnyObject?) {
        if let tabs = controller as? UITabBarController, tabs === tabController {
            tabProxy.app = delegate as? UITabBarControllerDelegate
            tabs.delegate = nil
            tabs.delegate = tabProxy
            presenter.session?.log("hatch tabs: delegate \(delegate.map { "\(type(of: $0))" } ?? "cleared")")
            return
        }
        guard let nav = controller as? UINavigationController, let stack = stacks[ObjectIdentifier(nav)] else { return }
        stack.proxy.app = delegate as? UINavigationControllerDelegate
        nav.delegate = nil
        nav.delegate = stack.proxy
        presenter.session?.log("hatch navigation \(stack.label): delegate \(delegate.map { "\(type(of: $0))" } ?? "cleared")")
    }

    // MARK: The content area (LLP 1075.003 §3.5)

    /// Whether a native container shows its bars, so the session's view
    /// takes the whole of its own (ExactViewIOS `fit`).
    var wantsWholeView: Bool {
        (tabOwner != nil && !ExactEnv.authoredChrome) || allNavigations.contains(where: barShows)
    }

    /// `wantsWholeView` for the root's containers alone, whatever a sheet
    /// shows: what the window's own viewport is, which the viewport units
    /// resolve against (LLP 1075.003 §9.11; Astra's review).
    var rootWantsWholeView: Bool {
        (tabOwner != nil && !ExactEnv.authoredChrome)
            || allNavigations.contains { nav in !presentedNavigations.contains { $0 === nav } && barShows(nav) }
    }

    /// What Exact's containers cover of each route — its controller's safe
    /// area past the insets the kernel already has as `env()`: a shown bar's
    /// top, and every edge while the containers take the whole view for a
    /// page that did not ask to cover it — and the header a bar replaces,
    /// reported when they change, after the batch being applied.
    func reportCovers() {
        var changes: [(UInt32, HostCover?)] = []
        var wanted: [UInt32: HostCover] = [:]
        let whole = wantsWholeView && presenter.viewportFit != "cover"
        for c in controllers.values where presenter.views[c.node.id] === c.node {
            guard let nav = c.navigationController, nav.viewControllers.contains(c) else { continue }
            let shows = barShows(nav)
            guard shows || whole else { continue }
            if let header = c.lifted { wanted[header.id] = .whole }
            let edges: HostCover.Edges
            // A collapsing title's scroller goes under the bar, which insets it;
            // its expanded inset is where CSS scrollTop 0 rests.
            // While the bar is not as the route wants it (mid-transition to or
            // from a route without one, §9.10), its last cover stands.
            // An active search's bar is UIKit's: the content follows it, as
            // UIKit's own does, but a title's insets are not sampled.
            let search = searching(c)
            let settled = search || nav.isNavigationBarHidden != routeShowsBar(c, in: nav)
            if settled, !search, let node = c.collapseScroll, let sv = node.scroll, sv.adjustedContentInset.top > 0 {
                let inset = sv.adjustedContentInset.top
                // tvOS has no large titles: every title is inline.
                #if os(tvOS)
                let inline = true
                #else
                let inline = c.navigationItem.largeTitleDisplayMode != .always
                #endif
                if inline {
                    // An inline title does not collapse: its inset is the one it has.
                    node.scrollOrigin = inset; node.scrollCollapsed = inset
                } else if nav.transitionCoordinator == nil {
                    // A large title's widest and narrowest, at rest only.
                    if inset > node.scrollOrigin { node.scrollOrigin = inset }
                    if node.scrollCollapsed == 0 || inset < node.scrollCollapsed { node.scrollCollapsed = inset }
                }
            }
            if settled, c.viewIfLoaded?.window != nil {
                let safe = c.view.safeAreaInsets, env = presenter.insets
                let top = c.collapseScroll == nil ? max(0, safe.top - env.top) : 0
                edges = .init(top: top, right: whole ? max(0, safe.right - env.right) : 0,
                              bottom: whole ? max(0, safe.bottom - env.bottom) : 0, left: whole ? max(0, safe.left - env.left) : 0)
            } else if case .edges(let e)? = covers[c.node.id] {
                edges = e
            } else { continue }
            if edges != .init(top: 0, right: 0, bottom: 0, left: 0) { wanted[c.node.id] = .edges(edges) }
        }
        // The tablist whose place the tab bar takes takes no room.
        if let list = adoptedTablist, tabOwner != nil, !ExactEnv.authoredChrome { wanted[list] = .whole }
        for (id, cover) in wanted where covers[id] != cover { changes.append((id, cover)) }
        for id in covers.keys where wanted[id] == nil && presenter.views[id] != nil { changes.append((id, nil)) }
        covers = wanted
        guard !changes.isEmpty else { return }
        // Under Q3 (c) a cold launch's first frame runs no app code: a hatch
        // that changes a bar the plan already showed moves the content once.
        if replayingHatches, changes.contains(where: { presenter.views[$0.0]?.props["navigationKey"] != nil }) {
            presenter.session?.log("hatch: the content area moved after the first frame, by a hatch run at launch")
        }
        // Never inside a batch: the stack is installed partway through the
        // first one, whose remaining frames would overwrite the covered ones.
        presenter.afterBatch { [weak presenter = self.presenter] in presenter?.onCovers?(changes) }
    }

    // MARK: The development check (LLP 1075.003 §3.5)

    /// Whether this build checks what Exact owns: every build but a
    /// production bake.
    static let checksOwnership: Bool = {
        let trust = ((BakedCompatibility.json["inputs"] as? [String: Any])?["trust"] as? String) ?? "development"
        return trust != "production"
    }()

    /// Record the controllers Exact set on a stack.
    func recordOwned(_ nav: UINavigationController) {
        guard NavigationHost.checksOwnership else { return }
        stacks[ObjectIdentifier(nav)]?.written = nav.viewControllers.map(ObjectIdentifier.init)
    }

    /// UIKit completed a transition (`didShow`): a pop the user made (UIKit's
    /// back button, the edge swipe) leaves a prefix of what Exact wrote, and
    /// is recorded; any other change is not Exact's and stays to be reported.
    func recordPop(_ nav: UINavigationController) {
        guard NavigationHost.checksOwnership, let stack = stacks[ObjectIdentifier(nav)] else { return }
        let now = nav.viewControllers.map(ObjectIdentifier.init)
        if now.count <= stack.written.count, Array(stack.written.prefix(now.count)) == now { stack.written = now }
    }

    /// Before each batch: compare what Exact owns on each hatched object with
    /// what Exact last wrote, and journal a difference once, by name. It is
    /// detection, not enforcement; a change made and undone between two
    /// batches is not seen.
    func checkOwned() {
        guard NavigationHost.checksOwnership, !changing, !syncing, presenter.session?.natives.hatchesConnected == true else { return }
        func say(_ what: String, _ property: String) {
            let line = "\(what): \(property) changed outside Exact, which owns it"
            if ownedReported.insert(line).inserted { presenter.session?.log(line) }
        }
        for nav in allNavigations {
            guard let stack = stacks[ObjectIdentifier(nav)], stack.hatched else { continue }
            if nav.delegate !== stack.proxy { say("navigation \(stack.label)", "delegate") }
            if nav.transitionCoordinator == nil, !searching(nav.topViewController as? RouteController), nav.isNavigationBarHidden == topShowsBar(nav) { say("navigation \(stack.label)", "navigation bar visibility") }
            if nav.viewControllers.map(ObjectIdentifier.init) != stack.written { say("navigation \(stack.label)", "viewControllers") }
            #if !os(tvOS)
            if let pop = nav.interactivePopGestureRecognizer, pop.delegate !== self { say("navigation \(stack.label)", "the pop gesture's delegate") }
            if #available(iOS 26.0, tvOS 26.0, *), let pop = nav.interactiveContentPopGestureRecognizer, pop.delegate !== self {
                say("navigation \(stack.label)", "the content pop gesture's delegate")
            }
            #endif
        }
        for c in controllers.values where c.hatched && presenter.views[c.node.id] === c.node {
            if c.navigationController != nil, c.isViewLoaded, c.node.superview !== c.view { say("route \(c.key)", "view") }
            guard let node = c.ownedScroll, let scroll = node.scroll else { continue }
            for property in NavigationHost.ownedChanges(scroll, of: node, collapsing: c.collapseScroll === node) { say("route \(c.key)", property) }
        }
    }

    /// What Exact sets on a node's scroll view and nothing else may (LLP
    /// 1075.003 §3.5) and differs from what Exact writes: no inset (a
    /// refresh control insets it while it spins), no automatic adjustment,
    /// the node as its delegate, the authored keyboard dismissal. The offset
    /// and the content size are Exact's too but are not checked: they move
    /// with the user and with layout, so no last write predicts them.
    static func ownedChanges(_ s: UIScrollView, of node: NodeView, collapsing: Bool) -> [String] {
        var out: [String] = []
        #if os(tvOS)
        if s.contentInset != .zero { out.append("contentInset") }
        #else
        if s.contentInset != .zero, s.refreshControl?.isRefreshing != true { out.append("contentInset") }
        #endif
        if s.contentInsetAdjustmentBehavior != (collapsing ? .always : .never) { out.append("contentInsetAdjustmentBehavior") }
        if s.delegate !== node { out.append("delegate") }
        let dismiss: UIScrollView.KeyboardDismissMode = switch node.props["keyboardDismissMode"] {
        case "interactive": .interactive
        case "on-drag": .onDrag
        default: .none
        }
        if s.keyboardDismissMode != dismiss { out.append("keyboardDismissMode") }
        return out
    }
}
#endif

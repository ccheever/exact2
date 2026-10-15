// @ref LLP 1075.003 §3.5, §3.7 — Exact's navigation bar on iOS: the
// header-shaped route's projection, the content area the bar leaves
// (`HostCover`), the delegate slot Exact keeps and forwards, the authored
// elements a hatch acts on, and the development check of what Exact owns.
// When the hatches run is NavigationIOS.swift's; the module side is
// ExactNativeModule.swift.
// @ref LLP 1115.000 D5.3 — whether the bar shows is each route's, never
// decided once for its stack (`routeShowsBar`, NavigationTitleIOS.swift).
#if os(iOS) || os(tvOS)
import UIKit

/// What a route's bar item is projected from: what it shows of the bar
/// (its header, the system Back alone, nothing) and that header's parts. A
/// change projects it again.
struct BarSource: Equatable {
    let bar: NavigationRules.Bar
    let authored: [UInt32], placed: [UInt32]
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
        let id: UInt32, title: String, symbol: String?, label: String?, disabled: Bool, accessibilityDisabled: Bool
        /// The button's `testId`: the item's `accessibilityIdentifier`, so
        /// XCUITest and Maestro address it as any other node (LLP 1116 D2).
        let testId: String?
        /// A face drawn from the button's one filled box (an avatar or a
        /// badge: a shape holding a text or a symbol), when the button has
        /// one: the item's image, in the box's own colours and corners.
        let badge: BadgeFace?
        /// Whether the button opens a popover (`Invocation.opensPopover`):
        /// the item's menu, its rows read when it opens, as LLP 1021 D3's
        /// pull-down.
        let menu: Bool
        /// A native button's prominent style: a prominent bar item (iOS 26),
        /// as LLP 1069.011.000 D2 maps `NSToolbarItem`.
        let prominent: Bool
        /// The colour the author gave the item's face, light and dark: a
        /// symbol's tint, else a `color` written on the button itself. Nil
        /// (UIKit's tint, the platform default) for a colour the button
        /// only inherits or the compiler implied.
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
            testId = button.props["testId"]
            badge = button.isNativeButton ? nil : BadgeFace(button)
            menu = button.presenter?.invocation(of: button)?.opensPopover == true
            self.symbol = symbol
            title = text
            label = button.props["accessibilityLabel"] ?? face?.label
            disabled = button.disabled
            accessibilityDisabled = button.ariaUnavailable
            prominent = ["filled", "bordered-prominent", "prominent-glass", "prominent-clear-glass"].contains(face?.style ?? "")
            // The platform's tint unless the author said otherwise (LLP
            // 1115.000 D2): a tint written on the symbol, else a `color`
            // written on the button, a platform colour as much as a literal.
            // One the button only inherits, or a sheet wrote, is unsaid.
            let symbolTint = ink?.1 == "tint_color" ? ink?.0.ownColor("tint_color") : nil
            let row = symbolTint ?? button.ownColor("text_color")
            if let row, let light = row.channels(dark: false), let dark = row.channels(dark: true) {
                tint = [light, dark]
            } else {
                tint = nil
            }
        }
        /// Everything a bar item is made from.
        var source: String { "\(id):\(title):\(symbol ?? ""):\(label ?? ""):\(disabled):\(accessibilityDisabled):\(prominent):\(badge?.source ?? ""):\(menu):\(tint ?? []):\(testId ?? "")" }
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
    /// The header's nodes the bar places (`presents`): the heading, every
    /// item's button (the Back control's too), the search field, the
    /// tablist when it has tabs for the segmented title, and what the title
    /// shows of the heading's group.
    let placed: [NodeView]
    /// Explicit opt-outs stay in the authored header with their descendants.
    let authored: [NodeView]
    let authoredBack: Bool

    /// `back` names the stack's Back control, left to UIKit's back button
    /// when there is one (`backIsUIKits`): the root of a presented stack
    /// has none, so there it is an item like the others.
    init?(route: NodeView, back: String?, backIsUIKits: Bool = true, projected: Bool = false) {
        // What is header-shaped is the kernel's rule, read over this route's views (`HeaderRule`, LLP 1115.001 D11).
        guard let rule = HeaderRule(route: route, back: back, backIsPlatforms: backIsUIKits, projected: projected) else { return nil }
        // Every role the header rule assigns can remain authored. In
        // particular the title's tap target is not one of its bar items.
        let roles = rule.items + [rule.heading, rule.tap, rule.search, rule.segments].compactMap { $0 }
        let authored = projected ? roles.filter { $0.wrote("appearance") && $0.style["appearance"]?.string == "none" } : []
        self.authored = authored
        let projects = { (node: NodeView) in !authored.contains { node === $0 || node.isDescendant(of: $0) } }
        authoredBack = back != nil && roles.contains { $0.props["id"] == back && !projects($0) }
        let heading = rule.heading, titleProjects = projects(heading)
        let search = rule.search.flatMap { projects($0) ? $0 : nil }
        let segments = rule.segments.flatMap { projects($0) ? $0 : nil }
        let tap = rule.tap.flatMap { projects($0) ? $0 : nil }
        header = rule.header
        title = titleProjects ? heading.accessibleText : ""
        level = titleProjects ? rule.level : 0
        leading = rule.leading.filter(projects).map { Item($0) }
        trailing = rule.trailing.filter(projects).map { Item($0) }
        self.search = search
        self.segments = segments
        let group = titleProjects ? HeaderTitle(header: header, heading: heading, tap: tap, apart: rule.items + [rule.search, rule.segments].compactMap { $0 }) : nil
        self.group = group
        let tablist = segments.flatMap { HeaderShape.titleTabs($0).isEmpty ? nil : $0 }
        // The title's pressable wrapper goes with the title it holds, unless
        // it holds an opted-out node, which stays where it is.
        let wrapper = titleProjects ? tap.flatMap { tap in authored.contains { $0.isDescendant(of: tap) } ? nil : tap } : nil
        placed = ([heading] + rule.items + [search, tablist, wrapper].compactMap { $0 } + (HeaderShape.presented(group, segments: segments)?.placed ?? [])).filter(projects)
    }

    /// The tabs of a header's tablist the segmented title shows (§9.8).
    static func titleTabs(_ list: NodeView) -> [NodeView] {
        list.container.subviews.compactMap { $0 as? NodeView }.filter {
            $0.isButton && $0.props["accessibilityRole"] == "tab" && $0.handlers.contains("press")
        }
    }

    static func == (a: HeaderShape, b: HeaderShape) -> Bool {
        a.header === b.header && a.authoredBack == b.authoredBack && a.title == b.title && a.level == b.level && a.leading == b.leading && a.trailing == b.trailing && a.search === b.search && a.segments === b.segments && a.group == b.group && a.authored.map(\.id) == b.authored.map(\.id)
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
/// own handling first, then the app's delegate, which receives straight
/// anything Exact does not implement — a custom transition's animator
/// (LLP 1075.003 §3.5). An animated push or pop is reconciled (LLP 1035.001
/// D1, D2) in its coordinator's completion, after both have heard `didShow`.
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
        if nav.viewIfLoaded?.window != nil { nav.view.layoutIfNeeded() }
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
final class HeaderSearch: NSObject, UISearchResultsUpdating, UISearchBarDelegate, UISearchControllerDelegate, UITextFieldDelegate {
    let controller = UISearchController(searchResultsController: nil)
    weak var host: NavigationHost?
    weak var field: NodeView? {
        didSet {
            if oldValue !== field { oldValue?.headerField = nil }
            #if os(iOS)
            field?.headerField = controller.searchBar.searchTextField
            controller.searchBar.searchTextField.pasteDelegate = field
            #endif
        }
    }
    private weak var textDelegate: UITextFieldDelegate?
    init(host: NavigationHost) {
        self.host = host
        super.init()
        controller.searchResultsUpdater = self
        controller.searchBar.delegate = self
        controller.delegate = self
        #if os(iOS)
        textDelegate = controller.searchBar.searchTextField.delegate
        controller.searchBar.searchTextField.delegate = self
        #endif
        controller.obscuresBackgroundDuringPresentation = false
        controller.hidesNavigationBarDuringPresentation = true
    }
    func updateSearchResults(for search: UISearchController) {
        let text = search.searchBar.text ?? ""
        guard let field, let presenter = host?.presenter, presenter.views[field.id] === field else { return }
        field.typedText(text)
    }
    func textField(_ textField: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String) -> Bool {
        guard field?.textField(textField, shouldChangeCharactersIn: range, replacementString: string) ?? true else { return false }
        return textDelegate?.textField?(textField, shouldChangeCharactersIn: range, replacementString: string) ?? true
    }
    func textFieldShouldClear(_ textField: UITextField) -> Bool {
        guard field?.textFieldShouldClear(textField) ?? true else { return false }
        return textDelegate?.textFieldShouldClear?(textField) ?? true
    }
    func textFieldDidChangeSelection(_ textField: UITextField) {
        textDelegate?.textFieldDidChangeSelection?(textField)
        if let field { host?.presenter.fieldSelections.changed(field) }
    }
    // UISearchBar remains the recipient of UIKit's focus, return and private callbacks.
    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (textDelegate?.responds(to: selector) ?? false)
    }
    override func forwardingTarget(for selector: Selector!) -> Any? {
        textDelegate?.responds(to: selector) == true ? textDelegate : nil
    }
    func searchBarTextDidBeginEditing(_ bar: UISearchBar) {
        guard let field else { return }
        host?.presenter.fieldSelections.focused(field)
        host?.presenter.focus(field.id)
    }
    /// The keyboard's Search key is this field's Return (LLP 1115.000 D9.2):
    /// its `submit`, then the keyboard goes away, as a hand-built search
    /// bar's delegate resigns it, and the search stays as typed.
    func searchBarSearchButtonClicked(_ bar: UISearchBar) {
        if let field, let presenter = host?.presenter, presenter.views[field.id] === field {
            presenter.commitEdit(field.id, bar.text ?? "", change: field.handlers.contains("change"))
            if field.handlers.contains("submit") { presenter.submit(field.id) }
        }
        bar.resignFirstResponder()
    }
    func searchBarTextDidEndEditing(_ bar: UISearchBar) {
        guard let field else { return }
        host?.presenter.commitEdit(field.id, bar.text ?? "", change: field.handlers.contains("change"))
        host?.presenter.blur(field.id)
    }
}

/// The header's tablist as a route's title view (§9.8), kept beside the
/// controller rather than in it.
final class TitleSegments: NSObject {
    let control = LabelledSegmentedControl()
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
    /// every batch projects every tab's routes. Its width is its own, the
    /// one UIKit fits it to (LLP 1115.000 D11.6); Exact held each segment to
    /// 90 points at least.
    func fit(_ titles: [String]) {
        let t = control.traitCollection
        let traits: [AnyHashable] = [t.preferredContentSizeCategory, t.legibilityWeight.rawValue]
        guard sized?.titles != titles || sized?.traits != traits else { return }
        sized = (titles, traits)
        control.sizeToFit()
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

/// One stack Exact built: its delegate, whether its routes may show the bar
/// (they may unless the app's `navigation` hatch says no; which of them
/// shows it is each route's, LLP 1115.000 D5.3), and what the development
/// check last saw.
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
    /// Whether a stack's routes may show the bar: they may, unless the app's
    /// hatch chose otherwise, and never under the agent's own chrome, where
    /// the authored header paints (LLP 1075.003 §3.4). Whether one route
    /// shows it is `routeShowsBar`.
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
        if tabBarShows, let root = container, let tabs = NavigationTabs.of(root, presenter), let index = tabs.tabs.firstIndex(where: { $0 === node }) {
            guard !node.disabled else { return ["error": "tab #\(id) is disabled"] }
            let root = reselectedRoot(index) // as the bar's own tap: the selected tab at its root scrolls to the top
            presenter.press(node.id)
            if let root { scrollToTop(root) }
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
    /// Exact's delegate, the `navigation` hatch (LLP 1075.003 Q3 (c)), then
    /// the bar as the first route has it. The stack's routes may show the
    /// bar whatever its first route is (LLP 1115.000 D5.3: a stack that
    /// opens on a route with no header still shows the bar for a later
    /// route that has one). Large titles are on, and each route's item says
    /// whether its title is large, so a level-1 heading pushed over an
    /// inline one is large; `prefersLargeTitles` is the app's from the
    /// hatch on (§3.5).
    func makeNavigation(first: NodeView?) -> UINavigationController {
        let nav = UINavigationController()
        stackCount += 1
        let stack = NavigationStack(showsBar: true, order: stackCount)
        stack.proxy.host = self
        stacks[ObjectIdentifier(nav)] = stack
        nav.delegate = stack.proxy
        #if !os(tvOS)
        nav.navigationBar.prefersLargeTitles = true
        #endif
        // The theme's label colour on the bar's titles (LLP 1095 D4); its
        // glyphs take the app's tint, which the theme's `AccentColor` sets.
        if let ink = ThemeRoles.color("label") ?? ThemeRoles.color("CanvasText") {
            nav.navigationBar.titleTextAttributes = [.foregroundColor: ink]
            #if !os(tvOS)
            nav.navigationBar.largeTitleTextAttributes = [.foregroundColor: ink]
            #endif
        }
        if presenter.session?.natives.hatchesConnected == true {
            stack.showsBar = presenter.session?.natives.navigationHatch(nav, built: true, showsBar: stack.showsBar, label: stack.label) ?? stack.showsBar
            stack.hatched = true
        }
        // Hidden for a first route that shows none, before UIKit shows it;
        // `showBar` follows every route from `willShow` on.
        let shows = barShows(nav) && first.map { bar(of: $0, pushed: false) != .hidden } ?? false
        nav.setNavigationBarHidden(!shows, animated: false)
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
        let stackShows = barShows(nav)
        let back = container?.props["navigationBack"]
        for (index, c) in routes.enumerated() {
            // The bar is each route's (LLP 1115.000 D5.2, D5.3): its header,
            // the system Back alone on a pushed route with none of its own
            // (no header, a hidden one, one that is not a bar's), or no bar
            // where the route says so or has nothing to go back to.
            c.pushed = index > 0
            let shape = stackShows ? HeaderShape.bar(of: c.node, back: back, backIsUIKits: index > 0) : nil
            let bar = stackShows ? self.bar(of: c.node, pushed: index > 0) : .hidden
            let shows = bar != .hidden
            let canGoBack = index > 0 && canInvokeBack && shape?.authoredBack != true
            let scroller = contentScroller(of: c), scroll = scroller?.scrollView
            let dataset = c.node.props["dataset"]
            let source = BarSource(bar: bar, authored: shape?.authored.map(\.id) ?? [], placed: shape?.placed.map(\.id) ?? [], header: shape?.header.id, title: shape?.title ?? "", level: shape?.level ?? 0, leading: shape?.leading.map(\.source) ?? [],
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
            collapse(c, shape: shape, shows: shows, scroller: scroller)
            reachBottom(c)
            c.paintBackdrop()
            searchField(shows ? shape?.search : nil, in: c)
            segmentedTitle(shows ? shape?.segments : nil, in: c)
            richTitle(shows ? shape : nil, in: c)
            guard c.projected != signature || !c.hatched else { continue }
            c.projected = signature
            guard presenter.session?.natives.hatchesConnected == true else { continue }
            presenter.session?.natives.routeHatch(c.hatched ? .changed : .built, controller: c, navigation: nav, scroll: scroll,
                                                 key: c.key, dataset: dataset)
            c.hatched = true
            if scroll != nil { c.ownedScroll = scroller }
        }
        coverLifted(routes)
        // A route's header shown or hidden in place; a push or pop sets the
        // bar in `willShow` instead, with UIKit's transition.
        if nav.transitionCoordinator == nil, let top = nav.topViewController as? RouteController, routes.contains(top) {
            showBar(nav, animated: nav.view.window != nil)
        }
    }

    /// The header a bar stands for takes no room from the turn it is lifted,
    /// before UIKit lays its route out (LLP 1115.000 D11.1). UIKit opens a
    /// pushed large title expanded only when the scroll view it follows is at
    /// its controller's top, under the bar, as the transition's first layout
    /// runs (measured on iOS 27: one 10 points lower arrives collapsed, and
    /// moving it up on the next turn does not open it). The cover reached
    /// layout with `reportCovers`, after the push had begun, so a pushed
    /// route's scroller was still below its header's height then, and every
    /// pushed level-1 title arrived inline and expanded only on a pull.
    private func coverLifted(_ routes: [RouteController]) {
        var lifted = false
        for c in routes {
            for node in c.lifted.map({ [$0] }) ?? c.barNodes where covers[node.id] != .whole {
                covers[node.id] = .whole
                lifted = true
            }
        }
        guard lifted else { return }
        presenter.afterBatch { [weak self] in self?.deliverCovers() }
    }

    /// Write the defaults an authored header gives a navigation item, and
    /// lift the header out of the route's layout while the bar shows it. A
    /// route that shows the bar with no header of the bar's (`shape` nil)
    /// is left UIKit's Back button alone: no title and no items, the
    /// button's title read from the route beneath, as UIKit reads it.
    private func project(_ shape: HeaderShape?, into c: RouteController, canGoBack: Bool, shows: Bool) {
        let item = c.navigationItem
        // The host's hiding goes; CSS's `display` decides (`NodeView.isHidden`
        // keeps the host's word apart: writing CSS's answer kept it hidden).
        for node in c.barNodes { node.isHidden = false }
        c.barNodes = []
        if let lifted = c.lifted, lifted !== shape?.header || shape?.authored.isEmpty == false {
            lifted.isHidden = false
            c.lifted = nil
        }
        guard shows else {
            // No stale items: the agent aims at a bar item by its press.
            c.barPresses = []
            item.leftBarButtonItems = nil
            item.rightBarButtonItems = nil
            return
        }
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
        if let shape {
            if shape.authored.isEmpty {
                shape.header.isHidden = true
                c.lifted = shape.header
            } else {
                c.barNodes = shape.placed
                for node in c.barNodes { node.isHidden = true }
            }
        }
    }

    /// A header's tablist of text tabs is the item's title view: a segmented
    /// control whose selection is the tabs' `aria-selected`, a tap on a
    /// segment pressing its tab (§9.8), as the content's segmented control
    /// does (LLP 1035.001 D10). The heading stays the item's title, which
    /// the back button on the next route reads.
    private func segmentedTitle(_ list: NodeView?, in c: RouteController) {
        let tabs = list.map(HeaderShape.titleTabs) ?? []
        guard !tabs.isEmpty else {
            if let old = c.titleSegments?.control, c.navigationItem.titleView === old { c.navigationItem.titleView = nil }
            c.titleSegments = nil
            return
        }
        let segments = c.titleSegments ?? TitleSegments(host: self)
        c.titleSegments = segments
        let control = segments.control
        segments.press.tabs = tabs.map(\.id)
        // A tab's words are its segment's title, an `aria-label` that says
        // otherwise its label (LLP 1116 D6), as in the content's segments.
        let titles = tabs.map { tab in
            if case .title(let words)? = tab.segmentFace { return words }
            return tab.accessibleName
        }
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
        control.segmentLabels = tabs.map(SegmentHost.segmentLabel)
        control.segmentIDs = tabs.map { $0.props["testId"] }
        control.segmentUnavailable = tabs.map { $0.disabled || $0.ariaUnavailable }
        for (index, tab) in tabs.enumerated() { control.setEnabled(!tab.disabled, forSegmentAt: index) }
        segments.fit(titles)
        if c.navigationItem.titleView !== control { c.navigationItem.titleView = control }
    }

    /// A header's search field is the item's `UISearchController` (§9.6):
    /// its placeholder and value are the field's; what the reader types is
    /// the field's `input` (and `focus`, `blur`), as typing in it would be.
    /// Where it sits and whether it hides as the content scrolls are
    /// UIKit's own answers for the device and the bar (LLP 1115.000 D11.2):
    /// Exact once pinned it under the title (`.stacked`, never hiding).
    private func searchField(_ field: NodeView?, in c: RouteController) {
        // tvOS navigation items carry no search controller.
        #if !os(tvOS)
        guard let field else {
            if let search = c.search {
                search.controller.searchBar.resignFirstResponder()
                search.controller.isActive = false
                search.field = nil
                c.navigationItem.searchController = nil
                c.search = nil
                // UIKit restores its bar at the end of search dismissal.
                // Reapply the route's choice after that transition too.
                let transition = search.controller.transitionCoordinator ?? c.navigationController?.transitionCoordinator
                transition?.animate(alongsideTransition: nil) { [weak self, weak c] _ in
                    guard let self, let c, let nav = c.navigationController else { return }
                    self.showBar(nav, animated: false)
                    self.coversChanged()
                }
            }
            return
        }
        let search = c.search ?? HeaderSearch(host: self)
        if c.search !== search {
            c.search = search
            c.navigationItem.searchController = search.controller
            c.definesPresentationContext = true
        }
        search.field = field
        let bar = search.controller.searchBar
        let placeholder = field.props["placeholder"] ?? ""
        if bar.placeholder != placeholder { bar.placeholder = placeholder }
        // The same font, colour and clearing rules as an ordinary field (LLP 1104 D3).
        field.applyPlaceholder(bar.searchTextField)
        // The Contract's value wins unless the reader is typing it.
        let value = field.props["value"] ?? ""
        if !bar.isFirstResponder, bar.text != value { bar.text = value }
        // UIKit hides the field as the content scrolls and keeps an active
        // search's in view. A search in effect that UIKit did not start (a
        // value the app or the agent wrote) keeps its field in view too.
        if c.navigationItem.hidesSearchBarWhenScrolling != value.isEmpty { c.navigationItem.hidesSearchBarWhenScrolling = value.isEmpty }
        // The field itself, which XCUITest finds as a search field and a
        // person types in, carries the test id (LLP 1116 D2).
        if bar.searchTextField.accessibilityIdentifier != field.props["testId"] { bar.searchTextField.accessibilityIdentifier = field.props["testId"] }
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
    /// content scroller (`contentScroller`: with nothing written, the first
    /// that scrolls vertically, LLP 1115.000 D11.1) when that scroller is at
    /// the route's top: right after the header the bar replaces, or first in
    /// a route whose bar holds the system Back alone. The scroller then goes
    /// under the bar, UIKit insets it
    /// and follows its offset, and CSS `scrollTop` is measured from the
    /// expanded title's inset (`scrollOrigin`), so an authored offset lands
    /// where the browser's does while the bar's height changes. An inline
    /// title's scroller goes under the bar the same way (§9.10), so the bar's
    /// scroll edge appearance follows it; its inset does not change. A
    /// scroller with something above it in the route keeps its place and the
    /// title keeps still: what is above it would sit under the bar.
    private func collapse(_ c: RouteController, shape: HeaderShape?, shows: Bool, scroller: NodeView?) {
        let kids = c.node.container.subviews.compactMap { $0 as? NodeView }
        let flowing = { (v: NodeView) in
            !v.isHidden && v.style["display"]?.string != "none" && !["absolute", "fixed"].contains(v.style["position_type"]?.string ?? "")
        }
        let node = shape.map { shape in kids.firstIndex { $0 === shape.header }.flatMap { kids.indices.contains($0 + 1) ? kids[$0 + 1] : nil } }
            ?? (shows ? kids.first(where: flowing) : nil)
        let target = shape?.authored.isEmpty != false && node?.scrollView != nil && node === scroller ? node : nil
        let physical = target?.scrollView
        guard c.collapseScroll !== target || c.collapseScrollView !== physical else { return }
        if let old = c.collapseScroll, let sv = c.collapseScrollView {
            old.retainScrollPosition(from: sv)
            let top = sv.contentOffset.y + old.scrollTopInset(sv)
            // The bottom edge's scroller stays UIKit's to inset (`reachBottom`).
            if old !== c.edgeScroll || sv !== old.scrollView { sv.contentInsetAdjustmentBehavior = .never }
            old.scrollOrigin = 0
            old.scrollCollapsed = 0
            sv.contentOffset.y = top - old.scrollTopInset(sv)
        }
        c.collapseScroll = target
        c.collapseScrollView = physical
        // A scroller at its start (CSS scrollTop 0) stays there as UIKit
        // begins to inset it: UIKit keeps the offset it had, a new
        // scroller's 0, which left its content's top under the bar.
        let shown = shownScroll(target)
        let atStart = shown.map { $0.contentOffset.y <= 0.5 - $0.adjustedContentInset.top } ?? false
        shown?.contentInsetAdjustmentBehavior = .always
        // …and is there before UIKit is handed it: a bar handed a
        // scroller that far from its top closes an open large title.
        if atStart, let sv = shown, sv.contentOffset.y != -sv.adjustedContentInset.top { sv.contentOffset.y = -sv.adjustedContentInset.top }
        let left = c.contentScrollView(for: .top)
        c.setContentScrollView(shown, for: .top)
        if let left, left !== shown, left.window != nil { retitle(c) }
        if atStart, let sv = shown { rest(sv, in: c) }
        let under = shape?.level == 1 ? "collapses its title with its scroller" : "scrolls its content under the bar"
        presenter.session?.log("navigation: route \(c.key) \(target == nil ? "keeps its title still" : under)")
    }

    /// Register a replacement before its first native layout. Resolve only
    /// the containing route, preserving that stack's Back relationships.
    func scrollBackendChanged(_ node: NodeView) {
        var responder: UIResponder? = node
        while let current = responder {
            if let route = current as? RouteController, route.host === self,
               let nav = route.navigationController {
                guard route.collapseScroll === node || route.edgeScroll === node || (node.scrollView != nil && contentScroller(of: route) === node) else { return }
                prepareRoutes(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
                return
            }
            responder = current.next
        }
    }

    /// UIKit moves a large title into the content scroll view it follows and
    /// takes it back only when that one leaves the window: handed another, or
    /// none, while the old one stays (chat-rooms' rooms list, moved below a
    /// failed load's reason), it left the title in the old one, at that one's
    /// top, clipped from sight: a blank band under the bar (LLP 1116.005 D4).
    /// The title made again goes where UIKit now looks, the new scroller or
    /// the bar itself. One turn, unanimated: no frame shows the inline title
    /// between, and the covers it moves are reported once, after.
    private func retitle(_ c: RouteController) {
        // tvOS has no large titles.
        #if !os(tvOS)
        let item = c.navigationItem
        guard item.largeTitleDisplayMode == .always, c.viewIfLoaded?.window != nil, let nav = c.navigationController,
              nav.topViewController === c, nav.transitionCoordinator == nil, !nav.isNavigationBarHidden else { return }
        UIView.performWithoutAnimation {
            item.largeTitleDisplayMode = .never
            nav.navigationBar.layoutIfNeeded()
            item.largeTitleDisplayMode = .always
            nav.navigationBar.layoutIfNeeded()
        }
        #endif
    }

    /// A route's new collapsing scroller at its start (`collapse`): its
    /// content's top just below the bar, where UIKit's inset puts it, and a
    /// large title expanded, as a new screen opens. A header and scroller
    /// replaced in place (trivia-quiz's results after its questions) find
    /// the bar as the content before left it, collapsed or inline. UIKit
    /// opens a large title as its scroller's offset passes above its top, so
    /// the scroller is pulled past its top, as a finger pulls it, and put at
    /// rest there in the same turn: no frame shows the pull. Exact asked the
    /// bar to `sizeToFit` before, which opens a title only on the first
    /// screen of a stack that has pushed nothing yet and closes one anywhere
    /// else (measured on iOS 27, in UIKit alone): a pushed route's scroller
    /// replaced in place lost its title.
    func rest(_ sv: UIScrollView, in c: RouteController) {
        if sv.contentOffset.y != -sv.adjustedContentInset.top { sv.contentOffset.y = -sv.adjustedContentInset.top }
        // tvOS has no large titles.
        #if !os(tvOS)
        guard c.navigationItem.largeTitleDisplayMode == .always, c.viewIfLoaded?.window != nil,
              let nav = c.navigationController, nav.topViewController === c, nav.transitionCoordinator == nil,
              !nav.isNavigationBarHidden else { return }
        UIView.performWithoutAnimation {
            // By the bar's height at a time, while the title still opens:
            // UIKit grows the inset by what the pull opened of it.
            for _ in 0..<4 {
                let top = sv.adjustedContentInset.top
                sv.contentOffset.y = -top - nav.navigationBar.frame.height
                nav.view.layoutIfNeeded()
                if sv.adjustedContentInset.top <= top + 0.5 { break }
            }
            sv.contentOffset.y = -sv.adjustedContentInset.top
        }
        #endif
    }

    /// A shown route whose collapsing scroller is still at its start gets
    /// the title a new screen has, expanded (`rest`). A stack's first route
    /// at launch needs it: its scroller was attached before its view had a
    /// window, where `rest` cannot open the title, and UIKit then showed a
    /// level-1 title inline over content that had not moved. Seen in a
    /// single stack; under a tab bar UIKit fits it itself.
    func restAtStart(_ c: RouteController) {
        guard let node = c.collapseScroll, let sv = shownScroll(node), sv.contentOffset.y <= 0.5 - sv.adjustedContentInset.top else { return }
        rest(sv, in: c)
    }

    /// The bottom edge, as `collapse` is the top's (LLP 1116 D2): a route
    /// whose last in-flow child is a scroller — a `navigationScroll` or not —
    /// lets it run to the screen's bottom edge, under the home indicator and
    /// a tab bar, and UIKit insets its content there
    /// (`adjustedContentInset.bottom`), as a hand-built table view's is. The
    /// route keeps no bottom cover for it (`reportCovers`). A footer under
    /// the scroller keeps the cover; so does a page under `viewport-fit=cover`,
    /// whose own `env()` insets would be added to UIKit's.
    private func reachBottom(_ c: RouteController) {
        let flowing = { (v: NodeView) in
            v.style["display"]?.string != "none" && !["absolute", "fixed"].contains(v.style["position_type"]?.string ?? "")
        }
        let last = c.node.container.subviews.last { ($0 as? NodeView).map(flowing) == true } as? NodeView
        // One that scrolls vertically: UIKit's bottom inset would let a
        // sideways carousel scroll up and down.
        let vertical = ["scroll", "auto"].contains(last?.style["overflow_y"]?.string ?? "")
        let target = presenter.viewportFit != "cover" && last?.scrollView != nil && vertical ? last : nil
        guard c.edgeScroll !== target || c.contentScrollView(for: .bottom) !== target?.scrollView else { return }
        if let old = c.edgeScroll, let sv = old.scrollView, old !== c.collapseScroll { sv.contentInsetAdjustmentBehavior = .never }
        if let previous = c.contentScrollView(for: .bottom), previous !== target?.scrollView,
           previous !== c.collapseScrollView { previous.contentInsetAdjustmentBehavior = .never }
        c.edgeScroll = target
        let shown = shownScroll(target)
        shown?.contentInsetAdjustmentBehavior = .always
        // tvOS has no bottom bar whose edge follows a scroller.
        #if !os(tvOS)
        c.setContentScrollView(shown, for: .bottom)
        #endif
        presenter.session?.log("navigation: route \(c.key) \(target == nil ? "keeps its content above the bottom edge" : "scrolls its content to the bottom edge")")
    }

    /// The physical backend owned by a content node.
    func shownScroll(_ node: NodeView?) -> UIScrollView? { node?.scrollView }

    /// Whether UIKit insets this scroller (`.always`): a route's collapse or
    /// bottom-edge scroller.
    func insetByUIKit(_ node: NodeView) -> Bool {
        controllers.values.contains { $0.collapseScroll === node || $0.edgeScroll === node }
    }

    /// Whether the page hides `view` only because a bar shows it: a lifted
    /// header, or a node the bar placed, that its author did not hide.
    func hiddenForBar(_ view: UIView) -> Bool {
        guard let node = view as? NodeView, node.style["display"]?.string != "none" else { return false }
        return controllers.values.contains { $0.lifted === node || $0.barNodes.contains { $0 === node } }
    }

    private func barItem(_ i: HeaderShape.Item, _ c: RouteController) -> UIBarButtonItem {
        let press = BarPress(i.id, self)
        c.barPresses.append(press)
        let action = #selector(BarPress.press)
        let image = i.badge?.image ?? i.symbol.flatMap { UIImage(systemName: $0) }
        let item: UIBarButtonItem
        if i.menu, let button = presenter.views[i.id] {
            // A pull-down: UIKit opens it on tap (`MenuHost.pullDown`).
            let deferred = presenter.menus.pullDown(of: button, inBar: true)
            item = image.map { UIBarButtonItem(image: $0, menu: UIMenu(children: [deferred])) }
                ?? UIBarButtonItem(title: i.title, menu: UIMenu(children: [deferred]))
        } else {
            item = image.map { UIBarButtonItem(image: $0, style: .plain, target: press, action: action) }
                ?? UIBarButtonItem(title: i.title, style: .plain, target: press, action: action)
        }
        item.accessibilityLabel = i.label ?? (i.title.isEmpty ? nil : i.title)
        item.accessibilityIdentifier = i.testId
        item.isEnabled = !i.disabled
        if i.disabled || i.accessibilityDisabled { item.accessibilityTraits.insert(.notEnabled) }
        else { item.accessibilityTraits.remove(.notEnabled) }
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

    /// A route's content scroller (LLP 1115.000 D11.1). With nothing written
    /// it is the first scroller under the route that scrolls vertically, as
    /// a hand-built screen's scroll view is its bar's unasked; a route names
    /// a different one with `navigationScroll`, resolved inside it by HTML
    /// id as its Back control is (E7: nothing searches), and a name that
    /// finds no scroller is none. One rule for everything that asks: the
    /// bar's scroller (`collapse`), the status bar's tap and a reselected
    /// tab's (`ScrollsToTopIOS`), and the scroll view a route hatch is given.
    func contentScroller(of c: RouteController) -> NodeView? {
        if let name = c.node.props["navigationScroll"], !name.isEmpty {
            return element(named: name, in: c.node).flatMap { $0.scrollView == nil ? nil : $0 }
        }
        return Presenter.firstVerticalScroller(under: c.node)
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

    /// Whether the root's routes have a native primary container (a stack
    /// or the tabs), so the session's view takes the whole of its own
    /// (ExactViewIOS `fit`): the document and the window the viewport units
    /// resolve against. Its bars, whether shown or not, and a
    /// presentation's containers never decide it (LLP 1123 D1); its routes
    /// clear what covers them (`reportCovers`). Not under the agent's own
    /// chrome, where no native container shows.
    var wantsWholeView: Bool { primaryOwner != nil && !ExactEnv.authoredChrome }

    /// What covers a route of its controller's safe area (`safe`): the
    /// edges past the document's insets (`env()`, which the page authored
    /// against) for a route in the document, and every edge of its own for a
    /// presented one, whose sheet the document's insets do not clear (LLP
    /// 1123 D2). The top is a collapsing title's scroller's when it has one,
    /// and the bottom a scroller's at the bottom edge (`reachBottom`); the
    /// sides and the bottom only while the containers take the whole view
    /// (`whole`), and the bottom only as far as the route reaches it.
    static func routeEdges(safe: UIEdgeInsets, document: UIEdgeInsets, presented: Bool, whole: Bool,
                           collapses: Bool, scrollsToBottom: Bool, reached: CGFloat) -> HostCover.Edges {
        let env = presented ? .zero : document
        return .init(top: collapses ? 0 : max(0, safe.top - env.top),
                     right: whole ? max(0, safe.right - env.right) : 0,
                     bottom: whole && !scrollsToBottom ? max(0, min(safe.bottom, reached) - env.bottom) : 0,
                     left: whole ? max(0, safe.left - env.left) : 0)
    }

    /// What Exact's containers cover of each route — its controller's safe
    /// area past the insets the kernel already has as `env()`: a shown bar's
    /// top, and every edge while the containers take the whole view for a
    /// page that did not ask to cover it — and the header a bar replaces,
    /// reported when they change, after the batch being applied.
    func reportCovers() {
        var changes: [(UInt32, HostCover?)] = []
        var wanted: [UInt32: HostCover] = [:]
        for c in controllers.values where presenter.views[c.node.id] === c.node {
            // A lifted header takes no room, on a stack yet or not (`coverLifted`).
            for node in c.lifted.map({ [$0] }) ?? c.barNodes { wanted[node.id] = .whole }
            guard let nav = c.navigationController, nav.viewControllers.contains(c) else { continue }
            // A presented route's controller takes its sheet's whole view, and
            // the document's insets (`env()`) are not the sheet's: its route is
            // covered for its own safe area whatever the document's fit (LLP
            // 1123 D2; its own clearance and credit are stage 2's)
            let presented = presentedNavigations.contains { $0 === nav }
            let whole = presented || (wantsWholeView && presenter.viewportFit != "cover")
            let shows = barShows(nav)
            guard shows || whole else { continue }
            let edges: HostCover.Edges
            // A collapsing title's scroller goes under the bar, which insets it;
            // its expanded inset is where CSS scrollTop 0 rests.
            // While the bar is not as the route wants it (mid-transition to or
            // from a route without one, §9.10), its last cover stands.
            // An active search's bar is UIKit's: the content follows it, as
            // UIKit's own does, but a title's insets are not sampled.
            let search = searching(c)
            let settled = search || nav.isNavigationBarHidden != routeShowsBar(c, in: nav)
            if settled, !search, let node = c.collapseScroll, let sv = c.collapseScrollView, sv === node.scrollView, sv.adjustedContentInset.top > 0 {
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
            // A scroller UIKit insets lays its content out between those
            // insets (LLP 1116.000 D1.2): content that fills its height rests
            // under the bar and above the home indicator, and scrolling
            // content still passes under both. A large title's inset is its
            // widest, at rest (CSS scrollTop 0).
            for node in [c.collapseScroll, c.edgeScroll].compactMap({ $0 }) where node.scrollView != nil {
                if settled, c.viewIfLoaded?.window != nil, let safe = node.scrollView?.safeAreaInsets {
                    let top = max(safe.top, node === c.collapseScroll ? node.scrollOrigin : 0)
                    if top > 0 || safe.bottom > 0 { wanted[node.id] = .inset(top: top, bottom: safe.bottom) }
                } else if case .inset? = covers[node.id] { wanted[node.id] = covers[node.id] }
            }
            if settled, c.viewIfLoaded?.window != nil {
                let safe = c.view.safeAreaInsets
                // Only what the route reaches of the bottom bars is covered:
                // none once the keyboard ends it above them (`ownerFrame`).
                let reached = c.node.convert(c.node.bounds, to: c.view).maxY - (c.view.bounds.height - safe.bottom)
                edges = Self.routeEdges(safe: safe, document: presenter.insets, presented: presented, whole: whole,
                                        collapses: c.collapseScroll != nil, scrollsToBottom: c.edgeScroll != nil, reached: reached)
            } else if case .edges(let e)? = covers[c.node.id] {
                edges = e
            } else { continue }
            if edges != .init(top: 0, right: 0, bottom: 0, left: 0) { wanted[c.node.id] = .edges(edges) }
        }
        // The tablist whose place the tab bar takes takes no room.
        if let list = adoptedTablist.flatMap({ presenter.views[$0] }) {
            measuredTablist = presenter.session?.view.map { list.convert(list.bounds, to: $0) }
            if adopts(tablist: list) { wanted[list.id] = .whole }
            else if tablistAppearanceNone, list.style["display"]?.string != "none",
                    wantsWholeView, presenter.viewportFit != "cover",
                    let view = presenter.session?.view, view.window != nil {
                // Authored tabs are page content outside the routes. Protect
                // only the edges they reach; leave the viewport and bar
                // backgrounds full-size. A sheet leaves them where they are
                // (LLP 1123 D1).
                let rect = list.convert(list.bounds, to: view)
                let safe = view.bounds.inset(by: view.safeAreaInsets)
                let edges = HostCover.Edges(
                    top: max(0, min(rect.height, safe.minY - rect.minY)),
                    right: max(0, min(rect.width, rect.maxX - safe.maxX)),
                    bottom: max(0, min(rect.height, rect.maxY - safe.maxY)),
                    left: max(0, min(rect.width, safe.minX - rect.minX)))
                if edges != .init(top: 0, right: 0, bottom: 0, left: 0) { wanted[list.id] = .edges(edges) }
            }
        }
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
        presenter.afterBatch { [weak self] in self?.deliverCovers() }
    }

    /// What the kernel is told is what is wanted when it is told. A report
    /// queued behind a batch ran after later ones that did not wait, and its
    /// covers, measured under the bar's first frame before the status bar's
    /// inset (116 points with a large title of 168), won: team-kanban's
    /// persona switcher lay under the large title, drawn over by it and
    /// untappable (LLP 1116.006 D1).
    private func deliverCovers() {
        var changes: [(UInt32, HostCover?)] = []
        for (id, cover) in covers where delivered[id] != cover { changes.append((id, cover)) }
        for id in delivered.keys where covers[id] == nil && presenter.views[id] != nil { changes.append((id, nil)) }
        delivered = covers
        guard !changes.isEmpty else { return }
        presenter.onCovers?(changes)
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
            guard let node = c.ownedScroll, let scroll = node.scrollView else { continue }
            for property in NavigationHost.ownedChanges(scroll, of: node, insetByUIKit: c.collapseScroll === node || c.edgeScroll === node) { say("route \(c.key)", property) }
        }
    }

    /// What Exact sets on a node's scroll view and nothing else may (LLP
    /// 1075.003 §3.5) and differs from what Exact writes: no inset (a
    /// refresh control insets it while it spins), no automatic adjustment
    /// but a route's collapse or bottom-edge scroller's, the backend's Exact-owned delegate, the node's keyboard dismissal (authored, or
    /// the platform's for what it scrolls, LLP 1115.000 D9.1). The offset
    /// and the content size are Exact's too but are not checked: they move
    /// with the user and with layout, so no last write predicts them.
    static func ownedChanges(_ s: UIScrollView, of node: NodeView, insetByUIKit: Bool) -> [String] {
        var out: [String] = []
        // A native backend owns layout/footer insets, composed with the
        // refresh control and keyboard; zero is only the plain-scroll baseline.
        if s === node.scroll {
            #if os(tvOS)
            if s.contentInset != .zero { out.append("contentInset") }
            #else
            if s.contentInset != .zero, s.refreshControl?.isRefreshing != true { out.append("contentInset") }
            #endif
        }
        if s.contentInsetAdjustmentBehavior != (insetByUIKit ? .always : .never) { out.append("contentInsetAdjustmentBehavior") }
        if s !== node.scrollView || !node.ownsScrollDelegate { out.append("delegate") }
        if s.keyboardDismissMode != node.keyboardDismissal { out.append("keyboardDismissMode") }
        return out
    }
}
#endif

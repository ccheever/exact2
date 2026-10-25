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
        let toolbarGroup: UInt32?
        /// A face drawn from the button's one filled box (an avatar or a
        /// badge: a shape holding a text or a symbol), when the button has
        /// one: the item's image, in the box's own colours and corners.
        let badge: BadgeFace?
        /// A face that says more than a title or symbol (a dot beside its
        /// label, a font of the author's), drawn as painted (`ChipFace`).
        let chip: ChipFace?
        /// Whether the button opens a menu-shaped popover or a confirmation
        /// (`MenuHost.pullsDown`): the item's menu, its rows read when it
        /// opens, as LLP 1021 D3's pull-down. Any other popover opens as
        /// itself, from the item (`MenuHost.openFromBar`).
        let menu: Bool
        /// The popover's `aria-label`: the menu's title, as an in-page
        /// button's is (MenusIOS: "Open location in", "Sort by").
        let menuTitle: String
        /// A native button's prominent style: a prominent bar item (iOS 26),
        /// as LLP 1069.011.000 D2 maps `NSToolbarItem`.
        let prominent: Bool
        /// A native button's `glass` or `clear-glass` style: a glass capsule
        /// of its own, not the one iOS 26 shares among adjacent items
        /// (`sharesBackground`). A bar item has no clear variant.
        let ownGlass: Bool
        /// The colour the author gave the item's face, light and dark: a
        /// symbol's tint, else a `color` written on the button itself. Nil
        /// (UIKit's tint, the platform default) for a colour the button
        /// only inherits or the compiler implied.
        let tint: [[Double]]?
        /// A native button's title font where its face's rows give a size or
        /// a weight (LLP 1069.011.001 D4), as its in-page control draws it:
        /// the item's title in it, so its glass fits those words. Nil is
        /// UIKit's bar title font.
        let font: UIFont?
        /// A readout's face (`role="status"`, LLP 1075.003 §9.15): an item
        /// no one presses, whose custom view shows the face as written and
        /// follows it in place (BarReadoutIOS.swift).
        let readout: ChipFace?
        /// The padding around a readout's face inside the glass: each side
        /// as its author wrote it, else UIKit's (`titleInset` beside words).
        let readoutPadding: UIEdgeInsets
        /// How far a `UIBarButtonItem(title:)`'s label sits inside its item
        /// on each side, which UIKit does not publish: 12 pt on iOS 26 and 27
        /// (measured), so its glass is 16 pt wider than the words each side.
        static let titleInset: CGFloat = 12
        /// The font UIKit draws a plain bar item's title in, which it does
        /// not publish: the system font at 17 pt, medium in the 26 design and
        /// bold under Bold Text (both measured on iOS 27,
        /// `testTheUnsaidTitleFontIsUIKits`); regular before it, semibold
        /// under Bold Text. A row its author left unsaid keeps this one's
        /// size or weight; the kernel has stepped the weights it sends.
        static func titleFont(bold: Bool) -> UIFont {
            if #available(iOS 26.0, *), LinkedDesign.liquidGlass { return .systemFont(ofSize: 17, weight: bold ? .bold : .medium) }
            return .systemFont(ofSize: 17, weight: bold ? .semibold : .regular)
        }
        init(_ button: NodeView, readout isReadout: Bool = false) {
            var symbol: String?, text = "", ink: (NodeView, String)?
            func walk(_ node: NodeView) {
                for case let child as NodeView in node.container.subviews {
                    if child.kind == "image", let name = child.faceSymbol {
                        if symbol == nil { symbol = name; ink = (child, "text_color") }
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
            var parent = button.superview
            var grouped: UInt32?
            while let view = parent {
                if let node = view as? NodeView {
                    if node.props["semanticTag"] == "header" { break }
                    if node.props["accessibilityRole"] == "group" { grouped = node.id; break }
                }
                parent = view.superview
            }
            toolbarGroup = grouped
            id = button.id
            testId = button.props["testId"]
            readout = isReadout ? ChipFace(button, readout: true) : nil
            // A side left unsaid is UIKit's. A bare custom view hugs its
            // capsule, whose round ends then cut into its words, so a face
            // with words takes the inset a title item's label has.
            let words = readout?.parts.contains { if case .text = $0 { true } else { false } } == true
            let pad = { (side: String, platform: CGFloat) in button.wrote("padding_\(side)") ? button.number("padding_\(side)") : platform }
            let beside: CGFloat = words ? Self.titleInset : 0
            readoutPadding = isReadout ? UIEdgeInsets(top: pad("top", 0), left: pad("left", beside), bottom: pad("bottom", 0), right: pad("right", beside)) : .zero
            badge = button.isNativeButton || isReadout ? nil : BadgeFace(button)
            chip = badge == nil && !isReadout ? ChipFace(button) : nil
            let invocation = button.presenter?.invocation(of: button)
            menu = invocation.map { $0.opensPopover && button.presenter?.menus.pullsDown($0.target) == true } == true
            menuTitle = menu ? invocation?.target.props["accessibilityLabel"] ?? "" : ""
            self.symbol = symbol
            title = text
            label = button.props["accessibilityLabel"] ?? face?.label ?? (isReadout ? Self.shownWords(button) : nil)
            disabled = button.disabled
            accessibilityDisabled = button.ariaUnavailable
            prominent = ["filled", "bordered-prominent", "prominent-glass", "prominent-clear-glass"].contains(face?.style ?? "")
            ownGlass = ["glass", "clear-glass"].contains(face?.style ?? "")
            #if os(tvOS)
            // tvOS's bar title font is not measured: its own stays.
            font = nil
            #else
            let rows = face?.rows.title ?? [:]
            font = rows["font_size"] != nil || rows["font_weight"] != nil
                ? ButtonConfigurationIOS.font(Self.titleFont(bold: button.traitCollection.legibilityWeight == .bold), rows: rows) : nil
            #endif
            // The platform's tint unless the author said otherwise (LLP
            // 1115.000 D2): a `color` written on the symbol, else a `color`
            // written on the button, a platform colour as much as a literal.
            // One the button only inherits, or a sheet wrote, is unsaid.
            let symbolTint = ink?.0.kind == "image" ? ink?.0.ownColor("text_color") : nil
            let row = symbolTint ?? button.ownColor("text_color")
            if let row, let light = row.channels(dark: false), let dark = row.channels(dark: true) {
                tint = [light, dark]
            } else {
                tint = nil
            }
        }
        /// What a readout says with no `aria-label`: its words that show,
        /// none `display: none` or `aria-hidden` (the bar's own hiding of the
        /// header is `isHidden`, not this).
        static func shownWords(_ node: NodeView) -> String {
            guard node.style["display"]?.string != "none", node.props["accessibilityElementsHidden"] != "true" else { return "" }
            if node.isParagraph { return node.accessibleText }
            return node.container.subviews.compactMap { ($0 as? NodeView).map(shownWords) }.filter { !$0.isEmpty }.joined(separator: " ")
        }
        /// Everything a bar item is made from. A readout's face is not: its
        /// view follows it in place (`showReadouts`).
        var source: String {
            if readout != nil { return "readout:\(toolbarGroup.map(String.init) ?? ""):\(id):\(testId ?? "")" }
            return "\(toolbarGroup.map(String.init) ?? ""):\(id):\(title):\(symbol ?? ""):\(label ?? ""):\(disabled):\(accessibilityDisabled):\(prominent):\(ownGlass):\(badge?.source ?? ""):\(chip?.source ?? ""):\(menu):\(menuTitle):\(tint ?? []):\(font.map { "\($0.fontName) \($0.pointSize) \(TextEngine.controlWeight($0))" } ?? ""):\(testId ?? "")"
        }
    }
    let header: NodeView
    let heading: NodeView?
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
        // A readout hidden as authored, itself or by a box around it in the
        // header, shows nothing (the bar's own hiding is `isHidden`, not this).
        let readouts = rule.readouts.filter { readout in
            var at: UIView? = readout
            while let view = at, view !== rule.header {
                if (view as? NodeView)?.style["display"]?.string == "none" { return false }
                at = view.superview
            }
            return true
        }
        let roles = rule.items + readouts + [rule.heading, rule.tap, rule.search, rule.segments].compactMap { $0 }
        let authored = projected ? roles.filter { $0.wrote("appearance") && $0.style["appearance"]?.string == "none" } : []
        self.authored = authored
        let projects = { (node: NodeView) in !authored.contains { node === $0 || node.isDescendant(of: $0) } }
        authoredBack = back != nil && roles.contains { $0.props["id"] == back && !projects($0) }
        let heading = rule.heading, titleProjects = projects(heading)
        self.heading = titleProjects ? heading : nil
        let search = rule.search.flatMap { projects($0) ? $0 : nil }
        let segments = rule.segments.flatMap { projects($0) ? $0 : nil }
        let tap = rule.tap.flatMap { projects($0) ? $0 : nil }
        header = rule.header
        // A heading of only white space names nothing (accname trims it);
        // UIKit would take it as a title and show a pair of quotes
        let text = titleProjects ? heading.accessibleText : ""
        title = text.unicodeScalars.allSatisfy(\.properties.isWhitespace) ? "" : text
        level = titleProjects ? rule.level : 0
        let item = { (node: NodeView) -> Item? in
            guard rule.readouts.contains(where: { $0 === node }) else { return Item(node) }
            guard readouts.contains(where: { $0 === node }) else { return nil }
            let item = Item(node, readout: true)
            return item.readout == nil ? nil : item
        }
        let before = rule.leading.filter(projects).compactMap(item), after = rule.trailing.filter(projects).compactMap(item)
        // A readout is apart from the title when it has a face to show: one
        // hidden, or with nothing to draw, is the page's, as the web has it.
        let drawn = (before + after).filter { $0.readout != nil }.map(\.id)
        let faced = readouts.filter { drawn.contains($0.id) }
        #if os(tvOS)
        // tvOS's bar has no item that focus passes by undimmed: no readout.
        leading = before.filter { $0.readout == nil }
        trailing = after.filter { $0.readout == nil }
        let shownReadouts: [NodeView] = []
        #else
        (leading, trailing) = (before, after)
        let shownReadouts = faced
        #endif
        self.search = search
        self.segments = segments
        let group = titleProjects ? HeaderTitle(header: header, heading: heading, tap: tap, apart: rule.items + faced + [rule.search, rule.segments].compactMap { $0 }) : nil
        self.group = group
        let tablist = segments.flatMap { HeaderShape.titleTabs($0).isEmpty ? nil : $0 }
        // The title's pressable wrapper goes with the title it holds, unless
        // it holds an opted-out node, which stays where it is.
        let wrapper = titleProjects ? tap.flatMap { tap in authored.contains { $0.isDescendant(of: tap) } ? nil : tap } : nil
        placed = ([heading] + rule.items + shownReadouts + [search, tablist, wrapper].compactMap { $0 } + (HeaderShape.presented(group, segments: segments)?.placed ?? [])).filter(projects)
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
        host?.followFloatingBar(nav, for: controller, animated: animated)
        host?.navigationController(nav, willShow: controller, animated: animated)
        app?.navigationController?(nav, willShow: controller, animated: animated)
    }

    func navigationController(_ nav: UINavigationController, didShow controller: UIViewController, animated: Bool) {
        // Restoring a hidden bar changes the route's safe area. Lay out its
        // container before reporting that cover to the kernel.
        nav.view.setNeedsLayout()
        if nav.viewIfLoaded?.window != nil {
            // That layout can start the bar's own update UIKit deferred to
            // the transition's end (iPadOS applies a push's items only then):
            // started inside the end, it takes the bar's touches and never
            // gives them back (UIKit gives them back when a transition ends,
            // and this one already has), so Back and a drawn title take no
            // taps. What this layout took is given back at once, before the
            // host and the app hear of the route: a bar either of them turns
            // off stays off. (An update UIKit animates takes taps a moment
            // early; one it leaves to a later turn is not this layout's.)
            let bar = nav.navigationBar, touches = bar.isUserInteractionEnabled
            nav.view.layoutIfNeeded()
            if touches, !bar.isUserInteractionEnabled { bar.isUserInteractionEnabled = true }
        }
        host?.followFloatingBar(nav, for: controller, animated: false) // (where it settled: a cancelled pop's source included)
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
final class HeaderSearch: NSObject, UISearchBarDelegate, UISearchControllerDelegate, UITextFieldDelegate {
    let controller = UISearchController(searchResultsController: nil)
    weak var host: NavigationHost?
    /// The field's placeholder is drawn in an authored colour, not UIKit's.
    var paintsPlaceholder = false
    /// The field's text is drawn in its own colour or the design's FieldText; its fill is the design's Field, not UIKit's capsule.
    var paintsInk = false, paintsGround = false
    /// The scope bar's tabs, and the segment last chosen or set (LLP 1134 D3).
    var scopeTabs: [UInt32] = [], scopeIDs: [String?] = [], scopeSettled: Int?
    weak var scopeList: NodeView?
    /// UIKit's field's text: the node's own colour, else the design's
    /// FieldText where it overrides it, else UIKit's (LLP 1115 D4, LLP 1121
    /// D9); and its fill, the design's Field where it overrides it. Set
    /// again on a live retheme.
    func ink() {
        #if !os(tvOS)
        guard let field else { return }
        let text = controller.searchBar.searchTextField
        // Any `backgroundColor`, nil too, replaces UIKit's capsule (iOS 26, measured): written only where a design names a Field, cleared (unfilled) only where one was.
        if let ground = field.fieldGroundColor { text.backgroundColor = ground; paintsGround = true }
        else if paintsGround { text.backgroundColor = nil; paintsGround = false }
        if field.showsOwnTextColor || ThemeRoles.color("FieldText", in: field) != nil {
            text.textColor = field.fieldInk
            paintsInk = true
        } else if paintsInk {
            text.textColor = .label
            paintsInk = false
        }
        #endif
    }
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
    private var editingCompletion: (value: String, finish: () -> Void)?
    init(host: NavigationHost) {
        self.host = host
        super.init()
        controller.searchBar.delegate = self
        controller.delegate = self
        #if os(iOS)
        textDelegate = controller.searchBar.searchTextField.delegate
        controller.searchBar.searchTextField.delegate = self
        #endif
        controller.obscuresBackgroundDuringPresentation = false
        controller.hidesNavigationBarDuringPresentation = true
    }
    /// The navigation item the controller is on.
    weak var item: UINavigationItem?
    /// The field scrolls away with the content only while it is empty
    /// (`NavigationHost.searchField`), and never when its author made it
    /// stick vertically, CSS's way to keep a box in view as its scroller
    /// scrolls (React Navigation's `hideWhenScrolling={false}`): `position:
    /// sticky` with a `top` or `bottom` that is not `auto` (CSS Positioned
    /// Layout 3 §3.4: a box with both `auto` does not stick on that axis).
    func applyHiding() {
        #if os(iOS)
        guard let item else { return }
        let sticks = field.map { f in
            f.layoutPosition == "sticky" && ["top", "bottom"].contains { f.style[$0]?.length(basis: 0) != nil }
        } ?? false
        let hides = (field?.props["value"] ?? "").isEmpty && !sticks
        if item.hidesSearchBarWhenScrolling != hides { item.hidesSearchBarWhenScrolling = hides }
        #endif
    }
    func didDismissSearchController(_ search: UISearchController) {
        applyHiding()
        #if os(iOS)
        showScopes()
        #endif
    }
    func searchBar(_ bar: UISearchBar, textDidChange text: String) {
        guard let field, let presenter = host?.presenter, presenter.views[field.id] === field else { return }
        field.typedText(text)
    }
    #if os(iOS)
    func searchBarCancelButtonClicked(_ bar: UISearchBar) {
        searchBar(bar, textDidChange: "")
    }
    #endif
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
        let completion = editingCompletion
        editingCompletion = nil
        if let field, let presenter = host?.presenter, presenter.views[field.id] === field {
            // UIKit can clear a dismissed bar before delivering its end-of-editing callback
            let value = completion?.value ?? bar.text ?? ""
            presenter.commitEdit(field.id, value, change: field.handlers.contains("change"))
            presenter.blur(field.id)
        }
        if let completion { host?.presenter.afterBatch(completion.finish) }
    }

    /// Keep the authored field alive until UIKit delivers its final edit and blur.
    func endEditing(completion: @escaping () -> Void) -> Bool {
        guard let field, let presenter = host?.presenter, presenter.views[field.id] === field,
              let editor = field.headerField, editor.isEditing else { return false }
        if editingCompletion == nil {
            editingCompletion = (editor.text ?? "", completion)
            controller.isActive = false
            controller.searchBar.endEditing(true)
        }
        return true
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
    @objc func press() {
        guard let host, let node = host.presenter.views[id] else { return }
        // A popover that is no menu, named with no press of its own, opens
        // from the item (`MenuHost.openFromBar`); with a press, after it
        // (`MenuHost.invoke`).
        if !node.handlers.contains("press"), let invocation = host.presenter.invocation(of: node), invocation.opensPopover,
           !host.presenter.menus.pullsDown(invocation.target) {
            _ = host.presenter.menus.openFromBar(node)
            return
        }
        _ = host.act(id, 0)
    }
}

/// One stack Exact built: its delegate, whether its routes may show the bar
/// (they may unless the app's `navigation` hatch says no; which of them
/// shows it is each route's, LLP 1115.000 D5.3), and its last written controllers.
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
    func activateChrome(_ node: NodeView, before: (CGPoint?) -> Bool = { _ in true }) -> [String: Any]? {
        guard !ExactEnv.authoredChrome else { return nil }
        let id = Int(node.id)
        if tabBarShows, let root = container, let tabs = NavigationTabs.of(root, presenter), let index = tabs.tabs.firstIndex(where: { $0 === node }) {
            guard !node.disabled else { return ["error": "tab #\(id) is disabled"] }
            guard before(nil), !node.disabled, !node.inert, presenter.views[node.id] === node else { return ["error": "tab #\(id) changed during its pointer handlers"] }
            let root = reselectedRoot(index) // as the bar's own tap: the selected tab at its root scrolls to the top
            presenter.press(node.id)
            if let root { scrollToTop(root) }
            return ["tapped": id, "pressed": id, "delivery": "host-activation", "native": "tab-bar-item"]
        }
        let shown = controllers.values.filter { c in
            guard let nav = c.navigationController, nav.topViewController === c, barShows(nav) else { return false }
            return c.viewIfLoaded?.window != nil
        }
        // A readout the bar shows (§9.15) is tapped as a person taps it:
        // nothing happens. Its header may be lifted whole or only in part.
        if let readout = shown.lazy.compactMap({ c in c.readouts.first { id, _ in
            id == node.id || self.presenter.views[id].map { node.isDescendant(of: $0) } == true }?.value }).first {
            let point = readout.customView.map { $0.convert(CGPoint(x: $0.bounds.midX, y: $0.bounds.midY), to: nil) }
            guard before(point) else { return ["error": "bar readout #\(id) is unavailable"] }
            return ["tapped": id, "delivery": "host-activation", "native": "bar-readout"]
        }
        guard shown.contains(where: { c in
            c.barPresses.contains { $0.id == node.id } || c.lifted.map { node === $0 || node.isDescendant(of: $0) } == true
        }) else { return nil }
        guard !node.disabled, before(nil), !node.disabled, !node.inert, presenter.views[node.id] === node else { return ["error": "bar item #\(id) is unavailable"] }
        // An item whose button only opens a menu is UIKit's pull-down, which
        // no public API opens: under the agent's painted popovers it opens
        // painted (`agentOpen(barItem:)`); under UIKit's menus a real touch opens it.
        if !node.handlers.contains("press"), !node.disabled, let invocation = presenter.invocation(of: node), invocation.opensPopover {
            // A popover that is no menu opens as itself, from the item, as
            // from a native button in the page (LLP 1021 D3 covers menus only).
            if !presenter.menus.pullsDown(invocation.target) {
                return presenter.menus.openFromBar(node)
                    ? ["tapped": id, "delivery": "host-activation", "native": "bar-button-item", "popover": "lifted"]
                    : ["error": "bar item #\(id) opens nothing now: it is ineligible or names no popover"]
            }
            switch presenter.menus.agentOpen(barItem: node) {
            case true?: return ["tapped": id, "delivery": "host-activation", "native": "bar-pull-down", "menu": "painted"]
            case false?: return ["error": "bar item #\(id) opens nothing now: its menu's button is ineligible or names no popover"]
            case nil: return ["error": "unsupported: bar item #\(id) opens its menu as UIKit's pull-down from the bar, which only a real touch opens: under --touch platform `tap` opens it and `tap <row>` picks a row (LLP 1021 D4)"]
            }
        }
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
    /// The theme's label colour on `nav`'s titles (LLP 1095 D4); its glyphs
    /// take the app's tint, which the theme's `AccentColor` sets. Set again
    /// when a live edit moves the roles (LLP 1121 stage 5).
    /// Only the colour is the theme's: what a native hatch set beside it (a
    /// font, spacing) stays, and so does a colour it set over the theme's.
    func titleInk(_ nav: UINavigationController) {
        guard let ink = ThemeRoles.color("label", in: presenter.root) ?? ThemeRoles.color("CanvasText", in: presenter.root) else { return }
        let bar = nav.navigationBar, before = titleInks.object(forKey: bar)
        func themed(_ attributes: [NSAttributedString.Key: Any]?) -> [NSAttributedString.Key: Any] {
            var attributes = attributes ?? [:]
            let now = attributes[.foregroundColor] as? UIColor
            if now == nil || now === before { attributes[.foregroundColor] = ink }
            return attributes
        }
        bar.titleTextAttributes = themed(bar.titleTextAttributes)
        #if !os(tvOS)
        bar.largeTitleTextAttributes = themed(bar.largeTitleTextAttributes)
        #endif
        titleInks.setObject(ink, forKey: bar)
    }

    /// Every bar this host made, its titles' ink set again, and each rich
    /// title's subtitle.
    func retheme() {
        var seen = Set<ObjectIdentifier>()
        for nav in [primaryNavigation].compactMap({ $0 }) + presentedNavigations + tabNavigations.values where seen.insert(ObjectIdentifier(nav)).inserted { titleInk(nav) }
        for c in controllers.values {
            c.titleView?.retheme()
            c.search?.ink()
        }
    }

    /// `publish` records the controller where Exact keeps it before the
    /// hatch runs, which may ask for it again (`HeldStackIOS`'s `realize`).
    func makeNavigation(first: NodeView?, publish: (UINavigationController) -> Void = { _ in }) -> UINavigationController {
        let nav = UINavigationController()
        stackCount += 1
        let stack = NavigationStack(showsBar: true, order: stackCount)
        stack.proxy.host = self
        stacks[ObjectIdentifier(nav)] = stack
        nav.delegate = stack.proxy
        #if !os(tvOS)
        nav.navigationBar.prefersLargeTitles = true
        #endif
        titleInk(nav)
        publish(nav)
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

    /// Project each route of a stack into its navigation item when its
    /// authored source changed, then run the route hatch — before UIKit lays
    /// the stack out (LLP 1075.003 §3.2, §3.5 "projected defaults"). A held
    /// stack (HeldStackIOS) is projected as a navigation controller that
    /// hides its bar would be; no `navigation` hatch has met it.
    func prepareRoutes(_ routes: [RouteController], in stack: UIViewController) {
        let nav = stack as? UINavigationController
        if let nav {
            hatchNavigation(nav)
            #if os(iOS)
            presenter.menus.focus.watch(nav.navigationBar) // a bar item's menu (MenuFocusIOS), every stack, rebuilt or not
            #endif
            projectBack(routes, in: nav)
        }
        followTablist(routes, in: stack)
        let stackShows = nav.map(barShows) ?? !ExactEnv.authoredChrome
        let back = container?.props["navigationBack"]
        let presented = nav.map { nav in presentedNavigations.contains { $0 === nav } } ?? false
        for (index, c) in routes.enumerated() {
            // The bar is each route's (LLP 1115.000 D5.2, D5.3): its header,
            // the system Back alone on a pushed route with none of its own
            // (no header, a hidden one, one that is not a bar's), or no bar
            // where the route says so or has nothing to go back to.
            c.pushed = index > 0
            let transparent = Self.transparentBar(c.node)
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
            let signature = HatchSource(bar: source, back: c.backSource ?? "", navigation: ObjectIdentifier(stack), dataset: dataset ?? "",
                                       scroll: scroll.map(ObjectIdentifier.init))
            if c.projectedSource != source {
                c.projectedSource = source
                project(shape, into: c, canGoBack: canGoBack, shows: shows)
            }
            #if os(iOS)
            scope(c, shape: shows ? shape : nil)
            #endif
            collapse(c, shape: shape, shows: shows, scroller: scroller)
            showReadouts(shows ? shape : nil, in: c)
            reachBottom(c, presented: presented)
            c.paintBackdrop()
            searchField(shows ? shape?.search : nil, in: c)
            segmentedTitle(shows ? shape?.segments : nil, in: c)
            richTitle(shows ? shape : nil, in: c)
            c.titleInk.update(shows ? shape?.heading?.ownUIColor("text_color") : nil, item: c.navigationItem, bar: nav?.navigationBar, transparent: transparent)
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
        if let nav, nav.transitionCoordinator == nil, let top = nav.topViewController as? RouteController, routes.contains(top) {
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
            for node in (c.lifted.map({ [$0] }) ?? c.barNodes) + [c.scoped].compactMap({ $0 }) where covers[node.id] != .whole {
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
        // A header the bar stops standing for shows its native buttons.
        defer { presenter.controls.syncShown() }
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
        #if EXACT_INSPECT
        c.shownHeader = shape?.header
        c.itemNodes = [:]
        #endif
        // tvOS has no large titles or back button.
        #if !os(tvOS)
        // @ref LLP 1141.001 D7 — a level-1 heading is large wherever its
        // route is, a sheet's too (a Settings sheet's); a heading of another
        // level or none is inline, as a sheet's title is unless its author
        // says otherwise (LLP 1116.020 D2).
        item.largeTitleDisplayMode = shape?.level == 1 ? .always : .never
        item.hidesBackButton = !canGoBack
        item.leftItemsSupplementBackButton = true
        #endif
        c.barPresses = []
        item.leftBarButtonItems = shape.map { barItems($0.leading, c) }
        item.rightBarButtonItems = shape.map { barItems(Array($0.trailing.reversed()), c) }
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
        search.scope(c.scoped)
        let bar = search.controller.searchBar
        let placeholder = field.props["placeholder"] ?? ""
        // Unsaid, the placeholder is the search field's own, which UIKit
        // draws darker than an ordinary field's `placeholderText`; an
        // authored `::placeholder` colour or theme role wins (LLP 1104 D3).
        if field.placeholderColor != nil || ThemeRoles.color("placeholder", in: presenter.root) != nil {
            field.applyPlaceholder(bar.searchTextField)
            search.paintsPlaceholder = true
        } else if search.paintsPlaceholder || bar.placeholder != placeholder {
            search.paintsPlaceholder = false
            bar.searchTextField.attributedPlaceholder = nil
            bar.placeholder = placeholder
        }
        search.ink()
        // The Contract's value wins unless the reader is typing it.
        let value = field.props["value"] ?? ""
        if !bar.isFirstResponder, bar.text != value { bar.text = value }
        // UIKit hides the field as the content scrolls and keeps an active
        // search's in view. A search in effect that UIKit did not start (a
        // value the app or the agent wrote) keeps its field in view too.
        // Changed while UIKit presents or dismisses the search, the item
        // leaves the active field and its Close button drawn over the bar
        // (round 17's contacts: Close clears the text, so the app's empty
        // value lands mid-dismissal), so it waits for the dismissal's end.
        search.item = c.navigationItem
        if !searching(c) { search.applyHiding() }
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
    /// title keeps still: what is above it would sit under the bar. A route
    /// with no bar in containers that take the whole view puts its first
    /// scroller under the status bar the same way (LLP 1130 D3).
    private func collapse(_ c: RouteController, shape: HeaderShape?, shows: Bool, scroller: NodeView?) {
        let kids = c.node.container.subviews.compactMap { $0 as? NodeView }
        // In flow by layout: a popover a context preview lays out is hidden
        // here while it pushes the scroller down (`MenusIOS`), and a title
        // must not collapse with a scroller that is no longer the route's top.
        let flowing = { (v: NodeView) in (!v.isHidden || v.presenter?.menus.isLaidOut(v) == true) && Self.inFlow(v) }
        let whole = wantsWholeView && presenter.viewportFit != "cover"
        let node = shape.map { shape in kids.firstIndex { $0 === shape.header }.flatMap { kids[($0 + 1)...].first(where: flowing) } }
            ?? (shows || whole ? kids.first(where: flowing) : nil)
        var riders: [NodeView] = []
        #if os(iOS)
        if #available(iOS 26.0, *), shape?.level == 1, let scroller,
           scroller.style["scroll_edge_effect"]?.string != "none", !Self.transparentBar(c.node),
           case let bands = presenter.controlBands(of: scroller, in: c.node, ControlBars(presenter)), !bands.top.isEmpty {
            riders = bands.top
        }
        #endif
        for old in c.ridingBars where !riders.contains(where: { $0 === old }) { old.controlRide = 0; old.applyTransform() }
        c.ridingBars = riders
        c.rideBars()
        let covered: Bool
        if case .edges(let edges)? = covers[c.node.id] { covered = edges.top > 0.5 } else { covered = false }
        let ordinary = shape?.authored.isEmpty != false && node?.scrollView != nil && node === scroller ? node : nil
        let arranged = scroller.flatMap { node in arrangementScrollers(in: c, top: true).contains(where: { $0 === node }) ? node : nil }
        let target = !riders.isEmpty ? (covered ? nil : scroller) : (ordinary ?? arranged)
        if let target, !riders.isEmpty {
            let frame = target.convert(target.bounds, to: c.node)
            let band = UIEdgeInsets(top: max(0, frame.minY), left: 0, bottom: target.pageBand.bottom, right: 0)
            var own = band
            own.top += min(band.top, c.view.safeAreaInsets.top)
            if target.pageBand != band || target.bandInset != own { target.setPageBand(band, inset: own) }
        }
        let physical = target?.scrollView
        // A route that turns transparent, or back, keeps its scroller: only
        // whether UIKit insets it follows.
        if let sv = physical, c.collapseScroll === target, c.collapseScrollView === sv {
            sv.contentInsetAdjustmentBehavior = Self.transparentBar(c.node) ? .never : .automatic
        }
        guard c.collapseScroll !== target || c.collapseScrollView !== physical else { return }
        let old = c.collapseScroll, oldView = c.collapseScrollView
        if let old, let sv = oldView { old.retainScrollPosition(from: sv) }
        c.collapseScroll = target
        c.collapseScrollView = physical
        // A scroller at its start (CSS scrollTop 0) stays there as UIKit
        // begins to inset it: UIKit keeps the offset it had, a new
        // scroller's 0, which left its content's top under the bar.
        let shown = shownScroll(target)
        let atStart = shown.map { $0.contentOffset.y <= 0.5 - $0.adjustedContentInset.top } ?? false
        // Under a transparent bar the content starts at the top edge, under
        // the bar, as on the web under a fixed transparent header: a hero
        // photo bleeds to the top, and the bar's material comes as it scrolls.
        shown?.contentInsetAdjustmentBehavior = Self.transparentBar(c.node) ? .never : .automatic
        // …and is there before UIKit is handed it: a bar handed a
        // scroller that far from its top closes an open large title.
        if atStart, let sv = shown, sv.contentOffset.y != -sv.adjustedContentInset.top { sv.contentOffset = PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: -sv.adjustedContentInset.top), in: sv) }
        c.setContentScrollView(shown ?? c.bandedTop, for: .top) // or the scroller its control bars are over (LLP 1131)
        // The scroller left behind gets the page's offsets back only once the
        // bar follows the new one: UIKit reads its inset taken away while the
        // bar still follows it as a scroll, and closes an open large title.
        if let old, let sv = oldView, sv !== shown {
            let top = sv.contentOffset.y + old.scrollTopInset(sv)
            // The bottom edge's scroller stays UIKit's to inset (`reachBottom`).
            if old !== c.edgeScroll || sv !== old.scrollView { sv.contentInsetAdjustmentBehavior = .never }
            old.scrollOrigin = 0
            old.scrollCollapsed = 0
            sv.contentOffset = PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: top - old.scrollTopInset(sv)), in: sv, clamped: !sv.isTracking && !sv.isDragging && !sv.isDecelerating)
        }
        // UIKit keeps a pushed route's large title inside the scroller the bar
        // follows and moves it only on the bar's next layout, which handing
        // the bar another scroller does not ask for: a scroller left in the
        // window below a reason keeps the title, clipped from sight (LLP
        // 1116.005 D4; UIKit alone does the same on iOS 27).
        c.navigationController?.navigationBar.setNeedsLayout()
        let under = shape?.level == 1 ? "collapses its title with its scroller" : "scrolls its content under the bar"
        presenter.session?.log("navigation: route \(c.key) \(target == nil ? "keeps its title still" : under)")
    }

    /// Whether a route asks `navigationBar="transparent"`: its bar shows
    /// with a clear scroll-edge appearance and insets no content.
    static func transparentBar(_ route: NodeView) -> Bool { route.props["navigationBar"] == "transparent" }

    /// The bar's appearance at a scroller's top edge for a transparent bar:
    /// nothing drawn until the content scrolls under it, UIKit's own
    /// standard appearance then (`UINavigationBarAppearance`, "Customizing
    /// Your App's Navigation Bar").
    static let clearScrollEdge: UINavigationBarAppearance = {
        let appearance = UINavigationBarAppearance()
        appearance.configureWithTransparentBackground()
        return appearance
    }()

    /// Register a replacement before its first native layout. Resolve only
    /// the containing route, preserving that stack's Back relationships.
    func scrollBackendChanged(_ node: NodeView) {
        for current in ViewPresentation.controllers(for: node) {
            if let route = current as? RouteController, route.host === self, let stack = route.stack {
                guard route.collapseScroll === node || route.edgeScroll === node || (node.scrollView != nil && contentScroller(of: route) === node) else { return }
                prepareRoutes(stack.stackRoutes.compactMap { $0 as? RouteController }, in: stack)
                return
            }
        }
    }

    /// The bottom edge, as `collapse` is the top's (LLP 1116 D2): a route
    /// whose last in-flow child is a scroller — a `navigationScroll` or not —
    /// lets it run to the screen's bottom edge, under the home indicator and
    /// a tab bar, and UIKit insets its content there
    /// (`adjustedContentInset.bottom`), as a hand-built table view's is. The
    /// route keeps its bottom cover, which the kernel pads nothing of while
    /// UIKit insets the scroller (`reportCovers`, `scroller_insets_bottom`),
    /// so an absolutely positioned footer still rests on the bars. A footer under
    /// the scroller keeps the cover; so does a page under `viewport-fit=cover`,
    /// whose own `env()` insets would be added to UIKit's, though not a
    /// sheet's route, which is covered for the sheet's whole safe area
    /// whatever the document's fit (`reportCovers`); and so does a
    /// transparent bar's scroller, which UIKit insets nothing (`collapse`).
    private func reachBottom(_ c: RouteController, presented: Bool) {
        // Until a scroller reaches the bottom edge the route's bottom cover
        // pads its in-flow content above the bars, and its content box is
        // the authored one: a scroller there ends that far above it. The
        // cover it has without one, so a scroller that does not grow as the
        // cover goes is at the edge either way.
        let peers = arrangementScrollers(in: c, top: false)
        let peer = contentScroller(of: c).flatMap { node in peers.contains(where: { $0 === node }) ? node : nil } ?? peers.first
        let found = presented || presenter.viewportFit != "cover" ? (Self.bottomScroller(in: c.node, cover: c.bottomClearance) ?? peer) : nil
        let target = found === c.collapseScroll && Self.transparentBar(c.node) ? nil : found
        guard c.edgeScroll !== target || c.contentScrollView(for: .bottom) !== (target?.scrollView ?? c.bandedBottom) else { return }
        if let old = c.edgeScroll, let sv = old.scrollView, old !== c.collapseScroll { sv.contentInsetAdjustmentBehavior = .never }
        if let previous = c.contentScrollView(for: .bottom), previous !== target?.scrollView,
           previous !== c.collapseScrollView { previous.contentInsetAdjustmentBehavior = .never }
        c.edgeScroll = target
        let shown = shownScroll(target)
        // A scroller at its start stays there as UIKit begins to inset it.
        // One made in this batch can hold a safe area from before its first
        // frame (at the route's top, under the bar): UIKit insets it by that
        // first, then, given it below, takes the inset away and moves the
        // offset by as much, scrolling its content out of its port.
        let atStart = shown.map { $0.contentOffset.y <= 0.5 - $0.adjustedContentInset.top } ?? false
        shown?.contentInsetAdjustmentBehavior = .automatic
        if atStart, let sv = shown, sv.contentOffset.y != -sv.adjustedContentInset.top { sv.contentOffset = PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: -sv.adjustedContentInset.top), in: sv) }
        // tvOS has no bottom bar whose edge follows a scroller.
        #if !os(tvOS)
        c.setContentScrollView(shown ?? c.bandedBottom, for: .bottom)
        #endif
        presenter.session?.log("navigation: route \(c.key) \(target == nil ? "keeps its content above the bottom edge" : "scrolls its content to the bottom edge")")
    }

    /// The scroller at a route's bottom edge: its last in-flow child, or
    /// that child's last, down through boxes that alone span their parent's
    /// bottom edge (a sheet's component whose root column holds its header
    /// and then its `scroll`), as a hand-built screen's scroll view pinned to
    /// its view's bottom is at the edge however deep it sits. A box that ends
    /// short of its parent's bottom (a padding, a border or a margin under
    /// it), or has a sibling beside it in a row or a grid, keeps the route's
    /// cover. One that scrolls vertically only: UIKit's bottom inset would
    /// let a sideways carousel scroll up and down. `cover` is the route's
    /// bottom cover, which the route's child may end above.
    static func bottomScroller(in route: NodeView, cover: CGFloat = 0) -> NodeView? {
        let flowing = inFlow
        // The last in-flow child spans the bottom edge alone when every other
        // ends above its top, as children stacked down a block or a column
        // do. Beside it in a row or a grid, a sibling reaches the edge too
        // (a column with a Save button at its foot) and keeps the cover. The
        // laid-out frames say so; the host is not sent the flex direction.
        func alone(_ last: NodeView, among children: [NodeView]) -> Bool {
            children.allSatisfy { $0 === last || $0.frame.maxY <= last.frame.minY + 0.5 }
        }
        var parent = route
        while true {
            let children = parent.container.subviews.compactMap { $0 as? NodeView }.filter(flowing)
            guard let last = children.last, alone(last, among: children) else { return nil }
            // Each reaches the foot of its parent's content box: the route's
            // child that box less the cover the kernel may pad it by, a
            // deeper one its parent's bottom, with nothing (a padding, a
            // border) under it.
            let foot = parent === route ? parent.contentBox().maxY - cover : parent.bounds.maxY
            if parent !== route, parent.contentBox().maxY < foot - 0.5 { return nil }
            if parent.convert(last.bounds, from: last).maxY < foot - 0.5 { return nil }
            if last.scrollView != nil { return ["scroll", "auto"].contains(last.style["overflow_y"]?.string ?? "") ? last : nil }
            parent = last
        }
    }

    /// Whether `v` takes a place in its parent's flow as the kernel lays it
    /// out: not `display: none`, not positioned out of it, not a popover
    /// (the kernel lays an open one out of flow, as HTML's top layer is, and
    /// a closed one not at all), and not a closed `dialog`, which the host
    /// leaves out of layout (`MenuHost.layOut`), as a browser's
    /// `dialog:not([open])` is `display: none`. So a route's menus and
    /// confirmations, written between its header and its scroller or after
    /// the scroller, leave the scroller its route's top and bottom edges. A
    /// dialog given back to layout to show in Exact's views counts as laid out.
    static func inFlow(_ v: NodeView) -> Bool {
        // A popover lays out absolute (`layoutPosition`), unless sticky; a
        // sticky one is in flow while laid out, as a dialog is (`MenuHost.layOut`).
        let dialog = v.props["semanticTag"] == "dialog" || v.props["popover"] != nil
        return v.style["display"]?.string != "none" && !["absolute", "fixed"].contains(v.layoutPosition)
            && (!dialog || v.presenter?.menus.isLaidOut(v) == true)
    }

    /// The physical backend owned by a content node.
    func shownScroll(_ node: NodeView?) -> UIScrollView? { node?.scrollView }

    /// Whether UIKit insets this scroller under its bars (`.automatic`): a
    /// route's collapse or bottom-edge scroller.
    func insetByUIKit(_ node: NodeView) -> Bool {
        controllers.values.contains { $0.collapseScroll === node || $0.edgeScroll === node ||
            (arrangementScrollers(in: $0, top: true) + arrangementScrollers(in: $0, top: false)).contains(where: { $0 === node }) }
    }

    func arrangementScrollers(in route: RouteController, top: Bool) -> [NodeView] {
        #if os(iOS)
        if #available(iOS 27.1, *), !Self.transparentBar(route.node) { return presenter.arrangements.scrollers(in: route.node, top: top) }
        #endif
        return []
    }

    /// UIKit's zero-width fixed space splits shared glass without authored metrics.
    func barItems(_ items: [HeaderShape.Item], _ route: RouteController) -> [UIBarButtonItem] {
        var result: [UIBarButtonItem] = []
        var previous: UInt32?
        for (index, item) in items.enumerated() {
            #if !os(tvOS)
            if #available(iOS 26.0, *), index > 0, previous != item.toolbarGroup {
                result.append(UIBarButtonItem(barButtonSystemItem: .fixedSpace, target: nil, action: nil))
            }
            #endif
            result.append(barItem(item, route))
            previous = item.toolbarGroup
        }
        return result
    }

    /// Whether the page hides `view` only because a bar shows it: a lifted
    /// header, or a node the bar placed, that its author did not hide.
    func hiddenForBar(_ view: UIView) -> Bool {
        guard let node = view as? NodeView, node.style["display"]?.string != "none" else { return false }
        return controllers.values.contains { $0.lifted === node || $0.scoped === node || $0.barNodes.contains { $0 === node } }
    }

    private func barItem(_ i: HeaderShape.Item, _ c: RouteController) -> UIBarButtonItem {
        let item = barItemMade(i, c)
        #if EXACT_INSPECT
        c.itemNodes[ObjectIdentifier(item)] = i.id // (inspect mode's bar walk, InspectIOS.swift)
        #endif
        return item
    }

    private func barItemMade(_ i: HeaderShape.Item, _ c: RouteController) -> UIBarButtonItem {
        if i.readout != nil { return readoutItem(i, c) }
        let press = BarPress(i.id, self)
        c.barPresses.append(press)
        let action = #selector(BarPress.press)
        let image = i.badge?.image ?? i.chip?.image(tint: presenter.viewport.tintColor) ?? i.symbol.flatMap { FaceSymbol.image($0, points: FaceSymbol.barItemPoints) }
        let item: UIBarButtonItem
        if i.menu, let button = presenter.views[i.id] {
            // A pull-down: UIKit opens it on tap (`MenuHost.pullDown`).
            let menu = presenter.menus.pullDown(of: button, titled: i.menuTitle, inBar: true)
            item = image.map { UIBarButtonItem(image: $0, menu: menu) } ?? UIBarButtonItem(title: i.title, menu: menu)
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
        if let tint = i.tint, !i.prominent, i.badge == nil, i.chip == nil {
            item.tintColor = UIColor { TextEngine.color(tint[$0.userInterfaceStyle == .dark ? 1 : 0]) }
        }
        if let font = i.font, image == nil {
            for state: UIControl.State in [.normal, .highlighted, .disabled, .selected, .focused] {
                item.setTitleTextAttributes([.font: font], for: state)
            }
        }
        if #available(iOS 26.0, tvOS 26.0, *) {
            if i.prominent { item.style = .prominent }
            // A drawn face is its own shape: no glass capsule around it.
            #if !os(tvOS)
            if i.badge != nil { item.hidesSharedBackground = true }
            else if i.ownGlass { item.sharesBackground = false }
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

    /// The authored content scroller, or the route's document viewport when none was named.
    func contentScrollView(of route: RouteController) -> UIScrollView? {
        if let node = contentScroller(of: route) { return shownScroll(node) }
        if let name = route.node.props["navigationScroll"], !name.isEmpty { return nil }
        return route.contentViewport
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
}
#endif

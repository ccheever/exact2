// @ref LLP 1075.003 §3.7, §9.9 — what a root tab shows, read from the
// authored tab: its symbol, if it has one, its label (a text-only tab is a
// title alone), whether it is disabled, and its badge. UIKit's tab bar item
// is made from it (NavigationTabsIOS.swift) and AppKit's sidebar row
// (LLP 1115.002 D3, SidebarMac.swift). Which tabs a root has is
// NavigationTabs.swift's.
#if canImport(AppKit)
import AppKit
#else
import UIKit
#endif

struct TabFace: Equatable {
    /// The symbol without a `.fill` ending: a bar chooses the variant it
    /// draws (UIKit's the filled one for the selected tab).
    /// `accessibilityDisabled`: `aria-disabled`, which reads as disabled
    /// and still presses.
    let base: String?, title: String, disabled: Bool, accessibilityDisabled: Bool
    /// A filled box holding a text among the tab's children: the item's
    /// badge (LLP 1075.003 §9.9), as the web paints it on the tab.
    let badge: String?
    init(_ tab: NodeView) {
        var symbol: String?, label = tab.props["accessibilityLabel"] ?? "", badges: [String] = []
        if tab.isNativeButton, let face = tab.face {
            // A native button's children are its face (LLP 1069.011.000 D1).
            symbol = face.symbol
            if let title = face.title, !title.isEmpty { label = title }
        }
        for case let child as NodeView in tab.container.subviews {
            if child.kind == "image", let name = child.props["symbolName"], !name.isEmpty { symbol = name }
            else if child.isParagraph, !child.accessibleText.isEmpty { label = child.accessibleText }
            else if let text = Self.badgeText(child) { badges.append(text) }
        }
        // One badge box, or none: two are not a badge.
        self.badge = badges.count == 1 ? badges[0] : nil
        base = symbol.map { $0.hasSuffix(".fill") ? String($0.dropLast(5)) : $0 }
        title = label
        disabled = tab.disabled
        accessibilityDisabled = tab.ariaUnavailable
    }
    /// A shown box with a visible fill whose only child (views and flat
    /// leaves alike) is one shown, non-empty text: that text. A pill around
    /// a symbol and a label is not, nor a transparent box or text.
    private static func badgeText(_ box: NodeView) -> String? {
        let shown = { (v: NodeView) in !v.isHidden && v.style["display"]?.string != "none" && v.shownAlpha > 0 }
        guard !box.isParagraph, box.kind != "image", shown(box),
              let fill = box.channels("background_color"), fill[3] > 0, !box.holdsFlatLeaves else { return nil }
        let children = box.container.subviews.compactMap { $0 as? NodeView }
        guard children.count == 1, let text = children.first, text.isParagraph, shown(text),
              !text.accessibleText.isEmpty else { return nil }
        return text.accessibleText
    }
}

private extension NodeView {
    #if canImport(AppKit)
    var shownAlpha: CGFloat { alphaValue }
    /// AppKit has no flat leaves: every child is a view.
    var holdsFlatLeaves: Bool { false }
    #else
    var shownAlpha: CGFloat { alpha }
    var holdsFlatLeaves: Bool { presenter?.flats.holdsLeaves(id) == true }
    #endif
}

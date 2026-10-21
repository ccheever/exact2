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
    /// draws (UIKit's the filled one for the selected tab). An svg-only
    /// icon's path token (`FaceSymbol`, stage 9 A2) has no variant.
    /// `accessibilityDisabled`: `aria-disabled`, which reads as disabled
    /// and still presses.
    let base: String?, title: String, disabled: Bool, accessibilityDisabled: Bool
    /// A child image's bundled raster (`app:/…` or a bundle-relative name)
    /// where the tab has no symbol: drawn as the item's template glyph.
    let raster: String?
    /// A filled box holding a text among the tab's children: the item's
    /// badge (LLP 1075.003 §9.9), as the web paints it on the tab.
    let badge: String?
    /// An svg-only icon's token name (`icon.reply`) where the face says it:
    /// the agent's name for a path `base`.
    let icon: String?
    init(_ tab: NodeView) {
        var icon: String?, raster: String?
        var symbol: String?, label = tab.props["accessibilityLabel"] ?? "", badges: [String] = []
        if tab.isNativeButton, let face = tab.face {
            // A native button's children are its face (LLP 1069.011.000 D1).
            symbol = face.symbol
            icon = face.symbolIcon
            if let title = face.title, !title.isEmpty { label = title }
        }
        for case let child as NodeView in tab.container.subviews {
            if child.kind == "image", let name = child.faceSymbol { symbol = name }
            else if child.kind == "image", let source = child.props["imageSource"], Self.isLocalRaster(source) { raster = source }
            else if child.isParagraph, !child.accessibleText.isEmpty { label = child.accessibleText }
            else if let text = Self.badgeText(child) { badges.append(text) }
        }
        // One badge box, or none: two are not a badge.
        self.badge = badges.count == 1 ? badges[0] : nil
        self.icon = icon
        self.raster = symbol == nil ? raster : nil
        base = symbol.map(Self.base)
        title = label
        disabled = tab.disabled
        accessibilityDisabled = tab.ariaUnavailable
    }
    /// `symbol` without a `.fill` ending (`base`); a path token as it is.
    static func base(_ symbol: String) -> String {
        symbol.hasSuffix(".fill") && !FaceSymbol.isPath(symbol) ? String(symbol.dropLast(5)) : symbol
    }
    /// A source the bar can read without fetching: the app's own file or a
    /// bundled asset, never a `symbol:`, `data:` or remote one.
    static func isLocalRaster(_ source: String) -> Bool {
        !source.isEmpty && !source.hasPrefix("symbol:") && !source.hasPrefix("data:") && !source.contains("://")
    }

    #if canImport(UIKit)
    /// A tab bar's selected glyph for `base`: its `.fill` variant, nil for a
    /// path or a symbol without one.
    static func filled(_ base: String) -> UIImage? {
        FaceSymbol.isPath(base) ? nil : UIImage(systemName: base + ".fill")
    }
    /// The raster at `url` as a tab bar item's glyph: fitted in `points`
    /// square at the screen's scale and template-rendered, so the bar tints
    /// it as it tints an SF Symbol (UIKit renders a tab's image as a
    /// template whatever its mode). Nil for a file that is no image.
    static func rasterImage(at url: URL, points: CGFloat) -> UIImage? {
        guard points > 0, let image = UIImage(contentsOfFile: url.path), image.size.width > 0, image.size.height > 0 else { return nil }
        let fit = min(points / image.size.width, points / image.size.height)
        let size = CGSize(width: (image.size.width * fit).rounded(), height: (image.size.height * fit).rounded())
        let format = UIGraphicsImageRendererFormat.default()
        format.scale = UIScreen.main.scale
        let drawn = UIGraphicsImageRenderer(size: size, format: format).image { _ in image.draw(in: CGRect(origin: .zero, size: size)) }
        return drawn.withRenderingMode(.alwaysTemplate)
    }
    #endif

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

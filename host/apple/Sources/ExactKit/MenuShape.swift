// @ref LLP 1021 D3 — which popovers an Apple host may present as the
// platform's own menu.

extension NodeView {
    /// Whether this popover is menu-shaped (LLP 1021 D3): `button` rows
    /// (a native one, one whose face fits a menu item, or text with at most
    /// one image) and `hr` separators, nothing else, so it may present as
    /// the platform's menu (`NSMenu`, `UIMenu`). A popover inside it (a
    /// submenu authored in its parent) is no row of it: closed, it is
    /// `display: none` on the web.
    var menuShaped: Bool {
        let rows = container.subviews.compactMap { $0 as? NodeView }
            .filter { $0.props["contextPreview"] != "true" && $0.props["popover"] == nil }
        func textOnly(_ node: NodeView) -> Bool {
            node.kind == "text" && node.container.subviews.compactMap { $0 as? NodeView }.allSatisfy(textOnly)
        }
        return !rows.isEmpty && rows.allSatisfy { row in
            if row.props["semanticTag"] == "hr" { return true }
            guard row.isButton else { return false }
            if row.isNativeButton { return true }
            if let face = row.face, face.fits, !face.raster { return true }
            // Text, and at most one image: the item's title and its image.
            let content = row.container.subviews.compactMap { $0 as? NodeView }
            let images = content.filter { $0.kind == "image" && $0.container.subviews.isEmpty }
            return images.count <= 1 && content.allSatisfy { textOnly($0) || images.contains($0) }
        }
    }
}

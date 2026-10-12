// A node's colour rows as AppKit and Core Graphics draw them, each in its own
// space (LLP 1100 D2), and the tint they resolve by. Moved out of
// NodeViewMac.swift (the 1,500-line cap).
#if os(macOS)
import AppKit

extension NodeView {
    func textChannels(_ key: String, dark: Bool? = nil) -> [Double]? { style[key].flatMap { $0.textChannels(dark: dark ?? drawsDark, contrast: drawsHighContrast, tint: ownTint(for: $0)) } }
    package func color(_ key: String, _ fallback: NSColor) -> NSColor { cgColor(key).flatMap { NSColor(cgColor: $0) } ?? fallback }
    /// A colour row as Core Graphics draws it (LLP 1100 D2).
    func cgColor(_ key: String, dark: Bool? = nil) -> CGColor? { style[key].flatMap { $0.cgColor(dark: dark ?? drawsDark, contrast: drawsHighContrast, tint: ownTint(for: $0)) } }

    /// The node whose `accent-color` is this view's tint: the nearest at or
    /// above it whose accent is a colour (`auto`, or one naming the tint
    /// itself, leaves the one above). AppKit gives a view no tint to
    /// inherit, so it is read up the view chain.
    private var accentNode: NodeView? {
        var view: NSView? = self
        while let at = view {
            if let node = at as? NodeView, let row = node.style["accent_color"], !row.namesTint, row.channels(dark: false) != nil { return node }
            view = at.superview
        }
        return nil
    }

    /// @ref LLP 1115.000 D3.4 — an authored `accent-color` is its subtree's
    /// tint on the Mac too, as it is UIKit's inherited `tintColor` on iOS
    /// (LLP 1116.001 D2): every row beneath that names the tint
    /// (`AccentColor`, a symbol that names it, an `svg`'s paint) resolves by
    /// the nearest one, not only a control. Nil with none: the system's
    /// accent. The colour follows the appearance it is asked in, as a
    /// `light-dark()` accent must. Reading it records whose it was
    /// (`tintSource`), so a move under another accent is known (`retint`).
    var nearestAccent: NSColor? {
        let node = accentNode
        tintSource = node.map(ObjectIdentifier.init)
        guard let node else { return nil }
        return NSColor(name: nil) { [weak node] appearance in
            let dark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            return node?.style["accent_color"]?.channels(dark: dark).map(TextEngine.color) ?? .controlAccentColor
        }
    }

    /// This node's `accent-color` changed: everything beneath it that names
    /// the tint resolves again. UIKit says so itself (`tintColorDidChange`);
    /// AppKit has no tint, so the subtree is walked. The node's own rows are
    /// applied by the style change that brought this.
    func accentChanged() {
        for child in subviews { NodeView.retint(under: child, changed: true) }
    }
    /// Mounted or moved in a live window: what is above it may be another
    /// accent, for it and for what it holds.
    func retintSubtree() { NodeView.retint(under: self, changed: false) }
    private static func retint(under view: NSView, changed: Bool) {
        (view as? NodeView)?.retint(changed: changed)
        for child in view.subviews { retint(under: child, changed: changed) }
    }
    /// What names the tint here, applied again by the accent now above it:
    /// its rows, its inline runs, its symbol and an `svg`'s paints. Nothing
    /// to do where it names none, or where the accent above is the node's
    /// it last resolved by and that node's accent did not change.
    func retint(changed: Bool = false) {
        guard kind == "svg" || symbolView != nil || style.values.contains(where: \.namesTint) || inlineText.contains(where: \.namesTint) else { return }
        guard changed || accentNode.map(ObjectIdentifier.init) != tintSource else { return }
        if kind == "svg" { presenter?.svg.reappear(id, dark: drawsDark, clock: presenter?.session?.clock, tint: viewTint) }
        reapplyColors()
    }
}
#endif

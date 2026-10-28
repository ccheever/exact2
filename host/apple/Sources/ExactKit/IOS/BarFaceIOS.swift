// A bar item's face as its author painted it (LLP 1115 §2: what the author
// writes wins). A header button's text becomes UIKit's title in the system
// font, which is the platform's answer when the author said nothing more. A
// button whose face says more, a status dot beside its label or a label in a
// font of the author's (a "● Clock" chip in 12-point monospace), keeps that
// face: its parts drawn in a row, in their own fonts, sizes, gap and corners,
// in the page's scheme, as the item's image (`.alwaysOriginal`), as a hand-built
// bar's custom view would draw them. Ink its author did not write (a colour
// only inherited from the page) is the bar's own, as a title's is (LLP 1115
// D4). The header a bar replaces is laid out as `display: none`, so the parts
// are read from their styles, not their frames: the host sends the authored
// size of a childless box and an authored gap for this (`kept_geometry`,
// host/apple/src/style.rs). A native button has no such face: its title's
// size and weight are the item's title font (`HeaderShape.Item.font`).
#if os(iOS) || os(tvOS)
import UIKit

struct ChipFace: Equatable {
    enum Part: Equatable {
        /// A childless filled box: its size, its fill (light, dark) and corners.
        case dot(CGSize, [[Double]], [CGSize])
        /// A system symbol at a point size, in its tint (light, dark), nil
        /// when its author wrote none.
        case symbol(String, CGFloat, [[Double]]?)
        /// A line of text in its font, in its colour (light, dark), nil when
        /// its author wrote none.
        case text(String, UIFont, [[Double]]?)
    }
    let parts: [Part]
    /// Each part's opacity: its own and its ancestors' below the button
    /// (a view's alpha, a flat leaf's `opacity`), as CSS multiplies them.
    let opacities: [CGFloat]
    /// Each part's line height as its author wrote it (a text's
    /// `line-height`), nil where unsaid: a readout's label is that tall,
    /// its line centred in it as CSS's half-leading does.
    let lineHeights: [CGFloat?]
    let gap: CGFloat
    /// The scheme it is drawn in. A bar item sized from one variant of a
    /// light/dark image asset kept that size for the other (a dark capsule
    /// three times the face's width, iOS 27), so the face is drawn for the
    /// scheme the page is in and drawn again when that changes.
    let dark: Bool
    /// Bold Text, which weights the system font, an authored weight included,
    /// as it does the titles beside it (LLP 1115.000 D12.3): the kernel's
    /// weights say it; kept so a change draws the face again.
    let bold: Bool

    private enum Either { case view(NodeView), leaf(FlatLeaf) }

    /// The face of `button`, or nil when what it shows is one symbol or one
    /// line of text in the platform's type: the bar's own title or image.
    /// `readout`: a header's `role="status"` (LLP 1075.003 §9.15), whose
    /// face it is however plain, the readout itself its first part (a dot
    /// alone, its opacity), its unwritten type the platform's body.
    /// `traits`: the text size and Bold Text to draw for, the button's own
    /// unless the view showing the face has newer ones.
    init?(_ button: NodeView, engine: TextEngine? = nil, readout: Bool = false, traits: UITraitCollection? = nil) {
        guard !button.isNativeButton, let engine = engine ?? button.text else { return nil }
        let traits = traits ?? button.traitCollection
        let bold = traits.legibilityWeight == .bold
        var parts: [Part] = [], opacities: [CGFloat] = [], lineHeights: [CGFloat?] = [], dotted = false, typed = false, gap: CGFloat?
        func pair(_ node: NodeView, _ key: String) -> [[Double]] {
            [node.undimmedChannels(key, dark: false) ?? [0, 0, 0, 1], node.undimmedChannels(key, dark: true) ?? [1, 1, 1, 1]]
        }
        // A childless box (often a flat leaf, a layer and no view,
        // FlatLeavesIOS) with no authored size in points has none to draw.
        // Its fill resolves in `host`'s traits and names the tint by
        // `host`'s, undimmed, as `pair` reads its colours: the face is
        // baked and kept (TintDimmingIOS.swift).
        func dot(_ style: NodeStyle, _ alpha: CGFloat, host: NodeView) {
            guard let width = style["width"]?.number, let height = style["height"]?.number, width > 0, height > 0,
                  let fill = style["background_color"] else { return }
            let size = CGSize(width: width, height: height)
            dotted = true
            let rect = CGRect(origin: .zero, size: size)
            let tint = host.undimmedTint(for: fill), contrast = host.drawsHighContrast, elevated = host.drawsElevated
            let sides = [false, true].map { fill.channels(dark: $0, contrast: contrast, elevated: elevated, tint: tint) ?? [0, 0, 0, 0] }
            parts.append(.dot(size, sides,
                              BorderPaint.reduced(BorderPaint.radii(style, in: rect), in: rect)))
            opacities.append(alpha)
            lineHeights.append(nil)
        }
        // `inked`: an ancestor up to the button wrote the colour its text inherits.
        func walk(_ node: NodeView, fonted: Bool, inked: Bool, alpha: CGFloat) {
            let presenter = node.presenter
            let ids = presenter?.flats.children(of: node.id) ?? node.container.subviews.compactMap { ($0 as? NodeView)?.id }
            let kids: [Either] = ids.compactMap { id in
                if let leaf = presenter?.flats.leaves[id] { return leaf.style["display"]?.string == "none" ? nil : .leaf(leaf) }
                guard let view = presenter?.views[id], view.style["display"]?.string != "none" else { return nil }
                return .view(view)
            }
            if kids.count > 1, gap == nil, let g = node.style["column_gap"]?.number { gap = CGFloat(g) }
            for kid in kids {
                guard case .view(let child) = kid else {
                    if case .leaf(let leaf) = kid { visitLeaf(leaf, in: presenter, alpha: alpha * CGFloat(leaf.opacity), host: node) }
                    continue
                }
                visit(child, fonted: fonted, inked: inked, alpha: alpha * child.alpha)
            }
        }
        // A flat box with children is walked as a view with them is.
        func visitLeaf(_ leaf: FlatLeaf, in presenter: Presenter?, alpha: CGFloat, host: NodeView) {
            let kids = (presenter?.flats.children(of: leaf.id) ?? []).compactMap { presenter?.flats.leaves[$0] }
            guard !kids.isEmpty else { dot(leaf.style, alpha, host: host); return }
            for kid in kids where kid.style["display"]?.string != "none" { visitLeaf(kid, in: presenter, alpha: alpha * CGFloat(kid.opacity), host: host) }
        }
        func visit(_ child: NodeView, fonted: Bool, inked: Bool, alpha: CGFloat) {
            let fonted = fonted || ["font_family", "font_size", "font_weight"].contains(where: child.wrote)
            let inked = inked || child.wrote("text_color")
            if child.isParagraph {
                let text = child.accessibleText
                guard !text.isEmpty else { return }
                typed = typed || fonted
                // Bold Text's step is in the weight already: the kernel
                // weights the system font and leaves a shipped one alone
                // (LLP 1115.000 D12.3).
                let family = Int(child.number("font_family"))
                let weight = Int(child.number("font_weight", 400))
                let font = readout && !fonted ? UIFont.preferredFont(forTextStyle: .body, compatibleWith: traits)
                    : engine.font(size: child.number("font_size", PageFacts.defaultRootFontSize), weight: min(1000, weight),
                                  family: family, italic: child.style["font_style"]?.string == "italic")
                parts.append(.text(text, font, inked ? pair(child, "text_color") : nil))
                opacities.append(alpha)
                lineHeights.append(child.wrote("line_height") ? child.usedLineHeight : nil)
            } else if child.kind == "image", let name = child.faceSymbol {
                parts.append(.symbol(name, child.number("font_size", PageFacts.defaultRootFontSize), inked ? pair(child, "text_color") : nil))
                opacities.append(alpha)
                lineHeights.append(nil)
            } else if (child.presenter?.flats.children(of: child.id) ?? child.container.subviews.compactMap { ($0 as? NodeView)?.id }).isEmpty,
                      child.style["background_color"] != nil {
                dot(child.style, alpha, host: child)
            } else {
                walk(child, fonted: fonted, inked: inked, alpha: alpha)
            }
        }
        // A readout is visited whole: one text (`text "Saved" role="status"`)
        // or a dot alone is its face, and its own opacity is its parts'.
        if readout { visit(button, fonted: false, inked: false, alpha: button.alpha) }
        else { walk(button, fonted: ["font_family", "font_size", "font_weight"].contains(where: button.wrote), inked: button.wrote("text_color"), alpha: 1) }
        guard !parts.isEmpty, readout || dotted || typed else { return nil }
        self.parts = parts
        self.opacities = opacities
        self.lineHeights = lineHeights
        self.gap = gap ?? 0
        dark = button.drawsDark
        self.bold = bold
    }

    /// Everything the face is drawn from (a font by its name and size).
    var source: String {
        parts.map { part -> String in
            switch part {
            case .dot(let size, let fill, let corners): return "d\(size.width)x\(size.height)\(fill)\(corners.map { "\($0.width),\($0.height)" })"
            case .symbol(let name, let points, let tint): return "s\(name)\(points)\(tint)"
            case .text(let text, let font, let color): return "t\(text)\(font.fontName)\(font.pointSize)\(color)"
            }
        }.joined(separator: "|") + "|\(opacities)|\(lineHeights)|\(gap)|\(dark)|\(bold)"
    }

    /// The face drawn, its unwritten ink the bar's (`tint`, what a plain
    /// item's title is drawn in before iOS 26; the label colour after).
    func image(tint: UIColor) -> UIImage {
        var bar = tint
        if #available(iOS 26.0, tvOS 26.0, *) { bar = .label }
        return draw(bar: bar.resolvedColor(with: UITraitCollection(userInterfaceStyle: dark ? .dark : .light))).withRenderingMode(.alwaysOriginal)
    }

    private func draw(bar: UIColor) -> UIImage {
        let ink = { (c: [[Double]]?) in c.map { TextEngine.color($0[dark ? 1 : 0]) } ?? bar }
        let drawn: [(CGSize, (CGRect) -> Void)] = parts.map { part in
            switch part {
            case .dot(let size, let fill, let corners):
                return (size, { rect in
                    ink(fill).setFill()
                    UIBezierPath(cgPath: BorderPaint.roundedRect(rect, corners, shape: nil)).fill()
                })
            case .symbol(let name, let points, let tint):
                let glyph = FaceSymbol.image(name, points: points, configuration: UIImage.SymbolConfiguration(pointSize: points))?.withTintColor(ink(tint), renderingMode: .alwaysOriginal)
                return (glyph?.size ?? .zero, { rect in glyph?.draw(in: rect) })
            case .text(let text, let font, let color):
                let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: ink(color)]
                let size = (text as NSString).size(withAttributes: attrs)
                return (CGSize(width: ceil(size.width), height: ceil(size.height)), { rect in (text as NSString).draw(at: rect.origin, withAttributes: attrs) })
            }
        }
        let height = drawn.map(\.0.height).max() ?? 0
        let width = drawn.map(\.0.width).reduce(0, +) + gap * CGFloat(max(0, drawn.count - 1))
        return UIGraphicsImageRenderer(size: CGSize(width: max(1, width), height: max(1, height))).image { context in
            var x: CGFloat = 0
            for ((size, paint), alpha) in zip(drawn, opacities) {
                context.cgContext.saveGState()
                context.cgContext.setAlpha(alpha)
                paint(CGRect(x: x, y: (height - size.height) / 2, width: size.width, height: size.height))
                context.cgContext.restoreGState()
                x += size.width + gap
            }
        }
    }
}
#endif

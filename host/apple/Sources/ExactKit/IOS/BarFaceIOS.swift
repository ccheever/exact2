// A bar item's face as its author painted it (LLP 1115 §2: what the author
// writes wins). A header button's text becomes UIKit's title in the system
// font, which is the platform's answer when the author said nothing more. A
// button whose face says more, a status dot beside its label or a label in a
// font of the author's (a "● Clock" chip in 12-point monospace), keeps that
// face: its parts drawn in a row, in their own fonts, colours and corners,
// in the page's scheme, as the item's image (`.alwaysOriginal`), as a hand-built
// bar's custom view would draw them. The header a bar replaces is laid out as
// `display: none`, so the parts are read from their styles, not their frames.
#if os(iOS) || os(tvOS)
import UIKit

struct ChipFace: Equatable {
    enum Part: Equatable {
        /// A childless filled box: its size, its fill (light, dark) and corners.
        case dot(CGSize, [[Double]], [CGSize])
        /// A system symbol at a point size, in its tint (light, dark).
        case symbol(String, CGFloat, [[Double]])
        /// A line of text in its font, in its colour (light, dark).
        case text(String, UIFont, [[Double]])
    }
    let parts: [Part]
    let gap: CGFloat
    /// The scheme it is drawn in. A bar item sized from one variant of a
    /// light/dark image asset kept that size for the other (a dark capsule
    /// three times the face's width, iOS 27), so the face is drawn for the
    /// scheme the page is in and drawn again when that changes.
    let dark: Bool

    /// The face of `button`, or nil when what it shows is one symbol or one
    /// line of text in the platform's type: the bar's own title or image.
    private enum Either { case view(NodeView), leaf(FlatLeaf) }

    init?(_ button: NodeView, engine: TextEngine? = nil) {
        guard !button.isNativeButton, let engine = engine ?? button.text else { return nil }
        var parts: [Part] = [], dotted = false, typed = false, gap: CGFloat?
        func pair(_ node: NodeView, _ key: String) -> [[Double]] {
            [node.channels(key, dark: false) ?? [0, 0, 0, 1], node.channels(key, dark: true) ?? [1, 1, 1, 1]]
        }
        // A childless box is a flat leaf, a layer and no view (FlatLeavesIOS),
        // which is sent its paint and not its size: a round dot is as wide as
        // its corners say, any other 8 points.
        func dot(_ style: NodeStyle) {
            let corners = ["top_left", "top_right", "bottom_right", "bottom_left"].compactMap { style["border_radius_\($0)"]?.number }
            let round = corners.count == 4 && Set(corners).count == 1 ? CGFloat(corners[0]) * 2 : nil
            let number = { (key: String) in style[key]?.number.map { CGFloat($0) } ?? round ?? 8 }
            let size = CGSize(width: number("width"), height: number("height"))
            guard size.width > 0, size.height > 0, let fill = style["background_color"] else { return }
            dotted = true
            let rect = CGRect(origin: .zero, size: size)
            parts.append(.dot(size, [fill.channels(dark: false) ?? [0, 0, 0, 0], fill.channels(dark: true) ?? [0, 0, 0, 0]],
                              BorderPaint.reduced(BorderPaint.radii(style, in: rect), in: rect)))
        }
        func walk(_ node: NodeView, fonted: Bool) {
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
                    if case .leaf(let leaf) = kid { dot(leaf.style) }
                    continue
                }
                let fonted = fonted || ["font_family", "font_size", "font_weight"].contains(where: child.wrote)
                if child.isParagraph {
                    let text = child.accessibleText
                    guard !text.isEmpty else { continue }
                    typed = typed || fonted
                    let font = engine.font(size: child.number("font_size", PageFacts.defaultRootFontSize), weight: Int(child.number("font_weight", 400)),
                                           family: Int(child.number("font_family")), italic: child.style["font_style"]?.string == "italic")
                    parts.append(.text(text, font, pair(child, "text_color")))
                } else if child.kind == "image", let name = child.props["symbolName"], !name.isEmpty {
                    parts.append(.symbol(name, child.number("font_size", PageFacts.defaultRootFontSize), pair(child, "tint_color")))
                } else if child.container.subviews.isEmpty, child.style["background_color"] != nil {
                    dot(child.style)
                } else {
                    walk(child, fonted: fonted)
                }
            }
        }
        walk(button, fonted: ["font_family", "font_size", "font_weight"].contains(where: button.wrote))
        guard !parts.isEmpty, dotted || typed else { return nil }
        self.parts = parts
        self.gap = gap ?? 6
        dark = button.drawsDark
    }

    /// Everything the face is drawn from (a font by its name and size).
    var source: String {
        parts.map { part -> String in
            switch part {
            case .dot(let size, let fill, let corners): return "d\(size.width)x\(size.height)\(fill)\(corners.map { "\($0.width),\($0.height)" })"
            case .symbol(let name, let points, let tint): return "s\(name)\(points)\(tint)"
            case .text(let text, let font, let color): return "t\(text)\(font.fontName)\(font.pointSize)\(color)"
            }
        }.joined(separator: "|") + "|\(gap)|\(dark)"
    }

    var image: UIImage { draw(dark: dark).withRenderingMode(.alwaysOriginal) }

    private func draw(dark: Bool) -> UIImage {
        let ink = { (c: [[Double]]) in TextEngine.color(c[dark ? 1 : 0]) }
        let drawn: [(CGSize, (CGRect) -> Void)] = parts.map { part in
            switch part {
            case .dot(let size, let fill, let corners):
                return (size, { rect in
                    ink(fill).setFill()
                    UIBezierPath(cgPath: BorderPaint.roundedRect(rect, corners, shape: nil)).fill()
                })
            case .symbol(let name, let points, let tint):
                let glyph = UIImage(systemName: name, withConfiguration: UIImage.SymbolConfiguration(pointSize: points))?.withTintColor(ink(tint), renderingMode: .alwaysOriginal)
                return (glyph?.size ?? .zero, { rect in glyph?.draw(in: rect) })
            case .text(let text, let font, let color):
                let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: ink(color)]
                let size = (text as NSString).size(withAttributes: attrs)
                return (CGSize(width: ceil(size.width), height: ceil(size.height)), { rect in (text as NSString).draw(at: rect.origin, withAttributes: attrs) })
            }
        }
        let height = drawn.map(\.0.height).max() ?? 0
        let width = drawn.map(\.0.width).reduce(0, +) + gap * CGFloat(max(0, drawn.count - 1))
        return UIGraphicsImageRenderer(size: CGSize(width: max(1, width), height: max(1, height))).image { _ in
            var x: CGFloat = 0
            for (size, paint) in drawn {
                paint(CGRect(x: x, y: (height - size.height) / 2, width: size.width, height: size.height))
                x += size.width + gap
            }
        }
    }
}
#endif

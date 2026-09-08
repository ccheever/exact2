// @ref LLP 1033: paint the same ordered inline runs the kernel measures.
import Foundation

extension NodeView {
    var isParagraph: Bool { kind == "text" && textParent == nil && (superview as? NodeView)?.kind != "text" }

    /// Inline nodes retain identity in the presenter map, but only their paragraph
    /// is mounted in the native hierarchy. Their styles and text are run data.
    func setTextChildren(_ children: [NodeView]) {
        for child in textChildren where child.textParent === self { child.textParent = nil }
        textChildren = children
        for child in children {
            child.removeFromSuperview()
            child.textParent = self
        }
        invalidateText()
    }

    var paragraphOwner: NodeView { textParent?.paragraphOwner ?? self }

    func paragraphLayout() -> Paragraph? {
        if let cached = cachedTextLayout, cached.width == bounds.width { return cached.paragraph }
        guard let paragraph = text?.paragraph(paragraphSpec(), width: bounds.width) else { return nil }
        cachedTextLayout = (bounds.width, paragraph)
        return paragraph
    }

    func invalidateText() {
        cachedTextSpec = nil
        cachedTextLayout = nil
    }

    func paragraphSpec() -> Spec {
        if let spec = cachedTextSpec { return spec }
        let align: Int
        switch style["text_align"] as? String {
        case "center": align = 1
        case "right": align = 2
        case "justify": align = 3
        default: align = 0
        }
        var runs: [Run] = []
        // An inline run is not in the view hierarchy — its paragraph owns it
        // and it was removed from any superview — so it has no appearance of
        // its own to read. It inherits the paragraph's, which is the one
        // actually on screen. @ref LLP 1034 D2
        let night = drawsDark
        func collect(_ node: NodeView) {
            if let value = node.props["text"] {
                runs.append(Run(text: value, size: node.number("font_size", 16),
                    weight: Int(node.number("font_weight", 400)), family: Int(node.number("font_family")),
                    italic: (node.style["font_style"] as? String) == "italic",
                    lineHeight: node.number("line_height"), letterSpacing: node.number("letter_spacing"),
                    color: node.channels("text_color", dark: night),
                    decoration: node.style["text_decoration_line"] as? String ?? "",
                    href: node.props["href"] ?? ""))
            } else {
                for child in node.textChildren where child.kind == "text" { collect(child) }
            }
        }
        collect(self)
        let spec = Spec(runs: runs, align: align, lineClamp: Int(number("line_clamp")),
                        color: channels("text_color", dark: night) ?? [0, 0, 0, 255])
        cachedTextSpec = spec
        return spec
    }
}

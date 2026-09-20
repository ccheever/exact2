// @ref LLP 1033: paint the same ordered inline runs the kernel measures.
import Foundation

extension NodeView {
    /// Resolve the computed tagged row after inheritance, using this node's font.
    var usedLineHeight: CGFloat? {
        if let ratio = style["line_height"] as? Double { return CGFloat(ratio) * number("font_size", 16) }
        if let length = style["line_height"] as? String, length.hasSuffix("px"), let n = Double(length.dropLast(2)) { return CGFloat(n) }
        return nil
    }

    func textRun(_ value: String) -> Run {
        Run(text: value, size: number("font_size", 16), weight: Int(number("font_weight", 400)),
            family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic",
            lineHeight: usedLineHeight, letterSpacing: number("letter_spacing"))
    }

    var isParagraph: Bool { kind == "text" && textParent == nil && (superview as? NodeView)?.kind != "text" }

    /// Inline nodes retain identity in the presenter map, but only their paragraph
    /// is mounted in the native hierarchy. Their styles and text are run data.
    func setTextChildren(_ children: [NodeView]) {
        for child in textChildren where child.textParent === self { child.textParent = nil }
        textChildren = children
        for child in children {
            child.removeFromSuperview()
            child.textParent = self
            #if os(macOS)
            child.wantsLayer = false
            #endif
        }
        invalidateText()
    }

    var paragraphOwner: NodeView { textParent?.paragraphOwner ?? self }

    func paragraphLayout() -> Paragraph? {
        #if os(macOS)
        if presenter?.session?.regions.owns(self) == true { return nil }
        #endif
        // The kernel measures the CSS content box; borders and padding must
        // not become extra wrapping room when that paragraph is painted.
        let width = contentBox().width
        if let cached = cachedTextLayout, cached.width == width { return cached.paragraph }
        guard let paragraph = text?.paragraph(paragraphSpec(), width: width, flow: flowShapes.map { $0.translated(CGPoint(x: -contentBox().minX, y: -contentBox().minY)) }) else { return nil }
        if paragraph.flowIncomplete { presenter?.session?.log("wrap-flow: text #\(id) is incomplete and uses ordinary layout") }
        cachedTextLayout = (width, paragraph)
        text?.accepted(paragraph)
        return paragraph
    }

    // @ref LLP 1043.000 §3 D4 — invalidate this view, including cached layer ink.
    func applyFlow(_ wire: [[String: Any]]) {
        flowShapes = wire.compactMap(TextFlowShape.init)
        cachedTextLayout = nil
        #if os(macOS)
        needsDisplay = true
        layer?.setNeedsDisplay()
        #else
        setNeedsDisplay()
        layer.setNeedsDisplay()
        #endif
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
                    lineHeight: node.usedLineHeight, letterSpacing: node.number("letter_spacing"),
                    color: node.channels("text_color", dark: night),
                    decoration: node.style["text_decoration_line"] as? String ?? "",
                    href: node.props["href"] ?? ""))
            } else {
                for child in node.textChildren where child.kind == "text" { collect(child) }
            }
        }
        collect(self)
        let spec = Spec(runs: runs, align: align, lineClamp: Int(number("line_clamp")),
                        color: channels("text_color", dark: night) ?? [0, 0, 0, 255],
                        overflowWrap: style["overflow_wrap"] as? String == "anywhere" ? 2 : style["overflow_wrap"] as? String == "break-word" ? 1 : 0, direction: style["direction"] as? String == "rtl" ? 1 : 0, whiteSpace: style["white_space"] as? String == "pre-wrap" ? 1 : 0, strut: textRun(""))
        cachedTextSpec = spec
        return spec
    }
}

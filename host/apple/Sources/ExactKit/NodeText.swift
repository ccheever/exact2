// @ref LLP 1033: paint the same ordered inline runs the kernel measures.
import Foundation

extension NodeView {
    /// Resolve the computed tagged row after inheritance, using this node's font.
    var usedLineHeight: CGFloat? {
        // JSON carries shortest f32 decimals; the measurement ABI carries f32
        // values. Resolve in the kernel's precision before widening for CoreText.
        if let ratio = style["line_height"]?.number { return CGFloat(Float(ratio) * Float(number("font_size", 16))) }
        if let length = style["line_height"]?.string, length.hasSuffix("px"), let n = Float(length.dropLast(2)) { return CGFloat(n) }
        return nil
    }

    func textRun(_ value: String) -> Run {
        Run(text: value, size: CGFloat(Float(number("font_size", 16))), weight: Int(number("font_weight", 400)),
            family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic",
            lineHeight: usedLineHeight, letterSpacing: CGFloat(Float(number("letter_spacing"))))
    }

    var isParagraph: Bool { kind == "text" }
    var paragraphOwner: NodeView { self }

    func paragraphLayout() -> Paragraph? {
        #if os(macOS)
        if readerParagraph != nil || presenter?.session?.regions.owns(self) == true { return nil }
        #endif
        // The kernel measures the CSS content box; borders and padding must
        // not become extra wrapping room when that paragraph is painted.
        let width = contentBox().width
        if textLayoutValid, let cached = cachedTextLayout, cached.width == width { return cached.paragraph }
        guard let paragraph = text?.paragraph(paragraphSpec(), width: width, flow: flowShapes.map { $0.translated(CGPoint(x: -contentBox().minX, y: -contentBox().minY)) }) else { return nil }
        if paragraph.flowIncomplete { presenter?.session?.log("wrap-flow: text #\(id) is incomplete and uses ordinary layout") }
        cachedTextLayout = (width, paragraph)
        textLayoutValid = true
        text?.accepted(paragraph)
        return paragraph
    }

    // @ref LLP 1043.000 §3 D4 — invalidate this view, including cached layer ink.
    func applyFlow(_ wire: [[String: Any]]) {
        // Worker surfaces encode ordinary lines only. Clear their key before
        // changing eligibility, including when an exclusion is removed.
        dropTextRaster()
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
        #if os(macOS)
        readerParagraph?.invalidatePaint()
        #endif
        cachedTextSpec = nil
        // Keep the previous accepted geometry alive through the next lookup.
        // A paint-only revision can reuse its ranges without breaking again.
        textLayoutValid = false
        // The old pixels stay up until the new ones replace them.
        textRasterKey = nil
        #if os(macOS)
        textRasterPending = false
        #else
        presenter?.requestTextPublication()
        #endif
    }

    func paragraphSpec() -> Spec {
        if let spec = cachedTextSpec { return spec }
        // Physical, as the kernel's paragraph is: CSS's initial `start` and
        // `end` follow `direction` (LLP 1053).
        let rtl = style["direction"]?.string == "rtl"
        let align: Int
        switch style["text_align"]?.string {
        case "left": align = 0
        case "center": align = 1
        case "right": align = 2
        case "justify": align = 3
        case "end": align = rtl ? 0 : 2
        default: align = rtl ? 2 : 0 // `start`
        }
        var runs: [Run] = []
        // An inline run is not in the view hierarchy — its paragraph owns it
        // and it was removed from any superview — so it has no appearance of
        // its own to read. It inherits the paragraph's, which is the one
        // actually on screen. @ref LLP 1034 D2
        let night = drawsDark
        if props["markup"] == "markdown", let source = props["text"] {
            // Markdown source: the archive expands it into runs, the same
            // expansion the measurer used (LLP 1045 D3).
            runs = MarkupRuns.expand(source, base: textRun(""), color: channels("text_color", dark: night))
        } else if let value = props["text"] {
            runs.append(InlineText.run(value, style: style, href: props["href"] ?? "", dark: night))
        } else {
            runs = inlineText.filter(\.paints).map { $0.run(dark: night) }
            // A container's background covers its descendants' fragments (CSS).
            if inlineText.contains(where: { !$0.paints && $0.run(dark: night).background != nil }) {
                let byId = Dictionary(inlineText.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
                for (i, leaf) in inlineText.filter(\.paints).enumerated() where runs[i].background == nil {
                    var up = byId[leaf.parent]
                    while let run = up, runs[i].background == nil { runs[i].background = run.run(dark: night).background; up = byId[run.parent] }
                }
            }
        }
        let spec = Spec(runs: runs, align: align, lineClamp: Int(number("line_clamp")),
                        color: channels("text_color", dark: night) ?? [0, 0, 0, 255],
                        overflowWrap: style["overflow_wrap"]?.string == "anywhere" ? 2 : style["overflow_wrap"]?.string == "break-word" ? 1 : 0, direction: rtl ? 1 : 0, whiteSpace: style["white_space"]?.string == "pre-wrap" ? 1 : 0, strut: textRun(""))
        cachedTextSpec = spec
        return spec
    }
}

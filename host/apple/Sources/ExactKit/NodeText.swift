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
            lineHeight: usedLineHeight, letterSpacing: CGFloat(Float(number("letter_spacing"))),
            numeric: Int(number("font_variant_numeric")))
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

    /// CSS start/end resolve from direction, for paragraphs and native editors.
    var textAlignmentCode: Int {
        let rtl = style["direction"]?.string == "rtl"
        switch style["text_align"]?.string {
        case "left": return 0
        case "center": return 1
        case "right": return 2
        case "justify": return 3
        case "end": return rtl ? 0 : 2
        default: return rtl ? 2 : 0
        }
    }
    func paragraphSpec() -> Spec {
        if let spec = cachedTextSpec { return spec }
        let align = textAlignmentCode, rtl = style["direction"]?.string == "rtl"
        var runs: [Run] = []
        // An inline run is not in the view hierarchy — its paragraph owns it
        // and it was removed from any superview — so it has no appearance of
        // its own to read. It inherits the paragraph's, which is the one
        // actually on screen. @ref LLP 1034 D2
        let night = drawsDark, contrast = drawsHighContrast, elevated = drawsElevated
        // The view's own tint, read once, where anything here names it (LLP 1095 D8).
        let tint = style.values.contains(where: \.namesTint) || inlineText.contains(where: \.namesTint) ? viewTint : nil
        if props["markup"] == "markdown", let source = props["text"] {
            // Markdown source: the archive expands it into runs, the same
            // expansion the measurer used (LLP 1045 D3).
            runs = MarkupRuns.expand(source, base: textRun(""), color: channels("text_color", dark: night))
            // `currentcolor` in a shadow or stroke is each piece's own colour.
            let rows = RunPaintRows(style), own = channels("text_color", dark: night) ?? SystemColor.canvasTextChannels(dark: night, contrast: contrast)
            for i in runs.indices { (runs[i].shadow, runs[i].stroke) = rows.resolve(dark: night, contrast: contrast, elevated: elevated, tint: tint, color: runs[i].color ?? own) }
        } else if let value = props["text"] {
            runs.append(InlineText.run(value, style: style, href: props["href"] ?? "", dark: night, contrast: contrast, elevated: elevated, tint: tint))
        } else {
            runs = inlineText.filter(\.paints).map { $0.run(dark: night, contrast: contrast, elevated: elevated, tint: tint) }
            // A container's background covers its descendants' fragments (CSS).
            if inlineText.contains(where: { !$0.paints && $0.run(dark: night, contrast: contrast, elevated: elevated, tint: tint).background != nil }) {
                let byId = Dictionary(inlineText.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
                for (i, leaf) in inlineText.filter(\.paints).enumerated() where runs[i].background == nil {
                    var up = byId[leaf.parent]
                    while let run = up, runs[i].background == nil { runs[i].background = run.run(dark: night, contrast: contrast, elevated: elevated, tint: tint).background; up = byId[run.parent] }
                }
            }
        }
        let whiteSpace = ["pre-wrap": 1, "nowrap": 2, "pre-line": 3, "pre": 4][style["white_space"]?.string ?? ""] ?? 0
        // CSS collapses white space before shaping, as the measurer does in
        // Rust with the same function (LLP 1053 G5); Markdown keeps its lines.
        let source = whiteSpace == 1 || whiteSpace == 4 || props["markup"] == "markdown" ? SourceMap() : SourceMap.collapse(&runs, whiteSpace: whiteSpace)
        let lineClamp = Int(number("line_clamp"))
        // `text-overflow: ellipsis` applies to a box that clips its inline overflow.
        let clips = (style["overflow_x"]?.string).map { $0 != "visible" } ?? false
        var spec = Spec(runs: runs, align: align, lineClamp: lineClamp,
                        color: channels("text_color", dark: night) ?? SystemColor.canvasTextChannels(dark: night, contrast: contrast),
                        overflowWrap: style["overflow_wrap"]?.string == "anywhere" ? 2 : style["overflow_wrap"]?.string == "break-word" ? 1 : 0, direction: rtl ? 1 : 0, whiteSpace: whiteSpace, strut: textRun(""))
        spec.ellipsis = lineClamp == 0 && clips && style["text_overflow"]?.string == "ellipsis"
        spec.source = source
        spec.textIndent = CGFloat(Float(number("text_indent")))
        spec.hyphens = ["none": 1, "auto": 2][style["hyphens"]?.string ?? ""] ?? 0
        // The kernel's measurer hyphenates by the same document language.
        if spec.hyphens == 2 { spec.language = presenter?.documentLanguage ?? "" }
        spec.hyphenateAuto()
        // One shadow over the paragraph when its runs agree (LLP 1077 D3).
        spec.gatherShadows()
        cachedTextSpec = spec
        return spec
    }

    /// A shaped-text offset in the node's source text, and back (LLP 1053 G5).
    func sourceOffset(_ shaped: Int) -> Int { paragraphSpec().source.source(shaped) }
    func shapedOffset(_ source: Int) -> Int { paragraphSpec().source.collapsed(source) }
}

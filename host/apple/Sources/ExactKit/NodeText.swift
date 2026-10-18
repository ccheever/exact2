// @ref LLP 1033: paint the same ordered inline runs the kernel measures.
import Foundation

extension NodeView {
    /// Resolve the computed tagged row after inheritance, using this node's font.
    package var usedLineHeight: CGFloat? {
        // JSON carries shortest f32 decimals; the measurement ABI carries f32
        // values. Resolve in the kernel's precision before widening for CoreText.
        if let ratio = style["line_height"]?.number { return CGFloat(Float(ratio) * Float(number("font_size", PageFacts.defaultRootFontSize))) }
        if let length = style["line_height"]?.string, length.hasSuffix("px"), let n = Float(length.dropLast(2)) { return CGFloat(n) }
        return nil
    }

    func textRun(_ value: String) -> Run {
        TextStyle.run(value, style: style, defaultSize: PageFacts.defaultRootFontSize)
    }

    var isParagraph: Bool { kind == "text" }
    var paragraphOwner: NodeView { self }

    func paragraphLayout() -> Paragraph? {
        #if os(macOS)
        if readerParagraph != nil || presenter?.session?.regions.owns(self) == true { return nil }
        #endif
        // The kernel measures the CSS content box; borders and padding must
        // not become extra wrapping room when that paragraph is painted.
        let width = paragraphBox().width
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
        if let presenter, presenter.applying { presenter.changedText.insert(id) }
        presenter?.requestTextPublication()
        #endif
    }

    /// CSS start/end resolve from direction, for paragraphs and native editors.
    var textAlignmentCode: Int {
        TextStyle.alignment(style)
    }
    func paragraphSpec() -> Spec {
        if let spec = cachedTextSpec { return spec }
        #if os(macOS)
        // In a table's selected row that AppKit draws emphasized, a role's ink is the selected item's (TableInkMac.swift).
        let outer = SystemColor.selectedItem
        SystemColor.selectedItem = inEmphasizedRow
        defer { SystemColor.selectedItem = outer }
        #endif
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
            // Every piece is in the node's ink, or a share of it: one kind.
            var base = textRun("")
            base.fixedInk = style["text_color"]?.fixesInk == true
            runs = MarkdownLink.installed?.expand(source, base: base, color: textChannels("text_color", dark: night),
                                                  boldText: text?.boldTextSteps(base.family) == true) ?? [textRun(source)]
            if style["visibility"]?.string == "hidden" { for i in runs.indices { runs[i].hidden = true } }
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
                    while let run = up, runs[i].background == nil {
                        let paint = run.run(dark: night, contrast: contrast, elevated: elevated, tint: tint)
                        if !paint.hidden { runs[i].background = paint.background }
                        up = byId[run.parent]
                    }
                }
            }
        }
        var spec = TextStyle.spec(runs: runs, style: style, props: props, strut: textRun(""),
                                  color: textChannels("text_color", dark: night) ?? SystemColor.canvasTextChannels(dark: night, contrast: contrast),
                                  language: presenter?.documentLanguage ?? "")
        // One shadow over the paragraph when its runs agree (LLP 1077 D3).
        spec.gatherShadows()
        cachedTextSpec = spec
        return spec
    }

    /// A shaped-text offset in the node's source text, and back (LLP 1053 G5).
    func sourceOffset(_ shaped: Int) -> Int { paragraphSpec().source.source(shaped) }
    func shapedOffset(_ source: Int) -> Int { paragraphSpec().source.collapsed(source) }
}

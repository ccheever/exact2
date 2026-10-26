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

    /// The node box this view stands in: the nearest node view above it,
    /// past the containers and scroll views a box holds its children in.
    var nodeSuperview: NodeView? {
        var at = superview
        while let v = at, !(v is NodeView) { at = v.superview }
        return at as? NodeView
    }

    /// The lines each box above this paragraph draws through its text and
    /// that box's colour, nearest first (CSS Text Decoration 3 §2.1: a block
    /// container's lines reach its in-flow children's text). A box out of
    /// flow or an atomic one (a button, a control) takes none from the boxes
    /// above it, so the walk stops there.
    func decoratingAncestors(dark: Bool) -> [(String, [Double]?)] {
        var out: [(String, [Double]?)] = []
        var box: NodeView = self
        // A popover is out of flow too (the kernel's top layer), unless sticky.
        while !["absolute", "fixed"].contains(box.layoutPosition),
              box === self || !(box.actsAsButton || box.kind == "control"),
              let parent = box.nodeSuperview {
            if let lines = parent.style["text_decoration_line"]?.string, !lines.isEmpty, lines != "none" {
                var colored: NodeView? = parent
                while let c = colored, c.style["text_color"] == nil { colored = c.nodeSuperview }
                out.append((lines, colored?.textChannels("text_color", dark: dark)))
            }
            box = parent
        }
        return out
    }

    /// After a style change: where this box's lines, or their colour,
    /// changed, the paragraphs it holds draw again.
    func decorationChanged(from old: NodeStyle) {
        let lines = style["text_decoration_line"], was = old["text_decoration_line"]
        guard lines != was || (lines != nil && old["text_color"] != style["text_color"]) else { return }
        var stack: [PlatformView] = subviews
        while let v = stack.popLast() {
            if let node = v as? NodeView, node.isParagraph {
                node.invalidateText()
                #if os(macOS)
                node.needsDisplay = true
                #else
                node.setNeedsDisplay()
                #endif
            }
            stack.append(contentsOf: v.subviews)
        }
    }
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
        let wasEmpty = flowShapes.isEmpty
        flowShapes = wire.compactMap(TextFlowShape.init)
        if wasEmpty != flowShapes.isEmpty { cachedTextSpec = nil }
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
        let ink = textChannels("text_color", dark: night) ?? SystemColor.canvasTextChannels(dark: night, contrast: contrast)
        if props["markup"] == "markdown", let source = props["text"] {
            // Markdown source: the archive expands it into runs, the same
            // expansion the measurer used (LLP 1045 D3).
            // Every piece is in the node's ink, or a share of it: one kind.
            var base = textRun("")
            base.fixedInk = style["text_color"]?.fixesInk == true
            let atoms = MarkdownAtoms(kinds: props["atoms"] ?? "", faces: props["atomFaces"] ?? "", styles: props["atomStyles"] ?? "", dark: night, contrast: contrast == true)
            runs = MarkdownLink.installed?.expand(source, base: base, color: textChannels("text_color", dark: night),
                                                  boldText: text?.boldTextSteps(base.family) == true, atoms: atoms) ?? [textRun(source)]
            if style["visibility"]?.string == "hidden" { for i in runs.indices { runs[i].hidden = true } }
            // `currentcolor` in a shadow or stroke is each piece's own colour.
            let rows = RunPaintRows(style), own = channels("text_color", dark: night) ?? SystemColor.canvasTextChannels(dark: night, contrast: contrast)
            for i in runs.indices { (runs[i].shadow, runs[i].stroke) = rows.resolve(dark: night, contrast: contrast, elevated: elevated, tint: tint, color: runs[i].color ?? own) }
        } else if let value = props["text"] {
            runs.append(InlineText.run(value, style: style, href: props["href"] ?? "", dark: night, contrast: contrast, elevated: elevated, tint: tint))
        } else {
            runs = inlineText.filter(\.paints).map { $0.run(dark: night, contrast: contrast, elevated: elevated, tint: tint) }
            let decorates = inlineText.contains(where: { !$0.paints && !$0.lightRun.decoration.isEmpty })
            let backs = inlineText.contains(where: { !$0.paints && $0.run(dark: night, contrast: contrast, elevated: elevated, tint: tint).background != nil })
            let byId = decorates || backs ? Dictionary(inlineText.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a }) : [:]
            // An inline container's lines reach its descendants' text, as the
            // paragraph's own do below, whatever the box's own visibility:
            // a hidden run paints neither its glyphs nor their lines.
            if decorates {
                for (i, leaf) in inlineText.filter(\.paints).enumerated() {
                    var up = byId[leaf.parent]
                    while let run = up {
                        up = byId[run.parent]
                        guard !run.lightRun.decoration.isEmpty else { continue }
                        let paint = run.run(dark: night, contrast: contrast, elevated: elevated, tint: tint)
                        runs[i].propagate(paint.decoration, color: paint.color ?? ink)
                    }
                }
            }
            // A container's background covers its descendants' fragments (CSS).
            if backs {
                // A leaf paints one background, its nearest, and a box's ends
                // are read from the characters that paint it: a box inside
                // another visible background, and that outer box, keep square
                // corners, which would otherwise show the page at their seams.
                let visible = { (c: [Double]?) in c.map { $0.count < 4 || $0[3] > 0 } ?? false }
                var square = Set<UInt32>()
                for (i, leaf) in inlineText.filter(\.paints).enumerated() {
                    var nested: [UInt32] = visible(runs[i].background) ? [runs[i].backgroundBox] : []
                    var up = byId[leaf.parent]
                    while let run = up {
                        up = byId[run.parent]
                        let paint = run.run(dark: night, contrast: contrast, elevated: elevated, tint: tint)
                        guard !paint.hidden, paint.background != nil else { continue }
                        if runs[i].background == nil {
                            runs[i].background = paint.background
                            (runs[i].backgroundBox, runs[i].backgroundRadii) = (paint.backgroundBox, paint.backgroundRadii)
                        }
                        if visible(paint.background), !nested.contains(paint.backgroundBox) { nested.append(paint.backgroundBox) }
                    }
                    if nested.count > 1 { square.formUnion(nested) }
                }
                for i in runs.indices where square.contains(runs[i].backgroundBox) { runs[i].backgroundRadii = [] }
            }
            // A box's identity is paint only where a box is rounded: square
            // backgrounds keep sharing what they share (TextPaint's keys).
            if !runs.contains(where: { $0.background != nil && $0.backgroundRadii.contains { !$0.isZero } }) {
                for i in runs.indices { runs[i].backgroundBox = 0; runs[i].backgroundRadii = [] }
            }
        }
        // The paragraph's own lines reach every run its children paint
        // (CSS Text Decoration 3 §2.1); its own text's run already has them.
        if props["text"] == nil || props["markup"] == "markdown", let lines = style["text_decoration_line"]?.string, !lines.isEmpty {
            for i in runs.indices { runs[i].propagate(lines, color: ink) }
        }
        var strut = InlineText.run("", style: style, href: props["href"] ?? "", dark: night,
                                   contrast: contrast, elevated: elevated, tint: tint)
        // So do the lines of the boxes it stands in, each in that box's colour.
        for (lines, color) in decoratingAncestors(dark: night) {
            for i in runs.indices { runs[i].propagate(lines, color: color ?? ink) }
            strut.propagate(lines, color: color ?? ink)
        }
        var spec = TextStyle.spec(runs: runs, style: style, props: props, strut: strut,
                                  color: ink,
                                  language: presenter?.documentLanguage ?? "", flow: !flowShapes.isEmpty)
        // One shadow over the paragraph when its runs agree (LLP 1077 D3).
        spec.gatherShadows()
        cachedTextSpec = spec
        return spec
    }

    /// A shaped-text offset in the node's source text, and back (LLP 1053 G5).
    func sourceOffset(_ shaped: Int) -> Int { paragraphSpec().source.source(shaped) }
    func shapedOffset(_ source: Int) -> Int { paragraphSpec().source.collapsed(source) }
}

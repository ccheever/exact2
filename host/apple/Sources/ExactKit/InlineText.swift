// @ref LLP 1044.000 §6 S1 — textual descendants are paragraph values.
import Foundation
import CoreText

struct InlineText {
    let id: UInt32
    let parent: UInt32
    let props: [String: String]
    private let lightRun: Run
    private let darkColor: [Double]?
    private let darkBackground: [Double]?
    private let colorRef: BatchValue?
    private let backgroundRef: BatchValue?
    private let paint: RunPaintRows
    let hasSchemeColor: Bool
    /// Whether a colour here is the view's tint (LLP 1095 D8).
    var namesTint: Bool { colorRef?.namesTint == true || backgroundRef?.namesTint == true || paint.namesTint }
    let handlers: Set<String>
    let paints: Bool
    var range: NSRange = NSRange(location: 0, length: 0)

    init(id: UInt32, parent: UInt32, props: [String: String], style: InlineStyle,
         handlers: Set<String>, paints: Bool) {
        self.id = id; self.parent = parent; self.props = props
        var value = style.run
        value.text = props["text"] ?? ""
        value.href = props["href"] ?? ""
        lightRun = value
        darkColor = style.darkColor
        darkBackground = style.darkBackground
        colorRef = style.colorRef; backgroundRef = style.backgroundRef
        paint = style.paint
        hasSchemeColor = style.paired || style.paint.paired
        self.handlers = handlers; self.paints = paints
    }

    var text: String { lightRun.text }
    /// The run in its paragraph owner's traits: `contrast` and `elevated`
    /// as `BatchValue.channels` takes them (LLP 1095 D5).
    func run(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil) -> Run {
        var value = lightRun
        if dark { value.color = darkColor; value.background = darkBackground }
        if let colorRef { value.color = colorRef.channels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) }
        if let backgroundRef { value.background = backgroundRef.channels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) }
        (value.shadow, value.stroke) = paint.resolve(dark: dark, contrast: contrast, elevated: elevated, tint: tint,
                                                     color: value.color ?? SystemColor.canvasTextChannels(dark: dark, contrast: contrast))
        return value
    }

    static func run(_ text: String, style: NodeStyle, href: String = "", dark: Bool, contrast: Bool? = nil, elevated: Bool = false,
                    tint: PlatformColor? = nil) -> Run {
        func number(_ key: String, _ fallback: Double = 0) -> Double { style[key]?.number ?? fallback }
        let size = Float(number("font_size", 16))
        let height: CGFloat?
        if let ratio = style["line_height"]?.number { height = CGFloat(Float(ratio) * size) }
        else if let px = style["line_height"]?.string, px.hasSuffix("px"), let value = Float(px.dropLast(2)) { height = CGFloat(value) }
        else { height = nil }
        var run = Run(text: text, size: CGFloat(size), weight: Int(number("font_weight", 400)),
                      family: Int(number("font_family")), italic: style["font_style"]?.string == "italic",
                      lineHeight: height, letterSpacing: CGFloat(Float(number("letter_spacing"))),
                      numeric: Int(number("font_variant_numeric")),
                      color: style["text_color"]?.channels(dark: dark, contrast: contrast, elevated: elevated, tint: tint),
                      decoration: style["text_decoration_line"]?.string ?? "", href: href)
        (run.shadow, run.stroke) = RunPaintRows(style).resolve(dark: dark, contrast: contrast, elevated: elevated, tint: tint,
                                                               color: run.color ?? SystemColor.canvasTextChannels(dark: dark, contrast: contrast))
        run.hidden = style["visibility"]?.string == "hidden"
        return run
    }
}

// Resolved while reading a paragraph; no dictionary of unused layout rows.
struct InlineStyle {
    var run = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
    var darkColor: [Double]?
    var darkBackground: [Double]?
    /// A platform colour by name, resolved when the run is read (LLP 1095 D5).
    var colorRef: BatchValue?
    var backgroundRef: BatchValue?
    /// `text-shadow` and `-webkit-text-stroke`, computed for the run (LLP 1077).
    var paint = RunPaintRows()
    var paired = false
}

extension Presenter {
    /// Values are located by logical id, but only their paragraph owns a view.
    func textHost(_ id: UInt32) -> NodeView? { views[id] ?? inlineOwners[id].flatMap { views[$0.owner] } }
    func inlineText(_ id: UInt32) -> InlineText? {
        guard let location = inlineOwners[id], let node = views[location.owner],
              node.inlineText.indices.contains(location.index) else { return nil }
        return node.inlineText[location.index]
    }
    func inlineEnabled(_ id: UInt32) -> Bool {
        guard let host = textHost(id), !host.inert, !host.disabled,
              let value = inlineText(id), value.props["disabled"] != "true" else { return false }
        var current: InlineText? = value
        while let run = current {
            if run.props["inert"] == "true" { return false }
            current = inlineText(run.parent)
        }
        return true
    }
    func forgetParagraph(_ node: NodeView) {
        for run in node.inlineText where inlineOwners[run.id]?.owner == node.id { inlineOwners.removeValue(forKey: run.id) }
        node.inlineText.removeAll()
    }
    func applyParagraph(_ id: UInt32, _ runs: [InlineText]) {
        guard let node = views[id] else { return }
        forgetParagraph(node)
        var offset = 0
        var rows = runs
        for i in rows.indices {
            let count = rows[i].paints ? rows[i].text.utf16.count : 0
            rows[i].range = NSRange(location: offset, length: count)
            offset += count
            inlineOwners[rows[i].id] = (id, i)
        }
        // Container runs expose the union of their descendants, including nested links.
        for i in rows.indices.reversed() {
            if let parent = inlineOwners[rows[i].parent], parent.owner == id {
                rows[parent.index].range.length = max(NSMaxRange(rows[parent.index].range), NSMaxRange(rows[i].range)) - rows[parent.index].range.location
            }
        }
        node.inlineText = rows
        node.invalidateText()
        node.updateInlineInteraction()
        node.updateTextAccessibility()
    }
}

extension NodeView {
    /// Hit the same CoreText geometry as selection, rejecting blank space after a line.
    func textOffset(at point: CGPoint) -> Int? {
        #if os(macOS)
        if let readerParagraph { return readerParagraph.offset(at: point, node: self) }
        #endif
        let box = contentBox()
        guard box.contains(point), let paragraph = paragraphLayout() else { return nil }
        let spec = paragraphSpec()
        guard let line = paragraph.lineIndex(at: CGPoint(x: point.x - box.minX, y: point.y - box.minY), align: spec.align, width: box.width) else { return nil }
        let x = box.minX + paragraph.origin(line, align: spec.align, width: box.width)
        let width = CGFloat(CTLineGetTypographicBounds(paragraph.lines[line], nil, nil, nil))
        guard point.x >= x, point.x <= x + width else { return nil }
        let offset = paragraph.stringIndex(in: line, at: point.x - x)
        return offset == kCFNotFound ? nil : offset
    }
    func inlineTarget(at point: CGPoint, handler: String? = nil) -> InlineText? {
        // Runs' ranges address the source; the hit, the shaped text (LLP 1053 G5).
        guard let offset = textOffset(at: point).map(sourceOffset) else { return nil }
        // Leaf lookup is logarithmic even in a paragraph with thousands of runs.
        var lo = 0, hi = inlineText.count
        while lo < hi {
            let mid = (lo + hi) / 2
            if inlineText[mid].range.location <= offset { lo = mid + 1 } else { hi = mid }
        }
        guard lo > 0 else { return nil }
        var run: InlineText? = inlineText[lo - 1]
        while let current = run {
            if NSLocationInRange(offset, current.range),
               handler.map({ current.handlers.contains($0) }) ?? (current.props["href"] != nil || current.paints) { return current }
            run = presenter?.inlineText(current.parent)
        }
        return nil
    }
    func inlineLink(at point: CGPoint) -> String? {
        var run = inlineTarget(at: point)
        while let current = run {
            if let href = current.props["href"], !href.isEmpty { return href }
            run = presenter?.inlineText(current.parent)
        }
        guard let offset = textOffset(at: point) else { return nil }
        var start = 0
        for run in paragraphSpec().runs {
            let end = start + run.text.utf16.count
            if offset >= start && offset < end { return run.href.isEmpty ? nil : run.href }
            start = end
        }
        return nil
    }
    func inlineActivationTarget(at point: CGPoint) -> InlineText? {
        var run = inlineTarget(at: point)
        while let current = run {
            if current.handlers.contains("press") || !(current.props["href"] ?? "").isEmpty { return current }
            run = presenter?.inlineText(current.parent)
        }
        return nil
    }
    var defaultLink: String? {
        guard kind == "button", !handlers.contains("press"), let url = props["href"], !url.isEmpty else { return nil }
        return url
    }
    func activateLink(_ url: String) -> Bool {
        guard let session = presenter?.session, !disabled, !inert else { return false }
        if url.hasPrefix("/") && !url.hasPrefix("//") { return session.navigate(url) }
        session.delegate?.exactSession(session, command: "openURL", args: [url]); return true
    }
    func activateInline(_ id: UInt32) -> Bool {
        guard let presenter, presenter.inlineEnabled(id), let run = presenter.inlineText(id) else { return false }
        if run.handlers.contains("press") { presenter.press(id); return true }
        // A route in the note navigates; anything else is the app's to open
        // (session.follow). A button's own `href` uses `activateLink`.
        if let url = run.props["href"], !url.isEmpty, let session = presenter.session { return session.follow(url) }
        return false
    }
}

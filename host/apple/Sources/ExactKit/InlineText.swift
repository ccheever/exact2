// @ref LLP 1044.000 §6 S1 — textual descendants are paragraph values.
import Foundation
import CoreText

struct InlineText {
    let id: UInt32
    let parent: UInt32
    let props: [String: String]
    let style: [String: Any]
    let handlers: Set<String>
    let paints: Bool
    var range: NSRange = NSRange(location: 0, length: 0)

    init?(_ wire: [String: Any]) {
        guard let id = wire["id"] as? Int, let parent = wire["parent"] as? Int else { return nil }
        self.id = UInt32(id); self.parent = UInt32(parent)
        props = wire["props"] as? [String: String] ?? [:]
        style = wire["style"] as? [String: Any] ?? [:]
        handlers = Set(wire["handlers"] as? [String] ?? [])
        paints = wire["paint"] as? Bool ?? false
    }

    var hasSchemeColor: Bool { style.values.contains { ($0 as? [[Double]])?.count == 2 } }
    func run(dark: Bool) -> Run { Self.run(props["text"] ?? "", style: style, href: props["href"] ?? "", dark: dark) }

    static func channels(_ value: Any?, dark: Bool) -> [Double]? {
        if let c = value as? [Double], c.count == 4 { return c }
        if let pair = value as? [[Double]], pair.count == 2, pair[dark ? 1 : 0].count == 4 { return pair[dark ? 1 : 0] }
        return nil
    }

    static func run(_ text: String, style: [String: Any], href: String = "", dark: Bool) -> Run {
        func number(_ key: String, _ fallback: Double = 0) -> Double { style[key] as? Double ?? fallback }
        let size = Float(number("font_size", 16))
        let height: CGFloat?
        if let ratio = style["line_height"] as? Double { height = CGFloat(Float(ratio) * size) }
        else if let px = style["line_height"] as? String, px.hasSuffix("px"), let value = Float(px.dropLast(2)) { height = CGFloat(value) }
        else { height = nil }
        return Run(text: text, size: CGFloat(size), weight: Int(number("font_weight", 400)),
                   family: Int(number("font_family")), italic: style["font_style"] as? String == "italic",
                   lineHeight: height, letterSpacing: CGFloat(Float(number("letter_spacing"))),
                   color: channels(style["text_color"], dark: dark),
                   decoration: style["text_decoration_line"] as? String ?? "", href: href)
    }
}

extension Presenter {
    /// Values are located by logical id, but only their paragraph owns a view.
    func textHost(_ id: UInt32) -> NodeView? { views[id] ?? inlineOwners[id].flatMap { views[$0.owner] } }
    func inlineText(_ id: UInt32) -> InlineText? {
        guard let location = inlineOwners[id], let node = views[location.owner],
              node.inlineText.indices.contains(location.index) else { return nil }
        return node.inlineText[location.index]
    }
    func forgetParagraph(_ node: NodeView) {
        for run in node.inlineText { inlineOwners.removeValue(forKey: run.id) }
        node.inlineText.removeAll()
    }
    func applyParagraph(_ id: UInt32, _ wire: [[String: Any]]) {
        guard let node = views[id] else { return }
        forgetParagraph(node)
        var offset = 0
        var rows = wire.compactMap(InlineText.init)
        for i in rows.indices {
            let count = rows[i].paints ? (rows[i].props["text"] ?? "").utf16.count : 0
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
        guard let offset = textOffset(at: point) else { return nil }
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
    func activateInline(_ id: UInt32) -> Bool {
        guard let presenter, !inert, !disabled, let run = presenter.inlineText(id), run.props["disabled"] != "true" else { return false }
        if run.handlers.contains("press") { presenter.press(id); return true }
        if let url = run.props["href"], let session = presenter.session {
            session.delegate?.exactSession(session, command: "openURL", args: [url]); return true
        }
        return false
    }
}

// @ref LLP 1033: selection is host state over the same CoreText lines that
// measure and paint. UTF-16 offsets match CoreText and NSString, including emoji.
#if os(macOS)
import AppKit
import CoreText

final class TextSelection {
    weak var presenter: Presenter?
    private weak var anchor: NodeView?
    private weak var focus: NodeView?
    private var anchorIndex = 0
    private var focusIndex = 0
    private var dragged = false
    private var ordered: [NodeView]?
    private var indices: [UInt32: Int] = [:]
    private var painted: [UInt32: NSRange] = [:]
    private struct Position {
        var key: String
        var row: Int
        var paragraph: Int
        var offset: Int
        var tuple: (String, Int, Int) { (key, paragraph, offset) }
        var order: (Int, Int, Int) { (row, paragraph, offset) }
    }
    private weak var list: NodeView?
    private var logicalAnchor: Position?
    private var logicalFocus: Position?
    private var allListText = false
    private var positions: [UInt32: Position] = [:]

    func structureChanged() {
        ordered = nil
        indices.removeAll(keepingCapacity: true)
        positions.removeAll(keepingCapacity: true)
        if let list, var a = logicalAnchor, var b = logicalFocus {
            if let ai = presenter?.onListIndex?(list.id, a.key), let bi = presenter?.onListIndex?(list.id, b.key) {
                a.row = ai; b.row = bi; logicalAnchor = a; logicalFocus = b
            } else {
                self.list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
                anchor = nil; focus = nil
            }
        }
    }

    init(_ presenter: Presenter) { self.presenter = presenter }

    var paragraphs: [NodeView] {
        if let ordered { return ordered }
        guard let presenter else { return [] }
        var result: [NodeView] = []
        func walk(_ view: NSView) {
            if view.isHidden { return }
            if let node = view as? NodeView, presenter.session?.regions.owns(node) == true { return }
            if let node = view as? NodeView, node.isParagraph { result.append(node); return }
            for child in view.subviews { walk(child) }
        }
        walk(presenter.root)
        ordered = result
        indices = Dictionary(uniqueKeysWithValues: result.enumerated().map { ($0.element.id, $0.offset) })
        return result
    }

    private func invalidate() {
        var next: [UInt32: NSRange] = [:]
        for node in paragraphs {
            let selected = range(node).flatMap { $0.length > 0 ? $0 : nil }
            if let selected { next[node.id] = selected }
            if selected != painted[node.id] { node.needsDisplay = true }
        }
        painted = next
    }

    func clear() {
        anchor = nil; focus = nil
        anchorIndex = 0; focusIndex = 0
        dragged = false
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        invalidate()
    }

    func begin(_ node: NodeView, event: NSEvent) {
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        anchor = node; focus = node
        anchorIndex = index(node, at: node.local(event.locationInWindow))
        focusIndex = anchorIndex
        dragged = false
        if event.clickCount >= 3 {
            anchorIndex = 0; focusIndex = length(node); dragged = true
        } else if event.clickCount == 2 {
            let text = node.paragraphSpec().runs.map(\.text).joined() as NSString
            if text.length > 0 {
                let at = min(anchorIndex, text.length - 1)
                var lo = at, hi = at
                func space(_ i: Int) -> Bool { CharacterSet.whitespacesAndNewlines.contains(UnicodeScalar(text.character(at: i)) ?? " ") }
                while lo > 0 && !space(lo - 1) { lo -= 1 }
                while hi < text.length && !space(hi) { hi += 1 }
                anchorIndex = lo; focusIndex = hi; dragged = true
            }
        }
        if let (owner, start) = position(node, offset: anchorIndex), let (_, end) = position(node, offset: focusIndex) {
            list = owner; logicalAnchor = start; logicalFocus = end
        }
        invalidate()
    }

    func drag(_ event: NSEvent) {
        guard anchor != nil || list != nil else { return }
        dragged = true
        let point = event.locationInWindow
        let nodes = paragraphs.filter { list == nil || $0.isDescendant(of: list!) }
        guard let node = nodes.min(by: { distance($0, point) < distance($1, point) }) else { return }
        focus = node
        focusIndex = index(node, at: node.local(point))
        if let (_, value) = position(node, offset: focusIndex), list != nil { logicalFocus = value }
        node.autoscroll(with: event)
        invalidate()
    }

    func end(_ node: NodeView, event: NSEvent) {
        guard !dragged, anchor === node, let url = link(node, at: node.local(event.locationInWindow)) else { return }
        // The containing app owns navigation (local Markdown, anchors,
        // browser URLs); no arbitrary URL scheme is launched by the presenter.
        if let session = presenter?.session { session.delegate?.exactSession(session, command: "openURL", args: [url]) }
    }

    func selectAll() {
        let nodes = paragraphs
        if let first = nodes.first(where: { position($0, offset: 0) != nil }),
           let (owner, start) = position(first, offset: 0),
           let last = nodes.last(where: { $0.isDescendant(of: owner) }),
           let (_, end) = position(last, offset: length(last)) {
            list = owner; logicalAnchor = start; logicalFocus = end; allListText = true
            anchor = first; focus = last; dragged = true
            invalidate()
            return
        }
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        anchor = nodes.first; focus = nodes.last
        anchorIndex = 0; focusIndex = nodes.last.map(length) ?? 0
        dragged = true
        invalidate()
    }

    func copy() {
        let text = selectedText()
        guard !text.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    func selectedText() -> String {
        if let list {
            return presenter?.onListText?(list.id, allListText ? nil : logicalAnchor?.tuple, allListText ? nil : logicalFocus?.tuple) ?? ""
        }
        let parts = paragraphs.compactMap { node -> String? in
            guard let range = range(node), range.length > 0 else { return nil }
            return (node.paragraphSpec().runs.map(\.text).joined() as NSString).substring(with: range)
        }
        return parts.joined(separator: "\n\n")
    }

    private func length(_ node: NodeView) -> Int { node.paragraphSpec().runs.reduce(0) { $0 + ($1.text as NSString).length } }

    func range(_ node: NodeView) -> NSRange? {
        if let list {
            guard let (owner, p) = position(node, offset: 0), owner === list else { return nil }
            let count = length(node)
            if allListText { return NSRange(location: 0, length: count) }
            guard let a = logicalAnchor, let b = logicalFocus else { return nil }
            let (start, end) = a.order <= b.order ? (a, b) : (b, a)
            guard (p.row, p.paragraph) >= (start.row, start.paragraph), (p.row, p.paragraph) <= (end.row, end.paragraph) else { return nil }
            let lo = (p.row, p.paragraph) == (start.row, start.paragraph) ? min(start.offset, count) : 0
            let hi = (p.row, p.paragraph) == (end.row, end.paragraph) ? min(end.offset, count) : count
            return NSRange(location: lo, length: max(0, hi - lo))
        }
        guard anchor != nil && focus != nil else { return nil }
        _ = paragraphs
        guard let anchor, let focus, let a = indices[anchor.id], let b = indices[focus.id],
              let n = indices[node.id], n >= min(a, b), n <= max(a, b) else { return nil }
        let forward = a < b || (a == b && anchorIndex <= focusIndex)
        let start = forward ? a : b, end = forward ? b : a
        let lo = n == start ? (forward ? anchorIndex : focusIndex) : 0
        let hi = n == end ? (forward ? focusIndex : anchorIndex) : length(node)
        let count = length(node)
        return NSRange(location: min(lo, count), length: max(0, min(hi, count) - min(lo, count)))
    }

    /// The wrapper's key survives retirement. Paragraph ordinals follow the
    /// same outer-text traversal as the runner's read-only text projection.
    private func position(_ node: NodeView, offset: Int) -> (NodeView, Position)? {
        var ancestor: NSView? = node
        var wrapper: NodeView?
        while let view = ancestor {
            if let n = view as? NodeView, n.props["listItemKey"] != nil { wrapper = n; break }
            ancestor = view.superview
        }
        guard let wrapper, let key = wrapper.props["listItemKey"], let index = Int(wrapper.props["accessibilityPosInSet"] ?? "") else { return nil }
        ancestor = wrapper.superview
        while let view = ancestor {
            if let owner = view as? NodeView, owner.kind == "list" {
                if positions[node.id] == nil {
                    let nodes = paragraphs.filter { $0.isDescendant(of: wrapper) }
                    for (ordinal, text) in nodes.enumerated() {
                        positions[text.id] = Position(key: key, row: index - 1, paragraph: ordinal, offset: 0)
                    }
                }
                guard var result = positions[node.id] else { return nil }
                result.offset = offset
                return (owner, result)
            }
            ancestor = view.superview
        }
        return nil
    }

    private func distance(_ node: NodeView, _ point: NSPoint) -> CGFloat {
        let p = node.local(point)
        let dy = max(0, max(-p.y, p.y - node.bounds.height))
        let dx = max(0, max(-p.x, p.x - node.bounds.width))
        return dy * 10000 + dx
    }

    private func line(_ node: NodeView, at point: NSPoint) -> (Paragraph, Spec, Int)? {
        guard let paragraph = node.paragraphLayout() else { return nil }
        let spec = node.paragraphSpec()
        guard !paragraph.lines.isEmpty else { return nil }
        var i = paragraph.lines.count - 1
        var bottom: CGFloat = 0
        for n in paragraph.lines.indices {
            bottom = n < paragraph.lineBottoms.count ? paragraph.lineBottoms[n] : paragraph.height
            if point.y - node.contentBox().minY < bottom { i = n; break }
        }
        return (paragraph, spec, i)
    }

    private func index(_ node: NodeView, at point: NSPoint) -> Int {
        let content = node.contentBox()
        if point.y < content.minY { return 0 }
        if point.y > content.maxY { return length(node) }
        guard let (p, spec, i) = line(node, at: point) else { return 0 }
        let row = p.lines[i]
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        let x = content.minX + CGFloat(CTLineGetPenOffsetForFlush(row, flush, Double(content.width)))
        let offset = CTLineGetStringIndexForPosition(row, CGPoint(x: point.x - x, y: 0))
        return offset == kCFNotFound ? length(node) : min(max(0, offset), length(node))
    }

    private func link(_ node: NodeView, at point: NSPoint) -> String? {
        let content = node.contentBox()
        guard content.contains(point), let (p, spec, i) = line(node, at: point) else { return nil }
        // Clicking blank space after a line must not open its last link.
        let width = CGFloat(CTLineGetTypographicBounds(p.lines[i], nil, nil, nil))
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        let x = content.minX + CGFloat(CTLineGetPenOffsetForFlush(p.lines[i], flush, Double(content.width)))
        guard point.x >= x && point.x <= x + width else { return nil }
        let offset = index(node, at: point)
        var start = 0
        for run in node.paragraphSpec().runs {
            let end = start + (run.text as NSString).length
            if offset >= start && offset < end { return run.href.isEmpty ? nil : run.href }
            start = end
        }
        return nil
    }

    func draw(_ node: NodeView, paragraph: Paragraph, spec: Spec, dirty: NSRect) {
        guard let selection = range(node), selection.length > 0 else { return }
        NSColor.selectedTextBackgroundColor.withAlphaComponent(0.45).setFill()
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        let content = node.contentBox()
        for (line, baseline) in zip(paragraph.lines, paragraph.baselines) {
            let r = CTLineGetStringRange(line)
            let lo = max(selection.location, r.location), hi = min(NSMaxRange(selection), r.location + r.length)
            guard hi > lo else { continue }
            var ascent: CGFloat = 0, descent: CGFloat = 0
            _ = CTLineGetTypographicBounds(line, &ascent, &descent, nil)
            let y = content.minY + baseline.rounded()
            guard y + descent >= dirty.minY && y - ascent <= dirty.maxY else { continue }
            let flushX = content.minX + CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(content.width)))
            let x0 = CTLineGetOffsetForStringIndex(line, lo, nil)
            let x1 = CTLineGetOffsetForStringIndex(line, hi, nil)
            NSRect(x: flushX + min(x0, x1), y: y - ascent, width: max(1, abs(x1 - x0)), height: ascent + descent).fill()
        }
    }
}

extension NodeView {
    @objc func copy(_ sender: Any?) { presenter?.selection.copy() }
}
#endif

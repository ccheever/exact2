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
    /// What each `selectionchange` text last heard, in source offsets.
    private var reported: [UInt32: NSRange] = [:]
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
    private var gesture: UInt64 = 0
    private var motion: UInt64 = 0
    private var pendingBegin = false
    private var deferredDrag: NSEvent?
    private var deferredEnd: (NodeView, NSEvent)?

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

    /// Whether anything is selected at all: the cheap question a paragraph
    /// asks before the exact one (`range`).
    var isActive: Bool { allListText || logicalAnchor != nil || (anchor != nil && focus != nil) }

    var paragraphs: [NodeView] {
        if let ordered { return ordered.filter(selectable) }
        guard let presenter else { return [] }
        var result: [NodeView] = []
        func walk(_ view: NSView) {
            if view.isHidden { return }
            if let node = view as? NodeView, presenter.session?.regions.owns(node) == true { return }
            if let node = view as? NodeView, node.isParagraph { if selectable(node) { result.append(node) }; return }
            for child in view.subviews { walk(child) }
        }
        walk(presenter.root)
        for dialog in presenter.dialogs.presented { walk(dialog) }
        for popover in presenter.menus.presented { walk(popover) }
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
        report(next)
    }

    /// The web's `selectionchange` on a `text` (the reader diary): its part
    /// of the selection, in its source text's UTF-16 offsets, when that part
    /// changes; nothing selected there is "" at 0, 0, as on the web.
    private func report(_ next: [UInt32: NSRange]) {
        guard let presenter else { return }
        var seen = Set<UInt32>()
        for node in paragraphs where node.handlers.contains("selectionchange") {
            seen.insert(node.id)
            var source = NSRange(location: 0, length: 0)
            if let shaped = next[node.id] {
                let text = node.paragraphText as NSString
                let map = { (offset: Int) in min(node.readerParagraph == nil ? node.sourceOffset(offset) : offset, text.length) }
                let lo = map(shaped.location), hi = map(NSMaxRange(shaped))
                if hi > lo { source = NSRange(location: lo, length: hi - lo) }
            }
            guard reported[node.id] ?? NSRange(location: 0, length: 0) != source else { continue }
            reported[node.id] = source
            let text = source.length > 0 ? (node.paragraphText as NSString).substring(with: source) : ""
            presenter.selectionChange(node.id, text, source.location, NSMaxRange(source))
        }
        reported = reported.filter { seen.contains($0.key) }
    }

    func clear() {
        gesture += 1; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        anchor = nil; focus = nil
        anchorIndex = 0; focusIndex = 0
        dragged = false
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        invalidate()
    }

    private func selectable(_ node: NodeView) -> Bool {
        var current: NSView? = node
        while let view = current {
            if (view as? NodeView)?.style["user_select"]?.string == "none" { return false }
            current = view.superview
        }
        return true
    }
    func begin(_ node: NodeView, event: NSEvent) {
        guard selectable(node) else { clear(); return }
        gesture += 1; motion = 0; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        let point = node.local(event.locationInWindow)
        if let reader = node.readerParagraph, reader.offset(at: point, node: node) == nil {
            pendingBegin = true
            let current = gesture
            reader.resolveOffset(at: point, node: node) { [weak self, weak node] index, _ in
                guard let self, let node, self.gesture == current else { return }
                self.pendingBegin = false
                guard let index else { self.clear(); return }
                self.begin(node, event: event, index: index)
                let drag = self.deferredDrag, end = self.deferredEnd
                self.deferredDrag = nil; self.deferredEnd = nil
                if let drag { self.drag(drag) }
                if let end { self.end(end.0, event: end.1) }
            }
        } else { begin(node, event: event, index: index(node, at: point)) }
    }

    private func begin(_ node: NodeView, event: NSEvent, index: Int) {
        list = nil; logicalAnchor = nil; logicalFocus = nil; allListText = false
        anchor = node; focus = node
        anchorIndex = index
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
        if pendingBegin { deferredDrag = event; return }
        guard anchor != nil || list != nil else { return }
        dragged = true
        let point = event.locationInWindow
        let nodes = paragraphs.filter { list == nil || $0.isDescendant(of: list!) }
        guard let node = nodes.min(by: { distance($0, point) < distance($1, point) }) else { return }
        motion += 1
        let local = node.local(point)
        if let reader = node.readerParagraph, reader.offset(at: local, node: node) == nil {
            let current = gesture, sequence = motion
            reader.resolveOffset(at: local, node: node) { [weak self, weak node] index, _ in
                guard let self, let node, self.gesture == current, self.motion == sequence, let index else { return }
                self.extend(node, event: event, index: index)
            }
        } else { extend(node, event: event, index: index(node, at: local)) }
    }

    private func extend(_ node: NodeView, event: NSEvent, index: Int) {
        focus = node
        focusIndex = index
        if let (_, value) = position(node, offset: focusIndex), list != nil { logicalFocus = value }
        node.autoscroll(with: event)
        invalidate()
    }

    func end(_ node: NodeView, event: NSEvent) {
        if pendingBegin { deferredEnd = (node, event); return }
        if !dragged, anchor === node, let reader = node.readerParagraph {
            let current = gesture
            reader.resolveOffset(at: node.local(event.locationInWindow), node: node) { [weak self] _, url in
                guard let self, self.gesture == current, let url, let session = self.presenter?.session else { return }
                session.delegate?.exactSession(session, command: "openURL", args: [url])
            }
            return
        }
        guard !dragged, anchor === node, let url = link(node, at: node.local(event.locationInWindow)) else { return }
        // The containing app owns navigation (local Markdown, anchors,
        // browser URLs); no arbitrary URL scheme is launched by the presenter.
        if let session = presenter?.session { session.delegate?.exactSession(session, command: "openURL", args: [url]) }
    }

    func selectAll() {
        gesture += 1; pendingBegin = false; deferredDrag = nil; deferredEnd = nil
        let nodes = paragraphs.filter { !$0.inert }
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

    private func length(_ node: NodeView) -> Int {
        if let source = node.readerParagraph?.accepted?.metadata.source { return source.utf16Count }
        return node.paragraphSpec().runs.reduce(0) { $0 + ($1.text as NSString).length }
    }

    func range(_ node: NodeView) -> NSRange? {
        guard selectable(node) else { return nil }
        if let list {
            guard let (owner, p) = position(node, offset: 0), owner === list else { return nil }
            let count = length(node)
            if allListText { return NSRange(location: 0, length: count) }
            guard let a = logicalAnchor, let b = logicalFocus else { return nil }
            let (start, end) = a.order <= b.order ? (a, b) : (b, a)
            guard (p.row, p.paragraph) >= (start.row, start.paragraph), (p.row, p.paragraph) <= (end.row, end.paragraph) else { return nil }
            let shaped = { (source: Int) in node.readerParagraph == nil ? node.shapedOffset(source) : source }
            let lo = (p.row, p.paragraph) == (start.row, start.paragraph) ? min(shaped(start.offset), count) : 0
            let hi = (p.row, p.paragraph) == (end.row, end.paragraph) ? min(shaped(end.offset), count) : count
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
                // The runner's projection addresses source text; the shaped
                // text may have collapsed white space (LLP 1053 G5).
                result.offset = node.readerParagraph == nil ? node.sourceOffset(offset) : offset
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
        let box = node.contentBox()
        guard let i = paragraph.lineIndex(at: CGPoint(x: point.x - box.minX, y: point.y - box.minY),
                                          align: spec.align, width: box.width) else { return nil }
        return (paragraph, spec, i)
    }

    private func index(_ node: NodeView, at point: NSPoint) -> Int {
        if let reader = node.readerParagraph { return reader.offset(at: point, node: node) ?? 0 }
        let content = node.contentBox()
        if point.y < content.minY { return 0 }
        if point.y > content.maxY { return length(node) }
        guard let (p, spec, i) = line(node, at: point) else { return 0 }
        let x = content.minX + p.origin(i, align: spec.align, width: content.width)
        let offset = p.stringIndex(in: i, at: point.x - x)
        return offset == kCFNotFound ? length(node) : min(max(0, offset), length(node))
    }

    private func link(_ node: NodeView, at point: NSPoint) -> String? { node.inlineLink(at: point) }

    func draw(_ node: NodeView, paragraph: Paragraph, spec: Spec, dirty: NSRect) {
        guard let selection = range(node), selection.length > 0 else { return }
        NSColor.selectedTextBackgroundColor.withAlphaComponent(0.45).setFill()
        for rect in paragraph.selectionRects(selection, align: spec.align, in: node.contentBox(), dirty: dirty) {
            rect.fill()
        }
    }
}

#endif

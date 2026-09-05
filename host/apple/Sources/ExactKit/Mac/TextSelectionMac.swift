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

    init(_ presenter: Presenter) { self.presenter = presenter }

    var paragraphs: [NodeView] {
        guard let presenter else { return [] }
        var result: [NodeView] = []
        func walk(_ view: NSView) {
            if view.isHidden { return }
            if let node = view as? NodeView, node.isParagraph { result.append(node); return }
            for child in view.subviews { walk(child) }
        }
        walk(presenter.root)
        return result
    }

    private func invalidate() { for node in paragraphs { node.needsDisplay = true } }

    func begin(_ node: NodeView, event: NSEvent) {
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
        invalidate()
    }

    func drag(_ event: NSEvent) {
        guard anchor != nil else { return }
        dragged = true
        let point = event.locationInWindow
        let nodes = paragraphs
        guard let node = nodes.min(by: { distance($0, point) < distance($1, point) }) else { return }
        focus = node
        focusIndex = index(node, at: node.local(point))
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
        anchor = nodes.first; focus = nodes.last
        anchorIndex = 0; focusIndex = nodes.last.map(length) ?? 0
        dragged = true
        invalidate()
    }

    func copy() {
        let parts = paragraphs.compactMap { node -> String? in
            guard let range = range(node), range.length > 0 else { return nil }
            return (node.paragraphSpec().runs.map(\.text).joined() as NSString).substring(with: range)
        }
        guard !parts.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(parts.joined(separator: "\n\n"), forType: .string)
    }

    private func length(_ node: NodeView) -> Int { node.paragraphSpec().runs.reduce(0) { $0 + ($1.text as NSString).length } }

    func range(_ node: NodeView) -> NSRange? {
        guard anchor != nil && focus != nil else { return nil }
        let nodes = paragraphs
        guard let anchor, let focus, let a = nodes.firstIndex(of: anchor), let b = nodes.firstIndex(of: focus),
              let n = nodes.firstIndex(of: node), n >= min(a, b), n <= max(a, b) else { return nil }
        let forward = a < b || (a == b && anchorIndex <= focusIndex)
        let start = forward ? a : b, end = forward ? b : a
        let lo = n == start ? (forward ? anchorIndex : focusIndex) : 0
        let hi = n == end ? (forward ? focusIndex : anchorIndex) : length(node)
        let count = length(node)
        return NSRange(location: min(lo, count), length: max(0, min(hi, count) - min(lo, count)))
    }

    private func distance(_ node: NodeView, _ point: NSPoint) -> CGFloat {
        let p = node.local(point)
        let dy = max(0, max(-p.y, p.y - node.bounds.height))
        let dx = max(0, max(-p.x, p.x - node.bounds.width))
        return dy * 10000 + dx
    }

    private func line(_ node: NodeView, at point: NSPoint) -> (Paragraph, Spec, Int)? {
        guard let engine = node.text else { return nil }
        let spec = node.paragraphSpec()
        let paragraph = engine.paragraph(spec, width: node.bounds.width)
        guard !paragraph.lines.isEmpty else { return nil }
        var i = paragraph.lines.count - 1
        var bottom: CGFloat = 0
        let lineHeight = spec.runs.map(\.lineHeight).max() ?? 0
        for n in paragraph.lines.indices {
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            _ = CTLineGetTypographicBounds(paragraph.lines[n], &ascent, &descent, &leading)
            bottom += lineHeight > 0 ? lineHeight : ascent + descent + leading
            if point.y < bottom { i = n; break }
        }
        return (paragraph, spec, i)
    }

    private func index(_ node: NodeView, at point: NSPoint) -> Int {
        if point.y < 0 { return 0 }
        if point.y > node.bounds.height { return length(node) }
        guard let (p, spec, i) = line(node, at: point) else { return 0 }
        let row = p.lines[i]
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        let x = CGFloat(CTLineGetPenOffsetForFlush(row, flush, Double(node.bounds.width)))
        let offset = CTLineGetStringIndexForPosition(row, CGPoint(x: point.x - x, y: 0))
        return offset == kCFNotFound ? length(node) : min(max(0, offset), length(node))
    }

    private func link(_ node: NodeView, at point: NSPoint) -> String? {
        guard node.bounds.contains(point), let (p, _, i) = line(node, at: point) else { return nil }
        // Clicking blank space after a line must not open its last link.
        let width = CGFloat(CTLineGetTypographicBounds(p.lines[i], nil, nil, nil))
        guard point.x >= 0 && point.x <= width else { return nil }
        let offset = index(node, at: point)
        var start = 0
        for run in node.paragraphSpec().runs {
            let end = start + (run.text as NSString).length
            if offset >= start && offset < end { return run.href.isEmpty ? nil : run.href }
            start = end
        }
        return nil
    }

    func draw(_ node: NodeView, paragraph: Paragraph, spec: Spec) {
        guard let selection = range(node), selection.length > 0 else { return }
        NSColor.selectedTextBackgroundColor.withAlphaComponent(0.45).setFill()
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        for (line, baseline) in zip(paragraph.lines, paragraph.baselines) {
            let r = CTLineGetStringRange(line)
            let lo = max(selection.location, r.location), hi = min(NSMaxRange(selection), r.location + r.length)
            guard hi > lo else { continue }
            var ascent: CGFloat = 0, descent: CGFloat = 0
            _ = CTLineGetTypographicBounds(line, &ascent, &descent, nil)
            let flushX = CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(node.bounds.width)))
            let x0 = CTLineGetOffsetForStringIndex(line, lo, nil)
            let x1 = CTLineGetOffsetForStringIndex(line, hi, nil)
            NSRect(x: flushX + min(x0, x1), y: baseline - ascent, width: max(1, abs(x1 - x0)), height: ascent + descent).fill()
        }
    }
}

extension NodeView {
    @objc func copy(_ sender: Any?) { presenter?.selection.copy() }
}
#endif

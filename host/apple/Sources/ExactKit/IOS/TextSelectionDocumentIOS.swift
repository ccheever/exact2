#if os(iOS)
import UIKit

/// One text input over prose that shares a selectable container. Its surface
/// is behind the paragraphs; hit testing sends their text touches to it.
final class SelectionDocument {
    struct Part {
        let node: NodeView, input: SelectableText, start: Int, length: Int
        var range: NSRange { NSRange(location: start, length: length) }
    }
    weak var owner: NodeView?
    let surface = SelectionSurface()
    let parts: [Part]
    let text: NSString
    private var reported: [UInt32: NSRange] = [:]

    static func container(of node: NodeView) -> NodeView? {
        var at = node.superview
        while let view = at {
            if let parent = view as? NodeView {
                if ["text", "all"].contains(parent.style["user_select"]?.string ?? "") { return parent }
                if parent.scrollView != nil || parent.pressable { return nil }
            }
            at = view.superview
        }
        return nil
    }
    static func paragraphs(in owner: NodeView, host: TextSelectionHost) -> [NodeView] {
        var result: [NodeView] = []
        func walk(_ view: UIView) {
            guard !view.isHidden else { return }
            if let node = view as? NodeView, node.isParagraph {
                if host.admits(node) { result.append(node) }
                return
            }
            for child in view.subviews where !(child is SelectionSurface) { walk(child) }
        }
        walk(owner)
        return result
    }
    init(owner: NodeView, paragraphs: [NodeView], host: TextSelectionHost) {
        self.owner = owner
        var parts: [Part] = [], strings: [String] = [], start = 0
        for node in paragraphs {
            let text = SelectableText.shaped(node)
            parts.append(Part(node: node, input: SelectableText(node: node, host: host, attach: false), start: start, length: text.length))
            strings.append(text as String)
            start += text.length + 1
        }
        self.parts = parts
        text = strings.joined(separator: "\n") as NSString
        surface.frame = owner.container.bounds
        surface.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        surface.isAccessibilityElement = false
        surface.layer.zPosition = -1e6
        owner.container.insertSubview(surface, at: 0)
        let pointer = PointerRecognizer(target: nil, action: nil)
        pointer.selection = self
        pointer.cancelsTouchesInView = false
        pointer.delaysTouchesBegan = false
        pointer.delaysTouchesEnded = false
        surface.addGestureRecognizer(pointer)
    }
    func contains(_ node: NodeView) -> Bool { parts.contains { $0.node === node } }
    func settle(host: TextSelectionHost) -> Bool {
        guard let owner else { return false }
        let now = Self.paragraphs(in: owner, host: host)
        return now.count == parts.count && zip(now, parts).allSatisfy { node, part in
            node === part.node && SelectableText.shaped(node).length == part.length
                && SelectableText.shaped(node) == part.input.shapedText
        }
    }
    func part(at offset: Int) -> Part? { parts.last { $0.start <= offset } ?? parts.first }
    func paragraph(at point: CGPoint) -> NodeView? {
        parts.first { $0.node.bounds.contains($0.node.convert(point, from: surface)) }?.node
    }
    func index(at point: CGPoint) -> Int {
        guard let part = parts.min(by: { distance(point, to: $0.node) < distance(point, to: $1.node) }) else { return 0 }
        return part.start + part.input.index(at: part.node.convert(point, from: surface))
    }
    private func distance(_ point: CGPoint, to node: NodeView) -> CGFloat {
        let box = node.convert(node.bounds, to: surface)
        let x = max(box.minX - point.x, 0, point.x - box.maxX), y = max(box.minY - point.y, 0, point.y - box.maxY)
        return x * x + y * y
    }
    func rects(_ range: NSRange) -> [UITextSelectionRect] {
        var boxes: [(CGRect, NSWritingDirection)] = []
        for part in parts {
            let selected = NSIntersectionRange(range, part.range)
            guard selected.length > 0 else { continue }
            let local = NSRange(location: selected.location - part.start, length: selected.length)
            let inputRange = SelectableRange(local)
            for rect in part.input.selectionRects(for: inputRange) {
                boxes.append((part.node.convert(rect.rect, to: surface), rect.writingDirection))
            }
        }
        return boxes.enumerated().map { i, box in
            SelectableRect(box.0, first: i == 0, last: i == boxes.count - 1, rtl: box.1 == .rightToLeft)
        }
    }
    func caret(at offset: Int) -> CGRect {
        guard let part = part(at: offset) else { return .zero }
        return part.node.convert(part.input.caretRect(for: SelectablePosition(min(part.length, offset - part.start))), to: surface)
    }
    func atomic(_ range: NSRange) -> NSRange {
        guard range.length > 0 else { return range }
        var lo = range.location, hi = NSMaxRange(range)
        for part in parts {
            let overlap = NSIntersectionRange(range, part.range)
            guard overlap.length > 0 else { continue }
            let whole = part.input.atomic(NSRange(location: overlap.location - part.start, length: overlap.length))
            if whole.length > 0 { lo = min(lo, part.start + whole.location); hi = max(hi, part.start + NSMaxRange(whole)) }
        }
        return NSRange(location: lo, length: hi - lo)
    }
    func report(_ range: NSRange, host: TextSelectionHost) {
        for part in parts where part.node.handlers.contains("selectionchange") {
            let selected = NSIntersectionRange(range, part.range)
            let lo = selected.length > 0 ? part.node.sourceOffset(selected.location - part.start) : 0
            let hi = selected.length > 0 ? part.node.sourceOffset(NSMaxRange(selected) - part.start) : 0
            let local = NSRange(location: lo, length: hi - lo)
            guard reported[part.node.id] ?? NSRange(location: 0, length: 0) != local else { continue }
            reported[part.node.id] = local
            host.report(part.node, (part.node.paragraphText as NSString).substring(with: local), local)
        }
    }
}

final class SelectionSurface: UIView {
    weak var input: SelectableText?
    private weak var touched: NodeView?
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let hit = super.hitTest(point, with: event)
        return hit === self ? nil : hit
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        touched = touches.first.flatMap { input?.document?.paragraph(at: $0.location(in: self)) }
        touched?.touchesBegan(touches, with: event)
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) { touched?.touchesMoved(touches, with: event) }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { touched?.touchesEnded(touches, with: event); touched = nil }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { touched?.touchesCancelled(touches, with: event); touched = nil }
    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        action == #selector(copy(_:)) && input?.selected.length ?? 0 > 0
    }
    override func copy(_ sender: Any?) { if let input { input.copyDocument() } }
}
#endif

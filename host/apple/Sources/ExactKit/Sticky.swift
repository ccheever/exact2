// `position: sticky` (LLP 1083 D4): the kernel lays a sticky box out where it
// would be unscrolled and sends its constraint (`StickyConstraint` in
// kernel/src/kernel/sticky.rs); here, each time its scroller scrolls, the box
// moves by the constraint's offset, as a browser's does. The move folds into
// `applyTransform` (iOS: the outer translation; macOS: the frame, as a lifted
// Arrange row's), so hit-testing follows it and a frame op keeps it.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// One sticky box's constraint: rectangles `[left, top, right, bottom]` in
/// its scroller's border-box space at offset zero.
struct StickyConstraint: Equatable {
    let scroller: UInt32
    let natural: [CGFloat], limit: [CGFloat], port: [CGFloat]
    /// `top`, `right`, `bottom`, `left`; `nil` where `auto`.
    let insets: [CGFloat?]

    init?(_ payload: [String: Any]) {
        func rect(_ key: String) -> [CGFloat]? {
            guard let a = payload[key] as? [Any], a.count == 4 else { return nil }
            let n = a.compactMap { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } }
            return n.count == 4 ? n : nil
        }
        guard let scroller = (payload["scroller"] as? NSNumber)?.uint32Value,
              let natural = rect("natural"), let limit = rect("limit"), let port = rect("port"),
              let insets = payload["insets"] as? [Any], insets.count == 4 else { return nil }
        self.scroller = scroller; self.natural = natural; self.limit = limit; self.port = port
        self.insets = insets.map { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } }
    }

    /// How far to move the box with its scroller at `scroll`: Chrome's rule,
    /// as the kernel's `StickyConstraint::offset` (the end inset pulls the
    /// box back into the scrollport, never before its limit's start; the
    /// start inset then pushes it, never past its limit's end).
    func offset(_ scroll: CGPoint) -> CGPoint {
        func axis(_ scroll: CGFloat, _ start: CGFloat?, _ end: CGFloat?, _ i: Int) -> CGFloat {
            let lo = natural[i], hi = natural[i + 2]
            var d: CGFloat = 0
            if let end {
                let pull = max(port[i + 2] + scroll - end - hi, min(limit[i] - lo, 0))
                if pull < 0 { d += pull }
            }
            if let start {
                let push = min(port[i] + scroll + start - lo, max(limit[i + 2] - hi, 0))
                if push > 0 { d += push }
            }
            return d
        }
        return CGPoint(x: axis(scroll.x, insets[3], insets[1], 0), y: axis(scroll.y, insets[0], insets[2], 1))
    }
}

/// The session's sticky boxes, by scroller.
final class StickyHost {
    private unowned let presenter: Presenter
    private var boxes: [UInt32: StickyConstraint] = [:]
    private var byScroller: [UInt32: Set<UInt32>] = [:]
    init(_ presenter: Presenter) { self.presenter = presenter }

    var isEmpty: Bool { boxes.isEmpty }

    /// A `sticky` op: the node's constraint, or none (no longer sticky).
    func apply(_ id: UInt32, _ payload: [String: Any]) {
        forget(id)
        guard let c = StickyConstraint(payload) else {
            presenter.views[id].map { place($0, .zero) }
            return
        }
        boxes[id] = c
        byScroller[c.scroller, default: []].insert(id)
        place(id)
    }
    func forget(_ id: UInt32) {
        guard let old = boxes.removeValue(forKey: id) else { return }
        byScroller[old.scroller]?.remove(id)
        if byScroller[old.scroller]?.isEmpty == true { byScroller[old.scroller] = nil }
    }
    /// A scroller moved; `nil` is the page, which scrolls every sticky box
    /// whose scroller is a root that does not scroll itself.
    func scrolled(_ scroller: UInt32?) {
        guard !boxes.isEmpty else { return }
        if let scroller {
            for id in byScroller[scroller] ?? [] { place(id) }
        } else {
            for (scroller, ids) in byScroller where pageScrolls(scroller) { for id in ids { place(id) } }
        }
    }

    private func place(_ id: UInt32) {
        guard let c = boxes[id], let view = presenter.views[id] else { return }
        place(view, c.offset(offset(of: c.scroller)))
    }
    private func place(_ view: NodeView, _ offset: CGPoint) {
        guard view.stickyOffset != offset else { return }
        view.stickyOffset = offset
        view.applyTransform()
    }

    /// Whether the page's scroll is this scroller's: a root view without a
    /// scroll view of its own.
    private func pageScrolls(_ scroller: UInt32) -> Bool {
        guard let v = presenter.views[scroller] else { return false }
        return v.scroll == nil && v.superview === presenter.root
    }
    /// The scroller's offset in its content's space: its own scroll view's,
    /// the page's for a root that does not scroll itself, else none (it
    /// clips without scrolling).
    private func offset(of scroller: UInt32) -> CGPoint {
        guard let v = presenter.views[scroller] else { return .zero }
        #if os(macOS)
        if let sv = v.scroll { return sv.contentView.bounds.origin }
        guard v.superview === presenter.root else { return .zero }
        let page = presenter.viewport.contentView.bounds.origin
        #else
        if let sv = v.scroll { return sv.contentOffset }
        guard v.superview === presenter.root else { return .zero }
        let page = presenter.viewport.contentOffset
        #endif
        return CGPoint(x: page.x - v.frame.minX, y: page.y - v.frame.minY)
    }
}

#if os(macOS)
extension NSView {
    /// CSS `z-index` orders siblings (`usedZIndex`, the layer's `zPosition`),
    /// which AppKit paints by but does not hit-test by: it asks subviews in
    /// reverse order. A sibling raised above the branch AppKit's `hit` came
    /// from takes the point first when it holds it, so a pinned sticky
    /// header is hit over the rows scrolling under it (LLP 1083 D6).
    func raisedHit(_ hit: NSView?, _ point: NSPoint) -> NSView? {
        guard let hit, hit !== self else { return hit }
        var branch = hit
        while let up = branch.superview, up !== self { branch = up }
        let z = branch.layer?.zPosition ?? 0
        let raised = subviews.reversed().filter { $0 !== branch && ($0.layer?.zPosition ?? 0) > z }
        guard branch.superview === self, !raised.isEmpty else { return hit }
        let local = convert(point, from: superview)
        for view in raised.sorted(by: { ($0.layer?.zPosition ?? 0) > ($1.layer?.zPosition ?? 0) }) {
            if let found = view.hitTest(local) { return found }
        }
        return hit
    }
}
#endif

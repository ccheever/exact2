#if os(macOS)
import AppKit

// Dropping across lists on AppKit (LLP 1094 D6): the ghost is the row's
// cached display in a topmost image view; facts are in window coordinates,
// which grow upward.
/// The lifted row's stand-in, above everything in the window.
final class ReorderGhost {
    let view: NSImageView
    private weak var window: NSWindow?
    /// Where it stands in the window, unscaled.
    private(set) var frame: CGRect
    private let scale: CGFloat
    init?(of wrapper: NodeView) {
        guard let window = wrapper.window, let rep = wrapper.bitmapImageRepForCachingDisplay(in: wrapper.bounds) else { return nil }
        wrapper.cacheDisplay(in: wrapper.bounds, to: rep)
        let image = NSImage(size: wrapper.bounds.size)
        image.addRepresentation(rep)
        frame = wrapper.convert(wrapper.bounds, to: nil)
        view = NSImageView(image: image)
        view.imageScaling = .scaleAxesIndependently
        view.wantsLayer = true
        // The host's look (D6): 0 8px 24px at 25% black (AppKit's y grows
        // up), and 1.03 unless motion is reduced.
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(0.25)
        shadow.shadowBlurRadius = 24
        shadow.shadowOffset = NSSize(width: 0, height: -8)
        view.shadow = shadow
        scale = DisplayPreferences.reducedMotion ? 1 : 1.03
        self.window = window
    }
    private func shown(_ rect: CGRect, scale: CGFloat) -> CGRect {
        let rect = rect.insetBy(dx: -rect.width * (scale - 1) / 2, dy: -rect.height * (scale - 1) / 2)
        return window?.contentView?.convert(rect, from: nil) ?? rect
    }
    func show() {
        guard let content = window?.contentView else { return }
        view.frame = shown(frame, scale: scale)
        content.addSubview(view, positioned: .above, relativeTo: nil)
    }
    func place(_ origin: CGPoint) {
        frame.origin = origin
        view.frame = shown(frame, scale: scale)
    }
    /// Spring onto `row` (where the dragged row is now), or fade without one.
    func land(on row: NodeView?, done: @escaping () -> Void) {
        guard let row, row.window != nil else {
            NSAnimationContext.runAnimationGroup({ $0.duration = 0.2; view.animator().alphaValue = 0 }, completionHandler: done)
            return
        }
        frame = row.convert(row.bounds, to: nil)
        let target = shown(frame, scale: 1)
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.3
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            view.animator().frame = target
        }, completionHandler: done)
    }
    func remove() { view.removeFromSuperview() }
}

/// A grouped list's port in the window, and its scrollTop as its collection
/// feedback reports it (D5).
struct ReorderPort {
    let rect: CGRect
    let scrollTop: Double
    init?(list node: NodeView, presenter: Presenter) {
        guard let scroll = node.scroll, scroll.window != nil,
              let top = presenter.collections.geometry(node.id)?.offset else { return nil }
        rect = scroll.contentView.convert(scroll.contentView.bounds, to: nil)
        scrollTop = top
    }
    /// The centre's viewport y, less the port's viewport top, plus its scrollTop.
    func contentY(_ p: CGPoint) -> Double { scrollTop + Double(rect.maxY - p.y) }
}

/// One scroller autoscroll may move, on its own axis (D7).
struct ReorderScroller {
    let scroll: NSScrollView
    let vertical: Bool
    let rect: CGRect
    /// D7, amended 2026-10-07: an ancestor joins while it holds the centre,
    /// or the contact once a ghost held off its middle has carried its centre
    /// past the edge (that edge's band then measures the centre beyond it).
    static func chain(from list: NodeView, holding centre: CGPoint, contact: CGPoint? = nil) -> [ReorderScroller] {
        var out: [ReorderScroller] = []
        if let scroll = list.scroll, scroll.window != nil {
            let rect = scroll.contentView.convert(scroll.contentView.bounds, to: nil)
            if centre.x >= rect.minX, centre.x <= rect.maxX { out.append(ReorderScroller(scroll: scroll, vertical: true, rect: rect)) }
        }
        var view = list.superview
        while let current = view {
            if let node = current as? NodeView, let scroll = node.scroll, scroll.window != nil {
                let rect = scroll.contentView.convert(scroll.contentView.bounds, to: nil)
                if rect.contains(centre) || contact.map({ rect.contains($0) }) == true {
                    let wide = (scroll.documentView?.frame.width ?? 0) > scroll.contentView.bounds.width + 0.5
                    out.append(ReorderScroller(scroll: scroll, vertical: !wide, rect: rect))
                }
            }
            view = current.superview
        }
        return out
    }
    func direction(_ centre: CGPoint) -> Double {
        vertical ? ReorderEdge.direction(offset: Double(rect.maxY - centre.y), height: Double(rect.height))
            : ReorderEdge.direction(offset: Double(centre.x - rect.minX), height: Double(rect.width))
    }
    /// The clip's offset from its document's start along the axis, and its end.
    private var range: (now: CGFloat, high: CGFloat) {
        let clip = scroll.contentView, document = scroll.documentView?.frame.size ?? .zero
        if vertical {
            let high = max(0, document.height - clip.bounds.height)
            return (clip.isFlipped ? clip.bounds.minY : high - clip.bounds.minY, high)
        }
        return (clip.bounds.minX, max(0, document.width - clip.bounds.width))
    }
    func canScroll(toward direction: Double) -> Bool {
        let r = range
        return direction < 0 ? r.now > 0 : direction > 0 ? r.now < r.high : false
    }
    func scroll(by delta: Double) -> Bool {
        let r = range, clip = scroll.contentView
        let next = min(r.high, max(0, r.now + CGFloat(delta)))
        guard next != r.now, delta.isFinite else { return false }
        var origin = clip.bounds.origin
        if vertical { origin.y = clip.isFlipped ? next : r.high - next } else { origin.x = next }
        clip.scroll(to: origin)
        scroll.reflectScrolledClipView(clip)
        return true
    }
}

extension NodeView {
    /// Where focus goes after a keyboard or custom-action move (D9).
    func focusAfterReorder() {
        if acceptsFirstResponder { window?.makeFirstResponder(self) }
    }
    /// "Move earlier", "Move later", "Move to previous list", "Move to next
    /// list" on a grouped grip (D9).
    package override func accessibilityCustomActions() -> [NSAccessibilityCustomAction]? {
        guard reorderGroupList != nil else { return super.accessibilityCustomActions() }
        return ReorderGroupStep.actions.map { name, step in
            NSAccessibilityCustomAction(name: name) { [weak self] in self?.reorderAction(step) ?? false }
        }
    }
}
#endif

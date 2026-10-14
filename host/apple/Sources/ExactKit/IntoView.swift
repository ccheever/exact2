// `scrollIntoView("element-id", block=, inline=, behavior=)` (minesweeper
// F3): `Element.scrollIntoView()` on any element by its HTML `id`, as
// `focus("id")` names one. Every scroll container above it aligns it,
// innermost first, then the page, as CSSOM View's "scroll an element into
// view" does; each clamps to its own range. A list's row by key is the
// runner's (LLP 1070.000). The scrollers' observers tell the app
// and a virtualized list's window, as a reader's scroll does.
import CoreGraphics

enum IntoView {
    /// Where a scroller's offset goes on one axis so the target `t0..<t1`
    /// (in the scroller's content) aligns in the port `p0..<p0 + size` by
    /// CSS's `start`, `center`, `end` or `nearest`, before clamping.
    static func aligned(_ align: String, target t0: CGFloat, _ t1: CGFloat, port p0: CGFloat, size: CGFloat) -> CGFloat {
        let p1 = p0 + size, length = t1 - t0
        switch align {
        case "start": return t0
        case "end": return t1 - size
        case "center": return (t0 + t1) / 2 - size / 2
        default:
            // `nearest`: nothing when it shows whole or covers the port;
            // else the edge it is past, or the far one when it is larger.
            if (t0 >= p0 && t1 <= p1) || (t0 < p0 && t1 > p1) { return p0 }
            if (t0 < p0 && length <= size) || (t1 > p1 && length > size) { return t0 }
            return t1 - size
        }
    }
    static func animates(_ args: [Any]) -> Bool {
        let behavior = args.count > 3 ? args[3] as? String : nil
        return !ExactEnv.agentFreezes && !DisplayPreferences.reducedMotion
            && behavior != "instant"
    }
    static func inset(_ rect: CGRect, style: [String: BatchValue], family: String, outward: Bool = false) -> CGRect {
        let top = style["\(family)_top"]?.length(basis: rect.height) ?? 0
        let bottom = style["\(family)_bottom"]?.length(basis: rect.height) ?? 0
        let left = style["\(family)_left"]?.length(basis: rect.width) ?? 0
        let right = style["\(family)_right"]?.length(basis: rect.width) ?? 0
        let sign: CGFloat = outward ? -1 : 1
        return CGRect(x: rect.minX + sign * left, y: rect.minY + sign * top,
                      width: max(0, rect.width - sign * (left + right)), height: max(0, rect.height - sign * (top + bottom)))
    }
    /// The options as the command carries them: block, inline, `none` the web's default.
    static func options(_ args: [Any]) -> (block: String, inline: String) {
        let text = { (i: Int) in args.count > i ? args[i] as? String : nil }
        return (text(1) ?? "start", text(2) ?? "nearest")
    }
}

#if os(macOS)
import AppKit

extension Presenter {
    func scrollElementIntoView(_ args: [Any]) {
        guard let name = args.first as? String,
              let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }),
              target.window != nil else {
            session?.log("scrollIntoView \"\(args.first ?? "")\" refused: no live node with that id")
            return
        }
        let (block, inline) = IntoView.options(args)
        var moves: [(NSClipView, NodeView?, NSPoint)] = []
        var from: NSView = target
        var frame = IntoView.inset(target.bounds, style: target.style, family: "scroll_margin", outward: true)
        for case let clip as NSClipView in sequence(first: target.superview, next: { $0?.superview }).compactMap({ $0 }) {
            frame = from.convert(frame, to: clip)
            let owner = (clip.superview?.superview as? NodeView)
            let port = IntoView.inset(clip.bounds, style: owner?.style ?? [:], family: "scroll_padding")
            let origin = NSPoint(
                x: IntoView.aligned(inline, target: frame.minX, frame.maxX, port: port.minX, size: port.width) - (port.minX - clip.bounds.minX),
                y: IntoView.aligned(block, target: frame.minY, frame.maxY, port: port.minY, size: port.height) - (port.minY - clip.bounds.minY))
            let to = clip.constrainBoundsRect(NSRect(origin: origin, size: clip.bounds.size)).origin
            moves.append((clip, owner, to))
            // Project the target at this clip's destination before aligning
            // its parent. The animator has not moved either clip yet.
            frame = frame.offsetBy(dx: clip.bounds.minX - to.x, dy: clip.bounds.minY - to.y)
            from = clip
        }
        for (clip, owner, to) in moves {
            owner?.cancelElementScroll()
            guard to != clip.bounds.origin else { continue }
            let end = owner?.scrollEnd ?? ScrollEnd()
            owner?.scrollEnd = end
            if IntoView.animates(args) {
                let token = UUID()
                end.elementAnimation = token; end.hold()
                NSAnimationContext.runAnimationGroup({ _ in clip.animator().setBoundsOrigin(to) }, completionHandler: { [weak owner] in
                    guard end.elementAnimation == token else { return }
                    end.elementAnimation = nil
                    owner?.endScroll()
                })
            } else {
                NSAnimationContext.runAnimationGroup({ c in c.duration = 0; clip.animator().setBoundsOrigin(to) })
                clip.scroll(to: to)
            }
            (clip.superview as? NSScrollView)?.reflectScrolledClipView(clip)
        }
    }
}
#elseif os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    func scrollElementIntoView(_ args: [Any]) {
        guard let name = args.first as? String,
              let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }),
              target.window != nil else {
            session?.log("scrollIntoView \"\(args.first ?? "")\" refused: no live node with that id")
            return
        }
        let (block, inline) = IntoView.options(args)
        let grouped = groupedLists?.scroller(for: target.id).flatMap { scroll in
            scroll.window === target.window && groupedLists?.projectedRect(for: target, in: scroll) != nil ? scroll : nil
        }
        var scrollers: [UIScrollView] = []
        for view in sequence(first: target.superview, next: { $0?.superview }).compactMap({ $0 }) {
            if let sv = view as? UIScrollView, sv.isScrollEnabled { scrollers.append(sv) }
            // A standard cell's authored original is a sibling of the native
            // collection. Insert that port at its owner, after any nested one.
            if let grouped, view === grouped.superview, grouped.isScrollEnabled,
               !scrollers.contains(where: { $0 === grouped }) { scrollers.append(grouped) }
        }
        for sv in scrollers {
            let frame: CGRect
            if let grouped, sv === grouped || grouped.isDescendant(of: sv),
               let projected = groupedLists?.projectedRect(for: target, in: grouped) {
                // Inner authored scrolls use the target's own box. The native
                // row frame projects only into its collection and outer ports.
                frame = grouped.convert(projected, to: sv)
            } else {
                frame = target.convert(target.bounds, to: sv)
            }
            // Content room belongs to the range; only UI insets (a bar or
            // keyboard) obstruct the viewport used for CSS alignment.
            let owner = (sv.superview as? NodeView).flatMap { $0.scrollView === sv ? $0 : nil }
            let inset = owner?.scrollPortInsets(sv) ?? sv.adjustedContentInset
            let port = IntoView.inset(sv.bounds.inset(by: inset), style: owner?.style ?? [:], family: "scroll_padding")
            let area = IntoView.inset(frame, style: target.style, family: "scroll_margin", outward: true)
            let x = IntoView.aligned(inline, target: area.minX, area.maxX, port: port.minX, size: port.width) - (port.minX - sv.bounds.minX)
            let y = IntoView.aligned(block, target: area.minY, area.maxY, port: port.minY, size: port.height) - (port.minY - sv.bounds.minY)
            let rangeInset = sv.adjustedContentInset
            let maxX = max(-rangeInset.left, sv.contentSize.width + rangeInset.right - sv.bounds.width)
            let maxY = max(-rangeInset.top, sv.contentSize.height + rangeInset.bottom - sv.bounds.height)
            let to = CGPoint(x: min(max(x, -rangeInset.left), maxX), y: min(max(y, -rangeInset.top), maxY))
            guard to != sv.contentOffset else { continue }
            // UIKit element jumps fall back to instant (LLP 1070.000 §14).
            sv.setContentOffset(to, animated: false)
        }
    }
}
#endif

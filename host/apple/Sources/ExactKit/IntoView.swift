// `scrollIntoView("element-id", block=, inline=, behavior=)` (minesweeper
// F3): `Element.scrollIntoView()` on any element by its HTML `id`, as
// `focus("id")` names one. Every scroll container above it aligns it,
// innermost first, then the page, as CSSOM View's "scroll an element into
// view" does; each clamps to its own range. A list's row by key is the
// runner's (LLP 1070.000). `behavior="smooth"` lands at once here, a
// deviation the reference declares. The scrollers' observers tell the app
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
        for case let clip as NSClipView in sequence(first: target.superview, next: { $0?.superview }).compactMap({ $0 }) {
            let frame = target.convert(target.bounds, to: clip), port = clip.bounds
            let origin = NSPoint(
                x: IntoView.aligned(inline, target: frame.minX, frame.maxX, port: port.minX, size: port.width),
                y: IntoView.aligned(block, target: frame.minY, frame.maxY, port: port.minY, size: port.height))
            let to = clip.constrainBoundsRect(NSRect(origin: origin, size: port.size)).origin
            guard to != port.origin else { continue }
            clip.scroll(to: to)
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
            let port = sv.bounds.inset(by: inset)
            let x = IntoView.aligned(inline, target: frame.minX, frame.maxX, port: port.minX, size: port.width) - inset.left
            let y = IntoView.aligned(block, target: frame.minY, frame.maxY, port: port.minY, size: port.height) - inset.top
            let rangeInset = sv.adjustedContentInset
            let maxX = max(-rangeInset.left, sv.contentSize.width + rangeInset.right - sv.bounds.width)
            let maxY = max(-rangeInset.top, sv.contentSize.height + rangeInset.bottom - sv.bounds.height)
            let to = CGPoint(x: min(max(x, -rangeInset.left), maxX), y: min(max(y, -rangeInset.top), maxY))
            guard to != sv.contentOffset else { continue }
            sv.setContentOffset(to, animated: false)
        }
    }
}
#endif

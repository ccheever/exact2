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
    func cancelPageElementScroll(_ clip: NSClipView) {
        guard pageElementAnimation != nil else { return }
        pageElementAnimation = nil
        let at = clip.bounds.origin
        NSAnimationContext.runAnimationGroup({ context in context.duration = 0; clip.animator().setBoundsOrigin(at) })
    }

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
            if let owner { owner.cancelElementScroll() } else { cancelPageElementScroll(clip) }
            guard to != clip.bounds.origin else { continue }
            if IntoView.animates(args) {
                let token = UUID()
                if let owner {
                    let end = owner.scrollEnd ?? ScrollEnd()
                    owner.scrollEnd = end
                    end.elementAnimation = token; end.hold()
                    NSAnimationContext.runAnimationGroup({ _ in clip.animator().setBoundsOrigin(to) }, completionHandler: { [weak owner] in
                        guard end.elementAnimation == token else { return }
                        end.elementAnimation = nil
                        owner?.endScroll()
                    })
                } else {
                    pageElementAnimation = token
                    NSAnimationContext.runAnimationGroup({ _ in clip.animator().setBoundsOrigin(to) }, completionHandler: { [weak self] in
                        guard self?.pageElementAnimation == token else { return }
                        self?.pageElementAnimation = nil
                    })
                }
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

/// A target's box in a scroller: where it is now and where it will be once
/// the reveals running on it and inside it arrive.
struct Projected {
    let view: UIView
    let now: CGRect
    let then: CGRect
}
private extension CGPoint {
    static func - (a: CGPoint, b: CGPoint) -> CGPoint { CGPoint(x: a.x - b.x, y: a.y - b.y) }
}

/// Hold `scrollIntoView` requests until their targets mount and pending fills finish.
/// Repeat them after navigation or presentation settles, following focus so UIKit
/// cannot scroll back to the focused field. Keep the latest request per HTML id;
/// a request for another element leaves the others in place.
final class ElementScrolls {
    private struct Held {
        weak var node: NodeView?
        let incarnation: UInt64
        let name: String
        let args: [Any]
    }
    private var held: [Held] = []
    private unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }

    var names: [String] { held.map(\.name) }
    func hold(_ node: NodeView, _ name: String, _ args: [Any]) {
        drop(name)
        held.append(Held(node: node, incarnation: node.incarnation, name: name, args: args))
        if let session = presenter.session, session.fillInFlight {
            session.whenIdle { [weak self] in self?.flush() }
        }
    }
    func drop(_ name: String) { held.removeAll { $0.name == name } }
    /// A closing sheet or dialog takes its requests with it.
    func cancel(inside root: UIView) { held.removeAll { $0.node?.isDescendant(of: root) ?? true } }
    /// A new document: nothing held, and no reveal goes on moving its page.
    func cancelAll() {
        held.removeAll()
        for scroll in reveals.values.compactMap(\.scroll) { cutReveal(scroll) }
        reveals.removeAll()
    }
    /// The person took a scroller: a request it would move is dropped and
    /// stays dropped, so nothing scrolls it back once the gesture ends. A
    /// reveal running on it is over: UIKit stops the animation for the drag.
    func userScrolled(_ scroll: UIScrollView) {
        FrameSampler.scrolled(scroll, in: presenter.session, person: true)
        reveals[ObjectIdentifier(scroll)] = nil
        held.removeAll { entry in
            guard let node = entry.node else { return true }
            return node.isDescendant(of: scroll) || presenter.groupedLists?.scroller(for: node.id) === scroll
        }
    }

    /// Carry out each request whose target can be scrolled to, outside a
    /// batch; keep it while a presentation or navigation is settling. A
    /// target replaced, recycled into another row, or on an inactive route
    /// cancels its request.
    func flush() {
        guard !held.isEmpty, !presenter.applying else { return }
        let settling = presenter.settlesElementScroll
        for entry in held {
            guard let node = entry.node, node.incarnation == entry.incarnation, presenter.views[node.id] === node,
                  node.props["id"] == entry.name, !presenter.navigation.isInactiveRoute(containing: node) else {
                drop(entry.name)
                presenter.session?.log("scrollIntoView \"\(entry.name)\" cancelled: its target was replaced or left")
                continue
            }
            guard presenter.canScrollIntoView(node) else { continue }
            presenter.scrollIntoView(node, entry.args)
            if !settling { drop(entry.name) }
        }
    }

    /// Reveals UIKit is animating (`setContentOffset(_:animated: true)`),
    /// one a scroller: where it is headed and what it does on arriving.
    /// UIKit reports the end of an animation that finishes, and of one a
    /// write here stops (`cutting`), but not always of one stopped another
    /// way (a scroller recycled with its delegate detached, or gone), so a
    /// reveal counts as running for `revealBound` at most.
    private struct Reveal {
        weak var scroll: UIScrollView?
        let to: CGPoint
        let until: TimeInterval
        /// Its target's node id and incarnation.
        let target: (UInt32, UInt64)
        /// Where the port aligns its target now, if the target is still there.
        let realigned: () -> CGPoint?
        let arrived: () -> Void
    }
    private var reveals: [ObjectIdentifier: Reveal] = [:]
    static let revealBound: TimeInterval = 1

    private func reveal(on scroll: UIScrollView) -> Reveal? {
        let key = ObjectIdentifier(scroll)
        guard let reveal = reveals[key] else { return nil }
        guard reveal.scroll === scroll, CACurrentMediaTime() < reveal.until else { reveals[key] = nil; return nil }
        return reveal
    }
    /// Where a reveal running on `scroll` is headed.
    func revealing(_ scroll: UIScrollView) -> CGPoint? { reveal(on: scroll)?.to }
    var revealsInFlight: Bool {
        reveals.values.compactMap(\.scroll).contains { revealing($0) != nil }
    }
    func revealTarget(_ scroll: UIScrollView) -> (UInt32, UInt64)? { reveal(on: scroll)?.target }
    func revealBegan(_ scroll: UIScrollView, to: CGPoint, target: (UInt32, UInt64), realigned: @escaping () -> CGPoint?, arrived: @escaping () -> Void) {
        let now = CACurrentMediaTime()
        reveals = reveals.filter { $0.value.scroll != nil && now < $0.value.until }
        reveals[ObjectIdentifier(scroll)] = Reveal(scroll: scroll, to: to, until: now + Self.revealBound, target: target, realigned: realigned, arrived: arrived)
    }
    /// UIKit's scroll animation on `scroll` ended: the reveal's, if one ran.
    func revealEnded(_ scroll: UIScrollView) {
        guard let reveal = reveal(on: scroll) else { return }
        reveals[ObjectIdentifier(scroll)] = nil
        reveal.arrived()
    }
    /// Content under a running reveal moved (a list's anchor correction,
    /// scroll anchoring): it heads for where its target now is, from where
    /// it is, and whether it moved says so. A target gone ends it there.
    @discardableResult func retarget(_ scroll: UIScrollView) -> Bool {
        guard let reveal = reveal(on: scroll) else { return false }
        guard let destination = reveal.realigned() else { cutReveal(scroll); return true }
        let to = PixelGrid.offset(destination, in: scroll)
        guard to != reveal.to else { return false }
        revealBegan(scroll, to: to, target: reveal.target, realigned: reveal.realigned, arrived: reveal.arrived)
        cutting = scroll
        defer { cutting = nil }
        scroll.setContentOffset(PixelGrid.offset(to, in: scroll), animated: true)
        return true
    }
    /// The scroller goes to another list (`NodePool`), its motion stopped.
    func forget(_ scroll: UIScrollView) { reveals[ObjectIdentifier(scroll)] = nil }
    /// The scroller a reveal is being cut on: UIKit reports the animation
    /// ended from inside the write that stops it, and the scroll goes on.
    private(set) weak var cutting: UIScrollView?
    /// Stop the reveal running on `scroll` where it is, as another request
    /// or an authored offset replaces it.
    func cutReveal(_ scroll: UIScrollView) {
        guard reveal(on: scroll) != nil else { return }
        reveals[ObjectIdentifier(scroll)] = nil
        cutting = scroll
        defer { cutting = nil }
        scroll.setContentOffset(scroll.contentOffset, animated: false)
    }
}

extension Presenter {
    var settlesElementScroll: Bool { navigation.defersFocus || modals.defersFocus }

    /// Whether `target` is laid out where it can be scrolled to: in a
    /// window, with a size. A row of a grouped list counts once its window
    /// is there: its authored original is a hidden sibling of the native
    /// collection, which projects the row (or, offscreen, scrolls to the
    /// original's place).
    func canScrollIntoView(_ target: NodeView) -> Bool {
        guard target.window != nil, session?.fillInFlight != true else { return false }
        return groupedLists?.scroller(for: target.id) != nil || target.bounds.width > 0 || target.bounds.height > 0
    }

    func scrollElementIntoView(_ args: [Any]) {
        guard let name = args.first as? String,
              let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else {
            session?.log("scrollIntoView \"\(args.first ?? "")\" refused: no live node with that id")
            return
        }
        guard canScrollIntoView(target), !applying else {
            elementScrolls.hold(target, name, args)
            session?.log("scrollIntoView \"\(name)\" waits for its target to mount or its pending layout to finish")
            return
        }
        scrollIntoView(target, args)
        if settlesElementScroll { elementScrolls.hold(target, name, args) } else { elementScrolls.drop(name) }
    }

    func scrollIntoView(_ target: NodeView, _ args: [Any]) {
        let (block, inline) = IntoView.options(args)
        let grouped = groupedLists?.scroller(for: target.id).flatMap { scroll in
            scroll.window === target.window && groupedLists?.projectedRect(for: target, in: scroll) != nil ? scroll : nil
        }
        // UIKit's `scrollToRow` lands on the row: estimated rows on the way would not.
        if let grouped { groupedLists?.measure(toward: target, in: grouped) }
        var scrollers: [UIScrollView] = []
        for view in sequence(first: target.superview, next: { $0?.superview }).compactMap({ $0 }) {
            if let sv = view as? UIScrollView, sv.isScrollEnabled { scrollers.append(sv) }
            // A standard cell's authored original is a sibling of the native
            // collection. Insert that port at its owner, after any nested one.
            if let grouped, view === grouped.superview, grouped.isScrollEnabled,
               !scrollers.contains(where: { $0 === grouped }) { scrollers.append(grouped) }
        }
        let animates = IntoView.animates(args) && target.window != nil
        // The target's box where it will be once the inner scrollers'
        // reveals arrive, and where it is now, in the last of them.
        for (index, sv) in scrollers.enumerated() {
            // The inner ports, weakly, for a destination asked again later.
            let inners = scrollers[..<index].map { inner in { [weak inner] in inner } }
            let projected = projection(target, through: inners.compactMap { $0() }, grouped: grouped, block: block, inline: inline)
            let to = PixelGrid.offset(elementDestination(target, in: sv, grouped: grouped, block: block, inline: inline, projected: projected).to, in: sv)
            let topInset = sv.adjustedContentInset.top
            // Where this port aligns the target now, the inner ports headed
            // where their reveals are going.
            let realigned = { [weak self, weak target, weak sv, weak grouped, id = target.id, incarnation = target.incarnation] () -> CGPoint? in
                guard let self, let target, let sv, self.views[id] === target, target.incarnation == incarnation else { return nil }
                let projected = self.projection(target, through: inners.compactMap { $0() }, grouped: grouped, block: block, inline: inline)
                return PixelGrid.offset(self.elementDestination(target, in: sv, grouped: grouped, block: block, inline: inline, projected: projected).to, in: sv)
            }
            // A jump that collapses a large title moves the offset by the
            // inset it took away, past the range's end near the bottom; the
            // next batch's restore clamped it there a moment later. Settle
            // that transition and align once more, against the new port.
            let settle = { [weak sv] in
                guard let sv, sv.adjustedContentInset.top != topInset else { return }
                sv.layoutIfNeeded()
                if let settled = realigned(), settled != sv.contentOffset { sv.setContentOffset(PixelGrid.offset(settled, in: sv), animated: false) }
            }
            let key = (target.id, target.incarnation)
            // A reveal already headed there goes on: a held request carried
            // out again, or another target the same offset aligns, which
            // it now follows.
            if animates, elementScrolls.revealing(sv) == to {
                if elementScrolls.revealTarget(sv).map({ $0 != key }) ?? true {
                    elementScrolls.revealBegan(sv, to: to, target: key, realigned: realigned, arrived: settle)
                }
                continue
            }
            // A new request replaces a running reveal, even one needing no move.
            elementScrolls.cutReveal(sv)
            guard to != sv.contentOffset else { continue }
            guard animates else { sv.setContentOffset(PixelGrid.offset(to, in: sv), animated: false); settle(); continue }
            // UIKit's own scroll animation; its end ends the reveal and
            // releases the scroller's `scrollend` (`ScrollEndIOS.swift`).
            sv.setContentOffset(PixelGrid.offset(to, in: sv), animated: true)
            elementScrolls.revealBegan(sv, to: to, target: key, realigned: realigned, arrived: settle)
        }
    }

    /// Where the target's box will be once the reveals running on `inners`
    /// (innermost first) arrive, in the outermost of them; where it is, for
    /// a port none moves.
    private func projection(_ target: NodeView, through inners: [UIScrollView], grouped: UIScrollView?, block: String, inline: String) -> Projected? {
        var projected: Projected?
        for inner in inners {
            let frame = elementDestination(target, in: inner, grouped: grouped, block: block, inline: inline, projected: projected).frame
            let at = elementScrolls.revealing(inner) ?? inner.contentOffset
            projected = Projected(view: inner, now: target.convert(target.bounds, to: inner),
                                  then: frame.offsetBy(dx: inner.contentOffset.x - at.x, dy: inner.contentOffset.y - at.y))
        }
        return projected
    }

    /// The target's box in `sv` and where `sv`'s offset goes to align it,
    /// measured now: a layout can size a native row that had an estimated
    /// height. With `projected`, the box is where the inner reveals take it.
    private func elementDestination(_ target: NodeView, in sv: UIScrollView, grouped: UIScrollView?, block: String, inline: String,
                                    projected: Projected?) -> (frame: CGRect, to: CGPoint) {
        // Content room belongs to the range; only UI insets (a bar or
        // keyboard) obstruct the viewport used for CSS alignment.
        let owner = (sv.superview as? NodeView).flatMap { $0.scrollView === sv ? $0 : nil }
        let frame: CGRect
        if let grouped, sv === grouped, let row = groupedLists?.projectedRect(for: target, in: grouped) {
            // Inner authored scrolls use the target's own box. The native
            // row frame projects only into its collection and outer ports,
            // moved as far as the inner reveals will move the target.
            let move = projected.map { $0.view.convert($0.then, to: grouped).origin - $0.view.convert($0.now, to: grouped).origin } ?? .zero
            frame = row.offsetBy(dx: move.x, dy: move.y)
        } else if let projected {
            frame = projected.view.convert(projected.then, to: sv)
        } else if let grouped, grouped.isDescendant(of: sv), let row = groupedLists?.projectedRect(for: target, in: grouped) {
            frame = grouped.convert(row, to: sv)
        } else {
            frame = target.convert(target.bounds, to: sv)
        }
        let area = IntoView.inset(frame, style: target.style, family: "scroll_margin", outward: true)
        let inset = owner?.scrollPortInsets(sv) ?? sv.adjustedContentInset
        var port = IntoView.inset(sv.pageBounds.inset(by: inset), style: owner?.style ?? [:], family: "scroll_padding")
        if let grouped, sv === grouped, let pinned = pinnedHeader(over: frame, in: grouped) {
            port.origin.y += pinned
            port.size.height = max(0, port.height - pinned)
        }
        let x = IntoView.aligned(inline, target: area.minX, area.maxX, port: port.minX, size: port.width) - (port.minX - sv.bounds.minX)
        let y = IntoView.aligned(block, target: area.minY, area.maxY, port: port.minY, size: port.height) - (port.minY - sv.bounds.minY)
        let rangeInset = sv.adjustedContentInset
        let maxX = max(-rangeInset.left, sv.contentSize.width + rangeInset.right - sv.bounds.width)
        let maxY = max(-rangeInset.top, sv.contentSize.height + rangeInset.bottom - sv.bounds.height)
        return (frame, CGPoint(x: min(max(x, -rangeInset.left), maxX), y: min(max(y, -rangeInset.top), maxY)))
    }

    /// The height of the section header a plain list pins over the port's
    /// top while `row` (in the collection's content) is in its section, as
    /// UIKit pins a plain list's headers: a row brought to the start lands
    /// below it, as UITableView's `scrollToRow(at:at: .top)` and SwiftUI's
    /// `scrollTo` leave it, not under it.
    private func pinnedHeader(over row: CGRect, in grouped: UIScrollView) -> CGFloat? {
        guard let collection = grouped as? UICollectionView, (collection.superview as? NodeView)?.props["listStyle"] == "plain",
              let path = collection.indexPathForItem(at: CGPoint(x: row.midX, y: row.midY)),
              let header = collection.layoutAttributesForSupplementaryElement(
                ofKind: UICollectionView.elementKindSectionHeader, at: IndexPath(item: 0, section: path.section)),
              header.frame.height > 0 else { return nil }
        return header.frame.height
    }
}
#endif

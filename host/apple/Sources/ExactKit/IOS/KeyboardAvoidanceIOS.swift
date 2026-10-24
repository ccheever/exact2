// @ref LLP 1116.003 D2.1 — a focused field and what follows it in its
// section stay above the keyboard, as in a UIKit table or a SwiftUI `Form`:
// the nearest scroller (a `scroll`, a list, a grouped list's collection
// view) scrolls by the least that shows them, in the keyboard's own
// animation (Presenter.applyKeyboard), on focus and on every keyboard frame;
// and a tab container keeps the screen's height under the keys, so its
// floating tab bar is covered by them as UIKit's is, never riding above them
// over the field. The viewport itself ends at the keyboard's top
// (`resizes-content`, LLP 1116 D2; ExactView `fit`).
//
// @ref LLP 1116.008 D8 — and above content pinned over the scroller's bottom
// edge (an absolute bar riding the keyboard): what a pinned bar hides of the
// port is not the field's to be shown in, as a UIKit table whose bottom inset
// takes in its toolbar scrolls the field above it.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// Scroll a node into the part of the viewport the keyboard leaves —
    /// the browser's rule for a focused field — through every scroll
    /// container above it, each moving only as far as it must, never past
    /// its edges. Animated by whatever animation block this runs inside.
    /// The nearest one keeps the rest of the node's section in view too when
    /// all of it fits (`kept`): the Add button under an inline number field.
    /// With `rect`, in the node's space, only that part of it is shown.
    func reveal(_ node: NodeView?, rect: CGRect? = nil) {
        guard let node, node.window != nil else { return }
        if surface(containing: node)?.kind == .popover { menus.rehostPopovers() }
        var v: UIView? = node.superview, nearest = true
        while let cur = v {
            var target: UIScrollView? = cur is ScrollView || cur is GroupedScroller ? cur as? UIScrollView : nil
            if let waiting = cur as? NodeView, waiting.scrollDormant { waiting.needScroll(); target = waiting.scrollView }
            if let sv = target {
                // A grouped list's collection view puts back the offset it
                // had in the layout that follows a change of its size or
                // insets (`_restoreOrAdjustContentOffset…`), and the bar's
                // large title collapsing as it scrolls is one: it went back
                // to its top under the keys. Each pass lays the window out,
                // so the next one sees what that restored, until it holds;
                // and the rows of the cells it brought in are carried there
                // (a cell kept from before the keys came up showed empty).
                for _ in 0..<(sv is GroupedScroller ? 3 : 1) {
                    guard show(node, rect: rect, in: sv, keep: nearest && rect == nil, covered: pinnedCover(over: sv, field: node)) else { break }
                    guard sv is GroupedScroller else { continue }
                    carryRows(laying: sv)
                    sv.window?.layoutIfNeeded()
                    carryRows(laying: sv)
                }
                nearest = false
            }
            v = cur.superview
        }
    }

    /// Show the caret of the text area being typed into through every
    /// scroller above it, as a browser does after an edit (UIKit and SwiftUI
    /// leave a growing text area's new line under the keys). A browser does
    /// so after the edit's `input` handlers run, so a handler that moved the
    /// focus or hid the text area leaves nothing to show.
    func revealCaret(of node: NodeView) {
        guard let text = node.textArea, editor === node,
              !sequence(first: text as UIView, next: \.superview).contains(where: \.isHidden),
              let end = text.selectedTextRange?.end else { return }
        let caret = text.caretRect(for: end)
        guard !caret.isNull, !caret.isInfinite else { return }
        // Keep the caret within what the text view shows: it scrolls itself
        // to the caret only later, and a large font's caret can be taller
        // than a short text area
        let shown = text.bounds, height = min(caret.height, shown.height)
        let y = min(max(caret.minY, shown.minY), shown.maxY - height)
        reveal(node, rect: text.convert(CGRect(x: caret.minX, y: y, width: caret.width, height: height), to: node))
    }

    /// The colour the content at the viewport's bottom edge goes on with
    /// below it (ExactView `paintUnderKeys`): the background of the scroller
    /// there (a grouped list's), or the nearest painted box above it (a
    /// route's backdrop), never a row's own. Nil where nothing is painted.
    func bottomEdgeColor() -> UIColor? {
        let b = viewport.bounds.inset(by: pageMargins) // the page's area, not the band under it
        guard b.height > 1, let hit = viewport.hitTest(CGPoint(x: b.midX, y: b.maxY - 1), with: nil) else { return nil }
        let chain = Array(sequence(first: hit, next: { $0 === self.viewport ? nil : $0.superview }))
        let from = chain.firstIndex { $0 is UIScrollView } ?? 0
        return chain[from...].lazy.compactMap { v in
            v.backgroundColor.flatMap { $0.resolvedColor(with: v.traitCollection).cgColor.alpha > 0.99 ? $0 : nil }
        }.first
    }

    /// How much of the bottom of `sv`'s port content pinned over it hides:
    /// the tallest box that is painted over the scroller (a later sibling of
    /// it or of one of its ancestors, taken out of the flow by `position:
    /// absolute`), spans most of its width and rests on its bottom edge.
    /// Where a bar rides the keyboard's top over the scroller, the focused
    /// field is shown above the bar, not under it (book-club's Add button over
    /// its Author field). A box the field sits in is not a cover for it.
    func pinnedCover(over sv: UIScrollView, field node: NodeView) -> CGFloat {
        guard let window = sv.window else { return 0 }
        let owner = (sv.superview as? NodeView).flatMap { $0.scrollView === sv ? $0 : nil }
        let port = sv.convert(sv.pageBounds.inset(by: owner?.scrollPortInsets(sv) ?? sv.adjustedContentInset), to: window)
        guard port.height > 1, port.width > 1 else { return 0 }
        var top = port.maxY
        func consider(_ box: NodeView) {
            guard !box.isHidden, box.alpha > 0.01, box !== sv, box.window === window, !node.isDescendant(of: box),
                  box.style["pointer_events"]?.string != "none" else { return }
            let frame = box.convert(box.bounds, to: window)
            let across = min(frame.maxX, port.maxX) - max(frame.minX, port.minX)
            guard across >= port.width * 0.5, frame.maxY >= port.maxY - 40, frame.minY < port.maxY - 1,
                  frame.minY >= port.minY + port.height * 0.4 else { return }
            top = min(top, frame.minY)
        }
        var child: UIView = sv, up = sv.superview
        while let parent = up {
            if let at = parent.subviews.firstIndex(of: child) {
                for case let sibling as NodeView in parent.subviews[(at + 1)...] where sibling.layoutPosition == "absolute" {
                    consider(sibling)
                }
            }
            child = parent
            up = parent.superview
        }
        return port.maxY - top
    }

    /// A grouped list laid out at its offset and the rows of its cells
    /// carried into them: put in place, not slid there by the animation
    /// this may run inside, which moves only the list's offset.
    func carryRows(laying list: UIScrollView) {
        UIView.performWithoutAnimation {
            list.layoutIfNeeded()
            groupedLists?.sync(changed: [])
        }
    }

    /// Scroll `sv` by the least that shows the node (or `rect` in it), with a
    /// little air, and with `keep` the rest of its section when all of it
    /// fits; whether it moved.
    private func show(_ node: NodeView, rect: CGRect? = nil, in sv: UIScrollView, keep: Bool, covered: CGFloat = 0) -> Bool {
        let field = node.convert(rect ?? node.bounds, to: sv)
        let owner = (sv.superview as? NodeView).flatMap { $0.scrollView === sv ? $0 : nil }
        var visible = sv.pageBounds.inset(by: owner?.scrollPortInsets(sv) ?? sv.adjustedContentInset)
        visible.size.height = max(0, visible.height - covered)
        var r = field.insetBy(dx: 0, dy: -8)
        if keep, let span = Self.kept(node, field: field, in: sv) {
            if span.height + 16 <= visible.height {
                r = CGRect(x: field.minX, y: span.minY, width: field.width, height: span.height).insetBy(dx: 0, dy: -8)
            } else if span.maxY > field.maxY {
                // Too tall to show whole: the field at the top and as much of
                // what follows it as fits, its hint first.
                let end = min(span.maxY, field.minY + visible.height - 16)
                r = CGRect(x: field.minX, y: field.minY, width: field.width, height: max(field.height, end - field.minY)).insetBy(dx: 0, dy: -8)
            }
        }
        var o = sv.contentOffset
        if r.maxY > visible.maxY { o.y += r.maxY - visible.maxY } else if r.minY < visible.minY { o.y -= visible.minY - r.minY }
        if r.maxX > visible.maxX { o.x += r.maxX - visible.maxX } else if r.minX < visible.minX { o.x -= visible.minX - r.minX }
        // A field at the end of the content, under a bar pinned over it, has no
        // offset that shows it above the bar: the scroller takes room below
        // its content for as long as the keyboard is up (`PinnedRoom`), and
        // none once it is down (a field still focused as it goes, revealed
        // again after the room was given back, would keep that room at rest).
        if covered > 0, keyboardTop != nil {
            let high = sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height
            if o.y > high + 0.5 { pinnedRoom.grow(sv, by: o.y - high) }
        }
        let insets = sv.adjustedContentInset
        guard abs(o.y - sv.contentOffset.y) > 0.5 || abs(o.x - sv.contentOffset.x) > 0.5 else { return false }
        o = PixelGrid.offset(o, in: sv)
        guard o != sv.contentOffset else { return false }
        sv.contentOffset = o
        // The bar's large title collapsing as the list scrolls changes its
        // insets inside this assignment, and UIKit moves the offset on by the
        // title's height to keep the content still: past the least scroll,
        // with the field shown, so no later pass takes it back. The offset
        // asked for, within the new insets, is the least.
        if sv.adjustedContentInset != insets {
            let held = PixelGrid.offset(o, in: sv)
            if held != sv.contentOffset { sv.contentOffset = held }
        }
        return true
    }

    /// From the node's row to the end of its section, in `sv`'s content
    /// space: in a grouped list, the rest of the list's section (its cells,
    /// laid out or not); elsewhere, the rest of the nearest `section`, `form`
    /// or `fieldset` around the node inside the scroller, else of its parent.
    static func kept(_ node: NodeView, field: CGRect, in sv: UIScrollView) -> CGRect? {
        if let list = sv as? UICollectionView {
            var at: UIView? = node
            while let view = at, view.superview !== list { at = view.superview }
            guard let cell = at as? UICollectionViewCell, let path = list.indexPath(for: cell) else { return nil }
            let count = list.numberOfItems(inSection: path.section)
            guard count > 0, let last = list.layoutAttributesForItem(at: IndexPath(item: count - 1, section: path.section)) else { return nil }
            return CGRect(x: cell.frame.minX, y: cell.frame.minY, width: cell.frame.width, height: max(0, last.frame.maxY - cell.frame.minY))
        }
        var group: NodeView?, at = node.superview
        while let view = at, view !== sv {
            if let box = view as? NodeView {
                group = group ?? box
                if ["section", "form", "fieldset"].contains(box.props["semanticTag"] ?? "") { group = box; break }
            }
            at = view.superview
        }
        guard let group else { return nil }
        let box = group.convert(group.bounds, to: sv)
        return CGRect(x: box.minX, y: field.minY, width: box.width, height: max(field.height, box.maxY - field.minY))
    }
}

/// The room below a scroller's content that lets a field at its end be shown
/// above content pinned over its bottom edge (LLP 1116.008 D8): what a hand-
/// built screen's bottom inset does for its toolbar. Taken as the field is
/// revealed and given back when the keyboard goes, never at rest. The active
/// scroll backend keeps this room alongside native content padding.
final class PinnedRoom {
    private struct Grown {
        weak var scroll: UIScrollView?
        var added: CGFloat
    }
    private var grown: [ObjectIdentifier: Grown] = [:]

    /// `extra` more points below the content, on top of what it already took.
    /// Only its own share of the inset is added and taken back: a page band
    /// may change the rest meanwhile (LLP 1130 D3).
    func grow(_ sv: UIScrollView, by extra: CGFloat) {
        let id = ObjectIdentifier(sv)
        var room = grown[id] ?? Grown(scroll: sv, added: 0)
        room.added += extra
        sv.contentInset.bottom += extra
        grown[id] = room
    }

    /// The keyboard went: each scroller keeps the inset it had, and an offset
    /// that was past its end goes back to it.
    func release() {
        for room in grown.values {
            guard let sv = room.scroll else { continue }
            sv.contentInset.bottom -= room.added
            let high = max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)
            if sv.contentOffset.y > high { sv.contentOffset = PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: high), in: sv) }
        }
        grown = [:]
    }
}

// @ref LLP 1115.000 D9.1 — how a drag puts the keyboard away.
extension NodeView {
    /// What a drag on this node's scroller does to the keyboard. Unsaid, on
    /// a scroller that scrolls vertically, the keyboard follows a drag that
    /// reaches it (`.interactive`), as a hand-built list or form has it; it
    /// never takes the keyboard from someone who was only scrolling. A
    /// sideways scroller keeps UIKit's `.none`. An authored
    /// `keyboardDismissMode` wins, `none` included: the prop has no default
    /// row, so unsaid it is absent from the node's props, and absence is the
    /// whole test.
    var keyboardDismissal: UIScrollView.KeyboardDismissMode {
        switch props["keyboardDismissMode"] {
        case "interactive": return .interactive
        case "on-drag": return .onDrag
        case "none": return .none
        default: return scrollsVertically ? .interactive : .none
        }
    }
    /// Written again when the prop or the axes the node scrolls change; a
    /// grouped list's collection view takes it from here (`mount`).
    func updateKeyboardDismissal() {
        guard let scroll = scrollView, scroll.keyboardDismissMode != keyboardDismissal else { return }
        scroll.keyboardDismissMode = keyboardDismissal
    }
}

extension Presenter {
    /// Return with `enterkeyhint="next"`: the next field, shown as focus
    /// shows any (`reveal`). In a grouped list that is the list's own order,
    /// which its cells among a window's views are not, and a row scrolled
    /// away has no cell, its views hidden beneath the list: the list scrolls
    /// to that row, which carries it into a cell, and its field takes focus,
    /// as SwiftUI's focus moves through a `Form`. Past the list's last field
    /// the keyboard goes; outside a list, the window's next field.
    func focusNextField(after node: NodeView) -> Bool {
        guard let (list, owner, path) = Self.listCell(of: node), let rows = groupedLists?.rows(for: owner.id),
              rows.indices.contains(path.section), rows[path.section].indices.contains(path.item) else {
            return moveFocus(backward: false, fields: true)
        }
        // The fields after `node` in its own row, then each later row's.
        var candidates = Self.fields(in: views[rows[path.section][path.item]]).drop { $0 !== node }.dropFirst().map { (path, $0) }
        for (s, section) in rows.enumerated() where s >= path.section {
            for (i, row) in section.enumerated() where s > path.section || i > path.item {
                candidates += Self.fields(in: views[row]).map { (IndexPath(item: i, section: s), $0) }
            }
            if !candidates.isEmpty { break }
        }
        guard let (at, next) = candidates.first, let field = next.field ?? next.textArea else { return false }
        let hidden = sequence(first: next as UIView, next: \.superview).contains { $0.isHidden }
        if next.window == nil || hidden {
            // Its row has no cell: bring it into the list's port first,
            // which makes its cell and carries the row there.
            guard let frame = list.layoutAttributesForItem(at: at)?.frame else { return false }
            let visible = list.bounds.inset(by: list.adjustedContentInset), i = list.adjustedContentInset
            var y = list.contentOffset.y
            if frame.maxY > visible.maxY { y += frame.maxY - visible.maxY } else if frame.minY < visible.minY { y -= visible.minY - frame.minY }
            y = min(max(y, -i.top), max(-i.top, list.contentSize.height + i.bottom - list.bounds.height))
            if ExactEnv.agentFreezes { list.contentOffset = PixelGrid.offset(CGPoint(x: list.contentOffset.x, y: y), in: list) } else { UIView.animate(withDuration: 0.25) { list.contentOffset = PixelGrid.offset(CGPoint(x: list.contentOffset.x, y: y), in: list) } }
            carryRows(laying: list)
        }
        chooseFocus(next)
        return field.becomeFirstResponder()
    }

    /// The grouped list cell `node` is carried in: its collection view, the
    /// list's node and the row's place.
    private static func listCell(of node: NodeView) -> (UICollectionView, NodeView, IndexPath)? {
        var at: UIView? = node
        while let view = at, !(view is UICollectionViewCell && view.superview is GroupedScroller) { at = view.superview }
        guard let cell = at as? UICollectionViewCell, let list = cell.superview as? UICollectionView,
              let owner = list.superview as? NodeView, let path = list.indexPath(for: cell) else { return nil }
        return (list, owner, path)
    }

    /// The enabled text fields and areas in a row, in document order.
    private static func fields(in row: NodeView?) -> [NodeView] {
        guard let row else { return [] }
        var out: [NodeView] = []
        func walk(_ node: NodeView) {
            if (node.field != nil || node.textArea != nil), !node.disabled, node.props["inert"] != "true" { out.append(node) }
            for case let child as NodeView in node.container.subviews { walk(child) }
        }
        walk(row)
        return out
    }
}

extension NavigationHost {
    /// The primary controller fills the document viewport. A tab container
    /// extends beneath a content-resizing keyboard so its tab bar stays
    /// covered while route allocations end at the keys.
    func ownerFrame(_ owner: UIViewController, in root: UIView) -> CGRect {
        let viewport = presenter.viewport
        var frame = !ExactEnv.authoredChrome && root.isDescendant(of: viewport) ? viewport.convert(viewport.pageBounds, to: root) : root.bounds
        frame.size.height += ownerKeyboardExtension(owner)
        // The authored tablist the agent paints takes its own place in the
        // root, as on the web; the routes end at its edge, not under it.
        if owner === tabOwner, ExactEnv.authoredChrome, let list = adoptedTablist.flatMap({ presenter.views[$0] }),
           list.style["display"]?.string != "none", list.isDescendant(of: root) {
            let rect = list.convert(list.bounds, to: root)
            if rect.height > 0, rect.minY >= frame.midY {
                frame.size.height = max(0, min(frame.maxY, rect.minY) - frame.minY)
            } else if rect.height > 0, rect.maxY <= frame.midY {
                let cut = max(0, rect.maxY - frame.minY)
                frame.origin.y += cut
                frame.size.height = max(0, frame.height - cut)
            }
        }
        return frame
    }

    func ownerKeyboardExtension(_ owner: UIViewController) -> CGFloat {
        if owner === tabOwner, !ExactEnv.authoredChrome, !presenter.modals.active, presenter.keyboardPolicy == "resizes-content" {
            return presenter.keyboardInset
        }
        #if os(iOS)
        if owner === primaryOwner, presenter.keyboardPolicy == "resizes-content" { return presenter.keyboardAccessoryHeight }
        #endif
        return 0
    }
}
#endif

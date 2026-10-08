#if os(iOS) || os(tvOS)
import UIKit

/// A plain container: the document, a canvas's overlay (LLP 1014). Hit-
/// testable at alpha 0 — a canvas's children painted through its surface
/// composite at alpha 0 and must still take a tap, which UIKit's default
/// hit-test refuses below 0.01 — and transparent to a hit on nothing, so
/// the touch reaches what holds it (the canvas, the viewport).
package final class PlainView: UIView {
    package override func didAddSubview(_ subview: UIView) { super.didAddSubview(subview); FocusSearch.joined(subview) }
    package override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
        return NodeView.hitChildren(in: self, at: point, with: event)
    }
}

/// A scroll container — the viewport over the document, and a node whose
/// effective `overflow` scrolls. The platform pans it (LLP 1002 D4: scroll
/// always wins; UIKit does not chain a pan out of a nested scroll view at
/// its edge). Which axes it scrolls comes from the node's rows; a tap's
/// wheel (the agent's) applies the web's chaining rule itself
/// (`AgentIOS.swift`).
package class ScrollView: UIScrollView {
    package override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard let hit = super.hitTest(point, with: event) else { return nil }
        return NodeView.hitChildren(in: self, at: point, with: event) ?? hit
    }

    package var scrollsX = true
    package var scrollsY = true
    #if os(tvOS)
    /// The offset the remote's last step scrolls to, and when it began
    /// (`RemoteTVOS.swift`): a step pressed during that animation goes on
    /// from there.
    var remoteStepTarget: (offset: CGPoint, at: CFTimeInterval)?
    #endif
    /// A pan cancels a touch in progress, as it does a custom button's; UIKit
    /// would leave a `UIControl` its touch, so a native button (LLP 1069.011
    /// D4) is named. A canvas that owns its input keeps it.
    package override func touchesShouldCancel(in view: UIView) -> Bool {
        !CanvasInputs.owns(view) && (view is NativeButtonIOS || super.touchesShouldCancel(in: view))
    }
    package override func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
        if gesture === panGestureRecognizer {
            let velocity = panGestureRecognizer.velocity(in: self)
            let location = panGestureRecognizer.location(in: self)
            let translation = panGestureRecognizer.translation(in: self)
            if !admitsPan(velocity: velocity, translation: translation, start: CGPoint(x: location.x - translation.x, y: location.y - translation.y)) { return false }
        }
        return super.gestureRecognizerShouldBegin(gesture)
    }
    /// The pan's own check, apart from UIKit's: `touch-action` intersected
    /// from the node hit at `start` through this container, then chaining.
    /// The direction is the velocity, or the movement that crossed the slop
    /// while UIKit has none yet, as a photo's yield reads it (LLP 1057.001
    /// rule 2), so the drag it steps aside from is taken here.
    func admitsPan(velocity: CGPoint, translation: CGPoint, start: CGPoint) -> Bool {
        let direction = velocity == .zero ? translation : velocity
        var view = hitTest(start, with: nil)
        if CanvasInputs.owns(view) { return false }
        // CSS intersects touch-action from the hit element through the
        // scroll container. It governs initial direction, not reversal.
        while let current = view {
            if let node = current as? NodeView, !node.allowsTouchPan(direction) { return false }
            if current === self { break }
            view = current.superview
        }
        if let owner = superview as? NodeView, !owner.allowsTouchPan(direction) { return false }
        return !handsOff(direction)
    }
    /// CSS's scroll chaining at a gesture's start (LLP 1070 G1, Q4 as ruled
    /// provisionally): under `overscroll-behavior: auto`, a drag that begins
    /// at this view's edge and pushes outward belongs to the enclosing
    /// scroller that can take it, as Chrome and Safari chain it, instead of
    /// UIKit's rubber band here. Only for a known direction, on an axis this
    /// view actually scrolls; `contain` keeps the band, `none` keeps the
    /// gesture with no band. Never mid-gesture: UIKit asks once, at begin.
    func handsOff(_ velocity: CGPoint) -> Bool {
        guard velocity != .zero, let owner = superview as? NodeView else { return false }
        let horizontal = abs(velocity.x) > abs(velocity.y)
        let behavior = owner.style[horizontal ? "overscroll_behavior_x" : "overscroll_behavior_y"]?.string ?? "auto"
        // One flag for both axes, written each gesture so a style that leaves
        // `none` gets its band back: off for this drag's `none`, or for an
        // axis this view scrolls under `none`.
        let still = { (axis: String) in owner.style["overscroll_behavior_\(axis)"]?.string == "none" }
        bounces = !(behavior == "none" || (scrollsX && still("x")) || (scrollsY && still("y")))
        return chains(velocity)
    }
    /// `handsOff`'s answer without its side effect: whether a drag in
    /// `velocity` begun now chains to an enclosing scroller (a descendant's
    /// recognizer asks it, LLP 1057.001 rule 2).
    package func chains(_ velocity: CGPoint) -> Bool {
        guard velocity != .zero, let owner = superview as? NodeView else { return false }
        let horizontal = abs(velocity.x) > abs(velocity.y)
        guard (owner.style[horizontal ? "overscroll_behavior_x" : "overscroll_behavior_y"]?.string ?? "auto") == "auto" else { return false }
        let i = adjustedContentInset
        let (at, low, high, scrolls) = horizontal
            ? (contentOffset.x, -i.left, contentSize.width + i.right - bounds.width, scrollsX)
            : (contentOffset.y, -i.top, contentSize.height + i.bottom - bounds.height, scrollsY)
        guard scrolls, high > low else { return false }
        // A finger moving toward +x pulls the content toward its start.
        let toward = horizontal ? velocity.x : velocity.y
        let atEdge = toward > 0 ? at <= low + 0.5 : at >= high - 0.5
        guard atEdge else { return false }
        // Only when an enclosing scroller can take that direction; with none,
        // the band here is all a finger can have.
        var up = superview
        while let current = up {
            if let outer = current as? UIScrollView, outer is ScrollView || outer is GroupedScroller {
                let o = outer.adjustedContentInset
                let (at, low, high, scrolls) = horizontal
                    ? (outer.contentOffset.x, -o.left, outer.contentSize.width + o.right - outer.bounds.width, (outer as? ScrollView)?.scrollsX ?? false)
                    : (outer.contentOffset.y, -o.top, outer.contentSize.height + o.bottom - outer.bounds.height, (outer as? ScrollView)?.scrollsY ?? (outer.superview as? NodeView)?.scrollsVertically ?? false)
                if scrolls && high > low && (toward > 0 ? at > low + 0.5 : at < high - 0.5) { return true }
            }
            up = current.superview
        }
        return false
    }
    /// A touch that no node took — nothing focusable, nothing pressable —
    /// ends the editing, as a tap on a page's blank ground blurs the field
    /// and sends the keyboard away (LLP 1008 §9).
    package override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        // An inert button can still retain focus, for example between the
        // two taps of a double-tap recognizer. Its unhandled touch is not
        // blank ground. Check before forwarding to enclosing scroll views.
        var target = touches.first?.view
        while let view = target {
            if let node = view as? NodeView {
                if node.presenter?.contextRetainsFocus(node) == true { return }
                break
            }
            target = view.superview
        }
        super.touchesEnded(touches, with: event)
        // A blur is the session's (LLP 1035.001 D5): its viewport's, never the
        // window's — found through the nearest node above a nested scroller;
        // the viewport itself has none above it and is its own.
        if let t = touches.first, bounds.contains(t.location(in: self)) {
            var viewport: UIView = self
            var above = superview
            while let current = above {
                if let node = current as? NodeView, let owned = node.presenter?.viewport { viewport = owned; break }
                above = current.superview
            }
            viewport.endEditing(true)
        }
    }
}

#if !os(tvOS)
/// UIKit's pull-to-refresh control, below the scroller's `padding-top`
/// (@ref LLP 1010 §6.9, the one rule a padded list takes that CSS has no
/// word for: React Native's `progressViewOffset`, placed as its
/// `RCTRefreshControl` places it). The padding is in the content, so a header
/// laid over it would hide the spinner; here the control's top sits that far
/// down the scroller's port, behind the content, and the pull uncovers it
/// between the header and the first row, where it stays while the app
/// refreshes. UIKit lays the control out as the pull goes; after each
/// layout its frame moves to that place, which is a no-op once it is there.
/// With no padding, UIKit's own place.
final class PaddedRefreshControl: UIRefreshControl {
    override func layoutSubviews() {
        super.layoutSubviews()
        guard let owner = superview?.superview as? NodeView else { return }
        let top = owner.number("padding_top")
        guard top != 0 else { return }
        let gap = convert(CGPoint(x: 0, y: top), from: owner).y
        if gap != 0 { frame = frame.offsetBy(dx: 0, dy: gap) }
    }
}
#endif

extension NodeView {
    /// A capability supplies the only physical scroller for this node;
    /// its delegate still owns cells and forwards the shared callbacks.
    package func mountNativeScrollView(_ view: UIScrollView?, contentInsets: UIEdgeInsets = .zero) {
        if extras?.nativeScrollView !== view, let previous = extras?.nativeScrollView {
            retainScrollPosition(from: previous)
        }
        if view != nil || extras?.nativeScrollView != nil {
            more.nativeScrollContentInsets = view == nil ? .zero : contentInsets
        }
        guard extras?.nativeScrollView !== view else { return }
        more.nativeScrollView = view
        more.nativeScrollDelegate = view?.delegate
        if scrollView == nil { presenter?.scrollers.remove(id) }
        else { presenter?.scrollers.insert(id) }
        scrollWritten = nil
        view?.contentInsetAdjustmentBehavior = .never
        view?.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        syncScroll()
        updateRefresh()
        applyAffordances()
        presenter?.navigation.scrollBackendChanged(self)
        presenter?.keyboardToolbars.backendChanged()
    }

    /// A replacement keeps the logical offset; an authored write takes priority.
    func retainScrollPosition(from view: UIScrollView) {
        let authored = extras?.pendingScrollTransfer != true && (pendingScrollTop != nil || pendingScrollLeft != nil)
        if pendingScrollTop == nil { pendingScrollTop = Double(view.contentOffset.y + scrollTopInset(view)) }
        if pendingScrollLeft == nil { pendingScrollLeft = Double(view.contentOffset.x) }
        more.pendingScrollTransfer = !authored
        presenter?.pendingScrolls.insert(id)
    }

    /// UIKit owns a native list's row geometry, including rows offscreen.
    private func scrollRect(_ node: NodeView, in sv: UIScrollView) -> CGRect? {
        if groupedOwner { return presenter?.groupedLists?.projectedRect(for: node, in: sv) }
        return node.isDescendant(of: sv) && !node.isHidden ? node.convert(node.bounds, to: sv) : nil
    }

    func captureScrollPosition() {
        beforeLayoutScroll = scrollView.map { ($0.contentOffset, scrollTopInset($0)) }
        followedScroll = nil
        readingAnchors.removeAll(keepingCapacity: true)
        scrollAnchor = nil
        guard props["scrollFollowEnd"] == "true", let sv = scrollView else {
            activeReadingAnchor = nil; anchoredScrollTop = nil; retainedScrollTop = nil
            if let sv = scrollView {
                if groupedOwner {
                    // UIKit quantizes offsets independently of fractional insets.
                    // A rounding difference at the start is not a reading anchor.
                    if sv.contentOffset.y + sv.adjustedContentInset.top > 0.5 { readingAnchors = presenter?.groupedLists?.scrollAnchors(for: id) ?? [] }
                } else { captureScrollAnchor(sv) }
            }
            return
        }
        let maximum = max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)
        // A retained route can gain height when another route hides the keyboard.
        // That clamp is not the reader choosing the end. Keep its intended offset
        // until the returning viewport can fit it, or the reader scrolls again.
        if anchoredScrollTop != sv.contentOffset.y { retainedScrollTop = nil }
        // An animated follow of the end (below) is still the end, mid-flight.
        let capturedTop = retainedScrollTop ?? sv.contentOffset.y
        followedScroll = (capturedTop,
                          (retainedScrollTop == nil && sv.contentOffset.y >= maximum - 1) || followingEndAnimated,
                          abs(capturedTop + sv.adjustedContentInset.top) <= 0.5)
        guard let followedScroll, !followedScroll.end, followedScroll.top + sv.adjustedContentInset.top > 0.5 else { return }
        // A scroll by the reader invalidates the prior choice. Anchoring's
        // own adjustment does not: keep the same surviving row across batches.
        if anchoredScrollTop == sv.contentOffset.y, let node = activeReadingAnchor,
           presenter?.views[node.id] === node, scrollRect(node, in: sv) != nil, !node.isHidden {
            let rect = scrollRect(node, in: sv) ?? .zero
            if rect.width > 0, rect.height > 0, rect.intersects(sv.bounds) { readingAnchors.append((node, rect.minY)) }
        }
        // Prefer the first fully visible box; descend into a partially visible
        // one. This keeps a message stable when rows above it change height.
        // Coordinates are in the scroll view's content space, not the window.
        func anchors(in view: UIView) {
            for case let node as NodeView in view.subviews where !node.isHidden {
                let rect = node.convert(node.bounds, to: sv)
                guard rect.width > 0, rect.height > 0, rect.intersects(sv.bounds) else { continue }
                if !sv.bounds.contains(rect) { anchors(in: node.container) }
                readingAnchors.append((node, rect.minY))
            }
        }
        // Keep later visible candidates too: a deleted anchor cannot hold the
        // reader's position, but the next surviving message still can.
        if groupedOwner { readingAnchors = presenter?.groupedLists?.scrollAnchors(for: id) ?? [] }
        else { anchors(in: sv) }
    }
    func restoreScrollPosition() {
        defer { followedScroll = nil; readingAnchors.removeAll(keepingCapacity: true); scrollAnchor = nil }
        guard props["scrollFollowEnd"] == "true", let sv = scrollView else {
            if let sv = scrollView {
                if groupedOwner, let anchor = readingAnchors.first(where: { presenter?.views[$0.node.id] === $0.node && scrollRect($0.node, in: sv) != nil }),
                   let rect = scrollRect(anchor.node, in: sv) {
                    let shift = rect.minY - anchor.y
                    let i = sv.adjustedContentInset
                    // Native layout may already have moved the offset. Restore
                    // the captured reading position, counting the row shift once.
                    let top = (beforeLayoutScroll?.offset.y ?? sv.contentOffset.y) + shift
                    let y = sv.isTracking || sv.isDecelerating ? top : min(max(top, -i.top), max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height))
                    if y != sv.contentOffset.y { sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: y), animated: false) }
                } else if !groupedOwner { restoreScrollAnchor(sv) }
            }
            return
        }
        let minimum = -sv.adjustedContentInset.top
        let maximum = max(minimum, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height)
        let prior = followedScroll ?? (top: maximum, end: true, start: false)
        // At rest at the start, UIKit's new minimum follows an inset change.
        // A captured physical offset would replay the obsolete inset instead.
        let restingStart = groupedOwner && prior.start && !(sv.isTracking || sv.isDragging || sv.isDecelerating)
        var top = restingStart ? minimum : prior.top
        activeReadingAnchor = nil
        if !prior.end, let anchor = readingAnchors.first(where: {
            presenter?.views[$0.node.id] === $0.node && scrollRect($0.node, in: sv) != nil
        }) {
            activeReadingAnchor = anchor.node
            top += (scrollRect(anchor.node, in: sv)?.minY ?? anchor.y) - anchor.y
        }
        let y = Self.followedTop(current: sv.contentOffset.y, minimum: minimum, maximum: maximum, end: prior.end, top: top,
                                 moving: sv.isTracking || sv.isDragging || sv.isDecelerating)
        let inactive = window == nil || presenter?.navigation.isInactiveRoute(containing: self) == true
        retainedScrollTop = !prior.end && top > maximum && (inactive || retainedScrollTop != nil) ? top : nil
        // While the reader's finger is down or the fling is running, an
        // absolute write would cut the pan, the deceleration or the rubber
        // band (a batch every 250 ms yanked a bottom overscroll back to the
        // end, mid-drag). Follow the end once the interaction is over, and
        // keep a surviving row in place by moving the offset by its shift
        // only, without clamping, as UIKit's own contentOffsetAdjustment does.
        if sv.isTracking || sv.isDecelerating {
            if prior.end { followsEndAfterInteraction = true }
            else if top != prior.top {
                if groupedOwner {
                    if sv.contentOffset.y != top { sv.contentOffset.y = top }
                }
                else { sv.contentOffset.y += top - prior.top }
            }
            anchoredScrollTop = sv.contentOffset.y
            return
        }
        if sv.contentOffset.y != y {
            // `scroll-behavior: smooth`: an end that moved down (a message
            // appended) is followed with UIKit's scroll animation, as a
            // smooth `scrollTop` write is; a shrink or a jump up lands at once.
            let animate = prior.end && y > sv.contentOffset.y && style["scroll_behavior"]?.string == "smooth" && !ExactEnv.agentFreezes && window != nil
            followingEndAnimated = animate
            sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: y), animated: animate)
        }
        // UIKit quantizes the assigned offset. Compare its actual stored value
        // next time so that rounding cannot masquerade as a reader's scroll.
        anchoredScrollTop = sv.contentOffset.y
    }


    func applyPendingScroll() {
        defer { pendingScrollTop = nil; pendingScrollLeft = nil }
        guard let sv = scrollView else { return }
        // An authored position takes over from a smooth correction.
        if (pendingScrollTop != nil || pendingScrollLeft != nil), presenter?.collections.animating.contains(id) == true {
            presenter?.collections.stopAnimation(id)
        }
        if pendingScrollTop != nil { retainedScrollTop = nil }
        guard hasScrollLayoutBox else {
            if hiddenScroll == nil {
                hiddenScroll = beforeLayoutScroll.map { CGPoint(x: $0.offset.x, y: $0.offset.y + $0.topInset) } ?? .zero
            }
            return
        }
        if let saved = hiddenScroll {
            hiddenScroll = nil
            writeScrollPosition(in: sv, left: Double(saved.x), top: Double(saved.y), animated: false, titleSlack: false)
        }
        guard pendingScrollTop != nil || pendingScrollLeft != nil else { return }
        // `scroll-behavior: smooth` (CSS) animates a prop write, never a
        // reader's own scroll. Under the agent's frozen clock it lands at once
        // (as a modal presents, LLP 1035.003 D5), so `layout` reads the target.
        let smooth = extras?.pendingScrollTransfer != true && style["scroll_behavior"]?.string == "smooth" && !ExactEnv.agentFreezes
        writeScrollPosition(in: sv, left: pendingScrollLeft, top: pendingScrollTop, animated: smooth, titleSlack: true)
    }

    /// Convert a logical write only after native layout. A collection can
    /// invalidate its offset when that write collapses the navigation title;
    /// settle that one inset transition and apply the same intent once more.
    private func writeScrollPosition(in sv: UIScrollView, left: Double?, top: Double?, animated: Bool, titleSlack: Bool) {
        func target() -> CGPoint {
            let i = sv.adjustedContentInset, inset = scrollTopInset(sv)
            // CSS ends where a large title rests collapsed.
            let slack = titleSlack && scrollOrigin > 0 ? max(0, i.top - scrollCollapsed) : 0
            let y = top.map { CGFloat($0) - inset == sv.contentOffset.y ? sv.contentOffset.y : min(max(CGFloat($0) - inset, -i.top), max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height - slack)) } ?? sv.contentOffset.y
            let x = left.map { CGFloat($0) == sv.contentOffset.x ? sv.contentOffset.x : min(max(CGFloat($0), -i.left), max(-i.left, sv.contentSize.width + i.right - sv.bounds.width)) } ?? sv.contentOffset.x
            return CGPoint(x: x, y: y)
        }
        let initial = target()
        guard sv.contentOffset != initial else { return }
        let inset = sv.adjustedContentInset.top
        sv.setContentOffset(initial, animated: animated)
        if groupedOwner, !animated, sv.adjustedContentInset.top != inset {
            sv.layoutIfNeeded()
            let settled = target()
            if sv.contentOffset != settled { sv.setContentOffset(settled, animated: false) }
        }
    }
    /// A `refresh` handler on a scroll container is UIKit's pull-to-refresh:
    /// the control fires the event; the app's `refreshing` going false ends it.
    func updateRefresh() {
        // tvOS has no refresh control.
        #if !os(tvOS)
        guard let sv = scrollView else { return }
        if handlers.contains("refresh") {
            if sv.refreshControl == nil {
                let control = PaddedRefreshControl()
                control.addTarget(self, action: #selector(pulledToRefresh), for: .valueChanged)
                sv.refreshControl = control
            }
            if props["refreshing"] != "true", let control = sv.refreshControl, control.isRefreshing {
                control.endRefreshing()
            }
        } else if sv.refreshControl != nil {
            sv.refreshControl = nil
        }
        #endif
    }
    #if !os(tvOS)
    @objc func pulledToRefresh() {
        presenter?.refresh(id)
        // An app that starts nothing leaves `refreshing` false: end promptly.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { [weak self, token = incarnation] in if self?.incarnation == token { self?.updateRefresh() } }
    }
    #endif
}
#endif

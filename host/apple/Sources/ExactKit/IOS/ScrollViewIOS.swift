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
        // The document takes the page's bands too, where its edge scrollers reach (LLP 1130 D3).
        let m = (superview as? Viewport)?.pageMargins ?? .zero
        guard !isHidden, isUserInteractionEnabled, bounds.inset(by: UIEdgeInsets(top: -m.top, left: -m.left, bottom: -m.bottom, right: -m.right)).contains(point) else { return nil }
        return NodeView.hitChildren(in: self, at: point, with: event)
    }
    package override var accessibilityElements: [Any]? {
        get { super.accessibilityElements ?? NodeView.emptyBoxAccessibility(in: self) }
        set { super.accessibilityElements = newValue }
    }
}

/// A scroll container — the viewport over the document, and a node whose
/// effective `overflow` scrolls. The platform pans it (LLP 1002 D4: scroll
/// always wins; UIKit does not chain a pan out of a nested scroll view at
/// its edge). Which axes it scrolls comes from the node's rows; a tap's
/// wheel (the agent's) applies the web's chaining rule itself
/// (`AgentIOS.swift`).
package class ScrollView: UIScrollView {
    var scrollingNode: NodeView? { (superview as? NodeView) ?? (self as? Viewport)?.documentNode }
    package override func layoutSubviews() {
        super.layoutSubviews()
        keepPlatformViewsOver()
    }
    package override var accessibilityElements: [Any]? {
        get { super.accessibilityElements ?? NodeView.emptyBoxAccessibility(in: self) }
        set { super.accessibilityElements = newValue }
    }
    package override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard let hit = super.hitTest(point, with: event) else { return nil }
        if let child = NodeView.hitChildren(in: self, at: point, with: event) { return child }
        // A paragraph's scroller with nothing to scroll is its node, as on the
        // web, where a click on it bubbles to the button around it: a
        // `UIScrollView` keeps a touch from the responder chain. CSS makes one
        // beside every `overflow-x: hidden` (`overflow-y` computes to `auto`),
        // so a truncating title is one. A scroller holding nodes (a page) keeps
        // its blank ground: its bounce, and the tap there that ends editing.
        if hit === self, !hasRange, !subviews.contains(where: { $0 is NodeView }), let owner = superview as? NodeView { return owner }
        return hit
    }

    /// Whether its content reaches past its port on an axis it scrolls, or a
    /// pull-to-refresh hangs above it.
    var hasRange: Bool {
        let inset = adjustedContentInset
        if scrollsX && contentSize.width + inset.left + inset.right > bounds.width + 0.5 { return true }
        if scrollsY && contentSize.height + inset.top + inset.bottom > bounds.height + 0.5 { return true }
        #if os(iOS)
        if refreshControl != nil { return true }
        #endif
        return false
    }

    package var scrollsX = true
    package var scrollsY = true
    #if os(tvOS)
    /// The offset the remote's last step scrolls to, and when it began
    /// (`RemoteTVOS.swift`): a step pressed during that animation goes on
    /// from there.
    var remoteStepTarget: (offset: CGPoint, at: CFTimeInterval)?
    #endif
    /// Let native body buttons (LLP 1069.011 D4) and tap-only invoker overlays
    /// yield to scrolling; UIKit normally keeps a UIControl's touch. Input-owning
    /// surfaces and other controls keep their existing rules.
    package override func touchesShouldCancel(in view: UIView) -> Bool {
        guard !CanvasInputs.owns(view) else { return false }
        if view is NativeButtonIOS { return true }
        if let button = view as? InvokerButton, !button.showsMenuAsPrimaryAction { return true }
        return super.touchesShouldCancel(in: view)
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
        if let owner = scrollingNode, !owner.allowsTouchPan(direction) { return false }
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
        guard velocity != .zero, let owner = scrollingNode else { return false }
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
        guard velocity != .zero, let owner = scrollingNode else { return false }
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
                if scrolls, let node = (outer as? ScrollView)?.scrollingNode ?? (outer.superview as? NodeView),
                   (node.style[horizontal ? "overscroll_behavior_x" : "overscroll_behavior_y"]?.string ?? "auto") != "auto" { return false }
            }
            up = current.superview
        }
        return false
    }
    /// A touch that no node took — nothing focusable, nothing pressable —
    /// ends the editing and sends the keyboard away. UIKit does nothing
    /// here by itself; hand-built iOS apps add exactly this, a tap on the
    /// background that calls `endEditing`, and that is the reason it stays
    /// (LLP 1115.000 D9.4; LLP 1008 §9 gave a page's blur as the reason).
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
        // A blur is the session's (LLP 1035.001 D5): wherever it is mounted,
        // never the window's — found through the nearest node above a nested
        // scroller; the viewport itself has none above it and is its own.
        if let t = touches.first, bounds.contains(t.location(in: self)) {
            var above = superview
            while let current = above {
                if let node = current as? NodeView, let presenter = node.presenter { presenter.endEditing(); return }
                above = current.superview
            }
            endEditing(true)
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
        // On a node's own scroll, whose padding is in its content. A grouped
        // list's collection view holds the padding as an inset: UIKit's place.
        guard superview is ScrollView, let owner = superview?.superview as? NodeView else { return }
        let top = owner.padding("top")
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
        if pageBand != .zero { setPageBand(.zero) } // the scroll view leaving gives its band back (LLP 1130 D3)
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

    func captureScrollPosition(focused: NodeView?) {
        // UIKit may have pending row geometry and its own offset adjustment.
        // Snapshot both from the same layout before preserving the reader.
        if groupedOwner { scrollView?.layoutIfNeeded() }
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
                } else { captureScrollAnchor(sv, focused: focused) }
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
                    // An element reveal heads for where its target now is instead (`IntoView.swift`).
                    if presenter?.elementScrolls.revealing(sv) != nil { presenter?.elementScrolls.retarget(sv) }
                    else if y != sv.contentOffset.y { sv.setContentOffset(PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: y), in: sv, clamped: !(sv.isTracking || sv.isDecelerating)), animated: false) }
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
                    if sv.contentOffset.y != top { sv.contentOffset = PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: top), in: sv, clamped: false) }
                }
                else { sv.contentOffset = PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: sv.contentOffset.y + top - prior.top), in: sv, clamped: false) }
            }
            anchoredScrollTop = sv.contentOffset.y
            return
        }
        if presenter?.elementScrolls.revealing(sv) != nil {
            // An element reveal heads for where its target now is instead (`IntoView.swift`).
            presenter?.elementScrolls.retarget(sv)
        } else if sv.contentOffset.y != y {
            // `scroll-behavior: smooth`: an end that moved down (a message
            // appended) is followed with UIKit's scroll animation, as a
            // smooth `scrollTop` write is; a shrink or a jump up lands at once.
            let animate = prior.end && y > sv.contentOffset.y && style["scroll_behavior"]?.string == "smooth" && !ExactEnv.agentFreezes && window != nil
            followingEndAnimated = animate
            sv.setContentOffset(PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: y), in: sv), animated: animate)
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
        // It replaces an element reveal, as on the Mac (LLP 1070.000 §14).
        if pendingScrollTop != nil || pendingScrollLeft != nil { presenter?.elementScrolls.cutReveal(sv) }
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

    /// Convert a logical write only after native layout. UIKit can
    /// invalidate its offset when that write collapses the navigation title;
    /// settle that one inset transition and apply the same intent once more.
    private func writeScrollPosition(in sv: UIScrollView, left: Double?, top: Double?, animated: Bool, titleSlack: Bool) {
        func target() -> CGPoint {
            let i = sv.adjustedContentInset, inset = scrollTopInset(sv)
            // CSS ends where a large title rests collapsed.
            let slack = titleSlack && scrollOrigin > 0 ? max(0, i.top - scrollCollapsed) : 0
            let y = top.map { CGFloat($0) - inset == sv.contentOffset.y ? sv.contentOffset.y : min(max(CGFloat($0) - inset, -i.top), max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height - slack)) } ?? sv.contentOffset.y
            let x = left.map { CGFloat($0) == sv.contentOffset.x ? sv.contentOffset.x : min(max(CGFloat($0), -i.left), max(-i.left, sv.contentSize.width + i.right - sv.bounds.width)) } ?? sv.contentOffset.x
            // On the pixel grid inside the range, so the content it scrolls
            // stays on it and UIKit does not clamp it off (`PixelGridIOS.swift`).
            let s = presenter?.gridScale ?? 1
            return CGPoint(x: PixelGrid.clamp(x, -i.left, max(-i.left, sv.contentSize.width + i.right - sv.bounds.width), scale: s),
                           y: PixelGrid.clamp(y, -i.top, max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height - slack), scale: s))
        }
        let initial = target()
        guard sv.contentOffset != initial else { return }
        let inset = sv.adjustedContentInset.top
        sv.setContentOffset(initial, animated: animated)
        if !animated, sv.adjustedContentInset.top != inset {
            sv.layoutIfNeeded()
            let settled = target()
            if sv.contentOffset != settled { sv.setContentOffset(settled, animated: false) }
        }
    }
    /// A `refresh` handler on a scroll container is UIKit's pull-to-refresh:
    /// the control fires the event; the app's `refreshing` going false ends it.
    /// The control is on the scroller that shows (LLP 1115.000 D11.3): a
    /// grouped list's collection view, which a finger pulls, never the
    /// hidden scroll beneath it. The list asks again when its collection
    /// view comes and goes.
    package func updateRefresh() {
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

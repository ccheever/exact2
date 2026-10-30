// `scrollend` on AppKit (`ScrollEnd.swift`): a live scroll (a trackpad's,
// its momentum included) ends with `didEndLiveScrollNotification`; a move
// outside one (a mouse wheel's notches, a jump the app wrote) ends once
// still.
#if os(macOS)
import AppKit

extension NodeView {
    /// Replacing an animation owes its end even if the new request or wheel
    /// moves nothing. A following animation or live gesture holds it again.
    func cancelElementScroll() {
        guard let end = scrollEnd, end.elementAnimation != nil, let clip = scroll?.contentView else { return }
        end.elementAnimation = nil
        let at = clip.bounds.origin
        NSAnimationContext.runAnimationGroup({ c in c.duration = 0; clip.animator().setBoundsOrigin(at) })
        end.settle { [weak self] in if self?.scrollEnd?.live == false { self?.endScroll() } }
    }

    /// The scroll view's live scrolls, start and end; `nil` drops the old
    /// one's, and what it owed, when the scroll view goes.
    func observeLiveScroll(_ sv: NSScrollView?, replacing old: NSScrollView? = nil) {
        if let old {
            NotificationCenter.default.removeObserver(self, name: NSScrollView.willStartLiveScrollNotification, object: old)
            NotificationCenter.default.removeObserver(self, name: NSScrollView.didEndLiveScrollNotification, object: old)
            scrollEnd?.reset()
        }
        guard let sv else { return }
        NotificationCenter.default.addObserver(self, selector: #selector(liveScrollStarted(_:)), name: NSScrollView.willStartLiveScrollNotification, object: sv)
        NotificationCenter.default.addObserver(self, selector: #selector(liveScrollEnded(_:)), name: NSScrollView.didEndLiveScrollNotification, object: sv)
    }
    @objc func liveScrollStarted(_ note: Notification) {
        presenter?.fragmentArmed = false // a fragment's scroll not yet made stays unmade (Fragment.swift)
        guard (handlers.contains("scrollend") || presenter?.session?.timerAdmissions.enabled == true), note.object as AnyObject? === scroll else { return }
        let end = scrollEnd ?? ScrollEnd()
        scrollEnd = end
        end.live = true
        end.hold()
    }
    @objc func liveScrollEnded(_ note: Notification) {
        guard note.object as AnyObject? === scroll else { return }
        scrollEnd?.live = false
        // After the other observers of this end: a collection's knob drag
        // puts back the origin it refused (`KnobDrag.ended`) first.
        DispatchQueue.main.async { [weak self] in self?.endScroll() }
    }

    /// The clip view's bounds changed (`clipScrolled`): a move of its origin
    /// owes an end; a resize does not.
    func scrollEndMoved() {
        // A collapse (`display: none`) clamps the origin and sends no `scroll`.
        guard (handlers.contains("scrollend") || presenter?.session?.timerAdmissions.enabled == true), hasScrollLayoutBox, let o = scroll?.contentView.bounds.origin else { return }
        let end = scrollEnd ?? ScrollEnd()
        scrollEnd = end
        guard end.moved(to: [o.x, o.y, 0]) else { return }
        if end.live || end.elementAnimation != nil { end.hold() } else { end.settle { [weak self] in if self?.scrollEnd?.live == false { self?.endScroll() } } }
    }

    /// The scroll is over: its last `scroll` first, then `scrollend`, once.
    func endScroll() {
        guard (handlers.contains("scrollend") || presenter?.session?.timerAdmissions.enabled == true), presenter?.views[id] === self, let end = scrollEnd, end.elementAnimation == nil, end.take(), hasScrollLayoutBox else { return }
        // The last `scroll` may run its handler at once; this view may be gone after.
        sendScrollEvent()
        guard presenter?.views[id] === self else { return }
        if handlers.contains("scrollend") { presenter?.scrollend(id) }
        presenter?.session?.timerAdmissions.check()
    }
}
#endif

// What the last frame showed, which alone moves in an animation UIKit has
// open around a batch. UIKit animates a layer from its current value to the
// one set inside the animation; for a view no frame has shown, that value is
// a place nobody saw, and the view flies in from it. UIKit's own containers
// lay a new view out before they animate (a child controller's view before
// its transition), and CSS starts no transition for an element that was not
// rendered at the previous style change (CSS Transitions 1 §3): a view made,
// put in a window or shown again since the last frame takes its place at
// once, and the views that showed move with UIKit's animation.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// `view` is not shown by the last frame: made, put in a window, taken
    /// out of one, or shown again (`isHidden`). Core Animation's next commit
    /// shows it if it is in a window then, not hidden; one out of a window
    /// waits for the commit after it moves into one (`didMoveToWindow`).
    func unshown(_ view: NodeView) {
        view.showed = false
        guard view.window != nil else { return }
        if awaitingFrame.isEmpty {
            // After Core Animation's commit (order 2000000), as `NativeViews.activateAfterCommit`
            // waits for it; exit covers a turn that never waits.
            let observer = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue | CFRunLoopActivity.exit.rawValue, false, 2_000_001) { [weak self] _, _ in
                self?.committed()
            }
            CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes)
        }
        awaitingFrame.append(WeakNodeView(view: view))
    }

    /// The turn's views that Core Animation committed in a window, shown:
    /// what its presentation copy has is what was last set, the view shown
    /// where it is. One whose place is not committed yet (a commit an open
    /// explicit transaction holds back: no copy, or an old one from an
    /// earlier time in a window, hidden or elsewhere) waits for a later turn
    /// while its window is one a scene shows, the only windows on screen.
    private func committed() {
        let waiting = awaitingFrame
        awaitingFrame = []
        for entry in waiting {
            guard let view = entry.view, let window = view.window, !view.isHidden else { continue }
            if let shown = view.layer.presentation(), !shown.isHidden, shown.position == view.layer.position, shown.bounds == view.layer.bounds { view.showed = true }
            else if window.windowScene != nil, !window.isHidden { unshown(view) }
        }
    }

    /// `view` is shown again (`isHidden` lifted): unshown if the last commit
    /// had it hidden, or never had it; a hide lifted before any commit (a
    /// capture's) leaves what showed as it was.
    func shownAgain(_ view: NodeView) {
        if view.layer.presentation()?.isHidden != false { unshown(view) }
    }

    /// Whether the last frame showed `view` where it is: it and every node
    /// above it, none hidden now.
    func showed(_ view: NodeView) -> Bool {
        var at: UIView? = view
        while let v = at {
            if let node = v as? NodeView, !node.showed || node.isHidden { return false }
            at = v.superview
        }
        return true
    }

    /// Makes `change` to `view` (its op, its frame, its parts' layout) inside
    /// the animation UIKit has open around it only if the last frame showed
    /// the view; otherwise with UIKit's animations off, so that it appears
    /// where it is laid out. With no animation open, as it comes.
    package func moveOrPlace(_ view: @autoclosure () -> NodeView?, _ change: () -> Void) {
        guard UIView.inheritedAnimationDuration > 0, !(view().map(showed) ?? false) else { return change() }
        UIView.performWithoutAnimation(change)
    }
}

extension NodeView {
    /// Makes `change` to this node or to a view the host keeps for it (its
    /// scroller's band, a native control or a cell over it) by
    /// `Presenter.moveOrPlace`: a view `new` under it, made or put there
    /// since the last frame, takes its place at once.
    func moveOrPlace(new: Bool = false, _ change: () -> Void) {
        guard let presenter else { return change() }
        presenter.moveOrPlace(new ? nil : self, change)
    }
}
#endif

// @ref LLP 1008 §9.1 — the keyboard as a content inset (overlays-content).
// Under `interactive-widget="overlays-content"` the keyboard covers the
// viewport rather than resizing it, as CSS has it: nothing is laid out again
// as the keyboard moves. A `role="toolbar" toolbarPlacement="keyboard"`
// rides the keyboard's top instead, moved by a transform, and the scrollers
// that end where it starts keep their content clear of it with a content
// inset and a scroll-indicator inset; one that was at its end stays there.
// Both are set inside the keyboard's own animation (Presenter.applyKeyboard),
// so they move with its duration and curve, finger-driven dismissal
// included. This is how UIKit apps (Signal's input toolbar, pinned to the
// keyboard layout guide over a collection view's bottom inset) do it.
#if os(iOS) || os(tvOS)
import UIKit

final class KeyboardToolbars {
    unowned let presenter: Presenter
    /// Insets belong to the physical backend, which a logical node can replace.
    private struct Inset {
        weak var scroll: UIScrollView?
        let amount: CGFloat
    }
    private var insets: [UInt32: Inset] = [:]
    private var lifted = Set<UInt32>()
    private var rideQueued = false
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Transfer an existing keyboard inset after replacement geometry settles.
    func backendChanged() {
        guard presenter.interactiveWidget == "overlays-content",
              presenter.keyboardInset > 0 || !insets.isEmpty, !rideQueued else { return }
        rideQueued = true
        presenter.afterBatch { [weak self] in
            guard let self else { return }
            self.finishBackendChange()
        }
    }

    /// Final geometry is available; apply owned insets before pending offsets.
    func finishBackendChange() {
        guard rideQueued else { return }
        rideQueued = false
        ride(overlap: presenter.keyboardInset)
    }

    /// The keyboard now covers `overlap` points of the viewport's bottom.
    func ride(overlap: CGFloat) {
        let toolbars = presenter.carrying("toolbarPlacement").filter {
            $0.props["toolbarPlacement"] == "keyboard" && !$0.isHidden && $0.window != nil
        }
        var still = Set<UInt32>(), owned = Set<UInt32>()
        for bar in toolbars {
            guard let window = bar.window else { continue }
            var box = bar.convert(bar.bounds, to: window)
            // Work from the authored position, not the lift already applied
            // on a previous keyboard frame. Convert the outer translation
            // through the same parent/context transforms as applyTransform.
            if bar.keyboardLift != 0, let parent = bar.superview {
                let origin = parent.convert(CGPoint.zero, to: window)
                let shifted = parent.convert(CGPoint(x: bar.contextTransform.c * bar.keyboardLift,
                                                      y: bar.contextTransform.d * bar.keyboardLift), to: window)
                box = box.offsetBy(dx: shifted.x - origin.x, dy: shifted.y - origin.y)
            }
            // The bar's own bottom padding already clears the home indicator;
            // it rises only by what the keyboard covers above that.
            let viewportBottom = presenter.viewport.convert(presenter.viewport.bounds, to: window).maxY
            let lift = max(0, overlap - max(0, viewportBottom - box.maxY) - bar.number("padding_bottom"))
            if bar.keyboardLift != lift { bar.keyboardLift = lift; bar.applyTransform() }
            if lift > 0 { still.insert(bar.id) }
            // The scrollers whose bottom is the bar's top, across its width.
            for node in presenter.views.values {
                guard let sv = node.scrollView, node.window === window, !node.isHidden else { continue }
                let frame = node.convert(node.bounds, to: window)
                guard abs(frame.maxY - box.minY) <= 1, frame.maxX > box.minX, frame.minX < box.maxX else { continue }
                owned.insert(node.id)
                apply(lift, to: sv, node: node)
            }
        }
        for id in lifted.subtracting(still) {
            guard let bar = presenter.views[id], bar.keyboardLift != 0 else { continue }
            bar.keyboardLift = 0; bar.applyTransform()
        }
        lifted = still
        for (id, inset) in insets where !owned.contains(id) {
            if let sv = inset.scroll { adjust(-inset.amount, on: sv) }
            insets.removeValue(forKey: id)
        }
        insets = insets.filter { $0.value.amount > 0 }
    }

    private func apply(_ lift: CGFloat, to sv: UIScrollView, node: NodeView) {
        let prior = insets[node.id]
        let previous = prior?.scroll === sv ? prior?.amount ?? 0 : 0
        if let prior, prior.scroll !== sv {
            if let old = prior.scroll { adjust(-prior.amount, on: old) }
            insets.removeValue(forKey: node.id)
        }
        guard previous != lift || sv.contentInset.bottom < lift else { return }
        adjust(lift - previous, on: sv)
        insets[node.id] = Inset(scroll: sv, amount: lift)
    }

    private func adjust(_ delta: CGFloat, on sv: UIScrollView) {
        let maximum = { max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height) }
        let atEnd = sv.contentOffset.y >= maximum() - 1
        sv.contentInset.bottom += delta
        sv.verticalScrollIndicatorInsets.bottom += delta
        if atEnd { sv.contentOffset.y = maximum() }
    }
}
#endif

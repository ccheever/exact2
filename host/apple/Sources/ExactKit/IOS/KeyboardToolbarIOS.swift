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
    /// Each scroller's inset this owns, by view id, to take back exactly.
    private var insets: [UInt32: CGFloat] = [:]
    private var lifted = Set<UInt32>()
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// The keyboard now covers `overlap` points of the viewport's bottom.
    func ride(overlap: CGFloat) {
        let toolbars = presenter.carrying("toolbarPlacement").filter {
            $0.props["toolbarPlacement"] == "keyboard" && !$0.isHidden && $0.window != nil
        }
        var still = Set<UInt32>(), owned: [UInt32: CGFloat] = [:]
        for bar in toolbars {
            guard let window = bar.window else { continue }
            let box = bar.convert(bar.bounds, to: window)
            // The bar's own bottom padding already clears the home indicator;
            // it rises only by what the keyboard covers above that.
            let viewportBottom = presenter.viewport.convert(presenter.viewport.bounds, to: window).maxY
            let lift = max(0, overlap - max(0, viewportBottom - box.maxY) - bar.number("padding_bottom"))
            if bar.keyboardLift != lift { bar.keyboardLift = lift; bar.applyTransform() }
            if lift > 0 { still.insert(bar.id) }
            // The scrollers whose bottom is the bar's top, across its width.
            for node in presenter.views.values {
                guard let sv = node.scroll, node.window === window, !node.isHidden else { continue }
                let frame = node.convert(node.bounds, to: window)
                guard abs(frame.maxY - box.minY) <= 1, frame.maxX > box.minX, frame.minX < box.maxX else { continue }
                owned[node.id] = lift
                apply(lift, to: sv, node: node)
            }
        }
        for id in lifted.subtracting(still) {
            guard let bar = presenter.views[id], bar.keyboardLift != 0 else { continue }
            bar.keyboardLift = 0; bar.applyTransform()
        }
        lifted = still
        for (id, _) in insets where owned[id] == nil {
            if let node = presenter.views[id], let sv = node.scroll { apply(0, to: sv, node: node) }
        }
        insets = owned.filter { $0.value > 0 }
    }

    private func apply(_ lift: CGFloat, to sv: UIScrollView, node: NodeView) {
        let previous = insets[node.id] ?? 0
        guard previous != lift || sv.contentInset.bottom < lift else { return }
        let maximum = { max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height) }
        let atEnd = sv.contentOffset.y >= maximum() - 1
        sv.contentInset.bottom += lift - previous
        sv.verticalScrollIndicatorInsets.bottom += lift - previous
        insets[node.id] = lift
        if atEnd { sv.contentOffset.y = maximum() }
    }
}
#endif

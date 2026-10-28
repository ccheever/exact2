// @ref LLP 1123 D1 — while a native container owns the screen, the
// session's document is mounted where no scroll view holds it. A scroll view
// above a container changes where UIKit places its bars: inside one (the
// session's viewport), a bar's search field stays in its column behind the
// keys, where a hand-built app's rises above them and spans a split (iOS
// 27.1, measured with the split moved into an empty `UIScrollView` and into
// a plain view). The viewport stays the document's coordinate space; the
// mount takes its place, frame and bounds alike, so every conversion
// through the viewport holds.
#if os(iOS) || os(tvOS)
import UIKit

/// The document's mount while a native container owns the screen: in the
/// viewport's place, transparent to a touch on nothing, and answering
/// UIKit's focus search as the viewport does. The container takes the whole
/// view, so the page has no bands past its area (`Presenter.pageMargins`)
/// while it is here.
package final class DocumentMount: UIView, RestsTint {
    /// Its tint undimmed, for the nodes mounted here, as the viewport keeps its own (`RestsTint`)
    var restingTint: UIColor? { tintAdjustmentMode == .dimmed ? undimmedTint : tintColor }
    private var undimmedTint: UIColor?
    package override func tintColorDidChange() {
        super.tintColorDidChange()
        if tintAdjustmentMode != .dimmed { undimmedTint = tintColor }
    }
    package override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil, tintAdjustmentMode != .dimmed { undimmedTint = tintColor }
    }
    package override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let hit = super.hitTest(point, with: event)
        return hit === self ? nil : hit
    }
    #if !os(tvOS)
    package override func focusItems(in rect: CGRect) -> [any UIFocusItem] { FocusSearch.items(self, in: rect) }
    #endif
}

extension Presenter {
    /// Whether the document is in its mount, under a native container.
    var documentFixed: Bool { root.superview === documentMount }

    /// The view the document is mounted in, which what is drawn over it joins.
    package var documentHost: UIView { documentFixed ? documentMount : viewport }

    /// Whether `view` is in the session's document, wherever its root is mounted.
    package func inDocument(_ view: UIView) -> Bool { view.isDescendant(of: viewport) || view.isDescendant(of: documentMount) }

    /// The document in the viewport, which scrolls it as a page, or (`fixed`)
    /// in its mount, where a native container's bars are UIKit's to place.
    func hostDocument(fixed: Bool) {
        let host: UIView = fixed ? documentMount : viewport
        guard root.superview !== host else { return }
        host.addSubview(root)
        viewport.isScrollEnabled = !fixed
        placeDocumentMount()
        // A fixed document takes no keyboard range; a scrolling one does again.
        setKeyboardInset(keyboardInset)
        // What is drawn over the document, and what hears its touches, moves with it.
        menus.rehostPopovers()
        menus.observeTouches()
    }

    /// The mount in the viewport's place: its transform (the agent's own
    /// size), its frame and its bounds, offset included.
    func placeDocumentMount() {
        let mount = documentMount
        if mount.transform != viewport.transform { mount.transform = viewport.transform }
        if mount.bounds != viewport.bounds { mount.bounds = viewport.bounds }
        if mount.center != viewport.center { mount.center = viewport.center }
    }
}
#endif

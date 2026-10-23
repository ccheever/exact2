#if os(iOS) || os(tvOS)
import UIKit

// While UIKit's navigation takes the whole view and the page has no range,
// the document is mounted beside the viewport, in `documentMount`, not in
// it: on the iPhone Duo, UIKit draws a navigation controller's vertical bar
// under the status capsule, and its items take no touch, while a scroll view
// is above the controller's view. The mount has the viewport's geometry, so
// a box converted to the viewport is the same place in either; what hosts
// views over the document or observes its touches uses `documentPort`.
extension Presenter {
    /// Where the document is mounted: the viewport, or the mount beside it.
    var documentPort: UIView { root.superview === documentMount ? documentMount : viewport }

    /// Mounts the document where it belongs now. Called wherever the
    /// viewport's geometry or range, or the navigation's hold on the view,
    /// can change. A view that embeds the presenter without the mount keeps
    /// the document in the viewport.
    func mountDocument() {
        guard let home = viewport.superview, documentMount.superview === home else { return }
        let still = navigation.wantsWholeView && !viewport.hasRange
        // With no range the rest offset is the only one: a page that shrank
        // while scrolled stands still there.
        if still, root.superview !== documentMount {
            let inset = viewport.adjustedContentInset
            viewport.setContentOffset(CGPoint(x: -inset.left, y: -inset.top), animated: false)
        }
        documentMount.transform = viewport.transform
        documentMount.bounds = viewport.bounds
        documentMount.center = viewport.center
        documentMount.isHidden = !still
        viewport.isScrollEnabled = !still
        let mount: UIView = still ? documentMount : viewport
        guard root.superview !== mount else { return }
        // Below the popovers' layers, which then follow it (`PopoverLayer.mount`).
        if let layer = mount.subviews.first(where: { $0 is PopoverLayer }) { mount.insertSubview(root, belowSubview: layer) } else { mount.addSubview(root) }
        menus.rehostPopovers()
        if modals.coordinateView == nil { menus.observeTouches(on: mount) }
    }
}
#endif

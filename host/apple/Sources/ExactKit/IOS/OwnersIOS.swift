// @ref LLP 1123 D2 — what of a window is the session's: its viewport, and
// each presentation's controller view, where a presented route is mounted.
// Whether a view is mounted there, where a finger reaches it, and ending
// the session's editing ask these, never whether the viewport holds a view.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// Whether `view` is mounted in this session: in its viewport, or in a
    /// presentation beside it.
    func mounts(_ view: UIView) -> Bool {
        view.isDescendant(of: viewport) || modals.mounts(view)
    }

    /// Where a finger reaches `view`, in viewport points (the viewport's
    /// coordinates less its scrolling, as the agent's boxes are): the
    /// viewport for the document; for a presentation's content, its
    /// presentation's view within the window.
    func reach(of view: UIView) -> CGRect {
        let port = CGRect(origin: .zero, size: viewport.bounds.size)
        guard !view.isDescendant(of: viewport), let window = viewport.window,
              let mount = modals.mountRoots.last(where: { view.isDescendant(of: $0) }) else { return port }
        let shown = mount.convert(mount.bounds, to: nil).intersection(window.bounds)
        let rect = viewport.convert(shown, from: nil)
        return rect.offsetBy(dx: -viewport.contentOffset.x, dy: -viewport.contentOffset.y)
    }

    /// Ends editing wherever the session is mounted; whether any editor resigned.
    @discardableResult
    package func endEditing() -> Bool {
        modals.mountRoots.reduce(viewport.endEditing(true)) { $1.endEditing(true) || $0 }
    }
}
#endif

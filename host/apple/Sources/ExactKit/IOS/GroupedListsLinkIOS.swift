// @ref LLP 1047.001 D4 — a grouped list (LLP 1084) is a linked capability:
// its implementation is the `ExactGroupedLists` module, which an app's
// composition links only when its plan uses one, and installs here. The core
// names no grouped-list type; without the module, a plan with a grouped list
// is refused at boot (`Unlinked("grouped_lists")`), so nothing below is asked.
#if os(iOS) || os(tvOS)
import UIKit

/// What the core asks of the grouped lists a presenter shows.
package protocol GroupedLists: AnyObject {
    /// Before a batch: every carried row back where the presenter put it.
    func prepare()
    /// After a batch; `changed` is the touched views and their ancestors.
    func sync(changed: Set<UInt32>?)
    func reset()
    /// The collection view a wheel on `id` scrolls.
    func scroller(for id: UInt32) -> UIScrollView?
    /// Row geometry from the native layout, never the hidden authored boxes.
    func projectedRect(for node: NodeView, in scroll: UIScrollView) -> CGRect?
    /// Visible rows in native reading order, for preserving the reading position.
    func scrollAnchors(for id: UInt32) -> [(node: NodeView, y: CGFloat)]
    /// Whether a list draws `id` (a row, or a row's toggle or detail button).
    func draws(_ id: UInt32) -> Bool
    /// Where a real finger aimed at `node` lands, for a row a list draws.
    func shown(_ node: NodeView) -> GroupedAim?
    /// The agent's `tap` on a row UIKit draws.
    func activate(_ node: NodeView) -> [String: Any]?
    /// `layout <id>`'s native fields for a list or a row it draws.
    func observation(_ node: NodeView) -> [String: Any]?
    // LLP 1080.001 D3: what the inspection walk accounts for.
    func inspectionOwns(_ view: UIView) -> Bool
    func hides(_ node: NodeView) -> Bool
    func projects(_ view: UIView) -> Bool
}

/// Where a finger aimed at a row a list draws lands (LLP 1080.000 D4): a
/// view inside a collection view's port, or why it can't.
package enum GroupedAim { case view(UIView, port: UIScrollView), refused(String) }

/// A grouped list's collection view, which scrolls in its list's place
/// (LLP 1084 D8).
package protocol GroupedScroller: UIScrollView {}

/// The grouped lists module's hooks, installed by the composition.
package enum GroupedListsLink {
    /// A presenter's grouped lists.
    package static var make: ((Presenter) -> GroupedLists)?
    /// The row and part of a grouped list's cell that a view is in.
    package static var part: ((UIView?) -> [String: Any]?)?
}
#endif

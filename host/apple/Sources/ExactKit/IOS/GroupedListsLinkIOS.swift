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
    /// Before a batch of `ops`: the carried rows it must read at home go
    /// back, every row of a list where it changes what they were carried
    /// from in a way their cells cannot follow.
    func prepare(_ ops: [BatchOp])
    /// A `children` op places `want` in `container`: those the presenter
    /// places, without the rows a list carries from there, which stay in
    /// their cells.
    func placing(_ container: UIView, _ want: [NodeView]) -> [NodeView]
    /// The rows carried from `container`, which are placed from it on the pixel grid.
    func carried(from container: UIView) -> [NodeView]
    /// Where a row a list carries is drawn in its cell when layout places
    /// it at `place` in its parent; nil for a view no list carries.
    func cellFrame(of view: NodeView, at place: GridPlace) -> CGRect?
    /// After a batch; `changed` is the touched views and their ancestors.
    func sync(changed: Set<UInt32>?)
    /// Whether some list holds a custom row's views in its cells.
    var carriesRows: Bool { get }
    func reset()
    /// The collection view a wheel on `id` scrolls.
    func scroller(for id: UInt32) -> UIScrollView?
    /// The fresh model's rows by section, in document order.
    func rows(for id: UInt32) -> [[UInt32]]?
    /// Row geometry from the native layout, never the hidden authored boxes.
    func projectedRect(for node: NodeView, in scroll: UIScrollView) -> CGRect?
    /// The rows between what `scroll` shows and the one holding `node` sized
    /// by their cells, so `projectedRect` is where the row will stay.
    func measure(toward node: NodeView, in scroll: UIScrollView)
    /// Visible rows in native reading order, for preserving the reading position.
    func scrollAnchors(for id: UInt32) -> [(node: NodeView, y: CGFloat)]
    /// Whether a list draws `id` (a row, or a row's toggle or detail button).
    func draws(_ id: UInt32) -> Bool
    /// A route's screen is about to show again (a pop back to it, with the
    /// transition it rides, if any): the rows its lists kept selected under
    /// the screen they pushed let go (LLP 1115.000 D11.3).
    func appearing(_ route: NodeView, alongside coordinator: UIViewControllerTransitionCoordinator?)
    /// The agent's reveal of what a list draws or carries: its row's cell
    /// scrolled into `port` (the collection view's visible part, in its
    /// bounds), with or without a cell now. Nil for a node no list draws or
    /// carries; else whether the list scrolled.
    func reveal(_ id: UInt32, port: (UIScrollView) -> CGRect) -> Bool?
    /// Where a real finger aimed at `node` lands, for a row a list draws.
    func shown(_ node: NodeView) -> GroupedAim?
    /// The agent's `tap` on a row UIKit draws.
    func activate(_ node: NodeView, before: (CGPoint?) -> Bool) -> [String: Any]?
    /// `layout <id>`'s native fields for a list or a row it draws.
    func observation(_ node: NodeView) -> [String: Any]?
    // LLP 1080.001 D3: what the inspection walk accounts for.
    func inspectionOwns(_ view: UIView) -> Bool
    func hides(_ node: NodeView) -> Bool
    func hidSection(_ node: NodeView) -> Bool
    func projects(_ view: UIView) -> Bool
    /// The authored parent of a row carried into a cell.
    func logicalParent(of view: UIView) -> UIView?
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

#if os(iOS)
package protocol GroupedMenuSurface: AnyObject {
    var menuSource: NodeView? { get }
}

extension Presenter {
    package func syncGroupedMenu(on view: UIView & GroupedMenuSurface) {
        menus.context.syncProjected(view)
    }
}
#endif

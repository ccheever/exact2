// Where a real finger aims when UIKit draws a node in its place (LLP 1080.000
// D4, LLP 1116.003): a header button's item in the navigation bar under
// `--chrome platform`. The authored node stays hidden in the content, so
// its own box is not where a person would touch.
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    /// Under `--chrome platform`, the navigation bar a header button's item
    /// sits in and the item's box in it (r5 weather's Cities, r5 mood
    /// journal's sheet Save). Nil for any other node; a refusal for an item
    /// UIKit has not placed.
    func barItem(_ node: NodeView) -> Result<(bar: UIView, item: CGRect), BarItemMissing>? {
        #if os(iOS)
        guard !ExactEnv.authoredChrome else { return nil }
        let nav = presenter.navigation
        let testId = node.props["testId"]
        for c in nav.controllers.values {
            guard let controller = c.navigationController, controller.topViewController === c, nav.barShows(controller),
                  c.viewIfLoaded?.window != nil else { continue }
            let items = (c.navigationItem.leftBarButtonItems ?? []) + (c.navigationItem.rightBarButtonItems ?? [])
            // A pull-down's item has no target: its test id names it.
            guard let item = items.first(where: { ($0.target as? BarPress)?.id == node.id })
                    ?? items.first(where: { $0.target == nil && testId != nil && $0.accessibilityIdentifier == testId }) else { continue }
            let bar = controller.navigationBar
            guard let frame = item.frame(in: bar), frame.width > 0, frame.height > 0 else { return .failure(BarItemMissing()) }
            return .success((bar, frame))
        }
        #endif
        return nil
    }

    /// Why a finger would not reach the segment or bar item UIKit draws in
    /// `node`'s place (a sheet over it, the keyboard), or nil: the tap that
    /// activates either is refused as a covered node's is.
    func drawnCovered(_ node: NodeView) -> String? {
        let target: UIView, region: CGRect
        if let (control, segment) = presenter.segments.drawn(node) { (target, region) = (control, segment) }
        else if case .success(let (bar, frame))? = barItem(node) { (target, region) = (bar, frame) }
        else { return nil }
        guard let win = target.window else { return nil }
        let p = target.convert(CGPoint(x: region.midX, y: region.midY), to: nil)
        guard let hit = win.hitTest(p, with: nil) else { return "its middle is off the window" }
        return obscured(target, at: p, hit: hit)
    }

    /// Whether `view` is in what `session` shows: its viewport, its view, or
    /// a navigation controller or tab bar controller of its own, a presented
    /// sheet's included, whose bars are outside both.
    static func shows(_ session: ExactSession, _ view: UIView) -> Bool {
        if view.isDescendant(of: session.presenter.viewport) || session.view.map({ view.isDescendant(of: $0) }) == true { return true }
        // A dialog's sheet holds the session's own nodes, outside its view (LLP 1115.000 D11.5).
        if session.presenter.menus.sheets.shows(view) { return true }
        let nav = session.presenter.navigation
        if let tabs = nav.tabController, view.isDescendant(of: tabs.view) { return true }
        return nav.allNavigations.contains { $0.viewIfLoaded.map { view.isDescendant(of: $0) } == true }
    }
}

/// A header button's bar item that UIKit has not placed in its bar.
struct BarItemMissing: Error {}
#endif

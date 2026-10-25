// @ref LLP 1143 D1 — why a target the agent names has no view on screen, from
// what `state.navigation` already observes (its transition phase, the route,
// the presentations) and whether the runner still has the node.
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    /// Tell the driver, unasked, when the app leaves the foreground and when it
    /// is back. A suspended app answers nothing, so the driver fails the op it
    /// waits on at once instead of waiting out its reply bound. The line goes
    /// out inside a background task, before the system suspends the process.
    static func watchForeground() {
        let notifications = NotificationCenter.default
        _ = notifications.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { _ in
            let task = UIApplication.shared.beginBackgroundTask(withName: "exact agent")
            raw("{\"pushed\":\"background\"}")
            UIApplication.shared.endBackgroundTask(task)
        }
        _ = notifications.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { _ in
            raw("{\"pushed\":\"foreground\"}")
        }
    }

    /// The refusal for a target with no view in a window, saying which of four
    /// causes it is: a transition still in progress, a route or sheet over it,
    /// a node the runner holds with no view (a list row outside its realized
    /// range), or a node that is gone.
    func miss(_ req: [String: Any]) -> [String: Any] {
        let id = req["id"] as? Int
        let head = "no view \(id.map(String.init) ?? "?") on screen"
        guard let id else { return ["error": head] }
        let navigation = presenter.navigation.observation()
        let route = navigation["route"] as? String
        let sheets = presenter.modals.routes.map { ($0.node.props["navigationKey"] ?? "", $0.kind) }
        let top = sheets.last.map { "the \($0.1) \($0.0)" } ?? route.map { "the route \($0)" }
        if (navigation["transition"] as? [String: Any])?["phase"] as? String == "in-progress" {
            return ["error": "\(head): a push, pop or presentation is still in progress\(top.map { " (on top: \($0))" } ?? "")", "miss": "transition"]
        }
        if let view = presenter.views[UInt32(id)] {
            // A view out of every window is under what is on top, or in a
            // route or sheet that is not shown.
            let owner = presenter.modals.routes.dropLast().first { view.isDescendant(of: $0.node) }?.node.props["navigationKey"]
            return ["error": "\(head): it is not in the shown screen\(owner.map { " (its sheet \($0) is not on top)" } ?? "")\(top.map { "; on top: \($0)" } ?? "")", "miss": "covered"]
        }
        let row = session.agent("{\"op\":\"tree\",\"target\":\(id),\"shallow\":true}")
        if row.hasPrefix("{\"error\":") { return ["error": "\(head): the app no longer has node \(id)", "miss": "gone"] }
        return ["error": "\(head): node \(id) has no view (a list row outside the rows the list realized): `tap <list> into <key>` brings it in", "miss": "unmounted"]
    }
}
#endif

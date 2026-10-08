// @ref LLP 1069.001 (amended 2026-10-07) — an indeterminate `progress` is
// UIKit's activity indicator, centred in the node's box: `.medium`, or
// `.large` where the box's shorter side holds its 37 points; `color` its
// colour. It turns while it shows, and stops where it is hidden
// (`display: none` on it or above it), removed, or under the agent's clock,
// which shows one still frame. It is its own accessibility element (UIKit's
// "In progress"), named by the node's `aria-label`.
#if os(iOS) || os(tvOS)
import UIKit

extension ControlHost {
    /// The shorter side at which the large indicator replaces the medium
    /// one: the large one's own size.
    static let largeSide: CGFloat = 37

    func syncProgress() {
        let owners = presenter.carrying(ControlKinds.progress).filter { $0.kind == "control" }
        let live = Set(owners.map(\.id))
        let leaving = Set(presenter.leaving.values.flatMap { $0.members.map(\.id) })
        for id in Array(spinners.keys) where !live.contains(id) && !leaving.contains(id) {
            spinners.removeValue(forKey: id)?.removeFromSuperview()
        }
        let held = ExactEnv.agentFreezes || presenter.session?.clock != nil
        for owner in owners {
            let spinner = spinners[owner.id] ?? {
                let made = UIActivityIndicatorView(style: .medium)
                // Stopped, it still shows its frame: the agent's still one.
                made.hidesWhenStopped = false
                made.isUserInteractionEnabled = false
                spinners[owner.id] = made
                return made
            }()
            let mount = owner.controlMount
            if spinner.superview !== mount { mount.addSubview(spinner) }
            let box = owner.contentBox()
            assign(spinner, \.style, min(box.width, box.height) >= Self.largeSide ? .large : .medium)
            if let ink = owner.channels("text_color").map({ TextEngine.color($0) }) { assign(spinner, \.color, ink) }
            assign(spinner, \.isHidden, owner.cssVisibilityHidden)
            assign(spinner, \.accessibilityIdentifier, owner.props["testId"])
            // None restores UIKit's own ("In progress").
            assign(spinner, \.accessibilityLabel, owner.props["accessibilityLabel"])
            let natural = spinner.intrinsicContentSize
            assign(spinner, \.frame, CGRect(x: box.midX - natural.width / 2, y: box.midY - natural.height / 2,
                                            width: natural.width, height: natural.height))
            let turns = !held && !owner.cssVisibilityHidden && Self.shown(owner)
            if turns != spinner.isAnimating { if turns { spinner.startAnimating() } else { spinner.stopAnimating() } }
        }
        // One leaving with its exit holds too, under the agent's clock.
        if held { for id in leaving { if let s = spinners[id], s.isAnimating { s.stopAnimating() } } }
    }

    /// Whether nothing from the node up hides it: `display: none` hides a
    /// node's view, and a hidden ancestor hides it too.
    static func shown(_ view: UIView) -> Bool {
        var at: UIView? = view
        while let v = at {
            if v.isHidden { return false }
            at = v.superview
        }
        return true
    }

    /// The agent's `native.control` for a progress.
    func progressObservation(_ node: NodeView) -> [String: Any]? {
        guard let s = spinners[node.id] else { return nil }
        return ["view": "UIActivityIndicatorView", "style": s.style == .large ? "large" : "medium", "animating": s.isAnimating,
                "size": [Agent.r2(s.bounds.width), Agent.r2(s.bounds.height)]]
    }
}
#endif

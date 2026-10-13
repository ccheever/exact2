// @ref LLP 1069.001 (amended 2026-10-07) — an indeterminate `progress` is
// UIKit's activity indicator, centred in the node's box: `.medium`, or
// `.large` where the box's shorter side holds its 37 points; a `color` set
// on it its colour (an inherited one leaves UIKit's, LLP 1115 D4). It turns
// while it shows, and stops where it is hidden (`display: none` on it or
// above it), removed, or under the agent's clock, which shows one still frame. It is its own accessibility element (UIKit's
// "In progress"), named by the node's `aria-label`.
// @ref LLP 1116 D8 — with a `value`, it is UIKit's progress view instead
// (`.default`), across the content box at its own height, tinted by the
// node's `accent-color` (none leaves UIKit's tint), at HTML's fraction.
#if os(iOS) || os(tvOS)
import UIKit

extension ControlHost {
    /// The shorter side at which the large indicator replaces the medium
    /// one: the large one's own size.
    static let largeSide: CGFloat = 37
    /// UIKit's own indicator colour, which an unsaid `color` leaves.
    static let platformInk: UIColor = UIActivityIndicatorView(style: .medium).color

    func syncProgress() {
        let all = presenter.carrying(ControlKinds.progress).filter { $0.kind == "control" }
        let leaving = Set(presenter.leaving.values.flatMap { $0.members.map(\.id) })
        let held = ExactEnv.agentFreezes || presenter.session?.clock != nil
        syncBars(all.filter { $0.props["value"] != nil }, leaving: leaving, held: held)
        let owners = all.filter { $0.props["value"] == nil }
        let live = Set(owners.map(\.id))
        for id in Array(spinners.keys) where !live.contains(id) && !leaving.contains(id) {
            spinners.removeValue(forKey: id)?.removeFromSuperview()
        }
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
            // UIKit's grey unless the spinner's own `color` says otherwise
            // (LLP 1115 D4: an inherited colour is not the spinner's).
            let ink = owner.ownColor("text_color") == nil ? nil : owner.channels("text_color").map { TextEngine.color($0) }
            assign(spinner, \.color, ink ?? Self.platformInk)
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

    /// The determinate bars (LLP 1116 D8). A change of value eases, as
    /// UIKit's does on a hand-built screen, except under the agent's clock,
    /// which shows where it lands; VoiceOver hears the percentage (or the
    /// node's `aria-valuetext`), as from UIKit's own.
    private func syncBars(_ owners: [NodeView], leaving: Set<UInt32>, held: Bool) {
        let live = Set(owners.map(\.id))
        for id in Array(bars.keys) where !live.contains(id) && !leaving.contains(id) {
            bars.removeValue(forKey: id)?.removeFromSuperview()
        }
        for owner in owners {
            let fresh = bars[owner.id] == nil
            let bar = bars[owner.id] ?? {
                let made = UIProgressView(progressViewStyle: .default)
                made.isUserInteractionEnabled = false
                bars[owner.id] = made
                return made
            }()
            let mount = owner.controlMount
            if bar.superview !== mount { mount.addSubview(bar) }
            let box = owner.contentBox()
            let height = bar.intrinsicContentSize.height
            assign(bar, \.frame, CGRect(x: box.minX, y: box.midY - height / 2, width: box.width, height: height))
            assign(bar, \.progressTintColor, owner.channels("accent_color").map { TextEngine.color($0) })
            assign(bar, \.isHidden, owner.cssVisibilityHidden)
            assign(bar, \.accessibilityIdentifier, owner.props["testId"])
            assign(bar, \.accessibilityLabel, owner.props["accessibilityLabel"])
            let fraction = Float(ProgressSpec(owner.props).fraction)
            if bar.progress != fraction { bar.setProgress(fraction, animated: !fresh && !held && bar.window != nil) }
            let words = owner.props["accessibilityValueText"].flatMap { $0.isEmpty ? nil : $0 }
            assign(bar, \.accessibilityValue, words ?? NumberFormatter.localizedString(from: NSNumber(value: fraction), number: .percent))
        }
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
        if let bar = bars[node.id] {
            return ["view": "UIProgressView", "progress": Double(bar.progress), "tinted": bar.progressTintColor != nil,
                    "size": [Agent.r2(bar.bounds.width), Agent.r2(bar.bounds.height)]]
        }
        guard let s = spinners[node.id] else { return nil }
        return ["view": "UIActivityIndicatorView", "style": s.style == .large ? "large" : "medium", "animating": s.isAnimating,
                "size": [Agent.r2(s.bounds.width), Agent.r2(s.bounds.height)]]
    }
}
#endif

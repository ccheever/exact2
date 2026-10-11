// @ref LLP 1069.001 (amended 2026-10-07) — an indeterminate `progress` is
// AppKit's spinning `NSProgressIndicator`, centred in the node's box: small,
// or regular where the box's shorter side holds its 32 points. AppKit gives
// a spinner no tint, so it draws in the system's colour, as `NSSwitch`
// takes no accent (D6's as built). It turns while it shows, and stops
// where it is hidden (`display: none` on it or above it), removed, or under
// the agent's clock, which shows one still frame. It is its own
// accessibility element (AppKit's busy indicator), named by the node's
// `aria-label`.
// @ref LLP 1116 D8 — with a `value`, it is AppKit's determinate bar
// instead, across the content box at its own height (regular where the box
// holds its 20 points, else small), from 0 to `max`. AppKit gives a bar no
// tint either: it draws in the system accent.
#if os(macOS)
import AppKit

/// An indicator a click passes through, to its node and then its
/// ancestors' `press`, as a box's content does (UIKit's indicator and bar
/// take no touch): the spinner, and the bar.
final class ExactIndicator: NSProgressIndicator {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

extension ControlHost {
    /// The shorter side at which the regular spinner replaces the small
    /// one: the regular one's own size.
    static let regularSide: CGFloat = 32
    /// The content height at which the regular bar replaces the small one:
    /// the regular one's own (20 points; the small one is 12).
    static let regularBar: CGFloat = 20

    func syncProgress() {
        let all = presenter.carrying(ControlKinds.progress).filter { $0.kind == "control" }
        let leaving = Set(presenter.leaving.values.flatMap { $0.members.map(\.id) })
        syncBars(all.filter { $0.props["value"] != nil }, leaving: leaving)
        let owners = all.filter { $0.props["value"] == nil }
        let live = Set(owners.map(\.id))
        for id in Array(spinners.keys) where !live.contains(id) && !leaving.contains(id) {
            if let gone = spinners.removeValue(forKey: id) {
                gone.stopAnimation(nil)
                gone.removeFromSuperview()
            }
        }
        let held = ExactEnv.agentFreezes || presenter.session?.clock != nil
        for owner in owners {
            let spinner = spinners[owner.id] ?? {
                let made = ExactIndicator()
                made.style = .spinning
                made.isIndeterminate = true
                // Stopped, it still shows its frame: the agent's still one.
                made.isDisplayedWhenStopped = true
                spinners[owner.id] = made
                return made
            }()
            let mount = owner.controlMount
            if spinner.superview !== mount { mount.addSubview(spinner) }
            let box = owner.contentBox()
            let size: NSControl.ControlSize = min(box.width, box.height) >= Self.regularSide ? .regular : .small
            if spinner.controlSize != size { spinner.controlSize = size }
            if spinner.isHidden != owner.cssVisibilityHidden { spinner.isHidden = owner.cssVisibilityHidden }
            spinner.setAccessibilityIdentifier(owner.props["testId"])
            spinner.setAccessibilityLabel(owner.props["accessibilityLabel"])
            let side: CGFloat = size == .regular ? 32 : 16
            let frame = NSRect(x: box.midX - side / 2, y: box.midY - side / 2, width: side, height: side)
            if spinner.frame != frame { spinner.frame = frame }
            let turns = !held && !spinner.isHiddenOrHasHiddenAncestor
            if turns != animating.contains(owner.id) {
                if turns { spinner.startAnimation(nil); animating.insert(owner.id) } else { spinner.stopAnimation(nil); animating.remove(owner.id) }
            }
        }
        // One leaving with its exit holds too, under the agent's clock.
        if held {
            for id in leaving where animating.contains(id) {
                spinners[id]?.stopAnimation(nil)
                animating.remove(id)
            }
        }
        animating.formIntersection(Set(spinners.keys))
    }

    /// The determinate bars (LLP 1116 D8): AppKit's own value, minimum and
    /// maximum are what VoiceOver reads, `aria-valuetext` its description.
    private func syncBars(_ owners: [NodeView], leaving: Set<UInt32>) {
        let live = Set(owners.map(\.id))
        for id in Array(bars.keys) where !live.contains(id) && !leaving.contains(id) {
            bars.removeValue(forKey: id)?.removeFromSuperview()
        }
        for owner in owners {
            let bar = bars[owner.id] ?? {
                let made = ExactIndicator()
                made.style = .bar
                made.isIndeterminate = false
                made.minValue = 0
                bars[owner.id] = made
                return made
            }()
            let mount = owner.controlMount
            if bar.superview !== mount { mount.addSubview(bar) }
            let box = owner.contentBox()
            let size: NSControl.ControlSize = box.height >= Self.regularBar ? .regular : .small
            if bar.controlSize != size { bar.controlSize = size }
            let height = bar.intrinsicContentSize.height
            let frame = NSRect(x: box.minX, y: box.midY - height / 2, width: box.width, height: height)
            if bar.frame != frame { bar.frame = frame }
            if bar.isHidden != owner.cssVisibilityHidden { bar.isHidden = owner.cssVisibilityHidden }
            let spec = ProgressSpec(owner.props)
            if bar.maxValue != spec.max { bar.maxValue = spec.max }
            if bar.doubleValue != spec.value { bar.doubleValue = spec.value }
            if bar.accessibilityIdentifier() != owner.props["testId"] ?? "" { bar.setAccessibilityIdentifier(owner.props["testId"]) }
            if bar.accessibilityLabel() != owner.props["accessibilityLabel"] { bar.setAccessibilityLabel(owner.props["accessibilityLabel"]) }
            let words = owner.props["accessibilityValueText"].flatMap { $0.isEmpty ? nil : $0 }
            if bar.accessibilityValueDescription() != words { bar.setAccessibilityValueDescription(words) }
        }
    }

    /// The agent's `native.control` for a progress.
    func progressObservation(_ node: NodeView) -> [String: Any]? {
        if let bar = bars[node.id] {
            return ["view": "NSProgressIndicator", "style": "bar", "value": bar.doubleValue, "max": bar.maxValue,
                    "controlSize": bar.controlSize == .regular ? "regular" : "small", "size": [Agent.r2(bar.frame.width), Agent.r2(bar.frame.height)]]
        }
        guard let s = spinners[node.id] else { return nil }
        return ["view": "NSProgressIndicator", "style": s.controlSize == .regular ? "regular" : "small", "animating": animating.contains(node.id),
                "size": [Agent.r2(s.frame.width), Agent.r2(s.frame.height)]]
    }
}
#endif

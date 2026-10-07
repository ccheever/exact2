// @ref LLP 1069.001 (amended 2026-10-07) — an indeterminate `progress` is
// AppKit's spinning `NSProgressIndicator`, centred in the node's box: small,
// or regular where the box's shorter side holds its 32 points. AppKit gives
// a spinner no tint, so it draws in the system's colour, as `NSSwitch`
// takes no accent (D6's as built). It turns while it shows, and stops
// where it is hidden (`display: none` on it or above it), removed, or under
// the agent's clock, which shows one still frame. It is its own
// accessibility element (AppKit's busy indicator), named by the node's
// `aria-label`.
#if os(macOS)
import AppKit

extension ControlHost {
    /// The shorter side at which the regular spinner replaces the small
    /// one: the regular one's own size.
    static let regularSide: CGFloat = 32

    func syncProgress() {
        let owners = presenter.carrying(ControlKinds.progress).filter { $0.kind == "control" }
        let live = Set(owners.map(\.id))
        let leaving = Set(presenter.leaving.values.flatMap { $0.members.map(\.id) })
        for id in Array(spinners.keys) where !live.contains(id) && !leaving.contains(id) {
            if let gone = spinners.removeValue(forKey: id) {
                gone.stopAnimation(nil)
                gone.removeFromSuperview()
            }
        }
        let held = ExactEnv.agentFreezes || presenter.session?.clock != nil
        for owner in owners {
            let spinner = spinners[owner.id] ?? {
                let made = NSProgressIndicator()
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
            if let label = owner.props["accessibilityLabel"] { spinner.setAccessibilityLabel(label) }
            let side: CGFloat = size == .regular ? 32 : 16
            let frame = NSRect(x: box.midX - side / 2, y: box.midY - side / 2, width: side, height: side)
            if spinner.frame != frame { spinner.frame = frame }
            let turns = !held && !spinner.isHiddenOrHasHiddenAncestor
            if turns != animating.contains(owner.id) {
                if turns { spinner.startAnimation(nil); animating.insert(owner.id) } else { spinner.stopAnimation(nil); animating.remove(owner.id) }
            }
        }
        animating.formIntersection(Set(spinners.keys))
    }

    /// The agent's `native.control` for a progress.
    func progressObservation(_ node: NodeView) -> [String: Any]? {
        guard let s = spinners[node.id] else { return nil }
        return ["view": "NSProgressIndicator", "style": s.controlSize == .regular ? "regular" : "small", "animating": animating.contains(node.id),
                "size": [Agent.r2(s.frame.width), Agent.r2(s.frame.height)]]
    }
}
#endif

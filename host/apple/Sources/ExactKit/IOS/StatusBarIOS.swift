// The status bar's style, declared from state (LLP 1105): `status-bar-style`
// on any node; the declaration painted on top of what covers the bar wins.
// tvOS has no status bar.
#if os(iOS)
import UIKit

/// What the bar shows (D1, D4) and how a change reaches it, with the node
/// that decided it (the agent's `layout`, D7).
struct StatusBarChoice: Equatable {
    var style: UIStatusBarStyle = .default
    var fade = false
    var source: UInt32?
}

extension Presenter {
    /// Resolve the style once and, if it changed, have every controller
    /// UIKit may ask answer it in this turn (D5, D6). While a navigation or
    /// a presentation is moving, the last committed style stays; its end
    /// resolves again.
    /// `settled`: a navigation's own end, whose coordinator is still winding
    /// down; a push it starts next still holds the style.
    func resolveStatusBar(settled: Bool = false) {
        guard !(settled ? navigation.started : navigation.transitioning), !modals.inTransition else { return }
        let choice = statusBarChoice()
        guard choice != statusBar else { return }
        statusBar = choice
        let apply = { [self] in
            onStatusBar?(choice)
            modals.statusBarChanged()
        }
        // `none` is no animation, even inside one already running (a keyboard's).
        if choice.fade, !ExactEnv.agentFreezes { UIView.animate(withDuration: 0.3, animations: apply) } else { UIView.performWithoutAnimation(apply) }
    }

    /// The declarations in scope that show; the one painted on top (D2, D3).
    func statusBarChoice() -> StatusBarChoice {
        let declared = carrying("statusBarStyle")
        guard !declared.isEmpty else { return StatusBarChoice() }
        let scope = modals.statusBarScope
        // A node shown in a context menu's preview is UIKit's, not the screen's:
        // the scope is the session's view and the presentations.
        let roots = [session?.view].compactMap { $0 } + scope.views
        let lifted = Set(navigation.controllers.values.compactMap { $0.lifted.map(ObjectIdentifier.init) })
        func shows(_ node: NodeView) -> Bool {
            // `visibility` is inherited and a child may show again: the node's own.
            guard node.window != nil, !node.cssVisibilityHidden else { return false }
            if session?.view != nil, !roots.contains(where: { node.isDescendant(of: $0) }) { return false }
            var up: UIView? = node
            while let u = up {
                if let n = u as? NodeView {
                    // A header the navigation bar shows in its place counts there.
                    if n.isHidden, !lifted.contains(ObjectIdentifier(n)) { return false }
                } else if u.isHidden { return false }
                up = u.superview
            }
            return true
        }
        let inScope: (NodeView) -> Bool = { node in
            if let covering = scope.covering { return node.isDescendant(of: covering) }
            return !scope.presented.contains { node.isDescendant(of: $0) }
        }
        let candidates = declared.filter { node in
            let value = node.props["statusBarStyle"] ?? ""
            guard ["light-content", "dark-content", "auto"].contains(value) else {
                noteStatusBarValue(node, "status-bar-style", value)
                return false
            }
            if let a = node.props["statusBarAnimation"], a != "none", a != "fade" { noteStatusBarValue(node, "status-bar-animation", a) }
            return inScope(node) && shows(node)
        }
        guard var winner = candidates.first else { return StatusBarChoice() }
        for next in candidates.dropFirst() where Self.paintedAbove(next, winner) { winner = next }
        let style: UIStatusBarStyle = switch winner.props["statusBarStyle"] {
        case "light-content": .lightContent
        case "dark-content": .darkContent
        default: Self.schemeStatusBar(winner)
        }
        return StatusBarChoice(style: style, fade: winner.props["statusBarAnimation"] == "fade", source: winner.id)
    }

    /// A bound value outside the set counts as unset (D1), said once per node and value.
    private func noteStatusBarValue(_ node: NodeView, _ attribute: String, _ value: String) {
        guard statusBarNoted.insert("\(node.id)|\(attribute)|\(value)").inserted else { return }
        session?.log("\(attribute)=\"\(value)\" on #\(node.id) is not one of its values; unset (LLP 1105 D1)")
    }

    /// Whether `a` paints over `b`: a descendant over its ancestor; between
    /// branches, at their nearest common ancestor, the higher `zPosition`,
    /// then the later subview, as Core Animation paints them.
    static func paintedAbove(_ a: UIView, _ b: UIView) -> Bool {
        if a.isDescendant(of: b) { return a !== b }
        if b.isDescendant(of: a) { return false }
        let chain = { (v: UIView) in sequence(first: v, next: \.superview).map { $0 } }
        let up = chain(b)
        for (child, parent) in zip(chain(a), chain(a).dropFirst()) {
            guard let i = up.firstIndex(where: { $0 === parent }), i > 0 else { continue }
            let other = up[i - 1]
            if child.layer.zPosition != other.layer.zPosition { return child.layer.zPosition > other.layer.zPosition }
            let subviews = parent.subviews
            return (subviews.firstIndex { $0 === child } ?? 0) > (subviews.firstIndex { $0 === other } ?? 0)
        }
        return false
    }

    /// `auto` (D4): the scheme a `color-scheme` subtree forces where the node
    /// sits, else the platform's default, which follows the system's.
    static func schemeStatusBar(_ node: UIView) -> UIStatusBarStyle {
        var up: UIView? = node
        while let v = up {
            if v.overrideUserInterfaceStyle != .unspecified { return v.overrideUserInterfaceStyle == .dark ? .lightContent : .darkContent }
            up = v.superview
        }
        return .default
    }
}
#endif

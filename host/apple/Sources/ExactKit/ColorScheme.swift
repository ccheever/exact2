// `color-scheme` on a subtree (LLP 1034 §8): a node view's own appearance,
// which the platform's appearance inheritance carries to every view below.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// `color-scheme` on a subtree (LLP 1034 §8): the view's own
    /// appearance, which UIKit's trait inheritance carries to every view
    /// below — so `light-dark()`, platform colours and materials there
    /// resolve in it — and each re-resolves through its trait-change
    /// registration. Outside a batch the trait update is taken now, so this
    /// style pass already reads the new scheme; in one, at its end
    /// (`ColorSchemes`).
    func applyColorScheme() {
        settleColorScheme()
        // The node views below that inherit through this one decide again.
        var below = subviews
        while let view = below.popLast() {
            if let node = view as? NodeView { node.settleColorScheme() } else { below += view.subviews }
        }
    }

    /// The property is inherited, so every node under one that sets it
    /// carries the same value, which UIKit's trait inheritance already gives
    /// it; UIKit allocates trait overrides and a trait collection for each
    /// view that overrides. So a view overrides only where its scheme is not
    /// the one it inherits: that of the node view above, of none past a view
    /// controller's view (whose traits are the controller's), and, not yet
    /// placed, the appearance it has. Again when it moves (`didMoveToSuperview`).
    func settleColorScheme() {
        let wanted = colorScheme
        let inherited: UIUserInterfaceStyle? = if superview == nil {
            overrideUserInterfaceStyle == .unspecified ? traitCollection.userInterfaceStyle : nil
        } else {
            schemeParent?.colorScheme
        }
        let own = wanted != .unspecified && inherited == wanted ? .unspecified : wanted
        guard overrideUserInterfaceStyle != own else { return }
        overrideUserInterfaceStyle = own
        ColorSchemes.changed(self)
    }

    private var colorScheme: UIUserInterfaceStyle {
        switch style["color_scheme"]?.string {
        case "dark": .dark
        case "light": .light
        default: .unspecified
        }
    }

    /// The nearest node view above, through plain views, whose appearance
    /// this view inherits.
    private var schemeParent: NodeView? {
        var at = superview
        while let view = at {
            if let node = view as? NodeView { return node }
            if view.next is UIViewController { return nil }
            at = view.superview
        }
        return nil
    }
}

/// The overrides a batch changes, made visible together at its end: each
/// override is set first and the traits are updated once, from the topmost
/// changed views. Updated as each changed, a view that kept its parent's old
/// scheme would show the new one until the walk below pinned it back (and
/// report it), and a chain of N views would update its subtree N times.
enum ColorSchemes {
    private static var depth = 0
    private static var staged: [NodeView] = []
    static func begin() { depth += 1 }
    static func changed(_ view: NodeView) {
        if depth == 0 { view.updateTraitsIfNeeded() } else { staged.append(view) }
    }
    static func end() {
        depth -= 1
        guard depth == 0, !staged.isEmpty else { return }
        let views = staged
        staged = []
        let changed = Set(views.map(ObjectIdentifier.init))
        for view in views {
            var above = view.superview, covered = false
            while let up = above, !covered { covered = changed.contains(ObjectIdentifier(up)); above = up.superview }
            if !covered { view.updateTraitsIfNeeded() }
        }
    }
}
#elseif os(macOS)
import AppKit

extension NodeView {
    /// `color-scheme` on a subtree (LLP 1034 §8): the view's own
    /// appearance, which AppKit's `effectiveAppearance` carries to every view
    /// below, so `light-dark()`, platform colours and materials there resolve
    /// in it; each re-resolves through `viewDidChangeEffectiveAppearance`.
    func applyColorScheme() {
        let wanted: NSAppearance? = switch style["color_scheme"]?.string {
        case "dark": NSAppearance(named: .darkAqua)
        case "light": NSAppearance(named: .aqua)
        default: nil
        }
        guard appearance?.name != wanted?.name else { return }
        appearance = wanted
    }
}
#endif

// `color-scheme` on a subtree (LLP 1034 §8): a node view's own appearance,
// which the platform's appearance inheritance carries to every view below.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// `color-scheme` on a subtree (LLP 1034 §8): the view's own
    /// appearance, which UIKit's trait inheritance carries to every view
    /// below — so `light-dark()`, platform colours and materials there
    /// resolve in it — and each re-resolves through its trait-change
    /// registration. The trait update is taken now, so this style pass
    /// already reads the new scheme.
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
        updateTraitsIfNeeded()
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

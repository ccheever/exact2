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
        let wanted: UIUserInterfaceStyle = switch style["color_scheme"]?.string {
        case "dark": .dark
        case "light": .light
        default: .unspecified
        }
        guard overrideUserInterfaceStyle != wanted else { return }
        overrideUserInterfaceStyle = wanted
        updateTraitsIfNeeded()
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

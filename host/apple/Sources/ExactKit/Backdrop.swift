// CSS `backdrop-filter: blur(σ)` on both Apple platforms (LLP 1053.000 D2).
//
// macOS: Core Image's Gaussian as the node layer's public
// `backgroundFilters`, run between the sRGB tone curves because Core Image
// blurs linear light and Chrome blurs encoded values; measured against
// Chrome's picture of scripts/fixtures/backdrop.contract its σ lands within
// 0.05 pt. AppKit filters a layer's backdrop only when the layer masks to
// its bounds, and the backdrop it sees is the superlayer's subtree: the
// parent's paint and the earlier siblings, not what a further ancestor
// paints. Both are declared (LLP 1053.000 §3, LLP 1001 §1): a backdrop
// node clips its children to its border box, and paint above its parent
// shows through unblurred. The node's own background and border draw over
// the blur, as CSS orders them.
//
// iOS has no public arbitrary-radius backdrop blur (`backgroundFilters` is
// macOS-only and `CABackdropLayer` is private), so a blur maps to the
// system material nearest it by measured pixels (ultra-thin), drawn as the node's
// material view under its children: a declared deviation with its measured
// bound (LLP 1053.000 §3). `backgroundMaterial` stays the host-policy
// spelling and wins on a node that has both, on every host.
import CoreGraphics
#if os(iOS)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    /// The material this node asks for: the host-policy prop, else a
    /// backdrop blur, else none.
    var materialRequest: String? {
        if let material = props["backgroundMaterial"] { return material }
        return number("backdrop_blur") > 0 ? "backdrop" : nil
    }
}

#if os(iOS)
/// A backdrop blur's material view, remembering the σ it was made for.
final class BackdropEffectView: UIVisualEffectView {
    var sigma: CGFloat = -1
}

extension NodeView {
    /// Whether the backdrop's effect no longer matches its σ.
    var backdropStale: Bool {
        (materialView as? BackdropEffectView).map { $0.sigma != number("backdrop_blur") } ?? false
    }

    /// The backdrop blur's effect, nil when this is not a backdrop.
    func backdropEffect() -> UIVisualEffect? {
        guard let view = materialView as? BackdropEffectView else { return nil }
        view.sigma = number("backdrop_blur")
        return UIBlurEffect(style: Backdrop.material(sigma: view.sigma))
    }
}

enum Backdrop {
    /// The material for a blur of σ points. The system materials blur with
    /// their own Gaussian (σ ≈ 19–33 pt on the iOS 27 simulator, whatever
    /// σ was asked) and tint, so none is nearer another σ; measured against
    /// Chrome over the parity page (LLP 1053.000 §3), ultra-thin is nearest
    /// for a tinted glass and 2–7/255 behind the best for a bare blur, and
    /// it adapts to the appearance as CSS's `light-dark()` tint would.
    static func material(sigma _: CGFloat) -> UIBlurEffect.Style { .systemUltraThinMaterial }
}
#else
import CoreImage

extension NodeView {
    /// The layer's backdrop blur, or none; a material (`backgroundMaterial`)
    /// takes the node's backdrop instead.
    func applyBackdrop() {
        guard let l = layer else { return }
        let sigma = materialView == nil && props["backgroundMaterial"] == nil ? max(0, number("backdrop_blur")) : 0
        guard sigma > 0 else {
            if l.backgroundFilters != nil { l.backgroundFilters = nil }
            return
        }
        if !layerUsesCoreImageFilters { layerUsesCoreImageFilters = true }
        let current = (l.backgroundFilters?.dropFirst().first as? CIFilter)?.value(forKey: kCIInputRadiusKey) as? Double
        if current != Double(sigma), let blur = CIFilter(name: "CIGaussianBlur"),
           let encode = CIFilter(name: "CILinearToSRGBToneCurve"), let decode = CIFilter(name: "CISRGBToneCurveToLinear") {
            blur.setValue(Double(sigma), forKey: kCIInputRadiusKey)
            l.backgroundFilters = [encode, blur, decode]
        }
        // AppKit filters the backdrop only inside a masking layer; one
        // radius rides it (differing radii take their smallest).
        if !clipsToBounds { clipsToBounds = true }
        let radii = ["top_left", "top_right", "bottom_right", "bottom_left"].map { CGFloat(number("border_radius_" + $0, number("border_radius"))) }
        let radius = max(0, radii.min() ?? 0)
        if l.cornerRadius != radius { l.cornerRadius = radius }
    }
}
#endif

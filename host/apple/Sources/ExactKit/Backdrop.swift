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
// blur style nearest it by measured pixels (`.light`, Charlie 2026-09-27), drawn as the node's
// material view under its children: a declared deviation with its measured
// bound (LLP 1053.000 §3). `backgroundMaterial` stays the host-policy
// spelling and wins on a node that has both, on every host.
import CExact
import CoreGraphics
#if os(iOS)
import UIKit
#else
import AppKit
#endif

/// `backgroundMaterial`'s vocabulary (LLP 1053.000 D4): the schema's table,
/// read through `exact_material_platform`, as this platform's own names.
enum Materials {
    #if os(iOS)
    static let platformCode: UInt8 = 0
    #else
    static let platformCode: UInt8 = 1
    #endif

    /// Whether `name` is a glass effect (`UIGlassEffect`, `NSGlassEffectView`).
    static func glass(_ name: String?) -> Bool { name == "glass" || name == "glass-clear" }

    /// The schema's name on this platform for `name`, and whether that is a
    /// stand-in for a material this platform lacks; nil for no such name.
    static func platform(_ name: String) -> (apple: String, standIn: Bool)? {
        var bytes = Array(name.utf8)
        var out: UnsafePointer<UInt8>?
        let n = exact_material_platform(&bytes, bytes.count, platformCode, &out)
        guard n > 0, let out else { return nil }
        let value = String(decoding: UnsafeBufferPointer(start: out, count: n), as: UTF8.self)
        return value.hasPrefix("~") ? (String(value.dropFirst()), true) : (value, false)
    }

    private static var noted = Set<String>()

    /// This platform's name for `name`: a stand-in where the table names
    /// one, silently (Charlie, 2026-09-27: "ok, (b)"; the agent's `state`
    /// shows it, `agentMaterial`); an unknown name draws `ultra-thin`'s and
    /// is logged once (`log`).
    static func resolve(_ name: String, log: (String) -> Void) -> String {
        if let (apple, _) = platform(name) { return apple }
        if noted.insert(name).inserted { log("backgroundMaterial `\(name)` is not a material; drawing ultra-thin") }
        return platform("ultra-thin")!.apple
    }

    /// What the agent's `state` reports for material `name`: the platform's
    /// material, a stand-in marked as one, or the fallback for an unknown name.
    static func agentMaterial(_ name: String) -> [String: Any] {
        guard let (apple, standIn) = platform(name) else {
            return ["name": name, "drawn": platform("ultra-thin")!.apple, "unknown": true]
        }
        return ["name": name, "drawn": apple, "standIn": standIn]
    }

    #if os(iOS)
    /// A `UIBlurEffect.Style` by its Swift name.
    static func blurStyle(_ apple: String) -> UIBlurEffect.Style? {
        switch apple {
        case "extraLight": .extraLight
        case "light": .light
        case "dark": .dark
        case "regular": .regular
        case "prominent": .prominent
        case "systemUltraThinMaterial": .systemUltraThinMaterial
        case "systemThinMaterial": .systemThinMaterial
        case "systemMaterial": .systemMaterial
        case "systemThickMaterial": .systemThickMaterial
        case "systemChromeMaterial": .systemChromeMaterial
        case "systemUltraThinMaterialLight": .systemUltraThinMaterialLight
        case "systemThinMaterialLight": .systemThinMaterialLight
        case "systemMaterialLight": .systemMaterialLight
        case "systemThickMaterialLight": .systemThickMaterialLight
        case "systemChromeMaterialLight": .systemChromeMaterialLight
        case "systemUltraThinMaterialDark": .systemUltraThinMaterialDark
        case "systemThinMaterialDark": .systemThinMaterialDark
        case "systemMaterialDark": .systemMaterialDark
        case "systemThickMaterialDark": .systemThickMaterialDark
        case "systemChromeMaterialDark": .systemChromeMaterialDark
        default: nil
        }
    }
    #else
    /// An `NSVisualEffectView.Material` by its Swift name.
    static func material(_ apple: String) -> NSVisualEffectView.Material? {
        switch apple {
        case "titlebar": .titlebar
        case "selection": .selection
        case "menu": .menu
        case "popover": .popover
        case "sidebar": .sidebar
        case "headerView": .headerView
        case "sheet": .sheet
        case "windowBackground": .windowBackground
        case "hudWindow": .hudWindow
        case "fullScreenUI": .fullScreenUI
        case "toolTip": .toolTip
        case "contentBackground": .contentBackground
        case "underWindowBackground": .underWindowBackground
        case "underPageBackground": .underPageBackground
        default: nil
        }
    }
    #endif
}

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

    /// The effect for material `kind` (LLP 1053.000 D4): a glass effect on
    /// iOS 26 (else ultra-thin), otherwise the table's blur style.
    func materialEffect(_ kind: String, interactive: Bool) -> UIVisualEffect {
        let apple = Materials.resolve(kind) { [weak self] in self?.presenter?.session?.log($0) }
        if apple == "glass" || apple == "glassClear" {
            if #available(iOS 26.0, *) {
                let glass = UIGlassEffect(style: apple == "glass" ? .regular : .clear)
                glass.isInteractive = interactive
                return glass
            }
            return UIBlurEffect(style: .systemUltraThinMaterial)
        }
        return UIBlurEffect(style: Materials.blurStyle(apple) ?? .systemUltraThinMaterial)
    }

    /// The backdrop blur's effect, nil when this is not a backdrop.
    func backdropEffect() -> UIVisualEffect? {
        guard let view = materialView as? BackdropEffectView else { return nil }
        view.sigma = number("backdrop_blur")
        return UIBlurEffect(style: Backdrop.material(sigma: view.sigma))
    }
}

enum Backdrop {
    /// The style for a blur of σ points. The system styles blur with their
    /// own Gaussian (σ ≈ 19–33 pt on the iOS simulator, whatever σ was
    /// asked) and tint, so none is nearer another σ. Measured against
    /// Chrome over the parity page, `.light` is nearest in every case, the
    /// tinted glass included (LLP 1053.000 §3; Charlie, 2026-09-27: "yes
    /// switch to regular or light"). It does not follow the appearance.
    static func material(sigma _: CGFloat) -> UIBlurEffect.Style { .light }
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
            if l.backgroundFilters != nil { l.backgroundFilters = nil; syncEllipticalClip() }
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
        let radii = cornerSizes(in: bounds)
        let radius = radii.allSatisfy { $0.width == $0.height } ? radii.map(\.width).min() ?? 0 : 0
        if l.cornerRadius != radius { l.cornerRadius = radius }
        syncEllipticalClip()
    }
}
#endif

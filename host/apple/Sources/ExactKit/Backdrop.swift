// CSS `backdrop-filter` on both Apple platforms (LLP 1053.000 D2).
//
// macOS: Core Image's Gaussian as the node layer's public
// `backgroundFilters`, run between the sRGB tone curves because Core Image
// blurs linear light and Chrome blurs encoded values; measured against
// Chrome's picture of scripts/fixtures/backdrop.contract its σ lands within
// 0.05 pt. Core Animation hands the filters the backdrop past the box,
// where Chrome reads only the box and mirrors it past its edges; the chain
// mirrors the box first, so the edges land within 2/255 per channel of
// Chrome's (#129's panels; a turned or skewed box still reads past it).
// AppKit filters a layer's backdrop only when the layer masks to its
// bounds, and the backdrop it sees is the superlayer's subtree: the
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
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

/// `backgroundMaterial`'s vocabulary (LLP 1053.000 D4): the schema's table,
/// read through `exact_material_platform`, as this platform's own names.
enum Materials {
    #if os(iOS) || os(tvOS)
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

    #if os(iOS) || os(tvOS)
    /// A `UIBlurEffect.Style` by its Swift name.
    static func blurStyle(_ apple: String) -> UIBlurEffect.Style? {
        switch apple {
        case "extraLight": .extraLight
        case "light": .light
        case "dark": .dark
        case "regular": .regular
        case "prominent": .prominent
        #if !os(tvOS)
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
        #endif
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

/// The kernel's ordered backdrop operations, decoded once from its typed wire.
enum BackdropOperation: Equatable {
    case blur(CGFloat), saturate(CGFloat)

    init?(_ value: BatchValue) {
        guard case .object(let fields) = value else { return nil }
        if let n = fields["blur"]?.number { self = .blur(CGFloat(n)) }
        else if let n = fields["saturate"]?.number { self = .saturate(CGFloat(n)) }
        else { return nil }
    }
}

extension NodeView {
    var backdropOperations: [BackdropOperation] {
        style["backdrop_filter"]?.array?.compactMap(BackdropOperation.init) ?? []
    }

    /// The material this node asks for: the host-policy prop, else a
    /// backdrop, else a popover's default surface (UIKit's popover glass).
    var materialRequest: String? {
        if let material = props["backgroundMaterial"] { return material }
        if !backdropOperations.isEmpty { return "backdrop" }
        // An authored corner the glass can't take (not one circle): no default surface, rather than a square one.
        return defaultSurfaceRadius == nil || unevenCorners ? nil : "glass"
    }

    /// The corner of a popover's default surface (LLP 1021; theme2
    /// 2026-10-21 19:55): the compiler marks a popover whose author wrote no
    /// surface (`defaultSurface`), and the host sends the corner only where
    /// this platform's metrics give one (`popover-radius`: iOS's, measured
    /// against iOS 26's glass popover on iOS 27; an earlier iOS draws the
    /// glass's ultra-thin stand-in with that corner and shadow, unmeasured
    /// against the pre-glass popover, no older runtime being installed). It
    /// is a popover while `popover` says so; a menu, by its role or as a
    /// node's context menu, is the platform's own. Nil: none here.
    var defaultSurfaceRadius: CGFloat? {
        guard props["defaultSurface"] == "true", props["popover"] != nil, props["accessibilityRole"] != "menu",
              let radius = props["defaultSurfaceRadius"].flatMap(Double.init) else { return nil }
        #if os(iOS)
        if let id = props["id"], presenter?.carrying("contextPopover").contains(where: { $0.props["contextPopover"] == id }) == true { return nil }
        #endif
        return CGFloat(radius)
    }

    #if os(iOS) || os(tvOS)
    /// A scrolling popover's children are its scroll view's, not the glass's
    /// content view's: the scroll view takes the default surface's corner
    /// while that applies, and gives it up when it no longer does.
    func syncDefaultSurfaceScroll() {
        // The corner the glass has: the author's (one circle) or the default's.
        let corner = materialView == nil || defaultSurfaceRadius == nil ? nil : writesCorner ? cornerSizes(in: bounds).first?.width : defaultSurfaceRadius
        guard let scroll, corner != nil || scrollCornered else { return }
        scroll.layer.cornerRadius = corner ?? 0
        scroll.layer.cornerCurve = corner == nil ? .circular : .continuous
        scrollCornered = corner != nil
    }
    #endif

    /// Whether its author's corners are not one circle, as written (a row
    /// each, or `border-radius` for the ones it covers, an unwritten one 0),
    /// whatever the box: they differ, or one isn't points (a percentage,
    /// `{"pct"}`, is an ellipse on a box that isn't square; a calculation
    /// waits on its basis), which the glass can't follow.
    var unevenCorners: Bool {
        let corners = ["top_left", "top_right", "bottom_right", "bottom_left"].map { style["border_radius_" + $0] ?? style["border_radius"] ?? .number(0) }
        return Set(corners.map(\.key)).count > 1 || corners.contains { $0.number == nil }
    }

    /// Whether its author wrote a corner, even 0: theirs, not the default's.
    var writesCorner: Bool {
        style["border_radius"] != nil || ["top_left", "top_right", "bottom_right", "bottom_left"].contains { style["border_radius_" + $0] != nil }
    }

    /// That surface's shadow, black at the metrics' alpha, offset down and
    /// blurred as they say (`defaultSurfaceShadow`: "y blur alpha"); in the
    /// light appearance only, as UIKit's popover casts it (none in dark,
    /// measured over a coloured page).
    var defaultSurfaceShadow: BoxShadowSpec? {
        guard defaultSurfaceRadius != nil, !drawsDark, let parts = props["defaultSurfaceShadow"]?.split(separator: " ").compactMap({ Double($0) }),
              parts.count == 3 else { return nil }
        return BoxShadowSpec(color: CGColor(gray: 0, alpha: parts[2]), offset: CGSize(width: 0, height: parts[0]),
                             blur: parts[1], spread: 0, inset: false)
    }
}

#if os(iOS) || os(tvOS)
/// The system material approximation, remembering its authored functions.
final class BackdropEffectView: UIVisualEffectView {
    var operations: [BackdropOperation] = []
}

extension NodeView {
    /// Whether the approximation is stale after a style update.
    var backdropStale: Bool {
        (materialView as? BackdropEffectView).map { $0.operations != backdropOperations } ?? false
    }

    /// The effect for material `kind` (LLP 1053.000 D4): a glass effect on
    /// iOS 26 (else ultra-thin), otherwise the table's blur style.
    func materialEffect(_ kind: String, interactive: Bool) -> UIVisualEffect {
        let apple = Materials.resolve(kind) { [weak self] in self?.presenter?.session?.log($0) }
        if apple == "glass" || apple == "glassClear" {
            if #available(iOS 26.0, tvOS 26.0, *) {
                let glass = UIGlassEffect(style: apple == "glass" ? .regular : .clear)
                glass.isInteractive = interactive
                return glass
            }
            #if os(tvOS)
            // tvOS has no system materials; .regular is its nearest blur.
            return UIBlurEffect(style: .regular)
            #else
            return UIBlurEffect(style: .systemUltraThinMaterial)
            #endif
        }
        #if os(tvOS)
        return UIBlurEffect(style: Materials.blurStyle(apple) ?? .regular)
        #else
        return UIBlurEffect(style: Materials.blurStyle(apple) ?? .systemUltraThinMaterial)
        #endif
    }

    /// The backdrop blur's effect, nil when this is not a backdrop.
    func backdropEffect() -> UIVisualEffect? {
        guard let view = materialView as? BackdropEffectView else { return nil }
        view.operations = backdropOperations
        return UIBlurEffect(style: Backdrop.material)
    }
}

enum Backdrop {
    /// The style for a blur of σ points. The system styles blur with their
    /// own Gaussian (σ ≈ 19–33 pt on the iOS simulator, whatever σ was
    /// asked) and tint, so none is nearer another σ. Measured against
    /// Chrome over the parity page, `.light` is nearest in every case, the
    /// tinted glass included (LLP 1053.000 §3; Charlie, 2026-09-27: "yes
    /// switch to regular or light"). It does not follow the appearance.
    static var material: UIBlurEffect.Style { .light }
}
#else
import CoreImage

extension NodeView {
    /// The layer's backdrop blur, or none; a material (`backgroundMaterial`)
    /// takes the node's backdrop instead.
    func applyBackdrop() {
        guard let l = layer else { return }
        let operations = !cssVisibilityHidden && materialView == nil && props["backgroundMaterial"] == nil ? backdropOperations : []
        guard !operations.isEmpty else {
            if l.backgroundFilters != nil { l.backgroundFilters = nil; backdropDrawn = nil; syncEllipticalClip() }
            return
        }
        if !layerUsesCoreImageFilters { layerUsesCoreImageFilters = true }
        // Core Animation hands the filters the backdrop in the superlayer's
        // space, where the box is the frame moved and scaled by the layer's
        // transform about its anchor (a press, a flight, `translate`). A
        // turned, skewed or 3D layer, or one under a parent's perspective,
        // reads past its box: not measured.
        let t = l.transform, size = frame.size
        let anchor = CGPoint(x: l.anchorPoint.x * size.width, y: l.anchorPoint.y * size.height)
        let flat = l.superlayer.map { CATransform3DIsIdentity($0.sublayerTransform) } ?? true
        let box = flat && CATransform3DIsAffine(t) && t.m12 == 0 && t.m21 == 0
            ? CGRect(origin: CGPoint(x: -anchor.x, y: -anchor.y), size: size).applying(CATransform3DGetAffineTransform(t))
                .offsetBy(dx: frame.minX + anchor.x, dy: frame.minY + anchor.y)
            : nil
        let drawn = BackdropDrawn(operations: operations, box: box)
        if backdropDrawn != drawn || l.backgroundFilters == nil {
            l.backgroundFilters = Backdrop.filters(drawn)
            backdropDrawn = drawn
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

/// The filter list and its box in the superlayer (nil: blur reads past the box).
struct BackdropDrawn: Equatable {
    var operations: [BackdropOperation]
    var box: CGRect?
}

enum Backdrop {
    /// Core Image's chain for `drawn`. Chrome reads the backdrop inside the
    /// border box and mirrors it past the edges: the box is scaled to a
    /// square, which `CIFourfoldReflectedTile` mirrors (its tile runs from
    /// its centre by its width), and scaled back. The Gaussian runs between
    /// the sRGB tone curves, as Chrome blurs encoded values.
    static func filters(_ drawn: BackdropDrawn) -> [CIFilter] {
        func filter(_ name: String, _ values: [String: Any] = [:]) -> CIFilter? {
            let f = CIFilter(name: name)
            for (key, value) in values { f?.setValue(value, forKey: key) }
            return f
        }
        func transform(_ t: CGAffineTransform) -> CIFilter? {
            let ns = NSAffineTransform()
            ns.transformStruct = NSAffineTransformStruct(m11: t.a, m12: t.b, m21: t.c, m22: t.d, tX: t.tx, tY: t.ty)
            return filter("CIAffineTransform", [kCIInputTransformKey: ns])
        }
        var chain: [CIFilter?] = []
        let blurs = drawn.operations.contains { if case .blur(let sigma) = $0 { return sigma > 0 }; return false }
        if blurs, let box = drawn.box, box.width > 0, box.height > 0 {
            let side = max(box.width, box.height)
            let square = CGAffineTransform(scaleX: side / box.width, y: side / box.height).translatedBy(x: -box.minX, y: -box.minY)
            let mirror = filter("CIFourfoldReflectedTile", [kCIInputCenterKey: CIVector(x: 0, y: 0), kCIInputWidthKey: Double(side),
                                                             kCIInputAngleKey: 0.0, "inputAcuteAngle": Double.pi / 2])
            chain = [transform(square), mirror, transform(square.inverted())]
        }
        chain.append(filter("CILinearToSRGBToneCurve"))
        for operation in drawn.operations {
            switch operation {
            case .blur(let sigma):
                chain.append(filter("CIGaussianBlur", [kCIInputRadiusKey: Double(sigma)]))
            case .saturate(let amount):
                // Filter Effects 1's saturation matrix, in encoded sRGB.
                // CIColorControls chooses its own grayscale weights. The
                // explicit matrix keeps CSS's weights, and its output must
                // clamp before a later blur sees it (amount may exceed one).
                let r = 0.213 * (1 - amount), g = 0.715 * (1 - amount), b = 0.072 * (1 - amount)
                chain.append(filter("CIColorMatrix", [
                    "inputRVector": CIVector(x: r + amount, y: g, z: b, w: 0),
                    "inputGVector": CIVector(x: r, y: g + amount, z: b, w: 0),
                    "inputBVector": CIVector(x: r, y: g, z: b + amount, w: 0)]))
                chain.append(filter("CIColorClamp", [
                    "inputMinComponents": CIVector(x: 0, y: 0, z: 0, w: 0),
                    "inputMaxComponents": CIVector(x: 1, y: 1, z: 1, w: 1)]))
            }
        }
        chain.append(filter("CISRGBToneCurveToLinear"))
        return chain.compactMap { $0 }
    }
}
#endif

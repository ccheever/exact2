import ExactCore
import Foundation
import CoreGraphics

extension BatchValue {
    package func channels(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil) -> [Double]? {
        if let c = numbers, c.count == 4 { return c }
        // @ref LLP 1095 D5 — a platform colour by name, its pair the fallback.
        if case .object(let o) = self, let name = o["sys"]?.string {
            return SystemColor.channels(name, dark: dark, contrast: contrast, elevated: elevated, tintColor: tint, fallback: o["c"]?.channels(dark: dark))
        }
        // A colour in its own space (LLP 1100 D2): its sRGB clip; a profile's
        // colour has none on the wire, so Core Graphics converts it (D3).
        if case .object(let o) = self, o["cs"] != nil {
            if let c = o["c"] { return c.channels(dark: dark) }
            guard let cg = cgColor(dark: dark), let srgb = CGColorSpace(name: CGColorSpace.sRGB),
                  let c = cg.converted(to: srgb, intent: .relativeColorimetric, options: nil)?.components, c.count == 4 else { return nil }
            return c.map { (Double($0) * 255).rounded() }
        }
        guard let a = array, a.count == 2, let c = a[dark ? 1 : 0].numbers, c.count == 4 else { return nil }
        return c
    }
    /// Text's channels (LLP 1100 D2): the sRGB four, then for a colour in its
    /// own space its space (0 sRGB, 1 Display P3, 2 linear sRGB) and its four
    /// components, which `TextEngine.color` reads.
    func textChannels(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil) -> [Double]? {
        guard let c = channels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) else { return nil }
        guard case .object(let o) = self, let halves = o["cs"]?.array, !halves.isEmpty,
              case .object(let half) = halves[dark && halves.count > 1 ? 1 : 0] else { return c }
        if let code = ["srgb": 0.0, "display-p3": 1, "srgb-linear": 2][half["s"]?.string ?? ""],
           let v = half["v"]?.numbers, v.count == 4 { return c + [code] + v }
        // A profile's colour: as extended sRGB (LLP 1100 D3).
        guard let cg = cgColor(dark: dark), let space = CGColorSpace(name: CGColorSpace.extendedSRGB),
              let e = cg.converted(to: space, intent: .relativeColorimetric, options: nil)?.components, e.count == 4 else { return c }
        return c + [0] + e.map { Double($0) }
    }
    /// The colour as Core Graphics draws it: in its own space, unclipped
    /// (LLP 1100 D2), else from its sRGB channels.
    func cgColor(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil) -> CGColor? {
        if case .object(let o) = self, let halves = o["cs"]?.array, !halves.isEmpty,
           case .object(let half) = halves[dark && halves.count > 1 ? 1 : 0],
           let name = half["s"]?.string, let v = half["v"]?.numbers {
            // @ref LLP 1100 D3
            if let space = half["s"]?.profileSpace ?? ProfileSpaces.space(name) {
                guard v.count == space.numberOfComponents + 1 else { return nil }
                guard let color = CGColor(colorSpace: space, components: v.map { CGFloat($0) }) else { return nil }
                // CGColor carries no rendering intent. Apply the authored ICC
                // transform once, before the color enters any paint path.
                if name.hasPrefix("icc-sha256:"), let target = CGColorSpace(name: CGColorSpace.extendedSRGB) {
                    return color.converted(to: target, intent: ProfileSpaces.intent(half["i"]?.string), options: nil).map(ColorRange.tagged)
                }
                return ColorRange.tagged(color)
            }
            guard v.count == 4, let spaceName = Self.wideSpaces[name], let space = CGColorSpace(name: spaceName) else { return nil }
            return CGColor(colorSpace: space, components: v.map { CGFloat($0) }).map(ColorRange.tagged)
        }
        guard let c = channels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) else { return nil }
        return CGColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
    }
    /// The spaces a wide colour crosses the wire in, in their extended forms;
    /// Rust (`push_wide`) sends any other as extended linear sRGB.
    private static let wideSpaces: [String: CFString] = [
        "srgb": CGColorSpace.extendedSRGB, "display-p3": CGColorSpace.extendedDisplayP3,
        "srgb-linear": CGColorSpace.extendedLinearSRGB]
}

extension BatchOp {
    var flat: FlatPaint? {
        get { preparedPaint as? FlatPaint }
        set { preparedPaint = newValue }
    }
}

extension Batch {
    static func decode(_ data: Data, resolver: AssetResolver? = nil) -> Batch {
        data.withUnsafeBytes { decode($0.bindMemory(to: UInt8.self), resolver: resolver) }
    }
    static func decode(_ bytes: UnsafeBufferPointer<UInt8>, resolver: AssetResolver? = nil) -> Batch {
        decodeWire(bytes, resolver: resolver, colorChannels: { $0.textChannels(dark: $1) })
    }
}

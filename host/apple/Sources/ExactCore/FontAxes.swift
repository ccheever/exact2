// CSS `font-variation-settings` (LLP 1019 D9): the axes a run sets after its
// weight, width and optical size, each winning for its own axis.
import Foundation
import CoreText
import CExact

package struct FontAxis: Hashable {
    /// The OpenType tag, big-endian (`'wght'` is `0x77676874`).
    package let tag: UInt32
    /// In the axis's own units, as the kernel holds it.
    package let value: Float
    package init(tag: UInt32, value: Float) { self.tag = tag; self.value = value }
}

extension FontAxis {
    /// The axes the kernel's run carries.
    package static func of(_ run: ExactTextRun) -> [FontAxis] {
        var tags = run.variation_tags, values = run.variation_values
        let count = min(Int(run.variation_count), 8)
        guard count > 0 else { return [] }
        return withUnsafeBytes(of: &tags) { t in
            withUnsafeBytes(of: &values) { v in
                (0..<count).map { FontAxis(tag: t.load(fromByteOffset: $0 * 4, as: UInt32.self),
                                           value: v.load(fromByteOffset: $0 * 4, as: Float.self)) }
            }
        }
    }

    /// The axes a style row carries: `[["wght",650],…]`.
    package static func of(_ value: BatchValue?) -> [FontAxis] {
        (value?.array ?? []).compactMap { pair in
            guard let p = pair.array, p.count == 2, let tag = p[0].string, tag.utf8.count == 4, let v = p[1].number else { return nil }
            return FontAxis(tag: tag.utf8.reduce(0) { $0 << 8 | UInt32($1) }, value: Float(v))
        }
    }

    /// Text for a font cache key; empty for none.
    package static func key(_ axes: [FontAxis]) -> String {
        axes.map { "\($0.tag)=\($0.value)" }.joined(separator: ",")
    }
}

extension TextEngine {
    /// `font-stretch` and `font-variation-settings` on a platform face (San
    /// Francisco has `wdth` 30–150): its `wdth` is the stretch, as Chrome
    /// and WebKit set a variable face's, then each axis the face has that
    /// the settings name. Once a variation is given CoreText no longer sets
    /// `opsz` from the size, so the size is set where the face has it.
    package static func cssAxes(_ font: PlatformFont, stretch: CGFloat, variations: [FontAxis], size: CGFloat) -> PlatformFont {
        guard stretch != 100 || !variations.isEmpty else { return font }
        let has = Set((CTFontCopyVariationAxes(font as CTFont) as? [[CFString: Any]] ?? [])
            .compactMap { ($0[kCTFontVariationAxisIdentifierKey] as? NSNumber)?.uint32Value })
        var axes = CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber] ?? [:]
        if stretch != 100, has.contains(0x77647468) { axes[0x77647468] = stretch as NSNumber } // 'wdth'
        for v in variations where has.contains(v.tag) { axes[v.tag as NSNumber] = v.value as NSNumber }
        if has.contains(0x6f70737a), axes[0x6f70737a] == nil { axes[0x6f70737a] = size as NSNumber } // 'opsz'
        guard !axes.isEmpty else { return font }
        let d = CTFontDescriptorCreateWithAttributes([kCTFontVariationAttribute: axes] as CFDictionary)
        return CTFontCreateCopyWithAttributes(font as CTFont, size, nil, d) as PlatformFont
    }
}

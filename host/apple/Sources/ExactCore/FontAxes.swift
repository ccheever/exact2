// CSS `font-variation-settings` (LLP 1019 D9): the axes a run sets after its
// weight, width and optical size, each winning for its own axis. A run under
// `font-optical-sizing: none` carries `FontAxis.ownOpticalSize` first.
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
    static let opsz: UInt32 = 0x6f70737a
    /// `font-optical-sizing: none`: the face keeps its own `opsz` rather than
    /// the size (CSS Fonts 4 §2.7). Not an axis value; the drawing code reads it.
    package static let ownOpticalSize = FontAxis(tag: opsz, value: -.infinity)

    /// The axes the kernel's run carries.
    package static func of(_ run: ExactTextRun) -> [FontAxis] {
        var tags = run.variation_tags, values = run.variation_values
        let count = min(Int(run.variation_count), 8)
        let own = run.optical_sizing_none != 0 ? [ownOpticalSize] : []
        guard count > 0 else { return own }
        return own + withUnsafeBytes(of: &tags) { t in
            withUnsafeBytes(of: &values) { v in
                (0..<count).map { FontAxis(tag: t.load(fromByteOffset: $0 * 4, as: UInt32.self),
                                           value: v.load(fromByteOffset: $0 * 4, as: Float.self)) }
            }
        }
    }

    /// The axes a node's style rows carry: `font-variation-settings`, after
    /// `font-optical-sizing`'s marker.
    package static func of(_ style: NodeStyle) -> [FontAxis] {
        opticalSizing(style["font_optical_sizing"]?.string, of(style["font_variation_settings"]))
    }

    /// `axes` with `font-optical-sizing` set to `value` (`none`, or `auto`).
    package static func opticalSizing(_ value: String?, _ axes: [FontAxis]) -> [FontAxis] {
        let settings = axes.filter { $0 != ownOpticalSize }
        return value == "none" ? [ownOpticalSize] + settings : settings
    }

    /// The settings `value` names, under `axes`' optical sizing.
    package static func settings(_ value: BatchValue?, keeping axes: [FontAxis]) -> [FontAxis] {
        axes.first == ownOpticalSize ? [ownOpticalSize] + of(value) : of(value)
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
    /// `opsz` from the size, so the size is set where the face has it, unless
    /// `font-optical-sizing` is `none`, which turns CoreText's own off too.
    package static func cssAxes(_ font: PlatformFont, stretch: CGFloat, variations: [FontAxis], size: CGFloat) -> PlatformFont {
        guard stretch != 100 || !variations.isEmpty else { return font }
        let has = Set((CTFontCopyVariationAxes(font as CTFont) as? [[CFString: Any]] ?? [])
            .compactMap { ($0[kCTFontVariationAxisIdentifierKey] as? NSNumber)?.uint32Value })
        let own = variations.first == FontAxis.ownOpticalSize
        var axes = CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber] ?? [:]
        if own { axes[FontAxis.opsz as NSNumber] = nil }
        if stretch != 100, has.contains(0x77647468) { axes[0x77647468] = stretch as NSNumber } // 'wdth'
        for v in variations where has.contains(v.tag) && v != FontAxis.ownOpticalSize { axes[v.tag as NSNumber] = v.value as NSNumber }
        if !own, has.contains(FontAxis.opsz), axes[FontAxis.opsz as NSNumber] == nil { axes[FontAxis.opsz as NSNumber] = size as NSNumber }
        guard !axes.isEmpty || own else { return font }
        var attributes: [CFString: Any] = [kCTFontVariationAttribute: axes]
        if own { attributes[kCTFontOpticalSizeAttribute] = "none" }
        let d = CTFontDescriptorCreateWithAttributes(attributes as CFDictionary)
        return CTFontCreateCopyWithAttributes(font as CTFont, size, nil, d) as PlatformFont
    }
}

extension TextEngine {
    /// The OpenType features CSS `font-variant-ligatures`' bits set, 1 on and
    /// 0 off: common ligatures are `liga` and `clig`, discretionary `dlig`,
    /// historical `hlig`, contextual alternates `calt` (CSS Fonts 4). The bits
    /// are the kernel's `FontVariantLigatures`, in the schema's order: each
    /// kind's on bit, then its off bit. Core Text applies the tags to AAT faces
    /// too; a kind left unsaid keeps the face's default.
    package static func ligatureFeatures(_ bits: Int) -> [(String, Int)] {
        let kinds: [([String], Int)] = [(["liga", "clig"], 0), (["dlig"], 2), (["hlig"], 4), (["calt"], 6)]
        return kinds.flatMap { tags, shift -> [(String, Int)] in
            let on = bits >> shift & 1 != 0, off = bits >> (shift + 1) & 1 != 0
            guard on || off else { return [] }
            return tags.map { ($0, on ? 1 : 0) }
        }
    }
}

import CoreText
import Foundation

/// CSS extents size the line; the platform baseline positions its glyphs.
/// Aggregating these separately prevents one run's native ascent and another
/// run's adjusted descent from making a mixed-font line taller than Chrome's.
package struct TextLineBox: Sendable {
    package private(set) var above: CGFloat = 0
    package private(set) var below: CGFloat = 0
    package private(set) var baseline: CGFloat = 0
    private var aboveExplicit = false
    private var belowExplicit = false
    package var height: CGFloat { above + below }
    package var explicit: Bool { aboveExplicit || belowExplicit }

    package init() {}

    /// A box of these extents: an image in the text (LLP 1011.002).
    package init(above: CGFloat, below: CGFloat) {
        self.above = max(0, above); self.below = max(0, below); baseline = self.above
    }

    /// `scale`: the device pixels a point the box rounds to (`CSSLineBox`).
    package init(_ font: CTFont, height: CGFloat?, scale: CGFloat = 1) {
        (above, below) = CSSLineBox.extents(font, height: height, scale: scale)
        aboveExplicit = height != nil; belowExplicit = aboveExplicit
        #if canImport(UIKit)
        baseline = height == nil ? CTFontGetAscent(font) : above
        #else
        baseline = above
        #endif
    }

    package mutating func include(_ other: TextLineBox) {
        if other.above > above { above = other.above; aboveExplicit = other.aboveExplicit }
        else if other.above == above { aboveExplicit = aboveExplicit && other.aboveExplicit }
        if other.below > below { below = other.below; belowExplicit = other.belowExplicit }
        else if other.below == below { belowExplicit = belowExplicit && other.belowExplicit }
        baseline = max(baseline, other.baseline)
    }
}

/// CSS line heights use the web host's metrics and Chrome's rounding, which
/// is in device pixels: `scale` is the device pixels a point. The Mac passes
/// its window's backing scale (`TextEngine.lineScale`); headless sessions and
/// every other host pass 1, whole points (LLP 1008 §3).
/// UIKit positions glyphs at its face's ascent within that height.
/// On iOS PresentedText.measure returns nil; FieldChromeIOS keeps UIFont.lineHeight
/// for native editors. Reader line boxes do not change those control metrics.
package enum CSSLineBox {
    /// A text advance as the browser's layout holds it: rounded up to its
    /// 1/64 px layout unit, never to a whole point (a chip's width).
    package static func layoutWidth(_ width: CGFloat) -> CGFloat { ceil(width * 64) / 64 }

    /// A `normal` paragraph's height: its line boxes' sum up to a whole device pixel.
    package static func normalHeight(_ height: CGFloat, scale: CGFloat = 1) -> CGFloat {
        let scale = max(1, scale)
        return ceil(height * scale) / scale
    }

    /// The content area (an inline background's extent) above and below the baseline.
    package static func content(_ font: CTFont, scale: CGFloat = 1) -> (CGFloat, CGFloat) {
        let scale = max(1, scale)
        let (ascent, descent) = devicePixels(font, scale: scale)
        return (ascent / scale, descent / scale)
    }

    /// The font's ascent and descent rounded to whole device pixels, as
    /// Chrome rounds them (`SkScalarRoundToInt` of the zoomed font's).
    private static func devicePixels(_ font: CTFont, scale: CGFloat) -> (CGFloat, CGFloat) {
        #if canImport(UIKit)
        // Chrome system-ui uses SFNS's hhea metrics (1980/-432 at 2048 units/em).
        // SFUI paints the same UI family with different vertical metrics; keep
        // its glyphs, but use the web's line box for author-laid-out text.
        if (CTFontCopyPostScriptName(font) as String).hasPrefix(".SFUI-") {
            let size = CTFontGetSize(font)
            return ((size * 1980 / 2048 * scale).rounded(), (size * 432 / 2048 * scale).rounded())
        }
        #endif
        return ((CTFontGetAscent(font) * scale).rounded(), (CTFontGetDescent(font) * scale).rounded())
    }

    /// Above and below the baseline: `line-height: normal` when `height` is
    /// nil, else the authored line height, its half-leading floored to a
    /// device pixel.
    package static func extents(_ font: CTFont, height: CGFloat?, scale: CGFloat = 1) -> (CGFloat, CGFloat) {
        let scale = max(1, scale)
        let (ascent, descent) = devicePixels(font, scale: scale)
        guard let height else {
            let gap = (CTFontGetLeading(font) * scale).rounded()
            return ((ascent + floor(gap / 2)) / scale, (descent + ceil(gap / 2)) / scale)
        }
        guard height.isFinite else { return (ascent / scale, descent / scale) }
        let above = (ascent + floor((height * scale - ascent - descent) / 2)) / scale
        return (above, height - above)
    }
}

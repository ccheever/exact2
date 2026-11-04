// A field's `line-height` as TextKit lays out its lines.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import CoreText
import ExactCore

extension NSMutableParagraphStyle {
    /// Lines `height` apart, each with its baseline where CSS puts it: half
    /// the leading above the glyphs and half below (CSS 2 §10.8.1), as a
    /// text node's and an input's are (`CSSLineBox.extents`). TextKit puts
    /// all of a forced line's leading above its glyphs, the baseline the
    /// line's descent above its bottom. So the forced line ends that descent
    /// below the CSS baseline, and the rest of `height` is line spacing,
    /// which TextKit sets between one line and the next. A line shorter than
    /// that has no spacing to give: TextKit 2 applies a negative spacing
    /// between some paragraphs and not others, so such a line is forced to
    /// `height` and its glyphs sit up to a point higher than CSS's. Where
    /// CSS keeps the baseline by the field's font, TextKit raises a line
    /// holding a run of a larger descent (an emoji) by the difference. Nil,
    /// the font's own line height. `scale`: the device pixels a point the
    /// CSS line box rounds to (`CSSLineBox`).
    package func setLineHeight(_ height: CGFloat?, font: PlatformFont, scale: CGFloat = 1) {
        lineHeightMultiple = 0
        guard let height else { minimumLineHeight = 0; maximumLineHeight = 0; lineSpacing = 0; return }
        guard height > 0 else {
            // TextKit's zero minimum and maximum mean unconstrained; its
            // smallest positive multiple makes the explicit zero box.
            minimumLineHeight = 0; maximumLineHeight = 0; lineSpacing = 0
            lineHeightMultiple = .leastNormalMagnitude
            return
        }
        #if canImport(UIKit)
        let descent = CTFontGetDescent(font as CTFont) // UIKit's TextKit takes it as it is
        #else
        let descent = CTFontGetDescent(font as CTFont).rounded() // AppKit's rounds it to a point
        #endif
        let line = min(height, CSSLineBox.extents(font as CTFont, height: height, scale: scale).0 + descent)
        minimumLineHeight = line
        maximumLineHeight = line
        lineSpacing = height - line
    }
}

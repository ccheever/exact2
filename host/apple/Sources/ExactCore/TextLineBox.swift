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

    package init(_ font: CTFont, height: CGFloat?) {
        (above, below) = CSSLineBox.extents(font, height: height)
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

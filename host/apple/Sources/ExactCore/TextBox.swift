// CSS `text-box` on CoreText (Inline Layout 3 §4): what a trim cuts off a
// laid-out paragraph, from the fonts its first and last lines are set in.
import CoreText
import Foundation

extension TextEngine {
    /// A laid-out paragraph trimmed in place: its baselines and line bottoms
    /// move up by the top's cut, the last bottom ends at the trimmed height
    /// (a column of it is that tall), and the height loses both cuts. The
    /// measurer and the painter both see this paragraph. `firstTop` is
    /// where the first line's box starts: a line flow pushed below an
    /// obstacle loses only its own space above the edge, never the
    /// clearance. Answers the top's cut, which a flow's fragments move by.
    @discardableResult
    package func trimTextBox(_ spec: Spec, lines: [CTLine], baselines: inout [CGFloat], lineBottoms: inout [CGFloat], height: inout CGFloat, firstTop: CGFloat = 0) -> CGFloat {
        guard spec.textBoxTrim != 0, !lines.isEmpty, let root = spec.strut ?? spec.runs.first else { return 0 }
        let (top, bottom) = Self.textBoxTrim((spec.textBoxTrim, spec.textBoxOver, spec.textBoxUnder), root: font(root) as CTFont,
                                             first: baselines[0] - firstTop, last: baselines[baselines.count - 1] - firstTop, height: height - firstTop)
        Self.trimLines((top, bottom), baselines: &baselines, lineBottoms: &lineBottoms, height: &height)
        return top
    }
    /// Lines cut by `(top, bottom)`: baselines and bottoms move up by the
    /// top's cut, the height loses both, and the last bottom is the height.
    package static func trimLines(_ cut: (top: CGFloat, bottom: CGFloat), baselines: inout [CGFloat], lineBottoms: inout [CGFloat], height: inout CGFloat) {
        baselines = baselines.map { $0 - cut.top }
        height = max(0, height - cut.top - cut.bottom)
        lineBottoms = lineBottoms.map { min($0 - cut.top, height) }
        if !lineBottoms.isEmpty { lineBottoms[lineBottoms.count - 1] = height }
    }
    /// How much `text-box-trim` cuts off a paragraph's top and bottom: from
    /// the first line's baseline up to the root inline box's ascent, cap
    /// height or x-height, and from the last line's baseline down to its
    /// descent or nothing (Inline Layout 3 §4.2). The root inline box is the
    /// paragraph's own font, never an inline run's or a fallback's: a 10 pt
    /// span alone in a 20 pt paragraph is trimmed to the 20 pt cap height,
    /// as CSS trims it. CoreText reads the metrics, so a future face or OS
    /// answers for itself.
    /// `box` is `Spec`'s trim, over and under; `first` and `last` the first
    /// and last lines' baselines.
    package static func textBoxTrim(_ box: (trim: Int, over: Int, under: Int), root: CTFont, first: CGFloat, last: CGFloat, height: CGFloat) -> (CGFloat, CGFloat) {
        var top: CGFloat = 0, bottom: CGFloat = 0
        if box.trim == 1 || box.trim == 3 {
            let over: CGFloat = switch box.over {
            case 1: CTFontGetCapHeight(root)
            case 2: CTFontGetXHeight(root)
            default: CTFontGetAscent(root)
            }
            top = first - over
        }
        if box.trim == 2 || box.trim == 3 {
            bottom = height - (last + (box.under == 1 ? 0 : CTFontGetDescent(root)))
        }
        return (top.isFinite ? top : 0, bottom.isFinite ? bottom : 0)
    }
}

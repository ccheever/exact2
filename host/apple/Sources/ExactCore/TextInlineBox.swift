// An image in a paragraph's text (LLP 1011.002): its run is U+FFFC with a
// CTRunDelegate the box's size, so CoreText breaks lines around it and leaves
// it no ink; the line box grows to hold it (CSS 2 §10.8), and the kernel
// places the image's own view where the measurer says the box went.
import Foundation
import CoreText
import CExact

/// The box a run reserves: the image's margin box, and its `vertical-align`.
package struct InlineBox: Hashable, Sendable {
    package var width: CGFloat
    package var height: CGFloat
    /// 0 baseline, 1 middle, 2 text-top, 3 text-bottom.
    package var align: Int
    /// The length it is raised by.
    package var shift: CGFloat

    package init(width: CGFloat, height: CGFloat, align: Int, shift: CGFloat) {
        self.width = width; self.height = height; self.align = align; self.shift = shift
    }

    /// A kernel request's run, when it is a box.
    package init?(_ run: ExactTextRun) {
        guard run.inline_box != 0 else { return nil }
        self.init(width: CGFloat(run.box_width), height: CGFloat(run.box_height),
                  align: Int(run.box_align), shift: CGFloat(run.box_shift))
    }

    /// The paragraph op's `inlineBox` prop: width, height, align and shift.
    package init?(prop: String?) {
        // The kernel's f32s, spelled to round-trip.
        guard let parts = prop?.split(separator: " ").compactMap({ Float($0) }), parts.count == 4 else { return nil }
        self.init(width: CGFloat(parts[0]), height: CGFloat(parts[1]), align: Int(parts[2]), shift: CGFloat(parts[3]))
    }

    /// How far its top stands above the baseline among text in `font`
    /// (the kernel's `InlineBox::ascent`): its descent is the rest.
    package func ascent(_ font: CTFont) -> CGFloat {
        let (above, below) = CSSLineBox.content(font)
        let top: CGFloat
        switch align {
        case 1: top = (height + CTFontGetXHeight(font)) / 2
        case 2: top = above
        case 3: top = height - below
        default: top = height
        }
        return top + shift
    }

    /// Its line box: what it adds above and below the baseline.
    package func lineBox(_ font: CTFont) -> TextLineBox {
        let above = ascent(font)
        return TextLineBox(above: above, below: height - above)
    }
}

private final class InlineBoxMetrics {
    let ascent: CGFloat, descent: CGFloat, width: CGFloat
    init(ascent: CGFloat, descent: CGFloat, width: CGFloat) {
        self.ascent = ascent; self.descent = descent; self.width = width
    }
    static func of(_ p: UnsafeMutableRawPointer) -> InlineBoxMetrics { Unmanaged.fromOpaque(p).takeUnretainedValue() }
}

package extension NSAttributedString.Key {
    /// An inline box's run, its index in the spec's runs (LLP 1011.002).
    static let exactInlineBox = NSAttributedString.Key("ExactInlineBox")
}

extension TextEngine {
    /// The attributes that make a box's U+FFFC the box: CoreText's run
    /// delegate gives it the box's width, ascent and descent.
    package static func boxAttributes(_ box: InlineBox, font: CTFont, index: Int, into a: inout [NSAttributedString.Key: Any]) {
        let ascent = box.ascent(font)
        let metrics = InlineBoxMetrics(ascent: max(0, ascent), descent: max(0, box.height - ascent), width: max(0, box.width))
        var callbacks = CTRunDelegateCallbacks(
            version: kCTRunDelegateVersion1,
            dealloc: { Unmanaged<InlineBoxMetrics>.fromOpaque($0).release() },
            getAscent: { InlineBoxMetrics.of($0).ascent },
            getDescent: { InlineBoxMetrics.of($0).descent },
            getWidth: { InlineBoxMetrics.of($0).width })
        if let delegate = CTRunDelegateCreate(&callbacks, Unmanaged.passRetained(metrics).toOpaque()) {
            a[NSAttributedString.Key(kCTRunDelegateAttributeName as String)] = delegate
        }
        a[.exactInlineBox] = index
    }

    /// @ref LLP 1011.002 D2 — each box's top left in content coordinates, in
    /// run order, in the paragraph `request` answers: the one painted.
    package func inlineBoxOrigins(_ request: ExactMeasureRequest) -> [CGFloat] {
        guard request.width >= 0, request.exclusion_count == 0, request.markup == 0 else { return [] }
        let spec = requestSpec(request)
        guard spec.runs.contains(where: { $0.box != nil }) else { return [] }
        let width = CGFloat(request.width)
        let p = paragraph(spec, width: width)
        var placed: [(Int, CGFloat, CGFloat)] = []
        for (i, line) in p.lines.enumerated() where p.baselines.indices.contains(i) {
            let x = p.origin(i, align: spec.align, width: width)
            for run in CTLineGetGlyphRuns(line) as! [CTRun] {
                let attributes = CTRunGetAttributes(run) as NSDictionary
                guard let index = attributes[NSAttributedString.Key.exactInlineBox] as? Int,
                      spec.runs.indices.contains(index), let box = spec.runs[index].box,
                      CTRunGetGlyphCount(run) > 0 else { continue }
                var position = CGPoint.zero
                CTRunGetPositions(run, CFRangeMake(0, 1), &position)
                placed.append((index, x + position.x, p.baselines[i] - box.ascent(font(spec.runs[index]) as CTFont)))
            }
        }
        // One pair per box in run order; a box on no line (`line-clamp`
        // cut it, or an ellipsis took it) is not a number.
        let at = Dictionary(placed.map { ($0.0, ($0.1, $0.2)) }, uniquingKeysWith: { a, _ in a })
        return spec.runs.indices.filter { spec.runs[$0].box != nil }.flatMap { i in
            at[i].map { [$0.0, $0.1] } ?? [.nan, .nan]
        }
    }

    /// The kernel's inline box hook (`exact_set_inline_boxes`); `ctx` is the engine.
    package static let inlineBoxesText: ExactLinesFn = { ctx, request, out, cap in
        guard let ctx, let request = request?.pointee else { return 0 }
        let origins = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue().inlineBoxOrigins(request)
        if let out { for (i, v) in origins.prefix(cap).enumerated() { out[i] = Float(v) } }
        return origins.count
    }
}

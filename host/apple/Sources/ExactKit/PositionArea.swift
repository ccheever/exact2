// LLP 1021 D2, §5: where a painted popover sits against the invoker that
// opened it — CSS `position-area` with the invoker as its implicit anchor, in
// the subset the contract admits — the same on iOS's and macOS's top layers,
// and the point macOS pops a chooser's NSMenu up at.
import CoreGraphics

enum PositionArea {
    /// A box's resolved margins, in points.
    struct Margins: Equatable {
        var top: CGFloat = 0, right: CGFloat = 0, bottom: CGFloat = 0, left: CGFloat = 0
    }

    /// The row's value on a popover (`none` when it has none).
    static func of(_ popover: NodeView) -> String { popover.style["position_area"]?.string ?? "none" }
    /// The popover's margins as the kernel resolved them: points; `auto`
    /// (and a percentage, which the host never sees resolved) is 0, as CSS
    /// resolves an auto margin on a box placed by `position-area`.
    static func margins(of popover: NodeView) -> Margins {
        Margins(top: popover.number("margin_top"), right: popover.number("margin_right"),
                bottom: popover.number("margin_bottom"), left: popover.number("margin_left"))
    }

    /// Whether `area` centres the box on the invoker horizontally: CSS's
    /// `anchor-center` self-alignment, which a single-keyword area (`top` is
    /// `top span-all`), an area spanning all three columns, and `center` take.
    static func centred(_ area: String) -> Bool {
        area == "top" || area == "bottom" || area == "center" || area.hasSuffix("span-all")
    }

    /// The top-left of a border box of `size` placed by `area` against
    /// `anchor`, in a layer of `bounds`. CSS aligns the margin box, so
    /// `margins` widen the box being placed and the border box sits inside
    /// them. `none` (and `bottom span-right`) is D2's rule: the margin box's
    /// top-left at the invoker's bottom-left. A `top` area puts its bottom at
    /// the invoker's top; `center` centres it over the invoker; `right
    /// span-bottom` (a submenu, §5) puts its top-left at the invoker's
    /// top-right. Then the margin box is clamped to the layer, as CSS shifts
    /// an absolutely positioned box that overflows its area back into its
    /// containing block; never flipped (a flip is `position-try`, not admitted).
    static func origin(_ area: String, anchor: CGRect, size: CGSize, margins: Margins = .init(), in bounds: CGRect) -> CGPoint {
        let outer = CGSize(width: size.width + margins.left + margins.right, height: size.height + margins.top + margins.bottom)
        let beside = area == "right span-bottom"
        let x = beside ? anchor.maxX : centred(area) ? anchor.midX - outer.width / 2 : anchor.minX
        let y = beside ? anchor.minY : area == "center" ? anchor.midY - outer.height / 2
            : area.hasPrefix("top") ? anchor.minY - outer.height : anchor.maxY
        return CGPoint(x: max(bounds.minX, min(x, bounds.maxX - outer.width)) + margins.left,
                       y: max(bounds.minY, min(y, bounds.maxY - outer.height)) + margins.top)
    }
}

// LLP 1021 D2, §5: where a painted popover sits against the invoker that
// opened it — CSS `position-area` with the invoker as its implicit anchor, in
// the subset the contract admits — the same on iOS's and macOS's top layers.
import CoreGraphics

enum PositionArea {
    /// The row's value on a popover (`none` when it has none).
    static func of(_ popover: NodeView) -> String { popover.style["position_area"]?.string ?? "none" }

    /// Whether `area` centres the box on the invoker horizontally: CSS's
    /// `anchor-center` self-alignment, which a single-keyword area (`top` is
    /// `top span-all`), an area spanning all three columns, and `center` take.
    static func centred(_ area: String) -> Bool {
        area == "top" || area == "bottom" || area == "center" || area.hasSuffix("span-all")
    }

    /// The top-left of a box of `size` placed by `area` against `anchor`, in
    /// a layer of `bounds`. `none` (and `bottom span-right`) is D2's rule:
    /// top-left at the invoker's bottom-left. A `top` area puts the box's
    /// bottom at the invoker's top; `center` centres it over the invoker.
    /// Then clamped to the layer, as CSS shifts an absolutely positioned box
    /// that overflows its area back into its containing block; never flipped
    /// (a flip is `position-try`, not admitted).
    static func origin(_ area: String, anchor: CGRect, size: CGSize, in bounds: CGRect) -> CGPoint {
        let x = centred(area) ? anchor.midX - size.width / 2 : anchor.minX
        let y = area == "center" ? anchor.midY - size.height / 2
            : area.hasPrefix("top") ? anchor.minY - size.height : anchor.maxY
        return CGPoint(x: max(bounds.minX, min(x, bounds.maxX - size.width)),
                       y: max(bounds.minY, min(y, bounds.maxY - size.height)))
    }
}

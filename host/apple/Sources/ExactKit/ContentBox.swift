// Every box with padding or a border is sent its CSS content box and the
// padding layout resolved (`fieldContent`; a percentage is of the
// containing block, CSS Box Model 3 §4.1). That applied geometry is the one
// source for what is placed in the box, for its insets and for pointer
// offsets: it is held with the frame beneath a sheet and through a failed
// layout, where a style row is not. A box sent none has no inset.
import CoreGraphics
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    /// Native fields keep their published editor box for field layout (LLP
    /// 1104 D5) and read their padding rows. Other content uses the published
    /// box, or the bounds of a box sent none.
    func contentBox() -> CGRect {
        if !isNativeTextControl {
            return kernelContentBox?.offsetBy(dx: bounds.minX, dy: bounds.minY) ?? bounds
        }
        let uniform = number("border_width")
        return bounds.insetBy(
            left: number("border_width_left", uniform) + number("padding_left"),
            top: number("border_width_top", uniform) + number("padding_top"),
            right: number("border_width_right", uniform) + number("padding_right"),
            bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
    }

    /// The padding a `fieldContent` payload sends, top, right, bottom, left:
    /// `nil` on a native button's side its author left unsaid.
    static func sentPadding(_ payload: [String: Any]) -> [CGFloat?]? {
        guard let sides = payload["padding"] as? [Any], sides.count == 4 else { return nil }
        return sides.map { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } }
    }

    /// A content-box update can move content without changing the node's frame.
    func applyKernelContentBox(_ box: CGRect?, padding: [CGFloat?]?) {
        kernelContentBox = box; kernelPadding = padding
        layoutField(); layoutTextArea()
        #if os(iOS) || os(tvOS)
        setNeedsLayout(); setNeedsDisplay()
        #else
        layerPaintCache = nil
        needsLayout = true; needsDisplay = true
        #endif
    }

    /// Places what draws in the content box at it, as a video is
    /// (`VideoModule.layout`): an `svg`'s scene, or a 2D canvas's bitmap,
    /// clipped to the content edge's curve as the web clips replaced content
    /// (CSS Backgrounds 3 §4.2, §4.3). Laid out with the view, it waits with
    /// its frame and content box wherever those wait.
    func layoutContent() {
        guard kind == "svg" || kind == "canvas2d", let presenter else { return }
        let box = contentBox()
        if kind == "svg" { return presenter.svg.place(id, in: box) }
        let radii = BorderPaint.contentRadii(cornerSizes(in: bounds), outer: bounds, inner: box)
        presenter.canvas2d.place(id, in: box, radii: radii)
    }

    /// The padding on `side` ("top", "right", "bottom" or "left") in points,
    /// as layout resolved and sent it.
    package func padding(_ side: String) -> CGFloat {
        if isNativeTextControl { return number("padding_" + side) }
        return writtenPadding(side) ?? 0
    }

    /// The padding on `side` as layout sent it, `nil` where none was sent or
    /// a native button's author left the side unsaid.
    func writtenPadding(_ side: String) -> CGFloat? {
        guard let p = kernelPadding else { return nil }
        switch side {
        case "top": return p[0]
        case "right": return p[1]
        case "bottom": return p[2]
        default: return p[3]
        }
    }
}

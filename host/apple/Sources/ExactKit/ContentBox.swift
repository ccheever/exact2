// Layout resolves percentage padding against the containing block, not this
// view. Use its published content box for both painting and pointer offsets.
import CoreGraphics
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    /// Native fields keep their published editor box for field layout (LLP 1104 D5).
    /// Other content uses the CSS box inside the resolved border and padding.
    func contentBox() -> CGRect {
        if !isNativeTextControl, let box = kernelContentBox {
            return box.offsetBy(dx: bounds.minX, dy: bounds.minY)
        }
        let uniform = number("border_width")
        return bounds.insetBy(
            left: number("border_width_left", uniform) + number("padding_left"),
            top: number("border_width_top", uniform) + number("padding_top"),
            right: number("border_width_right", uniform) + number("padding_right"),
            bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
    }

    /// A content-box update can move content without changing the node's frame.
    func applyKernelContentBox(_ box: CGRect?) {
        kernelContentBox = box
        layoutField(); layoutTextArea()
        if kind == "canvas2d" { presenter?.canvas2d.move(id, to: contentBox().origin) }
        #if os(iOS) || os(tvOS)
        setNeedsLayout(); setNeedsDisplay()
        #else
        layerPaintCache = nil
        needsLayout = true; needsDisplay = true
        #endif
    }

    /// Resolved padding in points, excluding the border.
    package func padding(_ side: String) -> CGFloat {
        guard !isNativeTextControl, let box = kernelContentBox else { return number("padding_" + side) }
        let border = number("border_width_" + side, number("border_width"))
        switch side {
        case "top": return max(0, box.minY - border)
        case "left": return max(0, box.minX - border)
        case "bottom": return max(0, bounds.height - box.maxY - border)
        default: return max(0, bounds.width - box.maxX - border)
        }
    }
}

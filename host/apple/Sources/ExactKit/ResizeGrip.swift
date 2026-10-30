// CSS `resize` on a textarea (CSS UI 4 §5.1): the grip in the padding box's
// bottom end corner that a person drags to size the box. Neither AppKit nor
// UIKit has one, so it is drawn as Chrome draws it, two diagonal strokes
// (measured at 1x: a 7 px and a 3 px stroke, 2 px in from the corner, in a
// mid grey that `secondaryLabel` is on a light ground); under `rtl` it is
// the bottom left corner. A drag hands the border box the pointer reached to
// the runner (`Runner::resize_box`) once a display frame at most, and the
// runner writes `width` and `height` as Chrome's inline style does. On the
// Mac the textarea's vertical scroller ends above the grip, as Chrome's
// scrollbar ends above its scroll corner. Each platform's view is
// `Mac/ResizeGripMac.swift` and `IOS/ResizeGripIOS.swift`; tvOS's textarea
// is read-only and has none.
import CoreGraphics
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

enum ResizeGripShape {
    /// The square at the corner a grip takes the pointer in: the 15 pt of
    /// Chrome's scroll corner.
    static let side: CGFloat = 15

    /// The two strokes in a grip of `side` whose corner is bottom right, or
    /// bottom left under `rtl`, in a flipped space.
    static func strokes(rtl: Bool) -> [(CGPoint, CGPoint)] {
        let x = { (inset: CGFloat) in rtl ? inset : side - inset }
        let y = { (inset: CGFloat) in side - inset }
        return [(2, 9), (2, 5)].map { near, far in
            (CGPoint(x: x(near), y: y(far)), CGPoint(x: x(far), y: y(near)))
        }
    }

    /// The border box a drag of the grip asks for, or nil when `resize`
    /// gives it no axis; nil keeps an axis (`Runner::resize_box`).
    /// `pointer` is where the drag is now in the box, `start` where it went
    /// down, `size` the border box then, and `movedX` how far it has moved
    /// across the box's parent. The corner follows the pointer from the
    /// box's top left, as Chrome measures it, so a box that moves as it grows
    /// (centred in its parent) reaches the pointer over a few moves. Under
    /// `rtl` the width grows by the pointer's move to the left: measured from
    /// the box's own edge it would grow without end where that edge stays put.
    static func dragged(axes: (width: Bool, height: Bool), rtl: Bool, pointer: CGPoint, start: CGPoint, size: CGSize,
                        movedX: CGFloat) -> (width: CGFloat?, height: CGFloat?)? {
        guard axes.width || axes.height else { return nil }
        let width = axes.width ? max(0, rtl ? size.width - movedX : pointer.x + size.width - start.x) : nil
        let height = axes.height ? max(0, pointer.y + size.height - start.y) : nil
        return (width, height)
    }
}

extension NodeView {
    /// The axes the computed `resize` lets a person drag: (width, height).
    /// Only a textarea has a grip on a native host.
    var resizeAxes: (width: Bool, height: Bool) {
        #if os(tvOS)
        return (false, false)
        #else
        guard textArea != nil else { return (false, false) }
        switch style["resize"]?.string {
        case "both": return (true, true)
        case "horizontal", "inline": return (true, false)
        case "vertical", "block": return (false, true)
        default: return (false, false)
        }
        #endif
    }

    var resizeGripRTL: Bool { (style["direction"]?.string ?? presenter?.documentDirection ?? "ltr") == "rtl" }

    /// The grip's frame: the padding box's bottom end corner.
    var resizeGripFrame: CGRect {
        let uniform = number("border_width")
        let padding = layoutBounds.insetBy(
            left: number("border_width_left", uniform), top: number("border_width_top", uniform),
            right: number("border_width_right", uniform), bottom: number("border_width_bottom", uniform))
        let side = ResizeGripShape.side
        let x = resizeGripRTL ? padding.minX : padding.maxX - side
        return CGRect(x: x, y: padding.maxY - side, width: side, height: side)
    }

    /// The border box a drag of this box's grip asks for (`ResizeGripShape.dragged`).
    func resizeGripSize(pointer: CGPoint, start: CGPoint, size: CGSize, movedX: CGFloat) -> (width: CGFloat?, height: CGFloat?)? {
        ResizeGripShape.dragged(axes: resizeAxes, rtl: resizeGripRTL, pointer: pointer, start: start, size: size, movedX: movedX)
    }

    /// Hands the runner the border box a drag reached, under one layout.
    func resizeGripSend(_ size: (width: CGFloat?, height: CGFloat?)) {
        presenter?.onResizeBox?(id, size.width, size.height)
    }
}

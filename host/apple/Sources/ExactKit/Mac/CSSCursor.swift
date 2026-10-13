#if os(macOS)
import AppKit

/// CSS cursor keywords mapped to AppKit's available cursor shapes.
enum CSSCursor {
    private static let invisible = NSCursor(image: NSImage(size: NSSize(width: 1, height: 1)), hotSpot: .zero)
    static func value(_ name: String) -> NSCursor? {
        if #available(macOS 15, *), let resize = frameResize(name) { return resize }
        switch name {
        case "auto": return nil
        case "none": return invisible
        case "pointer": return .pointingHand
        case "grab": return .openHand
        case "grabbing": return .closedHand
        case "text": return .iBeam
        case "vertical-text": return .iBeamCursorForVerticalLayout
        case "crosshair", "cell": return .crosshair
        case "context-menu": return .contextualMenu
        case "copy": return .dragCopy
        case "alias": return .dragLink
        case "no-drop", "not-allowed": return .operationNotAllowed
        // macOS 14 has no frame-resize cursors (`frameResize`): its older arrows.
        case "e-resize": return .resizeRight
        case "w-resize": return .resizeLeft
        case "n-resize": return .resizeUp
        case "s-resize": return .resizeDown
        case "ew-resize", "col-resize": return .resizeLeftRight
        case "ns-resize", "row-resize": return .resizeUpDown
        case "move", "all-scroll": return .openHand
        case "zoom-in":
            if #available(macOS 15, *) { return .zoomIn }
            return .crosshair
        case "zoom-out":
            if #available(macOS 15, *) { return .zoomOut }
            return .crosshair
        // AppKit 14 has no diagonal resize, help, wait or progress shape.
        // CSS lets the platform choose the cursor artwork; use a visible stand-in.
        case "ne-resize", "nw-resize", "se-resize", "sw-resize", "nesw-resize", "nwse-resize": return .crosshair
        default: return .arrow
        }
    }

    /// The resize keywords as Chrome and Safari draw them on macOS 15
    /// (Chromium's `ui/base/cocoa/cursor_utils.mm`): an edge or a corner
    /// moves outward, a two-way keyword both ways, and a divider is a column
    /// or a row.
    @available(macOS 15, *)
    private static func frameResize(_ name: String) -> NSCursor? {
        let position: NSCursor.FrameResizePosition
        switch name {
        case "col-resize": return .columnResize
        case "row-resize": return .rowResize
        case "n-resize", "ns-resize": position = .top
        case "s-resize": position = .bottom
        case "e-resize": position = .right
        case "w-resize", "ew-resize": position = .left
        case "ne-resize", "nesw-resize": position = .topRight
        case "nw-resize", "nwse-resize": position = .topLeft
        case "se-resize": position = .bottomRight
        case "sw-resize": position = .bottomLeft
        default: return nil
        }
        let twoWay = ["ns-resize", "ew-resize", "nesw-resize", "nwse-resize"].contains(name)
        return .frameResize(position: position, directions: twoWay ? .all : .outward)
    }
}
#endif

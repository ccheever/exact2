#if os(macOS)
import AppKit

/// CSS cursor keywords mapped to AppKit's available cursor shapes.
enum CSSCursor {
    private static let invisible = NSCursor(image: NSImage(size: NSSize(width: 1, height: 1)), hotSpot: .zero)
    static func value(_ name: String) -> NSCursor? {
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
}
#endif

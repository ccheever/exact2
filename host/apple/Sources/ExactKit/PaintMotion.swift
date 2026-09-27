// Paint motion (LLP 1062): a colour or shadow the engine is moving, painted
// over the style row it will arrive at. `channels` reads `paint` first, so
// every painter already in the view — the box layer, the border, the shadow
// caster, the paragraph's runs, a symbol's tint — paints the presented value;
// this only says which of them to ask again. `nil` hands the row back.
#if os(iOS)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    func present(paint key: String, _ value: [Double]?) {
        guard paint[key] != value else { return }
        paint[key] = value
        switch key {
        case "text_color":
            // The paragraph's pixels carry their colour: a new raster.
            invalidateText()
            field?.textColor = color("text_color", .black)
            #if os(iOS)
            setNeedsDisplay()
            #else
            needsDisplay = true
            #endif
        case "tint_color":
            #if os(iOS)
            symbolView?.tintColor = color("tint_color", .black)
            #else
            symbolView?.contentTintColor = color("tint_color", .black)
            #endif
        default:
            #if os(iOS)
            // Layer properties, no bitmap, unless `draw(_:)` owns the box.
            applyBoxLayer()
            if boxDrawn { setNeedsDisplay() }
            #else
            applyShadow()
            needsDisplay = true
            #endif
        }
    }
}

// Paint motion (LLP 1062): a colour or shadow the engine is moving, painted
// over the style row it will arrive at. `channels` reads `paint` first, so
// every painter already in the view — the box layer, the border, the shadow
// caster, the paragraph's runs, a symbol's tint — paints the presented value;
// this only says which of them to ask again. `nil` hands the row back. An
// inline run's colour is its paragraph's to paint, under `text_color#<run>`.
#if os(iOS)
import UIKit
#else
import AppKit
#endif

extension BatchOp {
    /// The paint key a `present` sets: its property, or an inline run's
    /// colour, which its paragraph keeps as `text_color#<run>`.
    var paintKey: String { run.map { "\(property)#\($0)" } ?? property }
}

extension NodeView {
    func present(paint key: String, _ value: [Double]?) {
        guard paint[key] != value else { return }
        paint[key] = value
        switch key {
        case _ where key.hasPrefix("text_color"):
            // A path's `currentcolor` paint follows it (LLP 1065).
            PathView.of(self)?.restyle()
            // The paragraph's pixels carry their colour: a new raster, painted
            // in this frame, not a worker's next (LLP 1062 D6).
            invalidateText()
            field?.textColor = color("text_color", .black)
            #if os(iOS)
            presenter?.presentedText.insert(id)
            setNeedsDisplay()
            #else
            needsDisplay = true
            #endif
        case "fill", "stroke":
            // A path's paint (LLP 1065): its layers, not the box.
            PathView.of(self)?.restyle()
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

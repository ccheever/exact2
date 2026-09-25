// A node's box on UIKit is its layer's properties where Core Animation can
// say it — the background, one corner radius over a mask of corners, one
// border — and a node with nothing else to paint keeps no bitmap. UIKit gives
// every view whose class overrides `draw(_:)` a backing store of its bounds
// times the scale squared, painted or not: a collection's spacer asked for
// gigabytes, and each card of a list carried its own. What Core Animation
// cannot say still draws, through `draw(_:)` as before: borders that differ
// by side, radii that differ by corner, an image, a paragraph without a
// raster, a capture's picture. (The macOS host's `wantsUpdateLayer` split.)
#if os(iOS)
import UIKit

/// Every node view's layer. `display` decides whether UIKit allocates a
/// backing store and calls `draw(_:)` at all.
final class NodeLayer: CALayer {
    override func display() {
        guard let node = delegate as? NodeView else { super.display(); return }
        node.applyBoxLayer()
        if node.drawsPaint { super.display(); return }
        contents = nil
        if node.isParagraph {
            // Text's pixels are the raster layer's; this one keeps none.
            node.presenter?.textRasters.ensure(node, urgent: node.presenter?.textIsVisible(node) == true)
            if node.textRasterFailed { super.display(); return }
        }
        node.repaintThrough()
        if node.presenter?.views[node.id] === node { node.firstDraw() }
    }
}

extension NodeView {
    /// Paint only `draw(_:)` makes: a box Core Animation cannot say, an
    /// image's pixels, a capture's picture, a paragraph with no raster.
    var drawsPaint: Bool {
        if boxDrawn || (kind == "image" && symbolView == nil && raster != nil) { return true }
        if Capture.capturing && (kind == "canvas" || Capture.web[id] != nil) { return true }
        return isParagraph && !canRasterText
    }

    /// The box onto the layer, or `boxDrawn` when `draw(_:)` must paint it.
    /// The web's box: background and border inside the border box, a
    /// uniform border following the curve, the radius clipping children only
    /// where the overflow clips.
    func applyBoxLayer() {
        let background = nativeSwipeBody ? nil : channels("background_color").map { TextEngine.color($0).cgColor }
        let fill = background.flatMap { $0.alpha > 0 ? $0 : nil }
        let uniform = number("border_width")
        let sides = ["top", "right", "bottom", "left"]
        let widths = sides.map { number("border_width_" + $0, uniform) }
        let top = color("border_color_top", .clear)
        let colors = sides.map { color("border_color_" + $0, top).cgColor }
        let width = widths[0]
        let oneBorder = widths.allSatisfy { $0 == width } && (width == 0 || colors.allSatisfy { $0 == colors[0] })
        // One radius over the corners that have one; CSS's reduction first,
        // and Core Animation's own limit (half the shorter side) not reached.
        let radii = cornerRadii(in: bounds)
        let radius = radii.max() ?? 0
        let oneRadius = radii.allSatisfy { $0 == 0 || abs($0 - radius) < 0.01 }
            && radius <= min(bounds.width, bounds.height) / 2 + 0.01
        boxDrawn = !(oneBorder && oneRadius) && (fill != nil || widths.contains { $0 > 0 })
        let onLayer = !boxDrawn
        var corners: CACornerMask = []
        let masks: [CACornerMask] = [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMaxXMaxYCorner, .layerMinXMaxYCorner]
        for (r, mask) in zip(radii, masks) where r > 0 { corners.insert(mask) }
        let cornerRadius = onLayer && oneRadius ? radius : 0
        let border = onLayer && width > 0 ? colors[0] : nil
        // A border stays under the children, as the web paints it, unless
        // none can reach it: they are clipped, scrolled, or painted through a
        // surface. Then it is the layer's own, which Core Animation paints
        // over the sublayers.
        let own = clipsToBounds || scroll != nil || overlay != nil
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let bg = onLayer ? fill : nil
        if layer.backgroundColor != bg { layer.backgroundColor = bg }
        if layer.cornerRadius != cornerRadius { layer.cornerRadius = cornerRadius }
        if cornerRadius > 0, layer.maskedCorners != corners { layer.maskedCorners = corners }
        let ownWidth = own && border != nil ? width : 0
        if layer.borderWidth != ownWidth { layer.borderWidth = ownWidth }
        if ownWidth > 0, layer.borderColor != border { layer.borderColor = border }
        guard let border, !own else { boxBorder?.removeFromSuperlayer(); boxBorder = nil; return }
        let b = boxBorder ?? CALayer()
        if b.superlayer !== layer { layer.insertSublayer(b, at: 0); boxBorder = b }
        if b.frame != bounds { b.frame = bounds }
        if b.cornerRadius != cornerRadius { b.cornerRadius = cornerRadius }
        if b.maskedCorners != layer.maskedCorners { b.maskedCorners = layer.maskedCorners }
        if b.borderWidth != width { b.borderWidth = width }
        if b.borderColor != border { b.borderColor = border }
    }
}
#endif

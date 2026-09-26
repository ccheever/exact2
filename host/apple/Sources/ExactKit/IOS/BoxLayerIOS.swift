// A node's box on UIKit is its layer's properties where Core Animation can
// say it — the background, one corner radius over a mask of corners, one
// border — and a node with nothing else to paint keeps no bitmap. UIKit gives
// every view whose class overrides `draw(_:)` a backing store of its bounds
// times the scale squared, painted or not: a collection's spacer asked for
// gigabytes, and each card of a list carried its own. What Core Animation
// cannot say still draws, through `draw(_:)` as before: borders that differ
// by side, radii that differ by corner, a paragraph without a raster, a
// capture's picture. (The macOS host's `wantsUpdateLayer` split.) An image's
// pixels are a sublayer's contents where its clip is one radius over the
// part of the content box it covers: no bitmap of the view's size is painted
// on the main thread, and the decoded pixels are the only copy; an image
// clipped otherwise draws as before.
#if os(iOS)
import UIKit

/// Every node view's layer. `display` decides whether UIKit allocates a
/// backing store and calls `draw(_:)` at all.
final class NodeLayer: CALayer {
    override func display() {
        guard let node = delegate as? NodeView else { super.display(); return }
        node.applyBoxLayer()
        node.applyImageLayer()
        node.applyGradientLayer()
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
    /// image's pixels no sublayer shows, a capture's picture, a paragraph
    /// with no raster.
    var drawsPaint: Bool {
        if boxDrawn || (kind == "image" && symbolView == nil && raster != nil && imageLayer == nil) { return true }
        if Capture.capturing && (kind == "canvas" || Capture.web[id] != nil) { return true }
        return isParagraph && !canRasterText
    }

    /// An image's pixels onto a sublayer, or none, and `draw(_:)` paints
    /// them. `draw(_:)`'s geometry: object-fit over the content box, clipped
    /// by it and by the border box's radius. The sublayer is the visible part
    /// of the fitted image, `contentsRect` selecting it; it can carry the
    /// radius only when it is the whole content box and that is the border
    /// box, or when no corner is rounded.
    func applyImageLayer() {
        guard kind == "image", symbolView == nil, let bitmap = raster?.image else {
            imageLayer?.removeFromSuperlayer(); imageLayer = nil; return
        }
        let uniform = number("border_width")
        let content = bounds.insetBy(
            left: number("border_width_left", uniform) + number("padding_left"),
            top: number("border_width_top", uniform) + number("padding_top"),
            right: number("border_width_right", uniform) + number("padding_right"),
            bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
        let rect = RasterGeometry.rect(natural: bitmap.naturalSize, content: content, fit: style["object_fit"]?.string ?? "fill")
        let shown = rect.intersection(content)
        let radii = cornerRadii(in: bounds)
        let radius = radii.max() ?? 0
        let oneRadius = radii.allSatisfy { $0 == 0 || abs($0 - radius) < 0.01 }
            && radius <= min(bounds.width, bounds.height) / 2 + 0.01
        let fits = radius == 0 || (oneRadius && content == bounds && shown == content)
        guard fits, !shown.isNull, !shown.isEmpty, rect.width > 0, rect.height > 0 else {
            imageLayer?.removeFromSuperlayer(); imageLayer = nil
            return
        }
        var corners: CACornerMask = []
        let masks: [CACornerMask] = [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMaxXMaxYCorner, .layerMinXMaxYCorner]
        for (r, mask) in zip(radii, masks) where r > 0 { corners.insert(mask) }
        let unit = CGRect(x: (shown.minX - rect.minX) / rect.width, y: (shown.minY - rect.minY) / rect.height,
                          width: shown.width / rect.width, height: shown.height / rect.height)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let l = imageLayer ?? CALayer()
        if l.superlayer !== layer {
            // Where `draw(_:)` paints it: under the border and the children.
            if let border = boxBorder, border.superlayer === layer { layer.insertSublayer(l, below: border) } else { layer.insertSublayer(l, at: 0) }
            imageLayer = l
        }
        if l.frame != shown { l.frame = shown }
        if l.contentsRect != unit { l.contentsRect = unit }
        if (l.contents as AnyObject?) !== bitmap.image { l.contents = bitmap.image }
        if l.cornerRadius != radius { l.cornerRadius = radius }
        if radius > 0, l.maskedCorners != corners { l.maskedCorners = corners }
        let clips = radius > 0
        if l.masksToBounds != clips { l.masksToBounds = clips }
    }

    /// A `background-image` gradient (LLP 1056) as a sublayer under
    /// everything else the layer holds — over the layer's background, under
    /// its border and children — with the box's one radius, which is all a
    /// box `draw(_:)` does not paint can have. A view that paints through
    /// `draw(_:)` paints the gradient there instead, in the same place.
    func applyGradientLayer() {
        guard !drawsPaint, let gradient = Gradient(style["background_image"]) else {
            boxGradient?.removeFromSuperlayer(); boxGradient = nil; return
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let g = boxGradient ?? CAGradientLayer()
        boxGradient = g
        if layer.sublayers?.first !== g { layer.insertSublayer(g, at: 0) }
        if g.frame != layer.bounds { g.frame = layer.bounds }
        if g.cornerRadius != layer.cornerRadius { g.cornerRadius = layer.cornerRadius }
        if g.maskedCorners != layer.maskedCorners { g.maskedCorners = layer.maskedCorners }
        if g.masksToBounds != (layer.cornerRadius > 0) { g.masksToBounds = layer.cornerRadius > 0 }
        gradient.apply(g, bounds: layer.bounds, box: gradientBox, dark: drawsDark)
    }

    /// The box onto the layer, or `boxDrawn` when `draw(_:)` must paint it.
    /// The web's box: background and border inside the border box, a
    /// uniform border following the curve, the radius clipping children only
    /// where the overflow clips.
    func applyBoxLayer() {
        let background = channels("background_color").map { TextEngine.color($0).cgColor }
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
        let gradient = style["background_image"] != nil
        boxDrawn = !(oneBorder && oneRadius) && (fill != nil || gradient || widths.contains { $0 > 0 })
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
        if b.superlayer !== layer {
            if let image = imageLayer, image.superlayer === layer { layer.insertSublayer(b, above: image) } else { layer.insertSublayer(b, at: 0) }
            boxBorder = b
        }
        if b.frame != bounds { b.frame = bounds }
        if b.cornerRadius != cornerRadius { b.cornerRadius = cornerRadius }
        if b.maskedCorners != layer.maskedCorners { b.maskedCorners = layer.maskedCorners }
        if b.borderWidth != width { b.borderWidth = width }
        if b.borderColor != border { b.borderColor = border }
    }
}
#endif

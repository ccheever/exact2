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
#if os(iOS) || os(tvOS)
import UIKit

/// Every node view's layer. `display` decides whether UIKit allocates a
/// backing store and calls `draw(_:)` at all.
final class NodeLayer: CALayer {
    override func display() {
        guard let node = delegate as? NodeView else { super.display(); return }
        node.applyBoxLayer()
        node.applyImageLayer()
        node.applyGradientLayer()
        node.applyBoxMask()
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
    ///
    /// A `tint-color` draws (LLP 1011 §4): a mask layer would say it, but a
    /// canvas's capture (`render(in:)`) drops masks and would show the tint's
    /// whole rectangle, so the template is `draw(_:)`'s, from the same pixels.
    func applyImageLayer() {
        guard kind == "image", symbolView == nil, style["tint_color"] == nil, let bitmap = raster?.image else {
            imageLayer?.removeFromSuperlayer(); imageLayer = nil; return
        }
        if let look = flightLook {
            // Flying (LLP 1013.000 D4): the whole image where the flight
            // puts it; the view's own bounds and radius clip it.
            CATransaction.begin(); CATransaction.setDisableActions(true)
            defer { CATransaction.commit() }
            let l = imageLayer ?? CALayer()
            if l.superlayer !== layer { imageLayer = l; insertBoxSublayer(l) }
            l.frame = look.image
            l.contentsRect = CGRect(x: 0, y: 0, width: 1, height: 1)
            let frame = AnimatedRasters.shared.frame(for: self) ?? bitmap.image
            if (l.contents as AnyObject?) !== frame { l.contents = frame }
            l.cornerRadius = 0
            l.masksToBounds = false
            return
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
        let oneRadius = cornerSizes(in: bounds).allSatisfy { abs($0.width - $0.height) < 0.01 } && radii.allSatisfy { $0 == 0 || abs($0 - radius) < 0.01 }
            && radius <= min(bounds.width, bounds.height) / 2 + 0.01
        // A shape the layer's radius cannot say draws (LLP 1077 D1).
        let shape = CornerShape(style["corner_shape"])
        let fits = radius == 0 || (oneRadius && content == bounds && shown == content && (shape == nil || shape?.isAppleContinuous == true))
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
            imageLayer = l
            insertBoxSublayer(l)
        }
        if l.frame != shown { l.frame = shown }
        if l.contentsRect != unit { l.contentsRect = unit }
        let frame = AnimatedRasters.shared.frame(for: self) ?? bitmap.image
        if (l.contents as AnyObject?) !== frame { l.contents = frame }
        l.applyDynamicRange(hdr: bitmap.isHDR, headroom: bitmap.headroom, limit: style["dynamic_range_limit"]?.string)
        if l.cornerRadius != radius { l.cornerRadius = radius }
        if l.cornerCurve != layer.cornerCurve { l.cornerCurve = layer.cornerCurve }
        if radius > 0, l.maskedCorners != corners { l.maskedCorners = corners }
        let clips = radius > 0
        if l.masksToBounds != clips { l.masksToBounds = clips }
    }

    /// A `background-image` gradient (LLP 1066) as a sublayer under
    /// everything else the layer holds — over the layer's background, under
    /// its border and children — with the box's one radius, which is all a
    /// box `draw(_:)` does not paint can have. A view that paints through
    /// `draw(_:)` paints the gradient there instead, in the same place —
    /// unless it is `background-attachment: fixed` (`gradientLayered`).
    func applyGradientLayer() {
        let fixed = gradientLayered
        guard !drawsPaint || fixed, surface == nil, let gradient = Gradient(style["background_image"]), !gradient.isConic else {
            boxGradient?.removeFromSuperlayer(); boxGradient = nil
            presenter?.fixedGradients.remove(self)
            return
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let g = boxGradient ?? CAGradientLayer()
        boxGradient = g
        if layer.sublayers?.first !== g { layer.insertSublayer(g, at: 0) }
        if g.frame != layer.bounds { g.frame = layer.bounds }
        if drawsPaint {
            // Over the drawn box: its outline, whatever its corners.
            let outline = (g.mask as? CAShapeLayer) ?? CAShapeLayer()
            let path = roundedPath(in: bounds).cgPath
            if outline.path != path { outline.path = path }
            if g.mask !== outline { g.mask = outline }
            if g.cornerRadius != 0 { g.cornerRadius = 0 }
            if g.masksToBounds { g.masksToBounds = false }
        } else {
            if g.mask != nil { g.mask = nil }
            if g.cornerRadius != layer.cornerRadius { g.cornerRadius = layer.cornerRadius }
            if g.maskedCorners != layer.maskedCorners { g.maskedCorners = layer.maskedCorners }
            if g.cornerCurve != layer.cornerCurve { g.cornerCurve = layer.cornerCurve }
            if g.masksToBounds != (layer.cornerRadius > 0) { g.masksToBounds = layer.cornerRadius > 0 }
        }
        gradient.apply(g, bounds: layer.bounds, box: gradientBox, dark: drawsDark)
        if fixed { presenter?.fixedGradients.add(self) } else { presenter?.fixedGradients.remove(self) }
    }

    /// `background-attachment: fixed` (LLP 1066 D7) on iOS is always the
    /// gradient layer: anything above the node scrolling re-aims it
    /// (`reaimFixedGradient`), a few points set on a layer, where a drawn
    /// gradient would repaint the box every frame. Over a box `draw(_:)`
    /// paints (corners of different radii: a chat bubble) the layer is
    /// masked to the outline, over the drawn fill — and over a drawn
    /// border, which is owed. Several layers, a conic one, or another
    /// `background-clip` draw as before, unfixed.
    var gradientLayered: Bool { gradientFixed && !gradientDraws && backgroundClip == "border-box" }

    /// The viewport in this view's coordinates: the gradient box of a
    /// fixed gradient.
    var fixedGradientPort: CGRect? {
        guard let port = presenter?.viewport, window != nil else { return nil }
        return port.convert(port.bounds, to: self)
    }

    /// The fixed gradient aimed again at where the viewport now is.
    func reaimFixedGradient() {
        guard let g = boxGradient, let gradient = Gradient(style["background_image"]) else { return }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        gradient.apply(g, bounds: layer.bounds, box: gradientBox, dark: drawsDark)
        CATransaction.commit()
    }

    /// The box onto the layer, or `boxDrawn` when `draw(_:)` must paint it.
    /// The web's box: background and border inside the border box, a
    /// uniform border following the curve, the radius clipping children only
    /// where the overflow clips.
    func applyBoxLayer() {
        defer { syncEllipticalClip(); applyColorRanges() }
        let background = cgColor("background_color")
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
        let oneRadius = cornerSizes(in: bounds).allSatisfy { abs($0.width - $0.height) < 0.01 } && radii.allSatisfy { $0 == 0 || abs($0 - radius) < 0.01 }
            && radius <= min(bounds.width, bounds.height) / 2 + 0.01
        let gradient = style["background_image"] != nil
        // A `corner-shape` the layer cannot say draws (LLP 1077 D1); Apple's
        // continuous curve over one radius is the layer's `cornerCurve`.
        let shape = CornerShape(style["corner_shape"])
        let continuous = shape?.isAppleContinuous == true && oneRadius
        let curve: CALayerCornerCurve = continuous ? .continuous : .circular
        // A layout transition's size shows the surface on its own layer.
        let away = surface != nil
        // A border that draws is under the children, as the web paints it,
        // unless none can reach it: they are clipped, scrolled, or painted
        // through a surface. Then it is the layer's own, which Core Animation
        // paints over the sublayers.
        let own = clipsToBounds || clipBox != nil || scroll != nil || overlay != nil
        // Sides in one colour that differ only in width, square-cornered and
        // under the children (a row's `border-bottom` separator): each side
        // a rectangle of one shape layer. Where two sides meet, the web's
        // mitred join is that same colour, so their union paints the same,
        // and the view keeps no backing store of its size for a hairline.
        let drawn = widths.indices.filter { widths[$0] > 0 }
        let sideColor = drawn.first.map { colors[$0] }
        let edges = !oneBorder && !own && radii.allSatisfy { $0 == 0 } && drawn.allSatisfy { colors[$0] == sideColor }
        boxDrawn = !away && (!((oneBorder || edges) && oneRadius && (shape == nil || continuous)) || gradientDraws || backgroundClip != "border-box") && (fill != nil || gradient || widths.contains { $0 > 0 })
        let onLayer = !boxDrawn
        var corners: CACornerMask = []
        let masks: [CACornerMask] = [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMaxXMaxYCorner, .layerMinXMaxYCorner]
        for (r, mask) in zip(radii, masks) where r > 0 { corners.insert(mask) }
        let cornerRadius = onLayer && oneRadius && (shape == nil || continuous) ? radius : 0
        let border = !onLayer || away ? nil : edges ? sideColor : width > 0 ? colors[0] : nil
        applyShadow(outline: roundedPath(in: bounds).cgPath)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if let box = clipBox {
            box.layer.cornerRadius = cornerRadius
            box.layer.maskedCorners = corners
            box.layer.cornerCurve = curve
        }
        if layer.cornerCurve != curve { layer.cornerCurve = curve }
        // A vibrant fill is its vibrancy view's (`VibrancyIOS.swift`).
        let bg = onLayer && !away && (vibrancyView == nil || isParagraph) ? fill : nil
        if layer.backgroundColor != bg { layer.backgroundColor = bg }
        // A flight interpolates the radius itself (LLP 1013.000 D4).
        if flightLook == nil, layer.cornerRadius != cornerRadius { layer.cornerRadius = cornerRadius }
        if let v = vibrancyView, !isParagraph {
            v.layer.cornerRadius = cornerRadius
            v.layer.maskedCorners = corners
            v.clipsToBounds = cornerRadius > 0
        }
        if cornerRadius > 0, layer.maskedCorners != corners { layer.maskedCorners = corners }
        let ownWidth = own && border != nil ? width : 0
        if layer.borderWidth != ownWidth { layer.borderWidth = ownWidth }
        if ownWidth > 0, layer.borderColor != border { layer.borderColor = border }
        guard let border, !own else { boxBorder?.removeFromSuperlayer(); boxBorder = nil; return }
        if let old = boxBorder, (old is CAShapeLayer) != edges { old.removeFromSuperlayer(); boxBorder = nil }
        let b = boxBorder ?? (edges ? CAShapeLayer() : CALayer())
        if b.superlayer !== layer {
            boxBorder = b
            insertBoxSublayer(b)
        }
        if b.frame != bounds { b.frame = bounds }
        if let shape = b as? CAShapeLayer {
            let (w, h) = (bounds.width, bounds.height)
            let path = CGMutablePath()
            for rect in [CGRect(x: 0, y: 0, width: w, height: widths[0]),
                         CGRect(x: w - widths[1], y: 0, width: widths[1], height: h),
                         CGRect(x: 0, y: h - widths[2], width: w, height: widths[2]),
                         CGRect(x: 0, y: 0, width: widths[3], height: h)] where rect.width > 0 && rect.height > 0 {
                path.addRect(rect)
            }
            if shape.path != path { shape.path = path }
            if shape.fillColor != border { shape.fillColor = border }
            return
        }
        if b.cornerRadius != cornerRadius { b.cornerRadius = cornerRadius }
        if b.maskedCorners != layer.maskedCorners { b.maskedCorners = layer.maskedCorners }
        if b.cornerCurve != curve { b.cornerCurve = curve }
        if b.borderWidth != width { b.borderWidth = width }
        if b.borderColor != border { b.borderColor = border }
    }
}
extension NodeView {
    /// The box's sublayers keep CSS's paint order, all under the children:
    /// shadow, gradient, inset shadow, image, border, text — whichever of
    /// them joins first (as `Mac/BoxLayerMac.swift`).
    func insertBoxSublayer(_ l: CALayer) {
        let order: [CALayer?] = [shadowCaster, boxGradient, insetCaster, imageLayer, boxBorder, textRasterLayer]
        guard let rank = order.firstIndex(where: { $0 === l }) else { return }
        if let below = order[..<rank].reversed().compactMap({ $0 }).first(where: { $0.superlayer === layer }) {
            layer.insertSublayer(l, above: below)
        } else {
            layer.insertSublayer(l, at: 0)
        }
    }
}
#endif

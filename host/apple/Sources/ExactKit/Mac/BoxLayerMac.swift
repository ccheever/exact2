// A node's box on AppKit is its layer's properties where Core Animation can
// say it, as on UIKit (`IOS/BoxLayerIOS.swift`): the background, one corner
// radius over a mask of corners, one border, a gradient sublayer, an image's
// pixels as a sublayer's contents. Such a view answers `wantsUpdateLayer`,
// so AppKit gives it no backing store and never calls `draw(_:)`: the live
// row's 48 waveform bars, a card's border, a pill's fill were each a bitmap
// painted on the main thread at every commit that touched them. What Core
// Animation cannot say still draws through `draw(_:)`: borders that differ by
// side in colour or width (unless square and one colour), radii that differ
// by corner, a radius past half the shorter side, a tinted or unevenly
// clipped image, a paragraph without a raster, a canvas, a web view, and
// every capture (`cacheDisplay` draws views, not layer properties).
#if os(macOS)
import AppKit

extension NodeView {
    /// A node whose box can ride its layer: not a paragraph (its raster and
    /// its own drawing hold the box), a canvas or a web view (drawn views).
    var layerBoxEligible: Bool {
        kind != "text" && readerParagraph == nil && kind != "canvas" && kind != "iframe"
    }

    /// Whether `draw(_:)` must paint the box: the decision `applyBoxLayer`
    /// acts on, with no side effect, so `wantsUpdateLayer` can ask it.
    var boxNeedsDraw: Bool { hasBoxPaint && boxPlan.drawn }

    /// Paint only `draw(_:)` makes (outside a capture).
    var drawsPaint: Bool {
        if let cached = layerPaintCache { return cached }
        let paints = boxNeedsDraw || (kind == "image" && symbolView == nil && raster != nil && imagePlan == nil)
        layerPaintCache = paints
        return paints
    }

    private struct BoxPlan {
        var drawn = false
        var fill: CGColor?
        var radius: CGFloat = 0
        var corners: CACornerMask = []
        var oneRadius = true
        var widths: [CGFloat] = [0, 0, 0, 0]
        var colors: [CGColor] = []
        var oneBorder = true
        var edges = false
        var sideColor: CGColor?
        var own = false
        /// Apple's continuous curve over one radius (LLP 1077 D1).
        var curve: CALayerCornerCurve = .circular
        /// A shape only a path says: the layer keeps no radius of its own.
        var shaped = false
    }

    /// Core Animation's corner mask names corners in the layer's own space;
    /// a node's layer is flipped (y down) as the view is, and iOS's names
    /// then mean the same corners.
    private func cornerMask(_ radii: [CGFloat]) -> CACornerMask {
        let flipped = layer?.contentsAreFlipped() ?? true
        let masks: [CACornerMask] = flipped
            ? [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMaxXMaxYCorner, .layerMinXMaxYCorner]
            : [.layerMinXMaxYCorner, .layerMaxXMaxYCorner, .layerMaxXMinYCorner, .layerMinXMinYCorner]
        var corners: CACornerMask = []
        for (r, mask) in zip(radii, masks) where r > 0 { corners.insert(mask) }
        return corners
    }

    private var boxPlan: BoxPlan {
        var p = BoxPlan()
        // Most nodes paint nothing and clip nothing: nothing to read.
        guard hasBoxPaint || clipsToBounds || clipBox != nil else { return p }
        // sRGB colours straight from the rows: an NSColor's `cgColor` is
        // made anew on each call, and this runs for every repaint.
        func cg(_ c: [Double]) -> CGColor { CGColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255) }
        p.fill = channels("background_color").flatMap { $0[3] > 0 ? cg($0) : nil }
        let uniform = number("border_width")
        let sides = ["top", "right", "bottom", "left"]
        p.widths = sides.map { number("border_width_" + $0, uniform) }
        if p.widths.contains(where: { $0 > 0 }) {
            let top = channels("border_color_top")
            p.colors = sides.map { side in (channels("border_color_" + side) ?? top).map(cg) ?? CGColor(gray: 0, alpha: 0) }
        } else {
            p.colors = Array(repeating: CGColor(gray: 0, alpha: 0), count: 4)
        }
        let width = p.widths[0]
        p.oneBorder = p.widths.allSatisfy { $0 == width } && (width == 0 || p.colors.allSatisfy { $0 == p.colors[0] })
        // One radius over the corners that have one; CSS's reduction first,
        // and Core Animation's own limit (half the shorter side) not reached.
        let radii = cornerRadii(in: bounds)
        p.radius = radii.max() ?? 0
        p.oneRadius = cornerSizes(in: bounds).allSatisfy { abs($0.width - $0.height) < 0.01 } && radii.allSatisfy { $0 == 0 || abs($0 - p.radius) < 0.01 }
            && p.radius <= min(bounds.width, bounds.height) / 2 + 0.01
        p.corners = cornerMask(radii)
        let gradient = style["background_image"] != nil
        // A `corner-shape` the layer cannot say draws (LLP 1077 D1).
        let shape = CornerShape(style["corner_shape"])
        let continuous = shape?.isAppleContinuous == true && p.oneRadius
        p.curve = continuous ? .continuous : .circular
        p.shaped = shape != nil && !continuous
        // A border under the children unless none can reach it (clipped,
        // scrolled, or painted through a surface): then the layer's own,
        // which Core Animation paints over the sublayers.
        p.own = clipsToBounds || clipBox != nil || scroll != nil || overlay != nil
        // Square sides in one colour that differ only in width (a row's
        // `border-bottom` separator) are rectangles of one shape layer.
        let drawn = p.widths.indices.filter { p.widths[$0] > 0 }
        p.sideColor = drawn.first.map { p.colors[$0] }
        p.edges = !p.oneBorder && !p.own && radii.allSatisfy { $0 == 0 } && drawn.allSatisfy { p.colors[$0] == p.sideColor }
        // A layout transition's size shows the surface on its own layer.
        let away = surface != nil
        p.drawn = !away && (!((p.oneBorder || p.edges) && p.oneRadius && (shape == nil || continuous)) || gradientDraws || backgroundClip != "border-box") && (p.fill != nil || gradient || p.widths.contains { $0 > 0 })
        // A border over the children needs the backing layer's own radius,
        // and AppKit makes a backing layer's radius clip: where the node
        // does not clip, a rounded one draws.
        if p.own && !clipsToBounds && p.radius > 0 && p.widths.contains(where: { $0 > 0 }) { p.drawn = !away }
        return p
    }

    /// The box onto the layer, or nothing on it when `draw(_:)` paints it.
    /// The web's box: background and border inside the border box, a
    /// uniform border following the curve, the radius clipping children
    /// only where the overflow clips.
    func applyBoxLayer() {
        defer { syncEllipticalClip() }
        guard layerBoxEligible else { applyClipOnly(); return }
        let p = boxPlan
        applyBoxLayer(p)
        // An image's answer also depends on its pixels' layer, set at paint.
        layerPaintCache = kind == "image" ? nil : hasBoxPaint && p.drawn
    }

    private func applyBoxLayer(_ p: BoxPlan) {
        guard let layer else { return }
        let onLayer = !p.drawn
        let away = surface != nil
        let border = !onLayer || away ? nil : p.edges ? p.sideColor : p.widths[0] > 0 ? p.colors[0] : nil
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if let box = clipBox?.layer {
            let clipRadius = p.oneRadius && !p.shaped ? p.radius : 0
            if box.cornerRadius != clipRadius { box.cornerRadius = clipRadius }
            if clipRadius > 0, box.maskedCorners != p.corners { box.maskedCorners = p.corners }
            if box.cornerCurve != p.curve { box.cornerCurve = p.curve }
        }
        // AppKit makes a backing layer's corner radius clip its sublayers
        // (setting one sets `masksToBounds`), so the backing layer carries a
        // radius only where the overflow clips; a rounded box that does not
        // clip fills a sublayer of its own under the children.
        let layerRadius = clipsToBounds && p.oneRadius && !p.shaped ? p.radius : 0
        if layer.cornerRadius != layerRadius { layer.cornerRadius = layerRadius }
        if layerRadius > 0, layer.maskedCorners != p.corners { layer.maskedCorners = p.corners }
        if layer.cornerCurve != p.curve { layer.cornerCurve = p.curve }
        let fill = onLayer && !away ? p.fill : nil
        let fillsSublayer = fill != nil && p.radius > 0 && layerRadius == 0
        let bg = fillsSublayer ? nil : fill
        if layer.backgroundColor != bg { layer.backgroundColor = bg }
        if fillsSublayer, let fill {
            let f = boxFill ?? CALayer()
            boxFill = f
            if f.superlayer !== layer { insertBoxSublayer(f) }
            if f.frame != bounds { f.frame = bounds }
            if f.backgroundColor != fill { f.backgroundColor = fill }
            if f.cornerRadius != p.radius { f.cornerRadius = p.radius }
            if f.maskedCorners != p.corners { f.maskedCorners = p.corners }
            if f.cornerCurve != p.curve { f.cornerCurve = p.curve }
        } else if let f = boxFill { f.removeFromSuperlayer(); boxFill = nil }
        let cornerRadius = p.oneRadius ? p.radius : 0
        let ownWidth = p.own && border != nil ? devicePixels(p.widths[0]) : 0
        if layer.borderWidth != ownWidth { layer.borderWidth = ownWidth }
        if ownWidth > 0, layer.borderColor != border { layer.borderColor = border }
        guard let border, !p.own else { boxBorder?.removeFromSuperlayer(); boxBorder = nil; return }
        if let old = boxBorder, (old is CAShapeLayer) != p.edges { old.removeFromSuperlayer(); boxBorder = nil }
        let b = boxBorder ?? (p.edges ? CAShapeLayer() : CALayer())
        boxBorder = b
        if b.superlayer !== layer { insertBoxSublayer(b) }
        if b.frame != bounds { b.frame = bounds }
        if let shape = b as? CAShapeLayer {
            let (w, h) = (bounds.width, bounds.height)
            let path = CGMutablePath()
            // The layer is flipped as the view is: y grows down.
            let d = p.widths.map(devicePixels)
            for rect in [CGRect(x: 0, y: 0, width: w, height: d[0]),
                         CGRect(x: w - d[1], y: 0, width: d[1], height: h),
                         CGRect(x: 0, y: h - d[2], width: w, height: d[2]),
                         CGRect(x: 0, y: 0, width: d[3], height: h)] where rect.width > 0 && rect.height > 0 {
                path.addRect(rect)
            }
            if shape.path != path { shape.path = path }
            if shape.fillColor != border { shape.fillColor = border }
            return
        }
        if b.cornerRadius != cornerRadius { b.cornerRadius = cornerRadius }
        if b.maskedCorners != p.corners { b.maskedCorners = p.corners }
        if b.cornerCurve != p.curve { b.cornerCurve = p.curve }
        if b.borderWidth != devicePixels(p.widths[0]) { b.borderWidth = devicePixels(p.widths[0]) }
        if b.borderColor != border { b.borderColor = border }
    }

    /// A paragraph, canvas or web view paints its box in `draw(_:)`; its
    /// layer carries only the clip's radius, reduced as CSS reduces it.
    private func applyClipOnly() {
        guard let l = clipBox?.layer ?? layer else { return }
        let radii = cornerSizes(in: bounds)
        let clips = clipsToBounds || clipBox != nil
        let first = radii[0]
        let circular = radii.allSatisfy { abs($0.width - $0.height) < 0.01 && abs($0.width - first.width) < 0.01 }
            && first.width <= min(bounds.width, bounds.height) / 2
        let radius = clips && circular ? first.width : 0
        if l.cornerRadius != radius { l.cornerRadius = radius }
    }

    /// A `background-image` gradient as a sublayer under everything else the
    /// layer holds, with the box's one radius; a box `draw(_:)` paints
    /// paints its gradient there instead.
    private func applyGradientLayer(_ p: BoxPlan) {
        guard let layer, layerBoxEligible, !(hasBoxPaint && p.drawn), surface == nil, let gradient = Gradient(style["background_image"]), !gradient.isConic else {
            boxGradient?.removeFromSuperlayer(); boxGradient = nil; return
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let g = boxGradient ?? CAGradientLayer()
        boxGradient = g
        if g.superlayer !== layer { insertBoxSublayer(g) }
        if g.frame != layer.bounds { g.frame = layer.bounds }
        let radius = p.radius
        let round = radius > 0 && p.oneRadius
        if g.cornerRadius != (round ? radius : 0) { g.cornerRadius = round ? radius : 0 }
        if round, g.maskedCorners != p.corners { g.maskedCorners = p.corners }
        if g.cornerCurve != p.curve { g.cornerCurve = p.curve }
        if g.masksToBounds != round { g.masksToBounds = round }
        gradient.apply(g, bounds: layer.bounds, box: gradientBox, dark: drawsDark)
    }

    /// Where an image's pixels go on a sublayer: the visible part of the
    /// fitted image (`contentsRect` selecting it), which can carry the radius
    /// only when it is the whole content box and that is the border box, or
    /// when no corner is rounded. `nil`: `draw(_:)` paints them.
    private var imagePlan: (shown: CGRect, unit: CGRect, radius: CGFloat, corners: CACornerMask)? {
        guard kind == "image", symbolView == nil, style["tint_color"] == nil, let bitmap = raster?.image else { return nil }
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
        guard fits, !shown.isNull, !shown.isEmpty, rect.width > 0, rect.height > 0 else { return nil }
        // `contentsRect` is in the image's unit space, y up from its bottom
        // edge in an unflipped layer; the fitted image is centred, so a
        // flipped layer selects the same band either way except for
        // `object-fit: none` past an edge, where the origin is mirrored.
        var unit = CGRect(x: (shown.minX - rect.minX) / rect.width, y: (shown.minY - rect.minY) / rect.height,
                          width: shown.width / rect.width, height: shown.height / rect.height)
        if !(layer?.contentsAreFlipped() ?? true) { unit.origin.y = 1 - unit.maxY }
        return (shown, unit, radius, cornerMask(radii))
    }

    /// The image's pixels onto a sublayer, or none (`draw(_:)` paints them).
    func applyImageLayer() {
        guard let layer, layerBoxEligible, let plan = imagePlan, let bitmap = raster?.image else {
            imageLayer?.removeFromSuperlayer(); imageLayer = nil; return
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let l = imageLayer ?? CALayer()
        imageLayer = l
        // Where `draw(_:)` paints it: over the box, under the border and the children.
        if l.superlayer !== layer { insertBoxSublayer(l) }
        if l.frame != plan.shown { l.frame = plan.shown }
        if l.contentsRect != plan.unit { l.contentsRect = plan.unit }
        if l.contentsGravity != .resize { l.contentsGravity = .resize }
        let frame = AnimatedRasters.shared.frame(for: self) ?? bitmap.image
        if (l.contents as AnyObject?) !== frame { l.contents = frame }
        if l.cornerRadius != plan.radius { l.cornerRadius = plan.radius }
        let curve: CALayerCornerCurve = CornerShape(style["corner_shape"])?.isAppleContinuous == true ? .continuous : .circular
        if l.cornerCurve != curve { l.cornerCurve = curve }
        if plan.radius > 0, l.maskedCorners != plan.corners { l.maskedCorners = plan.corners }
        let clips = plan.radius > 0
        if l.masksToBounds != clips { l.masksToBounds = clips }
    }

    /// The box's sublayers keep CSS's paint order, all under the children:
    /// shadow, fill, gradient, image, border.
    func insertBoxSublayer(_ l: CALayer) {
        guard let layer else { return }
        let order: [CALayer?] = [shadowCaster, boxFill, boxGradient, insetCaster, imageLayer, boxBorder]
        guard let rank = order.firstIndex(where: { $0 === l }) else { return }
        if let below = order[..<rank].reversed().compactMap({ $0 }).first(where: { $0.superlayer === layer }) {
            layer.insertSublayer(l, above: below)
        } else {
            layer.insertSublayer(l, at: 0)
        }
    }

    /// A border width as Chrome paints it: whole device pixels, and never
    /// less than one (a 0.5 pt hairline is one pixel at 1x, as `draw(_:)`'s
    /// backing store showed it).
    func devicePixels(_ width: CGFloat) -> CGFloat {
        guard width > 0 else { return 0 }
        let scale = window?.backingScaleFactor ?? layer?.contentsScale ?? 2
        return max(1, (width * scale + 0.001).rounded(.down)) / scale
    }

    /// The highest of the box's sublayers, which replaced content (a Canvas
    /// 2D bitmap) goes over.
    var boxSublayersTop: CALayer? {
        [boxBorder, imageLayer, insetCaster, boxGradient, boxFill, shadowCaster].compactMap { $0 }.first { $0.superlayer === layer }
    }

    /// Everything the layer can say, before `draw(_:)` or instead of it.
    func applyLayerPaint() {
        guard layerBoxEligible else { applyClipOnly(); return }
        let p = boxPlan
        applyBoxLayer(p)
        applyGradientLayer(p)
        applyImageLayer()
        applyBoxMask()
        // What the plan just decided is what `wantsUpdateLayer` answers
        // until something it reads changes.
        layerPaintCache = (hasBoxPaint && p.drawn) || (kind == "image" && symbolView == nil && raster != nil && imageLayer == nil)
    }
}
#endif

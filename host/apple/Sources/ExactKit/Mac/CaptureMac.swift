// The agent's default picture follows the same sibling order as input.
// Copy the layer tree, never reorder the live views (Tab and AX tree order).
#if os(macOS)
import AppKit

extension Capture {
    static func paintOrderBitmap(of view: NSView) -> NSBitmapImageRep? {
        view.layoutSubtreeIfNeeded()
        view.displayIfNeeded()
        let scale = view.window?.backingScaleFactor ?? 2
        let width = Int((view.bounds.width * scale).rounded()), height = Int((view.bounds.height * scale).rounded())
        guard let root = view.layer, width > 0, height > 0,
              let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        var owners: [ObjectIdentifier: NSView] = [:]
        func index(_ v: NSView) {
            if let layer = v.layer { owners[ObjectIdentifier(layer)] = v }
            for child in v.subviews { index(child) }
        }
        index(view)
        func picture(_ image: CGImage, in bounds: CGRect) -> CALayer {
            let layer = CALayer()
            layer.frame = bounds
            layer.contents = image
            return layer
        }
        func copy(_ source: CALayer) -> CALayer {
            source.displayIfNeeded()
            // CALayer(layer:) is the subclass copy initializer used by CA;
            // called directly, it gives an empty layer. Copy values explicitly.
            // Animated properties come from the displayed frame. Unanimated
            // layers use the model so a synchronous capture sees fresh writes
            // even before CA commits them. Identity and topology stay model-owned.
            let values = source.animationKeys()?.isEmpty == false ? (source.presentation() ?? source) : source
            let layer: CALayer
            if let shape = values as? CAShapeLayer {
                let out = CAShapeLayer()
                out.path = shape.path; out.fillColor = shape.fillColor; out.fillRule = shape.fillRule
                out.strokeColor = shape.strokeColor; out.lineWidth = shape.lineWidth
                out.lineCap = shape.lineCap; out.lineJoin = shape.lineJoin; out.miterLimit = shape.miterLimit
                out.lineDashPattern = shape.lineDashPattern; out.lineDashPhase = shape.lineDashPhase
                out.strokeStart = shape.strokeStart; out.strokeEnd = shape.strokeEnd
                layer = out
            } else if let gradient = values as? CAGradientLayer {
                let out = CAGradientLayer()
                out.colors = gradient.colors; out.locations = gradient.locations; out.type = gradient.type
                out.startPoint = gradient.startPoint; out.endPoint = gradient.endPoint
                layer = out
            } else { layer = values is CATransformLayer ? CATransformLayer() : CALayer() }
            layer.bounds = values.bounds; layer.position = values.position
            layer.anchorPoint = values.anchorPoint; layer.anchorPointZ = values.anchorPointZ
            layer.transform = values.transform; layer.sublayerTransform = values.sublayerTransform
            layer.isGeometryFlipped = values.isGeometryFlipped
            layer.opacity = values.opacity; layer.isHidden = values.isHidden
            layer.isDoubleSided = values.isDoubleSided; layer.masksToBounds = values.masksToBounds
            layer.cornerRadius = values.cornerRadius; layer.maskedCorners = values.maskedCorners
            layer.cornerCurve = values.cornerCurve
            layer.backgroundColor = values.backgroundColor
            layer.borderColor = values.borderColor; layer.borderWidth = values.borderWidth
            layer.contents = values.contents; layer.contentsScale = values.contentsScale
            layer.contentsRect = values.contentsRect; layer.contentsCenter = values.contentsCenter
            layer.contentsGravity = values.contentsGravity
            layer.minificationFilter = values.minificationFilter; layer.magnificationFilter = values.magnificationFilter
            layer.shadowPath = values.shadowPath; layer.shadowColor = values.shadowColor
            layer.shadowOffset = values.shadowOffset; layer.shadowRadius = values.shadowRadius
            layer.shadowOpacity = values.shadowOpacity
            layer.allowsGroupOpacity = values.allowsGroupOpacity
            layer.mask = source.mask.map(copy)
            // Order is explicit: negative ranks still follow the parent's
            // own fill/gradient layers. Authored 3D transforms stay intact.
            let owner = owners[ObjectIdentifier(source)]
            let node = owner as? NodeView
            let children = owner.map { NodeView.hitOrder($0.subviews).reversed().compactMap(\.layer) } ?? []
            let childSet = Set(children.map(ObjectIdentifier.init))
            let replacements = Set((owner?.subviews ?? []).compactMap { ($0 as? NodeView)?.boxFilter?.picture }.map(ObjectIdentifier.init))
            // Own contents and decorations first: shadow casters, fills,
            // text rasters, Canvas 2D pictures. Each view branch is below.
            for sub in source.sublayers ?? [] where !childSet.contains(ObjectIdentifier(sub)) && !replacements.contains(ObjectIdentifier(sub)) {
                layer.addSublayer(copy(sub))
            }
            if let node {
                // Metal and remote WebKit layers do not render into a CPU
                // context. Keep their existing readback/snapshot providers.
                if node.kind == "canvas", let image = node.canvases?.readback(view: node)?.cgImage {
                    layer.addSublayer(picture(image, in: node.bounds))
                }
                if let image = web[node.id]?.cgImage(forProposedRect: nil, context: nil, hints: nil) {
                    layer.addSublayer(picture(image, in: node.bounds))
                }
            }
            for child in children {
                // A filtered node's picture already includes its subtree.
                // Drawing the masked source too would duplicate it.
                if let node = owners[ObjectIdentifier(child)] as? NodeView, let filter = node.boxFilter,
                   filter.picture.superlayer === source {
                    layer.addSublayer(copy(filter.picture))
                } else { layer.addSublayer(copy(child)) }
            }
            return layer
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let tree = copy(root)
        // CA renders the root in bottom-up Quartz space; the agent's
        // bitmap (like AppKit's cacheDisplay) has its first row at the top.
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: scale, y: -scale)
        tree.render(in: ctx)
        guard let image = ctx.makeImage() else { return nil }
        let rep = NSBitmapImageRep(cgImage: image)
        rep.size = view.bounds.size
        return rep
    }
}
#endif

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
            let layer: CALayer
            if let shape = source as? CAShapeLayer {
                let out = CAShapeLayer()
                out.path = shape.path; out.fillColor = shape.fillColor; out.fillRule = shape.fillRule
                out.strokeColor = shape.strokeColor; out.lineWidth = shape.lineWidth
                out.lineCap = shape.lineCap; out.lineJoin = shape.lineJoin; out.miterLimit = shape.miterLimit
                out.lineDashPattern = shape.lineDashPattern; out.lineDashPhase = shape.lineDashPhase
                out.strokeStart = shape.strokeStart; out.strokeEnd = shape.strokeEnd
                layer = out
            } else if let gradient = source as? CAGradientLayer {
                let out = CAGradientLayer()
                out.colors = gradient.colors; out.locations = gradient.locations; out.type = gradient.type
                out.startPoint = gradient.startPoint; out.endPoint = gradient.endPoint
                layer = out
            } else { layer = source is CATransformLayer ? CATransformLayer() : CALayer() }
            layer.bounds = source.bounds; layer.position = source.position
            layer.anchorPoint = source.anchorPoint; layer.anchorPointZ = source.anchorPointZ
            layer.transform = source.transform; layer.sublayerTransform = source.sublayerTransform
            layer.isGeometryFlipped = source.isGeometryFlipped
            layer.opacity = source.opacity; layer.isHidden = source.isHidden
            layer.isDoubleSided = source.isDoubleSided; layer.masksToBounds = source.masksToBounds
            layer.cornerRadius = source.cornerRadius; layer.maskedCorners = source.maskedCorners
            layer.cornerCurve = source.cornerCurve
            layer.backgroundColor = source.backgroundColor
            layer.borderColor = source.borderColor; layer.borderWidth = source.borderWidth
            layer.contents = source.contents; layer.contentsScale = source.contentsScale
            layer.contentsRect = source.contentsRect; layer.contentsCenter = source.contentsCenter
            layer.contentsGravity = source.contentsGravity
            layer.minificationFilter = source.minificationFilter; layer.magnificationFilter = source.magnificationFilter
            layer.shadowPath = source.shadowPath; layer.shadowColor = source.shadowColor
            layer.shadowOffset = source.shadowOffset; layer.shadowRadius = source.shadowRadius
            layer.shadowOpacity = source.shadowOpacity
            layer.allowsGroupOpacity = source.allowsGroupOpacity
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

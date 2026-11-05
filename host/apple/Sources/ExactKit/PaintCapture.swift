// An ordered copy for Core Graphics, which ignores a layer's zPosition.
// Live views and layers stay in logical order for input and accessibility.
// The copy shares its layers' contents, AppKit's backing stores among them
// (with `redraw`, a node's own drawing is drawn again instead): draw it
// (`render(in:)`), never commit it to a window's layer tree, where sharing a
// view's backing store blanks the view (`PressLookMac.swift`).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Capture {
    /// `branchProbe` counts membership lookups so tests can bound work without timing.
    /// `redraw`, on the Mac, is the scale at which a node that AppKit draws
    /// (`draw(_:)`) is drawn again into a picture of the copy's own, in place
    /// of its backing stores (`borrowsBackingStore`).
    static func paintOrderLayer(of view: PaintView, root: CALayer, redraw: CGFloat? = nil, branchProbe: (() -> Void)? = nil) -> CALayer {
        var owners: [ObjectIdentifier: PaintView] = [:]
        func index(_ v: PaintView) {
            #if os(macOS)
            if let layer = v.layer { owners[ObjectIdentifier(layer)] = v }
            #else
            owners[ObjectIdentifier(v.layer)] = v
            #endif
            for child in v.subviews { index(child) }
        }
        index(view)
        #if os(macOS)
        // Route each view branch through its live intermediate layers, once.
        // They carry geometry and clipping even though no view owns them.
        var branches: [ObjectIdentifier: [CALayer]] = [:]
        var seen: [ObjectIdentifier: Set<ObjectIdentifier>] = [:]
        for owner in owners.values {
            guard let parent = owner.layer else { continue }
            for child in NodeView.hitOrder(owner.subviews).reversed().compactMap(\.layer) {
                var path = [child]
                while let next = path.last?.superlayer, next !== parent { path.append(next) }
                if path.last?.superlayer !== parent { path = [child] }
                var container = parent
                for branch in path.reversed() {
                    let key = ObjectIdentifier(container)
                    branchProbe?()
                    if seen[key, default: []].insert(ObjectIdentifier(branch)).inserted {
                        branches[key, default: []].append(branch)
                    }
                    container = branch
                }
            }
        }
        func picture(_ image: CGImage, in bounds: CGRect) -> CALayer {
            let layer = CALayer()
            layer.frame = bounds
            layer.contents = image
            return layer
        }
        #endif
        func overlay(_ animation: CAAnimation, from presentation: CALayer, onto layer: CALayer) {
            if let group = animation as? CAAnimationGroup {
                for child in group.animations ?? [] { overlay(child, from: presentation, onto: layer) }
            } else if let property = animation as? CAPropertyAnimation, let keyPath = property.keyPath {
                // Rank and topology stay model-owned. The animation's storage
                // key is arbitrary; keyPath names what it actually animates,
                // including components such as transform.translation.x.
                let key = String(keyPath.prefix { $0 != "." })
                guard !["zPosition", "mask", "sublayers"].contains(key) else { return }
                layer.setValue(presentation.value(forKeyPath: keyPath), forKeyPath: keyPath)
            }
        }
        func copy(_ source: CALayer, inheritedFlip: Bool = false) -> CALayer {
            source.displayIfNeeded()
            // Start with the model: presentation does not yet contain writes
            // in the current transaction, even on a layer animating another key.
            let values = source
            let layer = values.valueCopy()
            #if os(macOS)
            let flipped = source.contentsAreFlipped()
            // Contents inherit their orientation through the live ancestors;
            // the copied branch must preserve it in its new ancestry.
            layer.isGeometryFlipped = flipped != inheritedFlip
            #else
            let flipped = false
            layer.isGeometryFlipped = values.isGeometryFlipped
            #endif
            layer.isHidden = values.isHidden
            if let keys = source.animationKeys(), !keys.isEmpty, let presentation = source.presentation() {
                for key in keys {
                    if let animation = source.animation(forKey: key) { overlay(animation, from: presentation, onto: layer) }
                }
            }
            #if os(macOS)
            // A node's own drawing, its backing store and the content layers
            // AppKit adds for ink past its bounds, drawn again: a copy that
            // shares them, even uncommitted, can leave the live view showing
            // nothing at a fractional device scale (a page zoom) until it is
            // laid out again, however often it draws.
            let redrawn = redraw.flatMap { scale -> CALayer? in
                guard let node = owners[ObjectIdentifier(source)] as? NodeView else { return nil }
                let stores = (source.sublayers ?? []).filter(Capture.borrowsBackingStore)
                guard Capture.borrowsBackingStore(source) || !stores.isEmpty else { return nil }
                // Out to whole device pixels, so the picture is not resampled into its box.
                let ink = stores.reduce(node.bounds) { $0.union($1.frame) }
                let pixels = CGRect(x: ink.minX * scale, y: ink.minY * scale, width: ink.width * scale, height: ink.height * scale)
                    .insetBy(dx: 0.001, dy: 0.001).integral
                let box = CGRect(x: pixels.minX / scale, y: pixels.minY / scale, width: pixels.width / scale, height: pixels.height / scale)
                return Capture.drawing(of: node, in: box, scale: scale).map { picture($0, in: box) }
            }
            if redrawn != nil, Capture.borrowsBackingStore(source) { layer.contents = nil }
            if let redrawn { layer.addSublayer(redrawn) }
            #endif
            layer.mask = source.mask.map { copy($0) }
            // Order is explicit: negative ranks still follow the parent's
            // own fill/gradient layers. Authored 3D transforms stay intact.
            let owner = owners[ObjectIdentifier(source)]
            #if os(macOS)
            let children = branches[ObjectIdentifier(source)] ?? []
            #else
            // Flat leaves have no UIView: merge their tree slots with the
            // view branches before sorting, so they cover negative ranks.
            let viewLayers = Set((owner?.subviews ?? []).map { ObjectIdentifier($0.layer) })
            let children = (source.sublayers ?? []).enumerated().filter {
                viewLayers.contains(ObjectIdentifier($0.element)) || $0.element.flatHit != nil
            }.sorted {
                let a = $0.element.zPosition, b = $1.element.zPosition
                return a != b ? a < b : $0.offset < $1.offset
            }.map(\.element)
            #endif
            let childSet = Set(children.map(ObjectIdentifier.init))
            let replacements = Set(children.compactMap { child -> ObjectIdentifier? in
                guard let node = owners[ObjectIdentifier(child)] as? NodeView,
                      let picture = node.boxFilter?.picture, picture.superlayer === source else { return nil }
                return ObjectIdentifier(picture)
            })
            // Own contents and decorations first: shadow casters, fills,
            // text rasters, Canvas 2D pictures. Each view branch is below.
            #if os(macOS)
            let redrawnStore = { (sub: CALayer) in redrawn != nil && Capture.borrowsBackingStore(sub) }
            #else
            let redrawnStore = { (_: CALayer) in false }
            #endif
            let own = (source.sublayers ?? []).filter {
                !childSet.contains(ObjectIdentifier($0)) && !replacements.contains(ObjectIdentifier($0)) && !redrawnStore($0)
            }
            #if os(macOS)
            // A decoration the Mac raises over the node's own views (a
            // press's wash, PressLookMac.swift) is painted after them.
            let raised = own.filter { $0.zPosition >= 1 }
            #else
            let raised: [CALayer] = []
            #endif
            for sub in own where !raised.contains(where: { $0 === sub }) {
                layer.addSublayer(copy(sub, inheritedFlip: flipped))
            }
            #if os(macOS)
            if let node = owner as? NodeView {
                // Metal and remote WebKit layers do not render into a CPU
                // context. Keep their existing readback/snapshot providers.
                if node.kind == "canvas", let image = node.canvases?.readback(view: node)?.cgImage {
                    layer.addSublayer(picture(image, in: node.bounds))
                }
                if let image = web[node.id]?.cgImage(forProposedRect: nil, context: nil, hints: nil) {
                    layer.addSublayer(picture(image, in: node.bounds))
                }
            }
            #endif
            for child in children {
                // A filtered node's picture already includes its subtree.
                // Drawing the masked source too would duplicate it.
                if let node = owners[ObjectIdentifier(child)] as? NodeView, let filter = node.boxFilter,
                   filter.picture.superlayer === source {
                    layer.addSublayer(copy(filter.picture, inheritedFlip: flipped))
                } else { layer.addSublayer(copy(child, inheritedFlip: flipped)) }
            }
            for sub in raised { layer.addSublayer(copy(sub, inheritedFlip: flipped)) }
            return layer
        }
        return copy(root)
    }

    #if os(macOS)
    /// Whether a layer shows a backing store AppKit drew a view into, not a
    /// picture of Exact's (an image, a surface).
    static func borrowsBackingStore(_ layer: CALayer) -> Bool {
        guard let contents = layer.contents else { return false }
        let type = CFGetTypeID(contents as CFTypeRef)
        return type != CGImage.typeID && type != IOSurfaceGetTypeID() && !(contents is NSImage)
    }

    /// What a node's `draw(_:)` paints in `box` (its own coordinates, whole
    /// device pixels), at `scale` device pixels per point, its first row at
    /// the top.
    static func drawing(of node: NodeView, in box: CGRect, scale: CGFloat) -> CGImage? {
        let width = Int((box.width * scale).rounded()), height = Int((box.height * scale).rounded())
        guard width > 0, height > 0,
              let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        ctx.scaleBy(x: scale, y: scale)
        if node.isFlipped { ctx.translateBy(x: 0, y: box.height); ctx.scaleBy(x: 1, y: -1) }
        ctx.translateBy(x: -box.minX, y: -box.minY)
        NSGraphicsContext.saveGraphicsState()
        defer { NSGraphicsContext.restoreGraphicsState() }
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: node.isFlipped)
        node.draw(box)
        return ctx.makeImage()
    }
    #endif
}

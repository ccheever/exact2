// An ordered copy for Core Graphics, which ignores a layer's zPosition.
// Live views and layers stay in logical order for input and accessibility.
// The copy shares its layers' contents, AppKit's backing stores among them:
// draw it (`render(in:)`), never commit it to a window's layer tree, where
// sharing a view's backing store blanks the view (`PressLookMac.swift`).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Capture {
    /// `branchProbe` counts membership lookups so tests can bound work without timing.
    static func paintOrderLayer(of view: PaintView, root: CALayer, branchProbe: (() -> Void)? = nil) -> CALayer {
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
            let own = (source.sublayers ?? []).filter { !childSet.contains(ObjectIdentifier($0)) && !replacements.contains(ObjectIdentifier($0)) }
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
}

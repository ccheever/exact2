#if canImport(UIKit)
import QuartzCore
import UIKit

extension FrameSampler {
    /// The layers in `window` that cost the render server work of their own
    /// each frame it composites them: a backdrop (`CABackdropLayer`, which
    /// UIKit's blur and glass use), which renders what lies behind it first;
    /// a portal, which renders another part of the tree again; a filter, a
    /// blend mode, a mask, a shadow without a `shadowPath`, group opacity and
    /// rasterization, each an offscreen pass; and a Metal layer (MapKit's
    /// map), which the GPU draws itself. Read once, when a recording stops,
    /// so a frame the render server showed late can be matched to what was
    /// on screen at its `offset`: one row per kind, place and node, its
    /// layers' extent merged, or per kind and place when more than four
    /// nodes have it there. `content` rows are in the followed scroller's
    /// content coordinates; `fixed` rows (bars, overlays) in the window's.
    /// A list that builds its rows as they scroll in has only those near the
    /// viewport. Hidden and transparent subtrees, which the render server
    /// skips, are left out, and fixed layers outside the window.
    static func renderInventory(in window: UIWindow, scroller: UIScrollView?) -> [String: Any] {
        typealias Row = RenderRow
        var rows: [String: Row] = [:], order: [String] = [], layers = 0
        let content = scroller?.layer
        func walk(_ layer: CALayer, node: String, inContent: Bool) {
            guard !layer.isHidden, layer.opacity > 0 else { return }
            layers += 1
            let node = (layer.delegate as? NodeView).map { $0.props["testId"] ?? $0.props["id"].map { "#\($0)" } ?? "node \($0.id)" } ?? node
            let inContent = inContent || (content != nil && layer === content)
            let kinds = renderCosts(layer)
            if !kinds.isEmpty {
                let space = inContent ? content! : window.layer
                let frame = layer.convert(layer.bounds, to: space)
                if inContent || frame.intersects(window.bounds) {
                    for kind in kinds {
                        let place = inContent ? "content" : "fixed", key = "\(kind) \(place) \(node)"
                        if var row = rows[key] {
                            row.top = min(row.top, frame.minY); row.bottom = max(row.bottom, frame.maxY); row.count += 1
                            rows[key] = row
                        } else {
                            rows[key] = Row(kind: kind, place: place, node: node, top: frame.minY, bottom: frame.maxY, count: 1)
                            order.append(key)
                        }
                    }
                }
            }
            for sublayer in layer.sublayers ?? [] { walk(sublayer, node: node, inContent: inContent) }
        }
        walk(window.layer, node: "", inContent: false)
        let kept = collapsed(order.compactMap { rows[$0] }).prefix(renderRowsKept)
        return ["layers": layers, "rows": kept.map { [$0.kind, $0.place, $0.node, ($0.top * 10).rounded() / 10, (($0.bottom - $0.top) * 10).rounded() / 10, $0.count] as [Any] }]
    }

    struct RenderRow { let kind: String, place: String, node: String; var top: Double, bottom: Double, count: Int }

    /// `rows`, fixed then by top, a kind more than `renderNodesKept` nodes have
    /// in one place (a mask on each paragraph of a screen's text) as one
    /// row of their span and count.
    static func collapsed(_ rows: [RenderRow]) -> [RenderRow] {
        var grouped: [String: [RenderRow]] = [:]
        for row in rows { grouped["\(row.kind) \(row.place)", default: []].append(row) }
        let merged = grouped.values.flatMap { group -> [RenderRow] in
            guard group.count > renderNodesKept, let first = group.first else { return group }
            return [RenderRow(kind: first.kind, place: first.place, node: "\(group.count) nodes", top: group.map(\.top).min() ?? 0,
                              bottom: group.map(\.bottom).max() ?? 0, count: group.reduce(0) { $0 + $1.count })]
        }
        // The fixed rows first, which every frame shows, then the content's down the scroller
        let order = { (row: RenderRow) in (row.place == "fixed" ? 0 : 1, row.top, row.kind) }
        return merged.sorted { order($0) < order($1) }
    }

    static let renderRowsKept = 40
    /// The nodes a kind keeps rows of in one place before it is one row
    static let renderNodesKept = 4

    /// What `layer` itself costs the render server beyond drawing its contents.
    static func renderCosts(_ layer: CALayer) -> [String] {
        let name = NSStringFromClass(type(of: layer))
        var kinds: [String] = []
        let backdrop = name.contains("Backdrop")
        if backdrop { kinds.append("backdrop") }
        if name.contains("Portal") { kinds.append("portal") }
        if layer is CAMetalLayer { kinds.append("metal") }
        if !backdrop, layer.filters?.isEmpty == false { kinds.append("filter") }
        if let blend = layer.compositingFilter, "\(blend)" != "normalBlendMode" { kinds.append("blend") }
        if layer.mask != nil { kinds.append("mask") }
        if layer.shadowOpacity > 0, layer.shadowPath == nil { kinds.append("shadow") }
        if layer.opacity < 1, layer.allowsGroupOpacity, layer.sublayers?.isEmpty == false { kinds.append("group") }
        if layer.shouldRasterize { kinds.append("rasterize") }
        return kinds
    }
}
#endif

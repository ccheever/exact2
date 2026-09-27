// A `path` node's geometry and paint (LLP 1065), shared by UIKit and AppKit: the
// kernel's normalized path data (absolute M, L, C, Z — the host parses no
// SVG), SVG's `preserveAspectRatio` fit of a view box into a box, and the
// shape layers that draw it.
import CoreGraphics
import QuartzCore

/// A shape layer Core Animation never animates on its own: the engine is the
/// only clock, and a sublayer (unlike a view's backing layer) would otherwise
/// ease every change.
final class StillShapeLayer: CAShapeLayer {
    override func action(forKey event: String) -> CAAction? { nil }
}

/// A path's layers, hosted in its view's layer. `shape` fills, and strokes
/// between `strokeStart` and `strokeEnd`, which Core Animation measures
/// across the subpaths in order, as the web's split strokes do. A dashed
/// stroke is `dashes` instead: the whole path dashed as SVG dashes it,
/// masked by `reveal`, the trimmed undashed stroke, so the trim reveals the
/// dashes and never moves them (Core Animation's own dashing of a trimmed
/// path restarts at its start). The view box's fit is the layers'
/// transform (the mask shares its layer's), so a stroke scales with it — unevenly under
/// `preserveAspectRatio="none"`, as SVG's does; a non-scaling stroke's path
/// is mapped into the box instead, and its width and dashes stay pixels.
final class VectorLayers {
    let shape = StillShapeLayer()
    let dashes = StillShapeLayer()
    let reveal = StillShapeLayer()
    let markers = VectorMarkers()
    private weak var host: CALayer?
    private var data: String?
    private var unit: CGPath?
    private var viewBox: CGRect?
    private var aspect = "xMidYMid meet"
    private var nonScaling = false

    init(in host: CALayer) {
        self.host = host
        host.masksToBounds = true // SVG clips a path to its viewport
        host.addSublayer(shape)
        host.addSublayer(dashes)
        dashes.fillColor = nil
        dashes.mask = reveal
        reveal.fillColor = nil
        reveal.strokeColor = CGColor(gray: 0, alpha: 1)
        for layer in [shape, dashes, reveal] { layer.anchorPoint = .zero }
    }

    /// A view taken for a new node starts at identity strokes, as every
    /// presentation does (`NodePool.rebind`).
    func reset() {
        for layer in [shape, reveal] { layer.strokeStart = 0; layer.strokeEnd = 1 }
        markers.present(start: 0, end: 1)
    }

    /// A `present` op: the engine's value for this frame.
    func present(_ property: String, _ value: CGFloat) {
        let v = min(max(value, 0), 1)
        for layer in [shape, reveal] {
            if property == "stroke-start" { layer.strokeStart = v } else { layer.strokeEnd = v }
        }
        if property == "stroke-start" { markers.present(start: Double(v)) } else { markers.present(end: Double(v)) }
    }

    /// SVG's painting rows onto the layers. Absent is the initial value (the
    /// host sends only what differs): fill black, stroke none, nonzero, butt
    /// caps, miter joins, miter limit 4, no dashes. `none` and
    /// `currentcolor` cross as strings; a moving colour is `owner.paint`'s
    /// (LLP 1062), which `channels` reads first.
    func restyle(_ owner: NodeView, dark: Bool, props: [String: String]) {
        let style = owner.style
        if props["pathData"] != data {
            data = props["pathData"]
            unit = VectorPath.path(data)
        }
        viewBox = VectorPath.viewBox(props["viewBox"])
        aspect = props["preserveAspectRatio"] ?? "xMidYMid meet"
        nonScaling = style["vector_effect"]?.string == "non-scaling-stroke"
        func paint(_ key: String, _ initial: CGColor?) -> CGColor? {
            let color = { (c: [Double]) in TextEngine.color(c).cgColor }
            if let moving = owner.paint[key] { return color(moving) }
            switch style[key] {
            case nil: return initial
            case .string("currentcolor")?: return owner.channels("text_color", dark: dark).map(color)
            case let value?: return value.channels(dark: dark).map(color) // `none` is no colour
            }
        }
        shape.fillColor = paint("fill", CGColor(gray: 0, alpha: 1))
        shape.fillRule = style["fill_rule"]?.string == "evenodd" ? .evenOdd : .nonZero
        let pattern = (style["stroke_dasharray"]?.numbers ?? []).map { NSNumber(value: $0) }
        let dashed = pattern.contains { $0.doubleValue > 0 }
        let stroke = paint("stroke", nil)
        shape.strokeColor = dashed ? nil : stroke
        dashes.strokeColor = dashed ? stroke : nil
        // An odd list repeats, as SVG's does.
        dashes.lineDashPattern = dashed ? (pattern.count % 2 == 1 ? pattern + pattern : pattern) : nil
        dashes.lineDashPhase = CGFloat(style["stroke_dashoffset"]?.number ?? 0)
        let cap: CAShapeLayerLineCap = switch style["stroke_linecap"]?.string {
        case "round": .round
        case "square": .square
        default: .butt
        }
        let join: CAShapeLayerLineJoin = switch style["stroke_linejoin"]?.string {
        case "round": .round
        case "bevel": .bevel
        default: .miter
        }
        for layer in [shape, dashes, reveal] {
            layer.lineCap = cap
            layer.lineJoin = join
            layer.miterLimit = CGFloat(style["stroke_miterlimit"]?.number ?? 4)
            layer.lineWidth = CGFloat(style["stroke_width"]?.number ?? 1)
        }
        if let host { markers.update(props["markers"], in: host) }
        let current = owner.channels("text_color", dark: dark).map { TextEngine.color($0).cgColor }
        markers.paint(fill: shape.fillColor, stroke: stroke, current: current, dark: dark)
    }

    /// The view box fitted into `size`: the layers' transform, or for a
    /// non-scaling stroke the path itself.
    func place(_ host: CALayer, in size: CGSize) {
        let fit = VectorPath.fit(viewBox, aspect: aspect, in: size)
        var t = nonScaling ? fit : .identity
        let path = unit?.copy(using: &t)
        // About each layer's own origin (anchor zero): a host layer's
        // `sublayerTransform` turns about its centre on UIKit.
        for layer in [shape, dashes] { layer.setAffineTransform(nonScaling ? .identity : fit) }
        for layer in [shape, dashes, reveal] { layer.path = path }
        markers.place(fit)
    }
}

enum VectorPath {
    /// `M x y L x y C x1 y1 x2 y2 x y Z …`, as `exact_kernel::vector` writes it.
    static func path(_ text: String?) -> CGPath? {
        guard let text, !text.isEmpty else { return nil }
        let path = CGMutablePath()
        var numbers: [CGFloat] = []
        var command: Character = " "
        func flush() {
            func point(_ i: Int) -> CGPoint { CGPoint(x: numbers[i], y: numbers[i + 1]) }
            switch command {
            case "M" where numbers.count == 2: path.move(to: point(0))
            case "L" where numbers.count == 2: path.addLine(to: point(0))
            case "C" where numbers.count == 6: path.addCurve(to: point(4), control1: point(0), control2: point(2))
            case "Z": path.closeSubpath()
            default: break
            }
            numbers.removeAll(keepingCapacity: true)
        }
        for token in text.split(separator: " ") {
            if let n = Double(token) { numbers.append(CGFloat(n)); continue }
            flush()
            command = token.first ?? " "
        }
        flush()
        return path
    }

    /// `min-x min-y width height`, as the Rust host writes it.
    static func viewBox(_ text: String?) -> CGRect? {
        let n = (text ?? "").split(separator: " ").compactMap { Double($0) }
        guard n.count == 4, n[2] > 0, n[3] > 0 else { return nil }
        return CGRect(x: n[0], y: n[1], width: n[2], height: n[3])
    }

    /// The view box's user space in a box of `size` by SVG's
    /// `preserveAspectRatio` — `none`, or `x{Min,Mid,Max}Y{Min,Mid,Max}`
    /// then `meet` or `slice`, as the Rust host writes it canonically. No
    /// view box is the identity: path units are points.
    static func fit(_ viewBox: CGRect?, aspect: String = "xMidYMid meet", in size: CGSize) -> CGAffineTransform {
        guard let v = viewBox else { return .identity }
        var sx = max(0, size.width / v.width), sy = max(0, size.height / v.height)
        let words = aspect.split(separator: " ")
        guard let align = words.first, align != "none", align.count == 8 else {
            return CGAffineTransform(a: sx, b: 0, c: 0, d: sy, tx: -v.minX * sx, ty: -v.minY * sy)
        }
        let scale = words.last == "slice" ? max(sx, sy) : min(sx, sy)
        (sx, sy) = (scale, scale)
        func at(_ name: Substring) -> CGFloat { name == "Min" ? 0 : name == "Mid" ? 0.5 : 1 }
        let x = at(align.dropFirst(1).prefix(3)), y = at(align.dropFirst(5).prefix(3))
        return CGAffineTransform(a: sx, b: 0, c: 0, d: sy,
                                 tx: (size.width - v.width * sx) * x - v.minX * sx,
                                 ty: (size.height - v.height * sy) * y - v.minY * sy)
    }
}

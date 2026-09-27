// A `path` node's geometry and paint (LLP 1065), shared by UIKit and AppKit: the
// kernel's normalized path data (absolute M, L, C, Z — the host parses no
// SVG) and SVG's `xMidYMid meet` fit of a view box into a box.
import CoreGraphics
import QuartzCore

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

    /// SVG's painting rows onto a shape layer. Absent is the initial value
    /// (the host sends only what differs): fill black, stroke none, butt
    /// caps, miter joins, miter limit 4. `none` crosses as a string. Returns
    /// the stroke width, in path units.
    static func paint(_ shape: CAShapeLayer, _ style: NodeStyle, dark: Bool) -> CGFloat {
        func paint(_ key: String, _ initial: CGColor?) -> CGColor? {
            guard let value = style[key] else { return initial }
            return value.channels(dark: dark).map { TextEngine.color($0).cgColor }
        }
        shape.fillColor = paint("fill", CGColor(gray: 0, alpha: 1))
        shape.strokeColor = paint("stroke", nil)
        switch style["stroke_linecap"]?.string {
        case "round": shape.lineCap = .round
        case "square": shape.lineCap = .square
        default: shape.lineCap = .butt
        }
        switch style["stroke_linejoin"]?.string {
        case "round": shape.lineJoin = .round
        case "bevel": shape.lineJoin = .bevel
        default: shape.lineJoin = .miter
        }
        shape.miterLimit = 4
        return CGFloat(style["stroke_width"]?.number ?? 1)
    }

    /// The path fitted into `size`, its stroke width scaled with it.
    static func place(_ shape: CAShapeLayer, _ unit: CGPath?, viewBox: CGRect?, width: CGFloat, in size: CGSize) {
        var t = fit(viewBox, in: size)
        shape.path = unit?.copy(using: &t)
        shape.lineWidth = width * sqrt(abs(t.a * t.d - t.b * t.c))
    }

    /// The view box's user space in a box of `size`: uniform scale, centred.
    /// No view box is the identity — path units are points.
    static func fit(_ viewBox: CGRect?, in size: CGSize) -> CGAffineTransform {
        guard let v = viewBox else { return .identity }
        let scale = max(0, min(size.width / v.width, size.height / v.height))
        return CGAffineTransform(
            translationX: (size.width - v.width * scale) / 2 - v.minX * scale,
            y: (size.height - v.height * scale) / 2 - v.minY * scale
        ).scaledBy(x: scale, y: scale)
    }
}

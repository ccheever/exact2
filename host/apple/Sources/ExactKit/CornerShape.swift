// CSS `corner-shape` and `-exact-continuous` (LLP 1077 D1), shared by UIKit
// and AppKit. The outline is the kernel's (`exact_corner_outline`), so every
// host draws one shape for one name; the one case Core Animation says
// itself is `-exact-continuous` with one radius, `cornerCurve = .continuous`.
import CExact
import CoreGraphics
import QuartzCore

package struct CornerShape: Equatable {
    /// Each corner's superellipse K, top-left first; NaN is
    /// `-exact-continuous`.
    let k: [Float]

    /// The row, or nil when every corner is `round` (no row is sent).
    init?(_ value: BatchValue?) {
        guard let corners = value?.array, corners.count == 4 else { return nil }
        k = corners.map { c in
            switch c.string {
            case "apple": return .nan
            case "inf": return .infinity
            case "-inf": return -.infinity
            default: return Float(c.number ?? 1)
            }
        }
    }

    package static func == (a: CornerShape, b: CornerShape) -> Bool {
        zip(a.k, b.k).allSatisfy { $0 == $1 || ($0.isNaN && $1.isNaN) }
    }

    /// Every corner Apple's: the layer can draw it when the radius is one.
    var isAppleContinuous: Bool { k.allSatisfy { $0.isNaN } }

    /// The border box `rect` with these corners, `radii` already reduced as
    /// CSS reduces them, clockwise on screen.
    func outline(_ rect: CGRect, _ radii: [CGSize]) -> CGMutablePath {
        let path = CGMutablePath()
        guard rect.width > 0, rect.height > 0 else { return path }
        let r = radii.flatMap { [Float($0.width), Float($0.height)] }
        let (x, y, w, h) = (Float(rect.minX), Float(rect.minY), Float(rect.width), Float(rect.height))
        let n = exact_corner_outline(k, x, y, w, h, r, nil, 0)
        guard n > 0 else { path.addRect(rect); return path }
        var points = [Float](repeating: 0, count: n * 2)
        _ = exact_corner_outline(k, x, y, w, h, r, &points, n)
        path.move(to: CGPoint(x: CGFloat(points[0]), y: CGFloat(points[1])))
        for i in 1..<n { path.addLine(to: CGPoint(x: CGFloat(points[2 * i]), y: CGFloat(points[2 * i + 1]))) }
        path.closeSubpath()
        return path
    }
}

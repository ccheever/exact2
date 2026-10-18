// A `background-image` gradient as the batch carries it (LLP 1066; the
// emitter is `host/apple/src/gradient.rs`, the reading mirrors
// `ExactKit/Gradient.swift`): stops flat as `t, r, g, b, a` (positions
// 0–1; channels 0–255, or extended linear sRGB floats with alpha 0–1 when
// `space` is `srgb-linear`, LLP 1100 D2), a `dark` set when the colours
// pair, and the shape: `linear` degrees, `corner` `[right, bottom]`,
// `radial` `[circle, extent, x%, xpx, y%, ypx]`. A gradient with length
// stops arrives as its CSS (`{"css": …}`) and the kernel places it once the
// box is known (`exact_gradient`), answering `line` or `ellipse` in box
// coordinates. The vocabulary admits linear and radial gradients with
// strictly increasing stops; the profile refuses conic, repeating and
// coincident stops.
#if canImport(WidgetKit)
import SwiftUI
import CExact
import ExactCore

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct GradientRow {
    enum Shape { case angle(CGFloat), corner(right: Bool, bottom: Bool), radial(circle: Bool, extent: Int, at: [CGFloat]) }
    enum Placement { case axial(CGPoint, CGPoint), radial(CGPoint, CGSize) }

    let css: String?
    let light: [CGFloat]
    let dark: [CGFloat]?
    let lightLinear: Bool
    let darkLinear: Bool
    let shape: Shape
    /// From the kernel, for a `css` gradient it resolved.
    let resolved: Placement?

    init?(_ value: BatchValue?) {
        guard case .object(let object)? = value else { return nil }
        if let css = object["css"]?.string {
            guard !css.isEmpty else { return nil }
            self.css = css; light = []; dark = nil; lightLinear = false; darkLinear = false; shape = .angle(180); resolved = nil
            return
        }
        let numbers = { (key: String) -> [CGFloat]? in object[key]?.numbers?.map { CGFloat($0) } }
        guard let light = numbers("stops"), light.count >= 10, light.count % 5 == 0 else { return nil }
        css = nil
        self.light = light
        dark = numbers("dark")
        lightLinear = object["space"]?.string == "srgb-linear"
        darkLinear = object["darkSpace"]?.string == "srgb-linear"
        if let p = numbers("line"), p.count == 4 { resolved = .axial(CGPoint(x: p[0], y: p[1]), CGPoint(x: p[2], y: p[3])) }
        else if let p = numbers("ellipse"), p.count == 4 { resolved = .radial(CGPoint(x: p[0], y: p[1]), CGSize(width: p[2], height: p[3])) }
        else { resolved = nil }
        if let degrees = object["linear"]?.number { shape = .angle(CGFloat(degrees)) }
        else if let c = numbers("corner"), c.count == 2 { shape = .corner(right: c[0] != 0, bottom: c[1] != 0) }
        else if let r = numbers("radial"), r.count == 6 { shape = .radial(circle: r[0] != 0, extent: Int(r[1]), at: Array(r[2...])) }
        else { return nil }
    }

    /// Whether the ramp shown in an appearance crosses as linear sRGB, which
    /// the shading then interpolates in (`.linearColor`).
    func isLinear(dark isDark: Bool) -> Bool { isDark && dark != nil ? darkLinear : lightLinear }

    /// The stops under an appearance, as SwiftUI colours: the sRGB ramp, or
    /// the linear one in its own space.
    func stops(dark isDark: Bool) -> [Gradient.Stop] {
        let s = isDark ? dark ?? light : light
        let linear = isLinear(dark: isDark)
        var stops: [Gradient.Stop] = []
        for i in stride(from: 0, to: s.count, by: 5) {
            let color = linear ? Color(.sRGBLinear, red: s[i + 1], green: s[i + 2], blue: s[i + 3], opacity: s[i + 4])
                : Color(.sRGB, red: s[i + 1] / 255, green: s[i + 2] / 255, blue: s[i + 3] / 255, opacity: s[i + 4] / 255)
            stops.append(Gradient.Stop(color: color, location: s[i]))
        }
        return stops
    }

    /// A `css` gradient resolved by the kernel against the box; nil when
    /// this one needs no resolution or the kernel refuses it.
    func resolving(in box: CGRect, dark: Bool) -> GradientRow? {
        guard let css else { return nil }
        return Owner.shared.sync { () -> GradientRow? in
            let input = Array(css.utf8), w = Float(box.width), h = Float(box.height)
            let n = exact_gradient(input, input.count, w, h, dark ? 1 : 0, nil, 0)
            guard n > 0 else { return nil }
            var bytes = [UInt8](repeating: 0, count: n)
            _ = exact_gradient(input, input.count, w, h, dark ? 1 : 0, &bytes, n)
            return bytes.withUnsafeBufferPointer { buffer -> GradientRow? in
                var reader = BatchReader(bytes: buffer)
                return (try? reader.value()).flatMap { GradientRow($0) }
            }
        }
    }

    /// Where the gradient falls in `box` (CSS's gradient box), in points,
    /// as ExactKit places it (CSS Images 3 §3.1.1, §3.2.2).
    func placement(in box: CGRect) -> Placement {
        if let resolved {
            switch resolved {
            case .axial(let a, let b): return .axial(CGPoint(x: a.x + box.minX, y: a.y + box.minY), CGPoint(x: b.x + box.minX, y: b.y + box.minY))
            case .radial(let c, let r): return .radial(CGPoint(x: c.x + box.minX, y: c.y + box.minY), r)
            }
        }
        let (w, h) = (max(box.width, 0), max(box.height, 0))
        switch shape {
        case .angle, .corner:
            var (sin, cos): (CGFloat, CGFloat)
            if case .angle(let deg) = shape {
                (sin, cos) = (Foundation.sin(deg * .pi / 180), Foundation.cos(deg * .pi / 180))
            } else if case .corner(let right, let bottom) = shape {
                let (x, y) = (right ? h : -h, bottom ? w : -w), n = hypot(x, y)
                (sin, cos) = n == 0 ? (0, 1) : (x / n, -y / n)
            } else { (sin, cos) = (0, 1) }
            let half = (w * abs(sin) + h * abs(cos)) / 2
            return .axial(CGPoint(x: box.midX - sin * half, y: box.midY + cos * half), CGPoint(x: box.midX + sin * half, y: box.midY - cos * half))
        case .radial(let circle, let extent, let at):
            let c = CGPoint(x: box.minX + w * at[0] / 100 + at[1], y: box.minY + h * at[2] / 100 + at[3])
            let dx = [abs(c.x - box.minX), abs(box.minX + w - c.x)], dy = [abs(c.y - box.minY), abs(box.minY + h - c.y)]
            let near = CGSize(width: dx.min()!, height: dy.min()!), far = CGSize(width: dx.max()!, height: dy.max()!)
            let corners = dx.flatMap { x in dy.map { hypot(x, $0) } }
            let r: CGSize
            switch (circle, extent) {
            case (true, 0): let n = min(near.width, near.height); r = CGSize(width: n, height: n)
            case (true, 1): let n = corners.min()!; r = CGSize(width: n, height: n)
            case (true, 2): let n = max(far.width, far.height); r = CGSize(width: n, height: n)
            case (true, _): let n = corners.max()!; r = CGSize(width: n, height: n)
            case (false, 0): r = near
            case (false, 1): r = CGSize(width: near.width * 2.squareRoot(), height: near.height * 2.squareRoot())
            case (false, 2): r = far
            default: r = CGSize(width: far.width * 2.squareRoot(), height: far.height * 2.squareRoot())
            }
            return .radial(c, r)
        }
    }
}

/// The gradient layers of a box, the first on top, drawn in one `Canvas`
/// (which WidgetKit archives) in point space: an axial ramp between its
/// two points, a radial as a circle in a context scaled to the ellipse.
/// The canvas is the border box; `box` is the padding box the gradients
/// are positioned on, in its coordinates.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotGradients: View {
    let layers: [GradientRow]
    let box: CGRect
    let dark: Bool

    var body: some View {
        Canvas { context, size in
            let whole = Path(CGRect(origin: .zero, size: size))
            for row in layers.reversed() {
                let gradient = row.resolving(in: CGRect(origin: .zero, size: box.size), dark: dark) ?? row
                let stops = Gradient(stops: gradient.stops(dark: dark))
                let options: GraphicsContext.GradientOptions = gradient.isLinear(dark: dark) ? .linearColor : []
                switch gradient.placement(in: box) {
                case .axial(let a, let b):
                    context.fill(whole, with: .linearGradient(stops, startPoint: a, endPoint: b, options: options))
                case .radial(let c, let r):
                    guard r.width > 0, r.height > 0 else {
                        // CSS: a radial with no area is its last colour everywhere.
                        if let last = stops.stops.last { context.fill(whole, with: .color(last.color)) }
                        continue
                    }
                    context.drawLayer { layer in
                        layer.translateBy(x: c.x, y: c.y)
                        layer.scaleBy(x: r.width, y: r.height)
                        let reach = (size.width + size.height) / min(r.width, r.height) + 2
                        layer.fill(Path(CGRect(x: -reach, y: -reach, width: reach * 2, height: reach * 2)),
                                   with: .radialGradient(stops, center: .zero, startRadius: 0, endRadius: 1, options: options))
                    }
                }
            }
        }
    }
}
#endif

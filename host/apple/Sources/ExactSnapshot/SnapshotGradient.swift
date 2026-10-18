// A `background-image` gradient as the batch carries it (LLP 1066; the
// emitter is `host/apple/src/gradient.rs`, the reading mirrors
// `ExactKit/Gradient.swift`): stops flat as `t, r, g, b, a` (positions
// 0–1; channels 0–255, or extended linear sRGB floats with alpha 0–1 when
// `space` is `srgb-linear`, LLP 1100 D2), a `dark` set when the colours
// pair, and the shape: `linear` degrees, `corner` `[right, bottom]`,
// `radial` `[circle, extent, x%, xpx, y%, ypx]`, `conic` `[from, x%, xpx,
// y%, ypx]`. A gradient with length stops or repetition arrives as its CSS
// (`{"css": …}`) and the kernel places it once the box is known
// (`exact_gradient`), answering `line` or `ellipse` in box coordinates.
#if canImport(WidgetKit)
import SwiftUI
import CExact
import ExactCore

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct GradientRow {
    enum Shape { case angle(CGFloat), corner(right: Bool, bottom: Bool), radial(circle: Bool, extent: Int, at: [CGFloat]), conic(from: CGFloat, at: [CGFloat]) }
    enum Placement { case axial(CGPoint, CGPoint), radial(CGPoint, CGSize), conic(CGPoint, CGFloat) }

    let css: String?
    let light: [CGFloat]
    let dark: [CGFloat]?
    let lightLinear: Bool
    let darkLinear: Bool
    let shape: Shape
    /// From the kernel, for a `css` gradient it resolved.
    let resolved: Placement?

    init?(_ value: Any?) {
        guard let object = value as? [String: Any] else { return nil }
        if let css = object["css"] as? String {
            guard !css.isEmpty else { return nil }
            self.css = css; light = []; dark = nil; lightLinear = false; darkLinear = false; shape = .angle(180); resolved = nil
            return
        }
        let numbers = { (key: String) -> [CGFloat]? in (object[key] as? [Any])?.compactMap { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } } }
        guard let light = numbers("stops"), light.count >= 10, light.count % 5 == 0 else { return nil }
        css = nil
        self.light = light
        dark = numbers("dark")
        lightLinear = object["space"] as? String == "srgb-linear"
        darkLinear = object["darkSpace"] as? String == "srgb-linear"
        if let p = numbers("line"), p.count == 4 { resolved = .axial(CGPoint(x: p[0], y: p[1]), CGPoint(x: p[2], y: p[3])) }
        else if let p = numbers("ellipse"), p.count == 4 { resolved = .radial(CGPoint(x: p[0], y: p[1]), CGSize(width: p[2], height: p[3])) }
        else { resolved = nil }
        if let degrees = (object["linear"] as? NSNumber).map({ CGFloat($0.doubleValue) }) { shape = .angle(degrees) }
        else if let c = numbers("corner"), c.count == 2 { shape = .corner(right: c[0] != 0, bottom: c[1] != 0) }
        else if let r = numbers("radial"), r.count == 6 { shape = .radial(circle: r[0] != 0, extent: Int(r[1]), at: Array(r[2...])) }
        else if let c = numbers("conic"), c.count == 5 { shape = .conic(from: c[0], at: Array(c[1...])) }
        else { return nil }
    }

    /// The stops under an appearance, as SwiftUI colours.
    func stops(dark isDark: Bool) -> [Gradient.Stop] {
        let s = isDark ? dark ?? light : light
        let linear = isDark && dark != nil ? darkLinear : lightLinear
        var stops: [Gradient.Stop] = []
        for i in stride(from: 0, to: s.count, by: 5) {
            let color: Color
            if linear, let space = CGColorSpace(name: CGColorSpace.extendedLinearSRGB), let cg = CGColor(colorSpace: space, components: Array(s[i + 1..<i + 5])) {
                color = Color(cgColor: cg)
            } else {
                color = Color(.sRGB, red: s[i + 1] / 255, green: s[i + 2] / 255, blue: s[i + 3] / 255, opacity: s[i + 4] / 255)
            }
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
                return (try? reader.value()).flatMap { GradientRow($0.any) }
            }
        }
    }

    /// Where the gradient falls in `box` (CSS's gradient box, the padding
    /// box), as ExactKit places it.
    func placement(in box: CGRect) -> Placement {
        if let resolved {
            switch resolved {
            case .axial(let a, let b): return .axial(CGPoint(x: a.x + box.minX, y: a.y + box.minY), CGPoint(x: b.x + box.minX, y: b.y + box.minY))
            case .radial(let c, let r): return .radial(CGPoint(x: c.x + box.minX, y: c.y + box.minY), r)
            case .conic: return resolved
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
        case .conic(let from, let at):
            return .conic(CGPoint(x: box.minX + w * at[0] / 100 + at[1], y: box.minY + h * at[2] / 100 + at[3]), from)
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

/// One gradient layer painted over its box: an axial one as a
/// `LinearGradient`; a radial (an ellipse: the context scaled to it) or a
/// conic one drawn in a `Canvas`, which WidgetKit archives.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotGradient: View {
    let row: GradientRow
    let box: CGRect
    let dark: Bool

    var body: some View {
        let gradient = row.resolving(in: CGRect(origin: .zero, size: box.size), dark: dark) ?? row
        let local = CGRect(origin: .zero, size: box.size)
        let stops = Gradient(stops: gradient.stops(dark: dark))
        switch gradient.placement(in: local) {
        case .axial(let a, let b) where box.width > 0 && box.height > 0:
            LinearGradient(gradient: stops, startPoint: UnitPoint(x: a.x / box.width, y: a.y / box.height), endPoint: UnitPoint(x: b.x / box.width, y: b.y / box.height))
        case .radial(let c, let r):
            Canvas { context, size in
                let rect = CGRect(origin: .zero, size: size)
                guard r.width > 0, r.height > 0 else {
                    if let last = stops.stops.last { context.fill(Path(rect), with: .color(last.color)) }
                    return
                }
                context.translateBy(x: c.x, y: c.y)
                context.scaleBy(x: r.width, y: r.height)
                let reach = max(size.width, size.height) / min(r.width, r.height) + 2
                context.fill(Path(CGRect(x: -reach, y: -reach, width: reach * 2, height: reach * 2)),
                             with: .radialGradient(stops, center: .zero, startRadius: 0, endRadius: 1))
            }
        case .conic(let c, let from):
            Canvas { context, size in
                context.fill(Path(CGRect(origin: .zero, size: size)), with: .conicGradient(stops, center: c, angle: .degrees(from)))
            }
        default:
            EmptyView()
        }
    }
}
#endif

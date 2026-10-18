// A node's style rows as SwiftUI takes them: colours as roles (D4: never a
// resolved colour, so the rendering modes can tint them), the kernel's
// fonts, the box's shape, fill and border. `SnapshotStyle` is the one place
// a row is read off the headless model, so its wire shape is known here alone.
#if canImport(WidgetKit)
import SwiftUI
import ExactCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// A colour row: four channels (0–255), a `[light, dark]` pair, a platform
/// colour by name with its fallback pair (`{"sys": …, "c": …}`), or a
/// colour in its own space (`{"cs": …, "c": …}`), whose sRGB fallback shows.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
indirect enum ColorRow: Equatable {
    case channels([Double])
    case pair(light: [Double], dark: [Double])
    case platform(name: String, fallback: ColorRow?)
    case wide(fallback: ColorRow?)

    init?(_ value: Any?) {
        if let object = value as? [String: Any] {
            if let name = object["sys"] as? String { self = .platform(name: name, fallback: ColorRow(object["c"])) }
            else if object["cs"] != nil { self = .wide(fallback: ColorRow(object["c"])) }
            else { return nil }
            return
        }
        guard let array = value as? [Any] else { return nil }
        let numbers = { (list: [Any]) -> [Double]? in
            let c = list.compactMap { ($0 as? NSNumber)?.doubleValue }
            return c.count == 4 ? c : nil
        }
        if array.count == 2, let light = (array[0] as? [Any]).flatMap(numbers), let dark = (array[1] as? [Any]).flatMap(numbers) {
            self = .pair(light: light, dark: dark)
        } else if let c = numbers(array) {
            self = .channels(c)
        } else { return nil }
    }

    /// Whether the row names the view's tint (`AccentColor`, a link's
    /// colour, an unsaid symbol's): the accented group's colour (D5).
    var namesTint: Bool {
        if case .platform(let name, _) = self { return name.hasPrefix("@tint") }
        return false
    }
}

/// A `background-image` gradient as the batch carries it (LLP 1066): the
/// stops, five numbers each (r, g, b, a, position 0–1), a dark set when the
/// colours pair, and the shape: an angle, a corner, a radial, a conic. A
/// length-bearing one arrives as its CSS and is left unpainted: the kernel
/// resolves it against the box in a running host.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct GradientRow {
    enum Shape { case angle(CGFloat), corner(right: Bool, bottom: Bool), radial(at: [CGFloat]), conic(from: CGFloat, at: [CGFloat]) }
    let light: [CGFloat]
    let dark: [CGFloat]?
    let shape: Shape

    init?(_ value: Any?) {
        guard let object = value as? [String: Any], object["css"] == nil else { return nil }
        let numbers = { (key: String) -> [CGFloat]? in (object[key] as? [Any])?.compactMap { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } } }
        guard let light = numbers("stops"), light.count >= 10, light.count % 5 == 0 else { return nil }
        self.light = light
        dark = numbers("dark")
        if let degrees = (object["linear"] as? NSNumber).map({ CGFloat($0.doubleValue) }) { shape = .angle(degrees) }
        else if let c = numbers("corner"), c.count == 2 { shape = .corner(right: c[0] != 0, bottom: c[1] != 0) }
        else if let r = numbers("radial"), r.count == 6 { shape = .radial(at: Array(r[2...])) }
        else if let c = numbers("conic"), c.count == 5 { shape = .conic(from: c[0], at: Array(c[1...])) }
        else { return nil }
    }
}

/// A node's style rows, read here alone.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotStyle {
    private let rows: [String: Any]
    init(_ node: HeadlessSession.Node) { rows = node.style }

    func has(_ key: String) -> Bool { rows[key] != nil }
    func number(_ key: String) -> CGFloat? { (rows[key] as? NSNumber).map { CGFloat($0.doubleValue) } }
    func string(_ key: String) -> String? { rows[key] as? String }
    /// Points, or a length against a basis (`{"px", "pct"}`).
    func length(_ key: String, basis: CGFloat) -> CGFloat? {
        if let n = number(key) { return n }
        guard let object = rows[key] as? [String: Any] else { return nil }
        return CGFloat((object["pct"] as? NSNumber)?.doubleValue ?? 0) * basis / 100 + CGFloat((object["px"] as? NSNumber)?.doubleValue ?? 0)
    }
    func color(_ key: String) -> ColorRow? { ColorRow(rows[key]) }
    /// A `background-image`'s layers, the first on top.
    func gradients(_ key: String) -> [GradientRow] {
        if let list = rows[key] as? [Any] { return list.compactMap(GradientRow.init) }
        return GradientRow(rows[key]).map { [$0] } ?? []
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
enum SnapshotColor {
    enum Context { case ink, fill }

    static func style(_ row: ColorRow?, dark: Bool, context: Context) -> AnyShapeStyle? {
        switch row {
        case .platform(let name, let fallback): return named(name, context: context) ?? style(fallback, dark: dark, context: context)
        case .wide(let fallback): return style(fallback, dark: dark, context: context)
        case .channels, .pair: return color(row, dark: dark).map(AnyShapeStyle.init)
        case nil: return nil
        }
    }

    static func color(_ row: ColorRow?, dark: Bool) -> Color? {
        switch row {
        case .channels(let c): return channels(c)
        case .pair(let light, let dark_): return channels(dark ? dark_ : light)
        case .platform(let name, let fallback): return platformColor(name) ?? color(fallback, dark: dark)
        case .wide(let fallback): return color(fallback, dark: dark)
        case nil: return nil
        }
    }

    private static func channels(_ c: [Double]) -> Color? {
        guard c.count == 4 else { return nil }
        return Color(.sRGB, red: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, opacity: c[3] / 255)
    }

    /// A platform colour by its name, as a role the modes can tint: the
    /// label colours as the hierarchical foreground styles, the system fills
    /// as the hierarchical fills, the tint as the accent, and any other
    /// `UIColor`/`NSColor` class colour as the dynamic colour it names.
    private static func named(_ name: String, context: Context) -> AnyShapeStyle? {
        if name.hasPrefix("@tint") {
            let alpha = name.contains("/") ? Double(name.split(separator: "/").last ?? "") ?? 1 : 1
            return AnyShapeStyle(Color.accentColor.opacity(alpha))
        }
        switch (context, name) {
        case (.ink, "labelColor"), (.ink, "textColor"): return AnyShapeStyle(.primary)
        case (.ink, "secondaryLabelColor"): return AnyShapeStyle(.secondary)
        case (.ink, "tertiaryLabelColor"): return AnyShapeStyle(.tertiary)
        case (.ink, "quaternaryLabelColor"): return AnyShapeStyle(.quaternary)
        case (.fill, "systemFillColor"): return AnyShapeStyle(.primary)
        case (.fill, "secondarySystemFillColor"): return AnyShapeStyle(.secondary)
        case (.fill, "tertiarySystemFillColor"): return AnyShapeStyle(.tertiary)
        case (.fill, "quaternarySystemFillColor"): return AnyShapeStyle(.quaternary)
        case (.fill, "systemBackgroundColor"), (.fill, "textBackgroundColor"), (.fill, "windowBackgroundColor"): return AnyShapeStyle(.background)
        default: return platformColor(name).map(AnyShapeStyle.init)
        }
    }

    /// The dynamic colour a name looks up: an asset (`named:`), or a
    /// zero-argument class method of `UIColor`/`NSColor` returning a colour
    /// (the name is checked at the bake; no private selector is reachable).
    private static func platformColor(_ name: String) -> Color? {
        if name.hasPrefix("named:") { return Color(String(name.dropFirst(6)), bundle: .main) }
        guard name.hasSuffix("Color"), !name.hasPrefix("_"), !name.hasPrefix("@") else { return nil }
        #if canImport(UIKit)
        typealias Native = UIColor
        #else
        typealias Native = NSColor
        #endif
        let selector = NSSelectorFromString(name)
        guard let method = class_getClassMethod(Native.self, selector), method_getNumberOfArguments(method) == 2 else { return nil }
        let returns = method_copyReturnType(method)
        defer { free(returns) }
        guard String(cString: returns) == "@", let value = (Native.self as AnyObject).perform(selector)?.takeUnretainedValue() as? Native else { return nil }
        #if canImport(UIKit)
        return Color(uiColor: value)
        #else
        return Color(nsColor: value)
        #endif
    }
}

/// A box's paint: its corners, fill (colour and gradients) and border, from
/// its style rows, as the web paints them.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotPaint {
    let style: SnapshotStyle
    let dark: Bool

    /// Each corner's radius, reduced as CSS reduces them so neighbours never
    /// overlap, top-left first.
    func radii(in rect: CGRect) -> [CGSize] {
        let raw = ["top_left", "top_right", "bottom_right", "bottom_left"].map { name -> CGSize in
            let key = style.has("border_radius_" + name) ? "border_radius_" + name : "border_radius"
            return CGSize(width: max(0, style.length(key, basis: rect.width) ?? 0), height: max(0, style.length(key, basis: rect.height) ?? 0))
        }
        let sums = [raw[0].width + raw[1].width, raw[3].width + raw[2].width, raw[0].height + raw[3].height, raw[1].height + raw[2].height]
        let edges = [rect.width, rect.width, rect.height, rect.height]
        var factor: CGFloat = 1
        for i in 0..<4 where sums[i] > 0 { factor = min(factor, max(0, edges[i]) / sums[i]) }
        return raw.map { CGSize(width: $0.width * factor, height: $0.height * factor) }
    }

    /// The border box's shape: its rounded corners, or the rectangle.
    func shape(in rect: CGRect) -> AnyShape {
        let r = radii(in: rect)
        guard r.contains(where: { $0.width > 0 || $0.height > 0 }) else { return AnyShape(Rectangle()) }
        return AnyShape(UnevenRoundedRectangle(cornerRadii: RectangleCornerRadii(
            topLeading: r[0].width, bottomLeading: r[3].width, bottomTrailing: r[2].width, topTrailing: r[1].width)))
    }

    var hasBackground: Bool {
        !style.gradients("background_image").isEmpty || SnapshotColor.style(style.color("background_color"), dark: dark, context: .fill) != nil
    }

    /// The fill: the colour, then each gradient layer, the first on top.
    @ViewBuilder
    func fill(in rect: CGRect) -> some View {
        let shape = shape(in: rect)
        ZStack {
            if let color = SnapshotColor.style(style.color("background_color"), dark: dark, context: .fill) {
                shape.fill(color)
            }
            ForEach(Array(style.gradients("background_image").reversed().enumerated()), id: \.offset) { _, gradient in
                shape.fill(SnapshotGradient.style(gradient, in: rect, dark: dark))
            }
        }
    }

    /// The border: a uniform one stroked inside the shape; sides that differ
    /// as four strips, each in its own colour.
    @ViewBuilder
    func border(in rect: CGRect) -> some View {
        let sides = ["top", "right", "bottom", "left"]
        let widths = sides.map { style.number("border_width_" + $0) ?? 0 }
        let styles = sides.map { style.string("border_style_" + $0) ?? "none" }
        let colors = sides.map { SnapshotColor.style(style.color("border_color_" + $0) ?? style.color("text_color"), dark: dark, context: .fill) }
        let drawn = (0..<4).map { widths[$0] > 0 && styles[$0] != "none" && styles[$0] != "hidden" }
        if drawn.contains(true) {
            let uniform = drawn.allSatisfy { $0 } && widths.allSatisfy { $0 == widths[0] }
            if uniform, let color = colors[0] {
                // Inside the border box: the stroke straddles the outline, so
                // twice the width clipped to the shape.
                let shape = shape(in: rect)
                shape.stroke(color, lineWidth: widths[0] * 2).clipShape(shape)
            } else {
                ZStack(alignment: .topLeading) {
                    if drawn[0], let c = colors[0] { Rectangle().fill(c).frame(width: rect.width, height: widths[0]) }
                    if drawn[2], let c = colors[2] { Rectangle().fill(c).frame(width: rect.width, height: widths[2]).offset(y: rect.height - widths[2]) }
                    if drawn[3], let c = colors[3] { Rectangle().fill(c).frame(width: widths[3], height: rect.height) }
                    if drawn[1], let c = colors[1] { Rectangle().fill(c).frame(width: widths[1], height: rect.height).offset(x: rect.width - widths[1]) }
                }
            }
        }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
enum SnapshotGradient {
    private static func point(_ at: [CGFloat], in rect: CGRect) -> UnitPoint {
        guard at.count == 4, rect.width > 0, rect.height > 0 else { return .center }
        return UnitPoint(x: at[0] / 100 + at[1] / rect.width, y: at[2] / 100 + at[3] / rect.height)
    }

    static func style(_ row: GradientRow, in rect: CGRect, dark: Bool) -> AnyShapeStyle {
        let raw = dark ? (row.dark ?? row.light) : row.light
        let gradient = Gradient(stops: stride(from: 0, to: raw.count, by: 5).map {
            Gradient.Stop(color: Color(.sRGB, red: raw[$0] / 255, green: raw[$0 + 1] / 255, blue: raw[$0 + 2] / 255, opacity: raw[$0 + 3] / 255), location: raw[$0 + 4])
        })
        switch row.shape {
        case .angle(let degrees):
            // CSS: 0° points up, clockwise; the gradient line's ends are where
            // the perpendiculars through the corners cross it (CSS Images 3 §3.1.1).
            let theta = degrees * .pi / 180
            let length = abs(sin(theta)) * rect.width + abs(cos(theta)) * rect.height
            let dx = sin(theta) * length / 2, dy = -cos(theta) * length / 2
            let start = UnitPoint(x: 0.5 - dx / max(rect.width, 1), y: 0.5 - dy / max(rect.height, 1))
            let end = UnitPoint(x: 0.5 + dx / max(rect.width, 1), y: 0.5 + dy / max(rect.height, 1))
            return AnyShapeStyle(LinearGradient(gradient: gradient, startPoint: start, endPoint: end))
        case .corner(let right, let bottom):
            return AnyShapeStyle(LinearGradient(gradient: gradient, startPoint: UnitPoint(x: right ? 0 : 1, y: bottom ? 0 : 1),
                                                endPoint: UnitPoint(x: right ? 1 : 0, y: bottom ? 1 : 0)))
        case .radial(let at):
            let center = point(at, in: rect)
            let corner = max(hypot(center.x * rect.width, center.y * rect.height), hypot((1 - center.x) * rect.width, (1 - center.y) * rect.height))
            return AnyShapeStyle(RadialGradient(gradient: gradient, center: center, startRadius: 0, endRadius: corner))
        case .conic(let from, let at):
            return AnyShapeStyle(AngularGradient(gradient: gradient, center: point(at, in: rect), angle: .degrees(from - 90)))
        }
    }
}

extension Font.Weight {
    /// CSS's numeric weight as the nearest of the nine.
    init(css weight: Int) {
        let weights: [Font.Weight] = [.ultraLight, .thin, .light, .regular, .medium, .semibold, .bold, .heavy, .black]
        self = weights[min(8, max(0, Int((Double(weight) / 100).rounded()) - 1))]
    }
}
#endif

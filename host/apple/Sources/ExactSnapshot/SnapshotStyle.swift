// A node's style rows as SwiftUI takes them: colours as roles (D4: never a
// resolved colour, so the rendering modes can tint them), the box's shape,
// fill and border. `SnapshotStyle` is the one place a row is read off the
// headless model, so its wire shape is known here alone.
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
    /// CSS `clip-path`, as the kernel validated it: its fill rule and path commands.
    func clipPath(_ key: String) -> ClipPathRow? { ClipPathRow(rows[key]) }
    var visible: Bool { string("visibility") != "hidden" }
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
    /// label colours as the hierarchical foreground styles, the tint as the
    /// accent, and any other `UIColor`/`NSColor` class colour (the system
    /// fills among them) as the dynamic colour it names.
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
        case (.fill, "systemBackgroundColor"), (.fill, "textBackgroundColor"), (.fill, "windowBackgroundColor"): return AnyShapeStyle(.background)
        default: return platformColor(name).map(AnyShapeStyle.init)
        }
    }

    /// The dynamic colour a name looks up: an asset (`named:`, in this
    /// bundle or the app's when this is its extension), or a zero-argument
    /// class method of `UIColor`/`NSColor` returning a colour (the name is
    /// checked at the bake; no private selector is reachable). Nil leaves
    /// the row's fallback pair to show.
    private static func platformColor(_ name: String) -> Color? {
        #if canImport(UIKit)
        typealias Native = UIColor
        #else
        typealias Native = NSColor
        #endif
        if name.hasPrefix("named:") {
            let asset = String(name.dropFirst(6))
            for bundle in SnapshotBundles.all {
                #if os(watchOS)
                if bundle == Bundle.main, let color = UIColor(named: asset) { return Color(uiColor: color) }
                #elseif canImport(UIKit)
                if let color = UIColor(named: asset, in: bundle, compatibleWith: nil) { return Color(uiColor: color) }
                #else
                if let color = NSColor(named: asset, bundle: bundle) { return Color(nsColor: color) }
                #endif
            }
            return nil
        }
        guard name.hasSuffix("Color"), !name.hasPrefix("_"), !name.hasPrefix("@") else { return nil }
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

/// The bundles an entry's resources come from: this process's, and the
/// containing app's when this is its extension (`App.app/PlugIns/X.appex`).
enum SnapshotBundles {
    static let all: [Bundle] = {
        var bundles = [Bundle.main]
        let url = Bundle.main.bundleURL
        if url.pathExtension == "appex", let app = Bundle(url: url.deletingLastPathComponent().deletingLastPathComponent()) { bundles.append(app) }
        return bundles
    }()
}

/// A box's rounded outline with elliptical corners (CSS Backgrounds 3 §5),
/// top-left first, radii already reduced so neighbours never overlap.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct RoundedOutline: Shape {
    var radii: [CGSize]

    func path(in rect: CGRect) -> Path {
        var p = Path()
        guard rect.width > 0, rect.height > 0 else { return p }
        let r = SnapshotPaint.reduced(radii, in: rect)
        let (tl, tr, br, bl) = (r[0], r[1], r[2], r[3])
        let k: CGFloat = 0.5522847498
        p.move(to: CGPoint(x: rect.minX + tl.width, y: rect.minY))
        p.addLine(to: CGPoint(x: rect.maxX - tr.width, y: rect.minY))
        p.addCurve(to: CGPoint(x: rect.maxX, y: rect.minY + tr.height), control1: CGPoint(x: rect.maxX - tr.width * (1 - k), y: rect.minY), control2: CGPoint(x: rect.maxX, y: rect.minY + tr.height * (1 - k)))
        p.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY - br.height))
        p.addCurve(to: CGPoint(x: rect.maxX - br.width, y: rect.maxY), control1: CGPoint(x: rect.maxX, y: rect.maxY - br.height * (1 - k)), control2: CGPoint(x: rect.maxX - br.width * (1 - k), y: rect.maxY))
        p.addLine(to: CGPoint(x: rect.minX + bl.width, y: rect.maxY))
        p.addCurve(to: CGPoint(x: rect.minX, y: rect.maxY - bl.height), control1: CGPoint(x: rect.minX + bl.width * (1 - k), y: rect.maxY), control2: CGPoint(x: rect.minX, y: rect.maxY - bl.height * (1 - k)))
        p.addLine(to: CGPoint(x: rect.minX, y: rect.minY + tl.height))
        p.addCurve(to: CGPoint(x: rect.minX + tl.width, y: rect.minY), control1: CGPoint(x: rect.minX, y: rect.minY + tl.height * (1 - k)), control2: CGPoint(x: rect.minX + tl.width * (1 - k), y: rect.minY))
        p.closeSubpath()
        return p
    }
}

/// CSS `clip-path` as the kernel validated it: `{"rule": "nonzero" |
/// "evenodd", "commands": [["M", [x, y]], …]}` in the border box.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct ClipPathRow: Shape {
    let evenOdd: Bool
    let commands: [(String, [CGFloat])]

    init?(_ value: Any?) {
        guard let object = value as? [String: Any], let list = object["commands"] as? [Any], !list.isEmpty else { return nil }
        evenOdd = object["rule"] as? String == "evenodd"
        commands = list.compactMap { command in
            guard let pair = command as? [Any], let kind = pair.first as? String else { return nil }
            return (kind, (pair.last as? [Any])?.compactMap { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } } ?? [])
        }
    }

    func path(in rect: CGRect) -> Path {
        var p = Path()
        for (kind, v) in commands {
            func point(_ i: Int) -> CGPoint { CGPoint(x: rect.minX + v[i], y: rect.minY + v[i + 1]) }
            switch kind {
            case "M" where v.count == 2: p.move(to: point(0))
            case "L" where v.count == 2: p.addLine(to: point(0))
            case "Q" where v.count == 4: p.addQuadCurve(to: point(2), control: point(0))
            case "C" where v.count == 6: p.addCurve(to: point(4), control1: point(0), control2: point(2))
            case "Z": p.closeSubpath()
            default: break
            }
        }
        return p
    }

    /// A rule is the shape's fill style, which `clipShape` takes.
    var fillStyle: FillStyle { FillStyle(eoFill: evenOdd) }
}

/// A side's quadrilateral of the border area: from its two outer corners to
/// its two inner (padding box) corners, so the colour boundary at a corner
/// is the line between the two boxes' corners (CSS Backgrounds 3 §5.5).
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct BorderSide: Shape {
    let side: Int
    let widths: [CGFloat]

    func path(in rect: CGRect) -> Path {
        let inner = CGRect(x: rect.minX + widths[3], y: rect.minY + widths[0], width: max(0, rect.width - widths[1] - widths[3]), height: max(0, rect.height - widths[0] - widths[2]))
        let outer = [CGPoint(x: rect.minX, y: rect.minY), CGPoint(x: rect.maxX, y: rect.minY), CGPoint(x: rect.maxX, y: rect.maxY), CGPoint(x: rect.minX, y: rect.maxY)]
        let padding = [CGPoint(x: inner.minX, y: inner.minY), CGPoint(x: inner.maxX, y: inner.minY), CGPoint(x: inner.maxX, y: inner.maxY), CGPoint(x: inner.minX, y: inner.maxY)]
        let a = side, b = (side + 1) % 4
        var p = Path()
        p.move(to: outer[a]); p.addLine(to: outer[b]); p.addLine(to: padding[b]); p.addLine(to: padding[a]); p.closeSubpath()
        return p
    }
}

/// A box's paint: its corners, fill (colour and gradients) and border, from
/// its style rows, as the web paints them.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotPaint {
    let style: SnapshotStyle
    let dark: Bool

    /// Each corner's radius (horizontal, vertical), top-left first;
    /// percentages against the box's width and height.
    func radii(in rect: CGRect) -> [CGSize] {
        ["top_left", "top_right", "bottom_right", "bottom_left"].map { name -> CGSize in
            let key = style.has("border_radius_" + name) ? "border_radius_" + name : "border_radius"
            return CGSize(width: max(0, style.length(key, basis: rect.width) ?? 0), height: max(0, style.length(key, basis: rect.height) ?? 0))
        }
    }

    /// CSS's radius reduction: every corner scaled by the one factor that
    /// keeps two neighbours from overlapping an edge.
    static func reduced(_ radii: [CGSize], in rect: CGRect) -> [CGSize] {
        let sums = [radii[0].width + radii[1].width, radii[3].width + radii[2].width, radii[0].height + radii[3].height, radii[1].height + radii[2].height]
        let edges = [rect.width, rect.width, rect.height, rect.height]
        var factor: CGFloat = 1
        for i in 0..<4 where sums[i] > 0 { factor = min(factor, max(0, edges[i]) / sums[i]) }
        return radii.map { CGSize(width: max(0, $0.width * factor), height: max(0, $0.height * factor)) }
    }

    /// The border box's outline.
    func outline(in rect: CGRect) -> RoundedOutline { RoundedOutline(radii: radii(in: rect)) }
    var isRounded: Bool { radii(in: CGRect(x: 0, y: 0, width: 1000, height: 1000)).contains { $0.width > 0 && $0.height > 0 } }

    var widths: [CGFloat] { ["top", "right", "bottom", "left"].map { style.number("border_width_" + $0) ?? 0 } }

    /// The content edge's radii: each corner's, less the border and padding
    /// beside it (a replaced element's pixels clip to this).
    func contentRadii(outer: CGRect, inner: CGRect) -> [CGSize] {
        let r = Self.reduced(radii(in: outer), in: outer)
        let left = inner.minX - outer.minX, right = outer.maxX - inner.maxX, top = inner.minY - outer.minY, bottom = outer.maxY - inner.maxY
        return zip(r, [CGSize(width: left, height: top), CGSize(width: right, height: top), CGSize(width: right, height: bottom), CGSize(width: left, height: bottom)])
            .map { CGSize(width: max(0, $0.width - $1.width), height: max(0, $0.height - $1.height)) }
    }

    var hasBackground: Bool {
        !style.gradients("background_image").isEmpty || SnapshotColor.style(style.color("background_color"), dark: dark, context: .fill) != nil
    }

    /// The fill: the colour, then each gradient layer, the first on top.
    @ViewBuilder
    func fill(in rect: CGRect) -> some View {
        let shape = outline(in: rect)
        ZStack {
            if let color = SnapshotColor.style(style.color("background_color"), dark: dark, context: .fill) {
                shape.fill(color)
            }
            ForEach(Array(style.gradients("background_image").reversed().enumerated()), id: \.offset) { _, gradient in
                SnapshotGradient(row: gradient, box: rect, dark: dark).clipShape(shape)
            }
        }
    }

    /// The border: each side in its own colour and style (solid, dashed,
    /// dotted; the rest draw solid), clipped to its quadrilateral and to the
    /// rounded outline, the stroke centred in the border area.
    @ViewBuilder
    func border(in rect: CGRect) -> some View {
        let sides = ["top", "right", "bottom", "left"]
        let widths = widths
        let styles = sides.map { style.string("border_style_" + $0) ?? "none" }
        let colors = sides.map { SnapshotColor.style(style.color("border_color_" + $0) ?? style.color("text_color"), dark: dark, context: .fill) }
        let drawn = (0..<4).map { widths[$0] > 0 && styles[$0] != "none" && styles[$0] != "hidden" && colors[$0] != nil }
        if drawn.contains(true) {
            let outline = outline(in: rect)
            let uniform = drawn.allSatisfy { $0 } && widths.allSatisfy { $0 == widths[0] } && styles.allSatisfy { $0 == styles[0] }
                && (0..<4).allSatisfy { style.color("border_color_" + sides[$0]) == style.color("border_color_top") }
            ZStack(alignment: .topLeading) {
                ForEach(uniform ? [0] : (0..<4).filter { drawn[$0] }, id: \.self) { side in
                    let width = widths[side]
                    // The stroke's centre line: the outline inset by half the
                    // side's width, its radii shrunk by the same.
                    let inset = CGRect(x: width / 2, y: width / 2, width: max(0, rect.width - width), height: max(0, rect.height - width))
                    let radii = radii(in: rect).map { CGSize(width: max(0, $0.width - width / 2), height: max(0, $0.height - width / 2)) }
                    RoundedOutline(radii: radii)
                        .stroke(colors[side] ?? AnyShapeStyle(.clear), style: Self.stroke(styles[side], width: width))
                        .frame(width: inset.width, height: inset.height)
                        .offset(x: inset.minX, y: inset.minY)
                        .frame(width: rect.width, height: rect.height, alignment: .topLeading)
                        .clipShape(uniform ? AnyShape(outline) : AnyShape(BorderSide(side: side, widths: widths)))
                }
            }
            .clipShape(outline)
        }
    }

    /// CSS's border styles as strokes: dotted is round dots a width apart,
    /// dashed is dashes of three widths with gaps of three (as Chrome draws
    /// them); `double`, `groove`, `ridge`, `inset` and `outset` draw solid.
    static func stroke(_ style: String, width: CGFloat) -> StrokeStyle {
        switch style {
        case "dotted": return StrokeStyle(lineWidth: width, lineCap: .round, dash: [0.01, width * 2])
        case "dashed": return StrokeStyle(lineWidth: width, dash: [width * 3, width * 3])
        default: return StrokeStyle(lineWidth: width)
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

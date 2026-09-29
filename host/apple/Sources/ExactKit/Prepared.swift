// What a batch's ops need that is a plain value, built on the owner thread
// as the batch is decoded (LLP 1071 stage 4): the SVG scene's paths and its
// animations' signatures and keyframe values, and a flat leaf's paint. Main
// applies them to layers; it no longer parses them from the batch's numbers.
import CoreGraphics
import Foundation
import QuartzCore

/// A path built off main, carried in an SVG element's payload under "p".
final class PreparedPath {
    let path: CGPath
    init(_ path: CGPath) { self.path = path }
}

/// A CSS animation spec's lowered parts, carried under "#" in the spec: its
/// signature (what `CssAnimations.apply` keeps an unchanged one by), and the
/// keyframe animation's values, key times and timing functions.
final class PreparedAnimation {
    let signature: Int
    let values: [Any]
    let keyTimes: [NSNumber]
    let timing: [CAMediaTimingFunction]?
    init(signature: Int, values: [Any], keyTimes: [NSNumber], timing: [CAMediaTimingFunction]?) {
        self.signature = signature; self.values = values; self.keyTimes = keyTimes; self.timing = timing
    }
}

/// A flat leaf's paint (LLP 1068 §6.1), read once from its style: nil when
/// the style paints something a bare layer cannot.
struct FlatPaint: Equatable {
    /// The fill's RGBA bytes, and the colour made from them.
    let rgba: [Double]?
    let fill: CGColor?
    /// Top-left, top-right, bottom-right, bottom-left, each at least 0.
    let radii: [CGFloat]
    let hidden: Bool

    static let space = CGColorSpace(name: CGColorSpace.extendedSRGB)!

    static func == (a: FlatPaint, b: FlatPaint) -> Bool { a.rgba == b.rgba && a.radii == b.radii && a.hidden == b.hidden }

    /// Layout rows (the kernel's) and rows a childless, textless box never
    /// paints: they do not stop a leaf being flat.
    static let inert: Set<String> = [
        "width", "height", "min_width", "min_height", "max_width", "max_height",
        "padding_top", "padding_right", "padding_bottom", "padding_left",
        "margin_top", "margin_right", "margin_bottom", "margin_left",
        "flex_direction", "flex_wrap", "justify_content", "align_items", "align_self", "align_content",
        "flex_grow", "flex_shrink", "flex_basis", "position_type", "top", "right", "bottom", "left",
        "row_gap", "column_gap", "aspect_ratio", "box_sizing", "overflow_x", "overflow_y",
        "grid_auto_flow", "grid_template_columns", "grid_template_rows", "grid_column", "grid_row", "justify_items",
        "direction", "font_size", "font_weight", "font_style", "font_family", "line_height", "letter_spacing",
        "font_variant_numeric", "line_clamp", "white_space", "field_sizing", "overflow_wrap",
        "border_style_top", "border_style_right", "border_style_bottom", "border_style_left", "text_transform",
        "text_color", "text_align", "text_overflow", "text_decoration_line", "interpolate_size", "touch_action",
        "border_radius_top_left", "border_radius_top_right", "border_radius_bottom_right", "border_radius_bottom_left",
    ]

    init?(_ style: NodeStyle) {
        for (key, value) in style {
            if Self.inert.contains(key) { continue }
            switch key {
            case "background_color":
                // A fixed colour only: a `light-dark()` pair follows the
                // owning view's appearance, which a layer has not.
                guard let c = value.numbers, c.count == 4 else { return nil }
            case "border_width", "border_width_top", "border_width_right", "border_width_bottom", "border_width_left":
                guard value.number == 0 else { return nil }
            case "border_color_top", "border_color_right", "border_color_bottom", "border_color_left":
                continue // painted only with a width, which is refused above
            case "display":
                guard value.string != "none" else { return nil }
            default:
                return nil
            }
        }
        let c = style["background_color"]?.numbers
        rgba = c
        // UIKit's colours are extended sRGB; a view's fill is one.
        fill = c.flatMap { CGColor(colorSpace: FlatPaint.space, components: [$0[0] / 255, $0[1] / 255, $0[2] / 255, $0[3] / 255]) }
            .flatMap { $0.alpha > 0 ? $0 : nil }
        radii = [style["border_radius_top_left"], style["border_radius_top_right"],
                 style["border_radius_bottom_right"], style["border_radius_bottom_left"]]
            .map { CGFloat(max(0, $0?.number ?? 0)) }
        hidden = style["display"]?.string == "none"
    }
}

extension Batch {
    /// Build this batch's plain values; on the owner, as it is decoded.
    mutating func prepare() {
        for i in ops.indices {
            switch ops[i].op {
            case .create, .style:
                ops[i].flat = FlatPaint(ops[i].style)
            case .svg:
                if var scene = ops[i].payload["scene"] as? [String: Any] {
                    SvgPrepare.scene(&scene)
                    ops[i].payload["scene"] = scene
                }
            case .animations:
                if let specs = ops[i].payload["specs"] as? [[String: Any]] {
                    ops[i].payload["specs"] = specs.map(SvgPrepare.spec)
                }
            default: break
            }
        }
    }
}

enum SvgPrepare {
    /// A scene's element tree, in place: each shape's path, each animation.
    static func scene(_ scene: inout [String: Any]) {
        if let elements = scene["els"] as? [Any] { scene["els"] = elements.map(element) }
    }

    static func element(_ value: Any) -> Any {
        guard var e = value as? [String: Any] else { return value }
        if let p = e["p"], !(p is PreparedPath) { e["p"] = PreparedPath(SvgGeometry.path(p)) }
        if let children = e["c"] as? [Any] { e["c"] = children.map(element) }
        if let list = e["a"] as? [[String: Any]] { e["a"] = list.map(spec) }
        if var tf = e["tf"] as? [String: Any], let list = tf["a"] as? [[String: Any]] {
            tf["a"] = list.map(spec)
            e["tf"] = tf
        }
        return e
    }

    /// One CSS animation spec with its lowered parts under "#".
    static func spec(_ spec: [String: Any]) -> [String: Any] {
        guard spec["#"] == nil else { return spec }
        var out = spec
        var h = Hasher()
        CssAnimations.digest(spec, into: &h)
        let (values, keyTimes, timing) = CssAnimations.lowered(spec)
        out["#"] = PreparedAnimation(signature: h.finalize(), values: values, keyTimes: keyTimes, timing: timing)
        return out
    }
}

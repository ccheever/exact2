import CoreGraphics

/// A flat leaf's paint (LLP 1068 §6.1), read once from its style: nil when
/// the style paints something a bare layer cannot.
package struct FlatPaint: Equatable {
    /// The fill's RGBA bytes in the light and the dark appearance (the same
    /// unless the fill is a `light-dark()` pair), and the colours made from them.
    package let light: [Double]?
    package let dark: [Double]?
    private let lightFill: CGColor?
    private let darkFill: CGColor?
    /// Top-left, top-right, bottom-right, bottom-left, each at least 0.
    package let radii: [CGFloat]
    package let hidden: Bool

    package static let space = CGColorSpace(name: CGColorSpace.extendedSRGB)!

    package static func == (a: FlatPaint, b: FlatPaint) -> Bool { a.light == b.light && a.dark == b.dark && a.radii == b.radii && a.hidden == b.hidden }

    /// A `light-dark()` pair: it resolves in the appearance of the view the
    /// leaf is drawn in, as the pair on a view resolves in the view's.
    package var followsScheme: Bool { light != dark }
    package func rgba(dark: Bool) -> [Double]? { dark ? self.dark : light }
    package func fill(dark: Bool) -> CGColor? { dark ? darkFill : lightFill }

    /// Layout rows (the kernel's) and rows a childless, textless box never
    /// paints: they do not stop a leaf being flat.
    package static let inert: Set<String> = [
        "width", "height", "min_width", "min_height", "max_width", "max_height",
        "padding_top", "padding_right", "padding_bottom", "padding_left",
        "margin_top", "margin_right", "margin_bottom", "margin_left",
        "flex_direction", "flex_wrap", "justify_content", "align_items", "align_self", "align_content",
        "flex_grow", "flex_shrink", "flex_basis", "top", "right", "bottom", "left",
        "row_gap", "column_gap", "aspect_ratio", "box_sizing", "overflow_x", "overflow_y",
        "grid_auto_flow", "grid_template_columns", "grid_template_rows", "grid_column", "grid_row", "justify_items",
        "direction", "font_size", "font_weight", "font_style", "font_family", "line_height", "letter_spacing",
        "font_variant_numeric", "line_clamp", "white_space", "text_wrap_style", "field_sizing", "overflow_wrap",
        "border_style_top", "border_style_right", "border_style_bottom", "border_style_left", "text_transform",
        "text_color", "text_shadow", "text_stroke_width", "text_stroke_color", "text_align", "text_overflow", "text_decoration_line", "interpolate_size", "touch_action",
        "border_radius_top_left", "border_radius_top_right", "border_radius_bottom_right", "border_radius_bottom_left",
        // A fixed fill paints alike in either scheme (LLP 1034 §8).
        "color_scheme",
        // Which rows the author wrote (LLP 1115.000 D2): nothing a leaf paints.
        "written",
    ]

    package init?(_ style: NodeStyle, colorChannels: (BatchValue, Bool) -> [Double]?) {
        for (key, value) in style {
            // A percentage needs the final box; let the view paint it.
            if key.hasPrefix("border_radius_"), value.number == nil { return nil }
            if Self.inert.contains(key) { continue }
            switch key {
            case "position_type":
                guard value.string == "static" else { return nil }
            case "background_color":
                // A fixed colour or a `light-dark()` pair of them. A platform
                // colour also follows contrast and elevation, and a colour in
                // its own space needs a view's layer.
                guard value.numbers?.count == 4 || (value.array != nil && value.isSchemeColor) else { return nil }
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
        let value = style["background_color"]
        light = value.flatMap { colorChannels($0, false) }
        dark = value.flatMap { colorChannels($0, true) }
        // Its own `color-scheme` would resolve the pair apart from its parent.
        if light != dark, ["light", "dark"].contains(style["color_scheme"]?.string ?? "") { return nil }
        // UIKit's colours are extended sRGB; a view's fill is one.
        func color(_ c: [Double]?) -> CGColor? {
            c.flatMap { CGColor(colorSpace: FlatPaint.space, components: [$0[0] / 255, $0[1] / 255, $0[2] / 255, $0[3] / 255]) }
                .flatMap { $0.alpha > 0 ? $0 : nil }
        }
        lightFill = color(light)
        darkFill = light == dark ? lightFill : color(dark)
        radii = [style["border_radius_top_left"], style["border_radius_top_right"],
                 style["border_radius_bottom_right"], style["border_radius_bottom_left"]]
            .map { CGFloat(max(0, $0?.number ?? 0)) }
        hidden = style["display"]?.string == "none"
    }
}

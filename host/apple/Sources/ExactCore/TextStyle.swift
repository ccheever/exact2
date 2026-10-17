import Foundation

/// The metric half of a node's paragraph, shared by every Apple presentation.
package enum TextStyle {
    package static func alignment(_ style: NodeStyle) -> Int {
        let rtl = style["direction"]?.string == "rtl"
        switch style["text_align"]?.string {
        case "left": return 0
        case "center": return 1
        case "right": return 2
        case "justify": return 3
        case "end": return rtl ? 0 : 2
        default: return rtl ? 2 : 0
        }
    }
    package static func run(_ text: String, style: NodeStyle, defaultSize: Double = 16) -> Run {
        let size = Float(style["font_size"]?.number ?? defaultSize)
        let height: CGFloat?
        if let ratio = style["line_height"]?.number { height = CGFloat(Float(ratio) * size) }
        else if let px = style["line_height"]?.string, px.hasSuffix("px"), let value = Float(px.dropLast(2)) { height = CGFloat(value) }
        else { height = nil }
        return Run(text: text, size: CGFloat(size), weight: Int(style["font_weight"]?.number ?? 400),
                   family: Int(style["font_family"]?.number ?? 0), italic: style["font_style"]?.string == "italic",
                   lineHeight: height, letterSpacing: CGFloat(Float(style["letter_spacing"]?.number ?? 0)),
                   numeric: Int(style["font_variant_numeric"]?.number ?? 0))
    }
    package static func spec(runs input: [Run], style: NodeStyle, props: [String: String], strut: Run,
                             color: [Double] = [0, 0, 0, 255], language: String = "") -> Spec {
        var runs = input
        let align = alignment(style), rtl = style["direction"]?.string == "rtl"
        func number(_ name: String) -> Double { style[name]?.number ?? 0 }
        // A fitted paragraph is drawn at the scale the kernel's layout chose
        // for it, as it was measured (`-exact-fit`, LLP 1116.001 D1.4).
        let fit = props["fitScale"].flatMap(Float.init) ?? 1
        if fit != 1 { runs = runs.map { $0.scaled(fit) } }
        let whiteSpace = ["pre-wrap": 1, "nowrap": 2, "pre-line": 3, "pre": 4][style["white_space"]?.string ?? ""] ?? 0
        // CSS collapses white space before shaping, as the measurer does in
        // Rust with the same function (LLP 1053 G5); Markdown keeps its lines.
        let source = whiteSpace == 1 || whiteSpace == 4 || props["markup"] == "markdown" ? SourceMap() : SourceMap.collapse(&runs, whiteSpace: whiteSpace)
        let lineClamp = Int(number("line_clamp"))
        // `text-overflow: ellipsis` applies to a box that clips its inline overflow.
        let clips = (style["overflow_x"]?.string).map { $0 != "visible" } ?? false
        var spec = Spec(runs: runs, align: align, lineClamp: lineClamp,
                        color: color,
                        overflowWrap: style["overflow_wrap"]?.string == "anywhere" ? 2 : style["overflow_wrap"]?.string == "break-word" ? 1 : 0, direction: rtl ? 1 : 0, whiteSpace: whiteSpace, strut: strut.scaled(fit))
        spec.ellipsis = lineClamp == 0 && clips && style["text_overflow"]?.string == "ellipsis"
        spec.source = source
        spec.textIndent = CGFloat(Float(number("text_indent")))
        spec.hyphens = ["none": 1, "auto": 2][style["hyphens"]?.string ?? ""] ?? 0
        spec.textBoxTrim = ["trim-start": 1, "trim-end": 2, "trim-both": 3][style["text_box_trim"]?.string ?? ""] ?? 0
        let edge = style["text_box_edge"]?.string ?? "auto"
        spec.textBoxOver = edge.hasPrefix("cap") ? 1 : edge.hasPrefix("ex") ? 2 : 0
        spec.textBoxUnder = edge.hasSuffix("alphabetic") ? 1 : 0
        spec.balance = style["text_wrap_style"]?.string == "balance"
        // The kernel's measurer hyphenates by the same document language.
        if spec.hyphens == 2 { spec.language = language }
        spec.hyphenateAuto()
        return spec
    }
}

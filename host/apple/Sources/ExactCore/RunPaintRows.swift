import Foundation

package struct RunPaintRows: Equatable {
    /// Offset x, y and blur in points; nil is `none`.
    package var shadow: [Double]?
    /// The shadow's colour; nil is `currentcolor`.
    package var shadowColor: BatchValue?
    package var strokeWidth: Double = 0
    /// The stroke's colour; nil is `currentcolor`.
    package var strokeColor: BatchValue?

    package init() {}
    package init(_ style: NodeStyle) {
        for key in ["text_shadow", "text_stroke_width", "text_stroke_color"] {
            if let value = style[key] { set(key, value) }
        }
    }

    /// Whether a colour here is a `light-dark()` pair.
    package var paired: Bool { shadowColor?.isSchemeColor == true || strokeColor?.isSchemeColor == true }

    /// One row off the wire; false when `key` is not one of these rows.
    @discardableResult
    package mutating func set(_ key: String, _ value: BatchValue) -> Bool {
        switch key {
        case "text_shadow":
            guard case .object(let o) = value, let offset = o["o"]?.numbers, offset.count == 2 else { shadow = nil; return true }
            shadow = offset + [max(0, o["b"]?.number ?? 0)]
            shadowColor = o["c"]
        case "text_stroke_width": strokeWidth = value.number ?? 0
        case "text_stroke_color": strokeColor = value.string == nil ? value : nil
        default: return false
        }
        return true
    }

}

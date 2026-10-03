// LLP 1077 D13: UIKit's label, fill and separator colours (`-apple-system-*`)
// drawn as Apple draws them over a material — vibrantly, blended with what
// the material blurs — where elsewhere they are their light and dark pair.
// The kernel keeps a system colour's identity (`ColorValue::System`), and
// the style names the rows that hold one (`system_colors`, by row).
//
// macOS: a material's children are inside its `NSVisualEffectView`, which
// draws a descendant vibrantly when the view allows it. iOS: a blur
// material's vibrancy effect needs the drawing inside a vibrancy effect view
// within the blur's content view (`IOS/VibrancyIOS.swift`).
#if os(iOS)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    /// The system colour row `row` holds, by WebKit's name, if any.
    func systemColor(_ row: String) -> String? {
        guard case .object(let rows)? = style["system_colors"] else { return nil }
        return rows[row]?.string
    }

    /// The system colour this node draws in: its text's, or its fill's.
    var vibrantColor: String? { systemColor("text_color") ?? systemColor("background_color") }

    /// The nearest material around this node, when it is a blur (not glass,
    /// which tints its content itself, nor a CSS backdrop).
    var enclosingBlur: NodeView? {
        var view = superview
        while let v = view {
            if let n = v as? NodeView, n.materialView != nil {
                return n.materialBlurs ? n : nil
            }
            view = v.superview
        }
        return nil
    }
}

#if os(macOS)
extension NodeView {
    /// A blur's own material (`NSVisualEffectView`), not glass.
    var materialBlurs: Bool { materialView is NSVisualEffectView }

    /// AppKit draws this view vibrantly inside its material when it says so.
    override var allowsVibrancy: Bool { vibrantColor != nil && enclosingBlur != nil }
}
#endif

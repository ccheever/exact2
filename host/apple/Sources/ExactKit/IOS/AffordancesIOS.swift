// LLP 1076 §5 on UIKit: what iOS draws that CSS has no name for. Smart
// Invert leaving a node's pixels alone (D18), a scroll container's edge
// effect (D16, iOS 26), the iPad pointer's effect over a node (D17), and
// a text's numerals rolling as they change (D15).
#if os(iOS)
import ObjectiveC
import UIKit

extension NodeView {
    /// Everything here a style change can move.
    func applyAffordances() {
        // D18: images, video and canvases are left alone under `auto`.
        let invert = style["smart_invert"]?.string ?? "auto"
        let ignores = invert == "ignore" || ["image", "video", "canvas"].contains(kind)
        if accessibilityIgnoresInvertColors != ignores { accessibilityIgnoresInvertColors = ignores }
        if let sv = scroll { applyScrollEdge(sv) }
        applyHoverEffect()
    }

    /// D16: the edge effect where content meets a bar; `none` hides it.
    private func applyScrollEdge(_ sv: UIScrollView) {
        guard #available(iOS 26.0, *) else { return }
        let value = style["scroll_edge_effect"]?.string ?? "automatic"
        for edge in [sv.topEdgeEffect, sv.bottomEdgeEffect, sv.leftEdgeEffect, sv.rightEdgeEffect] {
            let hidden = value == "none"
            if edge.isHidden != hidden { edge.isHidden = hidden }
            let style: UIScrollEdgeEffect.Style = value == "soft" ? .soft : value == "hard" ? .hard : .automatic
            if edge.style != style { edge.style = style }
        }
    }

    /// D17: a pointer interaction whose style is the node's effect.
    private func applyHoverEffect() {
        let effect = style["hover_effect"]?.string ?? "auto"
        let current = interactions.compactMap { $0 as? UIPointerInteraction }.first
        guard effect != "auto", effect != "none" || current != nil else { return }
        if effect == "none" {
            if let current { removeInteraction(current) }
            return
        }
        let delegate = HoverEffect.of(self)
        delegate.effect = effect
        if current == nil { addInteraction(UIPointerInteraction(delegate: delegate)) }
    }
}

/// The pointer's style over one node (D17), kept on the node.
final class HoverEffect: NSObject, UIPointerInteractionDelegate {
    var effect = "auto"
    private static var key = 0
    static func of(_ view: UIView) -> HoverEffect {
        if let h = objc_getAssociatedObject(view, &key) as? HoverEffect { return h }
        let h = HoverEffect()
        objc_setAssociatedObject(view, &key, h, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return h
    }
    func pointerInteraction(_ interaction: UIPointerInteraction, styleFor region: UIPointerRegion) -> UIPointerStyle? {
        guard let view = interaction.view, view.window != nil else { return nil }
        let preview = UITargetedPreview(view: view)
        switch effect {
        case "highlight": return UIPointerStyle(effect: .highlight(preview))
        case "lift": return UIPointerStyle(effect: .lift(preview))
        case "hover": return UIPointerStyle(effect: .hover(preview))
        default: return nil
        }
    }
}

/// D15: a paragraph whose text changed under `content-transition: numeric`
/// rolls in from below (from above for `numeric-countdown`), as SwiftUI's
/// numeric text does. The whole line rolls: the raster is one picture.
enum NumeralRoll {
    private static var key = 0
    static func roll(_ ink: CALayer, node: NodeView) {
        let mode = node.style["content_transition"]?.string ?? "none"
        let text = node.props["text"] ?? node.inlineText.map(\.text).joined()
        let last = objc_getAssociatedObject(ink, &key) as? String
        objc_setAssociatedObject(ink, &key, text, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        guard mode != "none", let last, last != text, ink.contents != nil, !UIAccessibility.isReduceMotionEnabled else { return }
        let roll = CATransition()
        roll.type = .push
        roll.subtype = mode == "numeric-countdown" ? .fromTop : .fromBottom
        roll.duration = 0.3
        roll.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        ink.add(roll, forKey: "exact.numeric")
    }
}
#endif

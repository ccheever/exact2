// An icon path on a node (LLP 1121 D10; stage 9 A; ExactCore's IconPath draws
// it): its image and its tint, `-exact-tint-color` where written, else its
// `color` (path icons only: an SF Symbol keeps its own tint, 17:03).
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import ExactCore

extension NodeView {
    /// An icon path's image for this node `side` points square, when it has
    /// one (`symbolPath`).
    func iconPathImage(side: CGFloat) -> IconImage? {
        guard let commands = props["symbolPath"] else { return nil }
        return IconPath.image(commands, viewBox: props["symbolViewBox"] ?? "", size: CGSize(width: side, height: side))
    }
    /// An icon path's tint (D10): `-exact-tint-color` where written (its
    /// accent where it names one), else the node's text colour, else the
    /// platform's (live).
    var iconPathTint: PlatformColor {
        if style["tint_color"] != nil { return symbolTint ?? inheritedTint }
        #if os(macOS)
        return channels("text_color").map(TextEngine.color) ?? .textColor
        #else
        return channels("text_color").map(TextEngine.color) ?? .label
        #endif
    }
}

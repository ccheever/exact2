// An icon path on a node (LLP 1121 D10), drawn by ExactCore’s IconPath.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import ExactCore

extension NodeView {
    /// An icon path's image for this node `side` points square, when it has
    /// one (`symbolPath`), filled by its `symbolFillRule`.
    func iconPathImage(side: CGFloat) -> IconImage? {
        guard let commands = props["symbolPath"] else { return nil }
        return IconPath.image(commands, viewBox: props["symbolViewBox"] ?? "", size: CGSize(width: side, height: side),
                              rule: IconPath.rule(props["symbolFillRule"]))
    }
}

// A custom grouped row on UIKit (LLP 1084 D7), its views carried into a
// list cell, drawn as a standard cell beside it is: its leading symbol as
// UIKit's list draws a cell's image, and the line under it starting under
// its content, where a standard cell's starts under its text (Harmony,
// 2026-10-21: a row with a badge before its chevron is custom, and its
// symbol was a fifth smaller and its line ran from the card's edge).
import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

extension GroupedListView {
    /// Marks the row's leading symbol, its first shown child when that is a
    /// symbol no accessory names, to be drawn at a list cell's scale
    /// (`NodeView.listSymbol`), and no other child.
    func markLeadingSymbol(_ row: NodeView) {
        let shown = row.container.subviews.compactMap { $0 as? NodeView }.filter { $0.style["display"]?.string != "none" }
        for (i, child) in shown.enumerated() {
            let symbol = child.kind == "image" && (child.props["imageSource"] ?? "").hasPrefix("symbol:")
            let accessory = ["chevron.forward", "chevron.right", "checkmark"].contains(child.props["symbolName"] ?? "")
            child.listSymbol = i == 0 && symbol && !accessory
        }
    }

    /// Where the custom row `id`'s line starts in its cell: at its content
    /// box's leading edge (the row's inset, past a leading symbol), at the
    /// place the row is carried to (`carry`), as the kernel places the
    /// line where the sheet draws it (`Kernel::grouped_separator`).
    func separatorLeading(_ id: UInt32) -> CGFloat? {
        guard let row = host.presenter.views[id] else { return nil }
        let x = row.grid.map { PixelGrid.round($0.layout.minX, scale: host.presenter.gridScale) } ?? carried[id]?.frame.minX ?? row.frame.minX
        return x + row.contentBox().minX
    }
}
#endif

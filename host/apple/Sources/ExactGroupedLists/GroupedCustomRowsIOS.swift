// A custom grouped row on UIKit (LLP 1084 D7), its views carried into a
// list cell, drawn as a standard cell beside it is: its leading symbol as
// UIKit's list draws a cell's image, and the line under it starting under
// its content, where a standard cell's starts under its text.
import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

/// What a custom row draws as a standard cell would: the leading symbol it
/// marked (`NodeView.listSymbol`) and where its line started when last
/// carried, to know when it moves.
struct CustomLook {
    weak var symbol: NodeView?
    var line: CGFloat?
}

extension GroupedListView {
    /// The sheet's slot for a row's leading symbol on iOS
    /// (`grouped-symbol-width`, `-height`; `object-fit: contain`).
    private static let symbolSlot = CGSize(width: 24, height: 20)

    /// Marks the custom row `id`'s leading symbol and notes its line, as
    /// it is carried into its cell; a mark it no longer has is cleared, and
    /// a line that moved lays the list out again after the update (or,
    /// carried as a cell shows, at the next projection sync).
    func look(_ id: UInt32, row: NodeView) {
        var look = customLooks[id] ?? CustomLook()
        let symbol = leadingSymbol(of: row)
        if look.symbol !== symbol {
            look.symbol?.listSymbol = false
            symbol?.listSymbol = true
            look.symbol = symbol
        }
        let line = lineStart(id)
        if look.line != line { look.line = line; resized = true }
        customLooks[id] = look
    }

    /// The row is no custom row now, is gone, or the list is going: its
    /// mark goes.
    func forgetLook(_ id: UInt32) {
        customLooks.removeValue(forKey: id)?.symbol?.listSymbol = false
    }

    /// The row's first shown child when it is the sheet's leading symbol: a
    /// `symbol:` image no accessory names, in the sheet's slot, so an image
    /// its author sized or fitted otherwise, or one a column row holds, is
    /// left as laid out. (One its author gave exactly the sheet's box and
    /// fit is drawn as the sheet's: the host sees the rows, not who wrote
    /// them.)
    private func leadingSymbol(of row: NodeView) -> NodeView? {
        let first = row.container.subviews.lazy.compactMap { $0 as? NodeView }.first { $0.style["display"]?.string != "none" }
        guard let image = first, image.kind == "image", (image.props["imageSource"] ?? "").hasPrefix("symbol:"),
              !["chevron.forward", "chevron.right", "checkmark"].contains(image.props["symbolName"] ?? ""),
              image.style["object_fit"]?.string == "contain" else { return nil }
        let size = image.grid?.layout.size ?? image.bounds.size
        return abs(size.width - Self.symbolSlot.width) < 0.5 && abs(size.height - Self.symbolSlot.height) < 0.5 ? image : nil
    }

    /// Where the custom row `id`'s line starts, from its cell's leading
    /// edge, read live, so a row not yet carried has it too: at its content
    /// box's leading edge (the row's inset, past a leading symbol), the row
    /// at the place it is carried to (`carry`), as the kernel places the
    /// line where the sheet draws it (`Kernel::grouped_separator`). Right to
    /// left the leading edge is the right one, measured in the cell the row
    /// is carried into (before that, the row standing as far in from both
    /// edges).
    func lineStart(_ id: UInt32) -> CGFloat? {
        guard let row = host.presenter.views[id] else { return nil }
        let x = row.grid.map { PixelGrid.round($0.layout.minX, scale: host.presenter.gridScale) } ?? carried[id]?.frame.minX ?? row.frame.minX
        let box = row.contentBox().offsetBy(dx: -row.bounds.minX, dy: 0)
        let traits: UIUserInterfaceLayoutDirection = collection.traitCollection.layoutDirection == .rightToLeft ? .rightToLeft : .leftToRight
        guard UIView.userInterfaceLayoutDirection(for: collection.semanticContentAttribute, relativeTo: traits) == .rightToLeft else { return x + box.minX }
        let cell = carried[id] != nil ? row.superview?.bounds.width : nil
        return (cell ?? 2 * x + row.bounds.width) - (x + box.maxX)
    }
}
#endif

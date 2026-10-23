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
    /// Marks the custom row `id`'s leading symbol and notes its line, as
    /// it is carried into its cell; a mark it no longer has is cleared, and
    /// a line that moved lays the list out again after the update (or,
    /// carried as a cell shows, at the next projection sync).
    func look(_ id: UInt32, row: NodeView) {
        mark(id, leadingSymbol(of: row))
        let line = lineStart(id)
        if customLooks[id]?.line != line { customLooks[id]?.line = line; resized = true }
    }

    /// Every custom row noted, carried now or not, its mark checked again
    /// before the visible ones are carried: an image that left a row
    /// offscreen goes back to its own look.
    func refreshLooks() {
        for id in Array(customLooks.keys) {
            guard let row = host.presenter.views[id] else { forgetLook(id); continue }
            mark(id, leadingSymbol(of: row))
        }
    }

    /// The row `id`'s leading symbol is `symbol` now: the one it marked
    /// before is cleared unless another row marks it now, and one another
    /// row marked is that row's no longer, whatever order rows are carried in.
    private func mark(_ id: UInt32, _ symbol: NodeView?) {
        var look = customLooks[id] ?? CustomLook()
        if look.symbol !== symbol {
            if let old = look.symbol, !customLooks.contains(where: { $0.key != id && $0.value.symbol === old }) { old.listSymbol = false }
            if let symbol {
                for (other, seen) in customLooks where other != id && seen.symbol === symbol { customLooks[other]?.symbol = nil }
                symbol.listSymbol = true
            }
            look.symbol = symbol
        }
        customLooks[id] = look
    }

    /// The row is no custom row now, is gone, or the list is going: its
    /// mark goes.
    func forgetLook(_ id: UInt32) {
        customLooks.removeValue(forKey: id)?.symbol?.listSymbol = false
    }

    /// The row's first shown child when it is the sheet's leading symbol: a
    /// `symbol:` image no accessory names, sized and fitted by the sheet,
    /// so an image whose size or fit its author wrote at all, or one a
    /// column row holds, is left as laid out (`wrote`: the sheet's rows are
    /// the compiler's).
    private func leadingSymbol(of row: NodeView) -> NodeView? {
        // At an accessibility size UIKit's cell sizes its image by a rule
        // not yet measured, and the sheet's slot keeps the default's 24 by
        // 20 there (QUEUE): the slot's own drawing stays.
        guard !collection.traitCollection.preferredContentSizeCategory.isAccessibilityCategory else { return nil }
        let first = row.container.subviews.lazy.compactMap { $0 as? NodeView }.first { $0.style["display"]?.string != "none" }
        guard let image = first, image.kind == "image", (image.props["imageSource"] ?? "").hasPrefix("symbol:"),
              !["chevron.forward", "chevron.right", "checkmark"].contains(image.props["symbolName"] ?? ""),
              image.style["object_fit"]?.string == "contain", !["object_fit", "width", "height"].contains(where: image.wrote) else { return nil }
        return image
    }

    /// Where the custom row `id`'s line starts from its cell's left edge,
    /// read live, so a row not yet carried has it too: at its content box
    /// (the row's inset, past a leading symbol), the row at the place it is
    /// carried to (`carry`), as the kernel places the line where the sheet
    /// draws it (`Kernel::grouped_separator`). Right to left, none: the
    /// sheet's insets are physical and the row's views are not mirrored,
    /// so UIKit's own line stays (QUEUE).
    func lineStart(_ id: UInt32) -> CGFloat? {
        guard let row = host.presenter.views[id] else { return nil }
        let traits: UIUserInterfaceLayoutDirection = collection.traitCollection.layoutDirection == .rightToLeft ? .rightToLeft : .leftToRight
        if UIView.userInterfaceLayoutDirection(for: collection.semanticContentAttribute, relativeTo: traits) == .rightToLeft { return nil }
        let x = row.grid.map { PixelGrid.round($0.layout.minX, scale: host.presenter.gridScale) } ?? carried[id]?.frame.minX ?? row.frame.minX
        return x + row.contentBox().offsetBy(dx: -row.bounds.minX, dy: 0).minX
    }
}
#endif

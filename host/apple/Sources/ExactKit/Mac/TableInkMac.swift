// @ref LLP 1115.002 D5, LLP 1115.000 D3.7 — what Exact paints inside a
// table's selected row takes the selected item's ink, as a stock table's
// cell does. Measured on macOS 27.0.1 (a stock `NSTableView`, light and
// dark alike; the probe and what it printed are with the audit's records,
// `mac-table/probes/selected.swift`):
//
// - In a selected row drawn emphasized (its window key, the table first
//   responder), a label in a label colour is `alternateSelectedControlTextColor`:
//   whole for `labelColor`, at 0.92 for the secondary, 0.61 for the tertiary
//   and 0.39 for the quaternary. A template symbol is that colour whole,
//   whatever tinted it. A label in any other colour keeps it (the accent
//   too, which then cannot be read on the accent).
// - In a selected row that is not emphasized, and in any other row, nothing changes.
//
// A painted list's row in a split's list column is drawn selected as a
// table's is (`drawSelection`, SplitMac.swift), and the same rules hold in it.
//
// A row in a table also draws no disclosure chevron (LLP 1115.002 D5, §5
// Q4): SwiftUI's list of navigation links draws none on the Mac, and the
// table's selection says which row is open. The symbol's node keeps its box
// (layout is not asked again for a row as it mounts), and its glyph is not
// drawn while the row is in a cell.
//
// So here: text whose colour is unsaid or names a role (a label colour, the
// link's, the accent) is the selected item's text, a label colour at its
// measured share; a symbol whose tint is unsaid or names a role is drawn as
// AppKit draws a cell's image. A colour the author wrote as a colour stays
// the author's, for text and symbol alike. AppKit says when (`TableRowCell
// .backgroundStyle`); the roles resolve through `SystemColor.selectedItem`.
#if os(macOS)
import AppKit

/// A painted list's selected row's selection (`NodeView.drawSelection`).
final class RowSelectionLayer: CALayer {
    weak var row: NodeView?
    var emphasized = false
    override func action(forKey event: String) -> CAAction? { NSNull() }
}

extension NodeView {
    /// Whether this view is in a table's row that AppKit draws selected and
    /// emphasized, or in a painted list's row drawn so (`drawSelection`):
    /// read up the view chain, as the tint is (NodeColorMac.swift).
    var inEmphasizedRow: Bool {
        guard let presenter else { return false }
        if let row = presenter.navigation.split.emphasizedRow, isDescendant(of: row) { return true }
        guard !presenter.tables.feeds.isEmpty else { return false }
        var above = superview
        while let view = above, !(view is NSTableView) {
            if let cell = view as? TableRowCell { return cell.paintsSelection && cell.backgroundStyle == .emphasized }
            above = view.superview
        }
        return false
    }

    /// This node is the root view of a row that went into a table's cell,
    /// or came out of one, or a painted list that became, or stopped being,
    /// a split's list (SplitMac.swift): a disclosure chevron in it (the
    /// `forward-chevron` role, or SF's glyph by name) is not drawn there. Its
    /// glyph's view is made clear: whether it is hidden is CSS `visibility`'s
    /// (`applyCssVisibility`), written again with every style. Any other
    /// symbol's glyph is drawn, so one that stopped being a chevron shows.
    func rowDisclosure(hidden: Bool) {
        func walk(_ view: NSView) {
            if let node = view as? NodeView, let leaf = node.symbolView {
                let chevron = ["chevron.forward", "chevron.right"].contains(node.props["symbolName"] ?? "")
                leaf.alphaValue = hidden && chevron ? 0 : 1
            }
            for child in (view as? NodeView)?.container.subviews ?? view.subviews where child is NodeView { walk(child) }
        }
        walk(self)
    }

    /// Draw `mark` behind this row as an inset `NSTableView` draws its
    /// selected row, `emphasized` or not. Measured on macOS 27.0.1 (a stock
    /// inset table, 28-point rows): the selection is the row's height, 10
    /// points in from the table's sides and 8 before a cell's label, with
    /// 7-point corners, filled with `selectedContentBackgroundColor` while
    /// emphasized and `unemphasizedSelectedContentBackgroundColor`
    /// otherwise. A painted row's content can start nearer the edge (10
    /// points in the Mac's grouped form), or a swipe row can be inset by its
    /// margin, so the selection is placed by the content: 8 points before
    /// its first leaf and after the row's padding at the end, and no more
    /// than 10 points in from, or out past, the view that holds the row (a
    /// section's card, the list's document). It is a layer of that view,
    /// over its own box and under its rows, so a swipe row's scroll view
    /// does not clip it.
    func drawSelection(_ mark: RowSelectionLayer, emphasized: Bool) {
        guard let holder = superview else { mark.removeFromSuperlayer(); return }
        holder.wantsLayer = true
        guard let layer = holder.layer else { return }
        if mark.superlayer !== layer {
            mark.removeFromSuperlayer()
            let paint: [CALayer?] = (holder as? NodeView).map { [$0.shadowCaster, $0.boxFill, $0.boxGradient, $0.insetCaster, $0.imageLayer] } ?? []
            let box = layer.sublayers?.last { sublayer in paint.contains { $0 === sublayer } }
            if let box { layer.insertSublayer(mark, above: box) } else { layer.insertSublayer(mark, at: 0) }
        }
        var leaf: NodeView = self
        while let first = leaf.container.subviews.lazy.compactMap({ $0 as? NodeView }).first(where: { !$0.isHidden }) { leaf = first }
        let start = leaf === self ? frame.minX + number("padding_left") : holder.convert(leaf.bounds, from: leaf).minX
        let end = frame.maxX - number("padding_right"), edges = holder.bounds
        let left = min(max(start - 8, edges.minX), edges.minX + 10), right = max(min(end + 8, edges.maxX), edges.maxX - 10)
        let rect = NSRect(x: left, y: frame.minY, width: max(0, right - left), height: frame.height)
        if mark.frame != rect { mark.frame = rect }
        if mark.cornerRadius != 7 { mark.cornerRadius = 7 }
        mark.row = self
        mark.emphasized = emphasized
        colorSelection(mark)
    }

    /// The selection's colour, in this row's appearance and the system's
    /// accent as they are now.
    func colorSelection(_ mark: RowSelectionLayer) {
        effectiveAppearance.performAsCurrentDrawingAppearance {
            let fill = (mark.emphasized ? NSColor.selectedContentBackgroundColor : .unemphasizedSelectedContentBackgroundColor).cgColor
            if mark.backgroundColor != fill { mark.backgroundColor = fill }
        }
    }

    /// How a symbol's image cell is drawn in a row emphasized or not: as a
    /// cell’s image, unless its computed color is fixed. Asked
    /// again whenever the symbol's look is (`updateSymbol`).
    func symbolBackground(_ emphasized: Bool) -> NSView.BackgroundStyle {
        let ink = symbolInk != nil ? style["text_color"] : nil
        return emphasized && ink?.fixesInk != true ? .emphasized : .normal
    }

    /// The row this node is the root view of became, or stopped being, a
    /// selected row drawn emphasized: every paragraph in it resolves its ink
    /// again, and every symbol is drawn as a cell's image is.
    func rowEmphasisChanged(_ emphasized: Bool) {
        func walk(_ view: NSView) {
            if let node = view as? NodeView {
                if node.kind == "text" { node.reapplyColors() }
                if let leaf = node.symbolView { leaf.cell?.backgroundStyle = node.symbolBackground(emphasized) }
                node.tintIconShape(emphasized)
            }
            view.subviews.forEach(walk)
        }
        walk(self)
    }
}
#endif

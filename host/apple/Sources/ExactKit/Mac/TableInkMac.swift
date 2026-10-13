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

extension NodeView {
    /// Whether this view is in a table's row that AppKit draws selected and
    /// emphasized: read up the view chain, as the tint is (NodeColorMac.swift).
    var inEmphasizedRow: Bool {
        guard presenter?.tables.feeds.isEmpty == false else { return false }
        var above = superview
        while let view = above, !(view is NSTableView) {
            if let cell = view as? TableRowCell { return cell.backgroundStyle == .emphasized }
            above = view.superview
        }
        return false
    }

    /// This node is the root view of a row that went into a table's cell,
    /// or came out of one: a disclosure chevron in it (the `forward-chevron`
    /// role, or SF's glyph by name) is not drawn in the cell. Its glyph's
    /// view is made clear: whether it is hidden is CSS `visibility`'s
    /// (`applyCssVisibility`), written again with every style.
    func rowDisclosure(hidden: Bool) {
        func walk(_ view: NSView) {
            if let node = view as? NodeView, let leaf = node.symbolView, ["chevron.forward", "chevron.right"].contains(node.props["symbolName"] ?? "") {
                leaf.alphaValue = hidden ? 0 : 1
            }
            for child in (view as? NodeView)?.container.subviews ?? view.subviews where child is NodeView { walk(child) }
        }
        walk(self)
    }

    /// The row this node is the root view of became, or stopped being, a
    /// selected row drawn emphasized: every paragraph in it resolves its ink
    /// again, and every symbol is drawn as a cell's image is.
    func rowEmphasisChanged(_ emphasized: Bool) {
        func walk(_ view: NSView) {
            if let node = view as? NodeView {
                if node.kind == "text" { node.reapplyColors() }
                if let leaf = node.symbolView, node.style["tint_color"]?.fixesInk != true {
                    leaf.cell?.backgroundStyle = emphasized ? .emphasized : .normal
                }
            }
            view.subviews.forEach(walk)
        }
        walk(self)
    }
}
#endif

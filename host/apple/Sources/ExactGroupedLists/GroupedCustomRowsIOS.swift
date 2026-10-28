// A custom grouped row on UIKit (LLP 1084 D7), its views carried into a
// list cell, drawn as a standard cell beside it is: its leading symbol as
// UIKit's list draws a cell's image, and the line under it starting under
// its content, where a standard cell's starts under its text.
import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

/// What a custom row draws as a standard cell would: the leading symbol it
/// marked (`NodeView.listSymbol`), what it presses, marked as a cell's
/// (`NodeView.listRow`), and where its line started when last carried, to
/// know when it moves.
struct CustomLook {
    weak var symbol: NodeView?
    weak var press: NodeView?
    var line: CGFloat?
}

extension GroupedListModel.Look {
    /// A UIKit weight as CSS's number, to the nearest of CSS's hundreds.
    static func cssWeight(_ w: UIFont.Weight) -> Int {
        let steps: [(UIFont.Weight, Int)] = [(.ultraLight, 100), (.thin, 200), (.light, 300), (.regular, 400), (.medium, 500),
                                            (.semibold, 600), (.bold, 700), (.heavy, 800), (.black, 900)]
        return steps.min { abs($0.0.rawValue - w.rawValue) < abs($1.0.rawValue - w.rawValue) }?.1 ?? 400
    }
}

extension GroupedListHost {
    /// List `id`'s model, decoded again only when the kernel's JSON for it changed.
    func read(_ id: UInt32) -> GroupedListModel? {
        guard let json = presenter.session?.runtime.groupedList(id) else { return nil }
        if let known = reads[id], known.json == json { return known.model }
        var rows = reads[id]?.rows ?? [:]
        let model = GroupedListModel(json: json, rows: &rows)
        reads[id] = (json, model, rows)
        return model
    }
}

extension GroupedListView {
    /// Its cells, headers and separators follow the list's direction, as
    /// UIKit mirrors its own lists; a view's `semanticContentAttribute`
    /// reaches no subview, a trait does.
    func followDirection() {
        let direction: UISemanticContentAttribute = owner.style["direction"]?.string == "rtl" ? .forceRightToLeft : .forceLeftToRight
        guard collection.semanticContentAttribute != direction else { return }
        collection.semanticContentAttribute = direction
        collection.traitOverrides.layoutDirection = direction == .forceRightToLeft ? .rightToLeft : .leftToRight
    }

    /// Marks the custom row `id`'s leading symbol and notes its line, as
    /// it is carried into its cell; a mark it no longer has is cleared, and
    /// a line that moved lays the list out again after the update (or,
    /// carried as a cell shows, at the next projection sync).
    func look(_ id: UInt32, row: NodeView) {
        mark(id, leadingSymbol(of: row))
        let body = row.props["swipeContent"].flatMap { swipeBody(of: row, named: $0) } ?? row
        let press = Self.presses(body) ? body : link(in: body) ?? body
        if customLooks[id]?.press !== press {
            customLooks[id]?.press?.listRow = false
            press.listRow = true
            customLooks[id]?.press = press
        }
        let line = lineStart(id)
        // The separator handler read the same line as the layout ran: only
        // a line that moved since it was noted lays the list out again.
        if let noted = customLooks[id]?.line, noted != line { resized = true }
        customLooks[id]?.line = line
    }

    /// The row's own `link` when the row is not one, beside its buttons (an
    /// episode's link and its Mark and Add buttons, a ticker's star and its
    /// link): the cell's main press, which UIKit makes a button. A link
    /// inside another control is that control's.
    private func link(in row: NodeView) -> NodeView? {
        var level = [row]
        while !level.isEmpty {
            let next = level.flatMap { $0.container.subviews.compactMap { $0 as? NodeView } }
            if let found = next.first(where: { $0.props["accessibilityRole"] == "link" }) { return found }
            level = next.filter { !Self.presses($0) }
        }
        return nil
    }
    private static func presses(_ node: NodeView) -> Bool {
        node.kind == "button" || ["button", "link", "checkbox", "radio", "switch"].contains(node.props["accessibilityRole"] ?? "")
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
        let look = customLooks.removeValue(forKey: id)
        look?.symbol?.listSymbol = false
        look?.press?.listRow = false
    }

    /// The row's first shown child when it is the sheet's leading symbol: a
    /// `symbol:` image no accessory names, sized and fitted by the sheet,
    /// so an image whose size or fit its author wrote at all, or one a
    /// column row holds, is left as laid out (`wrote`: the sheet's rows are
    /// the compiler's).
    private func leadingSymbol(of row: NodeView) -> NodeView? {
        let first = row.container.subviews.lazy.compactMap { $0 as? NodeView }.first { $0.style["display"]?.string != "none" }
        guard let image = first, image.kind == "image", (image.props["imageSource"] ?? "").hasPrefix("symbol:"),
              !["chevron.forward", "chevron.right", "checkmark"].contains(image.props["symbolName"] ?? ""),
              image.style["object_fit"]?.string == "contain", !["object_fit", "width", "height"].contains(where: image.wrote) else { return nil }
        return image
    }

    /// Where the custom row `id`'s line starts from its cell's leading edge,
    /// read live, so a row not yet carried has it too: at its content box
    /// (the row's inset, past a leading symbol), the row at the place it is
    /// carried to (`carry`), as the kernel places the line where the sheet
    /// draws it (`Kernel::grouped_separator`). Right to left the leading
    /// edge is the right, where the kernel mirrors the sheet's insets.
    /// A `delete=` row is a swipe scroll whose content holds the row's
    /// insets (`swipe_rows::grouped`): its line starts at that content's
    /// box, past a leading symbol there, at the text as UIKit's does with
    /// an image, not at the card's edge.
    func lineStart(_ id: UInt32) -> CGFloat? {
        guard let row = host.presenter.views[id] else { return nil }
        let x = row.grid.map { PixelGrid.round($0.layout.minX, scale: host.presenter.gridScale) } ?? carried[id]?.frame.minX ?? row.frame.minX
        var box = row.contentBox().offsetBy(dx: -row.bounds.minX, dy: 0)
        if let body = row.props["swipeContent"].flatMap({ swipeBody(of: row, named: $0) }) {
            var content = body.contentBox()
            let shown = body.container.subviews.compactMap { $0 as? NodeView }.filter { $0.style["display"]?.string != "none" }
            if shown.count > 1, leadingSymbol(of: body) === shown[0] {
                let text = body.container.convert(shown[1].frame, to: body)
                if collection.effectiveUserInterfaceLayoutDirection == .rightToLeft {
                    content.size.width = max(0, text.maxX - content.minX)
                } else {
                    content = CGRect(x: text.minX, y: content.minY, width: max(0, content.maxX - text.minX), height: content.height)
                }
            }
            box = body.convert(content, to: row).offsetBy(dx: -row.bounds.minX, dy: 0)
        }
        if collection.effectiveUserInterfaceLayoutDirection == .rightToLeft { return row.bounds.width - box.maxX - x }
        return x + box.minX
    }

    /// The descendant of the swipe row `row` whose `id` is `name`: the
    /// content its `swipeContent` names, which resolves within the row.
    private func swipeBody(of row: NodeView, named name: String) -> NodeView? {
        func find(_ view: UIView) -> NodeView? {
            for child in view.subviews {
                if let node = child as? NodeView, node.props["id"] == name { return node }
                if let found = find(child) { return found }
            }
            return nil
        }
        return name.isEmpty ? nil : find(row)
    }
}
#endif

// HTML's top layer on Apple hosts: how a box is positioned for layout, as
// the kernel lays it out (`style::top_layer::layout_position`).

extension NodeView {
    /// How this box is positioned for layout: its `position` (the row is
    /// sent only when set), but `absolute` in the top layer, a popover its
    /// author didn't make `sticky` (a platform start's sticky, the neutral
    /// route header's, is no author's: `written` doesn't name it). Every
    /// read of the row that means layout goes through this;
    /// `scripts/position-reads.test.mjs` fails on a new direct read outside
    /// its allowlist.
    var layoutPosition: String { Self.layoutPosition(style, popover: props["popover"] != nil) }

    /// `layoutPosition` from a box's rows, before or after a change, and
    /// whether it is a popover.
    static func layoutPosition(_ style: NodeStyle, popover: Bool) -> String {
        let authored = style["position_type"]?.string ?? "static"
        let sticky = authored == "sticky" && style["written"]?.array?.contains(.string("position_type")) == true
        return popover && !sticky ? "absolute" : authored
    }
}

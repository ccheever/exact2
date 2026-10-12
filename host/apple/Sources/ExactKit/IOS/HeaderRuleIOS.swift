// @ref LLP 1115.001 D11 — what is header-shaped is one rule both hosts read:
// the kernel's (`kernel/src/bars.rs`), which the Android host reads from the
// kernel's own tree to make Material's top app bar. This host reads the same
// rule over its own views. They mirror that tree, and a query of the kernel
// would not serve here: a presenter fed a batch by hand (the tests') has no
// kernel behind it, and the bar is projected several times a batch. So the
// views' facts are handed to the rule as words (`exact_header_shape`), and
// it answers by their places in them.
#if os(iOS) || os(tvOS)
import CExact
import UIKit

/// A route's header as the shared rule reads it: its heading and level, its
/// buttons before and after the heading, all of them with the stack's Back
/// control, its search field, its tablist and the pressable around the
/// heading. `HeaderShape` is what UIKit's bar is made from it.
struct HeaderRule {
    let header: NodeView, heading: NodeView
    let level: Int
    let leading: [NodeView], trailing: [NodeView], items: [NodeView]
    let search: NodeView?, segments: NodeView?, tap: NodeView?

    /// What the rule asks of a node (`exact_kernel::bars::fact`).
    static func facts(_ node: NodeView, back: String?) -> UInt32 {
        var facts: UInt32 = 0
        if node.isHeading { facts |= 1 | UInt32(node.headingLevel ?? 0) << 16 }
        if node.kind == "input", node.props["type"] == "search" { facts |= 2 }
        if node.props["accessibilityRole"] == "tablist" { facts |= 4 }
        if node.handlers.contains("press") { facts |= 8 }
        if node.isButton, node.props["popovertarget"] != nil, node.props["popovertargetaction"] != "hide" { facts |= 16 }
        if back != nil, node.props["id"] == back { facts |= 32 }
        if node.props["semanticTag"] == "header" { facts |= 64 }
        if node.style["display"]?.string == "none" { facts |= 128 }
        return facts
    }

    /// `back` names the stack's Back control, which UIKit's back button
    /// stands for where there is one (`backIsUIKits`). `projected`: the
    /// header the route's bar is made from, so none for a route that says
    /// `navigationBar="none"` or a header hidden as authored.
    init?(route: NodeView, back: String?, backIsUIKits: Bool, projected: Bool) {
        guard let first = route.container.subviews.lazy.compactMap({ $0 as? NodeView }).first else { return nil }
        // The route, then its first child's subtree, in preorder: two words
        // a node, its facts and the count of its children.
        var nodes: [NodeView] = [route], words: [UInt32] = [route.props["navigationBar"] == "none" ? 256 : 0, 1]
        func add(_ node: NodeView) {
            let children = node.container.subviews.compactMap { $0 as? NodeView }
            nodes.append(node)
            words.append(Self.facts(node, back: back))
            words.append(UInt32(children.count))
            for child in children { add(child) }
        }
        add(first)
        var out = [UInt32](repeating: 0, count: 9 + 2 * nodes.count)
        let flags: UInt32 = (backIsUIKits ? 1 : 0) | (projected ? 2 : 0)
        let count = Int(exact_header_shape(words, words.count, flags, &out, out.count))
        guard count >= 9 else { return nil }
        let node = { (place: UInt32) in nodes[Int(place)] }
        let optional = { (place: UInt32) in place == 0 ? nil : nodes[Int(place) - 1] }
        header = node(out[0])
        heading = node(out[1])
        level = Int(out[2])
        tap = optional(out[3])
        search = optional(out[4])
        segments = optional(out[5])
        let before = Int(out[6]), after = Int(out[7]), all = Int(out[8])
        leading = out[9..<9 + before].map(node)
        trailing = out[9 + before..<9 + before + after].map(node)
        items = out[9 + before + after..<9 + before + after + all].map(node)
    }
}
#endif

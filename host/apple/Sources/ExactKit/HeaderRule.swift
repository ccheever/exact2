// @ref LLP 1115.001 D11 — what is header-shaped is one rule every host reads:
// the kernel's (`kernel/src/bars.rs`), which the Android host reads from the
// kernel's own tree to make Material's top app bar. The Apple hosts read the
// same rule over their own views: UIKit's navigation bar is made from it
// (`HeaderShape`, NavigationBarIOS.swift), and the Mac's title bar and
// toolbar (LLP 1115.002 D2, HeaderMac.swift). The views mirror that tree, and
// a query of the kernel would not serve here: a presenter fed a batch by hand
// (the tests') has no kernel behind it, and the bar is projected several
// times a batch. So the views' facts are handed to the rule as words
// (`exact_header_shape`), and it answers by their places in them.
import CExact
#if canImport(AppKit)
import AppKit
#else
import UIKit
#endif

/// A route's header as the shared rule reads it: its heading and level, its
/// items before and after the heading (buttons and readouts, `role="status"`),
/// its buttons with the stack's Back control, its search field, its tablist
/// and the pressable around the heading. Each platform's bar is made from it.
struct HeaderRule {
    let header: NodeView, heading: NodeView
    let level: Int
    let leading: [NodeView], trailing: [NodeView], items: [NodeView]
    let search: NodeView?, segments: NodeView?, tap: NodeView?

    /// The readouts among the leading and trailing items: nothing presses
    /// them (LLP 1075.003 §9.15).
    var readouts: [NodeView] { (leading + trailing).filter { node in !items.contains { $0 === node } } }

    /// The nodes handed to the shared rule, in all: what a projection walks,
    /// which the tests pin (a feed's rows are never among them).
    nonisolated(unsafe) static var walked = 0

    /// The route's first child is a shown `header` whose author wrote
    /// `appearance: none` (LLP 1075.003 §9.10): the rule makes no bar of it
    /// (`fact::AUTHORED`), and on iOS a pushed route with one gets no bar
    /// of the system Back alone either, since the author drew their own.
    static func keepsAuthoredHeader(_ route: NodeView) -> Bool {
        guard let header = route.container.subviews.lazy.compactMap({ $0 as? NodeView }).first,
              header.props["semanticTag"] == "header", header.style["display"]?.string != "none" else { return false }
        return header.wrote("appearance") && header.style["appearance"]?.string == "none"
    }

    /// What the rule asks of a node (`exact_kernel::bars::fact`).
    static func facts(_ node: NodeView, back: String?) -> UInt32 {
        var facts: UInt32 = 0
        if node.isHeading { facts |= 1 | UInt32(node.headingLevel ?? 0) << 16 }
        if node.kind == "input", node.props["type"] == "search" { facts |= 2 }
        if node.props["accessibilityRole"] == "tablist" { facts |= 4 }
        if node.props["accessibilityRole"] == "status" { facts |= 1024 }
        if node.handlers.contains("press") { facts |= 8 }
        if node.presenter?.invocation(of: node)?.opensPopover == true { facts |= 16 }
        if back != nil, node.props["id"] == back { facts |= 32 }
        if node.props["semanticTag"] == "header" { facts |= 64 }
        if node.style["display"]?.string == "none" { facts |= 128 }
        // Written, never implied (LLP 1075.003 §9.10): a header that says it keeps its authored bar.
        if node.wrote("appearance"), node.style["appearance"]?.string == "none" { facts |= 512 }
        return facts
    }

    /// `back` names the stack's Back control, which the platform's own Back
    /// stands for where there is one (`backIsPlatforms`: UIKit's back
    /// button, the Mac toolbar's Back). `projected`: the header the route's
    /// bar is made from, so none for a route that says
    /// `navigationBar="none"` or a header hidden as authored.
    init?(route: NodeView, back: String?, backIsPlatforms: Bool, projected: Bool) {
        // The rule makes a bar only of a first child that is a `header`
        // (`exact_kernel::bars::header_shape`); any other is answered here,
        // before walking it: a route whose first child is its feed would
        // hand every row's facts over at each projection, several a batch.
        guard let first = route.container.subviews.lazy.compactMap({ $0 as? NodeView }).first,
              first.props["semanticTag"] == "header" else { return nil }
        // The route, then its first child's subtree, in preorder: two words
        // a node, its facts and the count of its children.
        var nodes: [NodeView] = [route], words: [UInt32] = [route.props["navigationBar"] == "none" ? 256 : 0, 1]
        func add(_ node: NodeView) {
            let children = node.container.subviews.compactMap { $0 as? NodeView }
            nodes.append(node)
            Self.walked += 1
            words.append(Self.facts(node, back: back))
            words.append(UInt32(children.count))
            for child in children { add(child) }
        }
        add(first)
        var out = [UInt32](repeating: 0, count: 9 + 2 * nodes.count)
        let flags: UInt32 = (backIsPlatforms ? 1 : 0) | (projected ? 2 : 0)
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

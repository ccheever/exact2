// A box with no area can still show content: its children overflow it
// visibly (CSS Overflow 3, `overflow: visible`), and they paint and take
// touches. UIKit's accessibility leaves out a view whose bounds are empty
// together with everything under it, which kept a zero-height box's
// floating buttons from VoiceOver and XCTest while the web exposes them
// (#721). A view holding such a box lists its accessibility children
// itself, as UIKit would were the box a point tall: its subviews in
// UIKit's order, with that box's children in its place.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// A node's list, from its own subviews or, when its children are in a
    /// glass effect's content view (UIKit's class, which answers nothing
    /// here), from those.
    static func emptyBoxAccessibility(of node: NodeView) -> [Any]? {
        if let list = emptyBoxAccessibility(in: node) { return list }
        let holder = node.container
        guard holder !== node, !(holder is PlainView), !(holder is ScrollView), holder.accessibilityElements == nil else { return nil }
        return emptyBoxAccessibility(in: holder)
    }

    /// `holder`'s list when it holds such a box; nil leaves it to UIKit.
    static func emptyBoxAccessibility(in holder: UIView) -> [Any]? {
        guard holder.subviews.contains(where: { ($0 as? NodeView)?.passesAccessibilityThrough == true }) else { return nil }
        return ordered(holder.subviews)
    }

    /// The views in reading order, each such box replaced by the list it or
    /// its children's holder declares (an empty one too), else by its
    /// children in their order.
    private static func ordered(_ views: [UIView]) -> [Any] {
        readingOrder(views.filter { !$0.isHidden && !$0.accessibilityElementsHidden }).flatMap { view -> [Any] in
            guard let node = view as? NodeView, node.passesAccessibilityThrough else { return [view] }
            let holder = node.container
            return node.accessibilityElements ?? (holder === node ? nil : holder.accessibilityElements) ?? ordered(holder.subviews)
        }
    }

    /// UIKit's order for a container's children, as XCTest's snapshot of
    /// an iOS 27 app read it: by top edge, except that tops less than 8
    /// points apart are one line, read by leading edge (#721). A stable
    /// insertion, since that rule is not transitive.
    private static func readingOrder(_ views: [UIView]) -> [UIView] {
        guard views.count > 1 else { return views }
        let rtl = views[0].superview?.effectiveUserInterfaceLayoutDirection == .rightToLeft
        let frames = views.map { $0.convert($0.bounds, to: nil) }
        func before(_ a: Int, _ b: Int) -> Bool {
            let (fa, fb) = (frames[a], frames[b])
            if abs(fa.minY - fb.minY) >= 8 { return fa.minY < fb.minY }
            return rtl ? fa.maxX > fb.maxX : fa.minX < fb.minX
        }
        var order: [Int] = []
        for i in views.indices {
            var at = order.count
            while at > 0, before(i, order[at - 1]) { at -= 1 }
            order.insert(i, at: at)
        }
        return order.map { views[$0] }
    }

    /// An empty box whose children show outside it: no clip or clip path
    /// holds them, and it is neither an element (whose children are never
    /// visited) nor a modal boundary.
    private var passesAccessibilityThrough: Bool {
        bounds.isEmpty && !subviews.isEmpty && !isHidden && alpha > 0.01 && !accessibilityElementsHidden && !accessibilityViewIsModal
            && !clipsToBounds && clipBox == nil && clipPath == nil && scroll == nil && !isAccessibilityElement
    }
}
#endif

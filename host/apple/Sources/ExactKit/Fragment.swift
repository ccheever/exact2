// @ref LLP 1153 D5 — the runner's `scrollToFragment(visit, fragment)`, sent
// once a route's content has arrived: the browser's "scroll to the fragment"
// on a native host. The node with that HTML `id` in the visit's own route (a
// route retained beneath may hold the same id; the id as written, else
// percent-decoded) is aligned as `scrollIntoView(id)` with `block="start"`
// aligns it, every scroller above it moving, and on iOS VoiceOver's cursor
// goes to it, as a screen reader's starts from the target on the web. `top`,
// or no such node, takes the route's scrollers to their tops. A scroll by the
// person after the router asked for it (`fragmentArmed`) cancels it.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Presenter {
    func scrollToFragment(_ args: [Any]) {
        let visit = (args.first as? NSNumber).map { String($0.int64Value) } ?? ""
        let fragment = args.count > 1 ? args[1] as? String ?? "" : ""
        defer { fragmentArmed = false }
        guard fragmentArmed else {
            session?.log("fragment #\(fragment): the person scrolled first; the route stays where it is")
            return
        }
        let nodes = views.values.sorted { $0.id < $1.id }
        guard let route = nodes.first(where: { $0.props["navigationKey"] == visit && $0.props["navigationBack"] == nil }) else {
            session?.log("fragment #\(fragment): no route of visit \(visit) is on the screen")
            return
        }
        let id = fragment.removingPercentEncoding
        let target = [fragment, id ?? ""].lazy.filter { !$0.isEmpty }
            .compactMap { name in nodes.first { $0.props["id"] == name && $0.isDescendant(of: route) } }.first
        if let target, let name = target.props["id"] {
            scrollElementIntoView([name, "start", "nearest", "instant"], target: target)
            #if os(iOS)
            UIAccessibility.post(notification: .layoutChanged, argument: target)
            #endif
            return
        }
        if !fragment.isEmpty, id?.lowercased() != "top" {
            session?.log("fragment #\(fragment): no element in the route of visit \(visit) has that id; the route goes to its top")
        }
        for node in nodes where node === route || node.isDescendant(of: route) {
            #if os(macOS)
            guard let clip = node.scroll?.contentView, clip.bounds.origin.y != 0 else { continue }
            clip.scroll(to: NSPoint(x: clip.bounds.origin.x, y: 0))
            node.scroll?.reflectScrolledClipView(clip)
            #else
            guard let sv = node.scrollView else { continue }
            let top = CGPoint(x: sv.contentOffset.x, y: -sv.adjustedContentInset.top)
            if sv.contentOffset != top { sv.setContentOffset(top, animated: false) }
            #endif
        }
    }
}

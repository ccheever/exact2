// `share(title=, text=, url=)` (LLP 1069.003): the system share sheet, in
// ExactKit so an embedder (LLP 1031) gets it with the session. The runner
// rules first — bad data refused, and under the agent the request held for
// `tap @t shared|cancel`, never a sheet (D6) — then iOS opens a
// UIActivityViewController and macOS an NSSharingServicePicker, anchored to
// the node whose input ran the action (D3). The outcome is a journal line
// only (D2). Native hosts share from any action (Q2): a timer's share opens
// at the window's centre.
#if canImport(UIKit)
import UIKit
typealias ShareAnchorView = UIView
#else
import AppKit
typealias ShareAnchorView = NSView
#endif

extension ExactSession {
    func share(_ args: [Any], source: UInt32?) {
        var request: [String: Any] = ["command": "share", "agent": ExactEnv.agentMode]
        for (i, key) in ["title", "text", "url"].enumerated() where i < args.count {
            if let value = args[i] as? String { request[key] = value }
        }
        if let source { request["source"] = source }
        guard runtime.command(request)["present"] as? Bool == true else { return }
        var items: [Any] = []
        if let text = request["text"] as? String { items.append(text) }
        if let url = (request["url"] as? String).flatMap(URL.init(string:)) { items.append(url) }
        let (view, rect) = presenter.shareAnchor(source)
        guard let view, ShareSheet.present(items, title: request["title"] as? String, from: view, at: rect,
                                           outcome: { [weak self] line in self?.log(line) }) != nil
        else { log("share: refused: no window to present from"); return }
    }
}

extension Presenter {
    /// D3: the pressed node's view; a menu row's popover invoker when the
    /// row itself is not on screen (a menu item); else the window's content,
    /// centred (a timer, an answer, a menu-bar command).
    func shareAnchor(_ source: UInt32?) -> (ShareAnchorView?, CGRect) {
        func shown(_ v: ShareAnchorView) -> Bool {
            var at: ShareAnchorView? = v
            while let a = at { if a.isHidden { return false }; at = a.superview }
            return v.window != nil
        }
        if let id = source, let v = views[id] {
            if shown(v) { return (v, v.bounds) }
            var at: ShareAnchorView? = v.superview
            while let a = at, (a as? NodeView)?.props["popover"] == nil { at = a.superview }
            if let name = (at as? NodeView)?.props["id"],
               let invoker = carrying("popovertarget").first(where: { $0.props["popovertarget"] == name && shown($0) }) {
                return (invoker, invoker.bounds)
            }
        }
        #if canImport(UIKit)
        let content = views.values.first(where: { $0.window != nil })?.window?.rootViewController?.view
        #else
        let content = views.values.first(where: { $0.window != nil })?.window?.contentView
        #endif
        guard let content else { return (nil, .zero) }
        return (content, CGRect(x: content.bounds.midX, y: content.bounds.midY, width: 0, height: 0))
    }
}

enum ShareSheet {
    /// Open the platform's sheet with `items` off `view`'s `rect`; `outcome`
    /// gets the journal line. Returns what was shown, nil when nothing could
    /// present it.
    @discardableResult
    static func present(_ items: [Any], title: String?, from view: ShareAnchorView, at rect: CGRect,
                        outcome: @escaping (String) -> Void) -> AnyObject? {
        #if canImport(UIKit)
        var responder: UIResponder? = view
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        var controller = (responder as? UIViewController) ?? view.window?.rootViewController
        while let presented = controller?.presentedViewController, !presented.isBeingDismissed { controller = presented }
        guard let controller else { return nil }
        let sheet = UIActivityViewController(activityItems: items.map { ShareItem($0, title: title) }, applicationActivities: nil)
        sheet.popoverPresentationController?.sourceView = view
        sheet.popoverPresentationController?.sourceRect = rect
        sheet.completionWithItemsHandler = { _, completed, _, error in
            outcome(error.map { "share: refused: \($0.localizedDescription)" } ?? (completed ? "share: shared" : "share: dismissed"))
        }
        controller.present(sheet, animated: !ExactEnv.agentFreezes)
        return sheet
        #else
        _ = title  // the picker has no title; the services take the items
        guard view.window != nil else { return nil }
        let picker = NSSharingServicePicker(items: items)
        let delegate = ShareOutcome(outcome)
        picker.delegate = delegate
        objc_setAssociatedObject(picker, &ShareOutcome.key, delegate, .OBJC_ASSOCIATION_RETAIN)
        picker.show(relativeTo: rect, of: view, preferredEdge: .minY)
        return picker
        #endif
    }
}

#if canImport(UIKit)
/// One shared item; the title is the subject a mail or message takes.
private final class ShareItem: NSObject, UIActivityItemSource {
    let item: Any, title: String?
    init(_ item: Any, title: String?) { self.item = item; self.title = title }
    func activityViewControllerPlaceholderItem(_: UIActivityViewController) -> Any { item }
    func activityViewController(_: UIActivityViewController, itemForActivityType _: UIActivity.ActivityType?) -> Any? { item }
    func activityViewController(_: UIActivityViewController, subjectForActivityType _: UIActivity.ActivityType?) -> String { title ?? "" }
}
#else
/// The picker's outcome into the journal: no service chosen is `dismissed`;
/// a chosen one reports `shared`, or its failure.
private final class ShareOutcome: NSObject, NSSharingServicePickerDelegate, NSSharingServiceDelegate {
    nonisolated(unsafe) static var key = 0
    let outcome: (String) -> Void
    init(_ outcome: @escaping (String) -> Void) { self.outcome = outcome }
    func sharingServicePicker(_: NSSharingServicePicker, didChoose service: NSSharingService?) {
        if service == nil { outcome("share: dismissed") }
    }
    func sharingServicePicker(_: NSSharingServicePicker, delegateFor _: NSSharingService) -> NSSharingServiceDelegate? { self }
    func sharingService(_: NSSharingService, didShareItems _: [Any]) { outcome("share: shared") }
    func sharingService(_: NSSharingService, didFailToShareItems _: [Any], error: Error) {
        outcome((error as NSError).code == NSUserCancelledError ? "share: dismissed" : "share: refused: \(error.localizedDescription)")
    }
}
#endif

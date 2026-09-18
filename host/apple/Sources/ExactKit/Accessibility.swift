// HTML live regions and once-per-session autofocus, shared by Apple hosts.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension NodeView {
    var accessibleText: String {
        if let text = props["text"] { return text }
        let children = kind == "text" ? textChildren : container.subviews.compactMap { $0 as? NodeView }
        return children.map(\.accessibleText).filter { !$0.isEmpty }.joined(separator: " ")
    }
    var accessibleName: String { props["accessibilityLabel"] ?? accessibleText }
    var accessibilityVisible: Bool {
        guard paragraphOwner.window != nil, !inert else { return false }
        #if os(macOS)
        var ancestor: NSView? = paragraphOwner
        #else
        var ancestor: UIView? = paragraphOwner
        #endif
        while let view = ancestor {
            if view.isHidden || (view as? NodeView)?.inert == true { return false }
            ancestor = view.superview
        }
        return true
    }
}

extension Presenter {
    func syncAccessibility() {
        guard Thread.isMainThread else {
            DispatchQueue.main.async { [weak self] in self?.syncAccessibility() }
            return
        }
        for node in views.values.sorted(by: { $0.id < $1.id }) {
            if node.kind == "button" || node.props["accessibilityRole"] == "button" {
                #if os(macOS)
                node.setAccessibilityLabel(node.accessibleName)
                #else
                node.accessibilityLabel = node.accessibleName
                #endif
            }
            if let live = node.props["accessibilityLive"], live == "polite" || live == "assertive" {
                let text = node.accessibleText
                if node.accessibilityVisible,
                   let previous = node.liveText, previous != text, !text.isEmpty {
                    #if os(macOS)
                    NSAccessibility.post(element: node.paragraphOwner, notification: .announcementRequested,
                        userInfo: [.announcement: text, .priority: (live == "assertive" ? NSAccessibilityPriorityLevel.high : .low).rawValue])
                    #else
                    UIAccessibility.post(notification: .announcement, argument: NSAttributedString(string: text,
                        attributes: [.accessibilitySpeechQueueAnnouncement: live == "polite"]))
                    #endif
                }
                node.liveText = text
            } else { node.liveText = nil }
            guard !autofocusProcessed, node.props["autofocus"] == "true",
                  node.accessibilityVisible, !node.disabled, node.bounds.width > 0, node.bounds.height > 0 else { continue }
            // Mark before dispatch: a focus action can synchronously apply another batch.
            autofocusProcessed = true
            #if os(macOS)
            guard let window = node.window else { continue }
            let current = window.firstResponder
            guard current == nil || current === window || current === window.contentView || current === viewport || current === session?.view else { continue }
            let target: NSView = node.textArea ?? node.field ?? node
            if target.acceptsFirstResponder { _ = window.makeFirstResponder(target) }
            #else
            func hasFocus(_ view: UIView) -> Bool { view.isFirstResponder || view.subviews.contains(where: hasFocus) }
            guard let window = node.window, !hasFocus(window) else { continue }
            let target: UIResponder = node.textArea ?? node.field ?? node
            _ = target.becomeFirstResponder()
            #endif
        }
    }
}

extension Agent {
    func accessibilityTree(_ reply: [String: Any]) -> [String: Any] {
        var reply = reply
        let focus = (stateSections()["focus"] as? [String: Any])?["logical"] as? Int
        reply["nodes"] = (reply["nodes"] as? [[String: Any]] ?? []).map { row in
            var row = row
            if let id = row["id"] as? Int, let node = presenter.views[UInt32(id)] {
                row["focused"] = focus == id
                if node.props["accessibilityRole"] == "button" || node.props["accessibilityRole"] == "link" {
                    row["accessibleName"] = node.accessibleName
                }
            }
            return row
        }
        return reply
    }
}

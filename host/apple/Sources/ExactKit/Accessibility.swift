// HTML live regions and once-per-mounted-node autofocus, shared by Apple hosts.
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
        var ancestor = paragraphOwner
        while true {
            if ancestor.isHidden || ancestor.inert { return false }
            guard let parent = ancestor.superview as? NodeView else { return true }
            ancestor = parent
        }
    }
}

extension Presenter {
    func syncAccessibility() {
        var tookFocus = false
        for node in views.values.sorted(by: { $0.id < $1.id }) {
            if node.kind == "button" || node.props["accessibilityRole"] == "button" {
                #if os(macOS)
                node.setAccessibilityLabel(node.accessibleName)
                #else
                node.accessibilityLabel = node.accessibleName
                #endif
            }
            if let live = node.props["accessibilityLive"] {
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
            guard !node.didAutofocus, node.props["autofocus"] == "true",
                  node.accessibilityVisible, !node.disabled, node.bounds.width > 0, node.bounds.height > 0 else { continue }
            // Mark before dispatch: a focus action can synchronously apply another batch.
            node.didAutofocus = true
            if tookFocus { continue }
            #if os(macOS)
            let target: NSView = node.textArea ?? node.field ?? node
            tookFocus = target.acceptsFirstResponder && node.window?.makeFirstResponder(target) == true
            #else
            let target: UIResponder = node.textArea ?? node.field ?? node
            tookFocus = target.becomeFirstResponder()
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

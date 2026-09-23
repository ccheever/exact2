// HTML live regions and once-per-mounted-node autofocus, and focus across a
// carried restart, shared by Apple hosts.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension NodeView {
    var accessibleText: String {
        if let text = props["text"] { return text }
        if isParagraph { return inlineText.filter(\.paints).map(\.text).joined() }
        let children = container.subviews.compactMap { $0 as? NodeView }
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
        autofocusProcessed.formIntersection(Set(views.values.map { ObjectIdentifier($0) }))
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
            guard session?.autofocusHeld != true, !autofocusProcessed.contains(ObjectIdentifier(node)), node.props["autofocus"] == "true",
                  node.accessibilityVisible, !node.disabled, node.bounds.width > 0, node.bounds.height > 0 else { continue }
            #if os(macOS)
            guard let window = node.window else { continue }
            let current = window.firstResponder
            if (current as? NodeView)?.returnsPointerFocusToCanvas != true { autofocusProcessed.insert(ObjectIdentifier(node)) }
            guard current == nil || current === window || current === window.contentView || current === viewport || current === session?.view || (current as? NodeView)?.canvasInput != nil else { continue }
            // Blocked autofocus stays pending until the pointer hands focus back.
            autofocusProcessed.insert(ObjectIdentifier(node))
            let target: NSView = node.textArea ?? node.field ?? node
            if target.acceptsFirstResponder { _ = window.makeFirstResponder(target) }
            #else
            func hasFocus(_ view: UIView) -> Bool { (view.isFirstResponder && (view as? NodeView)?.canvasInput == nil) || view.subviews.contains(where: hasFocus) }
            if !views.values.contains(where: { $0.isFirstResponder && $0.returnsPointerFocusToCanvas }) { autofocusProcessed.insert(ObjectIdentifier(node)) }
            guard let window = node.window, !hasFocus(window) else { continue }
            autofocusProcessed.insert(ObjectIdentifier(node))
            let target: UIResponder = node.textArea ?? node.field ?? node
            _ = target.becomeFirstResponder()
            #endif
        }
    }
}

/// A restart with carried state (a dev reload, a delivered update) replaces
/// every view. Focus follows the focused node's place in the runner's tree —
/// its index among its siblings at each level, and its type — and the
/// restarted tree autofocuses nothing; a node mounted later still may.
struct FocusPlace: Equatable {
    let path: [Int]
    let type: String
}

/// The runner's `tree` read, as parents, children and types.
private struct FocusTree {
    let roots: [UInt32]
    let nodes: [UInt32: (parent: UInt32?, type: String, children: [UInt32])]
    init?(_ json: String) {
        guard let object = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any],
              let roots = object["roots"] as? [Int], let rows = object["nodes"] as? [[String: Any]] else { return nil }
        self.roots = roots.map { UInt32($0) }
        var nodes: [UInt32: (parent: UInt32?, type: String, children: [UInt32])] = [:]
        for row in rows {
            guard let id = row["id"] as? Int, let type = row["type"] as? String else { continue }
            nodes[UInt32(id)] = ((row["parent"] as? Int).map { UInt32($0) }, type, (row["children"] as? [Int] ?? []).map { UInt32($0) })
        }
        self.nodes = nodes
    }
    func place(of id: UInt32) -> FocusPlace? {
        guard let type = nodes[id]?.type else { return nil }
        var path: [Int] = [], at: UInt32? = id
        while let current = at, let node = nodes[current] {
            let siblings = node.parent.map { nodes[$0]?.children ?? [] } ?? roots
            guard let index = siblings.firstIndex(of: current) else { return nil }
            path.insert(index, at: 0)
            at = node.parent
        }
        return FocusPlace(path: path, type: type)
    }
    func view(at place: FocusPlace) -> UInt32? {
        var id: UInt32?, ids = roots
        for index in place.path {
            guard index < ids.count else { return nil }
            id = ids[index]
            ids = nodes[ids[index]]?.children ?? []
        }
        return id.flatMap { nodes[$0]?.type == place.type ? $0 : nil }
    }
}

extension Presenter {
    /// The node holding the focus: itself, its field (through its editor on
    /// AppKit) or its text area.
    var focusedNode: NodeView? {
        views.values.filter { node in
            #if os(macOS)
            guard let responder = node.window?.firstResponder else { return false }
            return responder === node || responder === node.textArea || node.field?.currentEditor().map { responder === $0 } == true
            #else
            return node.isFirstResponder || node.field?.isFirstResponder == true || node.textArea?.isFirstResponder == true
            #endif
        }.min(by: { $0.id < $1.id })
    }
    func focusPlace(tree json: String) -> FocusPlace? {
        focusedNode.flatMap { FocusTree(json)?.place(of: $0.id) }
    }
    func restoreFocus(_ kept: FocusPlace?, tree json: String) {
        for node in views.values where node.props["autofocus"] == "true" { autofocusProcessed.insert(ObjectIdentifier(node)) }
        guard let kept, let id = FocusTree(json)?.view(at: kept), let node = views[id], node.accessibilityVisible,
              !node.disabled, node.bounds.width > 0, node.bounds.height > 0 else { return }
        #if os(macOS)
        let target: NSView = node.textArea ?? node.field ?? node
        if target.acceptsFirstResponder { _ = node.window?.makeFirstResponder(target) }
        #else
        let target: UIResponder = node.textArea ?? node.field ?? node
        _ = target.becomeFirstResponder()
        #endif
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
                if node.placedAncestor != nil {
                    #if os(macOS)
                    let frame = node.accessibilityFrame()
                    #else
                    let frame = node.accessibilityFrame
                    #endif
                    row["accessibilityFrame"] = [frame.minX, frame.minY, frame.width, frame.height]
                    row["accessibilityHidden"] = node.placedAncestor?.placementHidden == true
                }
                if node.props["accessibilityRole"] == "button" || node.props["accessibilityRole"] == "link" {
                    row["accessibleName"] = node.accessibleName
                }
            }
            return row
        }
        return reply
    }
}

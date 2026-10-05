// HTML live regions and once-per-mounted-node autofocus, and focus across a
// carried restart, shared by Apple hosts.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension NodeView {
    /// A button: a custom one (`kind == "button"`) or a native one, under any
    /// role (LLP 1069.011.000 D1).
    var isButton: Bool { kind == "button" || isNativeButton }
    /// Its face, as the kernel reads it (LLP 1069.011.000 D1); nil when the
    /// presenter cannot read one.
    var face: ButtonFace? { presenter?.buttonFace?(id) }

    var accessibleText: String {
        if let text = props["text"] { return text }
        // A native button's children are its face, not views (LLP 1069.011 D5),
        // read from the kernel, current on its first batch (LLP 1069.011.000 D1).
        if isNativeButton { return face?.title ?? "" }
        if isParagraph { return inlineText.filter(\.paints).map(\.text).joined() }
        // accname: an `aria-hidden` child names nothing (habits F16: a tab's icon glyph).
        let children = container.subviews.compactMap { $0 as? NodeView }.filter { $0.props["accessibilityElementsHidden"] != "true" }
        return children.map(\.accessibleText).filter { !$0.isEmpty }.joined(separator: " ")
    }
    /// Its `aria-labelledby` text, else its `aria-label`, unless empty
    /// (accname: an empty label names nothing).
    var authoredLabel: String? { labelledBy ?? props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 } }
    /// accname's first step (ledger2 Rough 3): the text of the elements
    /// `aria-labelledby` names, in order — each by its own label or content,
    /// never its own `aria-labelledby`, as accname does not recurse.
    var labelledBy: String? {
        guard let refs = props["accessibilityLabelledBy"], let presenter else { return nil }
        let text = refs.split(separator: " ").compactMap { presenter.chrome.named[String($0)]?.min().flatMap { presenter.views[$0] } }
            .map { $0.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 } ?? $0.accessibleText }
            .filter { !$0.isEmpty }.joined(separator: " ")
        return text.isEmpty ? nil : text
    }
    var accessibleName: String { authoredLabel ?? accessibleText }
    /// A button to assistive technology: a `button`, or a box whose ARIA
    /// role is `button` or `link`, as the web's tree has a `div` with one
    /// (chat F14: a message bubble with `role="button"` was no button natively).
    /// A box with a checkable role is one too: the web's tree has a `div`
    /// with `role="checkbox"` as a checkbox (habits F16).
    var actsAsButton: Bool { kind == "button" || kind == "view" && ["button", "link", "checkbox", "radio", "switch"].contains(props["accessibilityRole"] ?? "") }
    /// ARIA's checkable roles (Core-AAM): `checkbox`, `radio` and `switch`,
    /// their state `aria-checked`; nil for any other role (onboarding F16:
    /// a `button role="radio"` read as a plain button on macOS).
    var checkedRole: (role: String, checked: Bool)? {
        guard let role = props["accessibilityRole"], ["checkbox", "radio", "switch"].contains(role) else { return nil }
        return (role, props["accessibilityChecked"] == "true")
    }
    /// ARIA `aria-pressed` on a button: its toggle state, `true`, `false` or
    /// `mixed`; nil when it is no toggle (absent, another word, another role).
    var pressedState: String? {
        guard [nil, "button"].contains(props["accessibilityRole"]),
              let pressed = props["accessibilityPressed"], ["true", "false", "mixed"].contains(pressed) else { return nil }
        return pressed
    }
    /// accname's description (onboarding F22): the text of the elements
    /// `aria-describedby` names, in order, else `aria-description`'s words.
    var accessibleDescription: String? {
        if let refs = props["accessibilityDescribedBy"], let presenter {
            let text = refs.split(separator: " ").compactMap { presenter.chrome.named[String($0)]?.min().flatMap { presenter.views[$0] } }
                .map(\.accessibleName).filter { !$0.isEmpty }.joined(separator: " ")
            if !text.isEmpty { return text }
        }
        return props["accessibilityHint"].flatMap { $0.isEmpty ? nil : $0 }
    }
    /// The ARIA states AppKit has no property for, under the attribute
    /// names browsers serve them by: WebKit's `AXInvalid` (onboarding F22),
    /// Chromium's `AXHasPopup` and `AXPopupValue` (spreadsheet F20), WebKit's
    /// `AXARIACurrent` (Depot: a navigation link's `aria-current`). A
    /// `false` (or absent) state serves none.
    func ariaAttribute(_ name: String) -> Any? {
        switch name {
        case "AXInvalid": return props["accessibilityInvalid"].flatMap { ["", "false"].contains($0) ? nil : $0 }
        case "AXHasPopup": return props["accessibilityHasPopup"].flatMap { ["", "false"].contains($0) ? nil : true }
        case "AXPopupValue": return props["accessibilityHasPopup"].flatMap { ["", "false"].contains($0) ? nil : $0 == "true" ? "menu" : $0 }
        case "AXARIACurrent": return props["accessibilityCurrent"].flatMap { ["", "false"].contains($0) ? nil : $0 }
        default: return nil
        }
    }
    static let ariaAttributes = ["AXInvalid", "AXHasPopup", "AXPopupValue", "AXARIACurrent"]
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

extension NodeView {
    /// The object the platform serves for this node — its field, native
    /// control or text area, else itself — given its description (AXHelp,
    /// UIKit's hint) and, on AppKit, `aria-required` (AXRequired) and a
    /// field's name. Each AppKit write posts a notification, so only a
    /// change is written.
    func applyFormAccessibility() {
        let description = accessibleDescription
        #if os(macOS)
        // A control's typed values are what AppKit serves its cell.
        let target: NSView = field ?? presenter?.controls.controls[id] ?? textArea as NSView? ?? self
        let required = props["accessibilityRequired"] == "true"
        if target.accessibilityHelp() != description { target.setAccessibilityHelp(description) }
        if target.isAccessibilityRequired() != required { target.setAccessibilityRequired(required) }
        // An `input`'s `aria-label` names its field, as a text area's names it.
        if let field, field.accessibilityLabel() != authoredLabel { field.setAccessibilityLabel(authoredLabel) }
        #else
        // UIKit has no property for `aria-required`, `aria-invalid`, `aria-haspopup` or `aria-current`.
        let target: UIView = field ?? presenter?.controls.controls[id] ?? textArea as UIView? ?? self
        if target.accessibilityHint != description { target.accessibilityHint = description }
        #endif
    }
}

#if os(macOS)
/// A text field's cell, the object AppKit serves for an `input`: it adds
/// the ARIA states AppKit has no property for (`NodeView.ariaAttribute`),
/// read from the node that holds the field.
final class FieldCell: NSTextFieldCell {
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + NodeView.ariaAttributes.filter { (controlView?.superview as? NodeView)?.ariaAttribute($0) != nil }.map { .init(rawValue: $0) }
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        (controlView?.superview as? NodeView)?.ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }
}
/// A password field's, as `FieldCell`.
final class SecureFieldCell: NSSecureTextFieldCell {
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + NodeView.ariaAttributes.filter { (controlView?.superview as? NodeView)?.ariaAttribute($0) != nil }.map { .init(rawValue: $0) }
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        (controlView?.superview as? NodeView)?.ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }
}

extension NSView {
    /// Core-AAM's toggle button: `AXCheckBox`, subrole `AXToggle`, value 0, 1
    /// or 2 (mixed); `role` when `pressed` is nil.
    func setAccessibilityToggle(_ pressed: String?, else role: NSAccessibility.Role) {
        setAccessibilityRole(pressed == nil ? role : .checkBox)
        setAccessibilitySubrole(pressed == nil ? nil : .toggle)
        setAccessibilityValue(pressed.map { $0 == "mixed" ? 2 : $0 == "true" ? 1 : 0 })
    }
    /// Core-AAM's checkable roles: `checkbox` is `AXCheckBox`, `switch` an
    /// `AXCheckBox` whose subrole is `AXSwitch`, `radio` an `AXRadioButton`;
    /// the value 1 when checked, else 0.
    func setAccessibilityChecked(_ role: String, _ checked: Bool) {
        setAccessibilityRole(role == "radio" ? .radioButton : .checkBox)
        setAccessibilitySubrole(role == "switch" ? .switch : nil)
        setAccessibilityValue(checked ? 1 : 0)
    }
}
#else
extension UIView {
    /// A toggle button as UIKit has one: the `toggleButton` trait, and the
    /// value VoiceOver reads as its state ("1" on, "0" off, "2" mixed).
    func setAccessibilityToggle(_ pressed: String?) {
        if pressed != nil { accessibilityTraits.insert(.toggleButton) } else { accessibilityTraits.remove(.toggleButton) }
        accessibilityValue = pressed.map { $0 == "mixed" ? "2" : $0 == "true" ? "1" : "0" }
    }
    /// A checkable role as Safari's VoiceOver reads one: a checkbox or
    /// switch is a button whose value is `checked` or `unchecked` (as the
    /// native checkbox, `ControlsIOS`); a radio is a button, selected while
    /// checked. Called after `setAccessibilityToggle`, which it overrides.
    func setAccessibilityChecked(_ role: String, _ checked: Bool) {
        accessibilityTraits.remove(.toggleButton)
        if role == "radio" {
            accessibilityValue = nil
            if checked { accessibilityTraits.insert(.selected) }
        } else {
            accessibilityValue = checked ? "checked" : "unchecked"
        }
    }
}
#endif

/// What an authored tab shows as one segment of the system's segmented
/// control (LLP 1035.001 D10): its one image, or its words.
enum SegmentFace: Equatable { case image(NodeView), symbol(String), title(String) }

extension NodeView {
    /// How this tab shows as a segment, or nil when a segment cannot show
    /// what was authored: exactly one image child is the segment's image;
    /// text alone, whose words are its accessible name, is its title. An
    /// icon beside a label, a badge, or any other node keeps the authored
    /// rendering — the web's, where a role never changes what is drawn.
    var segmentFace: SegmentFace? {
        // A native button's face, not views (LLP 1069.011.000 D4): a symbol
        // alone (named by its label), or a title its label agrees with.
        if isNativeButton {
            guard let face, face.fits else { return nil }
            if let symbol = face.symbol, face.title == nil { return .symbol(symbol) }
            if let title = face.title, face.symbol == nil, (props["accessibilityLabel"] ?? title) == title { return .title(title) }
            return nil
        }
        let children = container.subviews.compactMap { $0 as? NodeView }
        // An `aria-hidden` child (an icon glyph) is drawn but not named: no
        // segment shows one thing and is named another (habits F16).
        if children.contains(where: { $0.props["accessibilityElementsHidden"] == "true" }) { return nil }
        if children.count == 1, children[0].kind == "image" { return .image(children[0]) }
        let text = accessibleText
        guard !children.isEmpty, !text.isEmpty, children.allSatisfy(\.isParagraph),
              (props["accessibilityLabel"] ?? text) == text else { return nil }
        return .title(text)
    }
}

extension Presenter {
    /// Names, live regions and autofocus. `changed` limits the pass to the
    /// views a batch touched and their ancestors — a button's name reads its
    /// subtree — plus every live region and pending autofocus; nil reads
    /// every view.
    func syncAccessibility(changed: Set<UInt32>? = nil) {
        guard Thread.isMainThread else {
            DispatchQueue.main.async { [weak self] in self?.syncAccessibility() }
            return
        }
        let nodes: [NodeView]
        if let changed {
            let indexed = chrome.ids("accessibilityLive").union(chrome.ids("autofocus"))
            nodes = changed.union(indexed).compactMap { views[$0] }.sorted { $0.id < $1.id }
        } else {
            autofocusProcessed.formIntersection(Set(views.values.map { ObjectIdentifier($0) }))
            nodes = views.values.sorted(by: { $0.id < $1.id })
        }
        // A description reads the text of the elements it names, wherever they changed.
        let described = changed == nil ? [] : chrome.ids("accessibilityDescribedBy").subtracting(nodes.map(\.id)).compactMap { views[$0] }
        for node in described { node.applyFormAccessibility() }
        // A name read from other elements follows their text (ledger2 Rough 3).
        for node in chrome.ids("accessibilityLabelledBy").compactMap({ views[$0] }) where !node.isNativeButton {
            #if os(macOS)
            if node.accessibilityLabel() != node.accessibleName { node.setAccessibilityLabel(node.accessibleName) }
            #else
            if node.accessibilityLabel != node.accessibleName { node.accessibilityLabel = node.accessibleName }
            #endif
        }
        for node in nodes {
            node.applyFormAccessibility()
            // A native button's control is its accessibility element (LLP 1069.011 D4).
            if (node.actsAsButton || node.props["accessibilityRole"] == "button") && !node.isNativeButton {
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
            // The session's own view holding the focus for its shortcuts (ShortcutsIOS) is no focus a node took.
            func hasFocus(_ view: UIView) -> Bool { (view.isFirstResponder && (view as? NodeView)?.canvasInput == nil && !(view is ExactView)) || view.subviews.contains(where: hasFocus) }
            if !views.values.contains(where: { $0.isFirstResponder && $0.returnsPointerFocusToCanvas }) { autofocusProcessed.insert(ObjectIdentifier(node)) }
            guard let window = node.window, !hasFocus(window) else { continue }
            autofocusProcessed.insert(ObjectIdentifier(node))
            let target: UIResponder = node.textArea ?? node.field ?? node
            _ = target.becomeFirstResponder()
            #endif
        }
        #if os(iOS) || os(tvOS)
        syncModal()
        #endif
    }
    #if os(iOS) || os(tvOS)
    /// `aria-modal` (LLP 1080.003), after a batch and after a native
    /// transition settles. A view is modal while its prop is true and it is
    /// exposed: attached, displayed, not hidden, inert or leaving, with no
    /// accessibility-hidden ancestor and no visible sibling painted over it (a
    /// sheet's stack, say). UIKit hides a modal view's siblings
    /// (`accessibilityViewIsModal`), other modal views among them, so of
    /// siblings only the frontmost is modal, and one inside a branch another
    /// modal hides is not. VoiceOver moves into the innermost modal when that
    /// changes, or back to the screen, once no transition is running.
    func syncModal() {
        let candidates = chrome.ids("accessibilityModal").sorted().compactMap { views[$0] }
        func ancestors(_ view: UIView) -> [UIView] { sequence(first: view, next: \.superview).map { $0 } }
        func inFront(_ a: UIView, of b: UIView) -> Bool {
            if a.layer.zPosition != b.layer.zPosition { return a.layer.zPosition > b.layer.zPosition }
            let order = a.superview?.subviews ?? []
            return (order.firstIndex(of: a) ?? 0) > (order.firstIndex(of: b) ?? 0)
        }
        // A leaving view (hidden from accessibility as its exit starts) covers nothing.
        func painted(_ view: UIView) -> Bool {
            !view.isHidden && view.alpha > 0 && !view.accessibilityElementsHidden && (view as? NodeView)?.style["display"]?.string != "none"
        }
        // A cover is content: a node, or a controller's view (a sheet's
        // stack), not the host's own helper layers.
        func covered(_ view: UIView) -> Bool {
            view.superview?.subviews.contains { $0 !== view && ($0 is NodeView || $0.next is UIViewController) && painted($0)
                && inFront($0, of: view) && $0.frame.intersects(view.frame) } == true
        }
        func exposed(_ view: NodeView) -> Bool {
            guard view.props["accessibilityModal"] == "true", view.accessibilityVisible else { return false }
            // Hidden, undisplayed or painted over, it or any ancestor (a sheet's stack beside a root overlay).
            var below = true // covers are judged inside the session's viewport
            for at in ancestors(view) {
                if at === viewport { below = false }
                if at.accessibilityElementsHidden || (at as? NodeView)?.style["display"]?.string == "none" || (below && covered(at)) { return false }
            }
            return true
        }
        // Shallowest first: a winner's siblings, and every branch they hold, are hidden.
        var winners: [NodeView] = []
        for view in candidates.filter(exposed).sorted(by: { ancestors($0).count < ancestors($1).count }) {
            let above = ancestors(view).dropFirst()
            if above.contains(where: { a in winners.contains { $0 !== a && $0.superview === a.superview } }) { continue }
            if let i = winners.firstIndex(where: { $0.superview === view.superview }) {
                if inFront(view, of: winners[i]) { winners[i] = view }
            } else { winners.append(view) }
        }
        for view in candidates + modalViews.allObjects {
            let modal = winners.contains { $0 === view }
            if view.accessibilityViewIsModal != modal { view.accessibilityViewIsModal = modal }
        }
        modalViews.removeAllObjects()
        for view in winners { modalViews.add(view) }
        guard !navigation.inTransition, !modals.inTransition else { return }
        let innermost = winners.max { (ancestors($0).count, $0.id) < (ancestors($1).count, $1.id) }
        let key = innermost.map { (id: $0.id, incarnation: $0.incarnation) }
        guard key?.id != announcedModal?.id || key?.incarnation != announcedModal?.incarnation else { return }
        announcedModal = key
        // nil: VoiceOver picks the first element it can reach, inside the modal while there is one.
        Self.postScreenChanged(nil)
    }
    #endif
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
    /// The host's own facts on plain `tree`: focus and an open popover.
    func decorateTree(_ reply: [String: Any]) -> [String: Any] {
        var reply = reply
        let focus = (stateSections()["focus"] as? [String: Any])?["logical"] as? Int
        reply["nodes"] = (reply["nodes"] as? [[String: Any]] ?? []).map { row in
            var row = row
            if let id = row["id"] as? Int, let node = presenter.views[UInt32(id)] {
                row["focused"] = focus == id
                if node.props["popover"] != nil { row["open"] = presenter.menus.isOpen(node) }
            }
            return row
        }
        return reply
    }
}

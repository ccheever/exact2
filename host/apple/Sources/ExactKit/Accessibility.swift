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
        if isParagraph { return visibleParagraphText }
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
    /// ARIA disabled applies to focusable descendants, without changing input.
    package var ariaUnavailable: Bool {
        #if os(macOS)
        var ancestor: NSView? = self
        #else
        var ancestor: UIView? = self
        #endif
        while let view = ancestor {
            if let value = (view as? NodeView)?.props["accessibilityDisabled"] { return value == "true" }
            #if os(macOS)
            ancestor = presenter?.menus.parent(of: view) ?? view.superview
            #else
            ancestor = presenter?.accessibilityParent(of: view) ?? view.superview
            #endif
        }
        return false
    }
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
    /// The ARIA states served under AppKit attribute names: `AXEnabled`
    /// for ARIA disabled, WebKit's `AXInvalid` (onboarding F22),
    /// Chromium's `AXHasPopup` and `AXPopupValue` (spreadsheet F20), WebKit's
    /// `AXARIACurrent` (Depot: a navigation link's `aria-current`). A
    /// `false` (or absent) state serves none.
    func ariaAttribute(_ name: String) -> Any? {
        switch name {
        case "AXEnabled": return ariaUnavailable ? false : nil
        case "AXInvalid": return props["accessibilityInvalid"].flatMap { ["", "false"].contains($0) ? nil : $0 }
        case "AXHasPopup": return props["accessibilityHasPopup"].flatMap { ["", "false"].contains($0) ? nil : true }
        case "AXPopupValue": return props["accessibilityHasPopup"].flatMap { ["", "false"].contains($0) ? nil : $0 == "true" ? "menu" : $0 }
        case "AXARIACurrent": return props["accessibilityCurrent"].flatMap { ["", "false"].contains($0) ? nil : $0 }
        default: return nil
        }
    }
    static let ariaAttributes = ["AXEnabled", "AXInvalid", "AXHasPopup", "AXPopupValue", "AXARIACurrent"]
    /// ARIA's live-region roles with an implicit `aria-live` (WAI-ARIA 1.2):
    /// `status` is polite, `alert` assertive.
    static let liveRoles = ["status": "polite", "alert": "assertive"]
    var liveRole: String? { props["accessibilityRole"].flatMap { Self.liveRoles[$0] == nil ? nil : $0 } }
    #if os(macOS)
    /// The ARIA roles WebKit serves as AppKit's group with the role as its
    /// subrole (Core-AAM): the live regions and the dialogs, a `dialog`
    /// element's implicit role among them (HTML-AAM).
    static let groupSubroles = ["status": "AXApplicationStatus", "alert": "AXApplicationAlert",
                                "dialog": "AXApplicationDialog", "alertdialog": "AXApplicationAlertDialog"]
    var groupSubrole: String? {
        (props["accessibilityRole"] ?? (props["semanticTag"] == "dialog" ? "dialog" : nil)).flatMap { Self.groupSubroles[$0] }
    }
    #endif
    /// How a live region speaks: its `aria-live`, else its role's; nil when
    /// `off` or neither.
    var livePoliteness: String? {
        let live = props["accessibilityLive"] ?? liveRole.flatMap { Self.liveRoles[$0] }
        return live == "polite" || live == "assertive" ? live : nil
    }
    /// Whether a live region speaks as its text becomes `text`: when it
    /// changed, and an `alert` as it appears, as a browser's alert event has
    /// it (Core-AAM); one there at launch is read with the screen.
    func liveSpeaks(_ text: String, launching: Bool) -> Bool {
        guard accessibilityVisible, !text.isEmpty else { return false }
        if let previous = liveText { return previous != text }
        return liveRole == "alert" && !launching
    }
    /// ARIA's range roles (LLP 1116 D8): a value is `aria-valuenow` between
    /// `aria-valuemin` and `aria-valuemax`, and the children are
    /// presentational, so the node is one element. A control's own view
    /// (a `progress`'s indicator) says its role and value itself.
    static let rangeRoles: Set<String> = ["progressbar", "slider", "meter", "scrollbar", "spinbutton"]
    var rangeRole: String? { kind == "control" ? nil : props["accessibilityRole"].flatMap { Self.rangeRoles.contains($0) ? $0 : nil } }
    /// A range role's numbers, ARIA's 0 and 100 where a bound is unsaid;
    /// nil with no readable `aria-valuenow` or another role.
    var rangeNumbers: (now: Double, min: Double, max: Double)? {
        let number = { (key: String) in self.props[key].flatMap { Double($0.trimmingCharacters(in: .whitespaces)) }.flatMap { $0.isFinite ? $0 : nil } }
        guard rangeRole != nil, let now = number("accessibilityValueNow") else { return nil }
        return (now, number("accessibilityValueMin") ?? 0, number("accessibilityValueMax") ?? 100)
    }
    /// What the platform speaks for a range role: `aria-valuetext`, else
    /// its value as a percentage of its bounds, as UIKit's progress view and
    /// slider speak theirs ("30%"); nil with neither.
    var rangeValueText: String? {
        if rangeRole != nil, let text = props["accessibilityValueText"], !text.isEmpty { return text }
        guard let n = rangeNumbers else { return nil }
        let fraction = n.max > n.min ? (Swift.min(Swift.max(n.now, n.min), n.max) - n.min) / (n.max - n.min) : 0
        return NumberFormatter.localizedString(from: NSNumber(value: fraction), number: .percent)
    }
    var accessibilityVisible: Bool {
        guard paragraphOwner.window != nil, !inert, accessibilityExposed else { return false }
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
        let enabled = !disabled && !ariaUnavailable
        if target.isAccessibilityEnabled() != enabled { target.setAccessibilityEnabled(enabled) }
        if target.accessibilityHelp() != description { target.setAccessibilityHelp(description) }
        if target.isAccessibilityRequired() != required { target.setAccessibilityRequired(required) }
        // An `input`'s `aria-label` names its field, as a text area's names it.
        if let field, field.accessibilityLabel() != authoredLabel { field.setAccessibilityLabel(authoredLabel) }
        #else
        // UIKit has no property for `aria-required`, `aria-invalid`, `aria-haspopup` or `aria-current`.
        let target: UIView = field ?? presenter?.controls.controls[id] ?? textArea as UIView? ?? self
        if disabled || ariaUnavailable { target.accessibilityTraits.insert(.notEnabled) }
        else { target.accessibilityTraits.remove(.notEnabled) }
        if target.accessibilityHint != description { target.accessibilityHint = description }
        #endif
    }
}

#if os(macOS)
/// A text field's cell, the object AppKit serves for an `input`: it adds
/// the ARIA states AppKit has no property for (`NodeView.ariaAttribute`),
/// read from the node that holds the field.
final class FieldCell: NSTextFieldCell {
    private var numberField: Field? {
        guard let field = controlView as? Field, field.accessibilityRole() == .incrementor else { return nil }
        return field
    }
    /// The session's editor, which tells the field's node what the window's does not (`FieldEditor`).
    override func fieldEditor(for controlView: NSView) -> NSTextView? {
        (controlView.superview as? NodeView)?.presenter?.fieldEditor ?? super.fieldEditor(for: controlView)
    }
    override func drawingRect(forBounds rect: NSRect) -> NSRect {
        (controlView?.superview as? NodeView)?.nativeEditorRect(in: rect) ?? super.drawingRect(forBounds: rect)
    }
    override func titleRect(forBounds rect: NSRect) -> NSRect {
        (controlView?.superview as? NodeView)?.nativeEditorRect(in: rect) ?? super.titleRect(forBounds: rect)
    }
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + NodeView.ariaAttributes.filter { (controlView?.superview as? NodeView)?.ariaAttribute($0) != nil }.map { .init(rawValue: $0) }
            + (numberField != nil ? [.minValue, .maxValue] : [])
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if let field = numberField {
            switch attribute {
            case .role: return NSAccessibility.Role.incrementor.rawValue
            case .minValue: return field.accessibilityMinValue()
            case .maxValue: return field.accessibilityMaxValue()
            default: break
            }
        }
        return (controlView?.superview as? NodeView)?.ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }
    override func accessibilityActionNames() -> [NSAccessibility.Action] {
        super.accessibilityActionNames() + (numberField != nil ? [.increment, .decrement] : [])
    }
    override func accessibilityPerformAction(_ action: NSAccessibility.Action) {
        if let field = numberField {
            if action == .increment { _ = field.accessibilityPerformIncrement(); return }
            if action == .decrement { _ = field.accessibilityPerformDecrement(); return }
        }
        super.accessibilityPerformAction(action)
    }
}
/// A password field's, as `FieldCell`.
final class SecureFieldCell: NSSecureTextFieldCell {
    override func drawingRect(forBounds rect: NSRect) -> NSRect {
        (controlView?.superview as? NodeView)?.nativeEditorRect(in: rect) ?? super.drawingRect(forBounds: rect)
    }
    override func titleRect(forBounds rect: NSRect) -> NSRect {
        (controlView?.superview as? NodeView)?.nativeEditorRect(in: rect) ?? super.titleRect(forBounds: rect)
    }
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + NodeView.ariaAttributes.filter { (controlView?.superview as? NodeView)?.ariaAttribute($0) != nil }.map { .init(rawValue: $0) }
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        (controlView?.superview as? NodeView)?.ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }
}

extension NodeView {
    /// What VoiceOver reaches, as the web's accessibility tree and iOS's
    /// traits have it: a pressable is a button — a link, checkbox, radio or
    /// switch when its role says so — a labelled image (or `role="img"`,
    /// an svg's) an image, and a `group` or `radiogroup` a group of its
    /// children. Headings are paragraphs (`updateTextAccessibility`); names
    /// come from `syncAccessibility`.
    func updateRoleAccessibility() {
        let role = props["accessibilityRole"]
        if actsAsButton {
            setAccessibilityElement(true)
            if let checked = checkedRole { setAccessibilityChecked(checked.role, checked.checked) }
            else { setAccessibilityToggle(pressedState, else: role == "link" ? .link : .button) }
            // A link's `AXURL` is its `href`, as a run's is (X78).
            let url = role == "link" ? props["href"].flatMap { URL(string: $0) } : nil
            if accessibilityURL() != url { setAccessibilityURL(url) }
            setAccessibilitySelected(props["accessibilitySelected"] == "true")
            if let expanded = props["accessibilityExpanded"] { setAccessibilityExpanded(expanded == "true") }
        } else if kind == "image" || role == "img" {
            let labelled = authoredLabel != nil
            setAccessibilityElement(labelled)
            setAccessibilityRole(labelled ? .image : nil)
        } else if role == "group" || role == "radiogroup" {
            setAccessibilityElement(true)
            setAccessibilityRole(role == "radiogroup" ? .radioGroup : .group)
            if accessibilitySubrole() != nil { setAccessibilitySubrole(nil) }
        } else if let subrole = groupSubrole.map(NSAccessibility.Subrole.init(rawValue:)) {
            // A live-region or dialog role as WebKit serves it (Core-AAM): a
            // group whose subrole is the role, named only by its author, a
            // paragraph's words its children (`textAccessibilityChildren`).
            setAccessibilityElement(true)
            if accessibilityRole() != .group { setAccessibilityRole(.group) }
            if accessibilitySubrole() != subrole { setAccessibilitySubrole(subrole) }
            if isParagraph, accessibilityValue() != nil { setAccessibilityValue(nil); setAccessibilityLabel(props["accessibilityLabel"]) }
        } else if let range = rangeRole {
            // A range role is one element, its value between its bounds and
            // its `aria-valuetext` the description, as WebKit serves a drawn
            // one (LLP 1116 D8). Only a change is written: each posts.
            let axRole: NSAccessibility.Role = ["slider": .slider, "meter": .levelIndicator, "scrollbar": .scrollBar, "spinbutton": .incrementor][range] ?? .progressIndicator
            setAccessibilityElement(true)
            if accessibilityRole() != axRole { setAccessibilityRole(axRole) }
            let n = rangeNumbers
            let now = n.map { NSNumber(value: $0.now) }, low = n.map { NSNumber(value: $0.min) }, high = n.map { NSNumber(value: $0.max) }
            if (accessibilityValue() as? NSNumber) != now { setAccessibilityValue(now) }
            if (accessibilityMinValue() as? NSNumber) != low { setAccessibilityMinValue(low) }
            if (accessibilityMaxValue() as? NSNumber) != high { setAccessibilityMaxValue(high) }
            let text = props["accessibilityValueText"].flatMap { $0.isEmpty ? nil : $0 }
            if accessibilityValueDescription() != text { setAccessibilityValueDescription(text) }
        } else if !isParagraph, super.isAccessibilityElement() {
            // None of these roles now (one removed at runtime): a plain view
            // again, as a new one is, without the role's subrole or values.
            // A paragraph is reset by `updateTextAccessibility`.
            setAccessibilityElement(false)
            setAccessibilityRole(nil)
            if accessibilitySubrole() != nil { setAccessibilitySubrole(nil) }
            if accessibilityValue() != nil { setAccessibilityValue(nil) }
            if accessibilityMinValue() != nil { setAccessibilityMinValue(nil) }
            if accessibilityMaxValue() != nil { setAccessibilityMaxValue(nil) }
            if accessibilityValueDescription() != nil { setAccessibilityValueDescription(nil) }
        }
    }
}

extension NodeView {
    /// VoiceOver's show menu (AXShowMenu, VO-⇧-M) is the ContextMenu key's
    /// request at this node (X84), as Chrome sends both through Blink's
    /// `ShowNonLocatedContextMenu`: the `contextmenu`, then the
    /// `contextPopover`'s menu, of the first node from this one up that
    /// answers (`contextTarget`; the event bubbles), at this one's drawn middle.
    package override func accessibilityPerformShowMenu() -> Bool {
        guard let target = showMenuTarget, let presenter, let point = presenter.contextPoint(self) else { return false }
        return presenter.contextMenu(target, at: point)
    }
    /// AppKit offers every action whose method the class implements; show
    /// menu only where something answers it, as AppKit offers it only on a
    /// view with a `menu` (no node has one).
    package override func isAccessibilitySelectorAllowed(_ selector: Selector) -> Bool {
        selector == #selector(accessibilityPerformShowMenu) ? showMenuTarget != nil : super.isAccessibilitySelectorAllowed(selector)
    }
    private var showMenuTarget: NodeView? {
        guard let presenter, presenter.views[id] === self, !disabled, !inert, field == nil, textArea == nil,
              let target = presenter.contextTarget(self), !target.disabled, !target.inert else { return nil }
        return target
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
    /// A checkable role as UIKit reads one (LLP 1115.000 D13.3): a checkbox
    /// or switch is a toggle, the `toggleButton` trait with UIKit's value,
    /// "1" or "0", which VoiceOver speaks in the reader's language (as the
    /// native checkbox, `ControlsIOS`); a radio is a button, selected while
    /// checked. Called after `setAccessibilityToggle`, which it overrides.
    func setAccessibilityChecked(_ role: String, _ checked: Bool) {
        if role == "radio" {
            accessibilityTraits.remove(.toggleButton)
            accessibilityValue = nil
            if checked { accessibilityTraits.insert(.selected) }
        } else {
            accessibilityTraits.insert(.toggleButton)
            accessibilityValue = checked ? "1" : "0"
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
    /// text alone is its title — the visible words, with an `aria-label`
    /// that says more (`Temp`, named `Temperature`) its accessibility label
    /// (LLP 1116 D6). An icon beside a label, a badge, or any other node
    /// keeps the authored rendering — the web's, where a role never changes
    /// what is drawn.
    var segmentFace: SegmentFace? {
        // A native button's face, not views (LLP 1069.011.000 D4): a symbol
        // alone (named by its label), or a title.
        if isNativeButton {
            guard let face, face.fits else { return nil }
            if let symbol = face.symbol, face.title == nil { return .symbol(symbol) }
            if let title = face.title, face.symbol == nil { return .title(title) }
            return nil
        }
        let children = container.subviews.compactMap { $0 as? NodeView }
        // An `aria-hidden` child (an icon glyph) is drawn but not named: no
        // segment shows one thing and is named another (habits F16).
        if children.contains(where: { $0.props["accessibilityElementsHidden"] == "true" }) { return nil }
        if children.count == 1, children[0].kind == "image" { return .image(children[0]) }
        let text = accessibleText
        guard !children.isEmpty, !text.isEmpty, children.allSatisfy(\.isParagraph) else { return nil }
        return .title(text)
    }
}

extension Presenter {
    #if !os(macOS)
    /// Native presentations carry views out of their authored ancestry.
    func accessibilityParent(of view: UIView) -> UIView? {
        menus.logicalParent(of: view) ?? groupedLists?.logicalParent(of: view)
            ?? (view as? NodeView).flatMap { navigation.logicalParent(of: $0) } ?? view.superview
    }
    #endif
    /// Names, live regions and autofocus. `changed` limits the pass to the
    /// views a batch touched and their ancestors — a button's name reads its
    /// subtree — plus every live region and pending autofocus; nil reads
    /// every view.
    func syncAccessibility(changed: Set<UInt32>? = nil, disabledRoots: Set<UInt32> = []) {
        guard Thread.isMainThread else {
            DispatchQueue.main.async { [weak self] in self?.syncAccessibility() }
            return
        }
        let nodes: [NodeView]
        if let changed {
            let indexed = chrome.ids("accessibilityLive").union(chrome.ids("role:status")).union(chrome.ids("role:alert")).union(chrome.ids("autofocus"))
            var ids = changed.union(indexed)
            if !disabledRoots.isEmpty {
                for node in views.values {
                    #if os(macOS)
                    var ancestor: NSView? = node
                    #else
                    var ancestor: UIView? = node
                    #endif
                    while let view = ancestor {
                        if let owner = view as? NodeView, disabledRoots.contains(owner.id) { ids.insert(node.id); break }
                        #if os(macOS)
                        ancestor = menus.parent(of: view)
                        #else
                        ancestor = accessibilityParent(of: view)
                        #endif
                    }
                }
            }
            nodes = ids.compactMap { views[$0] }.sorted { $0.id < $1.id }
        } else {
            autofocusProcessed.formIntersection(Set(views.values.map { ObjectIdentifier($0) }))
            nodes = views.values.sorted(by: { $0.id < $1.id })
        }
        // A description reads the text of the elements it names, wherever they changed.
        let described = changed == nil ? [] : chrome.ids("accessibilityDescribedBy").subtracting(nodes.map(\.id)).compactMap { views[$0] }
        for node in described { node.applyFormAccessibility() }
        // A name read from other elements follows their text (ledger2 Rough 3).
        for node in chrome.ids("accessibilityLabelledBy").compactMap({ views[$0] }) {
            if node.isNativeButton {
                #if os(macOS)
                guard let button = controls.controls[node.id] as? NativeButtonMac else { continue }
                let name = node.authoredLabel ?? button.written?.face.title
                if button.accessibilityLabel() != name { button.setAccessibilityLabel(name) }
                #else
                guard let button = controls.controls[node.id] as? NativeButtonIOS else { continue }
                let name = node.authoredLabel ?? button.written?.face.title
                if button.accessibilityLabel != name { button.accessibilityLabel = name }
                #endif
                // Keep the configuration guard current without rewriting the
                // face or restarting native animations for referenced text.
                button.written?.label = name
            } else {
                #if os(macOS)
                if node.accessibilityLabel() != node.accessibleName { node.setAccessibilityLabel(node.accessibleName) }
                #else
                if node.accessibilityLabel != node.accessibleName { node.accessibilityLabel = node.accessibleName }
                #endif
            }
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
            if let live = node.livePoliteness {
                let text = node.accessibleText
                if node.liveSpeaks(text, launching: launchAutofocusPending) {
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
            guard session?.autofocusHeld != true, launchAutofocusReleased || session?.booted != true, !autofocusProcessed.contains(ObjectIdentifier(node)), node.props["autofocus"] == "true",
                  node.accessibilityVisible, !node.disabled, node.bounds.width > 0, node.bounds.height > 0 else { continue }
            #if os(macOS)
            guard let window = node.window else { continue }
            let current = window.firstResponder
            if (current as? NodeView)?.returnsPointerFocusToCanvas != true { autofocusProcessed.insert(ObjectIdentifier(node)) }
            guard current == nil || current === window || current === window.contentView || current === viewport || current === session?.view || (current as? NodeView)?.canvasInput != nil else { continue }
            // Blocked autofocus stays pending until the pointer hands focus back.
            autofocusProcessed.insert(ObjectIdentifier(node))
            let target = keyView(of: node)
            if target.acceptsFirstResponder { _ = window.makeFirstResponder(target) }
            #else
            // The session's own view holding the focus for its shortcuts (ShortcutsIOS) is no focus a node took,
            // nor is a button's that came from a touch (Safari's buttons take none from a tap): the button that
            // opened a sheet must not keep the sheet's field from its autofocus. A button reached by Tab or
            // `focus(id)` keeps it.
            func touchFocused(_ view: UIView) -> Bool { (view as? NodeView).map { $0.isButton && $0.focusedByTouch } == true }
            func hasFocus(_ view: UIView) -> Bool { (view.isFirstResponder && (view as? NodeView)?.canvasInput == nil && !touchFocused(view) && !(view is ExactView)) || view.subviews.contains(where: hasFocus) }
            guard let window = node.window else { continue }
            autofocusProcessed.insert(ObjectIdentifier(node))
            guard !hasFocus(window) else { continue }
            let target: UIResponder = node.textArea ?? node.field ?? node
            node.focusedByTouch = false // the app's focus now, as Tab's and focus(id)'s
            _ = target.becomeFirstResponder()
            #endif
        }
        #if os(macOS)
        let modal = macAccessibilityModal.map(ObjectIdentifier.init)
        if modal != announcedModal {
            announcedModal = modal
            NSAccessibility.post(element: viewport, notification: .layoutChanged)
        }
        #else
        syncModal()
        #endif
    }
    #if os(macOS)
    var macModalAccessibilityChildren: [NodeView]? {
        guard let modal = macAccessibilityModal else { return nil }
        // In its sheet the dialog is that window's (LLP 1115.002 D7.2): the page behind it exposes nothing.
        if modal.window !== viewport.window { return [] }
        return [modal] + menus.presented(within: modal).filter { $0 !== modal && $0.window === viewport.window }
    }
    /// The frontmost exposed authored modal, or the dialog in HTML's top
    /// layer. The viewport serves it directly so AppKit cannot flatten a
    /// scroll or a native control beside it into the accessible page.
    var macAccessibilityModal: NodeView? {
        if let active = dialogs.active, active.accessibilityVisible,
           active.props["accessibilityElementsHidden"] != "true", !active.isAccessibilityHidden(),
           leaving[active.id] == nil { return active }
        func path(_ view: NSView) -> [NSView] {
            Array(sequence(first: view, next: \.superview).prefix { $0 !== viewport }.reversed())
        }
        func inFront(_ a: NSView, of b: NSView) -> Bool {
            if a.isDescendant(of: b) { return true }
            if b.isDescendant(of: a) { return false }
            for (left, right) in zip(path(a), path(b)) where left !== right {
                if left.layer?.zPosition != right.layer?.zPosition { return (left.layer?.zPosition ?? 0) > (right.layer?.zPosition ?? 0) }
                let siblings = left.superview?.subviews ?? []
                return (siblings.firstIndex(of: left) ?? 0) > (siblings.firstIndex(of: right) ?? 0)
            }
            return false
        }
        func exposed(_ node: NodeView) -> Bool {
            guard node.props["accessibilityModal"] == "true", node.accessibilityVisible,
                  !node.isAccessibilityHidden(), leaving[node.id] == nil else { return false }
            for view in sequence(first: node as NSView, next: \.superview) {
                if view.isAccessibilityHidden() { return false }
                guard let parent = view.superview else { break }
                if parent.subviews.contains(where: { sibling in
                    guard sibling !== view, let cover = sibling as? NodeView,
                          !cover.cssVisibilityHidden, !cover.isHidden, !cover.isAccessibilityHidden(),
                          !isLeaving(cover), cover.alphaValue == 1, let layer = cover.layer,
                          layer.opacity == 1, layer.backgroundColor?.alpha == 1,
                          layer.cornerRadius == 0, layer.mask == nil, CATransform3DIsIdentity(layer.transform)
                    else { return false }
                    // Only a painted opaque rectangle covering the entire
                    // dialog establishes occlusion. Intersecting boxes do not.
                    // AppKit's visibleRect can extend beyond an unclipped box.
                    return cover.convert(cover.visibleRect.intersection(cover.bounds), to: node).contains(node.bounds) && inFront(cover, of: view)
                }) { return false }
                if view === viewport { break }
            }
            return true
        }
        return chrome.ids("accessibilityModal").sorted().compactMap { views[$0] }.filter(exposed)
            .max { inFront($1, of: $0) }
    }
    #endif
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
            return keyTarget(responder) === node
            #else
            return node.isFirstResponder || node.field?.isFirstResponder == true || node.textArea?.isFirstResponder == true
            #endif
        }.min(by: { $0.id < $1.id })
    }
    func focusPlace(tree json: String) -> FocusPlace? {
        focusedNode.flatMap { FocusTree(json)?.place(of: $0.id) }
    }
    /// A booted session's launch autofocus has not run yet: the agent's settle waits for it.
    var launchAutofocusPending: Bool { session?.booted == true && !launchAutofocusReleased }
    /// Runs a booted session's first autofocus the turn after its first activation, as UIKit apps focus
    /// in `viewDidAppear`. Showing the iOS keyboard or AppKit field editor earlier delays that frame.
    func releaseLaunchAutofocus() {
        guard !launchAutofocusReleased else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, !launchAutofocusReleased, session?.state != .destroyed else { return }
            launchAutofocusReleased = true
            syncAccessibility()
            #if os(macOS)
            focusInitialKeyView() // LLP 1115.000 D7.5, after the author's `autofocus` had its say
            #endif
        }
    }

    func restoreFocus(_ kept: FocusPlace?, tree json: String) {
        // A restart that finds a focus ends the launch autofocus. One that finds none before the release leaves it pending.
        if kept != nil { launchAutofocusReleased = true }
        if launchAutofocusReleased { for node in views.values where node.props["autofocus"] == "true" { autofocusProcessed.insert(ObjectIdentifier(node)) } }
        guard let kept, let id = FocusTree(json)?.view(at: kept), let node = views[id], node.accessibilityVisible,
              !node.disabled, node.bounds.width > 0, node.bounds.height > 0 else { return }
        #if os(macOS)
        let target = keyView(of: node)
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
        #if os(iOS) || os(tvOS)
        // A header node the navigation bar leaves out (LLP 1116 D2).
        let unshown = presenter.navigation.unshownByBar()
        #endif
        reply["nodes"] = (reply["nodes"] as? [[String: Any]] ?? []).map { row in
            var row = row
            if let id = row["id"] as? Int, let node = presenter.views[UInt32(id)] {
                row["focused"] = focus == id
                if node.props["popover"] != nil { row["open"] = presenter.menus.isOpen(node) }
                #if os(iOS) || os(tvOS)
                if let why = unshown[node.id] { row["unshown"] = why }
                #endif
            }
            return row
        }
        return reply
    }
}
